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
    CheckedUnitPartialAffineDiscardPlan, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralPathSegment,
    PermissionProvenance,
};
use super::{
    CheckFacts, CheckedScalarExpression, CheckedScalarExpressionRole, CheckedStructuralAccess,
    CheckedStructuralScalarParameterPlan, CheckedUnitEffectOperationPlan,
    CheckedUnitScalarResultBindingPlan, CheckedUnitStructuralParameterPlan,
    CheckedUnitStructuralResultBindingPlan, ExpressionNode, Multiplicity, PermissionEventSource,
    StatementNode, SymbolHandle, TypeReferenceNode, TypedTrees,
};
use crate::execution::terminal_unit::ScalarCalleePlans;
use crate::execution::terminal_unit::calls::{ExpectedCallValueResult, build_call_operation};
use crate::execution::terminal_unit::control::LocalConstructionTrace;
use crate::execution::terminal_unit::control::call_occurrences;
use crate::execution::terminal_unit::control::call_results::bind_scalar_call_result;
use crate::execution::terminal_unit::control::call_results::bind_structural_call_result;
use crate::execution::terminal_unit::control::call_results::checked_constructed_result_type;
use crate::execution::terminal_unit::control::call_results::checked_structural_result_type;
use crate::execution::terminal_unit::control::structural_operands;
use crate::execution::terminal_unit::returns::checked_boolean_contains_short_circuit;
use crate::execution::terminal_unit::scalar_locals::scalar_expression_local_at;
use crate::execution::terminal_unit::types::{
    ShapeCollector, base_type_identity, is_unit, machine_binders, parameter_qualifications,
};

use checked_trees::CheckedUnitStructuralReturnPlan;

mod assignments;
pub(in crate::execution::terminal_unit) mod bounded_byte_values;
mod completion;
mod local_data;
mod statement_call;
mod storage_initializer;

/// Completion forwards a whole parameter only when its exact output contract
/// and, for linear values, checked input-origin claim outcome agree.
fn returned_parameter(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    parameters: &[CheckedUnitStructuralParameterPlan],
    entry_claims: &[CheckedUnitEntryClaimPlan],
) -> Option<CheckedUnitStructuralReturnPlan> {
    let StatementNode::Expression(expression) = program
        .statement_table
        .statements(state.statement_nodes)
        .last()?
    else {
        return None;
    };
    let ExpressionNode::Name(path) = program.expression_table.expression(*expression) else {
        return None;
    };
    if path.head_symbol != path.symbol
        || program
            .expression_table
            .name_path_members(path.members)
            .len()
            != 1
    {
        return None;
    }
    let source_parameters = program.state_parameters(state);
    let (parameter_index, parameter) = parameters.iter().enumerate().find(|(_, parameter)| {
        source_parameters
            .get(parameter.position as usize)
            .is_some_and(|source| source.symbol == path.symbol)
    })?;
    let source = source_parameters.get(parameter.position as usize)?;
    if let Some(source) = super::super::reference_results::initializer_source(
        program,
        state,
        *expression,
        state.return_type,
    ) {
        if super::super::reference_results::source_parameter(program, state)?
            != parameter.position as usize
            || !parameter.qualifications.is_empty()
        {
            return None;
        }
        return Some(CheckedUnitStructuralReturnPlan {
            source: source.source.clone(),
            type_identity: program
                .normalized_type_identity(state.return_type)
                .into_string(),
            multiplicity: Multiplicity::Affine,
            reference_sources: vec![checked_trees::CheckedReferenceResultSourcePlan {
                path: Vec::new(),
                source,
            }],
        });
    }
    // A `&'a [u8]` whole-view return forwards the view parameter's own borrowed
    // storage: the same `Parameter` result source a whole owned forward mints,
    // carrying the shared-borrow access the view parameter was admitted with.
    // No reference is established — the loan passes through unchanged.
    if crate::execution::terminal_unit::types::borrowed_slice_view(program, state.return_type) {
        if parameter.access != CheckedStructuralAccess::SharedBorrow
            || !parameter.qualifications.is_empty()
            || program.normalized_type_identity(source.type_reference)
                != program.normalized_type_identity(state.return_type)
        {
            return None;
        }
        return Some(CheckedUnitStructuralReturnPlan {
            source: CheckedUnitStructuralArgumentSourcePlan::Parameter {
                parameter_index: u32::try_from(parameter_index).ok()?,
            },
            type_identity: parameter.type_identity.clone(),
            multiplicity: parameter.multiplicity,
            reference_sources: Vec::new(),
        });
    }
    if parameter.access != CheckedStructuralAccess::Owned
        || parameter.multiplicity != program.type_multiplicity(state.return_type)
        || program.normalized_type_identity(source.type_reference)
            != program.normalized_type_identity(state.return_type)
        || parameter.qualifications
            != validation::structural_result_qualifications(program, state.return_type).ok()?
    {
        return None;
    }
    if parameter.multiplicity == Multiplicity::Linear {
        let returned = validation::reconstruct_structural_parameter_return_claims(
            program,
            facts,
            machine.symbol,
            state.symbol,
            source.symbol,
        )
        .ok()?;
        let mut input = entry_claims
            .iter()
            .filter(|claim| claim.parameter_index as usize == parameter_index)
            .map(|claim| (claim.claim_identity, claim.path.clone()))
            .collect::<Vec<_>>();
        input.sort_by(|left, right| left.1.cmp(&right.1));
        // Completion and ordinary calls share one input-origin correspondence;
        // matching paths alone cannot replace the exact checked claim identities.
        if input != returned {
            return None;
        }
    }
    Some(CheckedUnitStructuralReturnPlan {
        source: CheckedUnitStructuralArgumentSourcePlan::Parameter {
            parameter_index: u32::try_from(parameter_index).ok()?,
        },
        type_identity: parameter.type_identity.clone(),
        multiplicity: parameter.multiplicity,
        reference_sources: if validation::reference_result_custody::is_reference_record(
            program,
            state.return_type,
        ) {
            validation::reference_result_custody::returned_record_sources(program, state)?
        } else {
            Vec::new()
        },
    })
}

