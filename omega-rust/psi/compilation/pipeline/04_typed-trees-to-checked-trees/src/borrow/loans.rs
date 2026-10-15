//! Statement loans: the borrows a `let` or an assignment forms. Each loan
//! records its owner (the local, and the field path inside it that holds the
//! reference), the place it borrows, its access kind and its lineage.
//!
//! `statement_borrow_loans` is the entry; `state::append_state_borrow_facts`
//! calls it for each statement in order and records every returned
//! `StatementBorrowLoan` as a loan fact. Loans come from:
//!
//! - a `let` of reference type (`reference_local_borrow_loans`);
//! - a `let` of borrow-carrying data (`borrow_carrying_data_loans`), one loan
//!   for each reference its initializer supplies (`aggregate`);
//! - an assignment to an earlier local, or to a field of one, whose target is
//!   a reference or borrow-carrying data, handled as the equivalent `let`
//!   with the field path prefixed to each loan's owner path.
//!
//! For a reference `let`, `returned_carriers` is tried first, for a value
//! taken from a call result whose fields carry borrows, and a `match` lends
//! each arm's target. Otherwise the borrowed place comes from the borrow,
//! recast, place expression or view-returning call
//! (`helper_call_borrow_loan_place`, which resolves the call's returned-view
//! source through `view_link`). A place rooted at a local that already holds
//! a loan in this state is rebased onto that loan's place, once per such
//! loan. Locals, stored leaves and replacements use the same formation rule:
//! `retained_reference_lineage` gives a cast, or a call that declares
//! no direct view source, `UnretainedDerived` unless it is an explicit
//! reborrow; otherwise a place that was not rebased is `DirectRoot`, an
//! explicit reborrow through a single retained parent loan is `Reborrow`,
//! and the rest are `UnretainedDerived`.
//!
//! `owner_paths` matches owner field paths against place segments, and
//! `types` answers whether a type is a reference and with which access.
//! Other modules use `borrow_initializer_expressions` (persistent storage
//! check),
//! `call_declares_direct_view_source` (resource lineage replay), and
//! `types::reference_borrow_access_kind` (borrow check details).
//! The state builder also captures normal returned leaves through
//! `returned_value_loans` while its current descriptor roster is available.
//! Those leaves are facts, separate from local activation/lifecycle events.

use crate::borrow::view_link::is_borrow_carrying_data;
use arena::Handle;
use checked_trees::expression::ExpressionHandle;
use checked_trees::name::Identifier;
use checked_trees::statement::StatementNode;
use symbols::SymbolHandle;

use super::accesses::{self, borrow_access_place};
use super::tracker::{BorrowOwnerSegment, StateLoanTracker};
use aggregate::borrowed_initializers;
use owner_paths::owner_path_matches;
pub(crate) use owner_paths::place_path_covers_owner;
pub(crate) use owner_paths::{
    owner_path_from_place_segments, owner_segment_matches_place_segment, store_destination_places,
};

mod aggregate;
mod owner_paths;
mod returned_carriers;
pub(crate) use returned_carriers::{call_reference_sources, returned_body_source_mismatch};
mod replacement;
mod stored_calls;
pub(in crate::borrow) use replacement::{CarrierReplacement, capture_replacement};
pub(crate) mod types;

use types::{is_reference_type, reference_borrow_access_kind};

mod reference_borrows;
pub(in crate::borrow) use aggregate_loans::borrowed_initializer_loans;
use aggregate_loans::*;
pub(crate) use call_views::argument_borrow_loan_place;
pub(in crate::borrow) use call_views::call_view_return_contract;
use reference_borrows::*;

mod aggregate_loans;
mod call_views;

#[derive(Clone)]
pub(super) struct StatementBorrowLoan {
    pub(super) formation_expression: ExpressionHandle,
    pub(super) owner_symbol: SymbolHandle,
    pub(super) storage_owner: facts::PlaceRoot,
    pub(super) owner_name: Identifier,
    pub(super) owner_path: Vec<BorrowOwnerSegment>,
    pub(super) place: accesses::BorrowAccessPlace,
    pub(super) retained_operand: Option<checked_trees::BorrowRetainedOperand>,
    pub(super) source_owner_symbol: SymbolHandle,
    pub(super) supplying_loans: Vec<Handle<checked_trees::BorrowLoanFact>>,
    pub(super) lineage: checked_trees::BorrowLoanLineage,
    pub(super) kind: checked_trees::BorrowAccessKind,
    /// The loan's retained root was captured from a call result, not from a
    /// borrow expression. It resolves receivers like any direct root, but it
    /// is not borrow ancestry: children formed through it stay derived rather
    /// than claiming `Reborrow` lineage.
    pub(super) call_result: bool,
}

