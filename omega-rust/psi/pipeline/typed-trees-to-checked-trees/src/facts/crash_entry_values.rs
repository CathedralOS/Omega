//! Relate saved crash operands to invocation-entry values.
//!
//! Ordinary calls and selected operators share this substitution boundary.
//! A stable binding is insufficient when its contents contain mutable loans or
//! interior authority. Reuse stable-observation validation before projecting
//! fields; shared loans may retain immutable contents without owning them.
//! State parameters resolve through every arrival that binds them: the
//! invocation itself for the entry state, plus each named transition edge
//! into the state, which must all produce the same entry-relative operand.
//! A mutable binding additionally keeps that bound snapshot only while its
//! storage is pristine: a write, an exclusive borrow, or a mutable receiver
//! call before the read (or before a `-> self`/same-state forwarding edge
//! that carries the storage into the next arrival) ends provenance. Field
//! projections version the storage below the binding root, so a field read
//! survives writes confined to disjoint siblings. The receiver is bound once
//! at the invocation and never rebound, so a mutable receiver's field keeps
//! its entry identity only while no statement that can precede the read can
//! write through it — the window spans every state that still reaches the
//! read's state, not one arrival's prefix.
//! Mutable bindings with unstable contents, divergent arrivals and
//! unresolvable cycles retain no entry identity. Substitution transports a
//! proven origin, never re-reads an initializer after later operands execute.
//! A guard leaf that reads only a member projection needs only that
//! projection's provenance: whole-operand failure is not a reason to widen a
//! surviving route to `Truth` when the read field's snapshot is intact.
//! Member identity comes from the same contextual resolver the canonical
//! place algebra uses, so a destructure-bound payload projection keeps its
//! `subject.Case::field` steps and a produced operand names
//! `receiver.Case::field` — same-named payload fields of different cases stay
//! distinct. This is source provenance, not a Terminal certificate.

use checked_trees::CrashPredicateExpression;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableNamePath};
use typed_trees::statement::{StatementNode, TransitionTargetNode};
use typed_trees::types::TypeReferenceNode;
use validation::has_stable_observable_contents;

mod mutable;
pub(super) use mutable::statement_may_overwrite_place;
use mutable::{PlaceSegment, storage_holds_bound_value};

/// Arrival provenance can revisit a state parameter through a transition
/// cycle, so the fold carries a depth bound. Exhaustion is unproven
/// provenance, never an affirmed origin.
const MAX_ENTRY_PROVENANCE_DEPTH: u32 = 64;

#[cfg(test)]
mod tests;

pub(super) fn entry_operand(
    program: &TypedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    before_statement: usize,
    expression: ExpressionHandle,
) -> Option<CrashPredicateExpression> {
    entry_operand_at(
        program,
        machine_symbol,
        state_symbol,
        before_statement,
        expression,
        0,
    )
}

/// Whether `operand`'s storage still proves the invocation-entry value at the
/// place `leaf` projects below its own root. `leaf` is a place occurrence
/// inside a crash guard and roots at one of the operator's parameter symbols;
/// `operand` is the caller expression bound to that parameter. A bare `cell`
/// leaf asks for the operand's whole storage — the same boundary
/// `entry_operand` draws — while `cell.count` asks only about the `count`
/// projection, so writes confined to disjoint sibling storage keep the
/// provenance the guard actually reads. The operand's own field steps prepend
/// to the leaf's projection, so a leaf `cell.count` bound to operand
/// `pair.cell` asks about `pair.cell.count`. Anything the rooted-place scan
/// cannot separate — an opaque projection, an unresolvable member, a
/// non-shared borrow, or a non-place operand carrying a non-empty projection
/// — refuses the same way the whole-operand path did.
pub(super) fn operand_entry_provenance(
    program: &TypedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    before_statement: usize,
    operand: ExpressionHandle,
    leaf: ExpressionHandle,
) -> bool {
    let mut projection = Vec::new();
    let mut leaf_place = leaf;
    loop {
        if !program.expression_table.expression_is_valid(leaf_place) {
            return false;
        }
        match program.expression_table.expression(leaf_place) {
            ExpressionNode::Member(member) => {
                // The walk collects leaf-to-root and reverses below, so push
                // the field step before its case hop. A member the shared
                // resolver cannot place is opaque below its receiver.
                match member_hop_path(program, member) {
                    Some((_, hop)) => {
                        for segment in hop.into_iter().rev() {
                            projection.push(segment);
                        }
                    }
                    None => projection.push(PlaceSegment::Opaque),
                }
                leaf_place = member.receiver;
            }
            ExpressionNode::Indexed(indexed) => {
                projection.push(PlaceSegment::Opaque);
                leaf_place = indexed.collection;
            }
            ExpressionNode::Borrow(borrow) => leaf_place = borrow.target,
            ExpressionNode::Name(_) => break,
            // The occurrence did not root at a name, so no operand can carry
            // its entry identity.
            _ => return false,
        }
    }
    projection.reverse();

    let mut operand_place = operand;
    loop {
        if !program.expression_table.expression_is_valid(operand_place) {
            return false;
        }
        match program.expression_table.expression(operand_place) {
            ExpressionNode::Member(member) => {
                // The operand's own member spine sits above the leaf's
                // projection; prepend each hop root-first. An unresolvable
                // member is not a separable place, so the operand keeps no
                // entry identity.
                let Some((_, hop)) = member_hop_path(program, member) else {
                    return false;
                };
                for segment in hop.into_iter().rev() {
                    projection.insert(0, segment);
                }
                operand_place = member.receiver;
            }
            ExpressionNode::Borrow(borrow)
                if borrow.access == language_core::ReferenceAccess::Shared =>
            {
                operand_place = borrow.target
            }
            ExpressionNode::Name(path) => {
                return entry_operand_name_at(
                    program,
                    machine_symbol,
                    state_symbol,
                    before_statement,
                    path,
                    &projection,
                    0,
                )
                .is_some();
            }
            // A non-place operand carries only whole-value provenance, and
            // only when the leaf read it without projecting.
            _ => {
                return projection.is_empty()
                    && entry_operand_at(
                        program,
                        machine_symbol,
                        state_symbol,
                        before_statement,
                        operand,
                        0,
                    )
                    .is_some();
            }
        }
    }
}

