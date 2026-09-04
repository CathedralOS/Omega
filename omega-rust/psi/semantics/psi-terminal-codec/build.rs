use std::fs;
use std::io;
use std::path::{Path, PathBuf};

fn main() -> io::Result<()> {
    let verifier_root = Path::new("../psi-terminal-verifier/src");
    println!("cargo:rerun-if-changed={}", verifier_root.display());

    let mut sources = Vec::new();
    collect_rust_sources(verifier_root, verifier_root, &mut sources)?;
    sources.sort_by(|left, right| left.0.cmp(&right.0));

    let mut closure = b"PSI-TERMINAL-VERIFIER-SOURCE-CLOSURE-v1\0".to_vec();
    for (relative, absolute) in sources {
        let path = relative
            .to_str()
            .expect("verifier source paths are valid UTF-8")
            .replace(std::path::MAIN_SEPARATOR, "/");
        let bytes = fs::read(absolute)?;
        closure.extend_from_slice(
            &u64::try_from(path.len())
                .expect("verifier source path length fits u64")
                .to_le_bytes(),
        );
        closure.extend_from_slice(path.as_bytes());
        closure.extend_from_slice(
            &u64::try_from(bytes.len())
                .expect("verifier source length fits u64")
                .to_le_bytes(),
        );
        closure.extend_from_slice(&bytes);
    }

    let output = PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo supplies OUT_DIR"))
        .join("psi-terminal-verifier-source-closure.bin");
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
                    .expect("collected verifier source remains below root")
                    .to_owned(),
                path,
            ));
        }
    }
    Ok(())
}
