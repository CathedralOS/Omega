//! Authored structural paths shared by parameter and call-result operands.
use super::{CheckedTrees, LoweringError, unsupported};
use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use checked_trees::statement::StatementNode;
use checked_trees::types::{TypeReferenceHandle, TypeReferenceNode};

enum SourceProjection {
    Field(ExpressionHandle, symbols::SymbolHandle),
    Index(u64),
    RuntimeIndex(checked_trees::CheckedRuntimeIndex),
    ByteRange(ExpressionHandle),
}

/// Identify a potential producer; `source_place_path` validates its typed path.
pub(crate) fn expression_producer(
    checked: &CheckedTrees,
    mut expression: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let mut visited = Vec::new();
    loop {
        if !checked.expression_table.expression_is_valid(expression)
            || visited.contains(&expression)
        {
            return None;
        }
        visited.push(expression);
        expression = match checked.expression_table.expression(expression) {
            ExpressionNode::Call(_) => return Some(expression),
            ExpressionNode::Borrow(borrow)
                if visited.len() == 1
                    && borrow.access == language_core::ReferenceAccess::Shared =>
            {
                borrow.target
            }
            ExpressionNode::Member(member) => member.receiver,
            ExpressionNode::Indexed(indexed)
                if matches!(
                    checked.expression_table.expression(indexed.index),
                    ExpressionNode::Integer(_)
                ) =>
            {
                indexed.collection
            }
            _ => return None,
        };
    }
}

pub(crate) fn source_path(
    checked: &CheckedTrees,
    machine: &checked_trees::machine::Machine,
    source_state: Option<&checked_trees::state::State>,
    source_type: TypeReferenceHandle,
    expression: ExpressionHandle,
) -> Result<
    (
        symbols::SymbolHandle,
        Vec<checked_trees::CheckedUnitStructuralPathSegment>,
        Option<checked_trees::CheckedStructuralAccess>,
    ),
    LoweringError,
> {
    let (root, path, access) =
        source_place_path(checked, machine, source_state, source_type, expression)?;
    let facts::PlaceRoot::Symbol(symbol) = root else {
        return unsupported("structural argument is not a named source place");
    };
    Ok((symbol, path, access))
}

/// The receiver place a call statement or named transition spells as a path
/// (`_ = self.roll(..)`, `self.tally.bump()`, `-> self.roll(..)`). These keep no
/// receiver expression, so this rejoins the authored root member, the lexical
/// root symbol and each projected field against their declarations, the same
/// exact field owners an expression receiver's `Member` chain resolves. The
/// final field must also match a call statement's captured endpoint. A named
/// transition retains its root and selected target, but not that endpoint.
/// Lexical `self` resolves
/// to the machine symbol; any other root must be a formal of `source_state`.
pub(crate) fn statement_receiver_path(
    checked: &CheckedTrees,
    machine: &checked_trees::machine::Machine,
    source_state: &checked_trees::state::State,
    statement: &StatementNode,
) -> Result<
    (
        symbols::SymbolHandle,
        Vec<checked_trees::CheckedUnitStructuralPathSegment>,
    ),
    LoweringError,
