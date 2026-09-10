//! Expand checked expression evaluation into private typed blocks.
//!
//! Source values remain the input prefix. Completed scalar operands extend that
//! prefix; array constructors establish separate places at their authored formal
//! positions. Selection converges with one scalar result, without hoisting the
//! selected path's structural effects. The calls and arrays modules own that
//! mixed schedule and its real structural results.

use super::*;
use crate::scalar_bindings as storage;
use crate::scalar_graph_lowering::lower_scalar_call;
use arena::Handle;
use checked_trees::{CheckedScalarComputation, CheckedScalarComputationKind};

pub(crate) mod arrays;
mod calls;
mod dispatch;
mod source_custody;
mod structural_arguments;
mod value_types;
pub(super) use value_types::computation_value_type;

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
    qualifications: &'a PreparedScalarQualifications,
    machine: symbols::SymbolHandle,
    base: usize,
    states: Vec<LoweredScalarBranchState>,
    calls: Vec<SourceCallCoordinate>,
    arrays: Vec<arrays::Slot>,
}

impl<'a> Expansion<'a> {
    pub(super) fn new(
        checked: &'a CheckedTrees,
        qualifications: &'a PreparedScalarQualifications,
        machine: symbols::SymbolHandle,
        base: usize,
    ) -> Self {
        Self {
            checked,
            qualifications,
            machine,
            base,
            states: Vec::new(),
            calls: Vec::new(),
            arrays: Vec::new(),
        }
    }

    pub(super) fn finish(self) -> Vec<LoweredScalarBranchState> {
        self.states
    }

    pub(super) fn with_arrays(mut self, arrays: &[arrays::Slot]) -> Self {
        self.arrays = arrays.to_vec();
        self
    }

