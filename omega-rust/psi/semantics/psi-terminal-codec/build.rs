use std::fs;
use std::io;
use std::path::{Path, PathBuf};

fn main() -> io::Result<()> {
    write_source_closure(
        Path::new("../psi-terminal-verifier/src"),
        b"PSI-TERMINAL-VERIFIER-SOURCE-CLOSURE-v1\0",
        "psi-terminal-verifier-source-closure.bin",
    )?;
    write_source_closure(
        Path::new("../../representations/psi-terminal/src"),
        b"PSI-TERMINAL-REPRESENTATION-SOURCE-CLOSURE-v1\0",
        "psi-terminal-representation-source-closure.bin",
    )
}

fn write_source_closure(root: &Path, domain: &[u8], output_name: &str) -> io::Result<()> {
    println!("cargo:rerun-if-changed={}", root.display());

    let mut sources = Vec::new();
    collect_rust_sources(root, root, &mut sources)?;
    // Order the published slash-separated paths, not host-specific PathBuf
    // components: a root file and its sibling directory must compare the same
    // way on every host and in independent closure reconstruction.
    sources.sort_by_cached_key(|(path, _)| {
        path.to_str()
            .expect("source paths are valid UTF-8")
            .replace(std::path::MAIN_SEPARATOR, "/")
    });

    let mut closure = domain.to_vec();
    for (relative, absolute) in sources {
        let path = relative
            .to_str()
            .expect("source paths are valid UTF-8")
            .replace(std::path::MAIN_SEPARATOR, "/");
        let bytes = fs::read(absolute)?;
        closure.extend_from_slice(
            &u64::try_from(path.len())
                .expect("source path length fits u64")
                .to_le_bytes(),
        );
        closure.extend_from_slice(path.as_bytes());
        closure.extend_from_slice(
            &u64::try_from(bytes.len())
                .expect("source length fits u64")
                .to_le_bytes(),
        );
        closure.extend_from_slice(&bytes);
    }

    let output = PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo supplies OUT_DIR"))
        .join(output_name);
    fs::write(output, closure)
}

fn collect_rust_sources(
    root: &Path,
    directory: &Path,
    sources: &mut Vec<(PathBuf, PathBuf)>,
) -> io::Result<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_rust_sources(root, &path, sources)?;
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            sources.push((
                path.strip_prefix(root)
                    .expect("collected source remains below root")
                    .to_owned(),
                path,
            ));
        }
    }
    Ok(())
}
