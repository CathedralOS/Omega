use crate::capture::PackageReviewInput;
use crate::capture::semantics::declarations::{nominal_identity, producer_callable_identity};
use crate::record::{
    PackageReviewEvaluatedImport, PackageReviewEvaluatedSyscall, PackageReviewExternalBinding,
    PackageReviewForeignLocator,
};
use diagnostics::Diagnostic;

pub(super) fn project_evaluated_binding(
    compilation: &PackageReviewInput<'_>,
    row: &provider_planning::evaluated_via_bindings::EvaluatedViaBindingRow,
) -> Result<PackageReviewExternalBinding, Vec<Diagnostic>> {
    match row.evaluated() {
        provider_planning::evaluated_via_bindings::EvaluatedNativeBinding::Import(evaluated) => {
            project_evaluated_import(compilation, row, evaluated)
        }
        provider_planning::evaluated_via_bindings::EvaluatedNativeBinding::Syscall(evaluated) => {
            project_evaluated_syscall(compilation, row, evaluated)
        }
    }
}

fn project_evaluated_import(
    compilation: &PackageReviewInput<'_>,
    row: &provider_planning::evaluated_via_bindings::EvaluatedViaBindingRow,
    evaluated: &effects::provider_plan::EvaluatedForeignImport,
) -> Result<PackageReviewExternalBinding, Vec<Diagnostic>> {
    let locator = evaluated.locator();
    let projected_locator = PackageReviewForeignLocator::from_candidate(locator.locator());
    let receipt = evaluated.receipt();
    let usage = receipt.evaluation_usage();
    Ok(PackageReviewExternalBinding::NormalizedImport(
        PackageReviewEvaluatedImport {
            target: locator.target().identity().as_str().to_owned(),
            locator: projected_locator,
            locator_identity_digest: locator.identity_digest().as_bytes(),
            producer: nominal_identity(compilation, row.producer_machine())?,
            producer_package: receipt.producer_package(),
            producer_callable_identity: producer_callable_identity(compilation, receipt),
            producer_closure_digest: receipt.producer_closure_digest().as_bytes(),
            evaluator_semantics_marker: receipt.evaluator_semantics_marker(),
            evaluation_usage: usage,
            evaluation_digest: receipt.evaluation_digest().as_bytes(),
            materializer_schema_version: receipt.materializer_schema_version(),
            materialization_digest: receipt.materialization_digest().as_bytes(),
            receipt_locator_identity_digest: receipt.locator_identity_digest().as_bytes(),
            receipt_identity_digest: receipt.identity_digest(),
        },
    ))
}

fn project_evaluated_syscall(
    compilation: &PackageReviewInput<'_>,
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
            producer_callable_identity: producer_callable_identity(compilation, receipt),
            producer_closure_digest: receipt.producer_closure_digest().as_bytes(),
            evaluator_semantics_marker: receipt.evaluator_semantics_marker(),
            evaluation_usage: usage,
            evaluation_digest: receipt.evaluation_digest().as_bytes(),
            materializer_schema_version: receipt.materializer_schema_version(),
            materialization_digest: receipt.materialization_digest().as_bytes(),
            receipt_binding_identity_digest: receipt.locator_identity_digest().as_bytes(),
            receipt_identity_digest: receipt.identity_digest(),
        },
    ))
}

