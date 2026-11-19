//! Verify the source files that own the public Psi record and its executable vocabulary.
use std::{fs, path::Path};

use serde::Deserialize;
use syn::{Expr, File, Item, Lit, Meta, Type, Visibility};

const PSI: &str = "source-rust/subsystems/compiler/psi";
const GROUPS: &[(&str, &str, &str)] = &[
    ("types", "TypeDeclarations", "types/declarations.rs"),
    ("scalars", "ScalarDeclarations", "types/scalars.rs"),
    ("interfaces", "ProgramInterfaces", "boundary/interfaces.rs"),
    ("access", "AccessDeclarations", "ownership/declarations.rs"),
    (
        "execution",
        "ExecutionDeclarations",
        "machines/execution.rs",
    ),
    ("proofs", "ProgramProofs", "evidence/program_proofs.rs"),
    (
        "layouts",
        "LayoutDeclarations",
        "types/layouts/declarations.rs",
    ),
];

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
                    && matches!(&record.fields, syn::Fields::Named(named) if !named.named.is_empty())
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

fn loads_module(file: &File, target: &str) -> bool {
    file.items.iter().any(|item| {
        let Item::Mod(module) = item else {
            return false;
        };
        if module.content.is_some() {
            return false;
        }
        if let Some(attribute) = module
            .attrs
            .iter()
            .find(|attr| attr.path().is_ident("path"))
        {
            return matches!(&attribute.meta, Meta::NameValue(value)
                if matches!(&value.value, Expr::Lit(expr)
                    if matches!(&expr.lit, Lit::Str(path) if path.value() == target)));
        }
        format!("{}.rs", module.ident) == target
    })
}

fn has_group(file: &File, name: &str, record: &str) -> bool {
    file.items.iter().any(|item| {
        let Item::Struct(program) = item else { return false };
        program.ident == "Psi" && program.fields.iter().any(|field|
            field.ident.as_ref().is_some_and(|ident| ident == name)
            && matches!(field.vis, Visibility::Public(_))
            && matches!(&field.ty, Type::Path(ty) if ty.qself.is_none() && ty.path.is_ident(record)))
    })
}

fn read_module(psi: &Path, path: &str) -> Result<File, String> {
    let source =
        fs::read_to_string(psi.join(path)).map_err(|err| format!("{PSI}/{path}: {err}"))?;
    syn::parse_file(&source).map_err(|err| format!("{PSI}/{path}: {err}"))
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
    let program = read_module(&psi, "psi.rs")?;
    if !Definition::Record(&["entry", "machines"]).present(&program, "Psi") {
        errors.push(format!(
            "{PSI}/psi.rs: must directly define the public Psi IR data"
        ));
    }
    for &(field, name, path) in GROUPS {
        if !has_group(&program, field, name) || !loads_module(&program, path) {
            errors.push(format!(
                "{PSI}/psi.rs: Psi.{field} must carry {name} from {path}"
            ));
        }
        if !Definition::Record(&[]).present(&read_module(&psi, path)?, name) {
            errors.push(format!(
                "{PSI}/{path}: must directly define the public {name} data"
            ));
        }
    }
    if !loads_module(&program, "machines/control_flow.rs") {
        errors.push(format!(
            "{PSI}/psi.rs: must load the machine control-flow module"
        ));
    }
    let control = read_module(&psi, "machines/control_flow.rs")?;
    for (path, name, definition) in [
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
        let module = path.strip_prefix("machines/").expect("machine module path");
        if !loads_module(&control, module) {
            errors.push(format!(
                "{PSI}/machines/control_flow.rs: must load {module}"
            ));
        }
        let syntax = read_module(&psi, path)?;
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
    fn psi_groups_use_the_domain_record_type_and_its_declared_module() {
        let root = syn::parse_quote! {
            pub struct Psi { pub types: TypeDeclarations, pub proofs: Vec<Proof> }
            #[path = "types/declarations.rs"] mod types_declarations;
            mod selection { #[path = "evidence/program_proofs.rs"] mod proofs; }
        };
        assert!(has_group(&root, "types", "TypeDeclarations"));
        assert!(!has_group(&root, "proofs", "ProgramProofs"));
        assert!(loads_module(&root, "types/declarations.rs"));
        assert!(!loads_module(&root, "evidence/program_proofs.rs"));
    }

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
