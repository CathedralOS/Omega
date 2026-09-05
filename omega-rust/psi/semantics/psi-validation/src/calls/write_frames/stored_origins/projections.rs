//! Project local or incoming parameter evidence without replaying an initializer.
//! FactPlan supplies selectors; exact declarations and selector-owned types
//! establish whether those selectors can transport the retained leaves.

use super::super::isolation::concrete_nominal_type;
use super::super::path_instantiation::aggregate_arguments::{AggregateOrigins, ReferenceLeaf};
use super::StoredLocalOrigins;
use psi_facts::{FactPlan, PlaceRoot, PlaceSegment};
use psi_symbols::SymbolKind;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::data::DataMember;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use psi_typed_trees::machine::Machine;
use psi_typed_trees::state::State;
use psi_typed_trees::statement::{StatementNode, TableLocalData};
use psi_typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

pub(super) fn moved_reference_leaves(
    program: &TypedTrees,
    state: &State,
    destination: &TableLocalData,
    expression: ExpressionHandle,
    expected: TypeReferenceHandle,
    stored: &[StoredLocalOrigins],
) -> Option<AggregateOrigins> {
    let before = program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .find(|statement| {
            matches!(statement, StatementNode::LocalData(local)
            if std::ptr::eq(local, destination))
        })?;
    reference_leaves_before_statement(
        program,
        state,
        before,
        expression,
        expected,
        Some(stored),
        None,
    )
}

/// Raw call instantiation retains symbolic binding paths. The existing caller
/// prefix transfer must expand those paths before filtering local storage.
pub(in crate::calls::write_frames) fn symbolic_reference_leaves(
    program: &TypedTrees,
    machine: &Machine,
    expression: ExpressionHandle,
    expected: TypeReferenceHandle,
    inference: &super::super::FrameInference,
) -> Option<AggregateOrigins> {
    let (state, before, _) = super::super::caller_aliases::caller_statement_at_site(
        program,
        machine,
        super::super::caller_aliases::CallerWriteSite::Expression(expression),
    )?;
    reference_leaves_before_statement(
        program,
        state,
        before,
        expression,
        expected,
        None,
        Some(inference),
    )
}

pub(in crate::calls::write_frames) fn reference_leaves_before_statement(
    program: &TypedTrees,
    state: &State,
    before: &StatementNode,
    expression: ExpressionHandle,
    expected: TypeReferenceHandle,
    stored: Option<&[StoredLocalOrigins]>,
    inference: Option<&super::super::FrameInference>,
) -> Option<AggregateOrigins> {
    let mut root_expression = expression;
    let root_name = loop {
        match program.expression_table.expression(root_expression) {
            ExpressionNode::Name(name) => break name,
            ExpressionNode::Member(member) => root_expression = member.receiver,
            ExpressionNode::Indexed(indexed) => root_expression = indexed.collection,
            _ => return None,
        }
    };
    let mut facts = FactPlan::default();
    let place = facts.append_place_from_expression(program, expression);
    let place = facts.places.get(place);
    let PlaceRoot::Symbol(root) = place.root else {
        return None;
    };
    if !root.is_valid() || root_name.head_symbol != root {
        return None;
    }
    let declaration = program.symbols.get(root);
    if declaration.parent != state.symbol {
        return None;
    }
    let parameter = (declaration.kind == SymbolKind::Parameter)
        .then(|| {
            program
                .state_parameters(state)
                .iter()
                .find(|parameter| parameter.symbol == root && !parameter.is_self)
        })
        .flatten();
    let (source_name, source_reference) = if let Some(parameter) = parameter {
        if super::super::type_reference_is_reference(program, parameter.type_reference) {
            return None;
        }
        (parameter.name.as_str(), parameter.type_reference)
    } else {
        if declaration.kind != SymbolKind::Local {
            return None;
        }
        let source = program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .take_while(|statement| !std::ptr::eq(*statement, before))
            .find_map(|statement| match statement {
                StatementNode::LocalData(local) if local.symbol == root => Some(local),
                _ => None,
            })?;
        (source.name.as_str(), source.type_reference)
    };
    let names = program
        .expression_table
        .name_path_members(root_name.members);
    if names.first()?.as_str() != source_name
        || program.symbols.name(root) != source_name
        || (names.len() == 1 && root_name.symbol != root)
    {
        return None;
    }
    let segments = facts.place_segments.span_or_empty(place.segments);
    let actual = projected_type(program, source_reference, segments)?;
    if !crate::type_references::type_references_match(program, actual, expected) {
        return None;
    }
    let declared_origins = if parameter.is_some() || stored.is_none() {
        let mut origins =
            super::type_origins::declared_origins(program, root, source_name, source_reference)?;
        if parameter.is_none()
            && let Some(cases) = inference.and_then(|inference| inference.local_cases(root))
        {
            origins.cases = cases.to_vec();
        }
        Some(origins)
    } else {
        None
    };
    let established = declared_origins.as_ref().or_else(|| {
        stored.and_then(|stored| stored.iter().find(|local| local.local_symbol == root))
    });
    if established.is_none()
        && !super::super::type_is_caller_isolated_local(program, source_reference)
    {
        return None;
    }
    let mut leaves = AggregateOrigins::default();
    for (index, selected) in segments.iter().enumerate() {
        if let PlaceSegment::Case { .. } = selected {
            let mut candidates = established?.cases.iter().filter(|case| {
                case.len() == index + 1 && prefix_matches(&segments[..index], &case[..index])
            });
            // Missing and mixed active cases are not evidence that the
            // selected payload is a private value with no reference leaves.
            if candidates
                .next()
                .is_none_or(|case| case[index] != *selected)
                || candidates.any(|case| case[index] != *selected)
            {
                return None;
            }
        }
    }
    for prior in established.into_iter().flat_map(|local| &local.references) {
        if !prefix_matches(segments, &prior.local_segments) {
            continue;
        }
        let local_segments = prior.local_segments[segments.len()..].to_vec();
        let mut local_suffix = String::new();
        for segment in &local_segments {
            match segment {
                PlaceSegment::Field { symbol } => {
                    local_suffix.push('.');
                    local_suffix.push_str(program.symbols.name(*symbol));
                }
                PlaceSegment::Case { .. } => {}
                _ => break,
            }
        }
        let mut origin = prior.origin.clone();
        if declared_origins.is_some() {
            // A type-derived wildcard describes all possible elements, but a
            // moved fixed projection selects one. Preserve that selector in
            // source evidence without narrowing the coarse write footprint.
            for (source, selected) in origin.source.segments.iter_mut().zip(segments) {
                if matches!(*source, PlaceSegment::Index { expression } if !expression.is_valid()) {
                    *source = *selected;
                }
            }
        }
        leaves.references.push(ReferenceLeaf {
            local_suffix,
            local_segments,
            origin,
        });
    }
    for case in established.into_iter().flat_map(|local| &local.cases) {
        if prefix_matches(segments, case) {
            leaves.cases.push(case[segments.len()..].to_vec());
        }
    }
    // Zero leaves follows a validated shape/projection, not failed lookup.
    // Symbolic paths still require caller-prefix expansion; only the frozen
    // transfer has already accounted for the preceding declarations here.
    Some(leaves)
}

