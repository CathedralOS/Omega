//! Operation emission for one composed-graph state or call leaf.
//!
//! `emit_call_operations` walks a state's checked operations in order and
//! emits every one through `attached_unit::operation_frame::OperationFrame`,
//! the per-operation emitter the ordinary machine uses. This file owns only
//! what is composed-specific: the frame over the state's environment (its
//! registry-backed result namespace, the machine-wide claim table, the
//! shared temporary roster and the primitive locals the sequence establishes)
//! and consumption of the same authored-position operand schedule as ordinary
//! machines. Structural snapshots must stay between earlier and later scalar
//! side effects even when the surrounding body has state transitions.
//!
//! One state evaluates retained values at many places — its scalar prefix, its
//! entry guard, a successor operand, a range endpoint, a return arm — and each
//! of them needs that same frame and the same closure-wide call namespace.
//! [`StateFrame`] builds the frame for one evaluation and
//! [`with_call_namespace`] borrows the namespace for one operation, so no site
//! re-derives either by hand.
use super::super::super::StructuralParameterDeclaration;
use super::super::argument_schedule::{self, Step};
use super::super::bodies::UnitPlans;
use super::super::operation_frame::{
    CallInputs, Callees, CallerCustody, OperationEmission, OperationFrame, StructuralResults,
    StructuralTypeRoster,
};
use super::super::primitive_locals::PrimitiveLocal;
use super::super::{CheckedUnitEffectOperationPlan, ValueDeclaration, unsupported};
use super::{CheckedTrees, LoweringError, catalogs, literal_arguments};
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::emission::operation_emission::calls::CallEmissionContext;
use std::borrow::Cow;

/// Storage established by one selected operation sequence. Scalar operands,
/// ordinary operations and its final guard share this custody frontier.
pub(super) struct OperationSequence {
    windows: crate::emission::borrowed_window::BorrowedWindowLedger,
    primitive_locals: Vec<PrimitiveLocal>,
}

impl OperationSequence {
    pub(super) fn new(evaluation: &super::super::argument_evaluation::Evaluation) -> Self {
        Self {
            windows: Default::default(),
            primitive_locals: evaluation.parameter_storage.clone(),
        }
    }

    /// Sibling paths inherit storage, never each other's later establishments.
    /// Forking does not depart: checked crash-only branches may abandon debt.
    pub(super) fn branch(&self) -> Self {
        Self {
            windows: self.windows.clone(),
            primitive_locals: self.primitive_locals.clone(),
        }
    }

    pub(super) fn require_closed(&self) -> Result<(), LoweringError> {
        self.windows.require_closed()
    }

    pub(super) fn finish_state(
        &self,
        checked: &CheckedTrees,
        state: &checked_trees::CheckedComposedUnitControlStatePlan,
    ) -> Result<(), LoweringError> {
        match state
            .terminator
            .all_paths_crash(&state.operations, &checked.facts.flow.guarded_exits)
        {
            Some(true) => Ok(()),
            Some(false) => self.require_closed(),
            None => unsupported("composed abandonment roster is stale"),
        }
    }
}

/// The operation half of the catalog borrows independently of scalar call
/// context construction. Both still use the catalog's one identity namespace.
pub(super) struct ComposedOperationCatalogs<'frame, 'catalogs> {
    pub(super) structural_types:
        &'frame mut Cow<'catalogs, [crate::unit::StructuralTypeDeclaration]>,
    type_ids: &'frame [(
        typed_trees::type_identity::TypeIdentityId,
        crate::unit::StructuralTypeId,
    )],
    service_ids: &'frame [(crate::unit::ServiceReachId, crate::unit::ServiceId)],
    signatures: &'frame [super::super::signatures::MachineSignature],
    boundary_parameters: &'frame [super::super::operation_frame::BoundaryParameters],
    domain_ids: &'frame [crate::terminal_identities::StructuralDomainBinding],
    domain_declarations: &'frame [terminal_psi::StructuralDomainDeclaration],
    closure: &'frame [symbols::SymbolHandle],
    temporary_places: &'frame mut Vec<crate::unit::StructuralPlaceDeclaration>,
    result_places: &'frame mut Vec<crate::unit::StructuralPlaceDeclaration>,
    next_place: &'frame mut u64,
}

