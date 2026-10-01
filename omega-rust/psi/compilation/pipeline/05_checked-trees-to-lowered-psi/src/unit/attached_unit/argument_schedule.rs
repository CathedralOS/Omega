//! Schedule scalar evaluation around nested structural producers without
//! changing checked call coordinates or inventing source-local bindings. Every
//! argument-owned construction or copy belongs to its enclosing call's exact
//! formal position. Moving a copy before an earlier scalar argument can observe
//! stale storage; moving it after a later argument can observe a later mutation.
//! Constructor leaves use the selective evaluator, not additional scheduled calls.

use super::{
    CheckedTrees, CheckedUnitEffectMachinePlan, CheckedUnitEffectOperationPlan, LoweringError,
    byte_subslices, parameters, unsupported,
};
use checked_trees::{
    CheckedArrayConstructionSource, CheckedUnitCallCoordinate,
    CheckedUnitStructuralResultBindingPlan,
};
pub(super) enum Step {
    Ordinary(usize),
    Begin,
    Argument { operation: usize, ordinal: usize },
    Subslice { operation: usize, ordinal: usize },
    Constructor(usize),
    Call(usize),
    End,
}

/// Construction kind does not determine evaluation order. Both specialized
/// scalar-array construction and general structural construction/copy retain
/// the same source owner and produced value identity.
pub(crate) fn argument_construction(
    operation: &CheckedUnitEffectOperationPlan,
) -> Option<(
    CheckedUnitCallCoordinate,
    u32,
    &CheckedUnitStructuralResultBindingPlan,
)> {
    let (source, result) = match operation {
        CheckedUnitEffectOperationPlan::EstablishScalarArray { source, result, .. }
        | CheckedUnitEffectOperationPlan::EstablishStructuralValue {
            operand_source: Some(source),
            result,
            ..
        } => (source, result),
        _ => return None,
    };
    let CheckedArrayConstructionSource::CallArgument {
        call_ordinal,
        parameter_position,
    } = source
    else {
        return None;
    };
    Some((
        CheckedUnitCallCoordinate {
            statement_index: result.statement_index,
            call_ordinal: *call_ordinal,
        },
        *parameter_position,
        result,
    ))
}

pub(super) fn build(
    checked: &CheckedTrees,
    plan: &CheckedUnitEffectMachinePlan,
) -> Result<Vec<Step>, LoweringError> {
    let operations = &plan.operations[..plan.operations.len() - 1];
    build_operations(checked, plan.machine, plan.state, operations)
}