pub(super) fn prefix_matches(selected: &[PlaceSegment], candidate: &[PlaceSegment]) -> bool {
    selected.len() <= candidate.len()
        && selected.iter().zip(candidate).all(|(selected, candidate)| {
            selected == candidate
                || matches!(
                    (selected, candidate),
                    (PlaceSegment::Index { .. }, PlaceSegment::FixedIndex { .. })
                )
                // Type-derived parameter leaves use an unknown element, not
                // a fabricated executable index. Exact/dynamic projections
                // both overlap it; projected_type separately checks bounds.
                || matches!((selected, candidate),
                    (PlaceSegment::FixedIndex { .. } | PlaceSegment::Index { .. },
                     PlaceSegment::Index { expression }) if !expression.is_valid())
        })
}

fn projected_type(
    program: &TypedTrees,
    mut reference: TypeReferenceHandle,
    segments: &[PlaceSegment],
) -> Option<TypeReferenceHandle> {
    let mut selected_case = None;
    for segment in segments {
        reference = unconstrained_type(program, reference)?;
        match (
            program.type_reference_table.type_reference(reference),
            segment,
        ) {
            (node, PlaceSegment::Case { variant }) if concrete_nominal_type(node).is_some() => {
                let (symbol, _) = concrete_nominal_type(node)?;
                if selected_case.is_some()
                    || program.symbols.get(*variant).kind != SymbolKind::Variant
                {
                    return None;
                }
                let definition = program
                    .data_definitions()
                    .iter()
                    .find(|definition| definition.symbol == symbol)?;
                selected_case =
                    program
                        .data_members(definition)
                        .iter()
                        .find_map(|member| match member {
                            DataMember::Variant(candidate) if candidate.symbol == *variant => {
                                Some(candidate)
                            }
                            _ => None,
                        });
                selected_case?;
            }
            (node, PlaceSegment::Field { symbol: field })
                if concrete_nominal_type(node).is_some() =>
            {
                let (symbol, _) = concrete_nominal_type(node)?;
                if program.symbols.get(*field).kind != SymbolKind::Field {
                    return None;
                }
                reference = if let Some(variant) = selected_case.take() {
                    program
                        .data_payload_fields(variant)
                        .iter()
                        .find(|candidate| candidate.symbol == *field)?
                        .type_reference
                } else {
                    let definition = program
                        .data_definitions()
                        .iter()
                        .find(|definition| definition.symbol == symbol)?;
                    program
                        .data_members(definition)
                        .iter()
                        .find_map(|member| match member {
                            DataMember::Field(candidate) if candidate.symbol == *field => {
                                Some(candidate.type_reference)
                            }
                            _ => None,
                        })?
                };
            }
            (
                TypeReferenceNode::FixedArray {
                    element_type,
                    length: FixedArrayLength::Literal(length),
                },
                index,
            ) => {
                if selected_case.is_some() {
                    return None;
                }
                match index {
                    PlaceSegment::FixedIndex { index } if index < length => {}
                    PlaceSegment::Index { expression }
                        if *length > 0
                            && program
                                .expression_table
                                .constant_integer_value(*expression)
                                .is_none()
                            && !matches!(
                                program.expression_table.expression(*expression),
                                ExpressionNode::Range(_)
                            ) => {}
                    _ => return None,
                }
                reference = *element_type;
            }
            // Never peel a reference to turn a loaded carrier into an owned
            // value move. Reference result relations have their own evidence.
            _ => return None,
        }
    }
    if selected_case.is_some() {
        return None;
    }
    let reference = unconstrained_type(program, reference)?;
    (!super::super::type_reference_is_reference(program, reference)).then_some(reference)
}

fn unconstrained_type(
    program: &TypedTrees,
    mut reference: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    while let TypeReferenceNode::Constrained { base_type, .. } =
        program.type_reference_table.type_reference(reference)
    {
        reference = *base_type;
    }
    reference.is_valid().then_some(reference)
}
