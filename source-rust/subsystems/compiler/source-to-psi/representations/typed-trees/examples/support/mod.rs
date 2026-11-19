use std::path::{Path, PathBuf};

pub fn typed_trees_directory() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|directory| directory.ends_with("source-to-psi"))
        .expect("typed-tree examples live under the source compiler")
        .join("representations/typed-trees")
}