/// Both straight-line machines and composed states retain the same authored
/// call coordinates. Their surrounding control graph does not change operand
/// order, so both routes consume this schedule over their operation sequence.
pub(super) fn build_operations(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    operations: &[CheckedUnitEffectOperationPlan],
) -> Result<Vec<Step>, LoweringError> {
    let mut steps = Vec::new();
    let mut index = 0;
    while index < operations.len() {
        // A nested boundary structural result joins its argument group below.
        if byte_subslices::contains(&operations[index])
            && !operations[index].call().is_some_and(|call| {
                call.is_boundary()
                    && call.structural_result().is_some()
                    && call.coordinate.call_ordinal != 0
            })
        {
            steps.push(Step::Begin);
            append(
                checked,
                machine,
                state,
                operations,
                index..index + 1,
                index,
                &mut Vec::new(),
                &mut Vec::new(),
                &mut steps,
            )?;
            steps.push(Step::End);
            index += 1;
            continue;
        }
        let statement = if let Some((coordinate, _, _)) = argument_construction(&operations[index])
        {
            coordinate.statement_index
        } else {
            match &operations[index] {
                // A nested call with a structural result, ordinary or boundary.
                CheckedUnitEffectOperationPlan::Call(call)
                    if call.structural_result().is_some() && call.coordinate.call_ordinal != 0 =>
                {
                    call.coordinate.statement_index
                }
                _ => {
                    steps.push(Step::Ordinary(index));
                    index += 1;
                    continue;
                }
            }
        };
        let start = index;
        let root = (start..operations.len())
            .find(|index| {
                operations[*index].call().is_some_and(|call| {
                    let coordinate = call.coordinate;
                    coordinate.statement_index == statement && coordinate.call_ordinal == 0
                })
            })
            .ok_or(LoweringError::Unsupported(
                "nested argument schedule has no enclosing call",
            ))?;
        steps.push(Step::Begin);
        let mut emitted = Vec::new();
        append(
            checked,
            machine,
            state,
            operations,
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

#[allow(clippy::too_many_arguments)]
fn append(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    operations: &[CheckedUnitEffectOperationPlan],
    group: std::ops::Range<usize>,
    index: usize,
    active: &mut Vec<usize>,
    emitted: &mut Vec<usize>,
    steps: &mut Vec<Step>,
) -> Result<(), LoweringError> {
    if active.contains(&index) || emitted.contains(&index) || !group.contains(&index) {
        return unsupported("nested argument schedule repeats or escapes its producer group");
    }
    if argument_construction(&operations[index]).is_some() {
        emitted.push(index);
        steps.push(Step::Constructor(index));
        return Ok(());
    }
    let Some(call) = operations[index].call() else {
        return unsupported("nested argument schedule requires an ordinary or boundary call");
    };
    let (coordinate, scalar_arguments, structural_arguments) = (
        call.coordinate,
        call.scalar_arguments.as_slice(),
        call.structural_arguments.as_slice(),
    );
    let authored =
        crate::emission::call_source_custody::authored::locate_source(checked, state, coordinate)?;
    let target = crate::emission::call_source_custody::authored::target_signature(
        checked,
        machine,
        authored.source_target,
    )?;
    let parameters = target.parameters;
    // An ordinary receiver occupies a structural slot even when implicit.
    // A boundary service receiver is erased: it owns requirement identity,
    // not a runtime argument. Keep authored positions separate from both
    // dense lanes rather than shifting later operands into the receiver slot.
    let implicit_self = |position: usize| {
        parameters[position].is_self
            && !authored
                .structural_arguments
                .iter()
                .any(|(formal, _)| *formal as usize == position)
    };
    let implicit_self_count = (0..parameters.len())
        .filter(|position| implicit_self(*position))
        .count();
    let structural_count = authored
        .structural_arguments
        .iter()
        .filter(|(position, _)| !call.is_boundary() || !parameters[*position as usize].is_self)
        .count()
        + if call.is_boundary() {
            0
        } else {
            implicit_self_count
        };
    if parameters.len()
        != authored.scalar_arguments.len()
            + authored.structural_arguments.len()
            + implicit_self_count
        || scalar_arguments.len() != authored.scalar_arguments.len()
        || structural_arguments.len() != structural_count
    {
        return unsupported("nested argument schedule has no exact positional signature");
    }
    active.push(index);
    let mut scalar_ordinal = 0;
    // Dense ordinal into the lowered structural argument list. Authored
    // positions and lowered slots agree except at an implicit `self`, which
    // has a slot but no authored formal.
    let mut structural_ordinal = 0;
    for (position, parameter) in parameters.iter().enumerate() {
        if call.is_boundary() && parameter.is_self {
            continue;
        }
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
        if implicit_self(position) {
            structural_ordinal += 1;
            continue;
        }
        let expression = authored
            .structural_arguments
            .iter()
            .find_map(|(formal, expression)| (*formal as usize == position).then_some(*expression))
            .ok_or(LoweringError::Unsupported(
                "nested structural argument has no authored position",
            ))?;
        let argument_ordinal = structural_ordinal;
        structural_ordinal += 1;
        let mut constructors = group.clone().filter_map(|producer| {
            let (source, formal, result) = argument_construction(&operations[producer])?;
            (source == coordinate && formal as usize == position).then_some((producer, result))
        });
        if let Some((constructor, result)) = constructors.next() {
            if constructors.next().is_some() {
                return unsupported("structural argument has duplicate retained constructors");
            }
            if !structural_arguments
                .get(argument_ordinal)
                .is_some_and(|argument| {
                    argument.source_structural_result_binding_ordinal()
                        == Some(result.binding_ordinal)
                })
            {
                return unsupported("structural argument schedule has a different result binding");
            }
            append(
                checked,
                machine,
                state,
                operations,
                group.clone(),
                constructor,
                active,
                emitted,
                steps,
            )?;
            continue;
        }
        if byte_subslices::arguments(&operations[index])
            .get(argument_ordinal)
            .is_some_and(|argument| byte_subslices::range(argument).is_some())
        {
            steps.push(Step::Subslice {
                operation: index,
                ordinal: argument_ordinal,
            });
            continue;
        }
        let Some(expression) = parameters::expression_producer(checked, expression) else {
            continue;
        };
        // A call with a structural result, ordinary or boundary, retains its site.
        let mut producers = group.clone().filter(|producer| {
            operations[*producer].call().is_some_and(|call| {
                call.structural_result().is_some()
                    && call.source_site()
                        == Some(Some(checked_trees::NominalMachineUseSite::Expression(
                            expression,
                        )))
            })
        });
        let producer = producers.next().ok_or(LoweringError::Unsupported(
            "nested structural argument has no retained producer",
        ))?;
        if producers.next().is_some() {
            return unsupported("nested structural argument has duplicate retained producers");
        }
        append(
            checked,
            machine,
            state,
            operations,
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
