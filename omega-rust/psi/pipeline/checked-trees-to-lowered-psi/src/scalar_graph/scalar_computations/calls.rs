//! One authored formal stream interleaves scalar operands and array construction.
//! An array's leaf staging is retired at its constructor, leaving earlier scalar
//! actuals intact. CPS attaches this stream only to the selected expression path.
use super::super::direct_expression_contains_short_circuit;
use super::{
    Argument, CheckedScalarComputationKind, CheckedScalarExpressionRole, CheckedTrees, Computation,
    Expansion, Handle, LoweringError, PreparedScalarQualifications, QualifiedScalarType, Site,
    arrays, cases, lower_scalar_call, parameter, source_custody, storage, structural_arguments,
    unsupported,
};
use crate::emission::operation_emission::LoweredScalarBinding;
use crate::emission::operation_emission::calls::{LoweredDirectCallBinding, ScalarCallCrashScope};
use crate::emission::operation_emission::expressions::LoweredDirectExpression;
use crate::scalar_graph::scalar_graph_lowering::prepared_graph::{
    LoweredScalarArrayConstruction, LoweredScalarBranchState, LoweredScalarBranchTerminator,
    LoweredScalarEffect,
};
use arena::HandleSpan;
use checked_trees::CheckedScalarComputationStructuralArgument;

enum Operand {
    Scalar(Argument),
    Array {
        slot: arrays::Slot,
        leaves: Vec<Argument>,
    },
    Case {
        slot: cases::Slot,
        fields: Vec<Argument>,
    },
}

/// A `LocalInitializer` call whose retained actuals all kept pure scalar plans
/// owns one `CallArgument` row per operand beside its computation root. Both
/// channels describe the same authored actual: the row must exist, rejoin
/// source custody, and agree with the computed operand value.
fn consume_pure_call_argument(
    checked: &CheckedTrees,
    bindings: &storage::ScalarBindings,
    state: symbols::SymbolHandle,
    statement: u32,
    binding_ordinal: u32,
    argument_ordinal: u32,
    operand: &checked_trees::CheckedScalarExpression,
) -> Result<(), LoweringError> {
    let retained = bindings.expression_at(
        checked,
        state,
        statement,
        CheckedScalarExpressionRole::CallArgument {
            binding_ordinal,
            argument_ordinal,
        },
    )?;
    if retained != bindings.expression(operand)? {
        return unsupported("computed call operand disagrees with its retained pure argument");
    }
    Ok(())
}

/// The same custody on the continuation path: `CallArgument` rows exist only
/// when every retained actual produced a pure scalar plan, so a single
/// computed operand means this call legitimately owns none.
pub(crate) fn consume_pure_call_arguments(
    checked: &CheckedTrees,
    bindings: &storage::ScalarBindings,
    state: symbols::SymbolHandle,
    statement: u32,
    binding_ordinal: u32,
    root: Computation,
) -> Result<(), LoweringError> {
    let plans = &checked.facts.values.scalar_computations;
    if !plans.nodes.is_valid(root) {
        return unsupported("scalar computation has no live root");
    }
    let CheckedScalarComputationKind::Call {
        arguments,
        structural_arguments,
        ..
    } = &plans.nodes.get(root).kind
    else {
        return Ok(());
    };
    let structural = plans
        .structural_arguments
        .span(*structural_arguments)
        .ok_or(LoweringError::Unsupported(
            "computed structural arguments have a stale span",
        ))?;
    if !structural.is_empty() {
        return Ok(());
    }
    let operands = plans
        .operands
        .span(*arguments)
        .ok_or(LoweringError::Unsupported(
            "scalar computation call has an invalid argument span",
        ))?;
    let mut values = Vec::with_capacity(operands.len());
    for operand in operands {
        if !plans.nodes.is_valid(*operand) {
            return unsupported("scalar computation has a stale operand");
        }
        let CheckedScalarComputationKind::Value(value) = &plans.nodes.get(*operand).kind else {
            return Ok(());
        };
        values.push(value);
    }
    for (argument_ordinal, operand) in values.into_iter().enumerate() {
        let argument_ordinal = u32::try_from(argument_ordinal)
            .map_err(|_| LoweringError::Unsupported("scalar call argument ordinal exceeds u32"))?;
        consume_pure_call_argument(
            checked,
            bindings,
            state,
            statement,
            binding_ordinal,
            argument_ordinal,
            operand,
        )?;
    }
    Ok(())
}

