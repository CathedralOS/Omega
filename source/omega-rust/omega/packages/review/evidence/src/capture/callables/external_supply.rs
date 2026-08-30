use crate::capture::source::{ProjectedNestedSourceLocation, ProjectedReviewRow};
use crate::record::{
    PackageReviewExternalBinding, PackageReviewExternalExecutableSupply,
    PackageReviewSourceLocationRole,
};
use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;

pub(super) fn project_external_executable_supply_with_source(
    machine: &psi_typed_trees::machine::Machine,
    conformance: &psi_typed_trees::machine::TraitConformance,
    row: PackageReviewExternalExecutableSupply,
) -> Result<ProjectedReviewRow<PackageReviewExternalExecutableSupply>, Vec<Diagnostic>> {
    let Some(source_span) = conformance.external_binding_source_span else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed external callable `{}` has no exact authored `via` custody",
            machine.name
        ))]);
    };
    Ok(ProjectedReviewRow {
        row,
        declaration: machine.symbol,
        nested_source_locations: vec![ProjectedNestedSourceLocation {
            source_span,
            role: PackageReviewSourceLocationRole::ExternalBinding,
        }],
    })
}

pub(super) fn validate_external_binding_payload(
    compilation: &CheckedCompilation,
    machine: &psi_typed_trees::machine::Machine,
    identity: &psi_language_semantics::ExternalBindingIdentity,
) -> Result<(), Vec<Diagnostic>> {
    use psi_language_semantics::ExternalBindingIdentity;

    let invalid = match identity {
        ExternalBindingIdentity::Import { library, symbol } if library.is_empty() => {
            Some("has no exact import-library identity")
        }
        ExternalBindingIdentity::Import { symbol, .. } if symbol.is_empty() => {
            Some("has no exact import-symbol identity")
        }
        ExternalBindingIdentity::Syscall { number } if u32::try_from(*number).is_err() => {
            Some("has a syscall number outside 0..=u32::MAX")
        }
        ExternalBindingIdentity::VtableSlot { index } if *index < 0 => {
            Some("has a negative vtable-slot index")
        }
        ExternalBindingIdentity::VtableField { field }
        | ExternalBindingIdentity::TableFunction { field }
            if field.is_empty() =>
        {
            Some("has no exact table-field identity")
        }
        ExternalBindingIdentity::VtableField { .. }
        | ExternalBindingIdentity::TableFunction { .. }
            if !machine.attached_data_symbol.is_valid()
                || machine.attached_data.is_none()
                || !compilation
                    .data_definitions()
                    .iter()
                    .any(|definition| definition.symbol == machine.attached_data_symbol) =>
        {
            Some("has table-field supply without one exact attached provider data declaration")
        }
        ExternalBindingIdentity::Import { .. }
        | ExternalBindingIdentity::Syscall { .. }
        | ExternalBindingIdentity::CompilerIntrinsic
        | ExternalBindingIdentity::VtableSlot { .. }
        | ExternalBindingIdentity::VtableField { .. }
        | ExternalBindingIdentity::TableFunction { .. } => None,
    };
    match invalid {
        Some(reason) => Err(vec![Diagnostic::error(format!(
            "reviewed external callable `{}` {reason}",
            machine.name
        ))]),
        None => Ok(()),
    }
}

pub(super) fn project_external_binding(
    identity: &psi_language_semantics::ExternalBindingIdentity,
) -> PackageReviewExternalBinding {
    match identity {
        psi_language_semantics::ExternalBindingIdentity::Import { library, symbol } => {
            PackageReviewExternalBinding::Import {
                library: library.clone(),
                symbol: symbol.clone(),
            }
        }
        psi_language_semantics::ExternalBindingIdentity::Syscall { number } => {
            PackageReviewExternalBinding::Syscall { number: *number }
        }
        psi_language_semantics::ExternalBindingIdentity::CompilerIntrinsic => {
            PackageReviewExternalBinding::CompilerIntrinsic
        }
        psi_language_semantics::ExternalBindingIdentity::VtableSlot { index } => {
            PackageReviewExternalBinding::VtableSlot { index: *index }
        }
        psi_language_semantics::ExternalBindingIdentity::VtableField { field } => {
            PackageReviewExternalBinding::VtableField {
                field: field.clone(),
            }
        }
        psi_language_semantics::ExternalBindingIdentity::TableFunction { field } => {
            PackageReviewExternalBinding::TableFunction {
                field: field.clone(),
            }
        }
    }
}

pub(super) fn external_binding_matches_provider_binding(
    compilation: &CheckedCompilation,
    machine: &psi_typed_trees::machine::Machine,
    binding: &PackageReviewExternalBinding,
    selected: &omega_effects::provider_plan::ProviderBinding,
) -> bool {
    let expected_machine_identity = compilation
        .normalized_machine_overload_identity(machine)
        .map(|identity| identity.identity())
        .unwrap_or_default();
    let expected_table = machine
        .attached_data
        .as_ref()
        .map(|name| name.as_str())
        .unwrap_or_default();
    match (binding, selected) {
        (
            PackageReviewExternalBinding::Import { library, symbol },
            omega_effects::provider_plan::ProviderBinding::StringBackedImportBootstrap {
                library: selected_library,
                symbol: selected_symbol,
            },
        ) => library == selected_library && symbol == selected_symbol,
        (
            PackageReviewExternalBinding::Syscall { number },
            omega_effects::provider_plan::ProviderBinding::Syscall {
                number: selected_number,
            },
        ) => number == selected_number,
        (
            PackageReviewExternalBinding::CompilerIntrinsic,
            omega_effects::provider_plan::ProviderBinding::CompilerIntrinsic {
                machine: selected_machine,
            },
        ) => selected_machine == &expected_machine_identity,
        (
            PackageReviewExternalBinding::VtableSlot { index },
            omega_effects::provider_plan::ProviderBinding::VtableSlot {
                index: selected_index,
            },
        ) => index == selected_index,
        (
            PackageReviewExternalBinding::VtableField { field },
            omega_effects::provider_plan::ProviderBinding::VtableField {
                table,
                field: selected_field,
            },
        ) => table == expected_table && field == selected_field,
        (
            PackageReviewExternalBinding::TableFunction { field },
            omega_effects::provider_plan::ProviderBinding::TableFunction {
                table,
                field: selected_field,
            },
        ) => table == expected_table && field == selected_field,
        _ => false,
    }
}
