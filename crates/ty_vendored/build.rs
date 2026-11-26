//! Build script to package our vendored typeshed files
//! into a zip archive that can be included in the Ruff binary.
//!
//! This script should be automatically run at build time
//! whenever the script itself changes, or whenever any files
//! in `crates/ty_vendored/vendor/typeshed` change.

use std::fs::File;
use std::io::Write;
use std::path::Path;

use path_slash::PathExt;
use zip::CompressionMethod;
use zip::result::ZipResult;
use zip::write::{SimpleFileOptions, ZipWriter};

const TYPESHED_SOURCE_DIR: &str = "vendor/typeshed";
const TY_EXTENSIONS_STUBS: &[(&str, &str)] = &[
    (
        "ty_extensions/__init__.pyi",
        "stdlib/ty_extensions/__init__.pyi",
    ),
    (
        "ty_extensions/_internal.pyi",
        "stdlib/ty_extensions/_internal.pyi",
    ),
    (
        "ty_extensions/pydantic.pyi",
        "stdlib/ty_extensions/pydantic.pyi",
    ),
];
const TYPESHED_ZIP_LOCATION: &str = "/zipped_typeshed.zip";

/// Recursively zip the contents of the entire typeshed directory and patch typeshed
/// on the fly to include the `ty_extensions` module.
///
/// This routine is adapted from a recipe at
/// <https://github.com/zip-rs/zip-old/blob/5d0f198124946b7be4e5969719a7f29f363118cd/examples/write_dir.rs>
fn write_zipped_typeshed_to(writer: File) -> ZipResult<File> {
    let mut zip = ZipWriter::new(writer);

    #[cfg(feature = "zstd")]
    let method = CompressionMethod::Zstd;
    #[cfg(all(not(feature = "zstd"), feature = "deflate"))]
    let method = CompressionMethod::Deflated;
    #[cfg(not(any(feature = "zstd", feature = "deflate")))]
    let method = CompressionMethod::Stored;

    let options = SimpleFileOptions::default()
        .compression_method(method)
        .unix_permissions(0o644);

    for entry in walkdir::WalkDir::new(TYPESHED_SOURCE_DIR) {
        let dir_entry = entry.unwrap();
        let absolute_path = dir_entry.path();
        let normalized_relative_path = absolute_path
            .strip_prefix(Path::new(TYPESHED_SOURCE_DIR))
            .unwrap()
            .to_slash()
            .expect("Unexpected non-utf8 typeshed path!");

        if absolute_path.is_file() {
            println!("adding file {absolute_path:?} as {normalized_relative_path:?} ...");
            zip.start_file(&*normalized_relative_path, options)?;
            let mut f = File::open(absolute_path)?;
            std::io::copy(&mut f, &mut zip).unwrap();

            if normalized_relative_path == "stdlib/VERSIONS" {
                writeln!(&mut zip, "ty_extensions: 3.0-")?;
            }
        } else if !normalized_relative_path.is_empty() {
            println!("adding dir {absolute_path:?} as {normalized_relative_path:?} ...");
            zip.add_directory(normalized_relative_path, options)?;
        }
    }

    zip.add_directory("stdlib/ty_extensions/", options)?;
    for (source, destination) in TY_EXTENSIONS_STUBS {
        println!("adding file {source} as {destination} ...");
        zip.start_file(destination, options)?;
        let mut f = File::open(source)?;
        std::io::copy(&mut f, &mut zip).unwrap();
    }

    zip.finish()
}

fn main() {
    assert!(
        Path::new(TYPESHED_SOURCE_DIR).is_dir(),
        "Where is typeshed?"
    );
    let out_dir = std::env::var("OUT_DIR").unwrap();

    let zipped_typeshed_location = format!("{out_dir}{TYPESHED_ZIP_LOCATION}");

    let zipped_typeshed_file = File::create(zipped_typeshed_location).unwrap();
    write_zipped_typeshed_to(zipped_typeshed_file).unwrap();
}
