//! Follow the selected stored discriminator through an intact sum's normal cleanup.
use omega_psi::{BlockTerminator, CleanupAction, LoweredMachine, OperationKind, Parameter, PlaceOrigin, ValueOrigin};

pub fn return_cleanup_for_case(
    machine: &LoweredMachine,
    formal: usize,
    resident: &str,
) -> Result<Vec<CleanupAction>, &'static str> {
    let body = &machine.body;
    let Some(Parameter::Structural(root)) = body.parameters.get(formal) else {
        return Err("missing sum formal");
    };
    let mut current = 0;
    let mut actions = Vec::new();
    for _ in 0..body.blocks.len() {
        let block = body.blocks.get(current).ok_or("missing successor")?;
        let edge = match &block.terminator {
            BlockTerminator::Return { cleanup, .. } => {
                actions.extend_from_slice(cleanup);
                return Ok(actions);
            }
            BlockTerminator::Jump(edge) => edge,
            BlockTerminator::Conditional {
                condition,
                when_true,
                when_false,
            } => {
                let Some(ValueOrigin::Operation(op)) = body.values.get(condition.index()).map(|v| &v.origin) else {
                    return Err("branch has no defining operation");
                };
                let Some(OperationKind::StructuralCaseMembership { source, case }) = body.operations.get(op.index()).map(|o| &o.kind) else {
                    return Err("branch does not test a case");
                };
                if source.root != *root
                    || !source.path.is_empty()
                    || !matches!(body.places[root.index()].origin, PlaceOrigin::Parameter)
                    || !block.operations.contains(&op.index())
                {
                    return Err("the test does not read this sum here");
                }
                if case.path == resident { when_true } else { when_false }
            }
            _ => return Err("unexpected terminal transfer"),
        };
        if let Some(selected) = &edge.selected_case
            && (selected.source.root != *root || !selected.source.path.is_empty() || selected.case.path != resident)
        {
            return Err("edge claims a different active payload");
        }
        actions.extend_from_slice(&edge.cleanup);
        current = edge.target.get() - 1;
    }
    Err("cleanup fails to reach a return")
}

/// Follow cleanup staging through structural moves, keeping each original
/// case, array ordinal and record field on the resulting path.
pub fn original_place(
    machine: &LoweredMachine,
    place: &omega_psi::ProjectedPlace,
) -> omega_psi::ProjectedPlace {
    let body = &machine.body;
    let mut original = place.clone();
    while let PlaceOrigin::Operation(producer) = body.places[original.root.index()].origin {
        let OperationKind::MoveStructural { source } = &body.operations[producer.index()].kind else {
            break;
        };
        let mut path = source.path.clone();
        path.append(&mut original.path);
        original = omega_psi::ProjectedPlace { root: source.root, path };
    }
    original
}

/// The normal path through a sum cleanup, with call, operation, and edge order
/// preserved. Clients supply the actual runtime tag of every inspected place.
#[derive(Debug, PartialEq, Eq)]
pub enum NormalStep<'a> {
    Operation(&'a omega_psi::PsiOperation),
    Cleanup(&'a CleanupAction),
    Terminal {
        edge: omega_psi::EdgeId,
        callee: omega_psi::CallTarget,
        arguments: &'a [omega_psi::CallArgument],
    },
    Return(omega_psi::ReturnEdge),
}

pub fn normal_steps<'a>(
    machine: &'a LoweredMachine,
    resident: impl Fn(&omega_psi::ProjectedPlace) -> Option<&'static str>,
) -> Result<Vec<NormalStep<'a>>, &'static str> {
    let body = &machine.body;
    let mut current = 0;
    let mut steps = Vec::new();
    for _ in 0..body.blocks.len() {
        let block = body.blocks.get(current).ok_or("missing successor")?;
        let operations = body
            .operations
            .get(block.operations.clone())
            .ok_or("invalid operation range")?;
        steps.extend(operations.iter().map(NormalStep::Operation));
        let edge = match &block.terminator {
            BlockTerminator::Return { result, cleanup, .. } => {
                steps.extend(cleanup.iter().map(NormalStep::Cleanup));
                steps.push(NormalStep::Return(*result));
                return Ok(steps);
            }
            BlockTerminator::TerminalCall {
                id,
                callee,
                arguments,
                cleanup,
                completion,
                ..
            } => {
                steps.push(NormalStep::Terminal {
                    edge: *id,
                    callee: *callee,
                    arguments,
                });
                steps.extend(cleanup.iter().map(NormalStep::Cleanup));
                let Some(completion) = completion else { return Ok(steps) };
                current = completion.get() - 1;
                continue;
            }
            BlockTerminator::Jump(edge) => edge,
            BlockTerminator::Conditional {
                condition,
                when_true,
                when_false,
            } => {
                let Some(ValueOrigin::Operation(operation)) = body.values.get(condition.index()).map(|value| &value.origin) else {
                    return Err("branch has no defining operation");
                };
                let Some(OperationKind::StructuralCaseMembership { source, case }) = body
                    .operations
                    .get(operation.index())
                    .map(|operation| &operation.kind)
                else {
                    return Err("branch does not test a sum tag");
                };
                if !block.operations.contains(&operation.index()) {
                    return Err("the tag is not read in the selecting block");
                }
                if resident(&original_place(machine, source)).ok_or("sum has no supplied runtime tag")? == case.path {
                    when_true
                } else {
                    when_false
                }
            }
            BlockTerminator::ExternalExit(_) => return Err("no normal completion"),
        };
        if let Some(selection) = &edge.selected_case
            && resident(&original_place(machine, &selection.source)) != Some(selection.case.path.as_str())
        {
            return Err("edge claims an inactive case");
        }
        steps.extend(edge.cleanup.iter().map(NormalStep::Cleanup));
        current = edge.target.get() - 1;
    }
    Err("cleanup fails to reach a normal completion")
}
