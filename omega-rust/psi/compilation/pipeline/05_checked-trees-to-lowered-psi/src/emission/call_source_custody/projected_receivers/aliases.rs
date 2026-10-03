//! Source replay for erased statically projected receiver alias chains.
//!
//! This binds erased names to their exact loans. The separate root-handoff
//! receiving pass retains complete lineage; neither pass grants restored use.

use crate::lowering_error::LoweringError;
use crate::lowering_error::unsupported;
use arena::Handle;
use checked_trees::CheckedTrees;
use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use checked_trees::statement::StatementNode;
use checked_trees::types::TypeReferenceNode;
use checked_trees::{
    BorrowAccessKind, BorrowLoanFact, BorrowLoanLineage, CheckedParentBorrowResource,
    CheckedReborrowAccessEffect, FlowBorrowWeakeningReason, FlowConstraintKind,
    FlowInvalidationSource,
};
use language_semantics::ReferenceAccess;
use symbols::SymbolHandle;

mod reborrow;
use reborrow::reborrow_resource;

pub(crate) fn parameter_source(
    checked: &CheckedTrees,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement_index: usize,
    owner: SymbolHandle,
    formation: bool,
) -> Result<Option<super::ReceiverSource>, LoweringError> {
    let (authored_machine, authored_state) =
        crate::expression_preparation::source_custody::authored_state(checked, state)?;
    if authored_machine.symbol != machine {
        return unsupported("receiver alias state belongs to another machine");
    }
    let statements = checked
        .statement_table
        .statements(authored_state.statement_nodes);
    let prefix = statements
        .iter()
        .take_while(|statement| matches!(statement, StatementNode::LocalData(_)))
        .count();
    // A carrier can be declared mid-body: the use site bounds the search, not
    // the leading local-data run.
    let declarations = statements
        .iter()
        .take(statement_index)
        .enumerate()
        .filter_map(|(position, statement)| {
            let StatementNode::LocalData(local) = statement else {
                return None;
            };
            (local.symbol == owner).then_some((position, local))
        })
        .collect::<Vec<_>>();
    let [(declaration_index, local)] = declarations.as_slice() else {
        return if declarations.is_empty() {
            Ok(None)
        } else {
            unsupported("receiver alias has duplicate declarations")
        };
    };
    if local.is_mutable
        || !owner.is_valid()
        || statement_index <= *declaration_index
        || (!formation && statement_index < prefix)
    {
        return unsupported("receiver alias is mutable or used before its prefix ends");
    }
    let ExpressionNode::Borrow(initializer) =
        checked.expression_table.expression(local.initial_value)
    else {
        if let Some(source) =
            call_result_source(checked, machine, state, *declaration_index, owner, local)?
        {
            return Ok(Some(source));
        }
        return unsupported("receiver alias has no explicit borrow initializer");
    };
    if !checked
        .expression_table
        .expression_is_valid(local.initial_value)
        || !matches!(
            initializer.access,
            ReferenceAccess::Mutable | ReferenceAccess::WriteOnly
        )
    {
        return unsupported("receiver alias initializer identity or access changed");
    }
    let access = match initializer.access {
        ReferenceAccess::Mutable => BorrowAccessKind::Mutable,
        ReferenceAccess::WriteOnly => BorrowAccessKind::WriteOnly,
        ReferenceAccess::Shared => return unsupported("receiver alias must have exclusive access"),
    };
    let captured = super::resolve_source(
        checked,
        machine,
        state,
        initializer.target,
        super::SourceUse::Store,
        Some((*declaration_index, true)),
    )?;
    // A parent in this ledger sense is an earlier local that itself carries a
    // loan: a value local is a storage root, and a loan captured on it is
    // DirectRoot lineage, not a derived or retained-parent reborrow.
    let parent_declaration = checked
        .state_local_declaration(authored_state, captured.owner)
        .is_some_and(|(ordinal, local)| {
            ordinal < *declaration_index
                && matches!(
                    checked
                        .type_reference_table
                        .type_reference(local.type_reference),
                    TypeReferenceNode::Reference { .. }
                )
        });
    let root_symbol = captured.root;
    let roots = checked
        .state_parameters(authored_state)
        .iter()
        .filter(|parameter| {
            parameter.symbol == root_symbol || (parameter.is_self && root_symbol == machine)
        })
        .collect::<Vec<_>>();
    // A view local is itself a loan binding, so it carries no parameter
    // row: the alias reborrows it through the state's declaration, and the
    // borrow-ledger checks below carry the custody evidence a parameter
    // root supplies for direct places. A mutable reference-typed root
    // local could rebind the captured place, so it declines here. A value
    // local's slot is the referent itself: writing its contents never
    // moves the place, and the ledger already suspends the root for the
    // loan's live interval, so owned storage grants every admitted access.
    let (root_is_self, root_is_const, root_type, root_access) = match roots.as_slice() {
        [root] => {
            let TypeReferenceNode::Reference {
                referee: root_type,
                access: root_access @ (ReferenceAccess::Mutable | ReferenceAccess::WriteOnly),
                ..
            } = checked
                .type_reference_table
                .type_reference(root.type_reference)
            else {
                return unsupported("receiver alias root cannot grant write-only access");
            };
            (root.is_self, root.is_const, *root_type, *root_access)
        }
        [] => {
            let Some((_, local_root)) =
                checked.state_local_declaration(authored_state, root_symbol)
            else {
                return unsupported("receiver alias has no unique parameter root");
            };
            let (root_type, root_access) = match checked
                .type_reference_table
                .type_reference(local_root.type_reference)
            {
                TypeReferenceNode::Reference {
                    referee,
                    access: root_access @ (ReferenceAccess::Mutable | ReferenceAccess::WriteOnly),
                    ..
                } => {
                    if local_root.is_mutable {
                        return unsupported("receiver alias root local is mutable");
                    }
                    (*referee, *root_access)
                }
                TypeReferenceNode::Reference { .. } => {
                    return unsupported("receiver alias root cannot grant write-only access");
                }
                _ => (local_root.type_reference, ReferenceAccess::Mutable),
            };
            (false, false, root_type, root_access)
        }
        _ => return unsupported("receiver alias has no unique parameter root"),
    };
    let TypeReferenceNode::Reference {
        referee: local_type,
        access: local_access,
        ..
    } = checked
        .type_reference_table
        .type_reference(local.type_reference)
    else {
        return unsupported("receiver alias is not a reference");
    };
    if *local_access != initializer.access {
        return unsupported("receiver alias declaration disagrees with its initializer access");
    }
    if root_access == ReferenceAccess::WriteOnly && access != BorrowAccessKind::WriteOnly {
        return unsupported("receiver alias widens its root access");
    }
    // Attached self carries machine/Self identity in the typed reference. Its
    // actual referent is the independently resolved attachment, as in signatures.
    let root_type = if root_is_self {
        let TypeReferenceNode::Named { symbol, .. } =
            checked.type_reference_table.type_reference(root_type)
        else {
            return unsupported("receiver alias self has no exact nominal referent");
        };
        if (*symbol != machine && *symbol != authored_machine.attached_data_symbol)
            || !authored_machine.attached_data_symbol.is_valid()
            || !checked
                .data_definitions()
                .iter()
                .any(|data| data.symbol == authored_machine.attached_data_symbol)
        {
            return unsupported("receiver alias self disagrees with its attachment");
        }
        if captured.path.is_empty() {
            checked
                .type_reference_table
                .find_named_type_reference(authored_machine.attached_data_symbol)
                .ok_or(LoweringError::Unsupported(
                    "receiver alias self lost its attachment",
                ))?
        } else {
            // A projected initializer supplies its exact field/element type.
            // It does not require a separately authored whole attachment type.
            root_type
        }
    } else {
        root_type
    };
    let projected_type = if captured.path.is_empty() {
        root_type
    } else {
        // A recast-spelled initializer names its referent type itself:
        // `&mut place as &mut T` views the place under T's stated shape, and
        // the source place's declared type is the byte carrier T re-views.
        let reference = match checked.expression_table.expression(initializer.target) {
            ExpressionNode::Cast(cast) if cast.form.is_recast() => cast.result_type,
            _ => validation::declared_place_type_raw(
                &checked.typed,
                authored_machine,
                Some(authored_state),
                initializer.target,
            )
            .ok_or(LoweringError::Unsupported(
                "receiver alias initializer has no exact referent type",
            ))?,
        };
        match checked.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Reference { referee, .. } => *referee,
            _ => reference,
        }
    };
    if root_is_const
        || checked.typed.normalized_type_identity(*local_type)
            != checked.typed.normalized_type_identity(projected_type)
    {
        return unsupported("receiver alias changes its referent type");
    }
    let borrow = &checked.facts.borrow;
    let states = borrow
        .states
        .iter()
        .filter(|(_, row)| row.machine_symbol == machine && row.state_symbol == state)
        .collect::<Vec<_>>();
    let [(_, borrow_state)] = states.as_slice() else {
        return unsupported("receiver alias has no unique borrow state");
    };
    let loans = borrow
        .loans
        .iter()
        .filter(|(handle, loan)| {
            borrow.state_owns_loan(borrow_state, *handle) && loan.owner_symbol == owner
        })
        .collect::<Vec<_>>();
    let [(loan_handle, loan)] = loans.as_slice() else {
        return unsupported("receiver alias has no unique loan occurrence");
    };
    let parent_loans = borrow
        .loans
        .iter()
        .filter(|(handle, candidate)| {
            borrow.state_owns_loan(borrow_state, *handle)
                && candidate.owner_symbol == captured.owner
        })
        .collect::<Vec<_>>();
    // A loan derived through an unretained parent (a view-local binding)
    // carries no parent handle: the ledger binds it by `source_owner` and
    // spells its place against the parent's ultimate root, so its captured
    // place is the parent's place extended by the authored projection.
    // `Reborrow` lineage is reserved for a retained parent loan.
    let derived_parent = parent_declaration && loan.lineage == BorrowLoanLineage::UnretainedDerived;
    let exact_lineage = if derived_parent {
        matches!(parent_loans.as_slice(), [(_, parent)]
        if loan.source_owner_symbol == captured.owner
            && loan.root_symbol == parent.root_symbol
            && parent.kind.direct_reborrow_effect(&access)
                == Some(CheckedReborrowAccessEffect::ExclusiveSuspension)
            && {
                let parent_segments = borrow.loan_segments(parent);
                let loan_segments = borrow.loan_segments(loan);
                loan_segments.len()
                    == parent_segments.len() + captured.captured_place.segments.len()
                    && loan_segments
                        .iter()
                        .zip(parent_segments)
                        .all(|(loan, parent)| loan == parent)
                    && loan_segments[parent_segments.len()..]
                        .iter()
                        .zip(&captured.captured_place.segments)
                        .all(|(loan, captured)| {
                            derived_segment_replay(checked, authored_state, loan, captured)
                        })
            })
    } else if parent_declaration {
        matches!(parent_loans.as_slice(), [(parent_handle, parent)]
            if loan.lineage == BorrowLoanLineage::Reborrow { parent_loan: *parent_handle }
                && loan.source_owner_symbol == captured.owner
                && loan.root_symbol == parent.root_symbol
                && parent.kind.direct_reborrow_effect(&access)
                    == Some(CheckedReborrowAccessEffect::ExclusiveSuspension))
    } else {
        loan.lineage == BorrowLoanLineage::DirectRoot
            && !loan.source_owner_symbol.is_valid()
            && loan.root_symbol == captured.captured_place.root_symbol
    };
    // A derived loan's place agreement folded into its lineage evidence
    // above: its root is the parent's ultimate root, not the authored
    // alias root the direct spellings carry.
    let recast_initializer = matches!(
        checked.expression_table.expression(initializer.target),
        ExpressionNode::Cast(cast) if cast.form.is_recast()
    );
    let exact_place = derived_parent
        || (loan.root_symbol == captured.captured_place.root_symbol
            && loan_place_agrees(
                borrow.loan_segments(loan),
                &captured.captured_place.segments,
                recast_initializer,
            ));
    if loan.statement_index != *declaration_index
        || loan.kind != access
        || !exact_lineage
        || !exact_place
        || !borrow.loan_owner_path(loan).is_empty()
    {
        return unsupported("receiver alias loan disagrees with its direct initializer");
    }
    let flow_states = checked
        .facts
        .flow
        .control
        .states
        .iter()
        .filter(|(_, row)| row.machine_symbol == machine && row.state_symbol == state)
        .collect::<Vec<_>>();
    let [(_, flow)] = flow_states.as_slice() else {
        return unsupported("receiver alias has no unique flow state");
    };
    let mut last_use = None;
    let mut current_use = false;
    for (position, statement) in statements.iter().enumerate().skip(*declaration_index + 1) {
        let (receiver, arguments) = match statement {
            StatementNode::LocalData(child) => {
                if !contains_owner(checked, child.initial_value, owner) {
                    continue;
                }
                // A scalar local initialized from member reads of the referent
                // observes the loan exactly as a guard's member reads do: the
                // copied scalar carries no loan, so the statement is a use.
                if checked
                    .typed
                    .primitive_type_reference(child.type_reference)
                    .is_some()
                    && owner_mentions_are_member_reads(checked, child.initial_value, owner)
                {
                    (owner, &[][..])
                } else {
                    let ExpressionNode::Borrow(initializer) =
                        checked.expression_table.expression(child.initial_value)
                    else {
                        return unsupported("receiver alias escapes through a local initializer");
                    };
                    if child.is_mutable
                        || !matches!(
                            (local_access, initializer.access),
                            (
                                ReferenceAccess::Mutable,
                                ReferenceAccess::Mutable | ReferenceAccess::WriteOnly
                            ) | (ReferenceAccess::WriteOnly, ReferenceAccess::WriteOnly)
                        )
                        || receiver_root(checked, initializer.target)? != owner
                    {
                        return unsupported("receiver alias child formation changed");
                    }
                    (owner, &[][..])
                }
            }
            StatementNode::Call(call) => (
                call.receiver_root_symbol,
                checked.statement_table.expression_handles(call.arguments),
            ),
            StatementNode::Expression(expression) => {
                // Completion expressions are ordinary source uses. An
                // unrelated value cannot extend this loan's interval; a
                // referent observation spends it just as a local initializer
                // or transition guard does. Actual carrier escapes still go
                // through the receiver/argument checks below.
                if !contains_owner(checked, *expression, owner) {
                    continue;
                }
                if owner_mentions_are_member_reads(checked, *expression, owner) {
                    (owner, &[][..])
                } else {
                    let ExpressionNode::Call(call) =
                        checked.expression_table.expression(*expression)
                    else {
                        return unsupported(
                            "receiver alias escapes through a completion expression",
                        );
                    };
                    (
                        receiver_root(checked, call.receiver)?,
                        checked.expression_table.expression_handles(call.arguments),
                    )
                }
            }
            // A store through the erased carrier is a use, not an escape: the
            // target's root decides whether this statement spends the alias,
            // while its value may not mention the carrier at all. An
            // unrelated assignment leaves the alias untouched. Mentions of
            // the owner inside the stored value observe the loan exactly as
            // a local initializer's member reads do — `x = cells[i].f`
            // reads the referent and carries no alias.
            StatementNode::Assignment(assignment) => {
                if !contains_owner(checked, assignment.value, owner) {
                    (receiver_root(checked, assignment.target)?, &[][..])
                } else if owner_mentions_are_member_reads(checked, assignment.value, owner) {
                    (owner, &[][..])
                } else {
                    (
                        receiver_root(checked, assignment.target)?,
                        std::slice::from_ref(&assignment.value),
                    )
                }
            }
            // The state's own terminator is not a use of the alias unless it
            // names it. A transition that mentions the owner in a target value
            // or successor argument would carry the alias into a successor
            // this pass does not model, so that still refuses; member reads of
            // the referent inside the guard spend the loan at the state
            // boundary instead. One that does not mention it simply ends the
            // scan's statement.
            StatementNode::Transition(transition) => {
                if !transition_mentions_owner(checked, transition, owner) {
                    continue;
                }
                if !transition_reads_owner_fields(checked, transition, owner) {
                    return unsupported("receiver alias escapes through a transition");
                }
                (owner, &[][..])
            }
            _ => return unsupported("receiver alias suffix contains a write, local, or escape"),
        };
        if arguments
            .iter()
            .any(|argument| contains_owner(checked, *argument, owner))
        {
            return unsupported("receiver alias escapes through an explicit argument");
        }
        if receiver != owner {
            continue;
        }
        let entries = checked
            .facts
            .flow
            .control
            .statements
            .span_or_empty(flow.statements)
            .iter()
            .filter(|row| row.statement_index == position)
            .collect::<Vec<_>>();
        let [entry] = entries.as_slice() else {
            return unsupported("receiver alias use has no unique statement entry");
        };
        let available = checked.facts.flow.contexts.constraint_refs
            .span_or_empty(entry.entry_constraints)
            .iter()
            .any(|row| matches!(row.kind, FlowConstraintKind::BorrowLoan { loan } if loan == *loan_handle));
        if !available {
            return unsupported("receiver alias loan is unavailable at its use");
        }
        last_use = Some(position);
        current_use |= position == statement_index;
    }
    if !current_use || last_use != Some(loan.last_use_statement_index) {
        return unsupported("receiver alias use interval disagrees with its source");
    }
    let activation = FlowInvalidationSource::Statement {
        statement_index: *declaration_index,
    };
    let boundary =
        loan.last_use_statement_index
            .checked_add(1)
            .ok_or(LoweringError::Unsupported(
                "receiver alias lifetime overflows",
            ))?;
    let expiry = (
        FlowInvalidationSource::Statement {
            statement_index: boundary,
        },
        if boundary == statements.len() {
            FlowBorrowWeakeningReason::StateExit
        } else {
            FlowBorrowWeakeningReason::LastUseExpired
        },
    );
    // A store through the erased carrier retires its loan at the assignment
    // itself; every other use expires one statement after its last
    // occurrence.
    let reassigned = (
        FlowInvalidationSource::Statement {
            statement_index: loan.last_use_statement_index,
        },
        FlowBorrowWeakeningReason::LocalReassigned,
    );
    let lifetimes = &checked.facts.flow.borrow_lifetimes;
    let activations = lifetimes
        .activations
        .span_or_empty(flow.borrow_activations)
        .iter()
        .filter(|row| row.loan == *loan_handle)
        .collect::<Vec<_>>();
    let weakenings = lifetimes
        .weakenings
        .span_or_empty(flow.borrow_weakenings)
        .iter()
        .filter(|row| row.loan == *loan_handle)
        .collect::<Vec<_>>();
    let [weakening_row] = weakenings.as_slice() else {
        return unsupported("receiver alias activation or weakening changed");
    };
    let (weakening, reason) = if boundary <= statements.len()
        && (weakening_row.source, &weakening_row.reason) == (expiry.0, &expiry.1)
    {
        expiry
    } else if (weakening_row.source, &weakening_row.reason) == (reassigned.0, &reassigned.1) {
        reassigned
    } else {
        return unsupported("receiver alias activation or weakening changed");
    };
    if !matches!(activations.as_slice(), [row] if row.source == activation) {
        return unsupported("receiver alias activation or weakening changed");
    }
    if parent_declaration {
        // An unretained parent owes no root handoff and mints no resource
        // row to replay: custody stays on the parent's own loan, the
        // child's derivation evidence was audited against the ledger
        // above, and its weakening retires the derived loan at last use.
        if derived_parent {
            return Ok(Some(captured));
        }
        if !formation && reason != FlowBorrowWeakeningReason::StateExit {
            return unsupported("receiver alias chain has no state-exit root handoff");
        }
        let [(parent_handle, _)] = parent_loans.as_slice() else {
            return unsupported("receiver alias has no exact parent loan");
        };
        reborrow_resource(
            checked,
            machine,
            state,
            *loan_handle,
            *parent_handle,
            owner,
            activation,
            weakening,
            reason,
        )?;
        return Ok(Some(captured));
    }
    let resources = borrow
        .direct_loan_resources
        .iter()
        .filter(|(_, row)| row.loan == *loan_handle)
        .collect::<Vec<_>>();
    let [(_, resource)] = resources.as_slice() else {
        return unsupported("receiver alias has no unique direct loan resource");
    };
    if resource.machine_symbol != machine
        || resource.state_symbol != state
        || resource.owner_symbol != owner
        || !resource.owner_path.is_empty()
        || resource.captured_place.root_symbol != captured.captured_place.root_symbol
        || !loan_place_agrees(
            &resource.captured_place.segments,
            &captured.captured_place.segments,
            recast_initializer,
        )
        || resource.access != access
        || resource.activation_source != activation
        || resource.weakening_source != weakening
        || resource.weakening_reason != reason
        || resource.parent_lifetime.machine_symbol != machine
        || resource.parent_lifetime.state_symbol != state
        || resource.parent_lifetime.root_symbol != loan.root_symbol
        || resource.restoration.parent != resource.parent_lifetime
        || resource.restoration.weakening_source != weakening
        || resource.restoration.weakening_reason != reason
    {
        return unsupported("receiver alias resource disagrees with its source lifetime");
    }
    Ok(Some(captured))
}