> {
    let (members, root, captured_endpoint) = match statement {
        StatementNode::Call(call) => {
            let members = checked.statement_table.name_path_members(call.receiver);
            if members.len() != call.receiver.count() as usize || !call.receiver_symbol.is_valid() {
                return unsupported("statement receiver has an invalid authored path");
            }
            (members, call.receiver_root_symbol, call.receiver_symbol)
        }
        StatementNode::Transition(transition) => {
            if !checked
                .statement_table
                .transition_target_is_valid(transition.target)
            {
                return unsupported("statement receiver has a stale invocation target");
            }
            let checked_trees::statement::TransitionTargetNode::Named { path, .. } =
                checked.statement_table.transition_target(transition.target)
            else {
                return unsupported("statement receiver has no named invocation");
            };
            let members = checked.statement_table.name_path_members(path.members);
            if members.len() != path.members.count() as usize {
                return unsupported("statement receiver has an invalid authored path");
            }
            let (_, receiver) = members.split_last().ok_or(LoweringError::Unsupported(
                "statement receiver has no invocation target",
            ))?;
            (receiver, path.head_symbol, symbols::SymbolHandle::invalid())
        }
        _ => return unsupported("statement receiver has no authored invocation"),
    };
    let Some(root_member) = members.first() else {
        return unsupported("statement receiver has no authored root");
    };
    if !root.is_valid() {
        return unsupported("statement receiver has an invalid authored path");
    }
    let parameters = checked.state_parameters(source_state);
    let root_parameter = if root == machine.symbol {
        (root_member.as_str() == "self")
            .then(|| parameters.iter().find(|parameter| parameter.is_self))
            .flatten()
    } else {
        parameters.iter().find(|parameter| {
            parameter.symbol == root && parameter.name.as_str() == root_member.as_str()
        })
    };
    let Some(root_parameter) = root_parameter else {
        return unsupported("statement receiver root is not a formal of its state");
    };
    let mut type_reference = root_parameter.type_reference;
    let mut endpoint = root;
    let mut path = Vec::new();
    for (position, member) in members.iter().enumerate().skip(1) {
        let nominal = validation::unwrapped_type_reference(&checked.typed, type_reference).ok_or(
            LoweringError::Unsupported("statement receiver field lost its nominal type"),
        )?;
        let TypeReferenceNode::Named { symbol, .. } =
            checked.type_reference_table.type_reference(nominal)
        else {
            return unsupported("statement receiver field has no declared record owner");
        };
        // Attached Self retains its machine symbol in the source type; only
        // the receiver's own first projection rejoins the attachment record.
        let owner = if *symbol == machine.symbol && position == 1 && root_parameter.is_self {
            machine.attached_data_symbol
        } else {
            *symbol
        };
        let data = checked
            .data_definitions()
            .iter()
            .find(|data| data.symbol == owner)
            .ok_or(LoweringError::Unsupported(
                "statement receiver field owner is absent",
            ))?;
        // Intermediate field symbols are not retained by the statement call;
        // the declared owner and unique field name select each one, and the
        // last must also match the captured endpoint.
        let retained = if position + 1 == members.len() {
            captured_endpoint
        } else {
            symbols::SymbolHandle::invalid()
        };
        let field = validation::exact_data_member_field(
            &checked.typed,
            data,
            retained,
            member.as_str(),
            None,
        )
        .ok_or(LoweringError::Unsupported(
            "statement receiver field disagrees with its declaration",
        ))?;
        if field.relevance.is_erased() {
            return unsupported("statement receiver cannot select an erased field");
        }
        path.push(checked_trees::CheckedUnitStructuralPathSegment::Field(
            field.path_identity(),
        ));
        type_reference = field.type_reference;
        endpoint = field.symbol;
    }
    if captured_endpoint.is_valid() && endpoint != captured_endpoint {
        return unsupported("statement receiver changed its captured endpoint");
    }
    Ok((root, path))
}

pub(crate) fn source_place_path(
    checked: &CheckedTrees,
    machine: &checked_trees::machine::Machine,
    source_state: Option<&checked_trees::state::State>,
    source_type: TypeReferenceHandle,
    mut expression: ExpressionHandle,
) -> Result<
    (
        facts::PlaceRoot,
        Vec<checked_trees::CheckedUnitStructuralPathSegment>,
        Option<checked_trees::CheckedStructuralAccess>,
    ),
    LoweringError,
