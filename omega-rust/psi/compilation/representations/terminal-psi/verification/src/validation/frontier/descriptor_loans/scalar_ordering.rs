//! Reconstruct scalar ordering inputs from this Terminal machine's executable
//! operations, selected successor arguments and exact scalar contracts. Contract
//! bounds remain conditional on independent call and callee proof obligations;
//! proof indices and producer-side alias tables do not supply ordering facts. Terminators have their own final position
//! even when a block contains no operations.

use semantic_vocabulary::{
    BlockId, IntegerValue, Proposition, PropositionContext, ScalarTerm, ScalarType, ValueId,
};
use std::collections::BTreeMap;
use terminal_psi::{Operation, OperationKind, TerminalMachine, TerminalModule, Terminator};
use terminal_semantics::scalar_ordering::{
    Binding, Edge, Effect, Position, ScalarOrdering, Step, instantiate_contract_ordering,
};

pub(super) fn reconstruct(module: &TerminalModule, machine: &TerminalMachine) -> ScalarOrdering {
    let steps = reconstruct_steps(machine).unwrap_or_default();
    let requirements = entry_requirements(machine);
    let returns = normal_returns(module, machine);
    ScalarOrdering::with_contracts(
        steps,
        (machine.entry, 0),
        machine
            .parameters
            .iter()
            .map(|parameter| (parameter.id, parameter.scalar_type))
            .collect(),
        &requirements,
        returns,
    )
}

// Contract shape validation precedes frontier replay, but a callee can occur
// later in module order. Reconstruct its complete scalar substitution here;
// never let an unmapped formal accidentally denote a caller's current value.
// Logical acceptance remains the ordinary call-requires/contract-ensures proof
// obligation set. These declaration facts grant no independent PCC evidence.
fn normal_returns(
    module: &TerminalModule,
    machine: &TerminalMachine,
) -> Vec<(Position, Vec<Proposition>)> {
    let mut values = BTreeMap::new();
    for declaration in machine
        .parameters
        .iter()
        .chain(machine.blocks.iter().flat_map(|block| {
            block.parameters.iter().chain(
                block
                    .operations
                    .iter()
                    .filter_map(|operation| operation.result.scalar_ref()),
            )
        }))
    {
        if values
            .insert(declaration.id, declaration.scalar_type)
            .is_some()
        {
            return Vec::new();
        }
    }
    let Ok(context) = PropositionContext::from_value_types(
        values
            .iter()
            .map(|(value, scalar_type)| (*value, *scalar_type)),
    ) else {
        return Vec::new();
    };
    machine
        .blocks
        .iter()
        .flat_map(|block| {
            block
                .operations
                .iter()
                .enumerate()
                .filter_map(|(index, operation)| {
                    let facts = call_guarantees(module, operation, &values, &context)?;
                    Some(((block.id, u32::try_from(index).ok()?), facts))
                })
        })
        .collect()
}

fn entry_requirements(machine: &TerminalMachine) -> Vec<Proposition> {
    let mut values = BTreeMap::new();
    for parameter in machine
        .parameters
        .iter()
        .chain(&machine.contract.erased_scalar_formals)
    {
        if values.insert(parameter.id, parameter.scalar_type).is_some() {
            return Vec::new();
        }
    }
    let bindings = values
        .into_iter()
        .map(|(value, scalar_type)| (value, ScalarTerm::value(value, scalar_type)))
        .collect::<Vec<_>>();
    machine
        .contract
        .requires
        .iter()
        .flat_map(|proposition| instantiate_contract_ordering(proposition, &bindings))
        .collect()
}

fn call_guarantees(
    module: &TerminalModule,
    operation: &Operation,
    values: &BTreeMap<ValueId, ScalarType>,
    caller_context: &PropositionContext,
) -> Option<Vec<Proposition>> {
    let (callee, arguments, erased_arguments, requirement_obligations) = match &operation.kind {
        OperationKind::Call {
            callee,
            arguments,
            erased_arguments,
            requirement_obligations,
            ..
        }
        | OperationKind::CallStructuralScalar {
            callee,
            arguments,
            erased_arguments,
            requirement_obligations,
            ..
        } => (
            *callee,
            arguments,
            erased_arguments,
            requirement_obligations,
        ),
        _ => return None,
    };
    let mut callees = module
        .machines
        .iter()
        .filter(|candidate| candidate.id == callee);
    let callee = callees.next()?;
    if callees.next().is_some()
        || arguments.len() != callee.parameters.len()
        || erased_arguments.len() != callee.contract.erased_scalar_formals.len()
        || requirement_obligations.len() != callee.contract.requires.len()
    {
        return None;
    }
    let result = operation.result.scalar()?;
    let callee_result = callee.result.scalar()?;
    if result.scalar_type != callee_result.scalar_type
        || values.get(&result.id) != Some(&result.scalar_type)
        || arguments.contains(&result.id)
        || erased_arguments.iter().any(|actual| {
            Proposition::Equal(actual.clone(), actual.clone())
                .any_value_id(|value| value == result.id)
        })
    {
        return None;
    }
    let mut substitutions = BTreeMap::new();
    let mut formal_types = BTreeMap::new();
    for (formal, actual) in callee.parameters.iter().zip(arguments) {
        if values.get(actual) != Some(&formal.scalar_type)
            || formal_types.insert(formal.id, formal.scalar_type).is_some()
        {
            return None;
        }
        substitutions.insert(formal.id, ScalarTerm::value(*actual, formal.scalar_type));
    }
    for (formal, actual) in callee
        .contract
        .erased_scalar_formals
        .iter()
        .zip(erased_arguments)
    {
        if actual.scalar_type() != formal.scalar_type
            || formal_types.insert(formal.id, formal.scalar_type).is_some()
            || caller_context
                .validate(&Proposition::Equal(actual.clone(), actual.clone()))
                .is_err()
        {
            return None;
        }
        substitutions.insert(formal.id, actual.clone());
    }
    if formal_types
        .insert(callee_result.id, callee_result.scalar_type)
        .is_some()
    {
        return None;
    }
    substitutions.insert(
        callee_result.id,
        ScalarTerm::value(result.id, result.scalar_type),
    );
    let bindings = substitutions.into_iter().collect::<Vec<_>>();
    Some(
        callee
            .contract
            .ensures
            .iter()
            .flat_map(|clause| instantiate_contract_ordering(&clause.proposition, &bindings))
            .collect(),
    )
}