/// A `&mut`-view call-result local (`let cell: &mut Cell = self.bag
/// .pick()`) binds its place through the call's minted result loan, not an
/// authored `&mut place` initializer. The loan row names the same root and
/// projection the borrow literal would have — custody and expiry are
/// validated by the reference-results pass — so the receiver rejoins the
/// loaned place directly. The same exclusivity invariants the borrow path
/// audits hold here: one Mutable loan, formed at the declaration, rooted at
/// caller storage.
fn call_result_source(
    checked: &CheckedTrees,
    machine: SymbolHandle,
    state: SymbolHandle,
    declaration_index: usize,
    owner: SymbolHandle,
    local: &checked_trees::statement::TableLocalData,
) -> Result<Option<super::ReceiverSource>, LoweringError> {
    if !matches!(
        checked.expression_table.expression(local.initial_value),
        ExpressionNode::Call(_)
    ) || validation::reference_result_custody::mutable_borrowed_parts(
        &checked.typed,
        local.type_reference,
    )
    .is_none()
    {
        return Ok(None);
    }
    let borrow = &checked.facts.borrow;
    let borrow_state = borrow
        .states
        .iter()
        .map(|(_, borrow_state)| borrow_state)
        .find(|borrow_state| {
            borrow_state.machine_symbol == machine && borrow_state.state_symbol == state
        })
        .ok_or(LoweringError::Unsupported(
            "receiver call alias has no borrow state",
        ))?;
    let mut loans = borrow.loans.iter().filter(|(handle, loan)| {
        borrow.state_owns_loan(borrow_state, *handle)
            && loan.owner_symbol == owner
            && loan.statement_index == declaration_index
            && loan.kind == BorrowAccessKind::Mutable
            && borrow.loan_owner_path(loan).is_empty()
    });
    let Some((_, loan)) = loans.next() else {
        return Ok(None);
    };
    if loans.next().is_some() {
        return unsupported("receiver call alias has duplicate loans");
    }
    let _ = loan;
    // The call result is the view itself: `cell.value` writes through the
    // descriptor bound on the local, so the source roots at the binding and
    // the structural-local roster resolves its loaned place downstream.
    Ok(Some(super::ReceiverSource {
        root: local.symbol,
        path: Vec::new(),
        stamp: owner,
        owner,
        captured_place: checked_trees::CapturedPlace {
            root_symbol: local.symbol,
            segments: Vec::new(),
        },
        erased_alias: false,
    }))
}