#[derive(Clone)]
struct RebasedBorrowPlace {
    place: accesses::BorrowAccessPlace,
    retained_operand: Option<checked_trees::BorrowRetainedOperand>,
    source_owner_symbol: SymbolHandle,
    parent_loan: Handle<checked_trees::BorrowLoanFact>,
    parent_lineage_is_retained: bool,
}

/// A reference keeps its computed backing's stored descriptors live, but those
/// descriptors are not fields of the reference variable. Capture them once
/// under the backing's storage identity. Aliases retain that identity and the
/// ordinary lifetime closure extends its contents without replaying formation.
pub(super) fn retain_backing_contents(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    statement_index: usize,
    machine_symbol: SymbolHandle,
    statement: &StatementNode,
    pending: &mut Vec<StatementBorrowLoan>,
    current: &[StateLoanTracker],
) {
    let (owner, name) = statement
        .as_local_data()
        .map_or((state.symbol, &state.name), |local| {
            (local.symbol, &local.name)
        });
    let mut backings = Vec::new();
    validation::visit_statement_value_occurrences(
        program,
        machine_symbol,
        state.symbol,
        statement_index,
        statement,
        &mut |expression, _, _| {
            let Some(borrow) = program.expression_table.borrow_expression(expression) else {
                return;
            };
            let Some(place) = validation::canonical_place_from_expression_in_state(
                program,
                state.symbol,
                statement_index,
                borrow.target,
            ) else {
                return;
            };
            if let facts::PlaceRoot::Expression(expression) = place.root
                && !backings.contains(&expression)
                && !current.iter().any(|loan| loan.storage_owner == place.root)
            {
                backings.push(expression);
            }
        },
    );
    for expression in backings {
        let Some(type_reference) = validation::expression_type_reference_in_state(
            program,
            state.symbol,
            statement_index,
            expression,
        ) else {
            continue;
        };
        if validation::reference_shell(program, type_reference).is_some()
            || !is_borrow_carrying_data(program, type_reference)
        {
            continue;
        }
        let backing = checked_trees::statement::TableLocalData {
            role: language_core::LocalBindingRole::Value,
            symbol: owner,
            name: name.clone(),
            type_reference,
            initial_value: expression,
            is_mutable: false,
            type_is_inferred: false,
            is_operand_capture: false,
            relevance: language_core::BindingRelevance::Relevant,
        };
        let contents = borrow_carrying_data_loans(
            program,
            state,
            statement_index,
            machine_symbol,
            &backing,
            current,
        );
        pending.extend(contents.into_iter().map(|mut loan| {
            loan.storage_owner = facts::PlaceRoot::Expression(expression);
            loan
        }));
    }
}

/// The initializer expressions that supply the references structurally carried
/// by `type_reference`. Persistent-storage checking uses this same traversal so
/// its static-source exemption cannot disagree with local aggregate loan
/// attribution about which nested fields actually borrow.
pub(crate) fn borrow_initializer_expressions(
    program: &dyn ::validation::CheckingInput,
    type_reference: typed_trees::types::TypeReferenceHandle,
    expression: ExpressionHandle,
) -> Vec<ExpressionHandle> {
    borrowed_initializers(program, type_reference, expression, &[], &[])
        .into_iter()
        .map(|initializer| initializer.expression)
        .collect()
}

