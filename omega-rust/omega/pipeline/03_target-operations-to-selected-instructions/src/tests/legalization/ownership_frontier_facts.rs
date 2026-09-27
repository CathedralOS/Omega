//! Ownership-frontier fact retention through construction and replay.

use crate::legalize_target_operations;
use crate::tests::fixtures::plain_unit::plain_unit_fixture;
use optimization_unit::{
    OwnershipFrontierFact, OwnershipFrontierSite, OwnershipFrontierSnapshot, PrunedMachineCustody,
    attach_ownership_frontier_facts,
};
use semantic_vocabulary::{BlockId, EdgeId, MachineId};

fn fact(
    unit: &optimization_unit::PsiOptimizationUnit,
    machine: MachineId,
    site: OwnershipFrontierSite,
) -> OwnershipFrontierFact {
    OwnershipFrontierFact::new(
        unit.psi,
        machine,
        site,
        OwnershipFrontierSnapshot {
            claims: Vec::new(),
            owned_places: Vec::new(),
            partial_custody: Vec::new(),
        },
    )
}

fn frontier_unit() -> (
    abstract_operations::AbstractOperationPlan,
    target_operations::TargetOperationPlan,
    optimization_unit::PsiOptimizationUnit,
    Vec<OwnershipFrontierFact>,
) {
    let (abstract_plan, target, mut unit) = plain_unit_fixture();
    let machine = MachineId::new(1).unwrap();
    let pruned = MachineId::new(2).unwrap();
    // The unit catalog is global: it can retain rows for machines an earlier
    // transform pruned, so the projection must filter by the function's own
    // machine rather than take the whole catalog.
    unit.pruned_machines.push(PrunedMachineCustody {
        machine: pruned,
        source_ordinal: 0,
    });
    // Rows are catalog-ordered by (machine, site).
    let retained = vec![
        fact(
            &unit,
            machine,
            OwnershipFrontierSite::BlockEntry(BlockId::new(1).unwrap()),
        ),
        fact(
            &unit,
            machine,
            OwnershipFrontierSite::EdgeEntry(EdgeId::new(1).unwrap()),
        ),
    ];
    let mut catalog = retained.clone();
    catalog.push(fact(
        &unit,
        pruned,
        OwnershipFrontierSite::BlockEntry(BlockId::new(1).unwrap()),
    ));
    let unit = attach_ownership_frontier_facts(unit, catalog).expect("canonical catalog");
    (abstract_plan, target, unit, retained)
}

#[test]
fn construction_projects_the_function_machine_scope() {
    let (abstract_plan, target, unit, retained) = frontier_unit();
    let legalized = legalize_target_operations(&target, &abstract_plan, &unit)
        .expect("frontier catalog legalizes");
    // Only this function's machine rows are retained; the pruned machine's
    // row stays in the unit catalog and out of the roster.
    assert_eq!(
        legalized.plan().scalar_functions[0].ownership_frontier_facts,
        retained
    );
    assert!(
        legalized.plan().scalar_functions[0]
            .ownership_frontier_facts
            .iter()
            .all(|fact| fact.machine == MachineId::new(1).unwrap())
    );
}