/// Loan bookkeeping and the authored expression spell the same place two
/// ways when a recast loan carries the view's element window: `&mut
/// place[i] as &mut T` records `FixedRange { i, i + extent }` where the
/// captured place names `FixedIndex { i }`. Under a recast initializer a
/// range that starts at the captured element index is the same place;
/// every other pair compares literally.
pub(crate) fn loan_place_agrees(
    loan_segments: &[facts::PlaceSegment],
    captured_segments: &[facts::PlaceSegment],
    recast_initializer: bool,
) -> bool {
    loan_segments.len() == captured_segments.len()
        && loan_segments
            .iter()
            .zip(captured_segments)
            .all(|(loan, captured)| {
                *loan == *captured
                    || (recast_initializer
                        && matches!(
                            (*loan, *captured),
                            (
                                facts::PlaceSegment::FixedRange { start, .. },
                                facts::PlaceSegment::FixedIndex { index }
                            ) if start == index
                        ))
            })
}

/// The ledger spells a runtime selector as `Index { expression }` even
/// when the checker's own replay folded it to a constant — the same fold
/// `owner_path_matches_source` applies on the checker side, plus the
/// const-local fold an immutable scalar binding answers. Every other
/// segment pair compares literally.
fn derived_segment_replay(
    checked: &CheckedTrees,
    authored_state: &checked_trees::state::State,
    loan_segment: &facts::PlaceSegment,
    captured_segment: &facts::PlaceSegment,
) -> bool {
    match (loan_segment, captured_segment) {
        (facts::PlaceSegment::Index { expression }, facts::PlaceSegment::FixedIndex { index }) => {
            let folded = match checked.expression_table.expression(*expression) {
                checked_trees::expression::ExpressionNode::Name(name) => {
                    crate::expression_preparation::source_custody::structural_places::constant_local_index(
                        checked,
                        authored_state,
                        name.symbol,
                    )
                    .and_then(|index| usize::try_from(index).ok())
                }
                _ => checked
                    .typed
                    .expression_table
                    .constant_integer_value(*expression)
                    .and_then(|value| usize::try_from(value).ok()),
            };
            folded.is_some_and(|folded| folded == *index)
        }
        _ => loan_segment == captured_segment,
    }
}

