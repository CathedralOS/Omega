//! Verify the source files that own the public Psi record and its executable vocabulary.
use std::{fs, path::Path};

use serde::Deserialize;
use syn::{Expr, File, Item, Lit, Meta, Type, Visibility};

const PSI: &str = "source-rust/subsystems/compiler/psi";
const GROUPS: &[(&str, &str)] = &[
    ("types", "TypeDeclarations"),
    ("scalars", "ScalarDeclarations"),
    ("interfaces", "ProgramInterfaces"),
    ("access", "AccessDeclarations"),
    ("execution", "ExecutionDeclarations"),
    ("proofs", "ProgramProofs"),
    ("layouts", "LayoutDeclarations"),
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
        format!("{}.rs", module.ident) == target || format!("{}/mod.rs", module.ident) == target
    })
}

fn field_type<'a>(file: &'a File, name: &str) -> Option<&'a Type> {
    let record = file.items.iter().find_map(|item| match item {
        Item::Struct(record) if record.ident == "Psi" => Some(record),
        _ => None,
    })?;
    let field = record.fields.iter().find(|field| {
        matches!(field.vis, Visibility::Public(_))
            && field.ident.as_ref().is_some_and(|ident| ident == name)
    })?;
    Some(&field.ty)
}

fn has_group(file: &File, name: &str, record: &str) -> bool {
    matches!(field_type(file, name), Some(Type::Path(ty)) if ty.qself.is_none() && ty.path.is_ident(record))
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
    let program = read_module(&psi, "psi.rs")?;
    for item in &program.items {
        if let Item::Mod(module) = item {
            let name = module.ident.to_string();
            if module.attrs.iter().any(|attr| attr.path().is_ident("path"))
                || (name != "exports" && field_type(&program, &name).is_none())
            {
                errors.push(format!(
                    "{PSI}/psi.rs: declare field modules directly and let each field own its children, found {name}"
                ));
            }
        }
    }
    for entry in fs::read_dir(&psi).map_err(|err| format!("{PSI}: {err}"))? {
        let entry = entry.map_err(|err| format!("{PSI}: {err}"))?;
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if path.is_dir()
            && !name.starts_with('.')
            && !matches!(name.as_ref(), "tests" | "target")
            && (field_type(&program, &name).is_none()
                || !loads_module(&program, &format!("{name}/mod.rs")))
        {
            errors.push(format!(
                "{PSI}/{name}: a program record directory must name a public Psi field"
            ));
        }
    }
    if !Definition::Record(&["entry", "machines"]).present(&program, "Psi") {
        errors.push(format!(
            "{PSI}/psi.rs: must directly define the public Psi IR data"
        ));
    }
    for &(field, name) in GROUPS {
        let owner = format!("{field}/mod.rs");
        if !has_group(&program, field, name) || !loads_module(&program, &owner) {
            errors.push(format!(
                "{PSI}/psi.rs: Psi.{field} must carry {name} from {owner}"
            ));
        }
        let module = read_module(&psi, &owner)?;
        let definition = Definition::Record(&[]);
        if definition.present(&module, name) {
            continue;
        }
        if !loads_module(&module, "declarations.rs")
            || !definition.present(
                &read_module(&psi, &format!("{field}/declarations.rs"))?,
                name,
            )
        {
            errors.push(format!(
                "{PSI}/{owner}: must define {name} or load its declaration"
            ));
        }
    }
    if !loads_module(&program, "machines/mod.rs") {
        errors.push(format!(
            "{PSI}/psi.rs: must load the machine control-flow module"
        ));
    }
    let control = read_module(&psi, "machines/mod.rs")?;
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
            errors.push(format!("{PSI}/machines/mod.rs: must load {module}"));
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
            pub mod types;
            mod selection { #[path = "proofs/declarations.rs"] mod proofs; }
        };
        assert!(field_type(&root, "types").is_some());
        assert!(field_type(&root, "codec").is_none());
        assert!(has_group(&root, "types", "TypeDeclarations"));
        assert!(!has_group(&root, "proofs", "ProgramProofs"));
        assert!(loads_module(&root, "types/mod.rs"));
        assert!(!loads_module(&root, "types/declarations.rs"));
        assert!(!loads_module(&root, "proofs/declarations.rs"));
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
