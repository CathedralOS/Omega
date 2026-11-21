//! Reconstruct a pending call prerequisite at the block that actually invokes
//! the callee. Saved artifacts and source publication use the same derivation.

use super::*;
use terminal_psi::ScalarPredicateCondition;

pub fn layout_call_conditions(
    module: &TerminalModule,
    machine: &TerminalMachine,
) -> Result<Vec<(ObligationId, ScalarPredicateCondition)>, &'static str> {
    let applications = &module.scalars.scalar_applications;
    if applications.declarations().is_empty() {
        return Ok(Vec::new());
    }
    let owner = applications
        .proof_scope
        .callable_declaration(TargetMachineCallable::Machine(machine.id));
    let index = OnceCell::new();
    let dominators = OnceCell::new();
    let storage = OnceCell::new();
    let branches = OnceCell::new();
    let mut conditions = Vec::new();
    for block in &machine.blocks {
        let paths = OnceCell::new();
        let calls = block
            .module_invocations()
            .map(|call| {
                let inputs = call.inputs.declared();
                (
                    inputs.callee,
                    inputs.required_contract,
                    inputs.requirement_obligations,
                )
            })
            .chain(block.boundary_invocations().map(|call| {
                (
                    TargetMachineCallable::Boundary(call.boundary),
                    None,
                    call.requirement_obligations,
                )
            }));
        for (callee, required, obligations) in calls {
            let Some(clauses) = module.invocation_requires(callee, required) else {
                continue;
            };
            for (clause, obligation) in clauses.iter().zip(obligations) {
                let condition = applications.layout_predicate(
                    owner,
                    || {
                        format!(
                            "layout-dependent call requirement {} of {callee:?}",
                            obligation.get()
                        )
                    },
                    clause,
                    || {
                        paths
                            .get_or_init(|| {
                                let mut trace = GroundRange {
                                    module,
                                    index: index.get_or_init(|| MachineScalarIndex::new(machine)),
                                    storage: &storage,
                                    dominators: &dominators,
                                    active: Vec::new(),
                                    steps: Vec::new(),
                                    remaining: 4096,
                                };
                                trace.call_guards(
                                    block.id,
                                    branches.get_or_init(|| guards::branches(machine)),
                                )
                            })
                            .as_slice()
                    },
                )?;
                if let Some(condition) = condition {
                    conditions.push((*obligation, condition));
                }
            }
        }
    }
    conditions.sort_unstable_by_key(|(obligation, _)| *obligation);
    if conditions.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return Err("layout call requirements reuse an obligation identity");
    }
    Ok(conditions)
}
