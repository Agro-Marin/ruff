#![allow(
    clippy::disallowed_methods,
    reason = "This implementation is specific to real file systems."
)]

use notify::event::{CreateKind, MetadataKind, ModifyKind, RemoveKind, RenameMode};
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher as _, recommended_watcher};

use ruff_db::system::{SystemPath, SystemPathBuf};

use crate::watch::{ChangeEvent, ChangedKind, CreatedKind, DeletedKind, ExistingPathKind};

/// Creates a new watcher observing file system changes.
///
/// The watcher debounces events, but guarantees to send all changes eventually (even if the file system keeps changing).
pub fn directory_watcher<H>(handler: H) -> notify::Result<Watcher>
where
    H: EventHandler,
{
    let (sender, receiver) = crossbeam::channel::bounded(20);

    let debouncer = std::thread::Builder::new()
        .name("watcher::debouncer".to_string())
        .spawn(move || {
            for message in &receiver {
                let event = match message {
                    DebouncerMessage::Event(event) => event,
                    DebouncerMessage::Flush => {
                        continue;
                    }
                };

                let mut debouncer = Debouncer::default();

                debouncer.add_result(event);

                let batch_start = std::time::Instant::now();

                loop {
                    crossbeam::select! {
                        recv(receiver) -> message => {
                            match message {
                                Ok(DebouncerMessage::Event(event)) => {
                                    debouncer.add_result(event);

                                    if batch_start.elapsed() > std::time::Duration::from_secs(3) {
                                        break;
                                    }
                                }
                                Ok(DebouncerMessage::Flush) => {
                                    break;
                                }

                                Err(_) => {
                                    return;
                                }
                            }
                        },
                        default(std::time::Duration::from_millis(10)) => {
                            break;
                        }
                    }
                }

                let events = debouncer.into_events();

                if !events.is_empty() {
                    handler.handle(events);
                }
            }
        })
        .unwrap();

    let debouncer_sender = sender.clone();
    let watcher =
        recommended_watcher(move |event| sender.send(DebouncerMessage::Event(event)).unwrap())?;

    Ok(Watcher {
        inner: Some(WatcherInner {
            watcher,
            debouncer_sender,
            debouncer_thread: debouncer,
        }),
    })
}

#[derive(Debug)]
enum DebouncerMessage {
    /// A new file system event.
    Event(notify::Result<notify::Event>),

    Flush,
}

pub struct Watcher {
    inner: Option<WatcherInner>,
}

struct WatcherInner {
    watcher: RecommendedWatcher,
    debouncer_sender: crossbeam::channel::Sender<DebouncerMessage>,
    debouncer_thread: std::thread::JoinHandle<()>,
}

impl Watcher {
    /// Returns a transaction-like view for updating watched paths in one backend operation.
    pub(crate) fn paths_mut(&mut self) -> WatcherPathsMut<'_> {
        WatcherPathsMut {
            inner: self.inner_mut().watcher.paths_mut(),
        }
    }

    /// Stops the file watcher.
    ///
    /// Pending events will be discarded.
    ///
    /// The call blocks until the watcher has stopped.
    pub(crate) fn stop(mut self) {
        tracing::debug!("Stop file watcher");
        self.set_stop();
    }

    /// Flushes any pending events.
    pub(crate) fn flush(&self) {
        self.inner()
            .debouncer_sender
            .send(DebouncerMessage::Flush)
            .unwrap();
    }

    fn set_stop(&mut self) {
        if let Some(inner) = self.inner.take() {
            drop(inner.watcher);

            drop(inner.debouncer_sender);

            inner.debouncer_thread.join().unwrap();
        }
    }

    fn inner(&self) -> &WatcherInner {
        self.inner.as_ref().expect("Watcher to be running")
    }

    fn inner_mut(&mut self) -> &mut WatcherInner {
        self.inner.as_mut().expect("Watcher to be running")
    }
}

pub(crate) struct WatcherPathsMut<'a> {
    inner: Box<dyn notify::PathsMut + 'a>,
}

impl WatcherPathsMut<'_> {
    pub(crate) fn add(&mut self, path: &SystemPath) -> notify::Result<()> {
        tracing::debug!("Watching path: `{path}`");
        self.inner.add(path.as_std_path(), RecursiveMode::Recursive)
    }

    pub(crate) fn remove(&mut self, path: &SystemPath) -> notify::Result<()> {
        tracing::debug!("Unwatching path: `{path}`");
        self.inner.remove(path.as_std_path())
    }

    pub(crate) fn commit(self) -> notify::Result<()> {
        self.inner.commit()
    }
}

impl Drop for Watcher {
    fn drop(&mut self) {
        self.set_stop();
    }
}

#[derive(Default)]
struct Debouncer {
    events: Vec<ChangeEvent>,
    rescan_event: Option<ChangeEvent>,
}

