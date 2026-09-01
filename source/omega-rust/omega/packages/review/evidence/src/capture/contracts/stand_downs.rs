//! Exact package projection of compiler-retained contract-entailment stand-downs.

use super::facts::exact_contract_entailment_stand_down_contract;
use crate::capture::callables::project_contract_entailment_open_contract;
use crate::capture::semantics::declarations::nominal_identity;
use crate::capture::source::ProjectedReviewRow;
use crate::capture::source::contracts::project_contract_source_locations;
use crate::record::{
    PackageReviewContractEntailmentOpenObligation, PackageReviewContractEntailmentOpenReason,
    PackageReviewContractKind,
};
use omega_compiler::CheckedCompilation;
use psi_core::PackageKeyIdentity;
use psi_diagnostics::Diagnostic;
use psi_language_semantics::MachineSupplyMode;

pub(in crate::capture) fn project_package_contract_entailment_open_obligations(
    compilation: &CheckedCompilation,
    package: PackageKeyIdentity,
) -> Result<Vec<ProjectedReviewRow<PackageReviewContractEntailmentOpenObligation>>, Vec<Diagnostic>>
{
    let mut projected = Vec::new();
    for stand_down in compilation
        .contract_entailment_stand_downs()
        .iter()
        .filter(|stand_down| {
            compilation
                .symbols
                .symbol_package_identity(stand_down.machine_symbol)
                == Some(package)
        })
    {
        let machine = compilation
            .machines()
            .iter()
            .find(|machine| machine.symbol == stand_down.machine_symbol)
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "contract-entailment stand-down names a missing package callable",
                )]
            })?;
        if !matches!(
            machine.supply_mode,
            MachineSupplyMode::CheckedBody | MachineSupplyMode::Boundary
        ) {
            return Err(vec![Diagnostic::error(
                "contract-entailment stand-down is not owned by a checked implementation",
            )]);
        }
        let callable = nominal_identity(compilation, machine.symbol)?;
        let contract_position = u32::try_from(stand_down.contract_index).map_err(|_| {
            vec![Diagnostic::error(
                "contract-entailment stand-down contract position exceeds canonical u32",
            )]
        })?;
        let fact_position = u32::try_from(stand_down.fact_index).map_err(|_| {
            vec![Diagnostic::error(
                "contract-entailment stand-down fact position exceeds canonical u32",
            )]
        })?;
        let machine_contract_commitment = compilation
            .facts
            .contract_plans
            .for_machine(machine.symbol)
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "contract-entailment stand-down callable has no checked contract plan",
                )]
            })?
            .commitment
            .as_bytes();
        if machine_contract_commitment == [0; 32] {
            return Err(vec![Diagnostic::error(
                "contract-entailment stand-down callable has a zero contract commitment",
            )]);
        }
        let goal = project_contract_entailment_open_contract(
            compilation,
            machine,
            stand_down.contract_index,
            stand_down.fact_index,
        )?;
        if goal.kind() != PackageReviewContractKind::Ensures || goal.result_case().is_some() {
            return Err(vec![Diagnostic::error(
                "contract-entailment stand-down does not name one plain ensures goal",
            )]);
        }
        let exact_source_contract = exact_contract_entailment_stand_down_contract(
            compilation,
            machine,
            stand_down.contract_index,
            stand_down.fact_index,
        )?;
        let nested_source_locations = project_contract_source_locations(
            compilation,
            std::slice::from_ref(&exact_source_contract),
        )?;
        let reason = match stand_down.reason {
            psi_validation::ContractEntailmentStandDownReason::UnsupportedEnsuresFact => {
                PackageReviewContractEntailmentOpenReason::UnsupportedEnsuresFact
            }
            psi_validation::ContractEntailmentStandDownReason::UnrecognizedInductiveBody => {
                PackageReviewContractEntailmentOpenReason::UnrecognizedInductiveBody
            }
            psi_validation::ContractEntailmentStandDownReason::OutsideEntailmentLanguage => {
                PackageReviewContractEntailmentOpenReason::OutsideEntailmentLanguage
            }
        };
        projected.push(ProjectedReviewRow {
            row: PackageReviewContractEntailmentOpenObligation {
                callable,
                contract_position,
                fact_position,
                machine_contract_commitment,
                goal,
                reason,
            },
            declaration: machine.symbol,
            nested_source_locations,
        });
    }

    projected.sort_by(|left, right| left.row.cmp(&right.row));
    if projected.windows(2).any(|rows| {
        rows[0].row.callable == rows[1].row.callable
            && rows[0].row.contract_position == rows[1].row.contract_position
            && rows[0].row.fact_position == rows[1].row.fact_position
    }) {
        return Err(vec![Diagnostic::error(
            "package review contains a duplicate contract-entailment stand-down coordinate",
        )]);
    }
    Ok(projected)
}