/// The operand's entry operand when only `leaf_projection` below its root is
/// read — the expression-returning twin of `operand_entry_provenance`. A
/// mutable binding written only outside the read projection still supplies
/// the entry snapshot for that field, so a surviving route may keep the
/// caller's per-field name instead of widening to `Truth`. The produced
/// expression names only the operand's own member spine; the leaf's
/// projection is applied by the substituting `Member` nodes above the
/// `Parameter` leaf, never doubled here.
pub(super) fn entry_operand_projected(
    program: &TypedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    before_statement: usize,
    operand: ExpressionHandle,
    leaf_projection: &[PlaceSegment],
) -> Option<CrashPredicateExpression> {
    let mut projection = leaf_projection.to_vec();
    let mut member_names = Vec::new();
    let mut operand_place = operand;
    loop {
        if !program.expression_table.expression_is_valid(operand_place) {
            return None;
        }
        match program.expression_table.expression(operand_place) {
            ExpressionNode::Member(member) => {
                // An unresolvable member is not a separable place, so the
                // operand keeps no entry identity.
                let (symbol, hop) = member_hop_path(program, member)?;
                member_names.insert(0, member_entry_name(program, member, symbol));
                for segment in hop.into_iter().rev() {
                    projection.insert(0, segment);
                }
                operand_place = member.receiver;
            }
            ExpressionNode::Borrow(borrow)
                if borrow.access == language_core::ReferenceAccess::Shared =>
            {
                operand_place = borrow.target;
            }
            ExpressionNode::Name(path) => {
                let root = entry_operand_name_at(
                    program,
                    machine_symbol,
                    state_symbol,
                    before_statement,
                    path,
                    &projection,
                    0,
                )?;
                return Some(member_names.iter().fold(root, |receiver, member| {
                    CrashPredicateExpression::Member {
                        receiver: Box::new(receiver),
                        member: member.clone(),
                    }
                }));
            }
            // A non-place operand carries only whole-value provenance, and
            // only when the leaf read it without projecting.
            _ => {
                return if projection.is_empty() {
                    entry_operand_at(
                        program,
                        machine_symbol,
                        state_symbol,
                        before_statement,
                        operand,
                        0,
                    )
                } else {
                    None
                };
            }
        }
    }
}

/// The place steps one member hop adds below its receiver, in root-to-leaf
/// order: the resolved field, preceded by its declaring case variant for a
/// payload field — matching `facts::payload_variant_for_field`'s canonical
/// spelling. Synthesized members — the destructure-bound payload projections
/// `subject.Case::field` — retain no `member_symbol`, so identity comes from
/// the shared contextual resolver the canonical place algebra uses, never
/// from the first same-named field. `None` when the member does not resolve
/// to a declared field.
///
/// A builtin extent read (`s.len`) resolves no member symbol either, but its
/// operand's entry observation is still exact: the binding's entry snapshot
/// supplies the extent, and the by-name `Member` spelling the callee's own
/// leaf carries (`Member { receiver, "len" }`) survives substitution
/// unchanged — typing keeps the actual's `len` the same extent read as the
/// formal's. The hop is one opaque projection: `paths_interfere` treats it
/// as reaching the whole binding, so any write that could rebind or resize
/// the receiver ends a mutable formal's stored-extent provenance, and the
/// leaf/walks the opaque member already degrade the same way. The returned
/// symbol stays invalid so `member_entry_name` keeps the authored `len`
/// name. Other unresolvable members still admit no hop at all.
fn member_hop_path(
    program: &TypedTrees,
    member: &typed_trees::expression::TableMemberExpression,
) -> Option<(SymbolHandle, Vec<PlaceSegment>)> {
    let symbol = crate::flow::effective_member_symbol(program, member.receiver, member);
    if !symbol.is_valid() {
        return (member.case_variant.is_none() && member.member.as_str() == "len")
            .then(|| (SymbolHandle::invalid(), vec![PlaceSegment::Opaque]));
    }
    let mut path = Vec::with_capacity(2);
    if let Some(variant) = facts::payload_variant_for_field(program, symbol) {
        path.push(PlaceSegment::Case(variant));
    }
    path.push(PlaceSegment::Field(symbol));
    Some((symbol, path))
}

