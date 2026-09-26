//! Explicit test acceptance of the exact canonical ProcessExit provider.
//!
//! The `ProcessExit` boundary trait is toolchain-owned
//! (`omega::language::core::process_exit`) and carries no package identity;
//! the bound package owns only the `ProcessExitNativeProvider` nominal and its
//! selected plan. Source spelling selects this repository's test policy; the
//! compiler still rejoins the exact trait, normalized schema, provider
//! custody, and plan digests before granting the terminal-event identity.

use diagnostics::Diagnostic;
use omega::compiler::CheckedCompilation;
use omega::package_compilation::{AcceptedSemanticBinding, AcceptedSemanticBindingRole};

pub fn candidate_process_exit_binding(
    checked: &CheckedCompilation,
    standard_library: semantic_vocabulary::PackageKeyIdentity,
) -> Result<AcceptedSemanticBinding, Vec<Diagnostic>> {
    let candidates = checked
        .selected_provider_plans()
        .plans()
        .iter()
        .zip(checked.selected_provider_provenance())
        .filter(|(plan, provenance)| {
            plan.schema.trait_name == "ProcessExit"
                && plan.rows.iter().any(|row| row.method == "exit_process")
                && provenance.provider.provider_type.is_some_and(|provider| {
                    checked.typed.symbols.symbol_package_identity(provider)
                        == Some(standard_library)
                })
        })
        .collect::<Vec<_>>();
    let [(plan, provenance)] = candidates.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "repository fixture ProcessExit acceptance resolved {} exact std provider plans instead of one",
            candidates.len()
        ))]);
    };
    let declaration_path = checked
        .typed
        .symbols
        .display_path(provenance.provider.schema.symbol(), "::");
    let binding = AcceptedSemanticBinding::new(
        AcceptedSemanticBindingRole::ProcessExitExitProcessI32,
        standard_library,
        declaration_path,
        plan.schema.identity_digest(),
        plan.identity_digest(),
    )
    .map_err(|error| {
        vec![Diagnostic::error(format!(
            "cannot construct repository fixture ProcessExit binding: {error}"
        ))]
    })?;
    let mut permissions = plan
        .schema
        .methods
        .iter()
        .filter(|method| method.name == "exit_process")
        .map(|method| {
            abstract_operations_to_target_operations::effects::ServiceTerminalAuthorityPermission::new(
                plan.schema.identity_digest(),
                method.requirement_identity.clone(),
                abstract_operations_to_target_operations::effects::TerminalAuthorityDisposition::from_classes(vec![
                    abstract_operations_to_target_operations::effects::TerminalAuthorityClass::ProcessTermination,
                ]),
            )
        })
        .collect::<Vec<_>>();
    permissions.sort_by(|left, right| {
        left.requirement_identity()
            .cmp(right.requirement_identity())
    });
    binding
        .with_terminal_authority_permissions(permissions)
        .map_err(|error| {
            vec![Diagnostic::error(format!(
                "cannot attach repository fixture ProcessExit terminal permissions: {error}"
            ))]
        })
}
