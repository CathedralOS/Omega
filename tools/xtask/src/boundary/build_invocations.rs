//! The project host supplies captured inputs and executes an issued Build invocation.
//! Package and host crates must not assemble another source compiler out of stage APIs.
use super::{collect_rs_files, is_test_source};
use std::collections::BTreeSet;
use std::path::Path;
use syn::visit::{self, Visit};

const STAGE_OPERATIONS: &[&str] = &[
    "ResolveAndTypeRequest",
    "SourcePreparationRequest",
    "TerminalSourceCompilation",
    "judge_terminal_source",
    "judge_source_roster",
    "prepare_lower_and_check",
];

#[derive(Default)]
struct Operations(BTreeSet<String>);

impl<'ast> Visit<'ast> for Operations {
    // Host unit tests may prepare stage-specific fixtures, including in inline modules.
    fn visit_item_mod(&mut self, item: &'ast syn::ItemMod) {
        if !item.attrs.iter().any(|attr| {
            attr.path().is_ident("cfg")
                && attr
                    .parse_args::<syn::Ident>()
                    .is_ok_and(|cfg| cfg == "test")
        }) {
            visit::visit_item_mod(self, item);
        }
    }

    fn visit_ident(&mut self, ident: &'ast syn::Ident) {
        let name = ident.to_string();
        if STAGE_OPERATIONS.contains(&name.as_str()) {
            self.0.insert(name);
        }
    }
}

pub(super) fn violations(root: &Path) -> Result<Vec<String>, String> {
    let mut violations = Vec::new();
    let mut files = Vec::new();
    for scope in [
        "source-rust/subsystems/compiler/build",
        "source-rust/subsystems/package-manager",
    ] {
        collect_rs_files(&root.join(scope), &mut files);
    }
    for path in files {
        let relative = path.strip_prefix(root).map_err(|error| error.to_string())?;
        let name = relative.to_string_lossy().replace('\\', "/");
        if is_test_source(&name) {
            continue;
        }
        let source = std::fs::read_to_string(&path).map_err(|error| format!("{name}: {error}"))?;
        let syntax = syn::parse_file(&source).map_err(|error| format!("{name}: {error}"))?;
        let mut operations = Operations::default();
        operations.visit_file(&syntax);
        for operation in operations.0 {
            violations.push(format!(
                "{name}: {operation} belongs in the source driver; request a checked Build invocation"
            ));
        }
    }
    Ok(violations)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_code_running_registered_tests_still_needs_a_checked_invocation() {
        assert!(!is_test_source("build/planning/registered_tests.rs"));
        assert!(is_test_source("build/execution/src/host/bindings/tests.rs"));
    }

    #[test]
    fn detects_stage_imports_and_construction_in_a_build_host() {
        let syntax = syn::parse_quote! {
            use source_compiler::{ResolveAndTypeRequest as Resolve, CheckedBuildInvocation};
            fn prepare() {
                let _ = source_compiler::SourcePreparationRequest { ..input };
                CheckedBuildInvocation::judge(inputs);
            }
            #[cfg(test)]
            mod tests {
                fn fixture() { judge_terminal_source(input); }
            }
        };
        let mut operations = Operations::default();
        operations.visit_file(&syntax);
        assert_eq!(
            operations.0.into_iter().collect::<Vec<_>>(),
            ["ResolveAndTypeRequest", "SourcePreparationRequest"]
        );
    }
}
