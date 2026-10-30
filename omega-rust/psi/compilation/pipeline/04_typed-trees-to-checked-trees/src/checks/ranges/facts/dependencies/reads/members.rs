//! Member, index, and selector reads: projected places, temporary member
//! symbols, and the operand rosters an operator call reads through.

use validation::CanonicalPlace;
use validation::canonical_place_from_expression_in_state;

use super::super::{EXPRESSION_WALK_DEPTH_BOUND, ExpressionHandle, ExpressionNode, Machine, State};
use super::ReadWalk;

use checked_trees::{
    CheckedOperatorResolutionStatus, CheckedValueOrigin, CheckedValueStatementRole,
};
use symbols::SymbolHandle;

impl ReadWalk<'_, '_, '_> {
    /// A member on a place chain reads its projected place. A member whose
    /// receiver produces a temporary instead of naming storage — `compute().a`,
    /// `Pair { .. }.a`, `compute().inner.a`, `match flag { .. }.a` — reads
    /// exactly whatever producing that temporary read; the projection itself
    /// touches no caller place, so the member expression's canonical place is
    /// expression-rooted and the place path cannot describe it. That shape admits
    /// only when the member still resolves to a declared field of the receiver's
    /// exact type — the identity `temporary_member_symbol` recovers — and the
    /// receiver's own read scan completes, so a missing call-occurrence custody
    /// row or an unproven operand still leaves the set incomplete. A `Borrow`
    /// receiver keeps the place floor either way: `(&x).f` is `x.f`, while a
    /// borrow of a temporary has no statement-use place custody to lend the
    /// projection.
    pub(super) fn collect_member_reads(
        &mut self,
        expression: ExpressionHandle,
        member: &typed_trees::expression::TableMemberExpression,
        depth: usize,
    ) -> bool {
        let program = self.program;
        let machine = self.machine;
        let state = self.state;
        let statement_index = self.statement_index;
        // Builtin length observes the receiver's descriptor, not a declared
        // field named `len`. Capture that storage and its selectors so bounds
        // can follow a selector snapshot and still retire on descriptor writes.
        if let Some(receiver) =
            validation::collection_length_receiver(program, machine, Some(state), expression)
        {
            return self.collect_place_read(receiver, depth + 1);
        }
        let receiver_is_borrow = matches!(
            program.expression_table.expression(member.receiver),
            ExpressionNode::Borrow(_)
        );
        if receiver_is_borrow {
            return self.collect_place_read(expression, depth);
        }
        if let Some(place) = canonical_place_from_expression_in_state(
            program,
            state.symbol,
            statement_index,
            expression,
        ) && matches!(place.root, facts::PlaceRoot::Symbol(_))
        {
            return self.descend_and_capture_symbol_place(expression, place, depth);
        }
        self.temporary_member_symbol(member).is_valid()
            && self.collect_reads(member.receiver, depth + 1)
    }

    /// The declared field identity of a member whose receiver produces a
    /// temporary. `effective_member_symbol` already answers receivers that keep a
    /// place position — names, calls, literals, indexed chains — plus any
    /// binder-stamped `member_symbol`. A `match` receiver has neither: value
    /// dispatch is a control-flow join the member binder never walked, so the
    /// identity must come from the arms' declared result types instead, and the
    /// same holds for a member receiver whose own receiver bottoms out below the
    /// binder's reach — `match { .. }.inner.a` resolves `a` on the declared type
    /// of the `inner` field the arm agreement supplied, so deeper chains keep
    /// resolving while each hop names a declared field of the previous hop's
    /// leaf. An arm whose result type cannot be recovered, leaves that disagree,
    /// a case-qualified hop (whose variant disambiguation the leaf alone cannot
    /// supply), or a member name no field of the agreed declaration owns all stay
    /// unproven. The footprint itself still comes from the receiver's own read
    /// scan — this answers only which field the projection selects.
    fn temporary_member_symbol(
        &self,
        member: &typed_trees::expression::TableMemberExpression,
    ) -> SymbolHandle {
        self.temporary_member_symbol_at(member, 0)
    }

    fn temporary_member_symbol_at(
        &self,
        member: &typed_trees::expression::TableMemberExpression,
        depth: usize,
    ) -> SymbolHandle {
        let program = self.program;
        if depth >= EXPRESSION_WALK_DEPTH_BOUND {
            return SymbolHandle::invalid();
        }
        let direct = validation::effective_member_symbol(program, member.receiver, member);
        if direct.is_valid() || member.case_variant.is_some() {
            return direct;
        }
        let Some(leaf) = self.temporary_receiver_leaf(member.receiver, depth + 1) else {
            return direct;
        };
        validation::resolve_member_symbol_from_type_symbol(program, leaf, member.member.as_str())
            .unwrap_or_else(SymbolHandle::invalid)
    }

    /// The declaration leaf a temporary-producing receiver's value stands on when
    /// the member binder's contextual walk cannot type it. A `match` receiver's
    /// leaf is the arm agreement: the checker's own result oracle is what
    /// `validate_match_dispatch` consults for arm compatibility, so requiring
    /// every arm's recovered leaf to name the same declaration is the
    /// member-level statement of that agreement — judged on leaf symbols rather
    /// than type handles, because separately authored references to one
    /// declaration intern separately. A member receiver whose own receiver is
    /// temporary recurses through the hop's declared type: `match { .. }.inner`
    /// stands on the leaf of the `inner` field the agreement supplied. Receivers
    /// the ordinary walk already types never reach this scan — the caller's
    /// `effective_member_symbol` answer came first — and every other kind keeps
    /// the leaf unproven.
    fn temporary_receiver_leaf(
        &self,
        receiver: ExpressionHandle,
        depth: usize,
    ) -> Option<SymbolHandle> {
        let program = self.program;
        let machine = self.machine;
        let state = self.state;
        if depth >= EXPRESSION_WALK_DEPTH_BOUND
            || !program.expression_table.expression_is_valid(receiver)
        {
            return None;
        }
        match program.expression_table.expression(receiver) {
            ExpressionNode::Match(dispatch) => {
                let mut leaf = SymbolHandle::invalid();
                for arm in program.expression_table.match_arms(dispatch.arms) {
                    let symbol = validation::expression_result_type_reference(
                        program, machine, state, arm.value,
                    )
                    .map(|reference| program.type_reference_table.type_symbol(reference))
                    .filter(|symbol| symbol.is_valid())?;
                    if leaf.is_valid() && leaf != symbol {
                        return None;
                    }
                    leaf = symbol;
                }
                leaf.is_valid().then_some(leaf)
            }
            // An intermediate member hop carries the same honest-identity floor
            // as the leaf member, then stands on that field's declared type. A
            // case-qualified hop cannot be disambiguated by a leaf alone, so the
            // chain stays unproven rather than guessing at a variant.
            ExpressionNode::Member(inner) => {
                let field = self.temporary_member_symbol_at(inner, depth + 1);
                if !field.is_valid() {
                    return None;
                }
                validation::symbol_type_symbol(program, field)
            }
            _ => None,
        }
    }

    /// A selected `[]`/`[..]` application is a checked occurrence, not a place:
    /// the exact `CheckedOperatorUseFact` at this statement authenticates which
    /// declaration the occurrence resolved to, and the operand expressions
    /// recovered from that row enumerate the caller storage the callee can
    /// observe. Missing, ambiguous, or inconsistent selection custody leaves the
    /// read set incomplete — the same evidence floor the checked-call join
    /// applies. Each operand recurses through the ordinary read scan, so a
    /// nested call or a selected nested indexing still has to prove its own
    /// complete footprint.
    pub(super) fn collect_selected_index_reads(
        &mut self,
        expression: ExpressionHandle,
        indexed: &typed_trees::expression::TableIndexedExpression,
        depth: usize,
    ) -> bool {
        use language_core::OperatorSpelling;

        let program = self.program;
        let spelling = if matches!(
            program.expression_table.expression(indexed.index),
            ExpressionNode::Range(_)
        ) {
            OperatorSpelling::Range
        } else {
            OperatorSpelling::Index
        };
        let Some(operands) = self.selected_operator_operands(expression, spelling) else {
            return false;
        };
        operands
            .iter()
            .all(|operand| self.collect_reads(*operand, depth + 1))
    }

    /// The operand expressions a selected operator application hands its
    /// declaration, authenticated by the exact `CheckedOperatorUseFact` at this
    /// statement occurrence. Use rows carry the enclosing statement's origin
    /// even for nested operands, so this join covers selector and
    /// subexpression positions; custody must agree across every recorded row
    /// for the occurrence, resolve to a single valid declaration under the
    /// expected spelling, and retain the candidate roster it was selected
    /// from. The operand count still has to match the retained signature so a
    /// drifted row cannot rename storage the operand scan never saw. Missing,
    /// ambiguous, or inconsistent custody yields no operands — the same
    /// evidence floor the checked-call join applies.
    pub(super) fn selected_operator_operands(
        &self,
        expression: ExpressionHandle,
        spelling: language_core::OperatorSpelling,
    ) -> Option<Vec<ExpressionHandle>> {
        let program = self.program;
        let machine = self.machine;
        let state = self.state;
        let statement_index = self.statement_index;
        let operators = self.operators?;
        let mut uses = operators.uses.iter().filter_map(|(_, selected)| {
            (selected.expression == expression
                && matches!(
                    selected.origin,
                    CheckedValueOrigin::StateStatement {
                        machine_symbol,
                        state_symbol,
                        statement_index: index,
                        ..
                    } if machine_symbol == machine.symbol
                        && state_symbol == state.symbol
                        && index == statement_index
                ))
            .then_some(selected)
        });
        let selected = uses.next()?;
        if selected.spelling != spelling
            || selected.status != CheckedOperatorResolutionStatus::Resolved
            || !selected.selected_operator_symbol.is_valid()
            || selected.candidate_count != operators.candidates(selected).len()
            || uses.any(|other| other != selected)
        {
            return None;
        }
        let candidate = operators.selected_candidate(selected)?;
        let operands = selected.operands(program)?;
        (candidate.parameter_count == operands.len()).then_some(operands)
    }

    /// Check typed identities before contextual spelling recovery, and collect the
    /// storage read to select this place. Do not read the whole collection merely
    /// to address one element: parent replacement overlaps its child path already.
    pub(super) fn descend_and_capture_symbol_place(
        &mut self,
        expression: ExpressionHandle,
        place: CanonicalPlace,
        depth: usize,
    ) -> bool {
        self.collect_selector_reads(expression, depth + 1)
            && self.capture_place_read(place, expression)
    }

    pub(super) fn collect_selector_reads(
        &mut self,
        expression: ExpressionHandle,
        depth: usize,
    ) -> bool {
        let program = self.program;
        let machine = self.machine;
        let state = self.state;
        let statement_index = self.statement_index;
        if depth >= EXPRESSION_WALK_DEPTH_BOUND
            || !program.expression_table.expression_is_valid(expression)
        {
            return false;
        }
        match program.expression_table.expression(expression) {
            ExpressionNode::Name(path) => {
                let members = program.expression_table.name_path_members(path.members);
                let symbols = program
                    .expression_table
                    .name_path_member_symbols(path.member_symbols);
                validation::first_valid_name_path_symbol(path, &program.expression_table).is_some()
                    && (members.len() <= 1 || members.len() == symbols.len())
                    && symbols.iter().all(|symbol| symbol.is_valid())
            }
            ExpressionNode::Member(member) => {
                // The typed member binder only covers `Name`/`Member`/`Indexed`/
                // `StructLiteral` receivers, and it deliberately never stamps a
                // case-qualified projection either: `subject.f@Case` reaches this
                // scan with `member_symbol` unset on every destructure-desugared
                // guard and target-argument read, and the unwalked hop shadows
                // every member stacked above it — `subject.inner@Case.v` leaves
                // `v` unstamped as well because the binder cannot type a
                // case-qualified receiver. For both shapes the honest identity
                // floor is `effective_member_symbol` — the same contextual
                // resolution the canonical-place production stamps into the
                // `Case`/`Field` segments, so the selector gate cannot admit a
                // place the walk below would reject. A member on a receiver the
                // binder did resolve keeps the authored row's own identity: a
                // symbol it saw but left unresolved cannot be recovered from the
                // receiver's spelling.
                (member.member_symbol.is_valid()
                    || ((member.case_variant.is_some()
                        || member_receiver_escapes_binder(program, member.receiver, 0))
                        && validation::effective_member_symbol(program, member.receiver, member)
                            .is_valid()))
                    && self.collect_selector_reads(member.receiver, depth + 1)
            }
            // An explicit `&`/`&mut` inside a place chain addresses exactly its
            // target's place: `(&x).f` reads `x.f`, and both canonical-place
            // productions already peel the borrow the same way. The peel admits
            // no new storage — the target's own selector scan still has to clear
            // the same floor, so `(&call()).f` or a borrow of foreign storage
            // stays incomplete.
            ExpressionNode::Borrow(inner) => self.collect_selector_reads(inner.target, depth + 1),
            ExpressionNode::Indexed(indexed) => {
                // The general bound-meaning query treats places as symbolic leaves.
                // Inspect each selector explicitly through the operand gate: it
                // keeps builtin bound meaning, or it proves an authored
                // arithmetic application through checked custody — and the gate
                // refuses a constant-shaped authored application before
                // syntax-based normalization can mint a fixed element
                // coordinate the selected declaration never established.
                has_builtin_index_meaning(
                    program,
                    machine,
                    state,
                    statement_index,
                    expression,
                    indexed,
                ) && self.collect_selector_reads(indexed.collection, depth + 1)
                    && self.collect_operand_reads(indexed.index, depth)
            }
            _ => false,
        }
    }
}

