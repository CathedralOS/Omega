//! The single-state body producer: authored-order scalar bindings, primitive
//! and structural field stores, borrowed-window moves and repairs, selected
//! operator and FMA applications, structural bindings, view-subslice locals,
//! calls and completion.
//! Constructors and calls share structural binding ordinals. Completion selects
//! one existing result or constructs its final expression in this same sequence.
//! Each statement is planned by the emitter its kind and evidence select; no
//! whole-body shape chooses a different producer, so any statement the
//! sequence admits composes with any other.
//!
//! `build` sets up the sequence and walks the body's statements in order:
//! `local_data` plans a `let`, `assignments` plans an assignment, and
//! `statement_call` then plans the call either statement leaves (or a call
//! or expression statement performs). `completion` closes the body: windows,
//! the structural or scalar result, and call-count agreement.
use super::super::{
    CheckedTrivialAffineStructuralLocalPlan, CheckedUnitEntryClaimPlan,
    CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralArgumentSourcePlan,
};
use super::{
    CheckFacts, CheckedScalarExpression, CheckedScalarExpressionRole, CheckedStructuralAccess,
    CheckedStructuralScalarParameterPlan, CheckedUnitEffectOperationPlan,
    CheckedUnitScalarResultBindingPlan, CheckedUnitStructuralParameterPlan,
    CheckedUnitStructuralResultBindingPlan, ExpressionNode, Multiplicity, StatementNode,
    SymbolHandle, TypedTrees,
};
use crate::execution::terminal_unit::ScalarCalleePlans;
use crate::execution::terminal_unit::calls::{ExpectedCallValueResult, build_call_operation};
use crate::execution::terminal_unit::control::LocalConstructionTrace;
use crate::execution::terminal_unit::control::call_occurrences;
use crate::execution::terminal_unit::control::call_results::bind_scalar_call_result;
use crate::execution::terminal_unit::control::call_results::bind_structural_call_result;
use crate::execution::terminal_unit::control::call_results::checked_structural_result_type;
use crate::execution::terminal_unit::control::structural_operands;
use crate::execution::terminal_unit::returns::checked_boolean_contains_short_circuit;
use crate::execution::terminal_unit::scalar_locals::scalar_expression_local_at;
use crate::execution::terminal_unit::types::{
    ShapeCollector, is_unit, machine_binders, parameter_qualifications,
};

use checked_trees::CheckedUnitStructuralReturnPlan;

mod assignments;
pub(in crate::execution::terminal_unit) mod bounded_byte_values;
mod completion;
mod local_data;
mod reference_retirement;
mod result_consumption;
mod returned_values;
mod statement_call;
mod storage_initializer;
pub(in crate::execution::terminal_unit) use result_consumption::{
    append_exit_reference_releases, append_reference_releases, consume_result, consume_results,
    consume_retained_results, consume_value_call_sources, consume_value_places,
    retain_selected_sources,
};
pub(in crate::execution::terminal_unit) use returned_values::{
    returned_byte_sequence_literal, returned_mutable_view, returned_named_view, returned_parameter,
    returned_reference_leaf, returned_subslice,
};

pub(in crate::execution::terminal_unit) struct StatementSequence {
    pub(in crate::execution::terminal_unit) scalar_result:
        Option<CheckedUnitScalarResultBindingPlan>,
    pub(in crate::execution::terminal_unit) scalar_control:
        Option<checked_trees::CheckedUnitScalarControlPlan>,
    pub(in crate::execution::terminal_unit) structural_result:
        Option<CheckedUnitStructuralReturnPlan>,
    pub(in crate::execution::terminal_unit) operations: Vec<CheckedUnitEffectOperationPlan>,
    pub(in crate::execution::terminal_unit) structural_local_symbols: Vec<SymbolHandle>,
    /// Every structural result binding the sequence established, keyed by
    /// the authored root it publishes (a `let` local's symbol or an
    /// anonymous operand's expression). A state's successor edges plan their
    /// own operand calls against these, as a later statement's call would.
    pub(in crate::execution::terminal_unit) structural_results:
        Vec<(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)>,
    /// The dense scalar binding ordinal the next retained result takes after
    /// this sequence: the prefix's immutable bindings, then every scalar
    /// result the sequence retained. A guarded arm's terminal call result
    /// numbers here, in the namespace its arm inherits.
    pub(in crate::execution::terminal_unit) next_scalar_binding: usize,
}