impl<'catalogs> catalogs::ComposedCatalogs<'catalogs> {
    pub(super) fn operation_catalogs(
        &mut self,
    ) -> (
        ComposedOperationCatalogs<'_, 'catalogs>,
        &mut super::scalar_calls::ComposedScalarCalls,
    ) {
        (
            ComposedOperationCatalogs {
                structural_types: &mut self.structural_types,
                type_ids: &self.type_ids,
                service_ids: &self.service_ids,
                signatures: &self.signatures,
                boundary_parameters: &self.boundary_parameters,
                domain_ids: &self.domain_ids,
                domain_declarations: self.domain_declarations,
                closure: self.closure,
                temporary_places: &mut self.temporary_places,
                result_places: &mut self.result_places,
                next_place: &mut self.next_place,
            },
            &mut self.scalar_calls,
        )
    }
}

impl ComposedOperationCatalogs<'_, '_> {
    /// Bind the state's actual source owner and transported parameters to the
    /// same catalog, private allocator and sequence frontier for every value.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn operation_frame<'frame, 'calls>(
        &'frame mut self,
        checked: &'frame CheckedTrees,
        machine: symbols::SymbolHandle,
        state: &'frame checked_trees::CheckedComposedUnitControlStatePlan,
        parameters: &'frame [StructuralParameterDeclaration],
        erased_parameters: &'frame [ValueDeclaration],
        sequence: &'frame mut OperationSequence,
        emission: OperationEmission<'frame, 'calls>,
    ) -> OperationFrame<'frame, 'calls> {
        let scalar_result = match &state.terminator {
            checked_trees::CheckedComposedUnitControlTerminatorPlan::ReturnScalar {
                completion: checked_trees::CheckedScalarReturnPlan::Binding(binding),
            } => Some(binding),
            _ => None,
        };
        OperationFrame {
            checked,
            machine,
            state: state.state,
            scalar_result,
            scalar_parameter_count: state.scalar_parameters.len(),
            source_value_count: emission.source_value_count,
            parameters: emission.evaluation.current_parameters(parameters),
            structural_types: match &mut *self.structural_types {
                Cow::Owned(types) => StructuralTypeRoster::Owned(types),
                Cow::Borrowed(types) => StructuralTypeRoster::Published(types),
            },
            type_ids: &self.type_ids,
            service_ids: &self.service_ids,
            primitive_locals: &sequence.primitive_locals,
            results: StructuralResults::StateGraph {
                state,
                places: &mut self.result_places,
            },
            private_places: &mut self.temporary_places,
            windows: &mut sequence.windows,
            evaluation: emission.evaluation,
            values: emission.values,
            next_place: &mut self.next_place,
            next_value: emission.next_value,
            next_block: emission.next_block,
            next_edge: emission.next_edge,
            calls: emission.calls,
            operations: emission.operations,
            callees: Callees {
                plans: UnitPlans::published(&checked.facts.flow),
                signatures: &self.signatures,
                boundaries: &self.boundary_parameters,
                domain_ids: &self.domain_ids,
                domain_declarations: self.domain_declarations,
                closure: self.closure,
            },
            caller: CallerCustody {
                erased_scalar_parameters: erased_parameters,
                erased_proof_parameters: &state.erased_proof_parameters,
                local_places: &[],
            },
        }
    }
}

/// The frame over one composed state's environment.
///
/// A state evaluates retained checked values at many places — its scalar
/// prefix, its entry guard, a successor operand, a range endpoint, a return
/// arm — and every one of them needs the same per-operation frame: the emitter
/// the ordinary machine also uses, over the state's checked owner, plan,
/// erased formals and custody frontier. `StateFrame` binds the parts that
/// cannot change between those places, so a site hands over only what is
/// genuinely local to it: the transported parameters, the operation sequence
/// under evaluation and the namespace that evaluation reads and advances.
pub(super) struct StateFrame<'f> {
    checked: &'f CheckedTrees,
    /// The machine that authored the emitting state. A fused roster mixes
    /// owners, so each position's facts resolve in its own.
    machine: symbols::SymbolHandle,
    state: &'f checked_trees::CheckedComposedUnitControlStatePlan,
    /// The state's erased formals: the machine contract's for a plain entry,
    /// its own block's declarations otherwise.
    erased_parameters: &'f [ValueDeclaration],
}

