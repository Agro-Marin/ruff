use ruff_db::files::{File, FilePath, system_path_to_file};
use ruff_db::system::SystemPath;
use ty_module_resolver::{
    ImportingFile, ModuleName, ResolverEnvironment, resolve_module, resolve_module_confident,
    resolve_real_module, resolve_real_module_confident,
};

use crate::ModuleDb;
use crate::collector::CollectedImport;

/// Collect all imports for a given Python file.
pub(crate) struct Resolver<'a> {
    db: &'a ModuleDb,
    file: Option<File>,
    environment: ResolverEnvironment<'a>,
}

impl<'a> Resolver<'a> {
    /// Initialize a [`Resolver`] with a given [`ModuleDb`].
    pub(crate) fn new(
        db: &'a ModuleDb,
        path: &SystemPath,
        environment: ResolverEnvironment<'a>,
    ) -> Self {
        let file = system_path_to_file(db, path).ok();
        Self {
            db,
            file,
            environment,
        }
    }

    /// Resolve the [`CollectedImport`] into a [`FilePath`].
    pub(crate) fn resolve(&self, import: CollectedImport) -> impl Iterator<Item = &'a FilePath> {
        match import {
            CollectedImport::Import(import) => {
                let file = self.resolve_module(&import);

                let source_file = file
                    .is_some_and(|file| file.extension() == Some("pyi"))
                    .then(|| self.resolve_real_module(&import))
                    .flatten();

                std::iter::once(file)
                    .chain(std::iter::once(source_file))
                    .flatten()
            }
            CollectedImport::ImportFrom(import) => {
                if let Some(file) = self.resolve_module(&import) {
                    let source_file = (file.extension() == Some("pyi"))
                        .then(|| self.resolve_real_module(&import))
                        .flatten();

                    return std::iter::once(Some(file))
                        .chain(std::iter::once(source_file))
                        .flatten();
                }

                let parent = import.parent();
                let file = parent
                    .as_ref()
                    .and_then(|parent| self.resolve_module(parent));

                let source_file = file
                    .is_some_and(|file| file.extension() == Some("pyi"))
                    .then(|| {
                        parent
                            .as_ref()
                            .and_then(|parent| self.resolve_real_module(parent))
                    })
                    .flatten();

                std::iter::once(file)
                    .chain(std::iter::once(source_file))
                    .flatten()
            }
            CollectedImport::StringImport(import, min_dots) => {
                let count = import.components().count();
                let (resolved, source_file) = import
                    .ancestors()
                    .take(count.saturating_sub(min_dots))
                    .find_map(|name| {
                        let file = self.resolve_module(&name)?;
                        if file.extension() == Some("pyi") {
                            Some((Some(file), self.resolve_real_module(&name)))
                        } else {
                            Some((Some(file), None))
                        }
                    })
                    .unwrap_or_default();

                std::iter::once(resolved)
                    .chain(std::iter::once(source_file))
                    .flatten()
            }
        }
    }

    /// Resolves a module name to a module.
    fn resolve_module(&self, module_name: &ModuleName) -> Option<&'a FilePath> {
        let module = if let Some(file) = self.file {
            resolve_module(
                self.db,
                ImportingFile::File(file, self.environment),
                module_name,
            )?
        } else {
            resolve_module_confident(self.db, self.environment, module_name)?
        };
        Some(module.file(self.db)?.path(self.db))
    }

    /// Resolves a module name to a module (stubs not allowed).
    fn resolve_real_module(&self, module_name: &ModuleName) -> Option<&'a FilePath> {
        let module = if let Some(file) = self.file {
            resolve_real_module(
                self.db,
                ImportingFile::File(file, self.environment),
                module_name,
            )?
        } else {
            resolve_real_module_confident(self.db, self.environment, module_name)?
        };
        Some(module.file(self.db)?.path(self.db))
    }
}