pub(super) fn has_structural_result(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    statement: &StatementNode,
) -> bool {
    let StatementNode::LocalData(local) = statement else {
        return false;
    };
    if !local.initial_value.is_valid() {
        return false;
    }
    if facts
        .values
        .structural_values
        .roots
        .iter()
        .any(|(_, root)| {
            root.machine == machine.symbol
                && root.expression == local.initial_value
                && root.type_reference == local.type_reference
        })
    {
        return true;
    }
    if validation::is_scalar_case_value(program, local.initial_value, local.type_reference) {
        return !local.is_mutable;
    }
    if validation::is_closed_primitive_array_type(program, local.type_reference) {
        return validation::scalar_array_elements(
            program,
            machine.symbol,
            local.initial_value,
            local.type_reference,
        )
        .is_some()
            || matches!(
                program.expression_table.expression(local.initial_value),
                ExpressionNode::Call(_)
            );
    }
    if !matches!(
        program.expression_table.expression(local.initial_value),
        ExpressionNode::Call(_)
    ) {
        return false;
    }
    // Only the local's own result shape decides sequencing: the call
    // operation rejoins the callee's actual result (including a routed or
    // generic callee's specialized one) when it binds this local, and
    // closure pruning still requires the actual ordinary/composed callee plan.
    super::checked_structural_result_type(
        program,
        &mut ShapeCollector::new(program),
        local.type_reference,
        &machine_binders(program, machine),
    )
    .is_some()
}

/// The trace phase for one statement kind in the shared sequence, so an
/// omission names which statement family stopped local construction.
fn statement_phase(statement: &StatementNode) -> &'static str {
    match statement {
        StatementNode::Assignment(_) => "statement sequence: assignment",
        StatementNode::LocalData(_) => "statement sequence: local data",
        StatementNode::Call(_) => "statement sequence: call",
        StatementNode::Expression(_) => "statement sequence: expression",
        StatementNode::RootBinding(_) => "statement sequence: root binding",
        StatementNode::AssemblyFact(_) => "statement sequence: assembly fact",
        StatementNode::Transition(_) => "statement sequence: transition",
    }
}

