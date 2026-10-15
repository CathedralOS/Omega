//! reference bounds slice of origins.
use validation::CanonicalPlace;

use super::{callee_values::*, leaf_arrivals::*};

use super::FlowStateFact;
use crate::flow::{self};
use facts::{PlaceRoot, PlaceSegment};
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use typed_trees::machine::Machine;
use typed_trees::statement::StatementNode;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(super) fn places_may_overlap(
    program: &dyn ::validation::CheckingInput,
    left: &CanonicalPlace,
    right: &CanonicalPlace,
) -> bool {
    (left.root) == (right.root)
        && flow::canonical_place_segments_may_overlap(program, &left.segments, &right.segments)
}

/// A callee reads a frozen input but cannot change which storage it names:
/// an immutable owned binding or a shared reference whose referent is
/// read-only for the whole call. `&mut`, write-only, and mutable bindings can
/// rebind or rewrite the returned path, so they carry exact caller provenance
/// only when the body's own write scan leaves the demanded projection
/// untouched (`callee_leaves_demanded_path_unwritten`).
pub(super) fn frozen_input_reference(
    program: &dyn ::validation::CheckingInput,
    reference: TypeReferenceHandle,
) -> bool {
    let reference = program
        .type_reference_table
        .unconstrained_type_reference(reference);
    match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Reference { access, .. } => {
            *access == language_semantics::ReferenceAccess::Shared
        }
        _ => true,
    }
}

/// How many reference-boundary hops one demanded place may compose before a
/// cyclic spelling is refused. Each hop resolves one authored boundary — a
/// reference-typed binding, a write-capable leaf's stored origin, or a shared
/// leaf's supplying store — so the bound matches the call-result recursion
/// depth.
pub(super) const REFERENCE_BOUNDARY_HOPS: usize = 16;

/// The per-frontier rebase the shared trace applies to each demanded place.
/// `rebase_exact_local_place` covers a write-capable leaf carried inside an
/// owned local through the prefix's stored origins; a shared leaf has no
/// write origins — nothing may write through it — so
/// `shared_reference_leaf_origin` resolves the store that supplied its
/// referent instead. `bound` is the frontier: one past the statement the scan
/// is examining, so stores at that statement count. An unresolved hop keeps
/// the literal place, which the scan then proves or refuses under its
/// ordinary rules; a resolved place may still cross another boundary, which
/// the next frontier's rebase — or the leaf scan's own relocation — resolves.
pub(super) fn reference_boundary_before_statement<Resolve>(
    program: &dyn ::validation::CheckingInput,
    frames: &validation::CallFrameResolver<'_>,
    machine: &Machine,
    state: &FlowStateFact,
    bound: usize,
    place: &CanonicalPlace,
    resolve: &Resolve,
) -> Option<CanonicalPlace>
where
    Resolve:
        Fn(&FlowStateFact, usize, &TableCallExpression, &[PlaceSegment]) -> Option<CanonicalPlace>,
{
    let mut next = flow::rebase_exact_local_place(
        program,
        state.state_symbol,
        bound,
        place.clone(),
        Some(frames),
    )
    .unwrap_or_else(|| place.clone());
    let exclusive =
        exclusive_reference_root_place(program, frames, machine, state, bound, &next, resolve);
    if let Some(resolved) = exclusive {
        next = resolved;
    }
    if let Some(resolved) =
        shared_reference_leaf_origin(program, frames, machine, state, bound, &next, resolve)
    {
        next = resolved;
    }
    (next != *place).then_some(next)
}

/// A demanded place spelled through an exclusive-binding root —
/// `r.view.scheduler` where `r: &mut RefBox` — names the referent the
/// binding's latest supply proved, but the shared prefix walk cannot always
/// name that referent: an exclusive borrow of a carrier that itself stores
/// shared leaves keeps no alias-table origin, leaving the operand's literal
/// root without a resolvable spelling. This scan replays the binding's own
/// provenance instead: the declaration's operand, then every later bare-name
/// store `r = ..`, each canonicalized the way a stored operand would be — a
/// borrow's target, another binding's name, or a checked call result the
/// resolver proves. A statement between the supply and `bound` whose writes
/// may touch the bare binding — a store the frame cannot name, an operand
/// call's frame, a `r.field` write reachable through the referent — fails
/// closed rather than guessing the referent survived. The referent itself
/// may ride through further reference bindings; each hop repeats the scan at
/// the position where it was captured.

