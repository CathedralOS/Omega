//! Reconstruct scalar ordering inputs from this Terminal machine's executable
//! operations and selected successor arguments. Proof indices and producer-side
//! alias tables do not supply facts. Terminators have their own final position
//! even when a block contains no operations.

use semantic_vocabulary::{BlockId, IntegerValue, ValueId};
use terminal_psi::{OperationKind, TerminalMachine, Terminator};
use terminal_semantics::scalar_ordering::{Binding, Edge, Effect, Position, ScalarOrdering, Step};

pub(super) fn reconstruct(machine: &TerminalMachine) -> ScalarOrdering {
    let steps = reconstruct_steps(machine).unwrap_or_default();
    ScalarOrdering::new(
        steps,
        (machine.entry, 0),
        machine
            .parameters
            .iter()
            .map(|parameter| (parameter.id, parameter.scalar_type))
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
