use super::rejected;
use crate::capture::PackageReviewInput;
use crate::capture::semantics::declarations::nominal_identity;
use crate::record::{
    PackagePolicyEvaluatedBindingProducer, PackagePolicyProviderBinding,
    PackagePolicyProviderEvaluatedSyscall, PackageReviewForeignLocator,
    PackageReviewNominalIdentity,
};
use diagnostics::Diagnostic;
use effects::provider_plan::ProviderBinding;
use provider_planning::evaluated_via_bindings::{EvaluatedNativeBinding, EvaluatedViaBindingRow};
use symbols::SymbolHandle;

pub(super) fn project(
    compilation: &PackageReviewInput<'_>,
    identities: &effects::provider_plan::ProviderIdentityTable,
    binding: &ProviderBinding,
    requirement: SymbolHandle,
    realization: SymbolHandle,
    toolchain_settled: bool,
) -> Result<PackagePolicyProviderBinding, Vec<Diagnostic>> {
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
    Ok(match binding {
        ProviderBinding::Import { evaluated } => {
            let row = evaluated_row(compilation, requirement, realization)?.ok_or_else(|| {
                rejected("evaluated import has no exact selected realization row")
            })?;
            if row.evaluated().as_import() != Some(evaluated) {
                return Err(rejected(
                    "evaluated import differs from its selected binding",
                ));
            }
            PackagePolicyProviderBinding::Import {
                target: evaluated.locator().target().identity().as_str().to_owned(),
                locator: PackageReviewForeignLocator::from_candidate(evaluated.locator().locator()),
                producer: producer(compilation, row)?,
            }
        }
        ProviderBinding::Syscall { number } => {
            let evaluated = evaluated_row(compilation, requirement, realization)?
                .map(|row| {
                    let Some(syscall) = row.evaluated().as_syscall() else {
                        return Err(rejected("selected syscall has a non-syscall evaluated row"));
                    };
                    if syscall.number() != *number {
                        return Err(rejected(
                            "evaluated syscall number differs from selected binding",
                        ));
                    }
                    Ok(PackagePolicyProviderEvaluatedSyscall {
                        target: syscall.target().identity().as_str().to_owned(),
                        producer: producer(compilation, row)?,
                    })
                })
                .transpose()?;
            PackagePolicyProviderBinding::Syscall {
                number: *number,
                evaluated,
            }
        }
        ProviderBinding::CompilerIntrinsic { machine } => {
            PackagePolicyProviderBinding::CompilerIntrinsic {
                machine: machine.clone(),
            }
        }
        ProviderBinding::VtableSlot { index } => {
            PackagePolicyProviderBinding::VtableSlot { index: *index }
        }
        ProviderBinding::VtableField { table, field } => {
            PackagePolicyProviderBinding::VtableField {
                table: table.clone(),
                field: field.clone(),
                table_declaration: table_declaration(compilation, realization)?,
            }
        }
        ProviderBinding::TableFunction { table, field } => {
            PackagePolicyProviderBinding::TableFunction {
                table: table.clone(),
                field: field.clone(),
                table_declaration: table_declaration(compilation, realization)?,
            }
        }
        ProviderBinding::CheckedAdapter {
            machine_identity,
            machine_package_identity,
        } => PackagePolicyProviderBinding::CheckedAdapter {
            machine_identity: identities.text(*machine_identity).to_owned(),
            machine_package_identity: *machine_package_identity,
        },
    })
}

fn evaluated_row<'a>(
    compilation: &'a PackageReviewInput<'_>,
    requirement: SymbolHandle,
    realization: SymbolHandle,
) -> Result<Option<&'a EvaluatedViaBindingRow>, Vec<Diagnostic>> {
    let machines = compilation
        .machines()
        .iter()
        .filter(|machine| machine.symbol == realization)
        .collect::<Vec<_>>();
    let [machine] = machines.as_slice() else {
        return Err(rejected("binding realization has no exact typed machine"));
    };
    let conformances = compilation
        .machine_trait_conformances(machine)
        .iter()
        .filter(|conformance| {
            conformance.requirement_symbol == requirement && conformance.via_expression.is_valid()
        })
        .collect::<Vec<_>>();
    match conformances.as_slice() {
        [] => Ok(None),
        [conformance] => compilation
            .custody
            .providers()
            .evaluated_via_bindings
            .exact(realization, conformance.symbol, requirement)
            .map(Some)
            .ok_or_else(|| rejected("ordinary via expression has no exact evaluated row")),
        _ => Err(rejected(
            "binding realization has ambiguous evaluated conformance rows",
        )),
    }
}

