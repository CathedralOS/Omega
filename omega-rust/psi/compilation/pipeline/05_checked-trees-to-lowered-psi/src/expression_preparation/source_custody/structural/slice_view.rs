//! Replay custody for an element-view selection value: a `BorrowedSliceView`
//! carrying a `SharedBorrow` argument or a `MutableSliceView` carrying a
//! `MutableBorrow` one. The checked plan recorded the authored
//! `place.as_slice()`, `place.as_mut_slice()`, or whole-collection borrow's exact collection root
//! symbol and member path; replay rebuilds that collection place and requires
//! the view's access on the argument, the exact path, and the projected
//! leaf's identity equal to the argument's recorded collection identity. Like
//! `shared_borrow`, a view join is provenance pass-through under either
//! access: it leaves owned custody records untouched — loan exclusivity is
//! the checked planner's `Mutable`-kind roster, not a replay concern. The two
//! accesses replay one walk and differ only in the planned access they
//! require and the view family their refusals name.
//! Retained range operations use the shared range source validator, including
//! array-backed byte views and both pure and computed endpoint operands.

use super::{
    CheckedTrees, ExpressionHandle, ExpressionNode, LoweringError, StatementNode, unsupported,
};
use crate::expression_preparation::bindings::LoanAccess;
use checked_trees::CheckedUnitStructuralArgumentPlan;

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
    expected: checked_trees::types::TypeReferenceHandle,
    argument: &CheckedUnitStructuralArgumentPlan,
    loan: LoanAccess,
    region: checked_trees::CheckedElementViewRegion,
) -> Result<
    Vec<(
        checked_trees::CheckedScalarComputationHandle,
        ExpressionHandle,
    )>,
    LoweringError,
> {
    if crate::unit::attached_unit::view_ranges::range(argument).is_some() {
        if region != checked_trees::CheckedElementViewRegion::WholeCollection
            || argument.access != loan.planned_access()
        {
            return unsupported(refusal!(loan, "range changed its region or access"));
        }
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
        return crate::expression_preparation::source_custody::view_ranges::validate_operand(
            checked, authored, statement, expected, expression, argument, site,
        )
        .map(|operands| {
            operands
                .into_iter()
                .map(|(expression, handle)| (handle, expression))
                .collect()
        });
    }
    // Non-range views open a whole collection or an explicit representation
    // region. Range plans were handled by the common operation above.
    let authored_region =
        validation::slice_recast_byte_region(&checked.typed, machine, authored, expression);
    let receiver =
        if let checked_trees::CheckedElementViewRegion::RepresentationBytes { start, end } = region
        {
            let source = authored_region.ok_or(LoweringError::Unsupported(refusal!(
                loan,
                "byte region lost its authored recast"
            )))?;
            let access = match loan {
                LoanAccess::Shared => language_semantics::ReferenceAccess::Shared,
                LoanAccess::Mutable => language_semantics::ReferenceAccess::Mutable,
            };
            if source.extent != end || source.access != access {
                return unsupported(refusal!(loan, "byte region changed its extent or access"));
            }
            let role =
                checked_trees::CheckedScalarExpressionRole::RepresentationViewOffset { expression };
            let (offset, primitive) =
                super::operand_source(checked, authored.symbol, statement, role)?;
            if offset != source.start
                || super::validate_operand(
                    checked,
                    machine.symbol,
                    authored.symbol,
                    statement,
                    role,
                    start,
                    source.start,
                )? != primitive
            {
                return unsupported(refusal!(loan, "byte region changed its offset computation"));
            }
            source.collection
        } else {
            if authored_region.is_some() {
                return unsupported(refusal!(
                    loan,
                    "byte region lost its representation contract"
                ));
            }
            if let Some(source) = validation::slice_recast_source(&checked.typed, expression) {
                let access = match loan {
                    LoanAccess::Shared => language_semantics::ReferenceAccess::Shared,
                    LoanAccess::Mutable => language_semantics::ReferenceAccess::Mutable,
                };
                if source.access != access {
                    return unsupported(refusal!(loan, "recast changed its authored access"));
                }
                source.source
            } else {
                match checked.expression_table.expression(expression) {
                    ExpressionNode::Borrow(borrow) => {
                        // A whole collection loan uses the same view descriptor
                        // as a view call. Replay its authored access before the
                        // common root, path, and collection-identity checks below.
                        let access = match loan {
                            LoanAccess::Shared => language_semantics::ReferenceAccess::Shared,
                            LoanAccess::Mutable => language_semantics::ReferenceAccess::Mutable,
                        };
                        if borrow.access != access {
                            return unsupported(refusal!(
                                loan,
                                "borrow changed its authored access"
                            ));
                        }
                        borrow.target
                    }
                    ExpressionNode::Call(call) => {
                        if !call.arguments.is_empty() {
                            return unsupported(refusal!(loan, "call gained arguments"));
                        }
                        call.receiver
                    }
                    _ => return unsupported(refusal!(loan, "lost its authored view call")),
                }
            }
        };
    if argument.access != loan.planned_access() {
        return unsupported(refusal!(loan, "changed its planned access"));
    }
    let (root, checked_path) = authored_collection_path(checked, machine, authored, receiver)?;
    if checked_path != argument.path {
        return unsupported(refusal!(loan, "path does not match its projected path"));
    }
    let leaf =
        validation::expression_result_type_reference(&checked.typed, machine, authored, receiver)
            .ok_or(LoweringError::Unsupported(refusal!(
            loan,
            "lost its collection type"
        )))?;
    if checked.normalized_type_identity(leaf).as_str() != argument.type_identity {
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
    Ok(Vec::new())
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