/// Whether a member's receiver chain escaped the typed member binder, so an
/// unset `member_symbol` on the member marks a hop the binder never typed
/// rather than a name it saw and refused. The binder descends a member chain
/// only while each hop resolves on the previous hop's declared type: a
/// case-qualified hop is never stamped, so every member stacked above it —
/// and above any earlier unwalked hop, recursively — keeps the contextual
/// `effective_member_symbol` floor. An explicit `&`/`&mut` receiver peels
/// before the binder's coverage test, so it escapes the same way. A receiver
/// the binder did resolve (or whose own refusal was reachable) returns false:
/// an unset symbol above it stays a refusal that contextual lookup must not
/// repair — an unqualified `subject.f` naming a payload field, for example,
/// cannot borrow `f@Case`'s identity merely because the name happens to
/// resolve inside one variant.
pub(super) fn member_receiver_escapes_binder(
    program: &dyn ::validation::CheckingInput,
    receiver: ExpressionHandle,
    depth: usize,
) -> bool {
    if depth >= EXPRESSION_WALK_DEPTH_BOUND
        || !program.expression_table.expression_is_valid(receiver)
    {
        return false;
    }
    match program.expression_table.expression(receiver) {
        ExpressionNode::Member(inner) => {
            inner.case_variant.is_some()
                || (!inner.member_symbol.is_valid()
                    && member_receiver_escapes_binder(program, inner.receiver, depth + 1))
        }
        ExpressionNode::Borrow(_) => true,
        _ => false,
    }
}