impl<'f> StateFrame<'f> {
    pub(super) fn new(
        checked: &'f CheckedTrees,
        machine: symbols::SymbolHandle,
        state: &'f checked_trees::CheckedComposedUnitControlStatePlan,
        erased_parameters: &'f [ValueDeclaration],
    ) -> Self {
        Self {
            checked,
            machine,
            state,
            erased_parameters,
        }
    }

    /// Build this state's operation frame for one evaluation and hand it to
    /// `evaluate`, publishing the advanced call-obligation counter back to the
    /// closure's call namespace. The frame borrows the site's value namespace,
    /// counters and operation buffer for the call, so the site reads and
    /// writes them through the frame it splits.
    pub(super) fn evaluate<T>(
        &self,
        catalogs: &mut catalogs::ComposedCatalogs<'_>,
        parameters: &[StructuralParameterDeclaration],
        sequence: &mut OperationSequence,
        values: FrameValues<'_>,
        evaluate: impl FnOnce(OperationFrame<'_, '_>) -> Result<T, LoweringError>,
    ) -> Result<T, LoweringError> {
        let (mut operation_catalogs, scalar_calls) = catalogs.operation_catalogs();
        with_call_namespace(scalar_calls, |calls| {
            let frame = operation_catalogs.operation_frame(
                self.checked,
                self.machine,
                self.state,
                parameters,
                self.erased_parameters,
                sequence,
                OperationEmission {
                    source_value_count: values.source_value_count,
                    evaluation: values.evaluation,
                    values: values.values,
                    next_value: values.next_value,
                    next_block: values.next_block,
                    next_edge: values.next_edge,
                    calls,
                    operations: values.operations,
                },
            );
            evaluate(frame)
        })
    }
}

/// The live namespace one frame evaluation reads and advances: the completed
/// values the operation's authored source can still name, the private staging
/// that follows any control split, the dense counters, and the operation
/// buffer the frame emits into. The call namespace and its obligation counter
/// belong to `with_call_namespace`, not to a site.
pub(super) struct FrameValues<'f> {
    pub(super) source_value_count: usize,
    pub(super) evaluation: &'f mut super::super::argument_evaluation::Evaluation,
    pub(super) values: &'f mut Vec<ValueDeclaration>,
    pub(super) next_value: &'f mut u64,
    pub(super) next_block: &'f mut u64,
    pub(super) next_edge: &'f mut u64,
    pub(super) operations: &'f mut OperationBuffer,
}

