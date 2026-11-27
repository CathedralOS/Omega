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

/// Reconstruct pending extent-equality conditions for array literals whose
/// target type has a target-dependent `ArrayExtent::Application`.
pub fn array_extent_literal_conditions(
    module: &TerminalModule,
    machine: &TerminalMachine,
) -> Result<Vec<ScalarPredicateCondition>, &'static str> {
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
        for operation in &block.operations {
            let count = match &operation.kind {
                OperationKind::EstablishScalarArray { elements } => elements.len(),
                OperationKind::EstablishStructuralArray { elements } => elements.len(),
                _ => continue,
            };
            let Some(result) = operation.result.structural() else {
                continue;
            };
            let Some(terminal_psi::StructuralTypeShape::FixedArray {
                length: terminal_psi::ArrayExtent::Application(application),
                ..
            }) = module
                .structural_type(result.structural_type)
                .map(|declaration| &declaration.shape)
            else {
                continue;
            };
            let scalar_type = applications
                .declaration(*application)
                .and_then(|declaration| declaration.scalar_type())
                .ok_or("array literal extent has no declared scalar application")?;
            let Some(integer) = scalar_type.as_integer() else {
                return Err("array literal extent is not an integer application");
            };
            let literal = u64::try_from(count)
                .ok()
                .and_then(|count| {
                    ScalarTerm::integer(integer, IntegerValue::Unsigned(u128::from(count))).ok()
                })
                .ok_or("array literal count does not fit in its extent carrier")?;
            let goal = Proposition::Equal(
                ScalarTerm::Application {
                    application: *application,
                    scalar_type,
                },
                literal,
            );
            let condition = applications.layout_predicate(
                owner,
                || {
                    format!(
                        "array literal length ({count}) matches target-dependent extent in {:?}",
                        machine.id
                    )
                },
                &goal,
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
                conditions.push(condition);
            }
        }
    }
    Ok(conditions)
}