/// The first statement after the prefix whose kind the sequence cannot plan
/// at all: a transition mid-body, an expression that neither calls nor
/// completes, or a statement with no sequencing route. Statements the
/// sequence admits by kind can still decline while planning; this names
/// only the kind refusal, with its index for the construction trace.
pub(super) fn first_unsupported_statement(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    construction_statement_count: usize,
) -> Option<usize> {
    let completion = scalar_control(program, facts, machine, state);
    let prefix_count = completion
        .as_ref()
        .map(|(_, count)| *count)
        .unwrap_or(usize::MAX);
    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
        .take(prefix_count)
        .skip(construction_statement_count)
        .find(|(index, statement)| !match statement {
            StatementNode::Call(_) | StatementNode::Assignment(_) => true,
            StatementNode::Expression(expression) => {
                call_occurrences::ordered_statement_call(program, machine, state, *index).is_some()
                    || (*index + 1
                        == program
                            .statement_table
                            .statements(state.statement_nodes)
                            .len()
                        && (u32::try_from(*index).ok().is_some_and(|ordinal| {
                            facts
                                .values
                                .structural_values
                                .root_for_expression(state.symbol, ordinal, *expression)
                                .is_some()
                        }) || validation::is_scalar_case_value(
                            program,
                            *expression,
                            state.return_type,
                        ) || matches!(
                            program.expression_table.expression(*expression),
                            ExpressionNode::Name(_)
                        ) || validation::reference_result_custody::source_leaf(program, state)
                            .is_some()
                            || (crate::execution::terminal_unit::types::borrowed_named_view(
                                program,
                                state.return_type,
                            ) && crate::flow::canonical_place_from_expression_in_state(
                                program,
                                state.symbol,
                                *index,
                                *expression,
                            )
                            .is_some_and(|place| {
                                matches!(place.root, facts::PlaceRoot::Symbol(_))
                            }))
                            || (crate::execution::terminal_unit::types::borrowed_slice_view(
                                program,
                                state.return_type,
                            ) && matches!(
                                program.expression_table.expression(*expression),
                                ExpressionNode::Indexed(_) | ExpressionNode::String(_)
                            ))
                            || bounded_byte_values::expression_octets(
                                program,
                                state.return_type,
                                *expression,
                            )
                            .is_some()
                            || validation::is_closed_primitive_array_type(
                                program,
                                state.return_type,
                            )
                            || program
                                .primitive_type_reference(state.return_type)
                                .is_some()))
            }
            // The local planner owns initializer/type admission. Repeating its
            // producers here creates a second vocabulary that can reject a
            // supported declaration before its own planner sees it.
            StatementNode::LocalData(_) => true,
            // Each transition arm lowers to its own statement, so a dispatch
            // is a trailing RUN of transitions ending the state body. The
            // sequence plans the statements before it and the terminator
            // machinery owns the run; a transition anywhere else would mean
            // authored statements the take_while below silently skips.
            StatementNode::Transition(_) => program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .skip(*index)
                .all(|rest| matches!(rest, StatementNode::Transition(_))),
            _ => false,
        })
        .map(|(index, _)| index)
}

/// Alias bindings whose referents are represented directly by their uses.
/// A supported capture instead establishes an ordinary reference value: later
/// transfers need its descriptor identity, even if the borrow also admits an
/// alias of the backing storage.
fn erased_alias_locals(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
) -> Vec<SymbolHandle> {
    let mut erased = super::super::receiver_aliases::aliases(program, facts, machine, state)
        .unwrap_or_default()
        .into_iter()
        .map(|alias| alias.owner)
        .collect::<Vec<_>>();
    erased.extend(
        super::super::receiver_aliases::restored_call_alias_locals(program, facts, machine, state)
            .unwrap_or_default(),
    );
    // A checked capture that establishes a descriptor is an ordinary value
    // binding, even when it also admits an alias of its backing. Erasing that
    // declaration leaves later descriptor transfers with no producer.
    erased.retain(|symbol| {
        let Some((statement_index, local)) = program.state_local_declaration(state, *symbol) else {
            return true;
        };
        validation::reference_result_custody::primitive_local_source(
            program,
            state,
            statement_index,
        )
        .or_else(|| {
            validation::reference_result_custody::borrowed_initializer_source(
                program,
                state,
                local.initial_value,
                local.type_reference,
            )
        })
        .is_none()
    });
    erased
}

/// A record pattern's marker (`let __destructure#x#y = self.pair;`) carries
/// only the spelled field set validation checks for exhaustiveness. The
/// pattern's bindings are its per-field locals, each an ordinary re-read of
/// the same place, so the marker declares no storage and plans no operation.
fn is_record_pattern_marker(local: &typed_trees::statement::TableLocalData) -> bool {
    local.name.as_str().starts_with("__destructure#")
}

/// The machine an assignment's right-hand side calls, when it is a direct call.
fn statement_call_target(
    program: &TypedTrees,
    assignment: &typed_trees::statement::TableAssignment,
) -> Option<SymbolHandle> {
    let ExpressionNode::Call(call) = program.expression_table.expression(assignment.value) else {
        return None;
    };
    Some(crate::flow::call_target_symbol(program, call))
}