/// A `value.body`-style completion selects one borrowed carrier leaf of an
/// owned record formal rather than forwarding the whole parameter. The leaf
/// argument already carries its exact ingress path; the shared
/// reference-result block emits its establishment and rewrites the result
/// source to that binding. The establishment consumes the carrier whole, so
/// `source_leaf` only admits a record whose declared reference roster is
/// exactly the selected leaf.
fn returned_reference_leaf(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    parameters: &[CheckedUnitStructuralParameterPlan],
) -> Option<CheckedUnitStructuralReturnPlan> {
    let (_, source) = validation::reference_result_custody::source_leaf(program, state)?;
    let CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } = source.source
    else {
        return None;
    };
    let parameter = parameters.get(parameter_index as usize)?;
    if parameter.access != CheckedStructuralAccess::Owned
        || parameter.multiplicity != Multiplicity::Affine
        || !parameter.qualifications.is_empty()
        || parameter.fused_service_erasure.is_some()
    {
        return None;
    }
    Some(CheckedUnitStructuralReturnPlan {
        source: source.source.clone(),
        type_identity: program
            .normalized_type_identity(state.return_type)
            .into_string(),
        multiplicity: Multiplicity::Affine,
        reference_sources: vec![checked_trees::CheckedReferenceResultSourcePlan {
            path: Vec::new(),
            source,
        }],
    })
}