fn reconstruct_steps(machine: &TerminalMachine) -> Option<Vec<Step>> {
    let mut positions = Vec::new();
    for block in &machine.blocks {
        let end = u32::try_from(block.operations.len()).ok()?;
        positions.extend((0..=end).map(|index| (block.id, index)));
    }
    let edge = |target: BlockId, arguments: Option<&[ValueId]>, condition| -> Option<Edge> {
        let block = machine.blocks.iter().find(|block| block.id == target)?;
        let target = positions
            .iter()
            .position(|position| *position == (target, 0))?;
        let parameters = block
            .parameters
            .iter()
            .map(|parameter| (parameter.id, parameter.scalar_type))
            .collect::<Vec<_>>();
        let bindings = if let Some(arguments) = arguments {
            if arguments.len() != parameters.len() {
                return None;
            }
            parameters
                .iter()
                .zip(arguments)
                .map(|((parameter, scalar_type), argument)| Binding {
                    parameter: *parameter,
                    argument: *argument,
                    scalar_type: *scalar_type,
                })
                .collect()
        } else {
            // Case payloads are fresh definitions, not aliases of a spelled
            // scalar. Missing bindings make the shared solver forget incoming
            // ordering when such parameters exist; no stale value survives.
            Vec::new()
        };
        Some(Edge {
            target,
            enters_block: true,
            parameters,
            bindings,
            condition,
        })
    };
    let mut steps = Vec::with_capacity(positions.len());
    for block in &machine.blocks {
        for (index, operation) in block.operations.iter().enumerate() {
            let position: Position = (block.id, u32::try_from(index).ok()?);
            let result = operation.result.scalar();
            let definitions = result
                .into_iter()
                .map(|result| (result.id, result.scalar_type))
                .collect();
            let effect = match (&operation.kind, result) {
                (
                    OperationKind::IntegerLessThan { left, right }
                    | OperationKind::IntegerLessOrEqual { left, right },
                    Some(result),
                ) => Effect::Compare {
                    result: result.id,
                    left: *left,
                    right: *right,
                    strict: matches!(operation.kind, OperationKind::IntegerLessThan { .. }),
                },
                (OperationKind::BooleanNot { operand }, Some(result)) => Effect::Not {
                    result: result.id,
                    operand: *operand,
                },
                (
                    OperationKind::IntegerConstant {
                        value: IntegerValue::Unsigned(value),
                    },
                    Some(result),
                ) => Effect::UnsignedLiteral {
                    result: result.id,
                    value: *value,
                },
                _ => Effect::None,
            };
            let target = positions
                .iter()
                .position(|next| *next == (block.id, position.1 + 1))?;
            steps.push(Step {
                position,
                definitions,
                effect,
                successors: vec![Edge {
                    target,
                    enters_block: false,
                    parameters: Vec::new(),
                    bindings: Vec::new(),
                    condition: None,
                }],
            });
        }
        let successors = match &block.terminator {
            Terminator::Jump {
                target, arguments, ..
            } => vec![edge(*target, Some(arguments), None)?],
            Terminator::Conditional {
                condition,
                when_true,
                when_false,
            } => vec![
                edge(
                    when_true.target,
                    Some(&when_true.arguments),
                    Some((*condition, true)),
                )?,
                edge(
                    when_false.target,
                    Some(&when_false.arguments),
                    Some((*condition, false)),
                )?,
            ],
            Terminator::StructuralCase { cases, .. } => cases
                .iter()
                .map(|case| edge(case.target, None, None))
                .collect::<Option<Vec<_>>>()?,
            Terminator::Return { .. }
            | Terminator::ReturnUnit { .. }
            | Terminator::ReturnUnitPartialAffine { .. }
            | Terminator::ReturnUnitNominalAffine { .. }
            | Terminator::ReturnStructural { .. }
            | Terminator::Crash { .. } => Vec::new(),
        };
        steps.push(Step {
            position: (block.id, u32::try_from(block.operations.len()).ok()?),
            definitions: Vec::new(),
            effect: Effect::None,
            successors,
        });
    }
    Some(steps)
}