/// Form normal result leaves through the ordinary binding operation while the
/// state's current incoming and established descriptors are available. The
/// result owner is only a path namespace; it creates no local activation.
pub(super) fn returned_value_loans(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    statement_index: usize,
    machine_symbol: SymbolHandle,
    statement: &StatementNode,
    loan_trackers: &[StateLoanTracker],
) -> Vec<StatementBorrowLoan> {
    use checked_trees::statement::{TransitionExit, TransitionTargetNode};

    if !is_reference_type(program, state.return_type)
        && !is_borrow_carrying_data(program, state.return_type)
    {
        return Vec::new();
    }
    let mut returned_value = checked_trees::statement::TableLocalData {
        role: language_core::LocalBindingRole::Value,
        symbol: state.symbol,
        name: state.name.clone(),
        type_reference: state.return_type,
        initial_value: ExpressionHandle::invalid(),
        is_mutable: false,
        type_is_inferred: false,
        is_operand_capture: false,
        relevance: language_core::BindingRelevance::Relevant,
    };
    let expression = match statement {
        StatementNode::Expression(expression)
            if statement_index + 1
                == program
                    .statement_table
                    .statements(state.statement_nodes)
                    .len() =>
        {
            *expression
        }
        StatementNode::Transition(transition)
            if transition.exit == TransitionExit::Ordinary
                && !transition.continuation.is_valid() =>
        {
            match program.statement_table.transition_target(transition.target) {
                TransitionTargetNode::Value(expression) => *expression,
                TransitionTargetNode::Named {
                    path, arguments, ..
                } => {
                    use crate::borrow::view_link::{
                        ViewReturnFieldSource, ViewReturnSource, resolve_view_return_source,
                    };

                    let Some((_, target)) = validation::find_state(program, path.symbol)
                        .or_else(|| validation::find_machine_head(program, path.symbol))
                    else {
                        return Vec::new();
                    };
                    // Preparation has selected this state's declaration. Its
                    // declared lifetime frontier remains conservative across
                    // the edge; ordinary source formation authenticates the
                    // actuals against the caller's current descriptor roster.
                    let fields = match resolve_view_return_source(program, target) {
                        ViewReturnSource::Fields { fields } => fields,
                        ViewReturnSource::Parameter { parameter_index } => {
                            let Some(kind) =
                                reference_borrow_access_kind(program, target.return_type)
                            else {
                                return Vec::new();
                            };
                            let Some(parameter) =
                                program.state_parameters(target).get(parameter_index)
                            else {
                                return Vec::new();
                            };
                            vec![ViewReturnFieldSource {
                                source:
                                    validation::borrow_contracts::ViewReturnInputSource::Referent,
                                owner_path: Vec::new(),
                                source_path: Vec::new(),
                                source_type: parameter.type_reference,
                                parameter_index,
                                kind,
                            }]
                        }
                        ViewReturnSource::NotApplicable | ViewReturnSource::Ambiguous(_) => {
                            return Vec::new();
                        }
                    };
                    return aggregate_loans::result_source_loans(
                        program,
                        state.symbol,
                        statement_index,
                        machine_symbol,
                        &returned_value,
                        target.symbol,
                        program.statement_table.expression_handles(*arguments),
                        fields,
                        &[],
                        &[],
                        loan_trackers,
                    );
                }
                TransitionTargetNode::Terminal => return Vec::new(),
            }
        }
        _ => return Vec::new(),
    };
    returned_value.initial_value = expression;
    value_borrow_loans(
        program,
        state,
        statement_index,
        machine_symbol,
        &StatementNode::LocalData(returned_value),
        loan_trackers,
    )
}

pub(super) fn statement_borrow_loans(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    statement_index: usize,
    machine_symbol: SymbolHandle,
    statement: &StatementNode,
    loan_trackers: &[StateLoanTracker],
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    replacement: Option<&CarrierReplacement>,
) -> Vec<StatementBorrowLoan> {
    let mut loans = stored_calls::loans(
        program,
        state,
        machine_symbol,
        statement,
        call_frames,
        loan_trackers,
    );
    // These rows describe the statement's final receiving slots, not every
    // intermediate descriptor generation. The enclosing store overwrites RHS
    // slot effects only within its captured subtree. Independently captured
    // children retain their own storage owners and loan ancestry.
    if let Some(replacement) = replacement {
        loans.retain(|loan| !replacement.replaces(program, loan.storage_owner, &loan.owner_path));
    }
    loans.extend(value_borrow_loans(
        program,
        state,
        statement_index,
        machine_symbol,
        statement,
        loan_trackers,
    ));
    loans
}

fn value_borrow_loans(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    statement_index: usize,
    machine_symbol: SymbolHandle,
    statement: &StatementNode,
    loan_trackers: &[StateLoanTracker],
) -> Vec<StatementBorrowLoan> {
    match statement {
        StatementNode::Assignment(assignment) => assignment_borrow_loans(
            program,
            state,
            statement_index,
            machine_symbol,
            assignment,
            loan_trackers,
        ),
        StatementNode::LocalData(local_data) => {
            // Borrow-carrying data local (`let msg: Message = Message { body: input }`):
            // the value borrows whatever its reference field is initialized from,
            // so its loan follows that source's place (decision 15 stage 2).
            if !is_reference_type(program, local_data.type_reference)
                && is_borrow_carrying_data(program, local_data.type_reference)
            {
                return borrow_carrying_data_loans(
                    program,
                    state,
                    statement_index,
                    machine_symbol,
                    local_data,
                    loan_trackers,
                );
            }
            if !is_reference_type(program, local_data.type_reference)
                && !element_view_local_binding(program, state, machine_symbol, local_data)
            {
                return Vec::new();
            }

            reference_local_borrow_loans(
                program,
                state,
                statement_index,
                machine_symbol,
                local_data,
                loan_trackers,
            )
        }
        StatementNode::Call(_)
        | StatementNode::RootBinding(_)
        | StatementNode::AssemblyFact(_)
        | StatementNode::Expression(_)
        | StatementNode::Transition(_) => Vec::new(),
    }
}