#[allow(clippy::too_many_arguments)]
pub(in crate::execution::terminal_unit) fn build(
    program: &TypedTrees,
    facts: &CheckFacts,
    scalar_callees: ScalarCalleePlans<'_>,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    structural_parameters: &mut [CheckedUnitStructuralParameterPlan],
    scalar_parameters: &[CheckedStructuralScalarParameterPlan],
    entry_claims: &[CheckedUnitEntryClaimPlan],
    calls: &[&checked_trees::FlowCallFact],
    trivial_affine_locals: &[(CheckedTrivialAffineStructuralLocalPlan, SymbolHandle)],
    construction_statement_count: usize,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    trace: &LocalConstructionTrace,
) -> Option<StatementSequence> {
    trace.phase("statement sequence: scalar control guard");
    let scalar_control = scalar_control(program, facts, machine, state).map(|(control, _)| control);
    // A bounded byte owner's primitive predicates survive both a shared loan
    // and a mutable loan whose stores preserve those predicates. Scalar
    // completion changes neither that obligation nor the backing's identity.
    let bounded_owner = |parameter: &CheckedUnitStructuralParameterPlan| {
        matches!(
            parameter.access,
            CheckedStructuralAccess::SharedBorrow | CheckedStructuralAccess::MutableBorrow
        ) && parameter.projected_qualifications.is_empty()
            && parameter.qualifications.iter().all(|qualification| {
                shapes.domains.iter().any(|domain| {
                    domain.domain == *qualification
                        && domain.carrier_type_identity == parameter.type_identity
                        && domain.byte_predicate.is_some()
                        && domain.establishment_routes.is_empty()
                })
            })
            && program
                .state_parameters(state)
                .get(parameter.position as usize)
                .and_then(|source| {
                    crate::execution::terminal_unit::types::byte_sequence_carrier(
                        program,
                        source.type_reference,
                        &[],
                    )
                })
                .is_some_and(|carrier| {
                    matches!(
                        carrier,
                        checked_trees::CheckedByteSequenceCarrier::BoundedOwned { .. }
                    )
                })
    };
    // A scalar-control tail does not change affine disposal eligibility.
    // The ordinary completion producer authenticates no-code cleanup from
    // the source exit rows; lowering schedules those owners against the
    // emitted selected transfers, including owners moved by an arm's call.
    // Live claims and qualified-carrier contracts remain separate from that
    // no-code frontier and cannot be discharged by a scalar result join. A
    // shared loan owes none: it cannot change its referent, so the
    // qualifications it arrived with still hold at every exit.
    if scalar_control.is_some()
        && (!entry_claims.is_empty()
            || structural_parameters.iter().any(|parameter| {
                parameter.multiplicity == Multiplicity::Linear
                    || (!parameter.qualifications.is_empty()
                        && parameter.access != CheckedStructuralAccess::SharedBorrow
                        && !bounded_owner(parameter))
            }))
    {
        return None;
    }
    let mut operations = Vec::new();
    // Erased borrow carriers have no storage binding: the sequence advances
    // past their declarations while each use's planner joins the captured
    // referent's checked loan evidence.
    let erased_locals = erased_alias_locals(program, facts, machine, state);
    let mut scalar_count = program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .take(construction_statement_count)
        .filter(|statement| {
            matches!(statement,
            StatementNode::LocalData(local) if !local.is_mutable
                && program.primitive_type_reference(local.type_reference).is_some())
        })
        .count();
    let mut structural_count = 0_usize;
    let mut structural_local_symbols = Vec::new();
    let statements = program.statement_table.statements(state.statement_nodes);
    let mut windows = super::super::borrowed_windows::OpenWindows::default();
    let mut array_bindings = Vec::<(SymbolHandle, CheckedUnitStructuralResultBindingPlan)>::new();
    let mut returned_call = None;
    let mut returned_scalar_call = None;
    // The result binding an atomic carrier's placeholder local reserved for
    // the carrier assignment that follows it (`atomic_operations.rs`).
    let mut atomic_result = None;
    // Value bindings remain separate from the call's replayed claim custody.
    let mut structural_results = Vec::new();
    let mut call_count = 0_usize;
    let binders = machine_binders(program, machine);
    trace.phase("statement sequence: scalar field store sequence");
    let mut stores =
        super::super::structural_scalar_store::build_structural_scalar_field_store_sequence_traced(
            program,
            facts,
            machine,
            state,
            structural_parameters,
            scalar_parameters,
            construction_statement_count,
            call_frames,
            trace,
        )?
        .into_iter()
        .peekable();
    for (index, statement) in statements
        .iter()
        .enumerate()
        .skip(construction_statement_count)
        .take_while(|(index, statement)| {
            (!matches!(statement, StatementNode::Transition(_))
                || call_occurrences::terminal_named_call(program, machine, state, *index).is_some())
                && !matches!(statement, StatementNode::LocalData(local) if local.name.as_str().starts_with("__arm_destructure#V="))
                && !matches!(statement, StatementNode::Expression(expression)
                    if !is_unit(program, state.return_type)
                        && (!matches!(program.expression_table.expression(*expression), ExpressionNode::Call(_))
                            || u32::try_from(*index).ok().is_some_and(|ordinal| facts.values.scalar_computations.root_at(
                                state.symbol, ordinal, CheckedScalarExpressionRole::Return,
                            ).is_some())))
        })
    {
        let statement_index = u32::try_from(index).ok()?;
        trace.phase(statement_phase(statement));
        trace.statement(Some(statement_index));
        append_reference_releases(program, facts, machine.symbol, state.symbol, statement_index, &structural_results, &mut operations)?;
        // A fact-only citation emits no runtime call or work; its flow row
        // is retired (`execution::citation_erasure`), so it plans nothing.
        if validation::is_fact_only_citation(program, statement) {
            continue;
        }
        // A value-returning body's completing expression is its final
        // statement: scalar tails carry a Return-rooted computation the scan
        // above already stops on, while a structural tail call reaches this
        // loop only in last position. A call statement in any earlier slot is
        // a discarded invocation classified by `unit_statement_call`, not the
        // machine's result producer — treating it as completing would bind
        // the machine's return type against the callee's own result shape.
        let completes_machine = matches!(statement, StatementNode::Expression(expression)
            if !is_unit(program, state.return_type)
                && (!matches!(program.expression_table.expression(*expression), ExpressionNode::Call(_))
                    || index.checked_add(1) == Some(statements.len())))
            // An unconditional named machine invocation in tail position
            // completes a value body the same way: its own result forwards as
            // the machine's result.
            || (matches!(statement, StatementNode::Transition(_))
                && !is_unit(program, state.return_type)
                && call_occurrences::terminal_named_call(program, machine, state, index)
                    .is_some());
        let mut structural_result = None;
        // A store whose source is this statement's own call is appended after
        // that call operation, in ordinary evaluation order.
        let mut call_result_stores = Vec::new();
        // A structural call-result field store also mints the moved-out
        // displaced binding, so it consumes one more structural ordinal than
        // the call's own result.
        let mut structural_store_bindings = 0_usize;
        // An affine field value displaced by the overwrite needs an explicit
        // discard on the call's continuation; unrestricted drops for free.
        if atomic_result.is_some()
            && !matches!(statement, StatementNode::Assignment(assignment)
                if validation::atomic_assignment_carrier(program, assignment).is_some())
        {
            return None;
        }
        let result = match statement {
            StatementNode::Assignment(assignment)
                if validation::atomic_assignment_carrier(program, assignment).is_some() =>
            {
                trace.phase("statement sequence: assignment: atomic event");
                trace.statement(Some(statement_index));
                operations.push(CheckedUnitEffectOperationPlan::AtomicAccess(
                    super::super::atomic_operations::writing_event(
                        program,
                        facts,
                        machine,
                        state,
                        structural_parameters,
                        statement_index,
                        assignment,
                        atomic_result.take(),
                        trace,
                    )?,
                ));
                continue;
            }
            StatementNode::Assignment(assignment) => match assignments::plan(
                assignments::Planner {
                    program,
                    facts,
                    scalar_callees,
                    shapes: &mut *shapes,
                    machine,
                    state,
                    structural_parameters: &mut *structural_parameters,
                    scalar_parameters,
                    entry_claims,
                    trivial_affine_locals,
                    trace,
                    binders: &binders,
                    stores: &mut stores,
                    operations: &mut operations,
                    scalar_count: &mut scalar_count,
                    structural_count: &mut structural_count,
                    windows: &mut windows,
                    structural_results: &mut structural_results,
                },
                statement_index,
                assignment,
            )? {
                assignments::AssignmentPlan::Planned => continue,
                assignments::AssignmentPlan::ScalarCall { result, store } => {
                    call_result_stores.push(store);
                    Some(result)
                }
                assignments::AssignmentPlan::StructuralCall {
                    result,
                    replacement,
                } => {
                    structural_store_bindings = usize::from(replacement.move_out.is_some());
                    call_result_stores.extend(replacement.into_operations());
                    // The store consumes this call's whole result, so the
                    // binding stays anonymous: no local claims it, and the
                    // discard gate below must not retire it before the store
                    // runs as the call's immediate continuation.
                    structural_result = Some((result, None));
                    None
                }
            },
            StatementNode::LocalData(local) => match local_data::plan(
                local_data::Planner {
                    program,
                    facts,
                    scalar_callees,
                    shapes: &mut *shapes,
                    machine,
                    state,
                    structural_parameters: &mut *structural_parameters,
                    entry_claims,
                    trivial_affine_locals,
                    trace,
                    binders: &binders,
                    erased_locals: &erased_locals,
                    operations: &mut operations,
                    scalar_count: &mut scalar_count,
                    structural_count: &mut structural_count,
                    structural_local_symbols: &mut structural_local_symbols,
                    windows: &mut windows,
                    array_bindings: &mut array_bindings,
                    atomic_result: &mut atomic_result,
                    structural_results: &mut structural_results,
                },
                index,
                statement_index,
                local,
            )? {
                local_data::LocalPlan::Planned => continue,
                local_data::LocalPlan::ScalarCall(result) => Some(result),
                local_data::LocalPlan::StructuralCall(result) => {
                    structural_result = Some((result, Some(local.symbol)));
                    None
                }
            },
            StatementNode::Call(call) if call.discards_result => {
                trace.phase("statement sequence: call: discarded result type");
                trace.statement(Some(statement_index));
                let result_type =
                    crate::flow::call_target_return_type(program, call.target_symbol)?;
                if let Some(primitive_type) = program.primitive_type_reference(result_type) {
                    // Discarding the value does not discard the invocation or
                    // change its result signature. Keep the ordinary SSA result
                    // without introducing a source local.
                    let result = CheckedUnitScalarResultBindingPlan {
                        statement_index,
                        binding_ordinal: u32::try_from(scalar_count).ok()?,
                        primitive_type,
                    };
                    Some(result)
                } else if !is_unit(program, result_type) {
                    let mut result =
                        checked_structural_result_type(program, shapes, result_type, &binders)?;
                    result.statement_index = statement_index;
                    // No source local exists for an explicit discard. The call
                    // still produces its own typed result and disposal debt.
                    structural_result = Some((result, None));
                    None
                } else {
                    None
                }
            }
            StatementNode::Call(_) => None,
            StatementNode::Expression(_) if completes_machine
                && program.primitive_type_reference(state.return_type).is_some() =>
            {
                let result = CheckedUnitScalarResultBindingPlan {
                    statement_index,
                    binding_ordinal: u32::try_from(scalar_count).ok()?,
                    primitive_type: program.primitive_type_reference(state.return_type)?,
                };
                scalar_count = scalar_count.checked_add(1)?;
                returned_scalar_call = Some(result);
                Some(result)
            }
            StatementNode::Expression(_) if completes_machine => {
                let mut result = checked_structural_result_type(program, shapes, state.return_type, &binders)?;
                result.statement_index = statement_index;
                structural_result = Some((result, None));
                None
            }
            StatementNode::Expression(_)
                if call_occurrences::tail_call(program, state, index).is_some() =>
            {
                None
            }
            // A Unit-result call in expression position mid-body composes like
            // a call statement: its erased result binds nothing, and the
            // shared call roster below owns its operands and receiver.
            StatementNode::Expression(_)
                if call_occurrences::unit_statement_call(program, machine, state, index)
                    .is_some() =>
            {
                None
            }
            StatementNode::Transition(_)
                if call_occurrences::terminal_named_call(program, machine, state, index).is_some() =>
            {
                if let Some(primitive_type) =
                    program.primitive_type_reference(state.return_type)
                {
                    let result = CheckedUnitScalarResultBindingPlan {
                        statement_index,
                        binding_ordinal: u32::try_from(scalar_count).ok()?,
                        primitive_type,
                    };
                    scalar_count = scalar_count.checked_add(1)?;
                    returned_scalar_call = Some(result);
                    Some(result)
                } else if !is_unit(program, state.return_type) {
                    let mut result = checked_structural_result_type(
                        program,
                        shapes,
                        state.return_type,
                        &binders,
                    )?;
                    result.statement_index = statement_index;
                    structural_result = Some((result, None));
                    None
                } else {
                    None
                }
            }
            _ => return None,
        };
        statement_call::plan(
            statement_call::Planner {
                program,
                facts,
                scalar_callees,
                shapes: &mut *shapes,
                machine,
                state,
                structural_parameters: &mut *structural_parameters,
                entry_claims,
                trivial_affine_locals,
                calls,
                trace,
                operations: &mut operations,
                structural_count: &mut structural_count,
                call_count: &mut call_count,
                structural_local_symbols: &mut structural_local_symbols,
                array_bindings: &mut array_bindings,
                returned_call: &mut returned_call,
                structural_results: &mut structural_results,
            },
            statement,
            index,
            statement_index,
            statement_call::StatementCall {
                completes_machine,
                result,
                structural_result,
                result_stores: call_result_stores,
                structural_store_bindings,
            },
        )?;
    }
    trace.statement(None);
    completion::complete(completion::Completion {
        program,
        facts,
        scalar_callees,
        shapes,
        machine,
        state,
        structural_parameters,
        entry_claims,
        calls,
        trivial_affine_locals,
        trace,
        binders: &binders,
        scalar_control,
        operations,
        scalar_count,
        structural_count,
        structural_local_symbols,
        windows,
        array_bindings,
        returned_call,
        returned_scalar_call,
        structural_results,
        call_count,
    })
}