/// The member name a produced entry operand carries. A case payload field
/// keeps its `Variant::field` qualification — the retained `case_variant`
/// spelling when present, else the field's declaring variant — so two
/// same-named payload fields of different cases cannot collide in the caller
/// namespace.
fn member_entry_name(
    program: &TypedTrees,
    member: &typed_trees::expression::TableMemberExpression,
    field_symbol: SymbolHandle,
) -> String {
    let Some(variant) = facts::payload_variant_for_field(program, field_symbol) else {
        return member.member.as_str().to_owned();
    };
    let case = member
        .case_variant
        .as_ref()
        .map(|case| case.as_str())
        .unwrap_or_else(|| program.symbols.name(variant));
    format!("{case}::{}", member.member.as_str())
}

/// The `PlaceSegment` projection `members` spells below a formal's declared
/// type. Each member resolves against the previous step's data declaration so
/// the pristine-storage scan compares the same field symbols writes carry; a
/// member that does not resolve to a plain data field — a case payload, an
/// unresolvable or non-record step — is opaque and interferes with every
/// write that reaches it. Resolution stops at the first opaque step: nothing
/// below it can be separated anyway.
pub(super) fn formal_member_projection(
    program: &TypedTrees,
    mut type_reference: typed_trees::types::TypeReferenceHandle,
    members: &[String],
) -> Vec<PlaceSegment> {
    let mut projection = Vec::with_capacity(members.len());
    for member in members {
        let data_symbol = loop {
            match program.type_reference_table.type_reference(type_reference) {
                TypeReferenceNode::Reference { referee, .. } => type_reference = *referee,
                TypeReferenceNode::Constrained { base_type, .. } => type_reference = *base_type,
                TypeReferenceNode::Named { symbol, .. }
                | TypeReferenceNode::Generic {
                    base_symbol: symbol,
                    ..
                } => break *symbol,
                _ => break SymbolHandle::invalid(),
            }
        };
        let mut owners = program
            .data_definitions()
            .iter()
            .filter(|data| data.symbol == data_symbol);
        let field = owners.next().and_then(|data| {
            (owners.next().is_none())
                .then(|| {
                    program
                        .data_members(data)
                        .iter()
                        .find_map(|candidate| match candidate {
                            typed_trees::data::DataMember::Field(field)
                                if field.name.as_str() == member.as_str() =>
                            {
                                Some(field)
                            }
                            _ => None,
                        })
                })
                .flatten()
        });
        let Some(field) = field else {
            projection.push(PlaceSegment::Opaque);
            break;
        };
        projection.push(PlaceSegment::Field(field.symbol));
        type_reference = field.type_reference;
    }
    projection
}