impl Debouncer {
    fn add_result(&mut self, result: notify::Result<notify::Event>) {
        tracing::trace!("Handling file watcher event: {result:?}");
        match result {
            Ok(event) => self.add_event(event),
            Err(error) => self.add_error(error),
        }
    }

    #[expect(clippy::unused_self, clippy::needless_pass_by_value)]
    fn add_error(&mut self, error: notify::Error) {
        tracing::warn!("File watcher error: {error:?}");
    }

    fn add_event(&mut self, event: notify::Event) {
        if self.rescan_event.is_some() {
            return;
        }

        if event.need_rescan() || self.events.len() > 10000 {
            self.events = Vec::new();
            self.rescan_event = Some(ChangeEvent::Rescan);

            return;
        }

        let kind = event.kind;

        let Some(path) = event.paths.into_iter().next() else {
            tracing::debug!("Ignoring change event with kind '{kind:?}' without a path",);
            return;
        };

        let path = match SystemPathBuf::from_path_buf(path) {
            Ok(path) => path,
            Err(path) => {
                tracing::debug!(
                    "Ignore change to non-UTF8 path `{path}`: {kind:?}",
                    path = path.display()
                );

                return;
            }
        };

        let event = match kind {
            EventKind::Create(create) => {
                let kind = match create {
                    CreateKind::File => CreatedKind::File,
                    CreateKind::Folder => CreatedKind::Directory,
                    CreateKind::Any | CreateKind::Other => CreatedKind::from(
                        ExistingPathKind::from_io_metadata(&path.as_std_path().metadata()),
                    ),
                };

                ChangeEvent::Created { path, kind }
            }

            EventKind::Modify(modify) => match modify {
                ModifyKind::Metadata(metadata) => {
                    if ExistingPathKind::from_io_metadata(&path.as_std_path().metadata())
                        != ExistingPathKind::File
                    {
                        return;
                    }

                    match metadata {
                        MetadataKind::Any | MetadataKind::Permissions | MetadataKind::Other => {
                            ChangeEvent::Changed {
                                path,
                                kind: ChangedKind::FileMetadata,
                            }
                        }

                        MetadataKind::AccessTime
                        | MetadataKind::WriteTime
                        | MetadataKind::Ownership
                        | MetadataKind::Extended => {
                            return;
                        }
                    }
                }

                ModifyKind::Data(_) => ChangeEvent::Changed {
                    kind: ChangedKind::FileContent,
                    path,
                },

                ModifyKind::Name(rename) => match rename {
                    RenameMode::From => {
                        ChangeEvent::Deleted {
                            kind: DeletedKind::Any,
                            path,
                        }
                    }

                    RenameMode::To => ChangeEvent::Created {
                        kind: CreatedKind::from(ExistingPathKind::from_io_metadata(
                            &path.as_std_path().metadata(),
                        )),
                        path,
                    },

                    RenameMode::Both => {
                        return;
                    }

                    RenameMode::Other => {
                        return;
                    }

                    RenameMode::Any => {
                        if path.as_std_path().exists() {
                            let file_type =
                                ExistingPathKind::from_io_metadata(&path.as_std_path().metadata());

                            ChangeEvent::Created {
                                kind: file_type.into(),
                                path,
                            }
                        } else {
                            ChangeEvent::Deleted {
                                kind: DeletedKind::Any,
                                path,
                            }
                        }
                    }
                },
                ModifyKind::Other => {
                    return;
                }
                ModifyKind::Any => {
                    if !ExistingPathKind::from_io_metadata(&path.as_std_path().metadata()).is_file()
                    {
                        return;
                    }

                    ChangeEvent::Changed {
                        path,
                        kind: ChangedKind::Any,
                    }
                }
            },

            EventKind::Access(_) => {
                return;
            }

            EventKind::Remove(kind) => {
                let kind = match kind {
                    RemoveKind::File => DeletedKind::File,
                    RemoveKind::Folder => DeletedKind::Directory,
                    RemoveKind::Any | RemoveKind::Other => DeletedKind::Any,
                };

                ChangeEvent::Deleted { path, kind }
            }

            EventKind::Other => {
                return;
            }

            EventKind::Any => {
                tracing::debug!("Skipping any FS event for `{path}`");
                return;
            }
        };

        self.events.push(event);
    }

    fn into_events(self) -> Vec<ChangeEvent> {
        if let Some(rescan_event) = self.rescan_event {
            vec![rescan_event]
        } else {
            self.events
        }
    }
}

pub trait EventHandler: Send + 'static {
    fn handle(&self, changes: Vec<ChangeEvent>);
}

impl<F> EventHandler for F
where
    F: Fn(Vec<ChangeEvent>) + Send + 'static,
{
    fn handle(&self, changes: Vec<ChangeEvent>) {
        let f = self;
        f(changes);
    }
}
