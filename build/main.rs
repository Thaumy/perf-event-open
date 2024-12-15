mod bindgen;
mod version;

use std::fs::{self, create_dir_all};
use std::path::Path;

use anyhow::{Context, Result};
use version::Version;

use crate::bindgen::bindgen;

const HEADERS_DIR: &str = "headers";
const BINDINGS_DIR: &str = "src/ffi/bindings";

fn main() -> Result<()> {
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    unimplemented!("`perf_event_open` syscall can only be used in linux or android target");

    println!("cargo:rerun-if-changed={}", BINDINGS_DIR);

    if Path::new(BINDINGS_DIR).exists() {
        return Ok(());
    }
    create_dir_all(BINDINGS_DIR)
        .with_context(|| format!("failed to create dir: {}", BINDINGS_DIR))?;

    let headers = fs::read_dir(HEADERS_DIR)
        .with_context(|| format!("failed to read dir: {}", HEADERS_DIR))?;

    for entry in headers {
        let entry = entry.with_context(|| format!("failed to access entry in: {}", HEADERS_DIR))?;

        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let headers_dir = &path.join("include");

        let version = Version::from_headers(headers_dir)
            .with_context(|| format!("when parsing header version from: {:?}", headers_dir))?;

        let to = {
            let headers_dir_name = path
                .file_name()
                .context("failed to get headers dir name")
                .context("when generating bindings file name")?;
            let name = headers_dir_name
                .to_str()
                .context("invalid headers dir name")
                .context("when generating bindings file name")?;
            Path::new(BINDINGS_DIR).join(format!(".{}.rs", name))
        };

        bindgen(&version, headers_dir, &to)
            .with_context(|| format!("when generating bindings from: {:?}", headers_dir))?;
    }

    Ok(())
}