fn entry_operand_at(
    program: &TypedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    before_statement: usize,
    expression: ExpressionHandle,
    depth: u32,
) -> Option<CrashPredicateExpression> {
    if depth >= MAX_ENTRY_PROVENANCE_DEPTH
        || !program.expression_table.expression_is_valid(expression)
    {
        return None;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Boolean(value) => Some(CrashPredicateExpression::Boolean(*value)),
        ExpressionNode::Integer(value) => {
            Some(CrashPredicateExpression::Integer(value.text().to_owned()))
        }
        // A float literal is already an entry value: its closed spelling
        // transports like the integer literal beside it.
        ExpressionNode::Float(value) => {
            Some(CrashPredicateExpression::Float(value.text().to_owned()))
        }
        ExpressionNode::Unary(unary)
            if unary.operator == typed_trees::expression::UnaryOperator::LogicalNot =>
        {
            Some(CrashPredicateExpression::Unary {
                operator: unary.operator as u8,
                operand: Box::new(entry_operand_at(
                    program,
                    machine_symbol,
                    state_symbol,
                    before_statement,
                    unary.operand,
                    depth + 1,
                )?),
            })
        }
        ExpressionNode::Borrow(borrow)
            if borrow.access == language_core::ReferenceAccess::Shared =>
        {
            // A shared borrow supplies access, not storage: every place a
            // guard observes through the reference is a place of the
            // referent, so the operand's entry identity is the referent's.
            // The referent's own pristine-storage window already covers the
            // containing statement — including this borrow expression — so a
            // mutable referent keeps provenance only while nothing in that
            // window could have disturbed it. Exclusive and write-only
            // loans keep no entry identity: the loan itself ends the
            // referent's bound-snapshot window, and an immutable referent
            // cannot form one at all.
            entry_operand_at(
                program,
                machine_symbol,
                state_symbol,
                before_statement,
                borrow.target,
                depth + 1,
            )
        }
        ExpressionNode::Member(member) => {
            // Walk the contiguous member projection down to its base so a
            // mutable root's pristine-storage check can version the bound
            // snapshot per field: a sibling write does not overwrite this
            // projection. A case payload hop keeps its variant step — a
            // destructure-bound operand resolves through `value.Case::field`
            // the same way the canonical place algebra does — while an
            // unresolvable member keeps its own resolution boundary.
            let (symbol, hop) = member_hop_path(program, member)?;
            let mut segments: Vec<PlaceSegment> = hop.into_iter().rev().collect();
            let mut member_names = vec![member_entry_name(program, member, symbol)];
            let mut base = member.receiver;
            loop {
                if !program.expression_table.expression_is_valid(base) {
                    return None;
                }
                match program.expression_table.expression(base) {
                    ExpressionNode::Member(inner) => {
                        let Some((symbol, hop)) = member_hop_path(program, inner) else {
                            break;
                        };
                        for segment in hop.into_iter().rev() {
                            segments.push(segment);
                        }
                        member_names.push(member_entry_name(program, inner, symbol));
                        base = inner.receiver;
                    }
                    _ => break,
                }
            }
            segments.reverse();
            member_names.reverse();
            let root = match program.expression_table.expression(base) {
                ExpressionNode::Name(path) => entry_operand_name_at(
                    program,
                    machine_symbol,
                    state_symbol,
                    before_statement,
                    path,
                    &segments,
                    depth + 1,
                )?,
                _ => entry_operand_at(
                    program,
                    machine_symbol,
                    state_symbol,
                    before_statement,
                    base,
                    depth + 1,
                )?,
            };
            Some(member_names.iter().fold(root, |receiver, member| {
                CrashPredicateExpression::Member {
                    receiver: Box::new(receiver),
                    member: member.clone(),
                }
            }))
        }
        ExpressionNode::Indexed(indexed) => {
            // An indexed read is a value projection like `Member`: the
            // operand's entry identity is the collection's entry storage
            // read at the index's entry value. The collection resolves at
            // its whole root — element writes cannot be separated below it
            // (`PlaceSegment::Opaque`), so a mutable collection's bound
            // snapshot must hold across its entire storage. The domain-free
            // reducers cannot fold an `Indexed` node, so transporting it
            // cannot substitute builtin meaning for a caller-authored `[]`
            // selection, the same reason arithmetic crosses unconditionally.
            Some(CrashPredicateExpression::Indexed {
                collection: Box::new(entry_operand_at(
                    program,
                    machine_symbol,
                    state_symbol,
                    before_statement,
                    indexed.collection,
                    depth + 1,
                )?),
                index: Box::new(entry_operand_at(
                    program,
                    machine_symbol,
                    state_symbol,
                    before_statement,
                    indexed.index,
                    depth + 1,
                )?),
            })
        }
        ExpressionNode::Range(range) => {
            // A range bound is an evaluated operand like any other leaf:
            // each bound must independently resolve at entry, while the
            // inclusivity flag transports verbatim since bounds alone do
            // not distinguish `..` from `..=`.
            Some(CrashPredicateExpression::Range {
                start: Box::new(entry_operand_at(
                    program,
                    machine_symbol,
                    state_symbol,
                    before_statement,
                    range.start,
                    depth + 1,
                )?),
                end: Box::new(entry_operand_at(
                    program,
                    machine_symbol,
                    state_symbol,
                    before_statement,
                    range.end,
                    depth + 1,
                )?),
                end_inclusive: range.end_inclusive,
            })
        }
        ExpressionNode::Binary(binary)
            if matches!(
                binary.operator,
                typed_trees::expression::BinaryOperator::Add
                    | typed_trees::expression::BinaryOperator::Subtract
                    | typed_trees::expression::BinaryOperator::Multiply
                    | typed_trees::expression::BinaryOperator::Divide
                    | typed_trees::expression::BinaryOperator::Modulo
                    | typed_trees::expression::BinaryOperator::BitwiseAnd
                    | typed_trees::expression::BinaryOperator::BitwiseOr
                    | typed_trees::expression::BinaryOperator::BitwiseXor
                    | typed_trees::expression::BinaryOperator::ShiftLeft
                    | typed_trees::expression::BinaryOperator::ShiftRight
            ) || (matches!(
                binary.operator,
                typed_trees::expression::BinaryOperator::And
                    | typed_trees::expression::BinaryOperator::Or
                    | typed_trees::expression::BinaryOperator::Equal
                    | typed_trees::expression::BinaryOperator::NotEqual
                    | typed_trees::expression::BinaryOperator::Less
                    | typed_trees::expression::BinaryOperator::LessOrEqual
                    | typed_trees::expression::BinaryOperator::Greater
                    | typed_trees::expression::BinaryOperator::GreaterOrEqual
            ) && builtin_binary_meaning(
                program,
                machine_symbol,
                state_symbol,
                expression,
            )) =>
        {
            // Only value-producing arithmetic crosses this boundary
            // unconditionally. The domain-free predicate reducers can never
            // fold these operators, so transporting them cannot substitute
            // builtin meaning for a caller-authored one; the checked scalar
            // channel remains the only evaluation authority over the
            // substituted expression. Comparisons and logical connectives are
            // decidable by those same reducers, so they transport only when
            // this occurrence already selected builtin meaning — a custom
            // operator spelled like one would otherwise be folded under laws
            // its selection rejected.
            Some(CrashPredicateExpression::Binary {
                operator: binary.operator as u8,
                left: Box::new(entry_operand_at(
                    program,
                    machine_symbol,
                    state_symbol,
                    before_statement,
                    binary.left,
                    depth + 1,
                )?),
                right: Box::new(entry_operand_at(
                    program,
                    machine_symbol,
                    state_symbol,
                    before_statement,
                    binary.right,
                    depth + 1,
                )?),
            })
        }
        ExpressionNode::Name(path) => entry_operand_name_at(
            program,
            machine_symbol,
            state_symbol,
            before_statement,
            path,
            &[],
            depth,
        ),
        _ => None,
    }
}

