//! Project local or incoming parameter evidence without replaying an initializer.
//! FactPlan supplies selectors; exact declarations and selector-owned types
//! establish whether those selectors can transport the retained leaves.

use super::super::isolation::concrete_nominal_type;
use super::super::path_instantiation::aggregate_arguments::{
    AggregateMove, AggregateOrigins, ReferenceLeaf,
};
use super::StoredLocalOrigins;
use facts::{FactPlan, PlaceRoot, PlaceSegment};
use symbols::SymbolKind;
use typed_trees::TypedTrees;
use typed_trees::data::DataMember;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TableLocalData};
use typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

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
        if parameter.is_none() {
            if let Some(cases) = inference.and_then(|inference| inference.local_cases(root)) {
                origins.cases = cases.to_vec();
            }
            if let Some(moves) = inference.and_then(|inference| inference.local_moves(root)) {
                origins.moves = moves.to_vec();
            }
        }
        Some(origins)
    } else {
        None
    };
    let established = declared_origins.as_ref().or_else(|| {
        stored.and_then(|stored| stored.iter().find(|local| local.local_symbol == root))
    });
    let Some(established) = established else {
        return (super::super::type_is_caller_isolated_local(program, source_reference)
            && !segments
                .iter()
                .any(|segment| matches!(segment, PlaceSegment::Case { .. })))
        .then(AggregateOrigins::default);
    };
    project_stored_origins(program, established, segments, declared_origins.is_some())
}