/// A `&'a V` named-result completion borrows the referent it names from one
/// readable structural carrier, either directly or through a field. The borrow is the
/// result — there are no subslice endpoints and no separate slice length.
/// Unlike `returned_reference_leaf`'s `&mut` record rosters the carrier may
/// itself be borrowed. A mutable input can lend shared observation; its
/// exclusivity does not require the output to remain mutable.
fn returned_named_view(
    program: &TypedTrees,
    shapes: &mut ShapeCollector<'_>,
    state: &typed_trees::state::State,
    parameters: &[CheckedUnitStructuralParameterPlan],
    binders: &[(SymbolHandle, String)],
) -> Option<CheckedUnitStructuralReturnPlan> {
    if !crate::execution::terminal_unit::types::borrowed_named_view(program, state.return_type) {
        return None;
    }
    let statements = program.statement_table.statements(state.statement_nodes);
    let statement_index = statements.len().checked_sub(1)?;
    // A bare `transition { _ -> &place }` completes the state exactly like a
    // trailing `(&place)` expression: its `Value` target is the authored
    // return expression.
    let expression = match statements.last()? {
        StatementNode::Expression(expression) => *expression,
        StatementNode::Transition(transition)
            if transition.exit == typed_trees::statement::TransitionExit::Ordinary =>
        {
            let typed_trees::statement::TransitionTargetNode::Value(expression) =
                program.statement_table.transition_target(transition.target)
            else {
                return None;
            };
            *expression
        }
        _ => return None,
    };
    let place = crate::flow::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        statement_index,
        expression,
    )?;
    let facts::PlaceRoot::Symbol(symbol) = place.root else {
        return None;
    };
    let authored = program.state_parameters(state);
    let position = authored
        .iter()
        .position(|parameter| parameter.symbol == symbol)?;
    let carrier = &authored[position];
    if carrier.is_self
        || !matches!(
            crate::execution::terminal_unit::types::structural_access_for_type_reference(
                program,
                carrier.type_reference,
            ),
            Some(
                CheckedStructuralAccess::SharedBorrow
                    | CheckedStructuralAccess::MutableBorrow
                    | CheckedStructuralAccess::Owned
            )
        )
    {
        return None;
    }
    let parameter_index = parameters
        .iter()
        .position(|parameter| parameter.position as usize == position)?;
    let (storage, path) = crate::execution::terminal_unit::calls::projected_argument_path(
        program,
        state.symbol,
        statement_index,
        &place,
    )?;
    let referee = crate::execution::terminal_unit::types::borrowed_view_referent(
        program,
        state.return_type,
        language_semantics::ReferenceAccess::Shared,
    )?;
    // The stored value the projection bottoms out in must be exactly the
    // result's `&'a V` or its referent `V`.
    if program.normalized_type_identity(storage)
        != program.normalized_type_identity(state.return_type)
        && program.normalized_type_identity(storage) != program.normalized_type_identity(referee)
    {
        return None;
    }
    // Only a stored reference adds an indirection. A projected ordinary
    // value already names the result's referent; a whole parameter borrow
    // likewise already names its referent storage.
    let mut path = path;
    if !path.is_empty() && validation::reference_shell(program, storage).is_some() {
        path.push(checked_trees::CheckedUnitStructuralPathSegment::Referent);
    }
    let source = CheckedUnitStructuralArgumentPlan {
        source: CheckedUnitStructuralArgumentSourcePlan::Parameter {
            parameter_index: u32::try_from(parameter_index).ok()?,
        },
        path,
        type_identity: shapes.add_type(referee, binders, &[])?,
        access: CheckedStructuralAccess::SharedBorrow,
    };
    Some(CheckedUnitStructuralReturnPlan {
        source: source.source.clone(),
        // The result spells its own `&'a V` type reference and loans the
        // referent's storage affinely — the same custody a `&mut` primitive
        // reference result carries.
        type_identity: shapes.add_view_type(
            state.return_type,
            binders,
            language_semantics::ReferenceAccess::Shared,
        )?,
        multiplicity: Multiplicity::Affine,
        reference_sources: vec![checked_trees::CheckedReferenceResultSourcePlan {
            path: Vec::new(),
            source,
        }],
    })
}

/// An authored `[start..end]` member or whole-parameter subslice completing a
/// borrowed `&[u8]`/`&[T]` view return: the last statement's own indexed
/// expression uses the ordinary checked subslice producer, as a local or call
/// operand does. Its descriptor carries backing and extent by value; Terminal
/// independently reconstructs the original backing through the producer.
fn returned_subslice(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    parameters: &[CheckedUnitStructuralParameterPlan],
    count: &mut usize,
) -> Option<(
    CheckedUnitStructuralReturnPlan,
    CheckedUnitEffectOperationPlan,
)> {
    if !crate::execution::terminal_unit::types::borrowed_slice_view(program, state.return_type) {
        return None;
    }
    let statements = program.statement_table.statements(state.statement_nodes);
    let StatementNode::Expression(expression) = statements.last()? else {
        return None;
    };
    let statement_ordinal = u32::try_from(statements.len() - 1).ok()?;
    let operation = super::super::state_graph::returns::view_result_operation(
        program,
        facts,
        shapes,
        machine,
        state,
        parameters,
        count,
        statement_ordinal,
        *expression,
    )?;
    let CheckedUnitEffectOperationPlan::EstablishViewSubslice { result, .. } = &operation else {
        return None;
    };
    Some((result.clone().into(), operation))
}