/// A single-member name's entry operand. `field_path` is the field projection
/// below this binding that the enclosing `Member` chain reads: for a mutable
/// local or parameter the pristine-storage window only has to keep that
/// projection unwritten, since sibling fields version independently.
fn entry_operand_name_at(
    program: &TypedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    before_statement: usize,
    path: &TableNamePath,
    field_path: &[PlaceSegment],
    depth: u32,
) -> Option<CrashPredicateExpression> {
    if program
        .expression_table
        .name_path_members(path.members)
        .len()
        != 1
        || !path.symbol.is_valid()
        || path.head_symbol != path.symbol
    {
        return None;
    }
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)?;
    let state = program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == state_symbol)?;
    let preceding = program
        .statement_table
        .statements(state.statement_nodes)
        .get(..before_statement)?;
    for (ordinal, statement) in preceding.iter().enumerate() {
        if let typed_trees::statement::StatementNode::LocalData(local) = statement
            && local.symbol == path.symbol
        {
            if !has_stable_observable_contents(program, local.type_reference)
                || (local.is_mutable
                    && !storage_holds_bound_value(
                        program,
                        machine_symbol,
                        state,
                        ordinal + 1,
                        before_statement.saturating_add(1),
                        local.symbol,
                        field_path,
                    ))
            {
                return None;
            }
            // This transports the bound value, not a current read of its
            // storage. A mutable local is admitted only while no statement
            // between its initializer and this read could have overwritten
            // the read projection or lent it exclusive access; the containing
            // statement itself stays in the window because its earlier
            // operands may already have run a call that writes through an
            // exclusive borrow. Every initializer dependency must
            // independently be entry-relative. Decreasing the prefix also
            // prevents recursive aliases.
            return entry_operand_at(
                program,
                machine_symbol,
                state_symbol,
                ordinal,
                local.initial_value,
                depth + 1,
            );
        }
    }
    state_parameter_entry_operand(
        program,
        machine,
        machine_symbol,
        state_symbol,
        receiver_parameter_symbol(program, machine, state, path).unwrap_or(path.symbol),
        before_statement,
        field_path,
        depth,
    )
}

/// `self` names the containing machine (or its attached data), not the
/// receiver's telescope row — the same resolution
/// `structural_fields::exact_self_parameter` applies for authored contract
/// predicates. Bind the name to the state's `is_self` parameter so receiver
/// projections take the parameter path below.
fn receiver_parameter_symbol(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    path: &TableNamePath,
) -> Option<SymbolHandle> {
    let [member] = program.expression_table.name_path_members(path.members) else {
        return None;
    };
    if member.as_str() != "self"
        || (path.symbol != machine.symbol && path.symbol != machine.attached_data_symbol)
    {
        return None;
    }
    program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.is_self)
        .map(|parameter| parameter.symbol)
}

/// Whether the receiver's storage — the machine's attached data — is stable
/// under shared observation. A `self` parameter's type names the machine
/// through `Self`, which the contents classifier cannot resolve to the data
/// declaration, so this checks the declaration itself: the exact resolved
/// owner application for a generic attachment, else each member's declared
/// type under the same root gates `check_contents` applies (no linear
/// custody, no attached `::drop` machine, no erased-relevance fields). Cycles
/// stay inside the per-field call's active set, so member-by-member checking
/// cannot diverge.
fn receiver_contents_stable(program: &TypedTrees, machine: &typed_trees::machine::Machine) -> bool {
    if machine.attached_data_application.is_valid() {
        return has_stable_observable_contents(program, machine.attached_data_application);
    }
    let mut owners = program
        .data_definitions()
        .iter()
        .filter(|data| data.symbol == machine.attached_data_symbol);
    let Some(data) = owners.next() else {
        return false;
    };
    if owners.next().is_some()
        || !program.data_type_parameters(data).is_empty()
        || data.properties.multiplicity == language_semantics::Multiplicity::Linear
        || program.machines().iter().any(|candidate| {
            candidate.attached_data_symbol == data.symbol
                && candidate.name.as_str().ends_with("::drop")
        })
    {
        return false;
    }
    program.data_members(data).iter().all(|member| {
        let fields: &[typed_trees::data::DataField] = match member {
            typed_trees::data::DataMember::Field(field) => std::slice::from_ref(field),
            typed_trees::data::DataMember::Variant(variant) => program.data_payload_fields(variant),
        };
        fields.iter().all(|field| {
            !field.relevance.is_erased()
                && has_stable_observable_contents(program, field.type_reference)
        })
    })
}

