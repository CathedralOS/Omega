use crate::lowering_error::{LoweringError, unsupported};
use checked_trees::CheckedTrees;
use checked_trees::{
    CheckedScalarCaseComputationField, CheckedScalarCaseConstruction, CheckedScalarComputationKind,
    CheckedScalarComputationStructuralArgument,
};
type Computation = checked_trees::CheckedScalarComputationHandle;

pub(crate) fn call_targets(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
) -> Result<Vec<symbols::SymbolHandle>, LoweringError> {
    collect_call_targets(checked, machine, false)
}

/// Discovery only: source replay and each call's signature independently
/// validate the structural lane before any invocation is published.
pub(crate) fn structural_call_targets(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
) -> Result<Vec<symbols::SymbolHandle>, LoweringError> {
    collect_call_targets(checked, machine, true)
}

fn collect_call_targets(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    structural_only: bool,
) -> Result<Vec<symbols::SymbolHandle>, LoweringError> {
    let plans = &checked.facts.values.scalar_computations;
    let roots = plans
        .roots
        .iter()
        .filter_map(|(_, root)| (root.machine == machine).then_some(root.root))
        .collect::<Vec<_>>();
    let mut targets = Vec::new();
    for handle in reachable_nodes(checked, &roots)? {
        if let CheckedScalarComputationKind::Call {
            target_machine,
            structural_arguments,
            ..
        } = plans.nodes.get(handle).kind
        {
            let arguments = plans
                .structural_arguments
                .span(structural_arguments)
                .ok_or(LoweringError::Unsupported(
                    "scalar computation closure has invalid structural arguments",
                ))?;
            if !structural_only || !arguments.is_empty() {
                targets.push(target_machine);
            }
        }
    }
    Ok(targets)
}

/// Discovery walks only retained roots, never abandoned speculative arena nodes.
/// Source correspondence and cycle rejection remain independent checks.
pub(crate) fn reachable_nodes(
    checked: &CheckedTrees,
    roots: &[Computation],
) -> Result<Vec<Computation>, LoweringError> {
    let plans = &checked.facts.values.scalar_computations;
    let mut pending = roots.to_vec();
    let mut visited = Vec::new();
    while let Some(handle) = pending.pop() {
        if visited.contains(&handle) {
            continue;
        }
        if !plans.nodes.is_valid(handle) {
            return unsupported("scalar computation closure has an invalid node");
        }
        visited.push(handle);
        match &plans.nodes.get(handle).kind {
            CheckedScalarComputationKind::CaseMembership { subject, .. } => {
                pending.extend(
                    operand_fields(checked, subject)?
                        .iter()
                        .map(|field| field.value),
                );
            }
            CheckedScalarComputationKind::SelectedComparison { left, right, .. } => {
                pending.extend([*left, *right])
            }
            CheckedScalarComputationKind::Qualification { operand, .. }
            | CheckedScalarComputationKind::BooleanToInteger { operand, .. } => {
                pending.push(*operand);
            }
            CheckedScalarComputationKind::Dispatch { subject, arms, .. } => {
                pending.push(*subject);
                for arm in plans
                    .dispatch_arms
                    .span(*arms)
                    .ok_or(LoweringError::Unsupported(
                        "scalar dispatch closure has stale arms",
                    ))?
                {
                    if let checked_trees::CheckedScalarDispatchPattern::Value(pattern) = arm.pattern
                    {
                        pending.push(pattern);
                    }
                    pending.push(arm.value);
                }
            }
            CheckedScalarComputationKind::Value(_)
            | CheckedScalarComputationKind::StructuralField { .. } => {}
            CheckedScalarComputationKind::Select {
                condition,
                when_true,
                when_false,
                ..
            } => {
                pending.extend([*condition, *when_true, *when_false]);
            }
            CheckedScalarComputationKind::Call {
                arguments,
                structural_arguments,
                ..
            } => {
                let structural_arguments = plans
                    .structural_arguments
                    .span(*structural_arguments)
                    .ok_or(LoweringError::Unsupported(
                    "scalar computation closure has invalid structural arguments",
                ))?;
                extend_elements(plans, structural_arguments, &mut pending)?;
                for argument in structural_arguments {
                    if let checked_trees::CheckedScalarComputationStructuralArgument::Case(
                        subject,
                    ) = argument
                    {
                        pending.extend(fields(checked, subject)?.iter().map(|field| field.value));
                    }
                }
                pending.extend(plans.operands.span(*arguments).ok_or(
                    LoweringError::Unsupported("scalar computation closure has invalid arguments"),
                )?);
            }
            CheckedScalarComputationKind::Apply { operands, .. } => pending.extend(
                plans
                    .operands
                    .span(*operands)
                    .ok_or(LoweringError::Unsupported(
                        "scalar computation closure has invalid operands",
                    ))?,
            ),
        }
    }
    Ok(visited)
}
pub(crate) fn extend_elements(
    plans: &checked_trees::CheckedScalarComputationPlans,
    arguments: &[CheckedScalarComputationStructuralArgument],
    pending: &mut Vec<Computation>,
) -> Result<(), LoweringError> {
    for argument in arguments {
        if let CheckedScalarComputationStructuralArgument::Array { elements, .. } = argument {
            pending.extend(
                plans
                    .operands
                    .span(*elements)
                    .ok_or(LoweringError::Unsupported(
                        "computed array elements have a stale span",
                    ))?,
            );
        }
    }
    Ok(())
}

pub(crate) fn fields<'a>(
    checked: &'a CheckedTrees,
    subject: &CheckedScalarCaseConstruction,
) -> Result<&'a [CheckedScalarCaseComputationField], LoweringError> {
    checked
        .facts
        .values
        .scalar_computations
        .case_fields
        .span(subject.fields)
        .ok_or(LoweringError::Unsupported(
            "computed case has a stale field span",
        ))
}

pub(crate) fn operand_fields<'a>(
    checked: &'a CheckedTrees,
    subject: &CheckedScalarComputationStructuralArgument,
) -> Result<&'a [CheckedScalarCaseComputationField], LoweringError> {
    match subject {
        CheckedScalarComputationStructuralArgument::Case(subject) => fields(checked, subject),
        CheckedScalarComputationStructuralArgument::Place(_) => Ok(&[]),
        CheckedScalarComputationStructuralArgument::Array { .. } => {
            unsupported("case membership cannot observe an array")
        }
    }
}