fn producer(
    compilation: &PackageReviewInput<'_>,
    row: &EvaluatedViaBindingRow,
) -> Result<PackagePolicyEvaluatedBindingProducer, Vec<Diagnostic>> {
    let receipt = match row.evaluated() {
        EvaluatedNativeBinding::Import(import) => import.receipt(),
        EvaluatedNativeBinding::Syscall(syscall) => syscall.receipt(),
    };
    Ok(PackagePolicyEvaluatedBindingProducer {
        declaration: nominal_identity(compilation, row.producer_machine())?,
        package: receipt.producer_package(),
        callable_identity: compilation
            .custody
            .providers()
            .evaluated_via_bindings
            .identities()
            .text(receipt.producer_callable_identity())
            .unwrap_or_default()
            .to_owned(),
    })
}

fn table_declaration(
    compilation: &PackageReviewInput<'_>,
    realization: SymbolHandle,
) -> Result<PackageReviewNominalIdentity, Vec<Diagnostic>> {
    let machines = compilation
        .machines()
        .iter()
        .filter(|machine| machine.symbol == realization)
        .collect::<Vec<_>>();
    let [machine] = machines.as_slice() else {
        return Err(rejected("table binding has no exact realization machine"));
    };
    let tables = compilation
        .data_definitions()
        .iter()
        .filter(|definition| definition.symbol == machine.attached_data_symbol)
        .collect::<Vec<_>>();
    let [table] = tables.as_slice() else {
        return Err(rejected(
            "table binding has no exact attached data declaration",
        ));
    };
    nominal_identity(compilation, table.symbol)
}

pub(super) fn native_producer(
    compilation: &PackageReviewInput<'_>,
    native: &provider_planning::native_bindings::NativeBuildRowOrigin,
) -> Result<PackagePolicyEvaluatedBindingProducer, Vec<Diagnostic>> {
    let production = &native.origin.primary;
    let receipt = production.evaluated.receipt();
    let origin = receipt
        .build_origin()
        .ok_or_else(|| rejected("native Build row has no Build-origin receipt"))?;
    let requirement = compilation
        .typed
        .trait_machine_signature(native.requirement)
        .map(|(owner, signature)| {
            compilation
                .typed
                .trait_requirement_callable_identity_text(owner, signature)
        })
        .ok_or_else(|| rejected("native Build producer lost its exact requirement"))?;
    if origin.requirement_identity != requirement
        || receipt.producer_package() != Some(production.producer.package)
        || compilation
            .custody
            .providers()
            .evaluated_via_bindings
            .identities()
            .text(receipt.producer_callable_identity())
            != Some(production.producer.callable_identity.as_str())
    {
        return Err(rejected(
            "native Build producer receipt differs from its checked package source",
        ));
    }
    Ok(PackagePolicyEvaluatedBindingProducer {
        declaration: crate::record::PackageReviewNominalIdentity {
            owner: crate::record::PackageReviewNominalOwner::Package(production.producer.package),
            path: production.producer.declaration_path.clone(),
        },
        package: Some(production.producer.package),
        callable_identity: production.producer.callable_identity.clone(),
    })
}

pub(crate) fn project_native(
    compilation: &PackageReviewInput<'_>,
    native: &provider_planning::native_bindings::NativeBuildRowOrigin,
    selected: &ProviderBinding,
) -> Result<PackagePolicyProviderBinding, Vec<Diagnostic>> {
    if native.origin.primary.evaluated.provider_binding() != *selected {
        return Err(rejected(
            "native Build binding differs from its retained evaluated intent",
        ));
    }
    let producer = native_producer(compilation, native)?;
    match &native.origin.primary.evaluated {
        EvaluatedNativeBinding::Import(import) => Ok(PackagePolicyProviderBinding::Import {
            target: import.locator().target().identity().as_str().to_owned(),
            locator: PackageReviewForeignLocator::from_candidate(import.locator().locator()),
            producer,
        }),
        EvaluatedNativeBinding::Syscall(syscall) => Ok(PackagePolicyProviderBinding::Syscall {
            number: syscall.number(),
            evaluated: Some(PackagePolicyProviderEvaluatedSyscall {
                target: syscall.target().identity().as_str().to_owned(),
                producer,
            }),
        }),
    }
}