/// The statement's own storage writes must not touch `slot`; `false`
/// reports an untracked touch that fails the walk.
#[allow(clippy::too_many_arguments)]
pub(super) fn statement_writes_miss_slot(
    program: &dyn ::validation::CheckingInput,
    machine_symbol: symbols::SymbolHandle,
    state_symbol: symbols::SymbolHandle,
    index: usize,
    statement: &StatementNode,
    frames: &validation::CallFrameResolver<'_>,
    slot: &CanonicalPlace,
) -> Option<bool> {
    let writes = flow::statement_storage_writes(
        program,
        machine_symbol,
        state_symbol,
        index,
        statement,
        Some(frames),
    )?;
    Some(
        writes
            .iter()
            .all(|write| !places_may_overlap(program, slot, write)),
    )
}

pub(super) fn exclusive_reference_root_place<Resolve>(
    program: &dyn ::validation::CheckingInput,
    frames: &validation::CallFrameResolver<'_>,
    machine: &Machine,
    state: &FlowStateFact,
    bound: usize,
    place: &CanonicalPlace,
    resolve: &Resolve,
) -> Option<CanonicalPlace>
where
    Resolve:
        Fn(&FlowStateFact, usize, &TableCallExpression, &[PlaceSegment]) -> Option<CanonicalPlace>,
{
    let typed_state = program.state_by_symbol(state.state_symbol)?;
    let statements = program
        .statement_table
        .statements(typed_state.statement_nodes);
    let mut demanded = place.clone();
    let mut cursor = bound;
    for _ in 0..REFERENCE_BOUNDARY_HOPS {
        let Some(root) = demanded.root.as_symbol() else {
            return Some(demanded);
        };
        let Some((decl_index, local)) = program
            .state_local_declaration(typed_state, root)
            .filter(|&(ordinal, _)| ordinal < cursor)
        else {
            // A parameter or member root keeps its access path.
            return Some(demanded);
        };
        let shared = match reference_type(program, local.type_reference) {
            None => return Some(demanded),
            Some(language_semantics::ReferenceAccess::Shared) => {
                // A shared binding is the ordinary reference query's subject:
                // ride it while it names the referent. When it declines — a
                // leaf operand read through an indexed carrier is a selector
                // it deliberately keeps coarse — the binding's own provenance
                // still answers exactly, so the replay below applies the same
                // operand-and-rebind discipline an exclusive hop uses.
                if let Some(resolved) = flow::local_reference_storage_before_statement(
                    program,
                    frames,
                    machine,
                    state,
                    cursor,
                    demanded.clone(),
                ) {
                    demanded = resolved;
                    continue;
                }
                true
            }
            _ => false,
        };
        // `slot` is the binding's own storage: only a bare-name assignment
        // may replace it, and any other overlap is an untracked touch.
        let slot = CanonicalPlace {
            root: PlaceRoot::Symbol(root),
            segments: Vec::new(),
        };
        let mut referent = reference_bound_operand_place(
            program,
            frames,
            machine,
            state,
            decl_index,
            local.initial_value,
            resolve,
            shared,
        )?;
        let mut captured = decl_index;
        for (index, statement) in statements
            .get(decl_index + 1..cursor)?
            .iter()
            .enumerate()
            .map(|(offset, statement)| (decl_index + 1 + offset, statement))
        {
            // Operand calls embedded in any statement may reach the binding
            // before its store replays; an unnamed frame fails the hop.
            if statement_may_touch_slot(program, machine, state, index, frames, statement, &slot)? {
                return None;
            }
            if let Some(assignment) = statement.as_assignment() {
                let target = flow::statement_mutated_place(
                    program,
                    machine.symbol,
                    state.state_symbol,
                    index,
                    statement,
                )?;
                if target.root == slot.root && target.segments.is_empty() {
                    // A bare-name store into a reference local either rebinds
                    // it — the referent becomes whatever the store's operand
                    // proves at this position — or writes through it, which
                    // the delegated scans cannot match on this spelling.
                    if frames.assignment_replaces_local_reference_binding(machine, statement)? {
                        referent = reference_bound_operand_place(
                            program,
                            frames,
                            machine,
                            state,
                            index,
                            assignment.value,
                            resolve,
                            shared,
                        )?;
                        captured = index;
                        continue;
                    }
                    return None;
                }
                if places_may_overlap(program, &slot, &target) {
                    return None;
                }
            }
            if !statement_writes_miss_slot(
                program,
                machine.symbol,
                state.state_symbol,
                index,
                statement,
                frames,
                &slot,
            )? {
                return None;
            }
            if let Some(call_statement) = statement.as_call() {
                let writes = flow::frame_storage_writes(
                    program,
                    machine.symbol,
                    state.state_symbol,
                    index,
                    &frames.may_write_frame(machine, call_statement),
                    Some(frames),
                )?;
                if writes
                    .iter()
                    .any(|write| places_may_overlap(program, &slot, write))
                {
                    return None;
                }
            }
        }
        let mut rebased = referent;
        rebased.segments.extend_from_slice(&demanded.segments);
        demanded = rebased;
        cursor = captured;
    }
    None
}