    /// Retain the caller prefix while completing each operand in source order.
    /// The target receives that prefix followed by the dense scalar arguments.
    /// A slice retains its original dense scalar ordinal for source custody.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn call_arguments(
        &mut self,
        state: symbols::SymbolHandle,
        coordinate: checked_trees::CheckedUnitCallCoordinate,
        boundary: bool,
        arguments: &[checked_trees::CheckedCallScalarArgument],
        argument_ordinal_start: usize,
        bindings: &storage::ScalarBindings,
        source_types: &[QualifiedScalarType],
        target: usize,
    ) -> Result<usize, LoweringError> {
        let site = Site {
            state,
            statement: coordinate.statement_index,
            bindings,
        };
        let mut operands = Vec::with_capacity(arguments.len());
        for (ordinal, argument) in arguments.iter().enumerate() {
            let ordinal =
                argument_ordinal_start
                    .checked_add(ordinal)
                    .ok_or(LoweringError::Unsupported(
                        "scalar call argument ordinal overflows",
                    ))?;
            let argument_ordinal = u32::try_from(ordinal).map_err(|_| {
                LoweringError::Unsupported("scalar call argument ordinal exceeds u32")
            })?;
            let role = if boundary {
                CheckedScalarExpressionRole::BoundaryCallArgument {
                    call_ordinal: coordinate.call_ordinal,
                    argument_ordinal,
                }
            } else {
                CheckedScalarExpressionRole::UnitCallArgument {
                    call_ordinal: coordinate.call_ordinal,
                    argument_ordinal,
                }
            };
            operands.push(match argument {
                checked_trees::CheckedCallScalarArgument::Pure(expression) => {
                    Argument::Value(bindings.expression(expression)?)
                }
                checked_trees::CheckedCallScalarArgument::Computation(root) => {
                    source_custody::validate(
                        self.checked,
                        self.machine,
                        &site,
                        role,
                        *root,
                        symbols::SymbolHandle::invalid(),
                    )?;
                    Argument::Computation(*root)
                }
            });
        }
        self.sequence(&operands, source_types, target, &site, &mut Vec::new())
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn successor(
        &mut self,
        state: symbols::SymbolHandle,
        successor: &CheckedScalarSuccessor,
        bindings: &storage::ScalarBindings,
        source_types: &[QualifiedScalarType],
        target: usize,
        target_types: &[QualifiedScalarType],
        structural_arguments: &[StructuralArgument],
    ) -> Result<Option<usize>, LoweringError> {
        let scalar_arguments = self
            .checked
            .facts
            .flow
            .terminal_scalar_graphs
            .scalar_arguments
            .span(successor.scalar_arguments)
            .ok_or(LoweringError::Unsupported(
                "scalar successor arguments have a stale span",
            ))?;
        let roles = scalar_arguments
            .iter()
            .map(|argument| argument.argument_ordinal)
            .map(|argument_ordinal| {
                if successor.is_continuation {
                    CheckedScalarExpressionRole::TransitionContinuationArgument { argument_ordinal }
                } else {
                    CheckedScalarExpressionRole::TransitionArgument { argument_ordinal }
                }
            })
            .zip(target_types)
            .map(|(role, value_type)| (role, *value_type))
            .collect::<Vec<_>>();
        self.destination(
            Site {
                state,
                statement: successor.statement_ordinal,
                bindings,
            },
            &roles,
            source_types,
            target,
            structural_arguments,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn return_value(
        &mut self,
        state: symbols::SymbolHandle,
        statement: u32,
        role: CheckedScalarExpressionRole,
        bindings: &storage::ScalarBindings,
        source_types: &[QualifiedScalarType],
        result_type: QualifiedScalarType,
        target: usize,
    ) -> Result<Option<usize>, LoweringError> {
        self.destination(
            Site {
                state,
                statement,
                bindings,
            },
            &[(role, result_type)],
            source_types,
            target,
            &[],
        )
    }

    /// Preserve the input prefix while a pure operand's selective expression
    /// completes through the ordinary scalar block emitter.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn retained_pure_value(
        &mut self,
        state: symbols::SymbolHandle,
        statement: u32,
        role: CheckedScalarExpressionRole,
        bindings: &storage::ScalarBindings,
        source_types: &[QualifiedScalarType],
        result_type: QualifiedScalarType,
        target: usize,
    ) -> Result<usize, LoweringError> {
        let expression = bindings.expression_at(self.checked, state, statement, role)?;
        if expression.value_type(source_types)? != result_type {
            return unsupported("retained scalar operand disagrees with its destination type");
        }
        self.argument(
            &Argument::Value(expression),
            source_types,
            target,
            &Site {
                state,
                statement,
                bindings,
            },
            &mut Vec::new(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn retained_value(
        &mut self,
        state: symbols::SymbolHandle,
        statement: u32,
        role: CheckedScalarExpressionRole,
        destination: symbols::SymbolHandle,
        bindings: &storage::ScalarBindings,
        source_types: &[QualifiedScalarType],
        result_type: QualifiedScalarType,
        target: usize,
    ) -> Result<usize, LoweringError> {
        let site = Site {
            state,
            statement,
            bindings,
        };
        let mut roots = self
            .checked
            .facts
            .values
            .scalar_computations
            .roots
            .iter()
            .map(|(_, root)| root)
            .filter(|root| {
                root.state == state && root.statement_ordinal == statement && root.role == role
            });
        let root = roots.next().ok_or(LoweringError::Unsupported(
            "scalar retained value has no checked computation root",
        ))?;
        if roots.next().is_some() || root.machine != self.machine {
            return unsupported("scalar computation root custody is duplicated or mismatched");
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
        source_custody::validate(
            self.checked,
            self.machine,
            &site,
            role,
            root.root,
            destination,
        )?;
        let argument = Argument::Computation(root.root);
        if self.argument_type(&argument, &site, source_types)? != result_type {
            return unsupported("scalar computation result disagrees with its destination");
        }
        // Unlike a state transfer, the following statements retain every prior
        // completed value, including immutable snapshots of overwritten storage.
        self.argument(&argument, source_types, target, &site, &mut Vec::new())
    }

    fn destination(
        &mut self,
        site: Site<'_>,
        roles: &[(CheckedScalarExpressionRole, QualifiedScalarType)],
        source_types: &[QualifiedScalarType],
        target: usize,
        structural_arguments: &[StructuralArgument],
    ) -> Result<Option<usize>, LoweringError> {
        let plans = &self.checked.facts.values.scalar_computations;
        if structural_arguments.is_empty()
            && !plans.roots.iter().any(|(_, root)| {
                root.state == site.state
                    && root.statement_ordinal == site.statement
                    && roles.iter().any(|(role, _)| *role == root.role)
            })
        {
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
        let mut arguments = Vec::with_capacity(roles.len());
        let mut argument_types = Vec::with_capacity(roles.len());
        for &(role, expected_type) in roles {
            let mut roots = plans.roots.iter().map(|(_, root)| root).filter(|root| {
                root.state == site.state
                    && root.statement_ordinal == site.statement
                    && root.role == role
            });
            let argument = if let Some(root) = roots.next() {
                if roots.next().is_some() || root.machine != self.machine {
                    return unsupported(
                        "scalar computation root custody is duplicated or mismatched",
                    );
                }
                source_custody::validate(
                    self.checked,
                    self.machine,
                    &site,
                    role,
                    root.root,
                    symbols::SymbolHandle::default(),
                )?;
                Argument::Computation(root.root)
            } else {
                Argument::Value(site.bindings.expression_at(
                    self.checked,
                    site.state,
                    site.statement,
                    role,
                )?)
            };
            let argument_type = self.argument_type(&argument, &site, source_types)?;
            if argument_type != expected_type {
                return unsupported("scalar computation result disagrees with its destination");
            }
            arguments.push(argument);
            argument_types.push(argument_type);
        }
        let mut completed_types = source_types.to_vec();
        completed_types.extend(&argument_types);
        let completion = self.push(LoweredScalarBranchState {
            structural_effects: Vec::new(),
            parameter_types: completed_types.clone(),
            bindings: Vec::new(),
            terminator: LoweredScalarBranchTerminator::Jump {
                target,
                structural_arguments: structural_arguments.to_vec(),
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

    pub(super) fn push(&mut self, state: LoweredScalarBranchState) -> usize {
        let index = self.base + self.states.len();
        self.states.push(state);
        index
    }

    fn argument_type(
        &self,
        argument: &Argument,
        site: &Site<'_>,
        source_types: &[QualifiedScalarType],
    ) -> Result<QualifiedScalarType, LoweringError> {
        match argument {
            Argument::Value(value) => value.value_type(source_types),
            Argument::Computation(handle) => computation_value_type(
                self.checked,
                self.qualifications,
                *handle,
                site.bindings,
                source_types,
            ),
        }
    }

    fn sequence(
        &mut self,
        arguments: &[Argument],
        input_types: &[QualifiedScalarType],
        target: usize,
        site: &Site<'_>,
        active: &mut Vec<Computation>,
    ) -> Result<usize, LoweringError> {
        let mut prefixes = vec![input_types.to_vec()];
        for argument in arguments {
            let mut next = prefixes.last().expect("input prefix").clone();
            next.push(self.argument_type(argument, site, input_types)?);
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
        input_types: &[QualifiedScalarType],
        retained: usize,
        target: usize,
        binding: LoweredScalarBinding,
    ) -> usize {
        let mut arguments = parameters(&input_types[..retained]);
        arguments.push(parameter(input_types.len(), binding.scalar_type().into()));
        self.push(LoweredScalarBranchState {
            structural_effects: Vec::new(),
            parameter_types: input_types.to_vec(),
            bindings: vec![binding],
            terminator: LoweredScalarBranchTerminator::Jump {
                target,
                arguments,
                structural_arguments: Vec::new(),
            },
        })
    }

    fn argument(
        &mut self,
        argument: &Argument,
        input_types: &[QualifiedScalarType],
        target: usize,
        site: &Site<'_>,
        active: &mut Vec<Computation>,
    ) -> Result<usize, LoweringError> {
        let Argument::Computation(handle) = argument else {
            let Argument::Value(expression) = argument else {
                unreachable!()
            };
            validate_direct_parameter_types(expression, &scalar_carriers(input_types))?;
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
        let result_type = self.argument_type(argument, site, input_types)?;
        active.push(*handle);
        let plans = &self.checked.facts.values.scalar_computations;
        let node = plans.nodes.get(*handle).clone();
        let entry = match node.kind {
            CheckedScalarComputationKind::Qualification { operand, .. } => {
                let operand = Argument::Computation(operand);
                let operand_type = self.argument_type(&operand, site, input_types)?;
                if operand_type == result_type {
                    self.argument(&operand, input_types, target, site, active)?
                } else {
                    let mut operand_types = input_types.to_vec();
                    operand_types.push(operand_type);
                    // The target was prepared with the full result type. Only
                    // this edge establishes it, leaving the operand unchanged.
                    let qualify = self.push(LoweredScalarBranchState {
                        parameter_types: operand_types.clone(),
                        bindings: Vec::new(),
                        structural_effects: Vec::new(),
                        terminator: LoweredScalarBranchTerminator::Qualify {
                            target,
                            arguments: parameters(&operand_types),
                            structural_arguments: Vec::new(),
                        },
                    });
                    self.argument(&operand, input_types, qualify, site, active)?
                }
            }
            CheckedScalarComputationKind::Dispatch { subject, arms, .. } => self.dispatch(
                subject,
                arms,
                result_type,
                input_types,
                target,
                site,
                active,
            )?,
            CheckedScalarComputationKind::Value(expression) => {
                let expression = site.bindings.expression(&expression)?;
                if expression.value_type(input_types)? != result_type {
                    return unsupported("scalar computation value carrier disagrees");
                }
                validate_direct_parameter_types(&expression, &scalar_carriers(input_types))?;
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
                ..
            } => {
                if self.argument_type(&Argument::Computation(condition), site, input_types)?
                    != ScalarType::Boolean.into()
                    || self.argument_type(&Argument::Computation(when_true), site, input_types)?
                        != result_type
                    || self.argument_type(&Argument::Computation(when_false), site, input_types)?
                        != result_type
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
                condition_types.push(ScalarType::Boolean.into());
                let dispatch = self.push(LoweredScalarBranchState {
                    structural_effects: Vec::new(),
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
                structural_arguments,
            } => self.call(
                source_call,
                target_machine,
                target_state,
                call_ordinal,
                arguments,
                structural_arguments,
                result_type,
                input_types,
                target,
                site,
                active,
            )?,
            CheckedScalarComputationKind::Apply {
                expression,
                operands,
                ..
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
                    operand_types.push(self.argument_type(operand, site, input_types)?);
                }
                let expression = storage::ScalarBindings::for_computation_operands(
                    input_types.len(),
                    operands.len(),
                )
                .expression(&expression)?;
                if expression.value_type(&operand_types)? != result_type {
                    return unsupported("scalar computation application carrier disagrees");
                }
                validate_direct_parameter_types(&expression, &scalar_carriers(&operand_types))?;
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

fn parameter(position: usize, value_type: QualifiedScalarType) -> LoweredDirectExpression {
    LoweredDirectExpression::Parameter {
        position,
        scalar_type: value_type.scalar_type,
    }
}

pub(super) fn parameters(types: &[QualifiedScalarType]) -> Vec<LoweredDirectExpression> {
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
    collect_call_targets(checked, machine, false)
}

/// Discovery only: source replay and each call's signature independently
/// validate the structural lane before any invocation is published.
pub(super) fn structural_call_targets(
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
            CheckedScalarComputationKind::Qualification { operand, .. } => {
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
            CheckedScalarComputationKind::Value(_) => {}
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
                arrays::extend_elements(plans, structural_arguments, &mut pending)?;
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
