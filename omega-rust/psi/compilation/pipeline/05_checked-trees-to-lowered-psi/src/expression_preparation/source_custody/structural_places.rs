//! Reconstruct authored structural places and runtime selector coordinates.
//! Call receivers and borrowed selections share declaration-based field replay;
//! the caller retains its own root custody and argument occurrence checks.

use crate::lowering_error::{LoweringError, unsupported};
use checked_trees::CheckedTrees;
use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use checked_trees::statement::StatementNode;
use symbols::SymbolHandle;

/// Walk an authored member/indexed place chain to its root name, rebuilding
/// the retained field/fixed-index path against the same declaration and
/// literal-bound rules the checker committed to. `view_root_index` admits a
/// literal index into a slice-typed (view) collection: only a shared-borrow
/// target may use it — the loan's own live extent is the bound, while copied
/// and scalar-case leaves still need the literal array length's evidence.
pub(crate) fn walk_exact_place(
    checked: &CheckedTrees,
    machine: &checked_trees::machine::Machine,
    authored: &checked_trees::state::State,
    target: ExpressionHandle,
    view_root_index: bool,
) -> Result<
    (
        Vec<checked_trees::CheckedUnitStructuralPathSegment>,
        ExpressionHandle,
    ),
    LoweringError,
> {
    let mut checked_path = Vec::new();
    let mut cursor = target;
    let mut visited = Vec::new();
    let root = loop {
        if !checked.expression_table.expression_is_valid(cursor) || visited.contains(&cursor) {
            return unsupported("structural place has a stale or cyclic source");
        }
        visited.push(cursor);
        match checked.expression_table.expression(cursor) {
            ExpressionNode::Member(member) => {
                if member.case_variant.is_some() {
                    return unsupported("borrowed selection target uses a case member");
                }
                let receiver = validation::declared_place_type_raw(
                    &checked.typed,
                    machine,
                    Some(authored),
                    member.receiver,
                )
                .and_then(|receiver| validation::unwrapped_type_reference(&checked.typed, receiver))
                .ok_or(LoweringError::Unsupported(
                    "borrowed selection lost its receiver type",
                ))?;
                // A `&'a` receiver on a lifetime-parameterized record unwraps
                // to a `Generic` node whose base symbol still names the data
                // declaration — the erased borrow-region arguments are not
                // part of the member-walk identity.
                let symbol = match checked.type_reference_table.type_reference(receiver) {
                    checked_trees::types::TypeReferenceNode::Named { symbol, .. } => *symbol,
                    checked_trees::types::TypeReferenceNode::Generic { base_symbol, .. } => {
                        *base_symbol
                    }
                    _ => {
                        return unsupported("borrowed selection receiver is not a named record");
                    }
                };
                // A `self` receiver keeps the machine-keyed `Self` alias; its
                // owner is the machine's attached data declaration, and the
                // authored member symbol lives in the receiver-view space, so
                // the field resolves by name there.
                let self_receiver = symbol == machine.symbol;
                let owner_symbol = if self_receiver {
                    machine.attached_data_symbol
                } else {
                    symbol
                };
                let owner = checked
                    .data_definitions()
                    .iter()
                    .find(|owner| owner.symbol == owner_symbol)
                    .ok_or(LoweringError::Unsupported(
                        "borrowed selection field owner is absent",
                    ))?;
                let field = validation::exact_data_member_field(
                    &checked.typed,
                    owner,
                    if self_receiver {
                        SymbolHandle::invalid()
                    } else {
                        member.member_symbol
                    },
                    member.member.as_str(),
                    None,
                )
                .ok_or(LoweringError::Unsupported(
                    "borrowed selection lost its member declaration",
                ))?;
                if field.relevance.is_erased() {
                    return unsupported("borrowed selection reads an erased member");
                }
                checked_path.push(checked_trees::CheckedUnitStructuralPathSegment::Field(
                    field.path_identity(),
                ));
                cursor = member.receiver;
            }
            // A literal fixed index is an exact place segment the same way a
            // record field is: the checked planner recorded its ordinal after
            // the same literal-length bound check replayed here. An immutable
            // local bound to a constant integer folds to the same literal the
            // checker folded at admission. A non-literal index replays the
            // checker's `RuntimeIndex` coordinate instead — the selector's
            // bound obligation is the terminal verifier's, not this walk's.
            ExpressionNode::Indexed(indexed) => {
                let literal = match checked.expression_table.expression(indexed.index) {
                    ExpressionNode::Integer(index) => {
                        index.value_bignum().and_then(|value| value.to_u64())
                    }
                    ExpressionNode::Name(name) => {
                        constant_local_index(checked, authored, name.symbol)
                    }
                    _ => None,
                };
                let Some(index) = literal else {
                    checked_path.push(
                        checked_trees::CheckedUnitStructuralPathSegment::RuntimeIndex(
                            runtime_index_selector(checked, machine, authored, indexed.index)?,
                        ),
                    );
                    cursor = indexed.collection;
                    continue;
                };
                let container = validation::declared_place_type_raw(
                    &checked.typed,
                    machine,
                    Some(authored),
                    indexed.collection,
                )
                .and_then(|container| {
                    validation::unwrapped_type_reference(&checked.typed, container)
                })
                .ok_or(LoweringError::Unsupported(
                    "borrowed selection index has no declared collection",
                ))?;
                // A slice-typed collection — a `&[Cell]` view root — carries no
                // literal bound: a shared loan's own live extent discharges the
                // index the way the runtime selector's obligation does. Copied
                // and scalar-case leaves instead mint the literal's exact place
                // and keep the literal array length's evidence rule.
                let mut unwrapped = container;
                let literal_length = loop {
                    match checked.type_reference_table.type_reference(unwrapped) {
                        checked_trees::types::TypeReferenceNode::Constrained {
                            base_type, ..
                        }
                        | checked_trees::types::TypeReferenceNode::Reference {
                            referee: base_type,
                            ..
                        } => unwrapped = *base_type,
                        checked_trees::types::TypeReferenceNode::FixedArray {
                            length: checked_trees::types::FixedArrayLength::Literal(length),
                            ..
                        } => break Some(*length),
                        checked_trees::types::TypeReferenceNode::Slice { .. }
                            if view_root_index =>
                        {
                            break None;
                        }
                        _ => {
                            return unsupported(
                                "borrowed selection index has no literal array length",
                            );
                        }
                    }
                };
                let Some(length) = literal_length else {
                    checked_path.push(checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(
                        index,
                    ));
                    cursor = indexed.collection;
                    continue;
                };
                if usize::try_from(index)
                    .ok()
                    .is_none_or(|index| index >= length)
                {
                    return unsupported("borrowed selection index is out of bounds");
                }
                checked_path.push(checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(
                    index,
                ));
                cursor = indexed.collection;
            }
            ExpressionNode::Name(_) => break cursor,
            _ => return unsupported("borrowed selection target is not an exact place"),
        }
    };
    checked_path.reverse();
    Ok((checked_path, root))
}

