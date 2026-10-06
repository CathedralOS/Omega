//! One authored formal stream interleaves scalar arguments, selector snapshots
//! and structural construction. Snapshots survive later effects without becoming
//! callee arguments; constructor leaves retire at their own constructor. CPS
//! attaches this stream only to the selected expression path.
//! Anonymous argument owners retain their complete homes through the call;
//! its normal continuation retires the checked ordered residual complement.
use super::super::direct_expression_contains_short_circuit;
use super::{
    Argument, CheckedScalarComputationKind, CheckedScalarExpressionRole, CheckedTrees, Computation,
    Expansion, Handle, LoweringError, PreparedScalarQualifications, QualifiedScalarType, Site,
    literals, lower_scalar_call, parameter, produced_values, storage, structural_arguments,
    subslices, unsupported,
};
use crate::emission::operation_emission::LoweredScalarBinding;
use crate::emission::operation_emission::calls::{LoweredDirectCallBinding, ScalarCallCrashScope};
use crate::scalar_graph::scalar_graph_lowering::prepared_graph::{
    LoweredScalarBranchState, LoweredScalarBranchTerminator, LoweredScalarEffect,
};
use arena::HandleSpan;
use checked_trees::{
    CheckedScalarComputationStructuralArgument, CheckedScalarComputationTemporaryCleanup,
};

enum Operand {
    Effect(LoweredScalarEffect),
    Range {
        argument: checked_trees::CheckedUnitStructuralArgumentPlan,
        slot: subslices::Slot,
        site: checked_trees::CheckedSubsliceSite,
    },
    /// An exact byte literal established into its reserved place before the
    /// call reads it through a shared borrow.
    ByteLiteral(literals::Slot),
    Scalar(Argument),
    Selector {
        structural_ordinal: usize,
        value: Argument,
    },
    /// Evaluation, not reservation, establishes this complete owned value.
    /// Its producer is shared with local bindings and expression observations.
    Produced(produced_values::Slot),
}

/// A `Call` computation operand rejoins its retained actual through the role
/// spelling the checker emitted for the call's authored statement:
/// `CallArgument` keyed by the local binding, `UnitCallArgument` keyed by the
/// authored call occurrence, or both at once. Whichever spelling is present is
/// load-bearing custody — its binding and expression rows must rejoin uniquely
/// and agree with the computed operand — and at least one must be present.
fn consume_pure_call_argument(
    checked: &CheckedTrees,
    bindings: &storage::ScalarBindings,
    state: symbols::SymbolHandle,
    statement: u32,
    binding_ordinal: u32,
    call_ordinal: u32,
    argument_ordinal: u32,
    operand: &checked_trees::CheckedScalarExpression,
) -> Result<(), LoweringError> {
    let operand_expression = bindings.expression(operand)?;
    let plans = &checked.facts.values.scalar_expressions;
    let mut retained = false;
    for role in [
        CheckedScalarExpressionRole::CallArgument {
            binding_ordinal,
            argument_ordinal,
        },
        CheckedScalarExpressionRole::UnitCallArgument {
            call_ordinal,
            argument_ordinal,
        },
    ] {
        let present = plans.source_bindings.iter().any(|(_, binding)| {
            binding.state == state && binding.statement_ordinal == statement && binding.role == role
        }) || plans.expressions.iter().any(|expression| {
            expression.state == state
                && expression.statement_ordinal == statement
                && expression.role == role
        });
        if !present {
            continue;
        }
        if bindings.expression_at(checked, state, statement, role)? != operand_expression {
            return unsupported("computed call operand disagrees with its retained pure argument");
        }
        retained = true;
    }
    if !retained {
        return unsupported("computed call operand lost its retained pure argument");
    }
    Ok(())
}