/// Share scalar exit discovery while leaving operation storage with its sequence.
/// The retained tail, when the state's statements end in control that returns
/// the machine result — independent of what the result's carrier is. The
/// scalar-control plan wraps the same terminator with its primitive carrier;
/// structural result machines ask the predicate directly so a transition-only
/// body counts as producing its result at the terminator.
pub(in crate::execution::terminal_unit) fn returning_terminator(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
) -> Option<(checked_trees::CheckedScalarStateTerminator, usize)> {
    let statements = program.statement_table.statements(state.statement_nodes);
    let prefix_count = statements
        .iter()
        .take_while(|statement| {
            matches!(
                statement,
                StatementNode::LocalData(_) | StatementNode::Assignment(_) | StatementNode::Call(_)
            )
        })
        .count();
    let mut guarded = facts
        .flow
        .terminal_scalar_graphs
        .guarded_tails
        .iter()
        .filter(|tail| tail.state == state.symbol);
    // Keep the existing scalar binary route when it already describes this
    // completion; the shared roster also serves structural return consumers.
    let binary = crate::execution::terminal_scalar::checked_terminator(
        program,
        machine,
        state,
        &facts.values.scalar_expressions,
        prefix_count,
    );
    let terminator =
        if let Some(terminator @ checked_trees::CheckedScalarStateTerminator::Conditional { .. }) =
            binary
        {
            terminator
        } else if let Some(tail) = guarded.next() {
            if guarded.next().is_some() {
                return None;
            }
            let arms = facts
                .flow
                .terminal_scalar_graphs
                .guarded_exits
                .span(tail.arms)?;
            if arms.first()?.guard_statement_ordinal as usize != prefix_count
                || !arms.iter().all(|arm| {
                    matches!(
                        arm.destination,
                        checked_trees::CheckedScalarBranchDestination::Return { .. }
                    )
                })
                || tail.fallback.as_ref().is_some_and(|fallback| {
                    !matches!(
                        fallback,
                        checked_trees::CheckedScalarBranchDestination::Return { .. }
                    )
                })
            {
                return None;
            }
            checked_trees::CheckedScalarStateTerminator::Guarded {
                arms: tail.arms,
                fallback: tail.fallback.clone(),
            }
        } else {
            crate::execution::terminal_scalar::checked_terminator(
                program,
                machine,
                state,
                &facts.values.scalar_expressions,
                prefix_count,
            )?
        };
    let returning_tail = match &terminator {
        // Final expressions already belong to the scalar-result route, which
        // also supports ownership frontiers outside scalar-control admission.
        // Explicit unconditional transitions use the same retained return
        // coordinate as guarded control, without inventing a Boolean guard.
        checked_trees::CheckedScalarStateTerminator::Return { statement_ordinal } => matches!(
            statements.get(*statement_ordinal as usize),
            Some(StatementNode::Transition(_))
        ),
        checked_trees::CheckedScalarStateTerminator::Guarded { .. }
        | checked_trees::CheckedScalarStateTerminator::Conditional {
            when_true: checked_trees::CheckedScalarBranchDestination::Return { .. },
            when_false: checked_trees::CheckedScalarBranchDestination::Return { .. },
            ..
        } => true,
        // A crash exit is a scalar tail the route already owns; the state
        // produces no value and needs no completion.
        checked_trees::CheckedScalarStateTerminator::Crash { .. } => true,
        _ => false,
    };
    returning_tail.then_some((terminator, prefix_count))
}