/// Keep a call with completed scalar expressions in its caller block. Splitting
/// these operands into forwarding blocks would replace live local results with
/// block parameters and lose their exact suspension storage provenance.
#[allow(clippy::too_many_arguments)]
pub(crate) fn lower_inline_call(
    checked: &CheckedTrees,
    qualifications: &PreparedScalarQualifications,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    statement: u32,
    role: CheckedScalarExpressionRole,
    bindings: &storage::ScalarBindings,
    input_types: &[QualifiedScalarType],
    result_type: QualifiedScalarType,
) -> Result<Option<LoweredDirectCallBinding>, LoweringError> {
    let plans = &checked.facts.values.scalar_computations;
    let root = plans
        .root_at(state, statement, role)
        .ok_or(LoweringError::Unsupported(
            "scalar binding has no unique computation root",
        ))?;
    if !plans.nodes.is_valid(root.root) {
        return unsupported("scalar computation has no live root");
    }
    let CheckedScalarComputationKind::Call {
        target_machine,
        target_state,
        call_ordinal,
        arguments,
        structural_arguments,
        ..
    } = plans.nodes.get(root.root).kind
    else {
        return Ok(None);
    };
    let structural = plans
        .structural_arguments
        .span(structural_arguments)
        .ok_or(LoweringError::Unsupported(
            "computed structural arguments have a stale span",
        ))?;
    if !structural.is_empty() {
        return Ok(None);
    }
    let operands = plans
        .operands
        .span(arguments)
        .ok_or(LoweringError::Unsupported(
            "scalar computation call has an invalid argument span",
        ))?;
    let mut arguments = Vec::with_capacity(operands.len());
    for (argument_ordinal, operand) in operands.iter().enumerate() {
        if !plans.nodes.is_valid(*operand) {
            return unsupported("scalar computation has a stale operand");
        }
        let CheckedScalarComputationKind::Value(value) = &plans.nodes.get(*operand).kind else {
            return Ok(None);
        };
        let argument = bindings.expression(value)?;
        if direct_expression_contains_short_circuit(&argument) {
            return Ok(None);
        }
        if let CheckedScalarExpressionRole::LocalInitializer { binding_ordinal } = role {
            let argument_ordinal = u32::try_from(argument_ordinal).map_err(|_| {
                LoweringError::Unsupported("scalar call argument ordinal exceeds u32")
            })?;
            consume_pure_call_argument(
                checked,
                bindings,
                state,
                statement,
                binding_ordinal,
                argument_ordinal,
                value,
            )?;
        }
        arguments.push(argument);
    }
    if root.machine != machine {
        return unsupported("scalar binding computation belongs to another machine");
    }
    if checked
        .facts
        .proof
        .proof_output_calls
        .iter()
        .any(|(_, call)| call.caller_machine_symbol == machine && call.runtime_call.is_some())
    {
        return unsupported(
            "scalar computation calls need exact named proof-output operation custody",
        );
    }
    source_custody::validate(
        checked,
        machine,
        &Site {
            state,
            statement,
            bindings,
        },
        role,
        root.root,
        symbols::SymbolHandle::invalid(),
    )?;
    lower_scalar_call(
        checked,
        qualifications,
        machine,
        state,
        statement,
        target_machine,
        target_state,
        call_ordinal,
        result_type,
        input_types,
        bindings,
        arguments,
        Vec::new(),
        ScalarCallCrashScope::CallerValues,
    )
    .map(Some)
}