> {
    let mut projections = Vec::new();
    let mut access = None;
    let mut visited = Vec::new();
    let root = loop {
        if !checked.expression_table.expression_is_valid(expression)
            || visited.contains(&expression)
        {
            return unsupported("scalar wrapper structural argument has a stale or cyclic source");
        }
        visited.push(expression);
        match checked.expression_table.expression(expression) {
            ExpressionNode::Borrow(borrow) if access.is_none() && projections.is_empty() => {
                access = Some(match borrow.access {
                    language_core::ReferenceAccess::Shared => {
                        checked_trees::CheckedStructuralAccess::SharedBorrow
                    }
                    language_core::ReferenceAccess::Mutable => {
                        checked_trees::CheckedStructuralAccess::MutableBorrow
                    }
                    language_core::ReferenceAccess::WriteOnly => {
                        checked_trees::CheckedStructuralAccess::WriteOnlyBorrow
                    }
                });
                expression = borrow.target;
            }
            ExpressionNode::Member(member) => {
                projections.push(SourceProjection::Field(expression, member.member_symbol));
                expression = member.receiver;
            }
            ExpressionNode::Indexed(indexed) => {
                if matches!(
                    checked.expression_table.expression(indexed.index),
                    ExpressionNode::Range(_)
                ) {
                    if !projections.is_empty()
                        || !matches!(
                            access,
                            Some(
                                checked_trees::CheckedStructuralAccess::MutableBorrow
                                    | checked_trees::CheckedStructuralAccess::SharedBorrow
                            )
                        )
                    {
                        return unsupported("byte range must end a borrowed call projection");
                    }
                    projections.push(SourceProjection::ByteRange(expression));
                    expression = indexed.collection;
                    continue;
                }
                // A bare immutable scalar of the calling state selects the
                // element at runtime; the checker minted the matching
                // `RuntimeIndex::Parameter` segment, so this mirror must
                // reproduce it to keep the paths identical.
                if let ExpressionNode::Name(name) =
                    checked.expression_table.expression(indexed.index)
                    && name.symbol.is_valid()
                    && name.head_symbol == name.symbol
                    && checked
                        .expression_table
                        .name_path_members(name.members)
                        .len()
                        == 1
                {
                    if let Some(position) = source_state.and_then(|state| {
                        runtime_index_parameter_position(checked, state, name.symbol)
                    }) {
                        projections.push(SourceProjection::RuntimeIndex(
                            checked_trees::CheckedRuntimeIndex::Parameter { position },
                        ));
                        expression = indexed.collection;
                        continue;
                    }
                    // A bare immutable local bound to a constant integer
                    // selects the element statically; the checker folds the
                    // same name to a `FixedIndex` segment, so this mirror
                    // must produce the fixed coordinate too.
                    if let Some(index) = source_state
                        .and_then(|state| constant_local_index(checked, state, name.symbol))
                    {
                        projections.push(SourceProjection::Index(index));
                        expression = indexed.collection;
                        continue;
                    }
                }
                // A field-chain selector (`self.at` in `cells[self.at]`) is a
                // storage read, not an operand: the checker minted the
                // `RuntimeIndex::StructuralField` coordinate and this mirror
                // reproduces it so the replayed path stays identical.
                if let Some(selector) = source_state.and_then(|state| {
                    crate::expression_preparation::source_custody::structural_places::runtime_index_selector(
                        checked,
                        machine,
                        state,
                        indexed.index,
                    )
                    .ok()
                }) {
                    projections.push(SourceProjection::RuntimeIndex(selector));
                    expression = indexed.collection;
                    continue;
                }
                let ExpressionNode::Integer(index) =
                    checked.expression_table.expression(indexed.index)
                else {
                    return unsupported(
                        "scalar wrapper structural argument requires a literal index",
                    );
                };
                let index = index
                    .value_bignum()
                    .and_then(|value| value.to_u64())
                    .ok_or(LoweringError::Unsupported(
                        "scalar wrapper structural index exceeds u64",
                    ))?;
                projections.push(SourceProjection::Index(index));
                expression = indexed.collection;
            }
            ExpressionNode::Name(name)
                if name.symbol.is_valid()
                    && name.symbol == name.head_symbol
                    && checked
                        .expression_table
                        .name_path_members(name.members)
                        .len()
                        == 1 =>
            {
                break facts::PlaceRoot::Symbol(name.symbol);
            }
            ExpressionNode::Call(_) => break facts::PlaceRoot::Expression(expression),
            _ => return unsupported("scalar wrapper structural argument is not a parameter place"),
        }
    };
    let mut type_reference = source_type;
    let mut path = Vec::new();
    for (position, projection) in projections.into_iter().rev().enumerate() {
        // Root parameters already present borrowed storage. A reference field
        // below that root is a stored descriptor: replay its crossing before
        // unwrapping the member owner, rather than silently dropping the load.
        if position > 0 && matches!(projection, SourceProjection::Field(..)) {
            while let Some(shell) = validation::reference_shell(&checked.typed, type_reference) {
                path.push(checked_trees::CheckedUnitStructuralPathSegment::Referent);
                type_reference = shell.referee;
            }
        }
        type_reference = unqualified_source_type(checked, type_reference)?;
        match projection {
            SourceProjection::ByteRange(expression) => {
                let ExpressionNode::Indexed(indexed) =
                    checked.expression_table.expression(expression)
                else {
                    return unsupported("byte window lost indexed source");
                };
                let ExpressionNode::Range(range) =
                    checked.expression_table.expression(indexed.index)
                else {
                    return unsupported("byte window lost range source");
                };
                let TypeReferenceNode::FixedArray {
                    element_type,
                    length: checked_trees::types::FixedArrayLength::Literal(extent),
                } = checked.type_reference_table.type_reference(type_reference)
                else {
                    return unsupported("byte window has no fixed-array backing");
                };
                let extent = u64::try_from(*extent)
                    .map_err(|_| LoweringError::Unsupported("byte window backing exceeds u64"))?;
                let endpoint = |expression, omitted| -> Result<u64, LoweringError> {
                    if !checked.expression_table.expression_is_valid(expression) {
                        return Ok(omitted);
                    }
                    let ExpressionNode::Integer(value) =
                        checked.expression_table.expression(expression)
                    else {
                        return unsupported("byte window requires constant source endpoints");
                    };
                    value.value_bignum().and_then(|value| value.to_u64()).ok_or(
                        LoweringError::Unsupported("byte window source endpoint exceeds u64"),
                    )
                };
                let start = endpoint(range.start, 0)?;
                let end = endpoint(range.end, extent)?;
                if range.end_inclusive
                    || extent == 0
                    || start > end
                    || end > extent
                    || checked.primitive_type_reference(*element_type)
                        != Some(checked_trees::types::PrimitiveType::U8)
                {
                    return unsupported("byte window does not fit its fixed byte backing");
                }
                path.push(
                    checked_trees::CheckedUnitStructuralPathSegment::FixedByteRange { start, end },
                );
            }
            SourceProjection::Field(expression, symbol) => {
                let owner = match checked.type_reference_table.type_reference(type_reference) {
                    TypeReferenceNode::Named { symbol, .. } => *symbol,
                    TypeReferenceNode::Generic { base_symbol, .. } => *base_symbol,
                    _ => return unsupported("scalar wrapper field has no declared record owner"),
                };
                // Attached Self retains its machine symbol in the source type.
                // Only an exact self-field observation rejoins that placeholder
                // to this machine's attached data declaration.
                let self_field = validation::exact_self_field(&checked.typed, machine, expression);
                let owner = if owner == machine.symbol
                    && root == facts::PlaceRoot::Symbol(machine.symbol)
                    && self_field.is_some()
                {
                    machine.attached_data_symbol
                } else {
                    owner
                };
                let data = checked
                    .data_definitions()
                    .iter()
                    .find(|data| data.symbol == owner)
                    .ok_or(LoweringError::Unsupported(
                        "scalar wrapper field owner is absent",
                    ))?;
                let symbol = self_field.map_or(symbol, |field| field.symbol);
                let ExpressionNode::Member(authored) =
                    checked.expression_table.expression(expression)
                else {
                    unreachable!("field projection retained its member expression")
                };
                // Local field selection can remain unresolved in typed syntax.
                // Its declared owner and unique authored field name resolve it;
                // a present field symbol must still match exactly.
                let field = validation::exact_data_member_field(
                    &checked.typed,
                    data,
                    symbol,
                    authored.member.as_str(),
                    authored.case_variant.as_ref().map(|case| case.as_str()),
                )
                .ok_or(LoweringError::Unsupported(
                    "scalar wrapper field substituted its declaration owner",
                ))?;
                if field.relevance.is_erased() {
                    return unsupported("structural source selects an erased field");
                }
                if authored.case_variant.is_some() {
                    path.push(checked_trees::CheckedUnitStructuralPathSegment::Case(
                        crate::expression_preparation::source_custody::structural_places::payload_case(
                            checked, field.symbol,
                        )?.path_identity(),
                    ));
                }
                path.push(checked_trees::CheckedUnitStructuralPathSegment::Field(
                    field.path_identity(),
                ));
                type_reference = field.type_reference;
            }
            SourceProjection::Index(index) => {
                // A borrowed element view carries no declared extent — its
                // live length is what the emitted segment's obligation
                // re-proves — so the hop stays unproven here exactly the way
                // the checker's slice admission leaves it.
                let element_type = match checked.type_reference_table.type_reference(type_reference)
                {
                    TypeReferenceNode::FixedArray {
                        element_type,
                        length: checked_trees::types::FixedArrayLength::Literal(length),
                    } => {
                        if usize::try_from(index)
                            .ok()
                            .is_none_or(|index| index >= *length)
                        {
                            return unsupported("scalar wrapper structural index is out of bounds");
                        }
                        *element_type
                    }
                    TypeReferenceNode::Slice { element_type } => *element_type,
                    _ => {
                        return unsupported(
                            "scalar wrapper index has no literal fixed-array owner",
                        );
                    }
                };
                path.push(checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(
                    index,
                ));
                type_reference = element_type;
            }
            SourceProjection::RuntimeIndex(selector) => {
                let element_type = match checked.type_reference_table.type_reference(type_reference)
                {
                    TypeReferenceNode::FixedArray { element_type, .. }
                    | TypeReferenceNode::Slice { element_type } => *element_type,
                    _ => {
                        return unsupported(
                            "scalar wrapper runtime index has no fixed-array backing",
                        );
                    }
                };
                path.push(checked_trees::CheckedUnitStructuralPathSegment::RuntimeIndex(selector));
                type_reference = element_type;
            }
        }
    }
    Ok((root, path, access))
}

