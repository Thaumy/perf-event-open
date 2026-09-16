use std::path::PathBuf;
use std::{env, fs, io};

use anyhow::{Context, Result};
use diffy::{apply, Patch};

const PATCHES_DIR: &str = "patches";

fn main() -> Result<()> {
    println!("cargo:rerun-if-changed={}", PATCHES_DIR);

    let bindings_dir = PathBuf::from(format!("{}/bindings", env::var("OUT_DIR")?));

    if bindings_dir.exists() {
        fs::remove_dir_all(&bindings_dir)
            .with_context(|| format!("failed to remove dir: {:?}", bindings_dir))?;
    }
    fs::create_dir_all(&bindings_dir)
        .with_context(|| format!("failed to create dir: {:?}", bindings_dir))?;

    let mut patch_paths = fs::read_dir(PATCHES_DIR)
        .with_context(|| format!("failed to read dir: {:?}", PATCHES_DIR))?
        .collect::<io::Result<Vec<_>>>()
        .with_context(|| format!("failed to access entry in: {:?}", PATCHES_DIR))?
        .into_iter()
        .map(|it| {
            let file_name = it.file_name();
            let file_name = file_name
                .to_str()
                .context(format!("failed to sort: {:?}", file_name))?;
            // linux-6.13.rs.patch -> 6.13.rs.patch -> 6, 13, ..
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
    patch_paths.sort_unstable_by_key(|(major, minor, _)| (*major, *minor));

    let mut prev_binding = String::new();
    for (_, _, patch_path) in patch_paths {
        let patch = fs::read_to_string(&patch_path)
            .context(format!("failed to read patch: {:?}", patch_path))?;
        let patch =
            Patch::from_str(&patch).context(format!("failed to parse patch: {:?}", patch_path))?;

        let binding = apply(&prev_binding, &patch)
            .context(format!("failed to apply patch: {:?}", patch_path))?;
        let binding_file_name = patch_path.file_stem().context(format!(
            "failed to generate binding file name with: {:?}",
            patch_path
        ))?;
        let binding_path = bindings_dir.join(binding_file_name);
        fs::write(&binding_path, &binding)
            .with_context(|| format!("failed to write: {:?}", patch_path))?;
        println!("generated: {:?}", binding_path);

        prev_binding = binding;
    }

    Ok(())
}
