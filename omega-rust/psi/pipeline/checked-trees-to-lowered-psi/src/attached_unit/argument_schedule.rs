//! Schedule scalar evaluation around nested structural producers without
//! changing checked call coordinates or inventing source-local bindings.

use super::*;

pub(super) enum Step {
    Ordinary(usize),
    Begin,
    Argument { operation: usize, ordinal: usize },
    Call(usize),
    End,
}

pub(super) fn build(
    checked: &CheckedTrees,
    plan: &CheckedUnitEffectMachinePlan,
) -> Result<Vec<Step>, LoweringError> {
    let operations = &plan.operations[..plan.operations.len() - 1];
    let mut steps = Vec::new();
    let mut index = 0;
    while index < operations.len() {
        let (CheckedUnitEffectOperationPlan::StructuralCall { coordinate, .. }
        | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { coordinate, .. }) =
            &operations[index]
        else {
            steps.push(Step::Ordinary(index));
            index += 1;
            continue;
        };
        if coordinate.call_ordinal == 0 {
            steps.push(Step::Ordinary(index));
            index += 1;
            continue;
        }
        let start = index;
        let statement = coordinate.statement_index;
        let root = (start..operations.len())
            .find(|index| match &operations[*index] {
                CheckedUnitEffectOperationPlan::StructuralCall { coordinate, .. }
                | CheckedUnitEffectOperationPlan::CallUnit { coordinate, .. }
                | CheckedUnitEffectOperationPlan::ScalarCall { coordinate, .. }
                | CheckedUnitEffectOperationPlan::BoundaryCall { coordinate, .. }
                | CheckedUnitEffectOperationPlan::BoundaryScalarCall { coordinate, .. }
                | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { coordinate, .. } => {
                    coordinate.statement_index == statement && coordinate.call_ordinal == 0
                }
                _ => false,
            })
            .ok_or(LoweringError::Unsupported(
                "nested argument schedule has no enclosing call",
            ))?;
        steps.push(Step::Begin);
        let mut emitted = Vec::new();
        append(
            checked,
            plan,
            start..root + 1,
            root,
            &mut Vec::new(),
            &mut emitted,
            &mut steps,
        )?;
        if !emitted.iter().copied().eq(start..root + 1) {
            return unsupported("argument schedule disagrees with checked structural call order");
        }
        steps.push(Step::End);
        index = root + 1;
    }
    Ok(steps)
}

fn append(
    checked: &CheckedTrees,
    plan: &CheckedUnitEffectMachinePlan,
    group: std::ops::Range<usize>,
    index: usize,
    active: &mut Vec<usize>,
    emitted: &mut Vec<usize>,
    steps: &mut Vec<Step>,
) -> Result<(), LoweringError> {
    if active.contains(&index) || emitted.contains(&index) || !group.contains(&index) {
        return unsupported("nested argument schedule repeats or escapes its producer group");
    }
    let (coordinate, scalar_arguments) = match &plan.operations[index] {
        CheckedUnitEffectOperationPlan::StructuralCall {
            coordinate,
            scalar_arguments,
            ..
        }
        | CheckedUnitEffectOperationPlan::CallUnit {
            coordinate,
            scalar_arguments,
            ..
        }
        | CheckedUnitEffectOperationPlan::ScalarCall {
            coordinate,
            scalar_arguments,
            ..
        }
        | CheckedUnitEffectOperationPlan::BoundaryCall {
            coordinate,
            scalar_arguments,
            ..
        }
        | CheckedUnitEffectOperationPlan::BoundaryScalarCall {
            coordinate,
            scalar_arguments,
            ..
        }
        | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
            coordinate,
            scalar_arguments,
            ..
        } => (*coordinate, scalar_arguments),
        _ => return unsupported("nested argument schedule requires an ordinary or boundary call"),
    };
    let authored =
        crate::call_source_custody::authored::locate_source(checked, plan.state, coordinate)?;
    let target = crate::call_source_custody::authored::target_signature(
        checked,
        plan.machine,
        authored.source_target,
    )?;
    let parameters = target.parameters;
    if parameters.iter().any(|parameter| parameter.is_self)
        || parameters.len() != authored.scalar_arguments.len() + authored.structural_arguments.len()
        || scalar_arguments.len() != authored.scalar_arguments.len()
    {
        return unsupported("nested argument schedule has no exact positional signature");
    }
    active.push(index);
    let mut scalar_ordinal = 0;
    for (position, parameter) in parameters.iter().enumerate() {
        if checked
            .primitive_type_reference(parameter.type_reference)
            .is_some()
        {
            steps.push(Step::Argument {
                operation: index,
                ordinal: scalar_ordinal,
            });
            scalar_ordinal += 1;
            continue;
        }
        let expression = authored
            .structural_arguments
            .iter()
            .find_map(|(formal, expression)| (*formal as usize == position).then_some(*expression))
            .ok_or(LoweringError::Unsupported(
                "nested structural argument has no authored position",
            ))?;
        let Some(expression) = parameters::expression_producer(checked, expression) else {
            continue;
        };
        let mut producers = group.clone().filter(|producer| matches!(
            &plan.operations[*producer],
            CheckedUnitEffectOperationPlan::StructuralCall { source_site, .. }
                | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { source_site, .. }
                if *source_site == Some(checked_trees::NominalMachineUseSite::Expression(expression))
        ));
        let producer = producers.next().ok_or(LoweringError::Unsupported(
            "nested structural argument has no retained producer",
        ))?;
        if producers.next().is_some() {
            return unsupported("nested structural argument has duplicate retained producers");
        }
        append(
            checked,
            plan,
            group.clone(),
            producer,
            active,
            emitted,
            steps,
        )?;
    }
    active.pop();
    emitted.push(index);
    steps.push(Step::Call(index));
    Ok(())
}