fn builtin_binary_meaning(
    program: &TypedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    expression: ExpressionHandle,
) -> bool {
    let Some(machine) = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
    else {
        return false;
    };
    let state = program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == state_symbol);
    validation::has_builtin_binary_expression_meaning(program, machine, state, expression)
}

/// A state parameter's saved actual is whatever every arrival binds to it:
/// the invocation for the entry state, and each named transition edge into
/// the state positionally. `-> self` forwards the current values; for an
/// immutable parameter that is always the bound snapshot, while a mutable
/// parameter's storage must still be pristine at the edge. A by-name edge
/// forwarding this same parameter back to its own state is tautological under
/// the same rule. Every remaining edge must resolve to one identical
/// entry-relative operand, or provenance stays unknown rather than picking a
/// winner. For a mutable parameter the read itself and every self-referential
/// edge must keep the projected `field_path` pristine — the produced `Member`
/// operand asserts only that projection is uniform across arrivals, never
/// that the whole bound record is.
fn state_parameter_entry_operand(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    parameter_symbol: SymbolHandle,
    before_statement: usize,
    field_path: &[PlaceSegment],
    depth: u32,
) -> Option<CrashPredicateExpression> {
    let states = program.machine_states(machine);
    let state_index = states
        .iter()
        .position(|state| state.symbol == state_symbol)?;
    let state = &states[state_index];
    let parameters = program.state_parameters(state);
    let (parameter_ordinal, parameter) = parameters
        .iter()
        .enumerate()
        .find(|(_, parameter)| parameter.symbol == parameter_symbol)?;
    let entry_index = crate::checks::termination::named_transition_target_state_index(
        program,
        machine,
        machine.symbol,
    )?;
    if parameter.is_self {
        // `self` binds once, at the invocation: transitions never rebind the
        // receiver, so an immutable receiver's storage is the entry storage in
        // every state — including through field projections. A mutable
        // receiver keeps entry identity only below a field projection no
        // statement that can precede the read can write through: `self.<field>`
        // writes root at the field rather than this parameter, and any earlier
        // arrival may already have run any state that still reaches this one,
        // so the pristine-storage window is bounded by re-entrancy
        // reachability (`mutable::receiver_field_holds_entry_value` — the
        // write-escape rule). The produced `Parameter` names the
        // receiver's position in the ENTRY state's telescope, matching the
        // ordinal authored `self.<field>` contract predicates take through
        // `parameter_names`.
        if !receiver_contents_stable(program, machine) {
            return None;
        }
        let entry_ordinal = program
            .state_parameters(&states[entry_index])
            .iter()
            .position(|candidate| candidate.is_self)?;
        if parameter.is_mutable
            && !mutable::receiver_field_holds_entry_value(
                program,
                machine,
                state_symbol,
                before_statement,
                field_path,
            )
        {
            return None;
        }
        return Some(CrashPredicateExpression::Parameter(
            u32::try_from(entry_ordinal).ok()?,
        ));
    }
    if !has_stable_observable_contents(program, parameter.type_reference)
        || (parameter.is_mutable
            && !storage_holds_bound_value(
                program,
                machine_symbol,
                state,
                0,
                before_statement.saturating_add(1),
                parameter_symbol,
                field_path,
            ))
    {
        return None;
    }
    let mutable = parameter.is_mutable;
    // Transition arguments bind only the non-self parameters, in order.
    let argument_index = parameters[..parameter_ordinal]
        .iter()
        .filter(|parameter| !parameter.is_self)
        .count();
    // The invocation itself is the entry state's one non-transition arrival.
    let mut provenance = if state_index == entry_index {
        Some(CrashPredicateExpression::Parameter(
            u32::try_from(parameter_ordinal).ok()?,
        ))
    } else {
        None
    };
    for (source_symbol, statement_ordinal, argument) in
        named_transition_arguments(program, machine, state_index, argument_index)
    {
        if source_symbol == state_symbol
            && program.expression_table.expression_is_valid(argument)
            && let ExpressionNode::Name(forwarded) = program.expression_table.expression(argument)
            && forwarded.symbol == parameter_symbol
            && forwarded.head_symbol == parameter_symbol
            && program
                .expression_table
                .name_path_members(forwarded.members)
                .len()
                == 1
        {
            // The edge forwards the parameter's current storage back into its
            // own arrival slot. That is tautological only while the read
            // projection still holds the bound value at the edge — a sibling
            // field may change between arrivals without moving this operand.
            // The edge's own argument expressions count because they evaluate
            // at this point.
            if mutable
                && !storage_holds_bound_value(
                    program,
                    machine_symbol,
                    state,
                    0,
                    statement_ordinal.saturating_add(1),
                    parameter_symbol,
                    field_path,
                )
            {
                return None;
            }
            continue;
        }
        let resolved = entry_operand_at(
            program,
            machine_symbol,
            source_symbol,
            statement_ordinal,
            argument,
            depth + 1,
        )?;
        if let Some(existing) = provenance.as_ref() {
            if *existing != resolved {
                return None;
            }
        } else {
            provenance = Some(resolved);
        }
    }
    if mutable {
        // `-> self` carries the current storage into the next arrival, so the
        // bound snapshot survives only while the read projection is still
        // pristine at every self edge — including the edge's own evaluated
        // arguments. A sibling field may drift between arrivals; the produced
        // `Member` operand only asserts the projected field is uniform.
        for ordinal in mutable::self_target_ordinals(program, state) {
            if !storage_holds_bound_value(
                program,
                machine_symbol,
                state,
                0,
                ordinal.saturating_add(1),
                parameter_symbol,
                field_path,
            ) {
                return None;
            }
        }
    }
    provenance
}

