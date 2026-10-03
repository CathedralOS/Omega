//! Replay custody for an element-view selection value: a `BorrowedSliceView`
//! carrying a `SharedBorrow` argument or a `MutableSliceView` carrying a
//! `MutableBorrow` one. The checked plan recorded the authored
//! `place.as_slice()` or `place.as_mut_slice()` view's exact collection root
//! symbol and member path; replay rebuilds that collection place and requires
//! the view's access on the argument, the exact path, and the projected
//! leaf's identity equal to the argument's recorded collection identity. Like
//! `shared_borrow`, a view join is provenance pass-through under either
//! access: it leaves owned custody records untouched — loan exclusivity is
//! the checked planner's `Mutable`-kind roster, not a replay concern. The two
//! accesses replay one walk and differ only in the planned access they
//! require and the view family their refusals name.

use super::{
    CheckedTrees, ExpressionHandle, ExpressionNode, LoweringError, StatementNode, unsupported,
};
use crate::expression_preparation::bindings::LoanAccess;
use checked_trees::{CheckedUnitStructuralArgumentPlan, types::TypeReferenceNode};

/// A replay refusal, spelled for the view family whose loan it rejoins: a
/// `BorrowedSliceView` holds a shared loan of the lent collection, a
/// `MutableSliceView` an exclusive one.
macro_rules! refusal {
    ($loan:expr, $reason:literal) => {
        match $loan {
            LoanAccess::Shared => concat!("borrowed slice view ", $reason),
            LoanAccess::Mutable => concat!("mutable slice view ", $reason),
        }
    };
}

