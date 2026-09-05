use crate::capture::semantics::declarations::nominal_identity;
use crate::capture::source::{ProjectedNestedSourceLocation, ProjectedReviewRow};
use crate::record::{
    PackageReviewEvaluatedBindingUsage, PackageReviewEvaluatedImport,
    PackageReviewEvaluatedSyscall, PackageReviewExternalBinding,
    PackageReviewExternalExecutableSupply, PackageReviewForeignLocator,
    PackageReviewSourceLocationRole,
};
use compiler::CheckedCompilation;
use diagnostics::Diagnostic;

pub(super) fn project_external_executable_supply_with_source(
    machine: &typed_trees::machine::Machine,
    conformance: &typed_trees::machine::TraitConformance,
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

pub(super) fn project_evaluated_binding(
    compilation: &CheckedCompilation,
    row: &provider_planning::evaluated_via_bindings::EvaluatedViaBindingRow,
) -> Result<PackageReviewExternalBinding, Vec<Diagnostic>> {
    match row.evaluated() {
        provider_planning::evaluated_via_bindings::EvaluatedViaBinding::Import(evaluated) => {
            project_evaluated_import(compilation, row, evaluated)
        }
        provider_planning::evaluated_via_bindings::EvaluatedViaBinding::Syscall(evaluated) => {
            project_evaluated_syscall(compilation, row, evaluated)
        }
    }
}

fn project_evaluated_import(
    compilation: &CheckedCompilation,
    row: &provider_planning::evaluated_via_bindings::EvaluatedViaBindingRow,
    evaluated: &effects::provider_plan::EvaluatedForeignImport,
) -> Result<PackageReviewExternalBinding, Vec<Diagnostic>> {
    let locator = evaluated.locator();
    let projected_locator = match locator.locator() {
        target::ForeignLocatorCandidate::PeByName { library, export } => {
            PackageReviewForeignLocator::PeByName {
                library: library.clone(),
                export: export.clone(),
            }
        }
        target::ForeignLocatorCandidate::PeByOrdinal { library, ordinal } => {
            PackageReviewForeignLocator::PeByOrdinal {
                library: library.clone(),
                ordinal: *ordinal,
            }
        }
        target::ForeignLocatorCandidate::ElfVersioned {
            object,
            symbol,
            version,
        } => PackageReviewForeignLocator::ElfVersioned {
            object: object.clone(),
            symbol: symbol.clone(),
            version: version.clone(),
        },
        target::ForeignLocatorCandidate::MachODylibSymbol {
            install_name,
            symbol,
        } => PackageReviewForeignLocator::MachODylibSymbol {
            install_name: install_name.clone(),
            symbol: symbol.clone(),
        },
    };
    let receipt = evaluated.receipt();
    let usage = receipt.evaluation_usage();
    Ok(PackageReviewExternalBinding::NormalizedImport(
        PackageReviewEvaluatedImport {
            target: locator.target().identity().as_str().to_owned(),
            locator: projected_locator,
            locator_identity_digest: locator.identity_digest().as_bytes(),
            producer: nominal_identity(compilation, row.producer_machine())?,
            producer_package: receipt.producer_package(),
            producer_callable_identity: receipt.producer_callable_identity().to_owned(),
            producer_closure_digest: receipt.producer_closure_digest().as_bytes(),
            evaluator_semantics_marker: receipt.evaluator_semantics_marker(),
            evaluation_usage: PackageReviewEvaluatedBindingUsage {
                usage_schema_version: usage.usage_schema_version(),
                step_schedule_marker: usage.step_schedule_marker(),
                fuel_units: usage.fuel_units(),
                fuel_ceiling: usage.fuel_ceiling(),
                build_log_bytes: usage.build_log_bytes(),
                filesystem_operation_attempts: usage.filesystem_operation_attempts(),
                peak_live_cells: usage.peak_live_cells(),
                peak_live_text_bytes: usage.peak_live_text_bytes(),
                result_cells: usage.result_cells(),
                result_text_bytes: usage.result_text_bytes(),
            },
            evaluation_digest: receipt.evaluation_digest().as_bytes(),
            materializer_schema_version: receipt.materializer_schema_version(),
            materialization_digest: receipt.materialization_digest().as_bytes(),
            receipt_locator_identity_digest: receipt.locator_identity_digest().as_bytes(),
            receipt_identity_digest: receipt.identity_digest(),
        },
    ))
}

fn project_evaluated_syscall(
    compilation: &CheckedCompilation,
    row: &provider_planning::evaluated_via_bindings::EvaluatedViaBindingRow,
    evaluated: &effects::provider_plan::EvaluatedForeignSyscall,
) -> Result<PackageReviewExternalBinding, Vec<Diagnostic>> {
    let receipt = evaluated.receipt();
    let usage = receipt.evaluation_usage();
    Ok(PackageReviewExternalBinding::NormalizedSyscall(
        PackageReviewEvaluatedSyscall {
            target: evaluated.target().identity().as_str().to_owned(),
            number: evaluated.number(),
            binding_identity_digest: evaluated.identity_digest().as_bytes(),
            producer: nominal_identity(compilation, row.producer_machine())?,
            producer_package: receipt.producer_package(),
            producer_callable_identity: receipt.producer_callable_identity().to_owned(),
            producer_closure_digest: receipt.producer_closure_digest().as_bytes(),
            evaluator_semantics_marker: receipt.evaluator_semantics_marker(),
            evaluation_usage: PackageReviewEvaluatedBindingUsage {
                usage_schema_version: usage.usage_schema_version(),
                step_schedule_marker: usage.step_schedule_marker(),
                fuel_units: usage.fuel_units(),
                fuel_ceiling: usage.fuel_ceiling(),
                build_log_bytes: usage.build_log_bytes(),
                filesystem_operation_attempts: usage.filesystem_operation_attempts(),
                peak_live_cells: usage.peak_live_cells(),
                peak_live_text_bytes: usage.peak_live_text_bytes(),
                result_cells: usage.result_cells(),
                result_text_bytes: usage.result_text_bytes(),
            },
            evaluation_digest: receipt.evaluation_digest().as_bytes(),
            materializer_schema_version: receipt.materializer_schema_version(),
            materialization_digest: receipt.materialization_digest().as_bytes(),
            receipt_binding_identity_digest: receipt.locator_identity_digest().as_bytes(),
            receipt_identity_digest: receipt.identity_digest(),
        },
    ))
}

pub(super) fn validate_external_binding_payload(
    compilation: &CheckedCompilation,
    machine: &typed_trees::machine::Machine,
    identity: &language_semantics::ExternalBindingIdentity,
) -> Result<(), Vec<Diagnostic>> {
    use language_semantics::ExternalBindingIdentity;

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
    identity: &language_semantics::ExternalBindingIdentity,
) -> PackageReviewExternalBinding {
    match identity {
        language_semantics::ExternalBindingIdentity::Import { library, symbol } => {
            PackageReviewExternalBinding::Import {
                library: library.clone(),
                symbol: symbol.clone(),
            }
        }
        language_semantics::ExternalBindingIdentity::Syscall { number } => {
            PackageReviewExternalBinding::Syscall { number: *number }
        }
        language_semantics::ExternalBindingIdentity::CompilerIntrinsic => {
            PackageReviewExternalBinding::CompilerIntrinsic
        }
        language_semantics::ExternalBindingIdentity::VtableSlot { index } => {
            PackageReviewExternalBinding::VtableSlot { index: *index }
        }
        language_semantics::ExternalBindingIdentity::VtableField { field } => {
            PackageReviewExternalBinding::VtableField {
                field: field.clone(),
            }
        }
        language_semantics::ExternalBindingIdentity::TableFunction { field } => {
            PackageReviewExternalBinding::TableFunction {
                field: field.clone(),
            }
        }
    }
}

pub(super) fn external_binding_matches_provider_binding(
    compilation: &CheckedCompilation,
    machine: &typed_trees::machine::Machine,
    binding: &PackageReviewExternalBinding,
    selected: &effects::provider_plan::ProviderBinding,
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
            PackageReviewExternalBinding::NormalizedImport(reviewed),
            effects::provider_plan::ProviderBinding::Import { evaluated },
        ) => compilation
            .machine_trait_conformances(machine)
            .first()
            .and_then(|conformance| {
                compilation.evaluated_via_bindings().exact(
                    machine.symbol,
                    conformance.symbol,
                    conformance.requirement_symbol,
                )
            })
            .is_some_and(|row| {
                row.evaluated().as_import() == Some(evaluated)
                    && project_evaluated_binding(compilation, row).is_ok_and(|projected| {
                        projected
                            == PackageReviewExternalBinding::NormalizedImport(reviewed.clone())
                    })
            }),
        (
            PackageReviewExternalBinding::NormalizedSyscall(reviewed),
            effects::provider_plan::ProviderBinding::Syscall {
                number: selected_number,
            },
        ) => compilation
            .machine_trait_conformances(machine)
            .first()
            .and_then(|conformance| {
                compilation.evaluated_via_bindings().exact(
                    machine.symbol,
                    conformance.symbol,
                    conformance.requirement_symbol,
                )
            })
            .is_some_and(|row| {
                row.evaluated().as_syscall().is_some_and(|evaluated| {
                    evaluated.number() == *selected_number
                        && project_evaluated_binding(compilation, row).is_ok_and(|projected| {
                            projected
                                == PackageReviewExternalBinding::NormalizedSyscall(reviewed.clone())
                        })
                })
            }),
        (
            PackageReviewExternalBinding::Import { library, symbol },
            effects::provider_plan::ProviderBinding::StringBackedImportBootstrap {
                library: selected_library,
                symbol: selected_symbol,
            },
        ) => library == selected_library && symbol == selected_symbol,
        (
            PackageReviewExternalBinding::Syscall { number },
            effects::provider_plan::ProviderBinding::Syscall {
                number: selected_number,
            },
        ) => number == selected_number,
        (
            PackageReviewExternalBinding::CompilerIntrinsic,
            effects::provider_plan::ProviderBinding::CompilerIntrinsic {
                machine: selected_machine,
            },
        ) => selected_machine == &expected_machine_identity,
        (
            PackageReviewExternalBinding::VtableSlot { index },
            effects::provider_plan::ProviderBinding::VtableSlot {
                index: selected_index,
            },
        ) => index == selected_index,
        (
            PackageReviewExternalBinding::VtableField { field },
            effects::provider_plan::ProviderBinding::VtableField {
                table,
                field: selected_field,
            },
        ) => table == expected_table && field == selected_field,
        (
            PackageReviewExternalBinding::TableFunction { field },
            effects::provider_plan::ProviderBinding::TableFunction {
                table,
                field: selected_field,
            },
        ) => table == expected_table && field == selected_field,
        _ => false,
    }
}
