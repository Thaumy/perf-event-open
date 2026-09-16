mod bindgen;
mod version;

use std::path::Path;
use std::{fs, io};

use anyhow::{Context, Result};
use diffy::create_patch;
use version::Version;

use crate::bindgen::bindgen;

fn main() -> Result<()> {
    let headers_dir = Path::new("headers");
    let patches_dir = Path::new("../patches");

    if patches_dir.exists() {
        println!("{:?} exists, skip running.", patches_dir);
        return Ok(());
    }
    fs::create_dir_all(patches_dir)
        .with_context(|| format!("failed to create dir: {:?}", patches_dir))?;

    //let mut header_paths: Vec<_> = fs::read_dir(headers_dir)
    let mut header_dirs = fs::read_dir(headers_dir)
        .with_context(|| format!("failed to read dir: {:?}", headers_dir))?
        .collect::<io::Result<Vec<_>>>()
        .with_context(|| format!("failed to access entry in: {:?}", headers_dir))?
        .into_iter()
        .filter(|it| it.path().is_dir())
        .map(|it| {
            let file_name = it.file_name();
            let file_name = file_name
                .to_str()
                .context(format!("failed to sort: {:?}", file_name))?;
            // linux-6.13 -> 6.13 -> 6, 13
            let mut versions = file_name.as_bytes()["linux-".len()..]
                .split(|b| *b == b'.')
                .flat_map(|it| std::str::from_utf8(it).ok())
                .flat_map(|it| it.parse::<usize>().ok());
            let major = versions
                .next()
                .context(format!("failed to sort: {:?}", file_name))?;
            let minor = versions
                .next()
                .context(format!("failed to sort: {:?}", file_name))?;
            Ok((major, minor, it.path()))
        })
        .collect::<Result<Vec<_>>>()?;
    header_dirs.sort_unstable_by_key(|(major, minor, _)| (*major, *minor));

    let mut prev_binding = String::new();
    for (_, _, header_dir) in header_dirs {
        let include_dir = &header_dir.join("include");
        let version = Version::from_include(include_dir)
            .with_context(|| format!("when parsing header version from: {:?}", include_dir))?;
        let binding = bindgen(&version, include_dir)
            .with_context(|| format!("when generating bindings from: {:?}", include_dir))?;

        let patch_path = {
            let mut file_name = header_dir
                .file_name()
                .context(format!(
                    "failed to get patch file name from: {:?}",
                    header_dir
                ))?
                .to_os_string();
            file_name.push(".rs.patch");
            Path::new(patches_dir).join(file_name)
        };
        let patch = create_patch(&prev_binding, &binding);
        fs::write(&patch_path, patch.to_string())
            .with_context(|| format!("failed to write patch: {:?}", patch_path))?;

        println!("generated: {:?}", patch_path);
        prev_binding = binding;
    }

    Ok(())
}