pub(super) fn validate(
    checked: &CheckedTrees,
    machine: &checked_trees::machine::Machine,
    authored: &checked_trees::state::State,
    statement: u32,
    expression: ExpressionHandle,
    argument: &CheckedUnitStructuralArgumentPlan,
    loan: LoanAccess,
) -> Result<(), LoweringError> {
    // A view is spelled `.as_slice()` over a whole collection, or as a range
    // index `collection[a..b]` — the byte-view arm mints this selection
    // without an authored call. The range replays as a `FixedByteRange`
    // path suffix over the collection the index names.
    let (receiver, window) = match checked.expression_table.expression(expression) {
        ExpressionNode::Call(call) => {
            if !call.arguments.is_empty() {
                return unsupported(refusal!(loan, "call gained arguments"));
            }
            (call.receiver, None)
        }
        ExpressionNode::Indexed(indexed) => {
            let ExpressionNode::Range(range) = checked.expression_table.expression(indexed.index)
            else {
                return unsupported(refusal!(loan, "lost its authored view call"));
            };
            if let checked_trees::CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice {
                start,
                end,
                ..
            }
            | checked_trees::CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice {
                start,
                end,
                ..
            } = &argument.source
            {
                // A range-source plan carries its window as recorded scalar
                // rows instead of a `FixedByteRange` path segment: replay
                // each authored bound through the row the scalar planner
                // bound at this occurrence's site and require the recorded
                // value. Literal bounds replay identically to computed ones,
                // so the window's extent never enters the path.
                let site = if let Some(StatementNode::LocalData(local)) = checked
                    .statement_table
                    .statements(authored.statement_nodes)
                    .get(statement as usize)
                    && local.initial_value == expression
                {
                    checked_trees::CheckedSubsliceSite::LocalBinding
                } else {
                    checked_trees::CheckedSubsliceSite::Expression { expression }
                };
                for (authored_endpoint, role, recorded) in [
                    (
                        range.start,
                        checked_trees::CheckedScalarExpressionRole::SubsliceStart { site },
                        start,
                    ),
                    (
                        range.end,
                        checked_trees::CheckedScalarExpressionRole::SubsliceEnd { site },
                        end,
                    ),
                ] {
                    if !authored_endpoint.is_valid() {
                        if recorded.is_some() {
                            return unsupported(refusal!(loan, "window endpoint presence changed"));
                        }
                        continue;
                    }
                    let (binding, selected) = checked
                        .facts
                        .values
                        .scalar_expressions
                        .bound_expression_at(authored.symbol, statement, role)
                        .ok_or(LoweringError::Unsupported(refusal!(
                            loan,
                            "window endpoint lost its source-bound plan"
                        )))?;
                    if binding.expression != authored_endpoint
                        || binding.destination.is_valid()
                        || selected.primitive_type()
                            != Some(checked_trees::types::PrimitiveType::U64)
                        || recorded.as_ref() != Some(selected)
                    {
                        return unsupported(refusal!(
                            loan,
                            "window endpoint differs from its recorded plan"
                        ));
                    }
                }
                (indexed.collection, None)
            } else {
                let window = (|| {
                    if range.end_inclusive || !range.start.is_valid() || !range.end.is_valid() {
                        return None;
                    }
                    let start = usize::try_from(
                        checked
                            .expression_table
                            .constant_integer_value(range.start)?,
                    )
                    .ok()?;
                    let end = usize::try_from(
                        checked.expression_table.constant_integer_value(range.end)?,
                    )
                    .ok()?;
                    Some((start, end))
                })();
                let Some(window) = window else {
                    return unsupported(refusal!(loan, "lost its authored view call"));
                };
                (indexed.collection, Some(window))
            }
        }
        _ => return unsupported(refusal!(loan, "lost its authored view call")),
    };
    if argument.access != loan.planned_access() {
        return unsupported(refusal!(loan, "changed its planned access"));
    }
    let (root, mut checked_path) = authored_collection_path(checked, machine, authored, receiver)?;
    if let Some((start, end)) = window {
        // A byte view records its window as a `FixedByteRange` path segment —
        // the range sources replayed above keep their bounds on the source
        // plan instead, so only whole-place borrow paths reach this arm.
        let (Ok(start), Ok(end)) = (u64::try_from(start), u64::try_from(end)) else {
            return unsupported(refusal!(loan, "lost its authored view call"));
        };
        checked_path
            .push(checked_trees::CheckedUnitStructuralPathSegment::FixedByteRange { start, end });
    }
    if checked_path != argument.path {
        return unsupported(refusal!(loan, "path does not match its projected path"));
    }
    let leaf =
        validation::expression_result_type_reference(&checked.typed, machine, authored, receiver)
            .ok_or(LoweringError::Unsupported(refusal!(
            loan,
            "lost its collection type"
        )))?;
    if let checked_trees::CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice { .. } =
        argument.source
    {
        // A wider scalar element's plan records the lent view type the
        // destination declared — the receiver-side proof is the literal
        // fixed array the window subslices, checked against its declaration
        // exactly the way the checked planner peeled it.
        let mut container =
            validation::unwrapped_type_reference(&checked.typed, leaf).unwrap_or(leaf);
        while let TypeReferenceNode::Constrained { base_type, .. }
        | TypeReferenceNode::Reference {
            referee: base_type, ..
        } = checked.type_reference_table.type_reference(container)
        {
            container = *base_type;
        }
        if !matches!(
            checked.type_reference_table.type_reference(container),
            TypeReferenceNode::FixedArray {
                length: checked_trees::types::FixedArrayLength::Literal(_),
                ..
            }
        ) {
            return unsupported(refusal!(loan, "element view is not a fixed-array window"));
        }
    } else if let checked_trees::CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice {
        ..
    } = argument.source
    {
        // The collection of a byte-window source is itself an established
        // byte view: the recorded identity is the view's peeled carrier
        // spelling, not the reference shell's nominal one.
        let mut view = leaf;
        while let TypeReferenceNode::Constrained { base_type, .. }
        | TypeReferenceNode::Reference {
            referee: base_type, ..
        } = checked.type_reference_table.type_reference(view)
        {
            view = *base_type;
        }
        if checked.normalized_type_identity(view).as_str() != argument.type_identity {
            return unsupported(refusal!(loan, "byte window is not a byte-view subslice"));
        }
    } else if checked.normalized_type_identity(leaf).as_str() != argument.type_identity {
        return unsupported(refusal!(loan, "does not project its collection"));
    }
    let ExpressionNode::Name(name) = checked.expression_table.expression(root) else {
        return unsupported(refusal!(loan, "receiver is not an exact place"));
    };
    if !name.symbol.is_valid()
        || name.head_symbol != name.symbol
        || checked
            .expression_table
            .name_path_members(name.members)
            .len()
            != 1
    {
        return unsupported(refusal!(loan, "root is not an exact name"));
    }
    let is_self_root = matches!(
        checked.expression_table.name_path_members(name.members),
        [spelling] if spelling.is_self_receiver()
    );
    if !super::owned_selection::source_plan_matches(
        checked,
        authored.symbol,
        name.symbol,
        &argument.source,
    ) {
        return unsupported(refusal!(loan, "source does not match its root"));
    }
    if is_self_root {
        // `self` names the machine's attached storage: the checked plan carries
        // it as `StructuralLocal{machine_symbol}` and the receiver parameter
        // owns no authored local slot.
        if !checked
            .state_parameters(authored)
            .iter()
            .any(|parameter| parameter.is_self)
        {
            return unsupported(refusal!(loan, "self root has no self parameter"));
        }
    } else if !checked
        .state_parameters(authored)
        .iter()
        .any(|parameter| parameter.symbol == name.symbol)
    {
        let mut locals = checked
            .statement_table
            .statements(authored.statement_nodes)
            .iter()
            .enumerate()
            .filter_map(|(index, candidate)| match candidate {
                StatementNode::LocalData(local) if local.symbol == name.symbol => {
                    Some((index, local))
                }
                _ => None,
            });
        let Some((index, local)) = locals.next() else {
            return unsupported(refusal!(loan, "loses its authored root"));
        };
        if locals.next().is_some()
            || index >= statement as usize
            || !(local.initial_value.is_valid()
                || validation::zeroed_structural_parts(&checked.typed, local.type_reference))
        {
            return unsupported(refusal!(loan, "root is not uniquely established"));
        }
    }
    // Lending preserves the already-established collection and its embedded
    // references. Exact source/path/type/access replay above owns correspondence;
    // represented element geometry and live loans are independently verified at
    // the emitted view establishment. Requiring wholly owned elements here
    // would confuse borrowing storage with constructing or moving its values.
    Ok(())
}

