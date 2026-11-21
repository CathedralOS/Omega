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