fn receiver_root(
    checked: &CheckedTrees,
    expression: ExpressionHandle,
) -> Result<SymbolHandle, LoweringError> {
    if !expression.is_valid() {
        return Ok(SymbolHandle::invalid());
    }
    let mut cursor = expression;
    let mut visited = Vec::new();
    loop {
        if !checked.expression_table.expression_is_valid(cursor) || visited.contains(&cursor) {
            return unsupported("receiver alias source is stale or cyclic");
        }
        visited.push(cursor);
        cursor = match checked.expression_table.expression(cursor) {
            ExpressionNode::Name(name) => return Ok(name.head_symbol),
            ExpressionNode::Member(member) => member.receiver,
            ExpressionNode::Indexed(indexed) => indexed.collection,
            _ => return Ok(SymbolHandle::invalid()),
        };
    }
}

fn contains_owner(
    checked: &CheckedTrees,
    expression: ExpressionHandle,
    owner: SymbolHandle,
) -> bool {
    let mut pending = vec![expression];
    let mut visited = Vec::new();
    while let Some(expression) = pending.pop() {
        if !checked.expression_table.expression_is_valid(expression) {
            return true;
        }
        if visited.contains(&expression) {
            continue;
        }
        visited.push(expression);
        match checked.expression_table.expression(expression) {
            ExpressionNode::Name(name) => {
                if name.symbol == owner
                    || name.head_symbol == owner
                    || checked
                        .expression_table
                        .name_path_member_symbols(name.member_symbols)
                        .contains(&owner)
                {
                    return true;
                }
            }
            ExpressionNode::Binary(binary) => pending.extend([binary.left, binary.right]),
            ExpressionNode::Match(dispatch) => {
                pending.push(dispatch.subject);
                let arms = checked.expression_table.match_arms(dispatch.arms);
                if arms.len() != dispatch.arms.len() {
                    return true;
                }
                for arm in arms {
                    if let checked_trees::expression::MatchPattern::Value(pattern) = arm.pattern {
                        pending.push(pattern);
                    }
                    pending.push(arm.value);
                }
            }
            ExpressionNode::Unary(unary) => pending.push(unary.operand),
            ExpressionNode::Cast(cast) => pending.push(cast.value),
            ExpressionNode::Borrow(borrow) => pending.push(borrow.target),
            ExpressionNode::Member(member) => pending.push(member.receiver),
            ExpressionNode::Indexed(indexed) => pending.extend([indexed.collection, indexed.index]),
            ExpressionNode::Range(range) => pending.extend(
                [range.start, range.end]
                    .into_iter()
                    .filter(|value| value.is_valid()),
            ),
            ExpressionNode::Call(call) => {
                if call.receiver.is_valid() {
                    pending.push(call.receiver);
                }
                pending
                    .extend_from_slice(checked.expression_table.expression_handles(call.arguments));
            }
            ExpressionNode::ArrayLiteral(values) => {
                pending.extend_from_slice(checked.expression_table.expression_handles(*values))
            }
            ExpressionNode::StructLiteral(literal) => pending.extend(
                checked
                    .expression_table
                    .struct_fields(literal.fields)
                    .iter()
                    .map(|field| field.value),
            ),
            ExpressionNode::Atomic(atomic) => pending.extend([atomic.value, atomic.result]),
            ExpressionNode::Boolean(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::UndeterminedNaN(_)
            | ExpressionNode::String(_)
            | ExpressionNode::ZeroValue(_) => {}
        }
    }
    false
}

/// Whether a transition names `owner` anywhere it could carry it into a
/// successor: its guard, either target's successor arguments, or a value
/// target's own expression.
fn transition_mentions_owner(
    checked: &CheckedTrees,
    transition: &checked_trees::statement::TableTransition,
    owner: SymbolHandle,
) -> bool {
    use checked_trees::statement::TransitionGuardNode;
    if let TransitionGuardNode::When(guard) = transition.guard
        && contains_owner(checked, guard, owner)
    {
        return true;
    }
    transition_targets_mention_owner(checked, transition, owner)
}

/// Whether a transition names `owner` in either target's successor arguments
/// or a value target's own expression — the places that would carry it into
/// a successor.
fn transition_targets_mention_owner(
    checked: &CheckedTrees,
    transition: &checked_trees::statement::TableTransition,
    owner: SymbolHandle,
) -> bool {
    use checked_trees::statement::TransitionTargetNode;
    [transition.target, transition.continuation]
        .into_iter()
        .filter(|handle| checked.statement_table.transition_target_is_valid(*handle))
        .any(
            |handle| match checked.statement_table.transition_target(handle) {
                TransitionTargetNode::Named { arguments, .. } => checked
                    .expression_table
                    .expression_handles(*arguments)
                    .iter()
                    .any(|argument| contains_owner(checked, *argument, owner)),
                TransitionTargetNode::Value(expression) => {
                    contains_owner(checked, *expression, owner)
                }
                TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => false,
            },
        )
}

/// A transition spends the alias at the state boundary when its guard
/// mentions the owner only inside member-access receivers — reads of the
/// borrowed referent — and no target carries it into a successor.
fn transition_reads_owner_fields(
    checked: &CheckedTrees,
    transition: &checked_trees::statement::TableTransition,
    owner: SymbolHandle,
) -> bool {
    use checked_trees::statement::TransitionGuardNode;
    matches!(transition.guard, TransitionGuardNode::When(guard)
        if owner_mentions_are_member_reads(checked, guard, owner))
        && !transition_targets_mention_owner(checked, transition, owner)
}

/// Whether every `owner` mention in the expression sits at the head of a
/// member-access receiver chain. A bare mention — an argument, a call
/// receiver, an index or pattern value — would carry the loan itself, so it
/// is not a referent read.
fn owner_mentions_are_member_reads(
    checked: &CheckedTrees,
    expression: ExpressionHandle,
    owner: SymbolHandle,
) -> bool {
    let mut pending = vec![expression];
    let mut visited = Vec::new();
    while let Some(expression) = pending.pop() {
        if visited.contains(&expression) {
            continue;
        }
        visited.push(expression);
        match checked.expression_table.expression(expression) {
            ExpressionNode::Member(member) if matches!(receiver_root(checked, member.receiver), Ok(root) if root == owner) => {
                if !member_receiver_chain_roots_at_owner(checked, member.receiver, owner) {
                    return false;
                }
            }
            ExpressionNode::Name(name) => {
                let members = checked
                    .expression_table
                    .name_path_member_symbols(name.member_symbols);
                if name.head_symbol == owner && !members.is_empty() {
                    continue;
                }
                if name.symbol == owner || name.head_symbol == owner || members.contains(&owner) {
                    return false;
                }
            }
            ExpressionNode::Binary(binary) => pending.extend([binary.left, binary.right]),
            ExpressionNode::Match(dispatch) => {
                pending.push(dispatch.subject);
                let arms = checked.expression_table.match_arms(dispatch.arms);
                if arms.len() != dispatch.arms.len() {
                    return false;
                }
                for arm in arms {
                    if let checked_trees::expression::MatchPattern::Value(pattern) = arm.pattern {
                        pending.push(pattern);
                    }
                    pending.push(arm.value);
                }
            }
            ExpressionNode::Unary(unary) => pending.push(unary.operand),
            ExpressionNode::Cast(cast) => pending.push(cast.value),
            ExpressionNode::Borrow(borrow) => pending.push(borrow.target),
            ExpressionNode::Member(member) => pending.push(member.receiver),
            ExpressionNode::Indexed(indexed) => pending.extend([indexed.collection, indexed.index]),
            ExpressionNode::Range(range) => pending.extend(
                [range.start, range.end]
                    .into_iter()
                    .filter(|value| value.is_valid()),
            ),
            ExpressionNode::Call(call) => {
                if call.receiver.is_valid() {
                    pending.push(call.receiver);
                }
                pending
                    .extend_from_slice(checked.expression_table.expression_handles(call.arguments));
            }
            ExpressionNode::ArrayLiteral(values) => {
                pending.extend_from_slice(checked.expression_table.expression_handles(*values))
            }
            ExpressionNode::StructLiteral(literal) => pending.extend(
                checked
                    .expression_table
                    .struct_fields(literal.fields)
                    .iter()
                    .map(|field| field.value),
            ),
            ExpressionNode::Atomic(atomic) => pending.extend([atomic.value, atomic.result]),
            ExpressionNode::Boolean(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::UndeterminedNaN(_)
            | ExpressionNode::String(_)
            | ExpressionNode::ZeroValue(_) => {}
        }
    }
    true
}

/// Walk a member-access receiver chain to its root `Name`; the chain spells
/// a read of the referent exactly when that root is `owner`. Segment indices
/// inside the receiver are part of the read, but a mention of the owner
/// inside an index is not.
fn member_receiver_chain_roots_at_owner(
    checked: &CheckedTrees,
    expression: ExpressionHandle,
    owner: SymbolHandle,
) -> bool {
    let mut cursor = expression;
    let mut visited = Vec::new();
    loop {
        if visited.contains(&cursor) {
            return false;
        }
        visited.push(cursor);
        match checked.expression_table.expression(cursor) {
            ExpressionNode::Name(name) => {
                return name.head_symbol == owner || name.symbol == owner;
            }
            ExpressionNode::Member(member) => cursor = member.receiver,
            ExpressionNode::Indexed(indexed) => {
                if contains_owner(checked, indexed.index, owner) {
                    return false;
                }
                cursor = indexed.collection;
            }
            _ => return false,
        }
    }
}
