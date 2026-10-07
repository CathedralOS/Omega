//! The shared depth-first walk of one state's retained computation roots.
//!
//! `owned_parameters` and `primitive_locals` audit the same graph with the
//! same pending/visited discipline and expand every non-`Call` arm's operand
//! children identically. `Call` nodes defer to the site's visitor, which
//! pushes the further operands it needs onto `pending` through
//! `expand_call_operands`.
use checked_trees::{
    CheckedScalarComputationHandle, CheckedScalarComputationKind, CheckedScalarComputationPlans,
    CheckedScalarComputationRoot,
};
use symbols::SymbolHandle;

/// The retained computation roots of one state, in fact order. Each site
/// gates the root's machine contract itself before walking it.
pub(super) fn state_computation_roots<'a>(
    computations: &'a CheckedScalarComputationPlans,
    state: SymbolHandle,
) -> impl Iterator<Item = &'a CheckedScalarComputationRoot> + 'a {
    computations
        .roots
        .iter()
        .filter(move |(_, root)| root.state == state)
        .map(|(_, root)| root)
}

/// One root's depth-first walk: a handle visited twice is skipped, an
/// invalid handle refuses the state. Every arm but `Call` expands its
/// operand children the same way both audits spell; `Call` defers to
/// `visit`, which returns None to refuse the state.
pub(super) fn walk_computation(
    computations: &CheckedScalarComputationPlans,
    root: &CheckedScalarComputationRoot,
    mut visit: impl FnMut(
        &mut Vec<CheckedScalarComputationHandle>,
        &CheckedScalarComputationKind,
        &CheckedScalarComputationRoot,
    ) -> Option<()>,
) -> Option<()> {
    let mut pending = vec![root.root];
    let mut visited = Vec::new();
    while let Some(handle) = pending.pop() {
        if visited.contains(&handle) {
            continue;
        }
        if !computations.nodes.is_valid(handle) {
            return None;
        }
        visited.push(handle);
        let node = computations.nodes.get(handle);
        match &node.kind {
            CheckedScalarComputationKind::CaseMembership {
                subject: checked_trees::CheckedScalarComputationStructuralArgument::Place(_),
                ..
            } => {}
            CheckedScalarComputationKind::CaseMembership {
                subject: checked_trees::CheckedScalarComputationStructuralArgument::Case(subject),
                ..
            } => {
                pending.extend(
                    computations
                        .case_fields
                        .span(subject.fields)?
                        .iter()
                        .map(|field| field.value),
                );
            }
            CheckedScalarComputationKind::SelectedComparison { left, right, .. } => {
                pending.extend([*left, *right])
            }
            CheckedScalarComputationKind::Qualification { operand, .. }
            | CheckedScalarComputationKind::AuthoredCast { operand, .. }
            | CheckedScalarComputationKind::BooleanToInteger { operand, .. } => {
                pending.push(*operand)
            }
            CheckedScalarComputationKind::Dispatch { subject, arms, .. } => {
                pending.push(*subject);
                for arm in computations.dispatch_arms.span(*arms)? {
                    if let checked_trees::CheckedScalarDispatchPattern::Value(pattern) = arm.pattern
                    {
                        pending.push(pattern);
                    }
                    pending.push(arm.value);
                }
            }
            CheckedScalarComputationKind::Value(_)
            | CheckedScalarComputationKind::StructuralField { .. }
            | CheckedScalarComputationKind::ByteSequenceLiteralComparison { .. }
            | CheckedScalarComputationKind::ByteSequencePredicate { .. } => {}
            CheckedScalarComputationKind::Select {
                condition,
                when_true,
                when_false,
                ..
            } => {
                pending.extend([*condition, *when_true, *when_false]);
            }
            CheckedScalarComputationKind::Apply { operands, .. } => {
                pending.extend_from_slice(computations.operands.span(*operands)?)
            }
            CheckedScalarComputationKind::Call { .. } => visit(&mut pending, &node.kind, root)?,
        }
    }
    Some(())
}

/// The structural-argument expansion both audits run inside a `Call` node:
/// the callee selector, its scalar operands, then each structural argument —
/// a `Case` subject pushes its field values, a `Place` actual pushes its
/// computation operands before `visit` audits it.
pub(super) fn expand_call_operands(
    computations: &CheckedScalarComputationPlans,
    kind: &CheckedScalarComputationKind,
    pending: &mut Vec<CheckedScalarComputationHandle>,
    mut visit: impl FnMut(&checked_trees::CheckedUnitStructuralArgumentPlan) -> Option<()>,
) -> Option<()> {
    let CheckedScalarComputationKind::Call {
        callee,
        arguments,
        structural_arguments,
        ..
    } = kind
    else {
        return Some(());
    };
    pending.extend(callee.selector());
    pending.extend_from_slice(computations.operands.span(*arguments)?);
    for argument in computations
        .structural_arguments
        .span(*structural_arguments)?
    {
        let argument = match argument {
            checked_trees::CheckedScalarComputationStructuralArgument::Case(subject) => {
                pending.extend(
                    computations
                        .case_fields
                        .span(subject.fields)?
                        .iter()
                        .map(|field| field.value),
                );
                continue;
            }
            checked_trees::CheckedScalarComputationStructuralArgument::Place(argument) => argument,
        };
        // Selectors and range endpoints evaluate before the receiving
        // call regardless of whether this actual borrows or transfers.
        pending.extend(argument.computation_operands());
        visit(argument)?;
    }
    Some(())
}