pub(in crate::execution::terminal_unit) fn scalar_control(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
) -> Option<(checked_trees::CheckedUnitScalarControlPlan, usize)> {
    let primitive_type = program.primitive_type_reference(state.return_type)?;
    let (terminator, prefix_count) = returning_terminator(program, facts, machine, state)?;
    Some((
        checked_trees::CheckedUnitScalarControlPlan {
            primitive_type,
            terminator,
        },
        prefix_count,
    ))
}

fn scalar_computation_local_at(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    statement_index: u32,
    binding_ordinal: u32,
    local: &typed_trees::statement::TableLocalData,
    trace: &LocalConstructionTrace,
) -> Option<(
    CheckedUnitScalarResultBindingPlan,
    checked_trees::CheckedCallScalarArgument,
)> {
    let role = CheckedScalarExpressionRole::LocalInitializer { binding_ordinal };
    let computations = &facts.values.scalar_computations;
    let mut roots = computations
        .roots
        .iter()
        .map(|(_, root)| root)
        .filter(|root| {
            root.state == state.symbol
                && root.statement_ordinal == statement_index
                && root.role == role
        });
    let Some(root) = roots.next() else {
        trace.phase("statement sequence: local data: scalar local: pure initializer");
        trace.statement(Some(statement_index));
        let (result, value) = scalar_expression_local_at(
            program,
            facts,
            state,
            statement_index,
            binding_ordinal,
            local,
        )?;
        return Some((
            result,
            checked_trees::CheckedCallScalarArgument::Pure(value),
        ));
    };
    trace.phase("statement sequence: local data: scalar local: computation initializer");
    trace.statement(Some(statement_index));
    let primitive_type = program.primitive_type_reference(local.type_reference)?;
    if roots.next().is_some()
        || root.machine != machine
        || local.is_mutable
        || !computations.nodes.is_valid(root.root)
        || computations.nodes.get(root.root).authored_root != local.initial_value
        || computations.nodes.get(root.root).primitive_type != primitive_type
        || facts
            .values
            .scalar_expressions
            .expression_at(state.symbol, statement_index, role)
            .is_some()
    {
        return None;
    }
    Some((
        CheckedUnitScalarResultBindingPlan {
            statement_index,
            binding_ordinal,
            primitive_type,
        },
        checked_trees::CheckedCallScalarArgument::Computation(root.root),
    ))
}
