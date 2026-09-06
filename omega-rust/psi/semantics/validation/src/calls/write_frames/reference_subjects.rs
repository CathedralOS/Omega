//! Exact local reference identity, separate from conservative write footprints.

use super::*;
use facts::PlaceSegment;

/// Retain an unresolved read-only binding without claiming a storage identity.
/// Only the exact-reference prefix query uses this marker, after checking RHS
/// effects and binding exposure. Write-capable carriers must remain opaque to
/// the whole query because their unknown origin can hide a caller write.
pub(super) fn unknown_readonly_origin(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    name: &str,
) -> Option<FramePlaceOrigin> {
    (type_reference_is_reference(program, reference) && !type_may_carry_write(program, reference))
        .then(|| FramePlaceOrigin {
            path: name.to_owned(),
            precision: FramePathPrecision::CollectionCoarse,
            source: FrameSourcePlace::default(),
        })
}

pub(super) fn replaces_binding(
    program: &TypedTrees,
    machine: &Machine,
    statement: &StatementNode,
) -> Option<bool> {
    let StatementNode::Assignment(assignment) = statement else {
        return Some(false);
    };
    let (state, _, index) = caller_aliases::caller_statement_at_site(
        program,
        machine,
        caller_aliases::CallerWriteSite::Statement(statement),
    )?;
    let target = FrameSourcePlace::from_expression(program, assignment.target);
    if !target.segments.is_empty()
        || program.symbols.get(target.root).kind != symbols::SymbolKind::Local
    {
        return Some(false);
    }
    let reference = source_root_type(program, machine, state, index, target.root)?;
    Some(
        type_reference_is_reference(program, reference)
            && local_aliases::expression_may_rebind_mutable_alias(
                program,
                machine,
                state,
                assignment.value,
            ),
    )
}

pub(super) fn local_origin(
    program: &TypedTrees,
    machine: &Machine,
    symbols: &TopLevelSymbols<'_>,
    before: &StatementNode,
    local_symbol: SymbolHandle,
) -> Option<FrameSourcePlace> {
    let (state, _, index) = caller_aliases::caller_statement_at_site(
        program,
        machine,
        caller_aliases::CallerWriteSite::Statement(before),
    )?;
    let statements = program.statement_table.statements(state.statement_nodes);
    let mut declarations = statements[..index]
        .iter()
        .filter_map(|statement| match statement {
            StatementNode::LocalData(local) if local.symbol == local_symbol => Some(local),
            _ => None,
        });
    let local = declarations.next()?;
    if declarations.next().is_some()
        || !type_reference_is_reference(program, local.type_reference)
        || program.symbols.get(local_symbol).parent != state.symbol
        || program.symbols.get(local_symbol).kind != symbols::SymbolKind::Local
        || program.symbols.name(local_symbol) != local.name.as_str()
    {
        return None;
    }
    let prefix = walk_state_write_prefix(
        program,
        machine,
        state,
        symbols,
        &mut FrameInference::default(),
        &mut Vec::new(),
        Some(StateWriteQuery::ReferenceBefore(before)),
    )?;
    let mut origins = prefix
        .aliases
        .iter()
        .filter(|(name, _)| name == local.name.as_str());
    let (_, origin) = origins.next()?;
    if origins.next().is_some() || origin.precision != FramePathPrecision::Exact {
        return None;
    }
    let mut source = origin.source.clone();
    let source_type = source_root_type(program, machine, state, index, source.root)?;
    validate_owned_projection(program, source_type, &source.segments)?;
    // A reference local left in the result has not been canonically resolved.
    // It is not evidence that the similarly named slot kept its old referent.
    if statements[..index].iter().any(|statement| {
        matches!(statement,
        StatementNode::LocalData(local) if local.symbol == source.root
            && type_reference_is_reference(program, local.type_reference))
    }) {
        return None;
    }
    if source.root == machine.symbol {
        source.root = program
            .state_parameters(state)
            .iter()
            .find(|parameter| parameter.is_self)?
            .symbol;
    }
    Some(source)
}

/// Only structurally declared places enter the exact-reference query. A
/// transparent helper may preserve a write footprint without proving this
/// stronger identity. Loaded reference slots need their own frozen evidence.
pub(super) fn validate_initializer(
    program: &TypedTrees,
    machine: &Machine,
    mut expression: ExpressionHandle,
) -> Option<()> {
    let explicit_borrow = matches!(
        program.expression_table.expression(expression),
        ExpressionNode::Borrow(_)
    );
    while let ExpressionNode::Borrow(borrow) = program.expression_table.expression(expression) {
        expression = borrow.target;
    }
    reference_origins::declared_origin_root(program, machine, expression)?;
    let (state, _, index) = caller_aliases::caller_statement_at_site(
        program,
        machine,
        caller_aliases::CallerWriteSite::Expression(expression),
    )?;
    if !explicit_borrow
        && !type_reference_is_reference(
            program,
            crate::places::declared_place_type_raw(program, machine, Some(state), expression)?,
        )
    {
        return None;
    }
    let source = FrameSourcePlace::from_expression(program, expression);
    let reference = source_root_type(program, machine, state, index, source.root)?;
    validate_owned_projection(program, reference, &source.segments)
}

fn source_root_type(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    index: usize,
    root: SymbolHandle,
) -> Option<TypeReferenceHandle> {
    if !root.is_valid() {
        return None;
    }
    let mut parameters = program.state_parameters(state).iter().filter(|parameter| {
        parameter.symbol == root || (root == machine.symbol && parameter.is_self)
    });
    if let Some(parameter) = parameters.next() {
        return parameters
            .next()
            .is_none()
            .then_some(parameter.type_reference);
    }
    if program.symbols.get(root).parent != state.symbol
        || program.symbols.get(root).kind != symbols::SymbolKind::Local
    {
        return None;
    }
    let mut locals = program.statement_table.statements(state.statement_nodes)[..index]
        .iter()
        .filter_map(|statement| match statement {
            StatementNode::LocalData(local) if local.symbol == root => Some(local),
            _ => None,
        });
    let local = locals.next()?;
    locals.next().is_none().then_some(local.type_reference)
}

fn validate_owned_projection(
    program: &TypedTrees,
    mut reference: TypeReferenceHandle,
    segments: &[PlaceSegment],
) -> Option<()> {
    while let TypeReferenceNode::Constrained { base_type, .. } =
        program.type_reference_table.type_reference(reference)
    {
        reference = *base_type;
    }
    if let TypeReferenceNode::Reference { referee, .. } =
        program.type_reference_table.type_reference(reference)
    {
        reference = *referee;
    }
    // No collection-footprint wildcard or runtime expression establishes an
    // exact source. The shared type walk validates nominal Field/Case owners.
    if segments.iter().any(|segment| {
        !matches!(
            segment,
            PlaceSegment::Field { .. } | PlaceSegment::Case { .. }
        )
    }) {
        return None;
    }
    stored_origins::projected_type(program, reference, segments).map(|_| ())
}