/// The place a reference-binding operand supplies: a borrow's canonicalized
/// target or another binding's name, and a result-position call asks the
/// domain resolver for the caller place its checked callee proves — the same
/// leaf-store discipline `leaf_value_candidate` applies to assignment
/// operands.
pub(super) fn reference_bound_operand_place<Resolve>(
    program: &dyn ::validation::CheckingInput,
    frames: &validation::CallFrameResolver<'_>,
    machine: &Machine,
    state: &FlowStateFact,
    index: usize,
    value: ExpressionHandle,
    resolve: &Resolve,
    shared_binding: bool,
) -> Option<CanonicalPlace>
where
    Resolve:
        Fn(&FlowStateFact, usize, &TableCallExpression, &[PlaceSegment]) -> Option<CanonicalPlace>,
{
    reference_bound_operand_place_segments(
        program,
        frames,
        machine,
        state,
        index,
        value,
        &[],
        resolve,
        shared_binding,
    )
}

/// `reference_bound_operand_place` carrying an additional projection: a
/// constructed literal selects an operand that itself carries the remaining
/// segment list, so the joined walk sees the operand's own place extended by
/// the projection instead of a suffix appended afterward.
pub(super) fn reference_bound_operand_place_segments<Resolve>(
    program: &dyn ::validation::CheckingInput,
    frames: &validation::CallFrameResolver<'_>,
    machine: &Machine,
    state: &FlowStateFact,
    index: usize,
    value: ExpressionHandle,
    extra: &[PlaceSegment],
    resolve: &Resolve,
    shared_binding: bool,
) -> Option<CanonicalPlace>
where
    Resolve:
        Fn(&FlowStateFact, usize, &TableCallExpression, &[PlaceSegment]) -> Option<CanonicalPlace>,
{
    let mut candidate = validation::canonical_place_from_expression_in_state(
        program,
        state.state_symbol,
        index,
        value,
    )?;
    candidate.segments.extend_from_slice(extra);
    match candidate.root {
        PlaceRoot::Symbol(root) => {
            // The operand must name storage this state owns — a local or
            // parameter declared under it, or the machine's own attached
            // data — never a same-spelled symbol living in another body.
            if !(root == machine.symbol || program.symbols.get(root).parent == state.state_symbol) {
                return None;
            }
            // The contextual resolver rejoins an unbound head by spelling,
            // so a foreign or corrupted root leaves the name's own written
            // head/member symbols disagreeing with the adopted root.
            if shared_binding
                && !shared_bound_operand_root_symbols_agree(program, value, candidate.root)
            {
                return None;
            }
            // When the operand's own path ends in a reference leaf —
            // `saved.context` supplying `let borrowed: &mut Context` — the
            // binding's referent is the leaf's pointee, not the slot. A
            // shared leaf's referent is its stored origin via the leaf scan;
            // a write-capable leaf's is the prefix's exact stored origin.
            // Provenance that stays unproven keeps the whole operand unproven.
            let root_type = statements_local_type(program, state, root, index).or_else(|| {
                program
                    .state_by_symbol(state.state_symbol)
                    .and_then(|typed_state| {
                        program
                            .state_parameter_by_symbol(typed_state, root)
                            .map(|parameter| parameter.type_reference)
                    })
            })?;
            let reached = validation::project_type_reference_from_segments(
                program,
                root_type,
                &candidate.segments,
            )?;
            match reference_type(program, reached) {
                Some(language_semantics::ReferenceAccess::Shared) => shared_reference_leaf_origin(
                    program, frames, machine, state, index, &candidate, resolve,
                ),
                Some(_) => flow::rebase_exact_local_place(
                    program,
                    state.state_symbol,
                    index,
                    candidate,
                    Some(frames),
                ),
                // A non-reference operand supplies a shared binding's
                // referent only through an authored borrow — the target's
                // spelling itself names the storage. An owned value read
                // declared `&` by construction is no referent.
                None => {
                    if shared_binding
                        && !matches!(
                            program.expression_table.expression(value),
                            ExpressionNode::Borrow(_)
                        )
                    {
                        return None;
                    }
                    Some(candidate)
                }
            }
        }
        PlaceRoot::Expression(rooted) => match program.expression_table.expression(rooted) {
            ExpressionNode::Call(call) => resolve(state, index, call, &candidate.segments),
            // A record literal has no storage of its own: the demanded leaf
            // arrives from the operand bound to that exact field, which then
            // proves a referent under the same operand rules.
            // `literal_expression_projections` keeps the operand
            // exact — a case-qualified, dynamic, or ranged selection yields
            // several candidates and stays unproven — and the chosen operand
            // re-enters this same provenance walk so a nested literal or a
            // leaf store supplies it.
            ExpressionNode::StructLiteral(_) => {
                let mut projections = flow::literal_expression_projections(
                    program,
                    rooted,
                    literal_operand_type(program, rooted, None)?,
                    &candidate.segments,
                    false,
                )?;
                if projections.len() != 1 {
                    return None;
                }
                let projection = projections.remove(0);
                // A selection that lands exactly on a nested literal returns
                // that literal itself: no leaf is selected, and re-entering
                // it would loop without naming a referent.
                if projection.expression == rooted {
                    return None;
                }
                reference_bound_operand_place_segments(
                    program,
                    frames,
                    machine,
                    state,
                    index,
                    projection.expression,
                    &projection.remaining,
                    resolve,
                    shared_binding,
                )
            }
            // An array literal has no named-type row to project against, so
            // only a leading literal `FixedIndex` selects an element here;
            // a dynamic or ranged selection names several operands and stays
            // unproven.
            ExpressionNode::ArrayLiteral(elements) => {
                let Some((PlaceSegment::FixedIndex { index: selected }, remaining)) =
                    candidate.segments.split_first()
                else {
                    return None;
                };
                let element = *program
                    .expression_table
                    .expression_handles(*elements)
                    .get(*selected)?;
                reference_bound_operand_place_segments(
                    program,
                    frames,
                    machine,
                    state,
                    index,
                    element,
                    remaining,
                    resolve,
                    shared_binding,
                )
            }
            _ => None,
        },
        _ => None,
    }
}