/// The same custody on the continuation path: an operand row exists only when
/// its retained actual produced a pure scalar plan, so a computation operand
/// that is not a completed `Value` legitimately owns none.
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
        call_ordinal,
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
            *call_ordinal,
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
                call_ordinal,
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
        crate::scalar_graph::scalar_contracts::caller_erased_proof_roster(checked, machine, state)?,
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
        temporary_cleanups: HandleSpan<CheckedScalarComputationTemporaryCleanup>,
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
            || validation::exact_external_leaf_boundary_requirement(
                &self.checked.typed,
                source.target_symbol,
            )
            .map_or(source.target_symbol, |(requirement, _)| requirement)
                != target_state
        {
            return unsupported("scalar computation invocation coordinate disagrees");
        }
        let signature = crate::declarations::target_signature(
            self.checked,
            self.machine,
            source.target_symbol,
        )?;
        if signature.machine != target_machine || signature.state != target_state {
            return unsupported("computed call target differs from its retained signature");
        }
        let plans = &self.checked.facts.values.scalar_computations;
        let structural =
            plans
                .structural_arguments
                .span(structural)
                .ok_or(LoweringError::Unsupported(
                    "computed structural arguments have a stale span",
                ))?;
        let mut structural_arguments = structural_arguments::lower(
            self.checked,
            source_call,
            target_machine,
            target_state,
            site.state,
            structural,
            site.bindings,
            &self.produced_values,
            &self.subslices,
            &self.literals,
            self.structural_types,
        )?;
        let scalars = plans
            .operands
            .span(arguments)
            .ok_or(LoweringError::Unsupported(
                "scalar computation call has an invalid argument span",
            ))?;
        let parameters = super::call_signature::structural_parameters(
            self.checked,
            target_machine,
            target_state,
        )?;
        let mut scalar_ordinal = 0usize;
        let mut structural_ordinal = 0usize;
        let mut operands = Vec::new();
        for (position, source_parameter) in signature.parameters.iter().enumerate() {
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
                if let CheckedScalarComputationStructuralArgument::Place(argument) = argument
                    && matches!(argument.source, checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralExpression { .. })
                {
                    let slot = self.produced_values.iter().find(|slot| {
                        slot.source_call == source_call && slot.argument_ordinal == structural_ordinal
                    }).ok_or(LoweringError::Unsupported(
                        "computed value has no reserved argument home",
                    ))?;
                    operands.push(Operand::Produced(slot.clone()));
                }
                if let CheckedScalarComputationStructuralArgument::Place(argument) = argument
                    && let Some(range) = subslices::range(argument)
                {
                    let slot = self
                        .subslices
                        .iter()
                        .find(|slot| slot.expression == range.expression)
                        .ok_or(LoweringError::Unsupported(
                            "computed range has no reserved structural result",
                        ))?;
                    let argument_ordinal = u32::try_from(structural_ordinal).map_err(|_| {
                        LoweringError::Unsupported(
                            "computed range has no explicit structural ordinal",
                        )
                    })?;
                    operands.push(Operand::Range {
                        argument: argument.clone(),
                        slot: slot.clone(),
                        site: checked_trees::CheckedSubsliceSite::CallArgument {
                            call_ordinal,
                            argument_ordinal,
                        },
                    });
                }
                if let CheckedScalarComputationStructuralArgument::Place(argument) = argument
                    && let Some(bytes) = literals::source_bytes(argument)
                {
                    let slot = self
                        .literals
                        .iter()
                        .find(|slot| {
                            slot.source_call == source_call
                                && slot.argument_ordinal == structural_ordinal
                                && slot.bytes.as_slice() == bytes
                        })
                        .ok_or(LoweringError::Unsupported(
                            "computed literal has no reserved place",
                        ))?
                        .clone();
                    operands.push(Operand::ByteLiteral(slot));
                }
                if matches!(
                    argument,
                    CheckedScalarComputationStructuralArgument::Place(argument)
                        if !matches!(argument.source, checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralExpression { .. })
                ) && parameter.access == checked_trees::CheckedStructuralAccess::Owned
                    && parameter.multiplicity == language_semantics::Multiplicity::Unrestricted
                {
                    let slot = self
                        .copies
                        .iter()
                        .find(|slot| {
                            slot.source_call == source_call
                                && slot.argument_ordinal == structural_ordinal
                        })
                        .ok_or(LoweringError::Unsupported(
                            "computed copy has no reserved operand place",
                        ))?;
                    let actual = &mut structural_arguments[structural_ordinal];
                    // A value argument snapshots readable contents now, before
                    // evaluating the next argument. Passing its original place
                    // would defer this read until the call, after later effects.
                    operands.push(Operand::Effect(LoweredScalarEffect::CopyStructuralValue {
                        source: site.bindings.structural_copy_source(
                            actual.place,
                            &actual.path,
                            self.structural_types,
                        )?,
                        place: slot.place,
                        structural_type: slot.structural_type,
                    }));
                    actual.place = slot.place;
                    actual.path.clear();
                    actual.selector_positions.clear();
                }
                for segment in &structural_arguments[structural_ordinal].path {
                    use crate::expression_preparation::bindings::structural_paths::PreparedStructuralPathSegment;
                    let value = match segment {
                        PreparedStructuralPathSegment::RuntimeIndex(
                            checked_trees::CheckedRuntimeIndex::StructuralField(field),
                        ) => Some(Argument::Value(site.bindings.runtime_index_field(field)?)),
                        PreparedStructuralPathSegment::ComputedIndex(root) => {
                            Some(Argument::Computation(*root))
                        }
                        _ => None,
                    };
                    if let Some(value) = value {
                        operands.push(Operand::Selector {
                            structural_ordinal,
                            value,
                        });
                    }
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
        let mut scalar_actuals = Vec::new();
        for operand in &operands {
            let mut prefix = prefixes
                .last()
                .ok_or(LoweringError::Unsupported(
                    "computed call lost its input prefix",
                ))?
                .clone();
            match operand {
                Operand::Scalar(argument) => {
                    let value_type = self.argument_type(argument, site, input_types)?;
                    scalar_actuals.push(parameter(prefix.len(), value_type));
                    prefix.push(value_type);
                }
                Operand::Selector {
                    structural_ordinal,
                    value,
                } => {
                    structural_arguments[*structural_ordinal]
                        .selector_positions
                        .push(prefix.len());
                    prefix.push(self.argument_type(value, site, input_types)?);
                }
                _ => {}
            }
            prefixes.push(prefix);
        }
        let call_types = prefixes.last().ok_or(LoweringError::Unsupported(
            "computed call lost its completed prefix",
        ))?;
        let coordinate = crate::emission::operation_emission::buffer::SourceCallCoordinate {
            state: site.state,
            statement_index: source.statement_index,
            call_ordinal: source.call_ordinal,
        };
        let call = if signature.boundary {
            LoweredScalarBinding::BoundaryCall(crate::emission::operation_emission::boundary_calls::LoweredBoundaryCallBinding {
                source_coordinate: coordinate,
                source_expression: source.authored_expression,
                target_machine,
                result_type,
                result_policy: crate::emission::operation_emission::boundary_calls::result_policy::BoundaryResultPolicy::prepare(
                    self.checked, self.machine, coordinate,
                    Some(checked_trees::NominalMachineUseSite::Expression(source.authored_expression)), target_machine,
                )?,
                arguments: scalar_actuals,
                structural_arguments,
            })
        } else {
            LoweredScalarBinding::DirectCall(lower_scalar_call(
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
                scalar_actuals,
                structural_arguments,
                ScalarCallCrashScope::Arguments,
                crate::scalar_graph::scalar_contracts::caller_erased_proof_roster(
                    self.checked,
                    self.machine,
                    site.state,
                )?,
            )?)
        };
        if self.calls.contains(&coordinate) {
            return unsupported("scalar computation repeats a call occurrence");
        }
        self.calls.push(coordinate);
        // The checked call owns retirement, not the producer's source family
        // or its borrowed access. Keep the scalar result and caller prefix live
        // across the exact normal continuation; a crashing call never reaches it.
        let discards = self.temporary_cleanup_actions(
            site.state,
            source_call,
            structural,
            temporary_cleanups,
        )?;
        let target = if temporary_cleanups.is_empty() {
            target
        } else {
            let mut parameter_types = input_types.to_vec();
            parameter_types.push(result_type);
            self.push(LoweredScalarBranchState {
                structural_parameters: Vec::new(),
                structural_effects: Vec::new(),
                bindings: Vec::new(),
                erased_formal_types: Vec::new(),
                erased_proof_formals: Vec::new(),
                terminator: LoweredScalarBranchTerminator::Jump {
                    arguments: super::parameters(&parameter_types),
                    target,
                    cleanup_actions: discards,
                    erased_arguments: Vec::new(),
                    erased_proof_arguments: Vec::new(),
                    structural_arguments: Vec::new(),
                },
                parameter_types,
            })
        };
        let mut continuation = self.binding(call_types, input_types.len(), target, call);
        for (ordinal, operand) in operands.iter().enumerate().rev() {
            let prefix = &prefixes[ordinal];
            continuation = match operand {
                Operand::Scalar(argument)
                | Operand::Selector {
                    value: argument, ..
                } => self.argument(argument, prefix, continuation, site, active)?,
                Operand::ByteLiteral(slot) => self.push(LoweredScalarBranchState {
                    structural_parameters: Vec::new(),
                    parameter_types: prefix.clone(),
                    erased_formal_types: Vec::new(),
                    erased_proof_formals: Vec::new(),
                    bindings: Vec::new(),
                    structural_effects: vec![LoweredScalarEffect::EstablishByteSequenceLiteral {
                        place: slot.place,
                        structural_type: slot.structural_type,
                        bytes: slot.bytes.clone(),
                    }],
                    terminator: LoweredScalarBranchTerminator::Jump {
                        cleanup_actions: Vec::new(),
                        target: continuation,
                        arguments: super::parameters(prefix),
                        erased_arguments: Vec::new(),
                        erased_proof_arguments: Vec::new(),
                        structural_arguments: Vec::new(),
                    },
                }),
                Operand::Range {
                    argument,
                    slot,
                    site: range_site,
                } => self.subslice(
                    argument,
                    slot,
                    *range_site,
                    prefix,
                    continuation,
                    site,
                    active,
                )?,
                Operand::Effect(effect) => self.push(LoweredScalarBranchState {
                    structural_parameters: Vec::new(),
                    parameter_types: prefix.clone(),
                    erased_formal_types: Vec::new(),
                    erased_proof_formals: Vec::new(),
                    bindings: Vec::new(),
                    structural_effects: vec![effect.clone()],
                    terminator: LoweredScalarBranchTerminator::Jump {
                        cleanup_actions: Vec::new(),
                        target: continuation,
                        arguments: super::parameters(prefix),
                        erased_arguments: Vec::new(),
                        erased_proof_arguments: Vec::new(),
                        structural_arguments: Vec::new(),
                    },
                }),
                Operand::Produced(slot) => {
                    let values = &self.checked.facts.values.structural_values;
                    let prepared = crate::scalar_graph::scalar_graph_lowering::structural_values::prepare_value(
                        self.checked,
                        self.qualifications,
                        site.state,
                        facts::PlaceRoot::Expression(values.nodes.get(slot.root).expression),
                        site.statement,
                        slot.root,
                        values.calls_for_root(slot.root).unwrap_or(&[]),
                        slot.type_reference,
                        site.bindings,
                        prefix,
                        self.structural_types,
                        &slot.storage,
                    )?;
                    if prepared.place != slot.place
                        || prepared.structural_type != slot.structural_type
                    {
                        return unsupported(
                            "computed value did not establish its reserved argument",
                        );
                    }
                    prepared.finish(site.state, site.bindings, prefix, continuation, self)?
                }
            };
        }
        Ok(continuation)
    }

    /// Translate the checked partition in its authored cleanup order. Admission
    /// owns the permission and complement derivation; this step only joins its
    /// exact source to storage and lowers the already selected subtree paths.
    fn temporary_cleanup_actions(
        &self,
        state: symbols::SymbolHandle,
        source_call: Handle<checked_trees::FlowCallFact>,
        arguments: &[CheckedScalarComputationStructuralArgument],
        retained: HandleSpan<CheckedScalarComputationTemporaryCleanup>,
    ) -> Result<Vec<crate::unit::unit_cleanup::PreparedNormalCleanupAction>, LoweringError> {
        let plans = &self.checked.facts.values.scalar_computations;
        let retained =
            plans
                .temporary_cleanups
                .span(retained)
                .ok_or(LoweringError::Unsupported(
                    "computed call lost its temporary retirement roster",
                ))?;
        let mut actions = Vec::new();
        let mut retired = Vec::with_capacity(retained.len());
        for discard in retained {
            let ordinal = discard.argument_ordinal as usize;
            let argument = arguments
                .get(ordinal)
                .and_then(|argument| argument.as_place())
                .ok_or(LoweringError::Unsupported(
                    "temporary retirement lost its structural operand",
                ))?;
            if retired.contains(&ordinal)
                || !matches!(
                    argument.source,
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralExpression { .. }
                )
            {
                return unsupported("temporary retirement repeats or loses its owner source");
            }
            let slot = self
                .produced_values
                .iter()
                .find(|slot| slot.source_call == source_call && slot.argument_ordinal == ordinal)
                .ok_or(LoweringError::Unsupported(
                    "temporary retirement lost its reserved owner",
                ))?;
            retired.push(ordinal);
            let discards = plans
                .temporary_cleanup_actions
                .span(discard.actions)
                .ok_or(LoweringError::Unsupported(
                    "temporary retirement has stale residual rows",
                ))?;
            let call = self.checked.facts.flow.control.calls.get(source_call);
            let owner = self
                .checked
                .facts
                .values
                .structural_values
                .nodes
                .get(slot.root)
                .expression;
            let claims = crate::emission::claim_bindings::prepare_claim_dispositions(
                self.checked,
                self.machine,
                state,
                language_semantics::PermissionEventSource::Call {
                    statement_index: call.statement_index,
                    call_ordinal: call.call_ordinal,
                    target_symbol: call.target_symbol,
                },
                facts::PlaceRoot::Expression(owner),
                slot.type_reference,
            )?;
            let action_start = actions.len();
            for action in discards {
                use crate::unit::unit_cleanup::PreparedNormalCleanupAction;
                if action.source() != &argument.source {
                    return unsupported("temporary cleanup names a different owner");
                }
                let path = crate::expression_preparation::bindings::structural_paths::lower_typed_argument_path(
                    action.path(), slot.structural_type, self.structural_types,
                    None, &mut std::iter::empty(), None,
                )?;
                let structural_type = self
                    .structural_types
                    .iter()
                    .find(|declaration| declaration.identity == action.type_identity())
                    .map(|declaration| declaration.id)
                    .ok_or(LoweringError::Unsupported(
                        "temporary cleanup has no structural type",
                    ))?;
                actions.push(match action {
                    checked_trees::CheckedNormalCleanupAction::Discard(_) if path.is_empty() => {
                        PreparedNormalCleanupAction::discard_root(slot.place)
                    }
                    checked_trees::CheckedNormalCleanupAction::Discard(_) => {
                        PreparedNormalCleanupAction::discard_residual(
                            terminal_psi::StructuralAffineDiscard {
                                place: slot.place,
                                path,
                                structural_type,
                            },
                        )
                    }
                    checked_trees::CheckedNormalCleanupAction::InvokeNominal(cleanup) => {
                        let consumed_claims = claims
                            .iter()
                            .filter(|claim| claim.path == path)
                            .map(|claim| claim.identity)
                            .collect();
                        PreparedNormalCleanupAction::InvokeNominal {
                            cleanup: cleanup.clone(),
                            place: slot.place,
                            path,
                            structural_type,
                            consumed_claims,
                        }
                    }
                });
            }
            let selected = claims
                .iter()
                .map(|claim| claim.identity)
                .collect::<Vec<_>>();
            let emitted = actions[action_start..]
                .iter()
                .flat_map(|action| match action {
                    crate::unit::unit_cleanup::PreparedNormalCleanupAction::InvokeNominal {
                        consumed_claims,
                        ..
                    } => consumed_claims.as_slice(),
                    _ => &[],
                })
                .copied()
                .collect::<Vec<_>>();
            if selected.len() != emitted.len()
                || selected.iter().any(|identity| {
                    emitted
                        .iter()
                        .filter(|candidate| *candidate == identity)
                        .count()
                        != 1
                })
                || emitted.iter().any(|identity| !selected.contains(identity))
            {
                return unsupported(
                    "temporary cleanup did not bind each selected claim exactly once",
                );
            }
        }
        Ok(actions)
    }
}