/// Borrow the closure's call namespace for one operation.
///
/// Every composed evaluation and call emission resolves against the one
/// scalar-call namespace this machine was admitted with, and an operation may
/// allocate requirement obligations in it. The advanced counter is published
/// back to the catalog when the operation ends so the next operation's
/// namespace starts past it; keeping the borrow and its publication together
/// keeps a site from publishing a stale counter.
pub(super) fn with_call_namespace<T>(
    scalar_calls: &mut super::scalar_calls::ComposedScalarCalls,
    run: impl FnOnce(&mut CallEmissionContext<'_>) -> Result<T, LoweringError>,
) -> Result<T, LoweringError> {
    let mut calls = scalar_calls.emission_context();
    let result = run(&mut calls);
    scalar_calls.next_call_obligation = calls.next_obligation_identity;
    result
}

#[allow(clippy::too_many_arguments)]
pub(super) fn emit_call_operations(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: &checked_trees::CheckedComposedUnitControlStatePlan,
    planned_operations: &[CheckedUnitEffectOperationPlan],
    catalogs: &mut catalogs::ComposedCatalogs,
    parameters: &[StructuralParameterDeclaration],
    evaluation: &mut super::super::argument_evaluation::Evaluation,
    values: &mut Vec<ValueDeclaration>,
    // The emitting machine's erased roster, for `ErasedParameter` actuals
    // forwarded through proof-only call operands.
    erased_parameters: &[ValueDeclaration],
    next_value: &mut u64,
    next_block: &mut u64,
    next_edge: &mut u64,
    operations: &mut OperationBuffer,
    sequence: &mut OperationSequence,
) -> Result<(), LoweringError> {
    // Borrowed-storage windows open and close within one straight-line
    // operation sequence. Every normal exit follows the last operation and
    // requires closed debt. An all-crash terminator abandons that frontier
    // without a returning owner, cleanup, or repair. A mixed crash/normal
    // roster still requires restoration; no state-graph join receives a hole.
    // Blocks split off by argument evaluation rejoin with the one frontier
    // this sequence carries.
    // The state's `mut` formal storage and the mutable primitive locals this
    // sequence establishes. Their places live in the shared private roster;
    // this roster keys later stores to them by authored symbol, as the
    // ordinary machine's roster does.
    let schedule =
        argument_schedule::build_operations(checked, machine, state.state, planned_operations)?;
    let mut schedule = schedule.iter().peekable();
    let mut retained_source_count = None;
    let mut staged_scalar_slots = vec![Vec::new(); planned_operations.len()];
    let mut staged_ranges = vec![Vec::new(); planned_operations.len()];
    let mut staged_selector_slots = vec![Vec::new(); planned_operations.len()];
    while let Some(step) = schedule.next() {
        let (operation_index, staged) = match *step {
            Step::CaptureBefore(statement) => {
                evaluation.prepare_storage_before(
                    identities,
                    checked,
                    state.state,
                    statement,
                    &catalogs.structural_types,
                    values,
                    &mut catalogs.next_place,
                    next_value,
                    operations,
                    &mut catalogs.temporary_places,
                )?;
                continue;
            }
            Step::Begin => {
                if retained_source_count.replace(values.len()).is_some() {
                    return unsupported("composed argument group was already active");
                }
                continue;
            }
            Step::End => {
                if retained_source_count.take().is_none() {
                    return unsupported("composed argument group was not active");
                }
                continue;
            }
            Step::Argument { operation, ordinal } => {
                let source_value_count = retained_source_count.ok_or(
                    LoweringError::Unsupported("composed scalar operand has no source prefix"),
                )?;
                if staged_scalar_slots[operation].len() != ordinal {
                    return unsupported("composed scalar operand ordinals are not dense");
                }
                let value = StateFrame::new(checked, machine, state, erased_parameters).evaluate(
                    catalogs,
                    parameters,
                    sequence,
                    FrameValues {
                        source_value_count,
                        evaluation,
                        values,
                        next_value,
                        next_block,
                        next_edge,
                        operations,
                    },
                    |frame| {
                        let (mut environment, emission) = frame.split();
                        emission.evaluation.argument_at(
                            identities,
                            checked,
                            machine,
                            state.state,
                            &planned_operations[operation],
                            ordinal,
                            source_value_count,
                            emission.values,
                            emission.next_value,
                            emission.next_block,
                            emission.next_edge,
                            emission.operations,
                            emission.calls,
                            &mut environment,
                        )
                    },
                )?;
                staged_scalar_slots[operation].push(values.len());
                values.push(value);
                continue;
            }
            Step::Selector {
                operation,
                argument,
                segment,
            } => {
                let source_value_count = retained_source_count.ok_or(
                    LoweringError::Unsupported("composed selector has no source prefix"),
                )?;
                let value = StateFrame::new(checked, machine, state, erased_parameters).evaluate(
                    catalogs,
                    parameters,
                    sequence,
                    FrameValues {
                        source_value_count,
                        evaluation,
                        values,
                        next_value,
                        next_block,
                        next_edge,
                        operations,
                    },
                    |frame| {
                        let (mut environment, emission) = frame.split();
                        emission.evaluation.call_selector(
                            identities,
                            checked,
                            machine,
                            state.state,
                            &planned_operations[operation],
                            argument,
                            segment,
                            source_value_count,
                            emission.values,
                            emission.next_value,
                            emission.next_block,
                            emission.next_edge,
                            emission.operations,
                            emission.calls,
                            &mut environment,
                        )
                    },
                )?;
                staged_selector_slots[operation].push(values.len());
                values.push(value);
                continue;
            }
            Step::Endpoint {
                operation,
                argument,
            } => {
                let source_value_count = retained_source_count.ok_or(
                    LoweringError::Unsupported("composed endpoint operand has no source prefix"),
                )?;
                StateFrame::new(checked, machine, state, erased_parameters).evaluate(
                    catalogs,
                    parameters,
                    sequence,
                    FrameValues {
                        source_value_count,
                        evaluation,
                        values,
                        next_value,
                        next_block,
                        next_edge,
                        operations,
                    },
                    |frame| {
                        let (mut environment, emission) = frame.split();
                        emission.evaluation.window_endpoints(
                            identities,
                            checked,
                            machine,
                            state.state,
                            &planned_operations[operation],
                            argument,
                            source_value_count,
                            emission.values,
                            emission.next_value,
                            emission.next_block,
                            emission.next_edge,
                            emission.operations,
                            emission.calls,
                            &mut environment,
                        )
                    },
                )?;
                continue;
            }
            Step::Subslice { operation, ordinal } => {
                let source_value_count = retained_source_count.ok_or(
                    LoweringError::Unsupported("composed range operand has no source prefix"),
                )?;
                if staged_ranges[operation]
                    .iter()
                    .any(|(prior, _)| *prior == ordinal)
                {
                    return unsupported("composed range operand is evaluated more than once");
                }
                let place = literal_arguments::stage_range(
                    identities,
                    checked,
                    state,
                    &planned_operations[operation],
                    ordinal,
                    source_value_count,
                    catalogs,
                    evaluation,
                    values,
                    next_value,
                    next_block,
                    next_edge,
                    operations,
                    machine,
                    parameters,
                    erased_parameters,
                    sequence,
                )?;
                staged_ranges[operation].push((ordinal, place));
                continue;
            }
            Step::Call(index) | Step::Operation(index) if retained_source_count.is_some() => {
                (index, true)
            }
            Step::Ordinary(index) if retained_source_count.is_none() => (index, false),
            _ => return unsupported("composed call schedule disagrees with its argument group"),
        };
        let operation = &planned_operations[operation_index];
        if matches!(
            operation,
            CheckedUnitEffectOperationPlan::ReleaseReference {
                defer_to_exit: true,
                ..
            }
        ) {
            continue;
        }
        let source_value_count = retained_source_count.unwrap_or(values.len());
        // Keep private argument values across constructor evaluation and any
        // split blocks. Read their rebound identities only at the consuming
        // call, never from the values they held before a later operand ran.
        let staged_scalars = staged_scalar_slots[operation_index]
            .iter()
            .map(|slot| {
                values.get(*slot).copied().ok_or(LoweringError::Unsupported(
                    "composed scalar operand lost its retained value",
                ))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let staged_selectors = staged_selector_slots[operation_index]
            .iter()
            .map(|slot| {
                values.get(*slot).copied().ok_or(LoweringError::Unsupported(
                    "composed selector lost its retained value",
                ))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let establishes_local = matches!(
            operation,
            CheckedUnitEffectOperationPlan::EstablishLocal(
                checked_trees::CheckedUnitLocalBindingPlan::Primitive { .. }
            )
        );
        let call_operands = if establishes_local || OperationFrame::lowers(operation) {
            None
        } else {
            Some(literal_arguments::evaluate(
                identities,
                checked,
                machine,
                state,
                operation,
                staged.then_some(literal_arguments::StagedArguments {
                    scalars: &staged_scalars,
                    ranges: &staged_ranges[operation_index],
                }),
                catalogs,
                evaluation,
                values,
                next_value,
                next_block,
                next_edge,
                operations,
                parameters,
                erased_parameters,
                sequence,
            )?)
        };
        // The final call publishes into the state's ordinary scalar binding
        // namespace. Retire argument temporaries first, after copying its
        // completed operands, so the result receives its actual source slot.
        if matches!(step, Step::Call(_)) && matches!(schedule.peek(), Some(Step::End)) {
            values.truncate(source_value_count);
        }
        let mut established_local = None;
        StateFrame::new(checked, machine, state, erased_parameters).evaluate(
            catalogs,
            parameters,
            sequence,
            FrameValues {
                source_value_count,
                evaluation,
                values,
                next_value,
                next_block,
                next_edge,
                operations,
            },
            |frame| {
                match &call_operands {
                    None if establishes_local => {
                        let local = frame.establish_primitive_local(identities, operation)?;
                        established_local = Some(local);
                    }
                    None => frame.emit(identities, operation)?,
                    Some((arguments, byte_places)) => frame.emit_call(
                        identities,
                        operation,
                        CallInputs {
                            evaluated_scalar_arguments: arguments.as_deref(),
                            byte_places,
                            evaluated_selectors: staged.then_some(staged_selectors.as_slice()),
                        },
                    )?,
                }
                Ok(())
            },
        )?;
        if let Some(local) = established_local {
            catalogs.temporary_places.push(local.declaration);
            sequence.primitive_locals.push(local);
        }
        if operations.external_completion.is_some() {
            break;
        }
    }
    sequence.finish_state(checked, state)
}