/// A store installs the replacement's loans in its actual carrier. A projected
/// store through a local reference belongs to that reference's backing storage,
/// so ending the temporary reference cannot end loans installed in the parent.
/// These rows retain conflicts within the source state; they do not establish
/// outlives authority for transporting local storage into a successor.
fn assignment_borrow_loans(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    statement_index: usize,
    machine_symbol: SymbolHandle,
    assignment: &checked_trees::statement::TableAssignment,
    loan_trackers: &[StateLoanTracker],
) -> Vec<StatementBorrowLoan> {
    let Some(target) = borrow_access_place(
        program,
        state.symbol,
        statement_index,
        assignment.target,
        machine_symbol,
    ) else {
        return Vec::new();
    };
    // The lexical event owner is not necessarily the storage owner. A direct
    // computed destination has only this statement's namespace; a named
    // reference keeps the installed descriptors live through its backing.
    let target_symbol = target.root.as_symbol().unwrap_or(state.symbol);
    let owner_name: Identifier = program.symbols.name(target_symbol).into();
    let Some(target_type) = program.machine(machine_symbol).and_then(|machine| {
        validation::assignment_stored_value_type(program, machine, state, assignment)
    }) else {
        return Vec::new();
    };
    if !is_reference_type(program, target_type) && !is_borrow_carrying_data(program, target_type) {
        return Vec::new();
    }
    let synthetic_local = checked_trees::statement::TableLocalData {
        role: language_core::LocalBindingRole::Value,
        symbol: target_symbol,
        name: owner_name,
        type_reference: target_type,
        initial_value: assignment.value,
        is_mutable: true,
        type_is_inferred: false,
        is_operand_capture: false,
        relevance: language_core::BindingRelevance::Relevant,
    };

    let loans = if is_reference_type(program, target_type) {
        reference_local_borrow_loans(
            program,
            state,
            statement_index,
            machine_symbol,
            &synthetic_local,
            loan_trackers,
        )
    } else {
        borrow_carrying_data_loans(
            program,
            state,
            statement_index,
            machine_symbol,
            &synthetic_local,
            loan_trackers,
        )
    };
    let writes_referent = validation::expression_type_reference_in_state(
        program,
        state.symbol,
        statement_index,
        assignment.target,
    )
    .is_some_and(|reference| is_reference_type(program, reference))
        && !is_reference_type(program, target_type);
    let destinations = store_destination_places(
        program,
        state,
        target,
        writes_referent,
        loan_trackers.iter().map(|loan| {
            (
                loan.storage_owner,
                loan.owner_path.as_slice(),
                loan.place.incoming_reference,
                loan.place.root,
                loan.place.segments.as_slice(),
            )
        }),
    );
    destinations
        .iter()
        .flat_map(|destination| {
            let destination_symbol = destination.root.as_symbol().unwrap_or(target_symbol);
            let owner_path = owner_path_from_place_segments(program, &destination.segments);
            loans.iter().cloned().map(move |mut loan| {
                loan.owner_symbol = destination_symbol;
                loan.storage_owner = destination.root;
                loan.owner_name = program.symbols.name(destination_symbol).into();
                let mut path = owner_path.clone();
                path.append(&mut loan.owner_path);
                loan.owner_path = path;
                loan
            })
        })
        .collect()
}

/// The borrow-fact access kind matching a reference access.
pub(crate) fn borrow_access_kind(
    access: language_semantics::ReferenceAccess,
) -> checked_trees::BorrowAccessKind {
    match access {
        language_semantics::ReferenceAccess::Shared => checked_trees::BorrowAccessKind::Read,
        language_semantics::ReferenceAccess::Mutable => checked_trees::BorrowAccessKind::Mutable,
        language_semantics::ReferenceAccess::WriteOnly => {
            checked_trees::BorrowAccessKind::WriteOnly
        }
    }
}
