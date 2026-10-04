use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

/// A test file: a path part named `tests`, or one starting with `test_`.
pub(crate) fn is_test_path(path: &Path) -> bool {
    path.components().any(|component| {
        let part = component.as_os_str().to_string_lossy();
        part == "tests" || part.starts_with("test_")
    })
}

/// A file of an Odoo addon: some ancestor directory holds a `__manifest__.py`.
pub(crate) fn in_addon(path: &Path) -> bool {
    static ADDON_DIRS: LazyLock<Mutex<HashMap<PathBuf, bool>>> =
        LazyLock::new(|| Mutex::new(HashMap::new()));
    let mut ancestor = path.parent();
    while let Some(dir) = ancestor {
        if let Some(&known) = ADDON_DIRS.lock().unwrap().get(dir) {
            return known;
        }
        if dir.join("__manifest__.py").is_file() {
            let mut cache = ADDON_DIRS.lock().unwrap();
            let mut below = path.parent();
            while let Some(entry) = below {
                cache.insert(entry.to_path_buf(), true);
                if entry == dir {
                    break;
                }
                below = entry.parent();
            }
            return true;
        }
        ancestor = dir.parent();
    }
    let mut cache = ADDON_DIRS.lock().unwrap();
    let mut below = path.parent();
    while let Some(entry) = below {
        cache.insert(entry.to_path_buf(), false);
        below = entry.parent();
    }
    false
}
