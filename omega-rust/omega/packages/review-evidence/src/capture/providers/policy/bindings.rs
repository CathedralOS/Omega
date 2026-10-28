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

pub(crate) fn project(
    compilation: &PackageReviewInput<'_>,
    identities: &effects::provider_plan::ProviderIdentityTable,
    binding: &ProviderBinding,
    requirement: SymbolHandle,
    origin: provider_planning::ProviderRowOrigin,
    toolchain_settled: bool,
) -> Result<PackagePolicyProviderBinding, Vec<Diagnostic>> {
    if let provider_planning::ProviderRowOrigin::BuildNative {
        receipt: receipt_identity,
        ..
    } = origin
    {
        let row = native_row(compilation, requirement, receipt_identity)?;
        if row.binding().as_ref() != Some(binding) {
            return Err(rejected(
                "Build-native selected binding differs from its admitted locator",
            ));
        }
        let producer = native_producer(compilation, row)?;
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

/// Rejoin an exact admitted occurrence. A matching physical locator is not an
/// origin: independent Build events retain their separate source receipts.
pub(crate) fn native_row<'a>(
    compilation: &'a PackageReviewInput<'_>,
    requirement: SymbolHandle,
    receipt_identity: [u8; 32],
) -> Result<&'a provider_planning::AdmittedNativeBinding, Vec<Diagnostic>> {
    let mut rows = compilation
        .custody
        .providers()
        .admitted_native_bindings
        .iter()
        .filter(|row| {
            row.requirement == requirement && row.receipt().identity_digest() == receipt_identity
        });
    let row = rows
        .next()
        .filter(|_| rows.next().is_none())
        .ok_or_else(|| rejected("Build-native row lost its exact admitted source occurrence"))?;
    Ok(row)
}

pub(crate) fn native_producer(
    compilation: &PackageReviewInput<'_>,
    row: &provider_planning::AdmittedNativeBinding,
) -> Result<PackagePolicyEvaluatedBindingProducer, Vec<Diagnostic>> {
    let receipt = row.receipt();
    let producer = PackagePolicyEvaluatedBindingProducer {
        declaration: match row.selecting_machine {
            Some(author) => nominal_identity(compilation, author)?,
            None => {
                let retained = row
                    .retained
                    .as_ref()
                    .ok_or_else(|| rejected("imported native row lost original author custody"))?;
                let package = retained
                    .origin
                    .author()
                    .source()
                    .package_identity
                    .ok_or_else(|| rejected("imported native author has no immutable supplier"))?;
                crate::record::PackageReviewNominalIdentity {
                    owner: crate::record::PackageReviewNominalOwner::Package(package),
                    path: retained.author_path.clone(),
                }
            }
        },
        package: receipt.producer_package(),
        callable_identity: row
            .identities
            .text(receipt.producer_callable_identity())
            .ok_or_else(|| rejected("Build-native receipt lost its callable identity"))?
            .to_owned(),
    };
    Ok(producer)
}
