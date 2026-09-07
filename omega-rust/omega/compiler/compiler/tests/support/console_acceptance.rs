//! Explicit test acceptance of an exact standard-library Console provider.

use compiler::CheckedCompilation;
use diagnostics::Diagnostic;
use package_compilation::{AcceptedSemanticBinding, AcceptedSemanticBindingRole};

pub fn candidate_console_exit_binding(
    checked: &CheckedCompilation,
    standard_library: semantic_vocabulary::PackageKeyIdentity,
    accepts_console_output: bool,
    accepts_console_input: bool,
) -> Result<AcceptedSemanticBinding, Vec<Diagnostic>> {
    let candidates = checked
        .selected_provider_plans()
        .plans()
        .iter()
        .zip(checked.selected_provider_provenance())
        .filter(|(plan, provenance)| {
            plan.schema.trait_name == "Console"
                && plan.rows.iter().any(|row| row.method == "exit_process")
                && checked
                    .typed
                    .symbols
                    .symbol_package_identity(provenance.provider.schema.symbol())
                    == Some(standard_library)
        })
        .collect::<Vec<_>>();
    let [(plan, provenance)] = candidates.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "repository fixture Console exit acceptance resolved {} exact std provider plans instead of one",
            candidates.len()
        ))]);
    };
    let declaration_path = checked
        .typed
        .symbols
        .display_path(provenance.provider.schema.symbol(), "::");
    let binding = AcceptedSemanticBinding::new(
        AcceptedSemanticBindingRole::ConsoleExitProcessI32,
        standard_library,
        declaration_path,
        plan.schema.identity_digest(),
        plan.identity_digest(),
    )
    .map_err(|error| {
        vec![Diagnostic::error(format!(
            "cannot construct repository fixture Console exit binding: {error}"
        ))]
    })?;
    let mut permissions = plan
        .schema
        .methods
        .iter()
        .filter(|method| {
            method.name == "exit_process"
                || (accepts_console_output && method.name == "write_byte")
                || (accepts_console_input && method.name == "read_byte")
        })
        .map(|method| {
            effects::ServiceTerminalAuthorityPermission::new(
                plan.schema.identity_digest(),
                method.requirement_identity.clone(),
                effects::TerminalAuthorityDisposition::from_classes(match method.name.as_str() {
                    "exit_process" => {
                        vec![effects::TerminalAuthorityClass::ProcessTermination]
                    }
                    "write_byte" => {
                        vec![effects::TerminalAuthorityClass::ProcessOutput]
                    }
                    "read_byte" => {
                        vec![effects::TerminalAuthorityClass::ProcessInput]
                    }
                    _ => unreachable!("filtered above"),
                }),
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
                "cannot attach repository fixture Console terminal permissions: {error}"
            ))]
        })
}
