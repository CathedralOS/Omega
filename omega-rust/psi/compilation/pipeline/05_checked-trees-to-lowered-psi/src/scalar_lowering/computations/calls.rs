//! One authored formal stream interleaves scalar arguments, selector snapshots
//! and structural construction. Snapshots survive later effects without becoming
//! callee arguments; constructor leaves retire at their own constructor. CPS
//! attaches this stream only to the selected expression path.
//! Anonymous argument owners retain their complete homes through the call;
//! its normal continuation retires the checked ordered residual complement.
use super::{
    Argument, CheckedScalarComputationKind, CheckedScalarExpressionRole, CheckedTrees, Computation,
    EstablishedValue, Expansion, Handle, LoweringError, QualifiedScalarType, Site, literals,
    lower_scalar_call, parameter, produced_values, storage, structural_arguments, subslices,
    unsupported,
};
use crate::emission::operation_emission::LoweredScalarBinding;
use crate::scalar_lowering::branch_states::prepared::{
    LoweredScalarBranchState, LoweredScalarBranchTerminator, LoweredScalarEffect,
};
use arena::HandleSpan;
use checked_trees::{
    CheckedScalarComputationStructuralArgument, CheckedScalarComputationTemporaryCleanup,
};

enum Operand {
    Copy {
        effect: LoweredScalarEffect,
        value: EstablishedValue,
    },
    Range {
        argument: checked_trees::CheckedUnitStructuralArgumentPlan,
        slot: subslices::Slot,
    },
    /// Exact byte materialization at the operand, before subsequent arguments.
    ByteLiteral(literals::Slot),
    Scalar(Argument),
    /// A copy projection's saved coordinate, never a callee argument.
    Snapshot(Argument),
    /// A selector call's dispatch operand: evaluated first, then named by the
    /// call as its selector rather than passed as an argument.
    DispatchSelector(Argument),
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
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    bindings: &storage::ScalarBindings,
    state: symbols::SymbolHandle,
    statement: u32,
    binding_ordinal: u32,
    call_ordinal: u32,
    argument_ordinal: u32,
    operand: &checked_trees::CheckedScalarExpression,
) -> Result<(), LoweringError> {
    let operand_expression =
        bindings.expression(identities, &checked.facts.proven_scalar_indexes, operand)?;
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
        if bindings.expression_at(identities, checked, state, statement, role)?
            != operand_expression
        {
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
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
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
        callee: checked_trees::CheckedScalarCallee::Body { .. },
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
            identities,
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

impl Expansion<'_> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn call(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        static_requirement_dispatch: Option<checked_trees::StaticRequirementDispatchFact>,
        authored_expression: typed_trees::expression::ExpressionHandle,
        source_call: Handle<checked_trees::FlowCallFact>,
        callee: &checked_trees::CheckedScalarCallee,
        call_ordinal: u32,
        arguments: HandleSpan<Computation>,
        structural: HandleSpan<CheckedScalarComputationStructuralArgument>,
        temporary_cleanups: HandleSpan<CheckedScalarComputationTemporaryCleanup>,
        claim_transfers: HandleSpan<checked_trees::CheckedUnitClaimTransferPlan>,
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
        let expressions = &self.checked.typed.expression_table;
        let source_dispatch = expressions
            .expression_is_valid(source.authored_expression)
            .then(|| expressions.call_node(source.authored_expression))
            .flatten()
            .and_then(|call| call.static_requirement_dispatch.as_ref());
        match (static_requirement_dispatch, source_dispatch) {
            (None, None) => {}
            (Some(fact), Some(dispatch))
                if source.authored_expression == authored_expression
                    && fact.application_commitment == dispatch.application_commitment
                    && fact.application_report_fingerprint
                        == dispatch.application_report_fingerprint
                    && fact.declaring_trait == dispatch.declaring_trait
                    && fact.requirement == dispatch.requirement
                    && fact.contract_signature == dispatch.contract_signature
                    && fact.realization_machine == dispatch.realization_machine
                    && fact.realization_state == dispatch.realization_state
                    && callee
                        .static_target()
                        .is_some_and(|(_, state)| state == source.target_symbol)
                    && matches!(callee, checked_trees::CheckedScalarCallee::Body { .. })
                    && validation::closed_requirement_dispatch_is_valid(
                        &self.checked.typed,
                        self.machine,
                        source.target_symbol,
                        dispatch,
                    ) => {}
            _ => {
                return unsupported(
                    "scalar computation lost its exact closed requirement selection",
                );
            }
        }

        // A static callee names its machine and entry; a selector call names
        // its instantiated slot for both coordinates.
        let (target_machine, target_state) = callee
            .static_target()
            .unwrap_or((callee.called_signature(), callee.called_signature()));
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
                != callee.called_signature()
        {
            return unsupported("scalar computation invocation coordinate disagrees");
        }
        let signature = crate::declarations::target_signature(
            self.checked,
            self.machine,
            source.target_symbol,
        )?;
        let plans = &self.checked.facts.values.scalar_computations;
        let structural =
            plans
                .structural_arguments
                .span(structural)
                .ok_or(LoweringError::Unsupported(
                    "computed structural arguments have a stale span",
                ))?;
        let mut structural_arguments = structural_arguments::lower(
            identities,
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
        let claim_transfers = plans
            .claim_transfers
            .span(claim_transfers)
            .ok_or(LoweringError::Unsupported(
                "computed call claim transfers have a stale span",
            ))?
            .to_vec();
        let mut scalar_ordinal = 0usize;
        let mut structural_ordinal = 0usize;
        let mut operands = Vec::new();
        if callee.authority_value().is_valid() {
            let slot = self
                .produced_values
                .iter()
                .find(|slot| slot.source_call == source_call && slot.argument_ordinal.is_none())
                .ok_or(LoweringError::Unsupported(
                    "computed authority has no reserved home",
                ))?;
            operands.push(Operand::Produced(slot.clone()));
        }
        for (position, source_parameter) in signature.parameters.iter().enumerate() {
            // An `[erased]` formal position has no operand of either kind
            // (contracts.md#explicit-erased-bindings); the typed relevance
            // decides that, not the operand roster being validated.
            if source_parameter.relevance.is_erased() {
                continue;
            }
            // A slot's operand 0 is the selector, kept apart from the data.
            if position == 0
                && let Some(selector) = callee.selector()
            {
                operands.push(Operand::DispatchSelector(Argument::Computation(selector)));
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
                        slot.source_call == source_call && slot.argument_ordinal == Some(structural_ordinal)
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
                    operands.push(Operand::Range {
                        argument: argument.clone(),
                        slot: slot.clone(),
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
                                && slot.literal.bytes.as_slice() == bytes
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
                        if !matches!(argument.source, checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralExpression { .. } | checked_trees::CheckedUnitStructuralArgumentSourcePlan::ByteSequenceLiteral { .. })
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
                    let expression = self
                        .checked
                        .facts
                        .flow
                        .control
                        .call_arguments
                        .span(source.authored_arguments)
                        .and_then(|arguments| arguments.get(position))
                        .copied()
                        .ok_or(LoweringError::Unsupported(
                            "computed copy lost its authored operand",
                        ))?;
                    let value = EstablishedValue::Copied {
                        expression,
                        place: slot.place,
                        structural_type: slot.structural_type,
                        source: actual.clone(),
                    };
                    // Selectors are ordinary operands before the snapshot. They
                    // retain their values through later selectors and argument calls.
                    let mut position = input_types.len()
                        + operands
                            .iter()
                            .filter(|operand| {
                                matches!(
                                    operand,
                                    Operand::Scalar(_)
                                        | Operand::DispatchSelector(_)
                                        | Operand::Selector { .. }
                                        | Operand::Snapshot(_)
                                )
                            })
                            .count();
                    let source = site.bindings.structural_copy_source(
                        identities,
                        actual.place,
                        &actual.path,
                        self.structural_types,
                        |identities, selector| {
                            use crate::expression_preparation::bindings::StructuralCopySelector;
                            let argument = match selector {
                                StructuralCopySelector::Computation(root) => {
                                    Argument::Computation(root)
                                }
                                StructuralCopySelector::Value(value) => Argument::Value(value),
                            };
                            let value_type =
                                self.argument_type(identities, &argument, site, input_types)?;
                            operands.push(Operand::Snapshot(argument));
                            let saved = super::parameter(position, value_type);
                            position += 1;
                            Ok(saved)
                        },
                    )?;
                    operands.push(Operand::Copy {
                        effect: LoweredScalarEffect::CopyStructuralValue {
                            source,
                            place: slot.place,
                            structural_type: slot.structural_type,
                        },
                        value,
                    });
                    actual.place = slot.place;
                    actual.path.clear();
                    actual.selector_positions.clear();
                }
                for segment in &structural_arguments[structural_ordinal].path {
                    use crate::expression_preparation::bindings::structural_paths::PreparedStructuralPathSegment;
                    let value = match segment {
                        PreparedStructuralPathSegment::RuntimeIndex(
                            checked_trees::CheckedRuntimeIndex::StorageRead { symbol },
                        ) => Some(Argument::Value(
                            site.bindings.runtime_index_storage(*symbol)?,
                        )),
                        PreparedStructuralPathSegment::RuntimeIndex(
                            checked_trees::CheckedRuntimeIndex::StructuralField(field),
                        ) => Some(Argument::Value(
                            site.bindings.runtime_index_field(identities, field)?,
                        )),
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
        // A direct callee's scalar formals as its body receives them. A formal
        // may carry a predicate-free, route-free tag the actual lacks: such a
        // tag "adds no membership obligation" (domains.md, declaration and
        // membership), so checking admitted the call, and the actual arrives
        // through a qualification edge exactly as an explicit `as` would.
        let arrival_types = match callee.static_target() {
            Some(_) if !signature.boundary => Some(
                self.qualifications
                    .scalar_state_types(self.checked, target_state)?
                    .0,
            ),
            _ => None,
        };
        let mut prefixes = vec![input_types.to_vec()];
        let mut scalar_actuals = Vec::new();
        let mut arrivals = Vec::with_capacity(operands.len());
        let mut selector_actual = None;
        for operand in &operands {
            let mut arrival = None;
            let mut prefix = prefixes
                .last()
                .ok_or(LoweringError::Unsupported(
                    "computed call lost its input prefix",
                ))?
                .clone();
            match operand {
                Operand::Scalar(argument) => {
                    let value_type = self.argument_type(identities, argument, site, input_types)?;
                    // Only membership may change on arrival; a carrier
                    // disagreement stays for the call binding to refuse.
                    let arrival_type = arrival_types
                        .as_ref()
                        .and_then(|types| types.get(scalar_actuals.len()))
                        .copied()
                        .filter(|arrival_type| arrival_type.scalar_type == value_type.scalar_type)
                        .unwrap_or(value_type);
                    if arrival_type != value_type {
                        arrival = Some(value_type);
                    }
                    scalar_actuals.push(parameter(prefix.len(), arrival_type));
                    prefix.push(arrival_type);
                }
                Operand::Snapshot(argument) => {
                    prefix.push(self.argument_type(identities, argument, site, input_types)?);
                }
                Operand::DispatchSelector(argument) => {
                    let value_type = self.argument_type(identities, argument, site, input_types)?;
                    selector_actual = Some(parameter(prefix.len(), value_type));
                    prefix.push(value_type);
                }
                Operand::Selector {
                    structural_ordinal,
                    value,
                } => {
                    structural_arguments[*structural_ordinal]
                        .selector_positions
                        .push(prefix.len());
                    prefix.push(self.argument_type(identities, value, site, input_types)?);
                }
                _ => {}
            }
            arrivals.push(arrival);
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
        let (_, caller_state) = crate::declarations::state_declaration(self.checked, site.state)?;
        let loan_completions = crate::emission::loan_completions::prepare(
            identities,
            self.checked,
            self.machine,
            coordinate,
            self.structural_types,
            |root| {
                site.bindings
                    .owned_structural_home(root, self.checked.state_parameters(caller_state))
            },
        )?;
        let call = if let checked_trees::CheckedScalarCallee::Dynamic {
            slot,
            interface_identity,
            ..
        } = callee
        {
            let requirement_identity = self
                .checked
                .facts
                .flow
                .terminal_unit_effects
                .dynamic_slots
                .iter()
                .find(|plan| plan.slot == *slot)
                .map(|plan| plan.requirement_identity.clone())
                .ok_or(LoweringError::Unsupported(
                    "selector call names a slot with no checked call shape",
                ))?;
            // A finite-family call's row-selection actuals are retained
            // beside the call as `DynamicFamilyArgument` scalar expressions;
            // lower each through the caller's namespace exactly as an
            // ordinary operand, in roster-binder order.
            let mut family_actuals = Vec::new();
            for family_ordinal in 0usize.. {
                let role = checked_trees::CheckedScalarExpressionRole::DynamicFamilyArgument {
                    call_ordinal,
                    family_ordinal: u32::try_from(family_ordinal).map_err(|_| {
                        LoweringError::Unsupported("dynamic family operand ordinal overflows")
                    })?,
                };
                let Some((_, expression)) = self
                    .checked
                    .facts
                    .values
                    .scalar_expressions
                    .bound_expression_at(
                        site.state,
                        u32::try_from(source.statement_index).map_err(|_| {
                            LoweringError::Unsupported(
                                "dynamic family call statement ordinal overflows",
                            )
                        })?,
                        role,
                    )
                else {
                    break;
                };
                family_actuals.push(site.bindings.expression(
                    identities,
                    &self.checked.facts.proven_scalar_indexes,
                    expression,
                )?);
            }
            LoweredScalarBinding::DynamicCall(
                crate::emission::operation_emission::dynamic_calls::LoweredDynamicCallBinding {
                    source_coordinate: coordinate,
                    loan_completions,
                    source_expression: source.authored_expression,
                    slot: *slot,
                    interface_identity: interface_identity.clone(),
                    requirement_identity,
                    result_type,
                    selector: selector_actual.ok_or(LoweringError::Unsupported(
                        "selector call lost its dispatch operand",
                    ))?,
                    arguments: scalar_actuals,
                    family_actuals,
                    structural_arguments,
                    claim_transfers:
                        crate::emission::claim_bindings::prepare_checked_claim_transfers(
                            identities,
                            self.checked,
                            &claim_transfers,
                            parameters,
                            self.structural_types,
                        )?,
                },
            )
        } else if signature.boundary {
            LoweredScalarBinding::BoundaryCall(crate::emission::operation_emission::boundary_calls::LoweredBoundaryCallBinding {
                source_coordinate: coordinate,
                loan_completions,
                source_expression: source.authored_expression,
                target_machine,
                result_type,
                result_policy: crate::emission::operation_emission::boundary_calls::result_policy::BoundaryResultPolicy::prepare(
                    self.checked, self.machine, coordinate,
                    Some(checked_trees::NominalMachineUseSite::Expression(source.authored_expression)), target_machine,
                )?,
                arguments: scalar_actuals,
                structural_arguments,
                completion_receipts: crate::emission::claim_bindings::prepare_checked_claim_transfers(identities, self.checked, &claim_transfers, parameters, self.structural_types)?,
            })
        } else {
            LoweredScalarBinding::DirectCall(lower_scalar_call(
                identities,
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
                claim_transfers,
                self.structural_types,
                crate::scalar_lowering::contracts::caller_erased_proof_roster(
                    self.checked,
                    self.machine,
                    site.state,
                )?,
                loan_completions,
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
            identities,
            site.state,
            source_call,
            callee.authority_value(),
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
                terminator: LoweredScalarBranchTerminator::jump_with_cleanup(
                    target,
                    super::parameters(&parameter_types),
                    discards,
                ),
                parameter_types,
            })
        };
        let mut continuation = self.binding(call_types, input_types.len(), target, call);
        for (ordinal, operand) in operands.iter().enumerate().rev() {
            let prefix = &prefixes[ordinal];
            // CPS builds successors backwards, but only earlier operands are
            // available at this evaluation point. Keep that lexical prefix
            // scoped while nested calls or selected branches are expanded.
            let established_prefix = self.established_values.len();
            let completed_prefix = self.completed.len();
            // Earlier scalar operands are snapshots in this lexical prefix,
            // just like earlier structural operands. A shared normalized node
            // consumes that carried value rather than invoking its source call
            // again. Distinct authored nodes keep their ordered evaluations.
            self.completed
                .extend(
                    operands[..ordinal]
                        .iter()
                        .enumerate()
                        .filter_map(|(earlier, operand)| match operand {
                            Operand::Snapshot(Argument::Computation(computation))
                            | Operand::Scalar(Argument::Computation(computation))
                            | Operand::DispatchSelector(Argument::Computation(computation))
                            | Operand::Selector {
                                value: Argument::Computation(computation),
                                ..
                            } => Some((*computation, prefixes[earlier].len())),
                            _ => None,
                        }),
                );
            self.established_values
                .extend(
                    operands[..ordinal]
                        .iter()
                        .filter_map(|operand| match operand {
                            Operand::Produced(slot) => Some(EstablishedValue::Produced {
                                root: slot.root,
                                place: slot.place,
                                structural_type: slot.structural_type,
                            }),
                            Operand::Copy { value, .. } => Some(value.clone()),
                            _ => None,
                        }),
                );
            continuation = match operand {
                Operand::Scalar(argument) => {
                    let arrival = match arrivals[ordinal] {
                        Some(value_type) => {
                            self.qualification_edge(prefix, value_type, continuation)
                        }
                        None => continuation,
                    };
                    self.argument(identities, argument, prefix, arrival, site, active)?
                }
                Operand::Snapshot(argument)
                | Operand::DispatchSelector(argument)
                | Operand::Selector {
                    value: argument, ..
                } => self.argument(identities, argument, prefix, continuation, site, active)?,
                Operand::ByteLiteral(slot) => self.push(LoweredScalarBranchState {
                    structural_parameters: Vec::new(),
                    parameter_types: prefix.clone(),
                    erased_formal_types: Vec::new(),
                    erased_proof_formals: Vec::new(),
                    bindings: Vec::new(),
                    structural_effects: vec![LoweredScalarEffect::EstablishByteSequenceLiteral {
                        literal: slot.literal.clone(),
                        memberships: Vec::new(),
                    }],
                    terminator: LoweredScalarBranchTerminator::jump(
                        continuation,
                        super::parameters(prefix),
                    ),
                }),
                Operand::Range { argument, slot } => self.subslice(
                    identities,
                    argument,
                    slot,
                    prefix,
                    continuation,
                    site,
                    active,
                )?,
                Operand::Copy { effect, .. } => self.push(LoweredScalarBranchState {
                    structural_parameters: Vec::new(),
                    parameter_types: prefix.clone(),
                    erased_formal_types: Vec::new(),
                    erased_proof_formals: Vec::new(),
                    bindings: Vec::new(),
                    structural_effects: vec![effect.clone()],
                    terminator: LoweredScalarBranchTerminator::jump(
                        continuation,
                        super::parameters(prefix),
                    ),
                }),
                Operand::Produced(slot) => {
                    let values = &self.checked.facts.values.structural_values;
                    let prepared =
                        crate::scalar_lowering::branch_states::structural_values::prepare_value(
                            identities,
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
                    if prepared.place != slot.place {
                        return unsupported(
                            "computed value did not establish its reserved operand place",
                        );
                    }
                    if prepared.structural_type != slot.structural_type {
                        return unsupported(
                            "computed value disagrees with its reserved operand carrier",
                        );
                    }
                    prepared.finish(
                        identities,
                        site.state,
                        site.bindings,
                        prefix,
                        continuation,
                        self,
                    )?
                }
            };
            self.established_values.truncate(established_prefix);
            self.completed.truncate(completed_prefix);
        }
        Ok(continuation)
    }

    /// Translate the checked partition in its authored cleanup order. Admission
    /// owns the permission and complement derivation; this step only joins its
    /// exact source to storage and lowers the already selected subtree paths.
    fn temporary_cleanup_actions(
        &self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        state: symbols::SymbolHandle,
        source_call: Handle<checked_trees::FlowCallFact>,
        authority: checked_trees::CheckedStructuralValueHandle,
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
            let ordinal = discard.argument_ordinal.map(|ordinal| ordinal as usize);
            let source = match ordinal {
                Some(ordinal) => arguments
                    .get(ordinal)
                    .and_then(|argument| argument.as_place())
                    .ok_or(LoweringError::Unsupported(
                        "temporary retirement lost its structural operand",
                    ))?
                    .source
                    .clone(),
                None if self
                    .checked
                    .facts
                    .values
                    .structural_values
                    .nodes
                    .is_valid(authority) =>
                {
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
                        expression: self
                            .checked
                            .facts
                            .values
                            .structural_values
                            .nodes
                            .get(authority)
                            .expression,
                    }
                }
                None => return unsupported("authority retirement lost its evaluated operand"),
            };
            if retired.contains(&ordinal)
                || !matches!(
                    source,
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
                identities,
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
                slot.structural_type,
                self.structural_types,
            )?;
            let action_start = actions.len();
            for action in discards {
                use crate::unit::unit_cleanup::PreparedNormalCleanupAction;
                if action.source() != &source {
                    return unsupported("temporary cleanup names a different owner");
                }
                let path = crate::expression_preparation::bindings::structural_paths::lower_typed_argument_path(
                    identities,
                    action.path(
            ),
            slot.structural_type,
            self.structural_types,
                    None,
                    &mut std::iter::empty(),
                    None,
                )?;
                let conditions = crate::unit::unit_cleanup::lower_cleanup_conditions(
                    identities,
                    self.checked,
                    &self
                        .checked
                        .facts
                        .flow
                        .terminal_unit_effects
                        .cleanup_control,
                    action.case_conditions(),
                    self.structural_types,
                    |identities, condition_path| {
                        let path = crate::expression_preparation::bindings::structural_paths::lower_typed_argument_path(identities, condition_path, slot.structural_type, self.structural_types, None, &mut std::iter::empty(), None)?;
                        Ok((slot.place, path))
                    },
                )?;
                let structural_type =
                    crate::terminal_identities::structural_declaration_by_identity(
                        self.structural_types,
                        identities,
                        self.checked,
                        action.type_identity(),
                        "temporary cleanup has no structural type",
                    )?
                    .id;
                actions.push(match action {
                    checked_trees::CheckedNormalCleanupAction::RetireHome { .. } => {
                        PreparedNormalCleanupAction::Resolved(
                            crate::unit::unit_cleanup::CleanupRegion::unconditional(
                                terminal_psi::TerminalAffineCleanupAction::RetireHome(slot.place),
                            ),
                        )
                    }
                    checked_trees::CheckedNormalCleanupAction::Discard(_)
                        if path.is_empty() && conditions.is_empty() =>
                    {
                        PreparedNormalCleanupAction::discard_root(slot.place)
                    }
                    checked_trees::CheckedNormalCleanupAction::Discard(_) => {
                        PreparedNormalCleanupAction::Resolved(
                            crate::unit::unit_cleanup::CleanupRegion {
                                selected_region: None,
                                conditions,
                                action: terminal_psi::TerminalAffineCleanupAction::DiscardResidual(
                                    terminal_psi::StructuralAffineDiscard {
                                        place: slot.place,
                                        path,
                                        structural_type,
                                    },
                                ),
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
                            conditions,
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