/// A `&mut place` tail completing an exclusive `&mut R`/`&mut [T]` view
/// return delegates to the state-graph's mutable-view producer: the denoted
/// place resolves through the state's own minted `&mut` loans until it roots
/// at a `&mut` parameter (or a `self` member the loan publishes as its own
/// root), and the projected path — runtime selectors included — spells the
/// result's reference source. The tail is the expression or unconditional
/// ordinary value-transition the state's exit spells, matching the shape the
/// terminator route reads.
fn returned_mutable_view(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    parameters: &[CheckedUnitStructuralParameterPlan],
    count: &mut usize,
) -> Option<(
    CheckedUnitStructuralReturnPlan,
    CheckedUnitEffectOperationPlan,
)> {
    if !crate::execution::terminal_unit::types::mutable_borrowed_view(program, state.return_type) {
        return None;
    }
    let expression = validation::reference_result_custody::tail_return_expression(program, state)?;
    let statement_ordinal = u32::try_from(
        program
            .statement_table
            .statements(state.statement_nodes)
            .len()
            .checked_sub(1)?,
    )
    .ok()?;
    let operation = super::super::state_graph::returns::mutable_view_result_operation(
        program,
        facts,
        shapes,
        machine,
        state,
        parameters,
        count,
        statement_ordinal,
        expression,
    )?;
    let CheckedUnitEffectOperationPlan::EstablishReference { result, source } = operation else {
        return None;
    };
    // The minted `&mut` view ends exactly once at the result, so the binding
    // spells Affine rather than the exclusive borrow's Unrestricted type
    // multiplicity. This permission transfer remains affine.
    let result = CheckedUnitStructuralResultBindingPlan {
        multiplicity: Multiplicity::Affine,
        ..result
    };
    Some((
        CheckedUnitStructuralReturnPlan {
            source: CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                binding_ordinal: result.binding_ordinal,
            },
            type_identity: result.type_identity.clone(),
            multiplicity: result.multiplicity,
            reference_sources: vec![checked_trees::CheckedReferenceResultSourcePlan {
                path: Vec::new(),
                source: source.clone(),
            }],
        },
        CheckedUnitEffectOperationPlan::EstablishReference { result, source },
    ))
}

/// A `("bytes")` tail completing a borrowed `&[u8]` view return: the literal
/// names its own immutable storage, so the result source is the spelled byte
/// sequence itself — the same source shape a boundary call argument carries.
/// No parameter, local, or result custody is involved and no reference source
/// is established: the verifier's literal place kind is self-describing.
pub(super) fn returned_byte_sequence_literal(
    program: &TypedTrees,
    state: &typed_trees::state::State,
) -> Option<CheckedUnitStructuralReturnPlan> {
    if !crate::execution::terminal_unit::types::borrowed_slice_view(program, state.return_type) {
        return None;
    }
    let statements = program.statement_table.statements(state.statement_nodes);
    let StatementNode::Expression(expression) = statements.last()? else {
        return None;
    };
    let ExpressionNode::String(bytes) = program.expression_table.expression(*expression) else {
        return None;
    };
    // Peeling a shared descriptor's storage identity changes neither its
    // multiplicity nor the lifetime of the immutable literal backing.
    let (return_type, multiplicity) =
        match validation::reference_result_custody::shared_place_borrowed_parts(
            program,
            state.return_type,
        ) {
            Some((referee, _)) => (
                referee,
                validation::reference_result_custody::result_multiplicity(
                    program,
                    state.return_type,
                ),
            ),
            None => (
                state.return_type,
                program.type_multiplicity(state.return_type),
            ),
        };
    Some(CheckedUnitStructuralReturnPlan {
        source: CheckedUnitStructuralArgumentSourcePlan::ByteSequenceLiteral {
            bytes: bytes.to_vec(),
        },
        type_identity: program.normalized_type_identity(return_type).into_string(),
        multiplicity,
        reference_sources: Vec::new(),
    })
}
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