/// Positional arguments of every named transition edge into `state_index`,
/// paired with the source state and the transition's own statement ordinal.
/// Typed lowering flattens nested transition forms into statements; inspect
/// ordinary targets and continuation targets, just as the termination graph
/// does. `-> self` and exits are not named arrivals; an edge short an
/// argument contributes an invalid handle that fails resolution above.
fn named_transition_arguments(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state_index: usize,
    argument_index: usize,
) -> Vec<(SymbolHandle, usize, ExpressionHandle)> {
    let mut incoming = Vec::new();
    for source in program.machine_states(machine) {
        for (ordinal, statement) in program
            .statement_table
            .statements(source.statement_nodes)
            .iter()
            .enumerate()
        {
            let StatementNode::Transition(transition) = statement else {
                continue;
            };
            for target in [transition.target, transition.continuation] {
                if !target.is_valid() {
                    continue;
                }
                let TransitionTargetNode::Named {
                    path, arguments, ..
                } = program.statement_table.transition_target(target)
                else {
                    continue;
                };
                if crate::checks::termination::named_transition_target_state_index(
                    program,
                    machine,
                    path.symbol,
                ) != Some(state_index)
                {
                    continue;
                }
                incoming.push((
                    source.symbol,
                    ordinal,
                    program
                        .statement_table
                        .expression_handles(*arguments)
                        .get(argument_index)
                        .copied()
                        .unwrap_or_else(ExpressionHandle::invalid),
                ));
            }
        }
    }
    incoming
}

pub(super) fn substitute_entry(
    expression: &CrashPredicateExpression,
    operands: &[Option<CrashPredicateExpression>],
) -> Option<CrashPredicateExpression> {
    Some(match expression {
        CrashPredicateExpression::Parameter(ordinal) => operands.get(*ordinal as usize)?.clone()?,
        // Literals and owner-scope names need no operand: a `Name` leaf only
        // survives predicate extraction when its spelling matched no formal,
        // so it already names a fixed path the caller side can spell the same
        // way. `Opaque` and `ContentConservation` may hide formals inside a
        // flattened display, so they keep refusing rather than leaking a
        // callee spelling into caller coordinates.
        CrashPredicateExpression::Boolean(_)
        | CrashPredicateExpression::Integer(_)
        | CrashPredicateExpression::Float(_)
        | CrashPredicateExpression::Name(_) => expression.clone(),
        CrashPredicateExpression::Binary {
            operator,
            left,
            right,
        } => CrashPredicateExpression::Binary {
            operator: *operator,
            left: Box::new(substitute_entry(left, operands)?),
            right: Box::new(substitute_entry(right, operands)?),
        },
        CrashPredicateExpression::Unary { operator, operand } => CrashPredicateExpression::Unary {
            operator: *operator,
            operand: Box::new(substitute_entry(operand, operands)?),
        },
        CrashPredicateExpression::Member { receiver, member } => CrashPredicateExpression::Member {
            receiver: Box::new(substitute_entry(receiver, operands)?),
            member: member.clone(),
        },
        // An indexed read substitutes inside both children: `left[0]` keeps
        // its exact route with `left`'s actual, and a range index's bounds
        // substitute the same way. A flattened `Opaque` child still refuses
        // because its display may hide formals.
        CrashPredicateExpression::Indexed { collection, index } => {
            CrashPredicateExpression::Indexed {
                collection: Box::new(substitute_entry(collection, operands)?),
                index: Box::new(substitute_entry(index, operands)?),
            }
        }
        // A `start..end` operand substitutes inside both bounds: a bound
        // carrying a formal binds that bound's actual, while the inclusivity
        // flag transports verbatim.
        CrashPredicateExpression::Range {
            start,
            end,
            end_inclusive,
        } => CrashPredicateExpression::Range {
            start: Box::new(substitute_entry(start, operands)?),
            end: Box::new(substitute_entry(end, operands)?),
            end_inclusive: *end_inclusive,
        },
        // A call leaf transports its authored target and substitutes inside
        // its receiver and arguments: `floor()` keeps its parameter-free
        // shape while `offset(left)` still requires `left`'s actual. The
        // `Invalid` receiver is the receiverless marker, not a broken guard.
        CrashPredicateExpression::Call {
            target,
            receiver,
            arguments,
        } => CrashPredicateExpression::Call {
            target: target.clone(),
            receiver: Box::new(match receiver.as_ref() {
                CrashPredicateExpression::Invalid => CrashPredicateExpression::Invalid,
                receiver => substitute_entry(receiver, operands)?,
            }),
            arguments: arguments
                .iter()
                .map(|argument| substitute_entry(argument, operands))
                .collect::<Option<Vec<_>>>()?,
        },
        _ => return None,
    })
}