/// Project frozen or symbolic aggregate evidence after the caller validates
/// the root declaration and the selected type. Case presence remains part of
/// projection, including subtrees with no exclusive-reference leaves.
pub(in crate::calls::write_frames) fn project_stored_origins(
    program: &TypedTrees,
    established: &StoredLocalOrigins,
    segments: &[PlaceSegment],
    symbolic: bool,
) -> Option<AggregateOrigins> {
    let mut leaves = AggregateOrigins::default();
    for (index, selected) in segments.iter().enumerate() {
        if let PlaceSegment::Case { .. } = selected {
            let mut candidates = established.cases.iter().filter(|case| {
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
    for prior in &established.references {
        if !prefix_matches(segments, &prior.local_segments) {
            continue;
        }
        // Type-derived rows describe every possible branch. A caller's
        // retained cases may exclude one, including a fixed element selected
        // from a type-derived unknown-element row.
        let mut selected_path = prior.local_segments.clone();
        for (candidate, selected) in selected_path.iter_mut().zip(segments) {
            if matches!(*candidate, PlaceSegment::Index { expression } if !expression.is_valid()) {
                *candidate = *selected;
            }
        }
        if !case_path_is_possible(&selected_path, &established.cases)? {
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
        if symbolic {
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
    for case in &established.cases {
        if prefix_matches(segments, case) {
            leaves.cases.push(case[segments.len()..].to_vec());
        }
    }
    for moved in &established.moves {
        if prefix_matches(segments, &moved.local_segments) {
            leaves.moves.push(AggregateMove {
                local_segments: moved.local_segments[segments.len()..].to_vec(),
                source: moved.source.clone(),
                type_reference: moved.type_reference,
            });
        } else if prefix_matches(&moved.local_segments, segments) {
            let remaining = &segments[moved.local_segments.len()..];
            leaves.moves.push(AggregateMove {
                local_segments: Vec::new(),
                source: moved.source.append_segments(remaining),
                type_reference: projected_type(program, moved.type_reference, remaining)?,
            });
        }
    }
    // Zero leaves follows a validated shape/projection, not failed lookup.
    // Symbolic paths still require caller-prefix expansion; only the frozen
    // transfer has already accounted for the preceding declarations here.
    Some(leaves)
}

/// An excluded outer branch makes all of its inner rows absent. Otherwise,
/// missing container evidence is incomplete, not proof of a reference-free
/// subtree. Runtime selectors retain every matching possible branch.
fn case_path_is_possible(path: &[PlaceSegment], cases: &[Vec<PlaceSegment>]) -> Option<bool> {
    for (index, selected) in path.iter().enumerate() {
        if !matches!(selected, PlaceSegment::Case { .. }) {
            continue;
        }
        let mut candidates = cases.iter().filter(|case| {
            case.len() == index + 1 && prefix_matches(&path[..index], &case[..index])
        });
        let first = candidates.next()?;
        if first[index] != *selected && !candidates.any(|case| case[index] == *selected) {
            return Some(false);
        }
    }
    Some(true)
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

pub(in crate::calls::write_frames) fn projected_type(
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calls::write_frames::stored_origins::StoredWriteOrigin;
    use crate::calls::write_frames::{FramePathPrecision, FramePlaceOrigin, FrameSourcePlace};
    use symbols::SymbolHandle;

    fn symbolic_reference(
        path: Vec<PlaceSegment>,
        cases: Vec<Vec<PlaceSegment>>,
    ) -> StoredLocalOrigins {
        let local_symbol = SymbolHandle::from_parts(1, 1);
        StoredLocalOrigins {
            local_symbol,
            references: vec![StoredWriteOrigin {
                local_symbol,
                local_path: "local".into(),
                local_segments: path.clone(),
                origin: FramePlaceOrigin {
                    path: "local".into(),
                    precision: FramePathPrecision::Exact,
                    source: FrameSourcePlace {
                        root: local_symbol,
                        segments: path,
                    },
                },
            }],
            cases,
            moves: Vec::new(),
        }
    }

    #[test]
    fn symbolic_projection_distinguishes_absent_missing_and_possible_cases() {
        let program = TypedTrees::default();
        let selected = PlaceSegment::Case {
            variant: SymbolHandle::from_parts(2, 1),
        };
        let empty = PlaceSegment::Case {
            variant: SymbolHandle::from_parts(3, 1),
        };
        let mut origins = symbolic_reference(vec![selected], vec![vec![empty]]);
        let projected =
            project_stored_origins(&program, &origins, &[], true).expect("known empty case");
        assert!(projected.references.is_empty());
        assert_eq!(projected.cases, vec![vec![empty]]);

        origins.cases.clear();
        assert!(project_stored_origins(&program, &origins, &[], true).is_none());
        origins.cases = vec![vec![empty], vec![selected]];
        assert_eq!(
            project_stored_origins(&program, &origins, &[], true)
                .expect("possible selected case")
                .references
                .len(),
            1
        );

        origins.references[0].local_segments.clear();
        origins.cases.clear();
        assert_eq!(
            project_stored_origins(&program, &origins, &[], true)
                .expect("case-free reference")
                .references
                .len(),
            1
        );

        origins.references[0].local_segments = vec![selected, selected];
        origins.cases = vec![vec![empty]];
        assert!(
            project_stored_origins(&program, &origins, &[], true)
                .expect("outer case excludes inner missing evidence")
                .references
                .is_empty()
        );
    }

    #[test]
    fn symbolic_projection_checks_selected_array_element_cases() {
        let program = TypedTrees::default();
        let selected = PlaceSegment::Case {
            variant: SymbolHandle::from_parts(2, 1),
        };
        let empty = PlaceSegment::Case {
            variant: SymbolHandle::from_parts(3, 1),
        };
        let wildcard = PlaceSegment::Index {
            expression: ExpressionHandle::invalid(),
        };
        let first = PlaceSegment::FixedIndex { index: 0 };
        let second = PlaceSegment::FixedIndex { index: 1 };
        let origins = symbolic_reference(
            vec![wildcard, selected],
            vec![vec![first, empty], vec![second, selected]],
        );
        assert!(
            project_stored_origins(&program, &origins, &[first], true)
                .expect("first is empty")
                .references
                .is_empty()
        );
        assert_eq!(
            project_stored_origins(&program, &origins, &[second], true)
                .expect("second is selected")
                .references
                .len(),
            1
        );
        assert_eq!(
            project_stored_origins(&program, &origins, &[wildcard], true)
                .expect("runtime may select either")
                .references
                .len(),
            1
        );
    }
}