/// Locals that are checked borrow carriers rather than storage bindings: an
/// exactly captured exclusive loan alias, or the parent/child roster a
/// reborrow-restored-call certificate names. Their declarations compose
/// through the sequence without an operation; each use's planner joins the
/// captured referent's evidence instead.
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
    // A shared loan of a bounded byte owner keeps its domain qualifications
    // on the declaration: the loan freezes its referent, so a scalar
    // completion reading it needs no qualification transport.
    let shared_bounded_owner = |parameter: &CheckedUnitStructuralParameterPlan| {
        parameter.access == CheckedStructuralAccess::SharedBorrow
            && !parameter.is_self
            && parameter.projected_qualifications.is_empty()
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
    if scalar_control.is_some()
        && (!entry_claims.is_empty()
            || !trivial_affine_locals.is_empty()
            || structural_parameters.iter().any(|parameter| {
                parameter.multiplicity != Multiplicity::Unrestricted
                    || (!parameter.qualifications.is_empty() && !shared_bounded_owner(parameter))
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
        append_reference_releases(program, facts, machine.symbol, state.symbol, statement_index, &mut operations)?;
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
        let mut displaced_discard = None;
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
                    call_result_stores.extend(replacement.move_out);
                    call_result_stores.push(replacement.store);
                    displaced_discard = replacement.displaced_discard;
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
                displaced_discard,
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

fn append_reference_releases(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement_index: u32,
    operations: &mut Vec<CheckedUnitEffectOperationPlan>,
) -> Option<()> {
    let mut pending = Vec::new();
    for operation in operations.iter() {
        // Only an ordinary structural call carries a custody record.
        if let Some(call) = operation.call()
            && let Some(custody) = call.structural_custody()
            && let Some(result) = call.structural_result()
            && custody.reference_loan.is_valid()
        {
            let loan = custody.reference_loan;
            let boundary = super::super::reference_results::release_boundary(
                program, facts, machine, state, loan,
            )?;
            if boundary == statement_index && !operations.iter().any(|operation| {
                matches!(operation, CheckedUnitEffectOperationPlan::ReleaseReference { loan: released, .. } if *released == loan)
            }) {
                pending.push(CheckedUnitEffectOperationPlan::ReleaseReference {
                    statement_index, binding_ordinal: result.binding_ordinal, loan,
                });
            }
        }
    }
    operations.extend(pending);
    Some(())
}

// Loans released by the state's natural exit carry their weakening at a
// boundary the per-statement loop never visits (the transition statement, or
// one past the last ordinary statement). The completion still needs the
// `ReleaseReference` op so the result-binding cleanup sees the disposal.
fn append_exit_reference_releases(
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    operations: &mut Vec<CheckedUnitEffectOperationPlan>,
) -> Option<()> {
    let mut pending = Vec::new();
    for operation in operations.iter() {
        // Only an ordinary structural call carries a custody record.
        if let Some(call) = operation.call()
            && let Some(custody) = call.structural_custody()
            && let Some(result) = call.structural_result()
            && custody.reference_loan.is_valid()
        {
            let loan = custody.reference_loan;
            let boundary =
                super::super::reference_results::release_statement(facts, machine, state, loan)?;
            if !operations.iter().any(|operation| {
                matches!(operation, CheckedUnitEffectOperationPlan::ReleaseReference { loan: released, .. } if *released == loan)
            }) && !pending.iter().any(|operation: &(u32, CheckedUnitEffectOperationPlan)| {
                matches!(operation, (_, CheckedUnitEffectOperationPlan::ReleaseReference { loan: released, .. }) if *released == loan)
            }) {
                pending.push((boundary, CheckedUnitEffectOperationPlan::ReleaseReference {
                    statement_index: boundary,
                    binding_ordinal: result.binding_ordinal,
                    loan,
                }));
            }
        }
    }
    pending.sort_by_key(|(boundary, _)| *boundary);
    operations.extend(pending.into_iter().map(|(_, operation)| operation));
    Some(())
}

#[allow(clippy::too_many_arguments)]
fn append_call_cleanup(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    operations: &mut Vec<CheckedUnitEffectOperationPlan>,
    statement_index: u32,
    results: &[(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)],
) -> Option<()> {
    // Argument temporaries die at the ordinary call's continuation, independently
    // of its result type. The exact operand bindings below select their owners;
    // boundary calls have no local continuation on which to schedule this cleanup.
    let Some(call) = operations
        .last()
        .and_then(CheckedUnitEffectOperationPlan::call)
        .filter(|call| !call.is_boundary())
    else {
        return Some(());
    };
    let coordinate = call.coordinate;
    let structural_arguments = &call.structural_arguments;
    let mut discards = Vec::new();
    for (result, root) in results.iter().rev() {
        let reference_record = matches!(root, facts::PlaceRoot::Symbol(_))
            && super::super::reference_results::local_record_loans(
                program,
                facts,
                machine,
                state,
                result.statement_index,
            )
            .is_some_and(|loans| {
                !loans.is_empty()
                    && loans.iter().all(|(_, loan)| {
                        super::super::reference_results::release_statement(
                            facts,
                            machine,
                            state.symbol,
                            *loan,
                        ) == statement_index.checked_add(1)
                    })
            })
            && structural_arguments.iter().any(|argument| {
                argument.source_structural_result_binding_ordinal() == Some(result.binding_ordinal)
                    && argument.path.last() == Some(&CheckedUnitStructuralPathSegment::Referent)
            });
        if !reference_record
            && (!matches!(root, facts::PlaceRoot::Expression(_))
                || result.statement_index != statement_index
                || !structural_arguments.iter().any(|argument| {
                    argument.source_structural_result_binding_ordinal()
                        == Some(result.binding_ordinal)
                        && argument.access == CheckedStructuralAccess::SharedBorrow
                }))
        {
            continue;
        }
        discards.push(checked_trees::CheckedUnitPartialAffineDiscardPlan {
            source: checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                binding_ordinal: result.binding_ordinal,
            },
            path: Vec::new(),
            type_identity: result.type_identity.clone(),
        });
    }
    if discards.is_empty() {
        return Some(());
    }
    for discard in &discards {
        let checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
            binding_ordinal,
        } = discard.source
        else {
            return None;
        };
        let (_, discard_result_on_return) = operations.iter_mut().find_map(|operation| {
            operation
                .discardable_structural_result_mut()
                .filter(|(result, _)| result.binding_ordinal == binding_ordinal)
        })?;
        if !*discard_result_on_return {
            return None;
        }
        *discard_result_on_return = false;
    }
    operations.push(CheckedUnitEffectOperationPlan::CallContinuationCleanup {
        coordinate,
        affine_discards: discards,
    });
    Some(())
}

/// A selected continuation owns both the chosen result and the conditional
/// complement until the actual death edge. Original producers must not also
/// publish unconditional return drops for those same owners.
fn retain_selected_sources(
    facts: &CheckFacts,
    state: SymbolHandle,
    statement: u32,
    sources: &[(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)],
    operations: &mut [CheckedUnitEffectOperationPlan],
) -> Option<()> {
    let Some((_, receipt)) = facts.flow.ownership.owned_selection_at(state, statement) else {
        return Some(());
    };
    for source in facts
        .flow
        .ownership
        .selection_sources
        .span(receipt.sources)?
    {
        // A state-entry source (an owned parameter) has no producing
        // operation to unmark: its residual custody already rides the
        // receipt's selected edges instead of an unconditional return drop.
        if matches!(
            source.provenance,
            PermissionProvenance::Established {
                source: PermissionEventSource::StateEntry,
                ..
            }
        ) {
            continue;
        }
        let (binding, _) = sources
            .iter()
            .find(|(_, root)| *root == facts::PlaceRoot::Symbol(source.symbol))?;
        let mut producers = operations.iter_mut().filter_map(|operation| {
            operation
                .discardable_structural_result_mut()
                .filter(|(result, _)| result.binding_ordinal == binding.binding_ordinal)
        });
        let (result, discard) = producers.next()?;
        if producers.next().is_some()
            || result.multiplicity != Multiplicity::Affine
            || result.statement_index != source.statement_ordinal
            || !*discard
        {
            return None;
        }
        *discard = false;
    }
    Some(())
}

/// Discharge only producers owned by this sequence. Call-owned operand
/// preparation has its own result namespace segment; its enclosing sequence
/// owns earlier locals. Each owner updates its own disposal debt, while source
/// custody independently rejoins every operand across that boundary.
pub(super) fn consume_retained_results(
    operations: &mut [CheckedUnitEffectOperationPlan],
    consumer: &CheckedUnitEffectOperationPlan,
) -> Option<()> {
    let Some(call) = consumer.call() else {
        return Some(());
    };
    for argument in &call.structural_arguments {
        let Some(binding) = argument.source_structural_result_binding_ordinal() else {
            continue;
        };
        if operations.iter().any(|operation| {
            operation
                .structural_result_binding()
                .is_some_and(|result| result.binding_ordinal == binding)
        }) {
            consume_result(
                operations,
                binding,
                argument.access,
                !argument.path.is_empty(),
            )?;
        }
    }
    Some(())
}

pub(super) fn consume_value_call_sources(
    facts: &CheckFacts,
    results: &[(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)],
    operations: &mut [CheckedUnitEffectOperationPlan],
    calls: &[checked_trees::CheckedStructuralValueCall],
) -> Option<()> {
    for call in calls {
        for operation in call
            .operands()
            .iter()
            .chain(std::iter::once(call.operation()))
            .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
        {
            if let CheckedUnitEffectOperationPlan::EstablishStructuralValue { value, .. } =
                operation
            {
                consume_value_places(facts, *value, results, operations)?;
            }
            consume_retained_results(operations, operation)?;
        }
    }
    Some(())
}

pub(in crate::execution::terminal_unit) fn consume_results(
    operations: &mut [CheckedUnitEffectOperationPlan],
    consumer: &CheckedUnitEffectOperationPlan,
) -> Option<()> {
    if let Some(call) = consumer.call() {
        for (binding_ordinal, access, projected) in
            call.structural_arguments.iter().filter_map(|argument| {
                argument
                    .source_structural_result_binding_ordinal()
                    .map(|ordinal| (ordinal, argument.access, !argument.path.is_empty()))
            })
        {
            consume_result(operations, binding_ordinal, access, projected)?;
        }
    }
    // A whole-result field store consumes the minted binding the same way a
    // call argument does: Owned access retires the producer's disposal debt.
    if let CheckedUnitEffectOperationPlan::StoreStructuralField { value, .. } = consumer
        && let Some(binding_ordinal) = value.source_structural_result_binding_ordinal()
    {
        consume_result(
            operations,
            binding_ordinal,
            value.access,
            !value.path.is_empty(),
        )?;
    }
    Some(())
}

fn consume_result(
    operations: &mut [CheckedUnitEffectOperationPlan],
    binding_ordinal: u32,
    access: CheckedStructuralAccess,
    projected: bool,
) -> Option<()> {
    // Every structural producer except an established reference.
    let mut producers = operations.iter_mut().filter(|operation| {
        !matches!(
            operation,
            CheckedUnitEffectOperationPlan::EstablishReference { .. }
        ) && operation
            .structural_result_binding()
            .is_some_and(|result| result.binding_ordinal == binding_ordinal)
    });
    let producer = producers.next()?;
    if producers.next().is_some() {
        return None;
    }
    // A detached field owns its moved custody, not a newly minted automatic
    // return discard. The ordinary call/store may consume that whole value;
    // source ownership and independent replay still account for every use.
    // Its original place's restoration debt remains in OpenWindows.
    if matches!(
        producer,
        CheckedUnitEffectOperationPlan::MoveStructuralField { .. }
    ) {
        return (access != CheckedStructuralAccess::Owned || !projected).then_some(());
    }
    if let Some(custody) = producer
        .call()
        .and_then(checked_trees::CheckedUnitCallPlan::structural_custody)
        && custody.reference_loan.is_valid()
    {
        // A borrowed result is consumed by a borrow of matching access:
        // `&mut` results only by a projected `&mut` argument (a re-borrow —
        // the whole exclusive loan cannot move), while a `&` result's loan
        // is shareable, so a `&` argument admits the forward projected or
        // whole. The forward names the shelled referent, not a copy.
        return (access == CheckedStructuralAccess::SharedBorrow
            || (projected && access == CheckedStructuralAccess::MutableBorrow))
            .then_some(());
    }
    // Unrestricted whole-value arguments copy their payload. They retain
    // a live producer without acquiring an affine disposal obligation. A
    // boundary structural result is not such a copy.
    let unrestricted_copy = match &*producer {
        CheckedUnitEffectOperationPlan::Call(call) => {
            !call.is_boundary()
                && call
                    .structural_result()
                    .is_some_and(|result| result.multiplicity == Multiplicity::Unrestricted)
        }
        CheckedUnitEffectOperationPlan::EstablishScalarArray { result, .. }
        | CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, .. }
        | CheckedUnitEffectOperationPlan::EstablishByteSequence { result, .. }
        | CheckedUnitEffectOperationPlan::EstablishViewSubslice { result, .. } => {
            result.multiplicity == Multiplicity::Unrestricted
        }
        _ => false,
    };
    if unrestricted_copy {
        if access == CheckedStructuralAccess::Owned && projected {
            return None;
        }
        return Some(());
    }
    let (result, discard_result_on_return) = producer.discardable_structural_result_mut()?;
    if result.multiplicity != Multiplicity::Affine || !*discard_result_on_return {
        // Linear results never acquire automatic disposal debt. Moving a
        // whole result continues its checked call claim frontier; source flow
        // and independent Terminal replay reject a second use of that claim.
        if result.multiplicity == Multiplicity::Linear
            && !*discard_result_on_return
            && access == CheckedStructuralAccess::Owned
            && !projected
        {
            return Some(());
        }
        return None;
    }
    match access {
        // A projected transfer leaves its root owner alive until the
        // exact complement is committed on the consumer continuation.
        CheckedStructuralAccess::Owned if projected => {}
        CheckedStructuralAccess::Owned => *discard_result_on_return = false,
        CheckedStructuralAccess::SharedBorrow
        | CheckedStructuralAccess::MutableBorrow
        | CheckedStructuralAccess::WriteOnlyBorrow => {}
    }
    Some(())
}

pub(super) fn consume_value_places(
    facts: &CheckFacts,
    root: checked_trees::CheckedStructuralValueHandle,
    results: &[(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)],
    operations: &mut [CheckedUnitEffectOperationPlan],
) -> Option<()> {
    let plans = &facts.values.structural_values;
    let mut pending = vec![root];
    let mut visited = Vec::new();
    while let Some(value) = pending.pop() {
        if !plans.nodes.is_valid(value) || visited.contains(&value) {
            return None;
        }
        visited.push(value);
        match &plans.nodes.get(value).kind {
            checked_trees::CheckedStructuralValueKind::Place(argument) => {
                if let checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                    symbol,
                } = argument.source
                {
                    let mut matching = results
                        .iter()
                        .filter(|(_, source)| *source == facts::PlaceRoot::Symbol(symbol));
                    let (result, _) = matching.next()?;
                    if matching.next().is_some() || result.type_identity != argument.type_identity {
                        return None;
                    }
                    consume_result(
                        operations,
                        result.binding_ordinal,
                        argument.access,
                        !argument.path.is_empty(),
                    )?;
                }
            }
            checked_trees::CheckedStructuralValueKind::Record { fields, .. }
            | checked_trees::CheckedStructuralValueKind::StructuralCase { fields, .. } => {
                for field in plans.record_fields.span(*fields)? {
                    if let checked_trees::CheckedStructuralRecordFieldValue::Structural(value) =
                        field.value
                    {
                        pending.push(value);
                    }
                }
            }
            checked_trees::CheckedStructuralValueKind::Dispatch { arms, .. } => {
                pending.extend(plans.dispatch_arms.span(*arms)?.iter().map(|arm| arm.value));
            }
            checked_trees::CheckedStructuralValueKind::Projection { source, .. } => {
                // The selected edge moves a projected child: its root owner
                // remains live for the residual complement that edge commits.
                if let checked_trees::CheckedStructuralValueKind::Place(argument) =
                    &plans.nodes.get(*source).kind
                    && let checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                        symbol,
                    } = argument.source
                {
                    let mut matching = results
                        .iter()
                        .filter(|(_, source)| *source == facts::PlaceRoot::Symbol(symbol));
                    let (result, _) = matching.next()?;
                    if matching.next().is_some() || result.type_identity != argument.type_identity {
                        return None;
                    }
                    consume_result(operations, result.binding_ordinal, argument.access, true)?;
                }
            }
            checked_trees::CheckedStructuralValueKind::FixedArray { elements } => {
                pending.extend(elements.iter().copied());
            }
            checked_trees::CheckedStructuralValueKind::Reference { .. }
            | checked_trees::CheckedStructuralValueKind::BorrowedSliceView { .. }
            | checked_trees::CheckedStructuralValueKind::MutableSliceView { .. }
            | checked_trees::CheckedStructuralValueKind::Call { .. }
            | checked_trees::CheckedStructuralValueKind::ScalarCasePlace { .. }
            | checked_trees::CheckedStructuralValueKind::CopiedStructuralPlace { .. }
            | checked_trees::CheckedStructuralValueKind::ViewElementCopy { .. }
            | checked_trees::CheckedStructuralValueKind::ZeroedScalarArray { .. }
            | checked_trees::CheckedStructuralValueKind::ScalarArray { .. }
            | checked_trees::CheckedStructuralValueKind::ZeroedStructural { .. }
            | checked_trees::CheckedStructuralValueKind::ByteLiteralView { .. }
            | checked_trees::CheckedStructuralValueKind::Case(_) => {}
        }
    }
    Some(())
}