impl Expansion<'_> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn call(
        &mut self,
        source_call: Handle<checked_trees::FlowCallFact>,
        target_machine: symbols::SymbolHandle,
        target_state: symbols::SymbolHandle,
        call_ordinal: u32,
        arguments: HandleSpan<Computation>,
        structural: HandleSpan<CheckedScalarComputationStructuralArgument>,
        result_type: QualifiedScalarType,
        input_types: &[QualifiedScalarType],
        target: usize,
        site: &Site<'_>,
        active: &mut Vec<Computation>,
    ) -> Result<usize, LoweringError> {
        let control = &self.checked.facts.flow.control;
        if !control.calls.is_valid(source_call) {
            return unsupported("scalar computation lost its exact checked invocation");
        }
        let source = control.calls.get(source_call);
        let state = control
            .states
            .iter()
            .map(|(_, state)| state)
            .find(|state| state.machine_symbol == self.machine && state.state_symbol == site.state)
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
        {
            return unsupported("scalar computation invocation coordinate disagrees");
        }
        let plans = &self.checked.facts.values.scalar_computations;
        let structural =
            plans
                .structural_arguments
                .span(structural)
                .ok_or(LoweringError::Unsupported(
                    "computed structural arguments have a stale span",
                ))?;
        let structural_arguments = structural_arguments::lower(
            self.checked,
            target_machine,
            target_state,
            structural,
            site.bindings,
            &self.arrays,
            &self.cases,
        )?;
        let scalars = plans
            .operands
            .span(arguments)
            .ok_or(LoweringError::Unsupported(
                "scalar computation call has an invalid argument span",
            ))?;
        let callee = crate::scalar_graph::scalar_call_closure::callee::CheckedScalarCallee::find_for_unit_call(
            self.checked,
            target_machine,
        )?;
        let (_, source_state) = crate::expression_preparation::source_custody::authored_state(
            self.checked,
            target_state,
        )?;
        let parameters = callee.structural_parameters();
        let mut scalar_ordinal = 0usize;
        let mut structural_ordinal = 0usize;
        let mut operands = Vec::new();
        for (position, source_parameter) in self
            .checked
            .state_parameters(source_state)
            .iter()
            .enumerate()
        {
            // An `[erased]` formal position has no operand of either kind
            // (contracts.md#explicit-erased-bindings); the typed relevance
            // decides that, not the operand roster being validated.
            if source_parameter.relevance.is_erased() {
                continue;
            }
            if let Some(parameter) = parameters.get(structural_ordinal)
                && parameter.position as usize == position
            {
                let argument =
                    structural
                        .get(structural_ordinal)
                        .ok_or(LoweringError::Unsupported(
                            "computed call is missing its structural actual",
                        ))?;
                if let CheckedScalarComputationStructuralArgument::Array {
                    expression,
                    elements,
                    ..
                } = argument
                {
                    let slot = self
                        .arrays
                        .iter()
                        .find(|slot| slot.expression == *expression)
                        .ok_or(LoweringError::Unsupported(
                            "computed array has no reserved structural result",
                        ))?
                        .clone();
                    let leaves = plans
                        .operands
                        .span(*elements)
                        .ok_or(LoweringError::Unsupported(
                            "computed array elements have a stale span",
                        ))?
                        .iter()
                        .copied()
                        .map(Argument::Computation)
                        .collect();
                    operands.push(Operand::Array { slot, leaves });
                }
                if let CheckedScalarComputationStructuralArgument::Case(subject) = argument {
                    let slot = self
                        .cases
                        .iter()
                        .find(|slot| slot.expression == subject.expression)
                        .ok_or(LoweringError::Unsupported(
                            "computed case has no reserved structural result",
                        ))?
                        .clone();
                    let retained = crate::expression_preparation::computation_graph::fields(
                        self.checked,
                        subject,
                    )?;
                    if retained.len() != slot.fields.len()
                        || retained
                            .iter()
                            .zip(&slot.fields)
                            .any(|(field, (symbol, _, _))| field.symbol != *symbol)
                    {
                        return unsupported("computed case field roster changed after reservation");
                    }
                    let fields = retained
                        .iter()
                        .map(|field| Argument::Computation(field.value))
                        .collect();
                    operands.push(Operand::Case { slot, fields });
                }
                structural_ordinal += 1;
            } else {
                let scalar = scalars
                    .get(scalar_ordinal)
                    .ok_or(LoweringError::Unsupported(
                        "computed call has no scalar actual at its formal position",
                    ))?;
                operands.push(Operand::Scalar(Argument::Computation(*scalar)));
                scalar_ordinal += 1;
            }
        }
        if scalar_ordinal != scalars.len() || structural_ordinal != structural.len() {
            return unsupported("computed call formal roster is incomplete");
        }
        let mut prefixes = vec![input_types.to_vec()];
        for operand in &operands {
            let mut prefix = prefixes
                .last()
                .ok_or(LoweringError::Unsupported(
                    "computed call lost its input prefix",
                ))?
                .clone();
            if let Operand::Scalar(argument) = operand {
                prefix.push(self.argument_type(argument, site, input_types)?);
            }
            prefixes.push(prefix);
        }
        let call_types = prefixes.last().ok_or(LoweringError::Unsupported(
            "computed call lost its completed prefix",
        ))?;
        let call = lower_scalar_call(
            self.checked,
            self.qualifications,
            self.machine,
            site.state,
            site.statement,
            target_machine,
            target_state,
            call_ordinal,
            result_type,
            call_types,
            site.bindings,
            parameters_for_actuals(call_types, input_types.len()),
            structural_arguments,
            ScalarCallCrashScope::Arguments,
        )?;
        if self.calls.contains(&call.source_coordinate) {
            return unsupported("scalar computation repeats a call occurrence");
        }
        self.calls.push(call.source_coordinate);
        let mut continuation = self.binding(
            call_types,
            input_types.len(),
            target,
            LoweredScalarBinding::DirectCall(call),
        );
        for (ordinal, operand) in operands.iter().enumerate().rev() {
            let prefix = &prefixes[ordinal];
            continuation = match operand {
                Operand::Scalar(argument) => {
                    self.argument(argument, prefix, continuation, site, active)?
                }
                Operand::Array { slot, leaves } => {
                    let mut leaf_types = prefix.clone();
                    for leaf in leaves {
                        leaf_types.push(self.argument_type(leaf, site, input_types)?);
                    }
                    let constructor = self.push(LoweredScalarBranchState {
                        structural_parameters: Vec::new(),
                        parameter_types: leaf_types.clone(),
                        erased_formal_types: Vec::new(),
                        bindings: Vec::new(),
                        structural_effects: vec![LoweredScalarEffect::EstablishScalarArray(
                            LoweredScalarArrayConstruction {
                                place: slot.place,
                                structural_type: slot.structural_type,
                                elements: parameters_for_actuals(&leaf_types, prefix.len()),
                            },
                        )],
                        terminator: LoweredScalarBranchTerminator::Jump {
                            trivial_affine_discards: Vec::new(),
                            target: continuation,
                            arguments: super::parameters(prefix),
                            erased_arguments: Vec::new(),
                            structural_arguments: Vec::new(),
                        },
                    });
                    self.sequence(leaves, prefix, constructor, site, active)?
                }
                Operand::Case { slot, fields } => {
                    let mut field_types = prefix.clone();
                    let mut completed_fields = Vec::new();
                    for (field, (_, identity, scalar_type)) in fields.iter().zip(&slot.fields) {
                        let qualified_type = self.argument_type(field, site, input_types)?;
                        if qualified_type != (*scalar_type).into() {
                            return unsupported(
                                "computed case field differs from its exact scalar type",
                            );
                        }
                        completed_fields
                            .push((*identity, parameter(field_types.len(), qualified_type)));
                        field_types.push(qualified_type);
                    }
                    let constructor = self.push(LoweredScalarBranchState {
                        parameter_types: field_types,
                        erased_formal_types: Vec::new(),
                        structural_parameters: Vec::new(),
                        bindings: Vec::new(),
                        structural_effects: vec![LoweredScalarEffect::EstablishScalarCase(
                            cases::Construction {
                                place: slot.place,
                                structural_type: slot.structural_type,
                                multiplicity: slot.multiplicity,
                                case: slot.case,
                                fields: completed_fields,
                            },
                        )],
                        terminator: LoweredScalarBranchTerminator::Jump {
                            trivial_affine_discards: Vec::new(),
                            target: continuation,
                            arguments: super::parameters(prefix),
                            erased_arguments: Vec::new(),
                            structural_arguments: Vec::new(),
                        },
                    });
                    self.sequence(fields, prefix, constructor, site, active)?
                }
            };
        }
        Ok(continuation)
    }
}

fn parameters_for_actuals(
    types: &[QualifiedScalarType],
    prefix: usize,
) -> Vec<LoweredDirectExpression> {
    super::parameters(types).into_iter().skip(prefix).collect()
}