/// An immutable integer local bound to a constant initializer resolves the
/// authored selector statically, mirroring the checker's `FixedIndex` fold of
/// the same name; mutable locals and non-constant initializers stay on the
/// runtime-selector routes.
fn constant_local_index(
    checked: &CheckedTrees,
    state: &checked_trees::state::State,
    symbol: symbols::SymbolHandle,
) -> Option<u64> {
    let local = checked
        .statement_table
        .statements(state.statement_nodes)
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

/// The dense scalar position of an immutable integer parameter of `state`,
/// matching the checker's `RuntimeIndex::Parameter` coordinate: erased and
/// non-scalar formals do not occupy positions.
fn runtime_index_parameter_position(
    checked: &CheckedTrees,
    state: &checked_trees::state::State,
    symbol: symbols::SymbolHandle,
) -> Option<u32> {
    let parameters = checked.state_parameters(state);
    let authored_position = parameters
        .iter()
        .position(|parameter| parameter.symbol == symbol)?;
    let parameter = &parameters[authored_position];
    if parameter.is_mutable
        || !matches!(
            checked.primitive_type_reference(parameter.type_reference),
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
    {
        return None;
    }
    u32::try_from(
        parameters[..authored_position]
            .iter()
            .filter(|parameter| {
                !parameter.relevance.is_erased()
                    && checked
                        .primitive_type_reference(parameter.type_reference)
                        .is_some()
            })
            .count(),
    )
    .ok()
}

fn unqualified_source_type(
    checked: &CheckedTrees,
    mut reference: TypeReferenceHandle,
) -> Result<TypeReferenceHandle, LoweringError> {
    let mut visited = Vec::new();
    loop {
        if !reference.is_valid() || visited.contains(&reference) {
            return unsupported("scalar wrapper source has an invalid type chain");
        }
        visited.push(reference);
        match checked.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            TypeReferenceNode::Reference { referee, .. } => reference = *referee,
            _ => return Ok(reference),
        }
    }
}