/// Project one retained typed external-binding identity to its review row,
/// validating the payload first. There is no string-backed import arm to
/// reject any more: two authored strings are not a physical locator, and the
/// typed identity that spelled them no longer exists.
pub(super) fn project_external_binding(
    compilation: &PackageReviewInput<'_>,
    machine: &typed_trees::machine::Machine,
    identity: &language_semantics::ExternalBindingIdentity,
) -> Result<PackageReviewExternalBinding, Vec<Diagnostic>> {
    use language_semantics::ExternalBindingIdentity;

    let rejected = |reason: &str| {
        Err(vec![Diagnostic::error(format!(
            "reviewed external callable `{}` {reason}",
            machine.name
        ))])
    };
    match identity {
        ExternalBindingIdentity::Syscall { number } if u32::try_from(*number).is_err() => {
            rejected("has a syscall number outside 0..=u32::MAX")
        }
        ExternalBindingIdentity::VtableSlot { index } if *index < 0 => {
            rejected("has a negative vtable-slot index")
        }
        ExternalBindingIdentity::VtableField { field }
        | ExternalBindingIdentity::TableFunction { field }
            if field.is_empty() =>
        {
            rejected("has no exact table-field identity")
        }
        ExternalBindingIdentity::VtableField { .. }
        | ExternalBindingIdentity::TableFunction { .. }
            if !machine.attached_data_symbol.is_valid()
                || machine.attached_data.is_none()
                || !compilation
                    .data_definition_by_symbol(machine.attached_data_symbol)
                    .is_some() =>
        {
            rejected("has table-field supply without one exact attached provider data declaration")
        }
        ExternalBindingIdentity::Syscall { number } => {
            Ok(PackageReviewExternalBinding::Syscall { number: *number })
        }
        ExternalBindingIdentity::CompilerIntrinsic => {
            Ok(PackageReviewExternalBinding::CompilerIntrinsic)
        }
        ExternalBindingIdentity::VtableSlot { index } => {
            Ok(PackageReviewExternalBinding::VtableSlot { index: *index })
        }
        ExternalBindingIdentity::VtableField { field } => {
            Ok(PackageReviewExternalBinding::VtableField {
                field: field.clone(),
            })
        }
        ExternalBindingIdentity::TableFunction { field } => {
            Ok(PackageReviewExternalBinding::TableFunction {
                field: field.clone(),
            })
        }
    }
}

/// The one evaluated-via row this machine's first trait conformance selects,
/// if custody retained it.
fn evaluated_row<'a>(
    compilation: &'a PackageReviewInput<'_>,
    machine: &typed_trees::machine::Machine,
) -> Option<&'a provider_planning::evaluated_via_bindings::EvaluatedViaBindingRow> {
    let conformance = compilation.machine_trait_conformances(machine).first()?;
    compilation
        .custody
        .providers()
        .evaluated_via_bindings
        .exact(
            machine.symbol,
            conformance.symbol,
            conformance.requirement_symbol,
        )
}

pub(super) fn external_binding_matches_provider_binding(
    compilation: &PackageReviewInput<'_>,
    machine: &typed_trees::machine::Machine,
    binding: &PackageReviewExternalBinding,
    selected: &effects::provider_plan::ProviderBinding,
) -> bool {
    let expected_machine_identity = compilation
        .machine_callable_identity_text(machine)
        .unwrap_or_default();
    let expected_table = compilation
        .typed
        .attached_data_path(machine)
        .unwrap_or_default();
    match (binding, selected) {
        (
            PackageReviewExternalBinding::NormalizedImport(reviewed),
            effects::provider_plan::ProviderBinding::Import { evaluated },
        ) => evaluated_row(compilation, machine).is_some_and(|row| {
            row.evaluated().as_import() == Some(evaluated)
                && project_evaluated_binding(compilation, row).is_ok_and(|projected| {
                    projected == PackageReviewExternalBinding::NormalizedImport(reviewed.clone())
                })
        }),
        (
            PackageReviewExternalBinding::NormalizedSyscall(reviewed),
            effects::provider_plan::ProviderBinding::Syscall {
                number: selected_number,
            },
        ) => evaluated_row(compilation, machine).is_some_and(|row| {
            row.evaluated().as_syscall().is_some_and(|evaluated| {
                evaluated.number() == *selected_number
                    && project_evaluated_binding(compilation, row).is_ok_and(|projected| {
                        projected
                            == PackageReviewExternalBinding::NormalizedSyscall(reviewed.clone())
                    })
            })
        }),
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
        ) => table == &expected_table && field == selected_field,
        (
            PackageReviewExternalBinding::TableFunction { field },
            effects::provider_plan::ProviderBinding::TableFunction {
                table,
                field: selected_field,
            },
        ) => table == &expected_table && field == selected_field,
        _ => false,
    }
}
