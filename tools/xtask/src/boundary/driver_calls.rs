//! Low-level source and native invocation belong to their drivers. A project host
//! supplies captured inputs, and a package never reconstructs either producer.
use super::{collect_rs_files, is_test_source, under};
use std::collections::BTreeSet;
use std::path::Path;
use syn::visit::{self, Visit};

const SOURCE_OPERATIONS: &[&str] = &[
    "ResolveAndTypeRequest",
    "SourcePreparationRequest",
    "TerminalSourceCompilation",
    "judge_terminal_source",
    "judge_source_roster",
    "prepare_lower_and_check",
];

const NATIVE_REQUEST: &str = "NativeCompileRequest";
const RETIRED_NATIVE_OPERATIONS: &[&str] = &[
    "retain_native_derivation_inputs",
    "recover_native_derivation_inputs",
];

fn operation_owner(relative: &str, name: &str) -> Option<&'static str> {
    if SOURCE_OPERATIONS.contains(&name) {
        let source = super::linear_pipeline::SOURCE_DRIVER;
        let module = relative
            .strip_prefix(source)
            .and_then(|path| path.strip_prefix('/'));
        let allowed = match name {
            "judge_terminal_source" | "judge_source_roster" => {
                matches!(module, Some("source_judgment.rs" | "stages.rs"))
            }
            "prepare_lower_and_check" => matches!(module, Some("source_to_psi.rs" | "product.rs")),
            _ => {
                under(relative, &[source])
                    && !under(
                        relative,
                        &["source-rust/subsystems/compiler/source-to-psi/representations"],
                    )
            }
        };
        return (!allowed).then_some("request source compilation through its driver");
    }
    if name == NATIVE_REQUEST {
        let owner = relative.strip_prefix("source-rust/subsystems/compiler/psi-to-native/");
        return (!matches!(
            owner,
            Some("psi_to_native.rs" | "driver.rs" | "request.rs" | "target_inputs.rs")
        ))
        .then_some("submit the published Psi program to the native driver");
    }
    Some("the retired derivation codec belongs in _recycling")
}

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
        if SOURCE_OPERATIONS.contains(&name.as_str())
            || name == NATIVE_REQUEST
            || RETIRED_NATIVE_OPERATIONS.contains(&name.as_str())
        {
            self.0.insert(name);
        }
    }
}

pub(super) fn violations(root: &Path) -> Result<Vec<String>, String> {
    let mut violations = Vec::new();
    for scope in [
        "source-rust/subsystems/compiler/build",
        "source-rust/subsystems/package-manager",
        super::linear_pipeline::SOURCE_DRIVER,
        super::native_stages::DRIVER,
    ] {
        if !root.join(scope).join("Cargo.toml").is_file() {
            return Err(format!(
                "{scope}: driver ownership audit is missing its Cargo manifest"
            ));
        }
    }
    let mut files = Vec::new();
    collect_rs_files(&root.join("source-rust"), &mut files);
    for path in files {
        let relative = path.strip_prefix(root).map_err(|error| error.to_string())?;
        let name = relative.to_string_lossy().replace('\\', "/");
        if is_test_source(&name) {
            continue;
        }
        let source = std::fs::read_to_string(&path).map_err(|error| format!("{name}: {error}"))?;
        if !SOURCE_OPERATIONS
            .iter()
            .chain(RETIRED_NATIVE_OPERATIONS)
            .copied()
            .chain([NATIVE_REQUEST])
            .any(|entry| source.contains(entry))
        {
            continue;
        }
        let syntax = syn::parse_file(&source).map_err(|error| format!("{name}: {error}"))?;
        let mut operations = Operations::default();
        operations.visit_file(&syntax);
        for operation in operations.0 {
            if let Some(reason) = operation_owner(&name, &operation) {
                violations.push(format!("{name}: {operation}: {reason}"));
            }
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
    fn compile_time_helpers_cannot_bypass_the_shared_source_judgment() {
        let nested = "source-rust/subsystems/compiler/source-to-psi/compile_time/calls.rs";
        assert!(operation_owner(nested, "judge_terminal_source").is_some());
        assert!(operation_owner(nested, "prepare_lower_and_check").is_some());
        assert!(
            operation_owner(
                "source-rust/subsystems/compiler/source-to-psi/source_judgment.rs",
                "judge_terminal_source"
            )
            .is_none()
        );
    }

    #[test]
    fn helpers_cannot_reconstruct_the_native_request_or_parked_codec() {
        assert!(
            operation_owner(
                "source-rust/subsystems/compiler/psi-to-native/request.rs",
                NATIVE_REQUEST
            )
            .is_none()
        );
        assert!(
            operation_owner(
                "source-rust/subsystems/compiler/psi-to-native/representations/shortcut.rs",
                NATIVE_REQUEST
            )
            .is_some()
        );
        assert!(
            operation_owner(
                "source-rust/foundation/worker.rs",
                "TerminalSourceCompilation"
            )
            .is_some()
        );
        assert!(
            operation_owner(
                "source-rust/subsystems/compiler/psi-to-native/driver.rs",
                "recover_native_derivation_inputs"
            )
            .is_some()
        );
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