/// The integer primitives a `RuntimeIndex` selector may carry: the same set
/// the checker's mint admits — an operand the terminal bound obligation can
/// replay.
fn selector_is_integer(
    checked: &CheckedTrees,
    reference: checked_trees::types::TypeReferenceHandle,
) -> bool {
    matches!(
        checked.primitive_type_reference(reference),
        Some(
            checked_trees::types::PrimitiveType::U8
                | checked_trees::types::PrimitiveType::U16
                | checked_trees::types::PrimitiveType::U32
                | checked_trees::types::PrimitiveType::U64
                | checked_trees::types::PrimitiveType::I8
                | checked_trees::types::PrimitiveType::I16
                | checked_trees::types::PrimitiveType::I32
                | checked_trees::types::PrimitiveType::I64
        )
    )
}

/// The literal index an immutable local scalar names: `let i: u64 = 2`
/// selects exactly element 2 for the whole state because nothing can write
/// the binding. Mirrors the checker's `FixedIndex` fold of the same name; a
/// bare `Name` selector spelled against a state-local immutable binding with
/// a constant initializer folds, and every other shape stays on the
/// runtime-selector routes.
pub(crate) fn constant_local_index(
    checked: &CheckedTrees,
    authored: &checked_trees::state::State,
    symbol: SymbolHandle,
) -> Option<u64> {
    let local = checked
        .statement_table
        .statements(authored.statement_nodes)
        .iter()
        .find_map(|statement| match statement {
            StatementNode::LocalData(local) if local.symbol == symbol && !local.is_mutable => {
                Some(local)
            }
            _ => None,
        })?;
    u64::try_from(
        checked
            .expression_table
            .constant_integer_value(local.initial_value)?,
    )
    .ok()
}