/// The authored collection place a view is taken over, replayed from its
/// member chain: the bare root name and the checked record-field path to the
/// collection, each field resolved against its owner's declaration exactly as
/// the checked planner spelled it (`#n` for an authored identity). A `.as_slice()`
/// receiver and the collection of a range over a fixed-array field
/// (`self.items[a..b]`) replay through this one walk.
pub(crate) fn authored_collection_path(
    checked: &CheckedTrees,
    machine: &checked_trees::machine::Machine,
    authored: &checked_trees::state::State,
    receiver: ExpressionHandle,
) -> Result<
    (
        ExpressionHandle,
        Vec<checked_trees::CheckedUnitStructuralPathSegment>,
    ),
    LoweringError,
> {
    let mut checked_path = Vec::new();
    let mut cursor = receiver;
    let root = loop {
        match checked.expression_table.expression(cursor) {
            ExpressionNode::Member(member) => {
                if member.case_variant.is_some() {
                    return unsupported("borrowed slice view receiver uses a case member");
                }
                let receiver = validation::declared_place_type_raw(
                    &checked.typed,
                    machine,
                    Some(authored),
                    member.receiver,
                )
                .and_then(|receiver| validation::unwrapped_type_reference(&checked.typed, receiver))
                .ok_or(LoweringError::Unsupported(
                    "borrowed slice view lost its receiver type",
                ))?;
                let checked_trees::types::TypeReferenceNode::Named { symbol, name } =
                    checked.type_reference_table.type_reference(receiver)
                else {
                    return unsupported("borrowed slice view receiver is not a named record");
                };
                let owner_symbol = if *symbol == machine.symbol && name.as_str() == "Self" {
                    machine.attached_data_symbol
                } else {
                    *symbol
                };
                let owner = checked
                    .data_definitions()
                    .iter()
                    .find(|owner| owner.symbol == owner_symbol)
                    .ok_or(LoweringError::Unsupported(
                        "borrowed slice view field owner is absent",
                    ))?;
                // The member's retained symbol is a resolution-scope symbol,
                // not the checked field symbol — the checked path segment (a
                // retained declaration identity, `#n` when authored) is the
                // authority. A `Self`-rooted owner resolves by name against
                // its attached data; an exact match must be unique.
                let mut named =
                    checked
                        .data_members(owner)
                        .iter()
                        .filter_map(|candidate| match candidate {
                            checked_trees::data::DataMember::Field(field)
                                if field.name.as_str() == member.member.as_str()
                                    && field.symbol.is_valid()
                                    && field.type_reference.is_valid() =>
                            {
                                Some(field)
                            }
                            _ => None,
                        });
                let field = named.next();
                if named.next().is_some() {
                    return unsupported("borrowed slice view member is ambiguous");
                }
                let field = field.ok_or(LoweringError::Unsupported(
                    "borrowed slice view lost its member declaration",
                ))?;
                if field.relevance.is_erased() {
                    return unsupported("borrowed slice view reads an erased member");
                }
                checked_path.push(checked_trees::CheckedUnitStructuralPathSegment::Field(
                    field.path_identity(),
                ));
                cursor = member.receiver;
            }
            // The checked planner may not record an indexed segment in a view
            // path: `FixedIndex` resolution is an established whole element,
            // not a collection. A computed index likewise has no runtime
            // collection end here.
            ExpressionNode::Indexed(_) => {
                return unsupported("borrowed slice view receiver is not an exact place");
            }
            ExpressionNode::Name(_) => break cursor,
            _ => return unsupported("borrowed slice view receiver is not an exact place"),
        }
    };
    checked_path.reverse();
    Ok((root, checked_path))
}
