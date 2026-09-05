//! Expand checked expression evaluation into private typed scalar blocks.
//!
//! Source values remain the input prefix. Every completed operand is appended
//! before the next operand starts; selection converges with one result value.

use super::*;
use arena::Handle;
use checked_trees::{CheckedScalarComputation, CheckedScalarComputationKind};

type Computation = Handle<CheckedScalarComputation>;

enum Argument {
    Value(LoweredDirectExpression),
    Computation(Computation),
}

struct Site<'a> {
    state: symbols::SymbolHandle,
    statement: u32,
    bindings: &'a storage::ScalarBindings,
}

pub(super) struct Expansion<'a> {
    checked: &'a CheckedTrees,
    machine: symbols::SymbolHandle,
    base: usize,
    states: Vec<LoweredScalarBranchState>,
    calls: Vec<SourceCallCoordinate>,
}

impl<'a> Expansion<'a> {
    pub(super) fn new(
        checked: &'a CheckedTrees,
        machine: symbols::SymbolHandle,
        base: usize,
    ) -> Self {
        Self {
            checked,
            machine,
            base,
            states: Vec::new(),
            calls: Vec::new(),
        }
    }

    pub(super) fn finish(self) -> Vec<LoweredScalarBranchState> {
        self.states
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn successor(
        &mut self,
        state: symbols::SymbolHandle,
        successor: &CheckedScalarSuccessor,
        bindings: &storage::ScalarBindings,
        source_types: &[ScalarType],
        target: usize,
        target_types: &[PrimitiveType],
    ) -> Result<Option<usize>, LoweringError> {
        let plans = &self.checked.facts.values.scalar_computations;
        let roles = (0..successor.argument_count)
            .map(|argument_ordinal| {
                if successor.is_continuation {
                    CheckedScalarExpressionRole::TransitionContinuationArgument { argument_ordinal }
                } else {
                    CheckedScalarExpressionRole::TransitionArgument { argument_ordinal }
                }
            })
            .collect::<Vec<_>>();
        if !plans.roots.iter().any(|(_, root)| {
            root.state == state
                && root.statement_ordinal == successor.statement_ordinal
                && roles.contains(&root.role)
        }) {
            return Ok(None);
        }
        if self
            .checked
            .facts
            .proof
            .proof_output_calls
            .iter()
            .any(|(_, call)| {
                call.caller_machine_symbol == self.machine && call.runtime_call.is_some()
            })
        {
            return unsupported(
                "scalar computation calls need exact named proof-output operation custody",
            );
        }
        let site = Site {
            state,
            statement: successor.statement_ordinal,
            bindings,
        };
        let mut arguments = Vec::with_capacity(roles.len());
        let mut argument_types = Vec::with_capacity(roles.len());
        for (role, primitive_type) in roles.into_iter().zip(target_types) {
            let mut roots = plans.roots.iter().map(|(_, root)| root).filter(|root| {
                root.state == state
                    && root.statement_ordinal == successor.statement_ordinal
                    && root.role == role
            });
            let argument = if let Some(root) = roots.next() {
                if roots.next().is_some() || root.machine != self.machine {
                    return unsupported(
                        "scalar computation root custody is duplicated or mismatched",
                    );
                }
                Argument::Computation(root.root)
            } else {
                Argument::Value(bindings.expression_at(
                    self.checked,
                    state,
                    site.statement,
                    role,
                )?)
            };
            let argument_type = self.argument_type(&argument)?;
            if argument_type != terminal_scalar_type(*primitive_type)? {
                return unsupported("scalar computation result disagrees with its destination");
            }
            arguments.push(argument);
            argument_types.push(argument_type);
        }
        let mut completed_types = source_types.to_vec();
        completed_types.extend(&argument_types);
        let completion = self.push(LoweredScalarBranchState {
            parameter_types: completed_types.clone(),
            bindings: Vec::new(),
            terminator: LoweredScalarBranchTerminator::Jump {
                target,
                arguments: parameters(&completed_types)
                    .into_iter()
                    .skip(source_types.len())
                    .collect(),
            },
        });
        Ok(Some(self.sequence(
            &arguments,
            source_types,
            completion,
            &site,
            &mut Vec::new(),
        )?))
    }

    fn push(&mut self, state: LoweredScalarBranchState) -> usize {
        let index = self.base + self.states.len();
        self.states.push(state);
        index
    }

    fn argument_type(&self, argument: &Argument) -> Result<ScalarType, LoweringError> {
        match argument {
            Argument::Value(value) => Ok(value.scalar_type()),
            Argument::Computation(handle) => {
                let nodes = &self.checked.facts.values.scalar_computations.nodes;
                if !nodes.is_valid(*handle) {
                    return unsupported("scalar computation has an invalid node handle");
                }
                terminal_scalar_type(nodes.get(*handle).primitive_type)
            }
        }
    }

    fn sequence(
        &mut self,
        arguments: &[Argument],
        input_types: &[ScalarType],
        target: usize,
        site: &Site<'_>,
        active: &mut Vec<Computation>,
    ) -> Result<usize, LoweringError> {
        let mut prefixes = vec![input_types.to_vec()];
        for argument in arguments {
            let mut next = prefixes.last().expect("input prefix").clone();
            next.push(self.argument_type(argument)?);
            prefixes.push(next);
        }
        let mut continuation = target;
        for (index, argument) in arguments.iter().enumerate().rev() {
            continuation = self.argument(argument, &prefixes[index], continuation, site, active)?;
        }
        Ok(continuation)
    }

    fn binding(
        &mut self,
        input_types: &[ScalarType],
        retained: usize,
        target: usize,
        binding: LoweredScalarBinding,
    ) -> usize {
        let mut arguments = parameters(&input_types[..retained]);
        arguments.push(parameter(input_types.len(), binding.scalar_type()));
        self.push(LoweredScalarBranchState {
            parameter_types: input_types.to_vec(),
            bindings: vec![binding],
            terminator: LoweredScalarBranchTerminator::Jump { target, arguments },
        })
    }

    fn argument(
        &mut self,
        argument: &Argument,
        input_types: &[ScalarType],
        target: usize,
        site: &Site<'_>,
        active: &mut Vec<Computation>,
    ) -> Result<usize, LoweringError> {
        let Argument::Computation(handle) = argument else {
            let Argument::Value(expression) = argument else {
                unreachable!()
            };
            validate_direct_parameter_types(expression, input_types)?;
            return Ok(self.binding(
                input_types,
                input_types.len(),
                target,
                LoweredScalarBinding::Expression(expression.clone()),
            ));
        };
        if active.contains(handle) {
            return unsupported("scalar computation contains a cycle");
        }
        let result_type = self.argument_type(argument)?;
        active.push(*handle);
        let plans = &self.checked.facts.values.scalar_computations;
        let node = plans.nodes.get(*handle).clone();
        let entry = match node.kind {
            CheckedScalarComputationKind::Value(expression) => {
                let expression = site.bindings.expression(&expression)?;
                if expression.scalar_type() != result_type {
                    return unsupported("scalar computation value carrier disagrees");
                }
                validate_direct_parameter_types(&expression, input_types)?;
                self.binding(
                    input_types,
                    input_types.len(),
                    target,
                    LoweredScalarBinding::Expression(expression),
                )
            }
            CheckedScalarComputationKind::Select {
                condition,
                when_true,
                when_false,
            } => {
                if self.argument_type(&Argument::Computation(condition))? != ScalarType::Boolean
                    || self.argument_type(&Argument::Computation(when_true))? != result_type
                    || self.argument_type(&Argument::Computation(when_false))? != result_type
                {
                    return unsupported("scalar computation selection carriers disagree");
                }
                let when_true_target = self.argument(
                    &Argument::Computation(when_true),
                    input_types,
                    target,
                    site,
                    active,
                )?;
                let when_false_target = self.argument(
                    &Argument::Computation(when_false),
                    input_types,
                    target,
                    site,
                    active,
                )?;
                let mut condition_types = input_types.to_vec();
                condition_types.push(ScalarType::Boolean);
                let dispatch = self.push(LoweredScalarBranchState {
                    parameter_types: condition_types,
                    bindings: Vec::new(),
                    terminator: LoweredScalarBranchTerminator::Conditional {
                        condition: LoweredBooleanReturnExpression::Parameter {
                            position: input_types.len(),
                        },
                        when_true_target,
                        when_true_arguments: parameters(input_types),
                        when_false_target,
                        when_false_arguments: parameters(input_types),
                    },
                });
                self.argument(
                    &Argument::Computation(condition),
                    input_types,
                    dispatch,
                    site,
                    active,
                )?
            }
            CheckedScalarComputationKind::Call {
                source_call,
                target_machine,
                target_state,
                call_ordinal,
                arguments,
            } => {
                let control = &self.checked.facts.flow.control;
                if !control.calls.is_valid(source_call) {
                    return unsupported("scalar computation lost its exact checked invocation");
                }
                let source = control.calls.get(source_call);
                let state = control
                    .states
                    .iter()
                    .map(|(_, state)| state)
                    .find(|state| {
                        state.machine_symbol == self.machine && state.state_symbol == site.state
                    })
                    .ok_or(LoweringError::Unsupported(
                        "scalar computation invocation state is absent",
                    ))?;
                if !control
                    .calls
                    .span_or_empty(state.calls)
                    .iter()
                    .any(|call| std::ptr::eq(call, source))
                    || source.statement_index != site.statement as usize
                    || source.call_ordinal != call_ordinal as usize
                    || source.target_symbol != target_state
                    || source.has_receiver
                {
                    return unsupported("scalar computation invocation coordinate disagrees");
                }
                let arguments = plans
                    .operands
                    .span(arguments)
                    .ok_or(LoweringError::Unsupported(
                        "scalar computation call has an invalid argument span",
                    ))?
                    .iter()
                    .copied()
                    .map(Argument::Computation)
                    .collect::<Vec<_>>();
                let mut call_types = input_types.to_vec();
                for argument in &arguments {
                    call_types.push(self.argument_type(argument)?);
                }
                let call = lower_scalar_call(
                    self.checked,
                    self.machine,
                    site.state,
                    site.statement,
                    target_machine,
                    target_state,
                    call_ordinal,
                    result_type,
                    &call_types,
                    parameters(&call_types)
                        .into_iter()
                        .skip(input_types.len())
                        .collect(),
                    ScalarCallCrashScope::Arguments,
                )?;
                if self.calls.contains(&call.source_coordinate) {
                    return unsupported("scalar computation repeats a call occurrence");
                }
                self.calls.push(call.source_coordinate);
                let invoke = self.binding(
                    &call_types,
                    input_types.len(),
                    target,
                    LoweredScalarBinding::DirectCall(call),
                );
                self.sequence(&arguments, input_types, invoke, site, active)?
            }
            CheckedScalarComputationKind::Apply {
                expression,
                operands,
            } => {
                let operands = plans
                    .operands
                    .span(operands)
                    .ok_or(LoweringError::Unsupported(
                        "scalar computation application has an invalid operand span",
                    ))?
                    .iter()
                    .copied()
                    .map(Argument::Computation)
                    .collect::<Vec<_>>();
                let mut operand_types = input_types.to_vec();
                for operand in &operands {
                    operand_types.push(self.argument_type(operand)?);
                }
                let expression = storage::ScalarBindings::for_computation_operands(
                    input_types.len(),
                    operands.len(),
                )
                .expression(&expression)?;
                if expression.scalar_type() != result_type {
                    return unsupported("scalar computation application carrier disagrees");
                }
                validate_direct_parameter_types(&expression, &operand_types)?;
                let apply = self.binding(
                    &operand_types,
                    input_types.len(),
                    target,
                    LoweredScalarBinding::Expression(expression),
                );
                self.sequence(&operands, input_types, apply, site, active)?
            }
        };
        active.pop();
        Ok(entry)
    }
}

fn parameter(position: usize, scalar_type: ScalarType) -> LoweredDirectExpression {
    LoweredDirectExpression::Parameter {
        position,
        scalar_type,
    }
}

pub(super) fn parameters(types: &[ScalarType]) -> Vec<LoweredDirectExpression> {
    types
        .iter()
        .copied()
        .enumerate()
        .map(|(position, scalar_type)| parameter(position, scalar_type))
        .collect()
}

pub(super) fn call_targets(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
) -> Result<Vec<symbols::SymbolHandle>, LoweringError> {
    let plans = &checked.facts.values.scalar_computations;
    let mut pending = plans
        .roots
        .iter()
        .filter_map(|(_, root)| (root.machine == machine).then_some(root.root))
        .collect::<Vec<_>>();
    let mut visited = Vec::new();
    let mut targets = Vec::new();
    while let Some(handle) = pending.pop() {
        if visited.contains(&handle) {
            continue;
        }
        if !plans.nodes.is_valid(handle) {
            return unsupported("scalar computation closure has an invalid node");
        }
        visited.push(handle);
        match &plans.nodes.get(handle).kind {
            CheckedScalarComputationKind::Value(_) => {}
            CheckedScalarComputationKind::Select {
                condition,
                when_true,
                when_false,
            } => pending.extend([*condition, *when_true, *when_false]),
            CheckedScalarComputationKind::Call {
                target_machine,
                arguments,
                ..
            } => {
                targets.push(*target_machine);
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
    Ok(targets)
}