/// Entry substitution with per-field operand provenance. `resolve(ordinal,
/// members)` answers the operand's entry operand restricted to the member
/// projection `members` — a contiguous `Member` spine above a `Parameter`
/// leaf applies the authored projection to the operand's storage, so a
/// binding whose unread sibling fields were written still supplies the read
/// field's saved actual rather than widening the route to `Truth`. The
/// `Member` spine re-applies its own names over the resolved root, so the
/// resolver returns the operand's entry operand unprojected by `members`.
/// Parameter-free nodes need no operand and transport under the same rule
/// `substitute_entry` uses; `Opaque`, `ContentConservation` and a bare
/// `Invalid` keep the same refusal here.
pub(super) fn substitute_entry_projected(
    expression: &CrashPredicateExpression,
    resolve: &mut (impl FnMut(u32, &[String]) -> Option<CrashPredicateExpression> + ?Sized),
) -> Option<CrashPredicateExpression> {
    Some(match expression {
        CrashPredicateExpression::Parameter(ordinal) => resolve(*ordinal, &[])?,
        CrashPredicateExpression::Member { .. } => {
            // Collect the contiguous member spine root-to-leaf so the
            // parameter at its base resolves at the full authored projection.
            let mut members = Vec::new();
            let mut base = expression;
            while let CrashPredicateExpression::Member { receiver, member } = base {
                members.push(member.clone());
                base = receiver;
            }
            members.reverse();
            let base = match base {
                CrashPredicateExpression::Parameter(ordinal) => {
                    resolve(*ordinal, members.as_slice())?
                }
                _ => substitute_entry_projected(base, resolve)?,
            };
            members
                .iter()
                .fold(base, |receiver, member| CrashPredicateExpression::Member {
                    receiver: Box::new(receiver),
                    member: member.clone(),
                })
        }
        CrashPredicateExpression::Boolean(_)
        | CrashPredicateExpression::Integer(_)
        | CrashPredicateExpression::Float(_)
        | CrashPredicateExpression::Name(_) => expression.clone(),
        CrashPredicateExpression::Binary {
            operator,
            left,
            right,
        } => CrashPredicateExpression::Binary {
            operator: *operator,
            left: Box::new(substitute_entry_projected(left, resolve)?),
            right: Box::new(substitute_entry_projected(right, resolve)?),
        },
        CrashPredicateExpression::Unary { operator, operand } => CrashPredicateExpression::Unary {
            operator: *operator,
            operand: Box::new(substitute_entry_projected(operand, resolve)?),
        },
        // An indexed read's `Parameter` collection resolves at the whole
        // operand: element writes cannot be separated below the binding root
        // (`PlaceSegment::Opaque`), so the bound snapshot must hold across
        // the collection's entire storage, exactly as
        // `operand_entry_provenance` already treats the same leaf.
        CrashPredicateExpression::Indexed { collection, index } => {
            CrashPredicateExpression::Indexed {
                collection: Box::new(substitute_entry_projected(collection, resolve)?),
                index: Box::new(substitute_entry_projected(index, resolve)?),
            }
        }
        // A range operand's bounds carry no member projection of their own:
        // each substitutes under the same resolver, so a bound naming a
        // formal resolves at that formal's whole operand.
        CrashPredicateExpression::Range {
            start,
            end,
            end_inclusive,
        } => CrashPredicateExpression::Range {
            start: Box::new(substitute_entry_projected(start, resolve)?),
            end: Box::new(substitute_entry_projected(end, resolve)?),
            end_inclusive: *end_inclusive,
        },
        CrashPredicateExpression::Call {
            target,
            receiver,
            arguments,
        } => CrashPredicateExpression::Call {
            target: target.clone(),
            receiver: Box::new(match receiver.as_ref() {
                CrashPredicateExpression::Invalid => CrashPredicateExpression::Invalid,
                receiver => substitute_entry_projected(receiver, resolve)?,
            }),
            arguments: arguments
                .iter()
                .map(|argument| substitute_entry_projected(argument, resolve))
                .collect::<Option<Vec<_>>>()?,
        },
        _ => return None,
    })
}
