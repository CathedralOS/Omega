use super::rejected;
use crate::capture::PackageReviewInput;
use crate::capture::semantics::declarations::nominal_identity;
use crate::record::{
    PackagePolicyEvaluatedBindingProducer, PackagePolicyProviderBinding,
    PackagePolicyProviderEvaluatedSyscall, PackageReviewForeignLocator,
};
use diagnostics::Diagnostic;
use effects::provider_plan::ProviderBinding;
use symbols::SymbolHandle;

pub(super) fn project(
    compilation: &PackageReviewInput<'_>,
    identities: &effects::provider_plan::ProviderIdentityTable,
    binding: &ProviderBinding,
    requirement: SymbolHandle,
    origin: provider_planning::ProviderRowOrigin,
    toolchain_settled: bool,
) -> Result<PackagePolicyProviderBinding, Vec<Diagnostic>> {
    if let provider_planning::ProviderRowOrigin::BuildNative {
        selecting_machine,
        source_span,
        event_ordinal,
        ..
    } = origin
    {
        let mut rows = compilation
            .custody
            .providers()
            .admitted_native_bindings
            .iter()
            .filter(|row| {
                row.requirement == requirement
                    && row.selecting_machine == selecting_machine
                    && row.source_span == source_span
                    && row.event_ordinal == event_ordinal
            });
        let row = rows
            .next()
            .filter(|_| rows.next().is_none())
            .ok_or_else(|| {
                rejected("Build-native row lost its exact admitted source occurrence")
            })?;
        if row.binding().as_ref() != Some(binding) {
            return Err(rejected(
                "Build-native selected binding differs from its admitted locator",
            ));
        }
        let receipt = row.receipt();
        let producer = PackagePolicyEvaluatedBindingProducer {
            declaration: nominal_identity(compilation, selecting_machine)?,
            package: receipt.producer_package(),
            callable_identity: row
                .identities
                .text(receipt.producer_callable_identity())
                .ok_or_else(|| rejected("Build-native receipt lost its callable identity"))?
                .to_owned(),
        };
        return match &row.locator {
            provider_planning::AdmittedNativeLocator::Unavailable { .. } => Err(rejected(
                "unavailable native intent cannot supply a selected physical row",
            )),
            provider_planning::AdmittedNativeLocator::Import(value) => {
                Ok(PackagePolicyProviderBinding::Import {
                    target: value.locator().target().identity().as_str().to_owned(),
                    locator: PackageReviewForeignLocator::from_candidate(value.locator().locator()),
                    producer,
                })
            }
            provider_planning::AdmittedNativeLocator::Table {
                identity,
                table_name,
                field_name,
                drops_receiver,
                ..
            } => {
                let table_declaration = nominal_identity(compilation, identity.table)?;
                if *drops_receiver {
                    Ok(PackagePolicyProviderBinding::TableFunction {
                        table: table_name.clone(),
                        field: field_name.clone(),
                        table_declaration,
                    })
                } else {
                    Ok(PackagePolicyProviderBinding::VtableField {
                        table: table_name.clone(),
                        field: field_name.clone(),
                        table_declaration,
                    })
                }
            }
            provider_planning::AdmittedNativeLocator::Syscall(value) => {
                Ok(PackagePolicyProviderBinding::Syscall {
                    number: value.number(),
                    evaluated: Some(PackagePolicyProviderEvaluatedSyscall {
                        target: value.target().identity().as_str().to_owned(),
                        producer,
                    }),
                })
            }
        };
    }
    if toolchain_settled {
        // A toolchain-settled row's binding comes from the settlement table:
        // only positional syscalls exist there, and they carry no evaluated
        // `via` receipt because no authored machine realized the row.
        return match binding {
            ProviderBinding::Syscall { number } => Ok(PackagePolicyProviderBinding::Syscall {
                number: *number,
                evaluated: None,
            }),
            _ => Err(rejected(
                "toolchain-settled row binds a non-syscall realization",
            )),
        };
    }
    if origin.machine().is_none() {
        return Err(rejected("authored binding lost its realization machine"));
    }
    Ok(match binding {
        ProviderBinding::CompilerIntrinsic { machine } => {
            PackagePolicyProviderBinding::CompilerIntrinsic {
                machine: machine.clone(),
            }
        }
        ProviderBinding::CheckedAdapter {
            machine_identity,
            machine_package_identity,
        } => PackagePolicyProviderBinding::CheckedAdapter {
            machine_identity: identities.text(*machine_identity).to_owned(),
            machine_package_identity: *machine_package_identity,
        },
        ProviderBinding::Import { .. }
        | ProviderBinding::Syscall { .. }
        | ProviderBinding::VtableSlot { .. }
        | ProviderBinding::VtableField { .. }
        | ProviderBinding::TableFunction { .. } => {
            return Err(rejected(
                "physical provider supply requires an exact Build-native request origin",
            ));
        }
    })
}