/// Re-derive the `RuntimeIndex` coordinate the checker minted for a
/// non-literal index segment. A bare immutable integer parameter names its
/// dense scalar position — the coordinate `CheckedScalarExpression` indexes.
/// Any other selector is a pure field chain below an entry parameter
/// (`self.count`): the `StructuralField` coordinate names the parameter's
/// authored position plus the field path and requires an integer leaf. A
/// spelling that matches neither never reached the checked plan, so failing
/// here keeps the replay honest rather than inventing a coordinate.
pub(crate) fn runtime_index_selector(
    checked: &CheckedTrees,
    machine: &checked_trees::machine::Machine,
    authored: &checked_trees::state::State,
    expression: ExpressionHandle,
) -> Result<checked_trees::CheckedRuntimeIndex, LoweringError> {
    if let ExpressionNode::Name(name) = checked.expression_table.expression(expression) {
        let parameters = checked.state_parameters(authored);
        let members = checked.expression_table.name_path_members(name.members);
        let authored_position = (members.len() == 1)
            .then(|| {
                parameters.iter().position(|parameter| {
                    if name.symbol.is_valid() {
                        parameter.symbol == name.symbol
                    } else if name.head_symbol.is_valid() {
                        parameter.symbol == name.head_symbol
                    } else {
                        parameter.name.as_str() == members[0].as_str()
                    }
                })
            })
            .flatten();
        if let Some(authored_position) = authored_position {
            let parameter = &parameters[authored_position];
            if parameter.is_mutable || !selector_is_integer(checked, parameter.type_reference) {
                return unsupported("runtime index selector is not an immutable integer parameter");
            }
            let position = parameters[..authored_position]
                .iter()
                .filter(|parameter| {
                    !parameter.relevance.is_erased()
                        && checked
                            .primitive_type_reference(parameter.type_reference)
                            .is_some()
                })
                .count();
            return u32::try_from(position)
                .map(|position| checked_trees::CheckedRuntimeIndex::Parameter { position })
                .map_err(|_| {
                    LoweringError::Unsupported("runtime index selector position exceeds u32")
                });
        }
    }
    field_selector(checked, machine, authored, expression)
        .map(checked_trees::CheckedRuntimeIndex::StructuralField)
}

/// Reconstruct a field snapshot's exact authored parameter and pure member
/// path. This entry does not accept a bare parameter: callers that retain a
/// scalar parameter coordinate keep their own parameter-identity contract.
pub(crate) fn field_selector(
    checked: &CheckedTrees,
    machine: &checked_trees::machine::Machine,
    authored: &checked_trees::state::State,
    expression: ExpressionHandle,
) -> Result<checked_trees::CheckedStructuralParameterField, LoweringError> {
    // Validate the entire syntax chain before recursive type resolution. A
    // malformed selector must not recurse through a cycle or another index.
    let mut cursor = expression;
    let mut visited = Vec::new();
    loop {
        if !checked.expression_table.expression_is_valid(cursor) || visited.contains(&cursor) {
            return unsupported("runtime index field selector has a stale or cyclic source");
        }
        visited.push(cursor);
        match checked.expression_table.expression(cursor) {
            ExpressionNode::Member(member) if member.case_variant.is_none() => {
                cursor = member.receiver;
            }
            ExpressionNode::Name(name)
                if name.symbol.is_valid()
                    && name.head_symbol == name.symbol
                    && checked
                        .expression_table
                        .name_path_members(name.members)
                        .len()
                        == 1 =>
            {
                break;
            }
            _ => return unsupported("runtime index field selector is not an exact member chain"),
        }
    }
    // A field selector is itself an exact place walk: a pure member chain
    // rooted at an entry parameter (a `self` root names the is_self slot by
    // the machine's durable symbol). Indexed or case segments inside the
    // selector never mint a `StructuralField` coordinate.
    let (path, root) = walk_exact_place(checked, machine, authored, expression, false)?;
    if path.is_empty() {
        return unsupported("runtime index selector names no field below its parameter");
    }
    let mut predicate_path = Vec::with_capacity(path.len());
    for segment in path {
        let checked_trees::CheckedUnitStructuralPathSegment::Field(identity) = segment else {
            return unsupported("runtime index field selector is not a pure member chain");
        };
        predicate_path.push(checked_trees::CheckedStructuralPredicatePathSegment::Field(
            identity,
        ));
    }
    let ExpressionNode::Name(name) = checked.expression_table.expression(root) else {
        return unsupported("runtime index field selector has no named root");
    };
    let parameters = checked.state_parameters(authored);
    let parameter_position = parameters
        .iter()
        .position(|parameter| {
            parameter.symbol == name.symbol || (parameter.is_self && name.symbol == machine.symbol)
        })
        .ok_or(LoweringError::Unsupported(
            "runtime index field selector has no parameter root",
        ))?;
    let leaf =
        validation::expression_result_type_reference(&checked.typed, machine, authored, expression)
            .ok_or(LoweringError::Unsupported(
                "runtime index field selector lost its leaf type",
            ))?;
    if !selector_is_integer(checked, leaf) {
        return unsupported("runtime index field selector leaf is not an integer");
    }
    Ok(checked_trees::CheckedStructuralParameterField {
        parameter_position: u32::try_from(parameter_position).map_err(|_| {
            LoweringError::Unsupported("runtime index parameter position exceeds u32")
        })?,
        path: predicate_path,
    })
}
