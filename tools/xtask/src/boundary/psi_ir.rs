//! Verify the source files that own the public Psi record and its executable vocabulary.
use std::{fs, path::Path};

use serde::Deserialize;
use syn::{File, Item, Visibility};

const PSI: &str = "source-rust/subsystems/compiler/psi";

#[derive(Deserialize)]
struct Manifest {
    lib: Library,
}

#[derive(Deserialize)]
struct Library {
    path: String,
}

enum Definition {
    Record(&'static [&'static str]),
    Enum,
}

impl Definition {
    fn present(&self, file: &File, name: &str) -> bool {
        file.items.iter().any(|item| match (self, item) {
            (Self::Record(fields), Item::Struct(record)) => {
                record.ident == name
                    && matches!(record.vis, Visibility::Public(_))
                    && fields.iter().all(|field| {
                        record.fields.iter().any(|f| {
                            f.ident.as_ref().is_some_and(|id| id == field)
                                && matches!(f.vis, Visibility::Public(_))
                        })
                    })
            }
            (Self::Enum, Item::Enum(record)) => {
                record.ident == name
                    && matches!(record.vis, Visibility::Public(_))
                    && !record.variants.is_empty()
            }
            _ => false,
        })
    }
}

pub(super) fn violations(root: &Path) -> Result<Vec<String>, String> {
    let psi = root.join(PSI);
    let manifest = fs::read_to_string(psi.join("Cargo.toml"))
        .map_err(|err| format!("{PSI}/Cargo.toml: {err}"))?;
    let manifest: Manifest =
        toml::from_str(&manifest).map_err(|err| format!("{PSI}/Cargo.toml: {err}"))?;
    let mut errors = Vec::new();
    if manifest.lib.path != "psi.rs" {
        errors.push(format!(
            "{PSI}/Cargo.toml: the Psi crate root must be psi.rs"
        ));
    }
    if psi.join("src").exists() {
        errors.push(format!(
            "{PSI}/src: place the IR under its semantic domain directories"
        ));
    }
    for (path, name, definition) in [
        ("psi.rs", "Psi", Definition::Record(&["entry", "machines"])),
        (
            "machines/machine.rs",
            "TerminalMachine",
            Definition::Record(&["entry", "blocks"]),
        ),
        (
            "machines/block.rs",
            "Block",
            Definition::Record(&["operations", "terminator"]),
        ),
        (
            "machines/operation.rs",
            "Operation",
            Definition::Record(&["kind"]),
        ),
        ("machines/terminator.rs", "Terminator", Definition::Enum),
    ] {
        let contents =
            fs::read_to_string(psi.join(path)).map_err(|err| format!("{PSI}/{path}: {err}"))?;
        let syntax = syn::parse_file(&contents).map_err(|err| format!("{PSI}/{path}: {err}"))?;
        if !definition.present(&syntax, name) {
            errors.push(format!(
                "{PSI}/{path}: must directly define the public {name} IR data"
            ));
        }
    }
    Ok(errors)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reexport_or_empty_wrapper_does_not_own_the_psi_program() {
        let record = Definition::Record(&["entry", "machines"]);
        for syntax in [
            syn::parse_quote!(
                pub use hidden::TerminalModule as Psi;
            ),
            syn::parse_quote!(
                pub struct Psi(pub hidden::TerminalModule);
            ),
            syn::parse_quote!(
                pub struct Psi {
                    pub machines: Vec<Machine>,
                }
            ),
            syn::parse_quote!(
                mod hidden {
                    pub struct Psi {
                        pub entry: Entry,
                        pub machines: Vec<Machine>,
                    }
                }
            ),
        ] {
            assert!(!record.present(&syntax, "Psi"));
        }
        let syntax = syn::parse_quote!(
            pub struct Psi {
                pub entry: Entry,
                pub machines: Vec<Machine>,
            }
        );
        assert!(record.present(&syntax, "Psi"));
    }
}
