//! Replay custody for a `BorrowedSliceView` selection value carrying a
//! `SharedBorrow` element-view argument. The checked plan recorded the authored
//! `place.as_slice()` view's exact collection root symbol and member path;
//! replay rebuilds that collection place and requires shared access on the
//! argument, the exact path, and the projected leaf's identity equal to the
//! argument's recorded collection identity. Like `shared_borrow`, a shared
//! view join is provenance pass-through: it leaves owned custody records
//! untouched.

use super::{
    CheckedTrees, ExpressionHandle, ExpressionNode, LoweringError, StatementNode, unsupported,
};
use checked_trees::CheckedUnitStructuralArgumentPlan;

pub(super) fn validate(
    checked: &CheckedTrees,
    machine: &checked_trees::machine::Machine,
    authored: &checked_trees::state::State,
    statement: u32,
    expression: ExpressionHandle,
    argument: &CheckedUnitStructuralArgumentPlan,
) -> Result<(), LoweringError> {
    let ExpressionNode::Call(call) = checked.expression_table.expression(expression) else {
        return unsupported("borrowed slice view lost its authored view call");
    };
    if !call.arguments.is_empty() {
        return unsupported("borrowed slice view call gained arguments");
    }
    if argument.access != checked_trees::CheckedStructuralAccess::SharedBorrow {
        return unsupported("borrowed slice view changed its planned access");
    }
    let (root, checked_path) = authored_collection_path(checked, machine, authored, call.receiver)?;
    if checked_path != argument.path {
        return unsupported("borrowed slice view path does not match its projected path");
    }
    let leaf = validation::expression_result_type_reference(
        &checked.typed,
        machine,
        authored,
        call.receiver,
    )
    .ok_or(LoweringError::Unsupported(
        "borrowed slice view lost its collection type",
    ))?;
    if checked.normalized_type_identity(leaf).as_str() != argument.type_identity {
        return unsupported("borrowed slice view does not project its collection");
    }
    let ExpressionNode::Name(name) = checked.expression_table.expression(root) else {
        return unsupported("borrowed slice view receiver is not an exact place");
    };
    if !name.symbol.is_valid()
        || name.head_symbol != name.symbol
        || checked
            .expression_table
            .name_path_members(name.members)
            .len()
            != 1
    {
        return unsupported("borrowed slice view root is not an exact name");
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
        return unsupported("borrowed slice view source does not match its root");
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
            return unsupported("borrowed slice view self root has no self parameter");
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
            return unsupported("borrowed slice view loses its authored root");
        };
        if locals.next().is_some() || index >= statement as usize || !local.initial_value.is_valid()
        {
            return unsupported("borrowed slice view root is not uniquely established");
        }
    }
    // The lent place is the projected collection, not the root's whole record:
    // an attached receiver may legitimately hold members (services, loans) the
    // pipeline cannot carry while the projected array keeps plain storage.
    if !validation::has_linear_owned_contents(
        &checked.typed,
        validation::unwrapped_type_reference(&checked.typed, leaf).unwrap_or(leaf),
    ) {
        return unsupported(
            "borrowed slice view root is not a structural place the pipeline can carry",
        );
    }
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
                    field
                        .identity
                        .map(|identity| format!("#{identity}"))
                        .unwrap_or_else(|| field.name.as_str().to_owned()),
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
