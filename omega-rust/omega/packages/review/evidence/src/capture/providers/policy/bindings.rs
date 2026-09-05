use super::rejected;
use crate::capture::semantics::declarations::nominal_identity;
use crate::record::{
    PackagePolicyEvaluatedBindingProducer, PackagePolicyProviderBinding,
    PackagePolicyProviderEvaluatedSyscall, PackageReviewForeignLocator,
    PackageReviewNominalIdentity,
};
use compiler::CheckedCompilation;
use diagnostics::Diagnostic;
use effects::provider_plan::ProviderBinding;
use provider_planning::evaluated_via_bindings::{EvaluatedViaBinding, EvaluatedViaBindingRow};
use symbols::SymbolHandle;

pub(super) fn project(
    compilation: &CheckedCompilation,
    binding: &ProviderBinding,
    requirement: SymbolHandle,
    realization: SymbolHandle,
) -> Result<PackagePolicyProviderBinding, Vec<Diagnostic>> {
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
                locator: locator(evaluated.locator().locator()),
                producer: producer(compilation, row)?,
            }
        }
        ProviderBinding::StringBackedImportBootstrap { library, symbol } => {
            PackagePolicyProviderBinding::StringBackedImportBootstrap {
                library: library.clone(),
                symbol: symbol.clone(),
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
            machine_identity: machine_identity.clone(),
            machine_package_identity: *machine_package_identity,
        },
    })
}

fn evaluated_row(
    compilation: &CheckedCompilation,
    requirement: SymbolHandle,
    realization: SymbolHandle,
) -> Result<Option<&EvaluatedViaBindingRow>, Vec<Diagnostic>> {
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
            .evaluated_via_bindings()
            .exact(realization, conformance.symbol, requirement)
            .map(Some)
            .ok_or_else(|| rejected("ordinary via expression has no exact evaluated row")),
        _ => Err(rejected(
            "binding realization has ambiguous evaluated conformance rows",
        )),
    }
}

fn producer(
    compilation: &CheckedCompilation,
    row: &EvaluatedViaBindingRow,
) -> Result<PackagePolicyEvaluatedBindingProducer, Vec<Diagnostic>> {
    let receipt = match row.evaluated() {
        EvaluatedViaBinding::Import(import) => import.receipt(),
        EvaluatedViaBinding::Syscall(syscall) => syscall.receipt(),
    };
    Ok(PackagePolicyEvaluatedBindingProducer {
        declaration: nominal_identity(compilation, row.producer_machine())?,
        package: receipt.producer_package(),
        callable_identity: receipt.producer_callable_identity().to_owned(),
    })
}

fn table_declaration(
    compilation: &CheckedCompilation,
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

fn locator(locator: &target::ForeignLocatorCandidate) -> PackageReviewForeignLocator {
    match locator {
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
    }
}
