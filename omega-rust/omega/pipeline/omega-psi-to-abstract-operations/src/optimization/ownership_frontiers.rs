use super::{VerifiedPsiOptimizationInput, VerifiedPsiOptimizationUnitBuildError};
use crate::shared::*;

pub(super) fn project_ownership_frontiers(
    input: &VerifiedPsiOptimizationInput,
) -> Result<
    Vec<omega_optimization_unit::OwnershipFrontierFact>,
    VerifiedPsiOptimizationUnitBuildError,
> {
    use omega_optimization_unit::OwnershipFrontierSite as Site;

    let mut facts = Vec::new();
    let context = input.context();
    for machine in &context.module().machines {
        let frontiers = context.structural_frontiers().machine(machine.id).ok_or(
            VerifiedPsiOptimizationUnitBuildError::MissingStructuralFrontierMachine(machine.id),
        )?;
        for block in &machine.blocks {
            push_ownership_frontier(
                &mut facts,
                input.plan().psi,
                machine.id,
                Site::BlockEntry(block.id),
                frontiers.block_entry(block.id),
            )?;
            for operation in &block.operations {
                push_ownership_frontier(
                    &mut facts,
                    input.plan().psi,
                    machine.id,
                    Site::OperationEntry(operation.id),
                    frontiers.operation_entry(operation.id),
                )?;
                push_ownership_frontier(
                    &mut facts,
                    input.plan().psi,
                    machine.id,
                    Site::OperationExit(operation.id),
                    frontiers.operation_exit(operation.id),
                )?;
            }
            for edge in block.terminator.edges() {
                push_ownership_frontier(
                    &mut facts,
                    input.plan().psi,
                    machine.id,
                    Site::EdgeEntry(edge),
                    frontiers.edge_entry(edge),
                )?;
                if let Some(snapshot) = frontiers.edge_exit(edge) {
                    facts.push(omega_optimization_unit::OwnershipFrontierFact::new(
                        input.plan().psi,
                        machine.id,
                        Site::EdgeExit(edge),
                        ownership_frontier_snapshot(snapshot),
                    ));
                }
            }
        }
    }
    facts.sort_by_key(|fact| (fact.machine, fact.site));
    Ok(facts)
}

fn push_ownership_frontier(
    facts: &mut Vec<omega_optimization_unit::OwnershipFrontierFact>,
    psi: psi_terminal::TerminalPsiIdentity,
    machine: MachineId,
    site: omega_optimization_unit::OwnershipFrontierSite,
    snapshot: Option<&psi_terminal_verifier::VerifiedStructuralOwnershipFrontier>,
) -> Result<(), VerifiedPsiOptimizationUnitBuildError> {
    let snapshot = snapshot.ok_or(
        VerifiedPsiOptimizationUnitBuildError::MissingStructuralFrontier { machine, site },
    )?;
    facts.push(omega_optimization_unit::OwnershipFrontierFact::new(
        psi,
        machine,
        site,
        ownership_frontier_snapshot(snapshot),
    ));
    Ok(())
}

fn ownership_frontier_snapshot(
    snapshot: &psi_terminal_verifier::VerifiedStructuralOwnershipFrontier,
) -> omega_optimization_unit::OwnershipFrontierSnapshot {
    omega_optimization_unit::OwnershipFrontierSnapshot {
        claims: snapshot
            .claims()
            .iter()
            .map(
                |claim| omega_optimization_unit::OwnershipFrontierLiveClaim {
                    claim: claim.claim,
                    input: claim.input,
                    path: claim.path.clone(),
                    multiplicity: claim.multiplicity,
                },
            )
            .collect(),
        owned_places: snapshot
            .owned_places()
            .iter()
            .map(
                |place| omega_optimization_unit::OwnershipFrontierOwnedPlace {
                    place: place.place,
                    multiplicity: place.multiplicity,
                },
            )
            .collect(),
        partial_custody: snapshot
            .partial_custody()
            .iter()
            .map(
                |partial| omega_optimization_unit::OwnershipFrontierPartialCustody {
                    place: partial.place,
                    moved_paths: partial.moved_paths.clone(),
                },
            )
            .collect(),
    }
}