pub(super) fn has_builtin_index_meaning(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    expression: ExpressionHandle,
    indexed: &typed_trees::expression::TableIndexedExpression,
) -> bool {
    use crate::checks::ranges::types::expression_type_reference;
    use language_core::OperatorSpelling;

    // The occurrence's spelling follows the index shape production recorded:
    // a range index spells `[..]` and resolves against the decomposed
    // collection/start/end operand tuple, not a two-operand `[]` search that
    // cannot see a declared window operator (or lets a scalar `[]`
    // declaration masquerade as governing window syntax).
    let spelling = if indexed.index.is_valid()
        && matches!(
            program.expression_table.expression(indexed.index),
            ExpressionNode::Range(_)
        ) {
        OperatorSpelling::Range
    } else {
        OperatorSpelling::Index
    };
    let operands = validation::indexed_operand_types(
        program,
        indexed,
        CheckedValueOrigin::StateStatement {
            machine_symbol: machine.symbol,
            state_symbol: state.symbol,
            statement_index,
            role: CheckedValueStatementRole::Expression,
        },
    );
    // Successful builtin projection requires array/slice storage geometry; a
    // nominal collection's authored index operation is not a primitive read.
    // A window's result is a slice view rather than an element projection,
    // so `[..]` judges the collection shell itself.
    let builtin_storage = if spelling == OperatorSpelling::Range {
        expression_type_reference(program, machine, state, indexed.collection)
            .and_then(|collection| validation::unwrapped_type_reference(program, collection))
            .is_some_and(|collection| {
                matches!(
                    program.type_reference_table.type_reference(collection),
                    typed_trees::types::TypeReferenceNode::FixedArray { .. }
                        | typed_trees::types::TypeReferenceNode::Slice { .. }
                )
            })
    } else {
        expression_type_reference(program, machine, state, expression).is_some()
    };
    let builtin_operator = if spelling == OperatorSpelling::Index {
        typed_trees::operator::builtin_index_read_is_spelled(
            program,
            machine.symbol,
            expression,
            &operands,
        )
    } else {
        typed_trees::operator::resolve_indexed_spelling_for_operands(
            program,
            spelling,
            &operands,
            program.expression_table.source_span(expression),
        )
        .is_empty()
            && typed_trees::operator::has_builtin_spelled_expression_meaning(
                program,
                machine.symbol,
                expression,
                spelling,
                &operands,
            )
    };
    builtin_storage && builtin_operator
}