/// Whether the written root symbols on `value`'s root name agree with the
/// symbol `root` resolved to. The contextual canonicalizer rejoins an unbound
/// head by name spelling, so a corrupted or foreign `head_symbol` still
/// produces the in-scope root while the name's own written identity disagrees
/// with it; a shared binding may not lend that adopted identity a referent.
pub(super) fn shared_bound_operand_root_symbols_agree(
    program: &dyn ::validation::CheckingInput,
    value: ExpressionHandle,
    root: PlaceRoot,
) -> bool {
    let Some(resolved) = root.as_symbol() else {
        return true;
    };
    let mut current = value;
    loop {
        match program.expression_table.expression(current) {
            ExpressionNode::Borrow(borrow) => current = borrow.target,
            ExpressionNode::Member(member) => current = member.receiver,
            ExpressionNode::Indexed(indexed) => current = indexed.collection,
            ExpressionNode::Name(path) => {
                // `path.symbol` names the root on a single-member spelling;
                // on a dotted path it may name the leaf instead.
                let single_member = program
                    .expression_table
                    .name_path_members(path.members)
                    .len()
                    == 1;
                return [Some(path.head_symbol), single_member.then_some(path.symbol)]
                    .into_iter()
                    .flatten()
                    .chain(
                        program
                            .expression_table
                            .name_path_member_symbols(path.member_symbols)
                            .first()
                            .copied(),
                    )
                    .all(|symbol| !symbol.is_valid() || symbol == resolved);
            }
            _ => return true,
        }
    }
}
