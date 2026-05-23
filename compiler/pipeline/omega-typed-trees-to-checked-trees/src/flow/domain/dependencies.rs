use super::*;
use crate::lookup::{machine_by_symbol, machine_symbol_from_type_reference_handle};

#[derive(Debug, Clone, Default)]
pub(crate) struct DomainDependencyCache {
    pub(crate) by_domain: Vec<DomainDependencyCacheEntry>,
}

#[derive(Debug, Clone)]
pub(crate) struct DomainDependencyCacheEntry {
    pub(crate) domain_symbol: SymbolHandle,
    pub(crate) dependencies: Vec<Vec<omega_facts::PlaceSegment>>,
}

pub(crate) fn build_domain_facts(
    program: &omega_typed_trees::TypedTrees,
    semantic: &FactPlan,
) -> DomainFacts {
    let mut cache = DomainDependencyCache::default();
    let mut segments = omega_core::arena::Arena::new();
    let mut dependency_paths = omega_core::arena::Arena::new();
    let mut dependencies =
        omega_core::arena::Arena::with_capacity(program.domain_definitions().len());

    for domain in program.domain_definitions() {
        let dependency_segments =
            domain_dependency_segments(program, semantic, &mut cache, domain.symbol);
        let mut dependency_span = omega_core::arena::HandleSpan::empty();
        for dependency in dependency_segments {
            let mut segment_span = omega_core::arena::HandleSpan::empty();
            for segment in dependency {
                segments.append_to_span(&mut segment_span, *segment);
            }
            dependency_paths.append_to_span(
                &mut dependency_span,
                DomainDependencyPathFact {
                    segments: segment_span,
                },
            );
        }

        dependencies.append(DomainDependencyFact {
            domain_symbol: domain.symbol,
            dependencies: dependency_span,
        });
    }

    DomainFacts {
        segments,
        dependency_paths,
        dependencies,
    }
}

pub(crate) fn domain_dependency_segments<'cache>(
    program: &omega_typed_trees::TypedTrees,
    semantic: &FactPlan,
    cache: &'cache mut DomainDependencyCache,
    domain_symbol: SymbolHandle,
) -> &'cache [Vec<omega_facts::PlaceSegment>] {
    if !cache.by_domain.iter().any(|entry| entry.domain_symbol == domain_symbol) {
        let mut visiting = BTreeSet::new();
        let dependencies = compute_domain_dependency_segments(
            program,
            semantic,
            cache,
            domain_symbol,
            &mut visiting,
        );
        cache.by_domain.push(DomainDependencyCacheEntry {
            domain_symbol,
            dependencies,
        });
    }

    cache
        .by_domain
        .iter()
        .find(|entry| entry.domain_symbol == domain_symbol)
        .map(|entry| entry.dependencies.as_slice())
        .unwrap_or(&[])
}

fn compute_domain_dependency_segments(
    program: &omega_typed_trees::TypedTrees,
    semantic: &FactPlan,
    cache: &mut DomainDependencyCache,
    domain_symbol: SymbolHandle,
    visiting: &mut BTreeSet<u32>,
) -> Vec<Vec<omega_facts::PlaceSegment>> {
    if let Some(cached) = cache
        .by_domain
        .iter()
        .find(|entry| entry.domain_symbol == domain_symbol)
    {
        return cached.dependencies.clone();
    }
    let domain_key = domain_symbol.arena_index();
    if !visiting.insert(domain_key) {
        return vec![Vec::new()];
    }

    let mut dependencies = Vec::new();
    let self_type_symbol = program
        .domain_definitions()
        .iter()
        .find(|domain| domain.symbol == domain_symbol)
        .map(|domain| machine_symbol_from_type_reference_handle(program, domain.target_type))
        .filter(|symbol| symbol.is_valid());
    for fact in semantic.facts_for_symbol(domain_symbol) {
        match fact.payload {
            FactPayload::BooleanExpression(expression) => {
                collect_dependency_paths_from_expression(
                    program,
                    expression,
                    self_type_symbol,
                    &mut dependencies,
                );
            }
            FactPayload::DomainMembership {
                domain_symbol: imported_domain,
                ..
            } => {
                let FactPlace::Place(place_handle) = fact.place else {
                    dependencies.push(Vec::new());
                    continue;
                };
                let place = semantic.places.get(place_handle);
                let base_segments: Vec<_> = semantic
                    .place_segments
                    .span_or_empty(place.segments)
                    .iter()
                    .copied()
                    .collect();
                let imported_dependencies = compute_domain_dependency_segments(
                    program,
                    semantic,
                    cache,
                    imported_domain,
                    visiting,
                );
                if imported_dependencies.is_empty() {
                    dependencies.push(base_segments);
                } else {
                    for imported in imported_dependencies {
                        let mut rebased = Vec::with_capacity(
                            base_segments.len().saturating_add(imported.len()),
                        );
                        rebased.extend(base_segments.iter().copied());
                        rebased.extend(imported);
                        dependencies.push(rebased);
                    }
                }
            }
            _ => {}
        }
    }

    visiting.remove(&domain_key);
    dedupe_dependency_segments(&mut dependencies);
    dependencies
}

fn collect_dependency_paths_from_expression(
    program: &omega_typed_trees::TypedTrees,
    expression: ExpressionHandle,
    self_type_symbol: Option<SymbolHandle>,
    dependencies: &mut Vec<Vec<omega_facts::PlaceSegment>>,
) {
    if !expression.is_valid() {
        return;
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::ArrayLiteral(values) => {
            for value in program.expression_table.expression_handles(*values) {
                collect_dependency_paths_from_expression(
                    program,
                    *value,
                    self_type_symbol,
                    dependencies,
                );
            }
        }
        ExpressionNode::Binary(binary) => {
            collect_dependency_paths_from_expression(
                program,
                binary.left,
                self_type_symbol,
                dependencies,
            );
            collect_dependency_paths_from_expression(
                program,
                binary.right,
                self_type_symbol,
                dependencies,
            );
        }
        ExpressionNode::Call(call) => {
            if call.receiver.is_valid() {
                collect_dependency_paths_from_expression(
                    program,
                    call.receiver,
                    self_type_symbol,
                    dependencies,
                );
            }
            for argument in program.expression_table.expression_handles(call.arguments) {
                collect_dependency_paths_from_expression(
                    program,
                    *argument,
                    self_type_symbol,
                    dependencies,
                );
            }
        }
        ExpressionNode::Cast(cast) => {
            collect_dependency_paths_from_expression(
                program,
                cast.value,
                self_type_symbol,
                dependencies,
            );
        }
        ExpressionNode::Indexed(indexed) => {
            if let Some(place) = canonical_place_from_expression(program, expression) {
                dependencies.push(place.segments);
            } else if let Some(segments) =
                relative_place_segments_from_expression(program, expression, self_type_symbol)
            {
                dependencies.push(segments);
            } else {
                collect_dependency_paths_from_expression(
                    program,
                    indexed.collection,
                    self_type_symbol,
                    dependencies,
                );
            }
            collect_dependency_paths_from_expression(
                program,
                indexed.index,
                self_type_symbol,
                dependencies,
            );
        }
        ExpressionNode::Member(member) => {
            if let Some(place) = canonical_place_from_expression(program, expression) {
                dependencies.push(place.segments);
            } else if let Some(segments) =
                relative_place_segments_from_expression(program, expression, self_type_symbol)
            {
                dependencies.push(segments);
            } else {
                collect_dependency_paths_from_expression(
                    program,
                    member.receiver,
                    self_type_symbol,
                    dependencies,
                );
            }
        }
        ExpressionNode::Mutable(inner) => {
            collect_dependency_paths_from_expression(
                program,
                *inner,
                self_type_symbol,
                dependencies,
            );
        }
        ExpressionNode::Name(_) => {
            if let Some(place) = canonical_place_from_expression(program, expression) {
                dependencies.push(place.segments);
            } else if let Some(segments) =
                relative_place_segments_from_expression(program, expression, self_type_symbol)
            {
                dependencies.push(segments);
            }
        }
        ExpressionNode::StructLiteral(struct_literal) => {
            for field in program.expression_table.struct_fields(struct_literal.fields) {
                collect_dependency_paths_from_expression(
                    program,
                    field.value,
                    self_type_symbol,
                    dependencies,
                );
            }
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::String(_) => {}
    }
}

fn relative_place_segments_from_expression(
    program: &omega_typed_trees::TypedTrees,
    expression: ExpressionHandle,
    self_type_symbol: Option<SymbolHandle>,
) -> Option<Vec<omega_facts::PlaceSegment>> {
    if !expression.is_valid() {
        return None;
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::Mutable(inner) => {
            relative_place_segments_from_expression(program, *inner, self_type_symbol)
        }
        ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            let head = members.first()?.as_str();
            if head != "self" {
                return None;
            }

            Some(Vec::new())
        }
        ExpressionNode::Member(member) => {
            let mut segments = relative_place_segments_from_expression(
                program,
                member.receiver,
                self_type_symbol,
            )?;
            let member_symbol = if let Some(symbol) =
                resolve_member_symbol_from_type(program, self_type_symbol, member.member.as_str())
            {
                symbol
            } else {
                effective_member_symbol(program, member.receiver, member)
            };
            segments.push(omega_facts::PlaceSegment::Field {
                symbol: member_symbol,
            });
            Some(segments)
        }
        ExpressionNode::Indexed(indexed) => {
            let mut segments = relative_place_segments_from_expression(
                program,
                indexed.collection,
                self_type_symbol,
            )?;
            segments.push(omega_facts::PlaceSegment::Index {
                expression: indexed.index,
            });
            Some(segments)
        }
        _ => None,
    }
}

fn resolve_member_symbol_from_type(
    program: &omega_typed_trees::TypedTrees,
    type_symbol: Option<SymbolHandle>,
    member_name: &str,
) -> Option<SymbolHandle> {
    let type_symbol = type_symbol?;

    if let Some(data) = program
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == type_symbol)
    {
        for member in program.data_members(data) {
            match member {
                omega_typed_trees::data::DataMember::Field(field)
                    if field.name.as_str() == member_name =>
                {
                    return Some(field.symbol);
                }
                omega_typed_trees::data::DataMember::Variant(variant)
                    if variant.name.as_str() == member_name =>
                {
                    return Some(variant.symbol);
                }
                _ => {}
            }
        }
    }

    if let Some(machine) = machine_by_symbol(program, type_symbol) {
        for owned in program.machine_owned_data(machine) {
            if owned.name.as_str() == member_name {
                return Some(owned.symbol);
            }
        }
        for contained in program.machine_contained_objects(machine) {
            if contained.name.as_str() == member_name {
                return Some(contained.symbol);
            }
        }
    }

    None
}

fn dedupe_dependency_segments(dependencies: &mut Vec<Vec<omega_facts::PlaceSegment>>) {
    let mut unique: Vec<Vec<omega_facts::PlaceSegment>> = Vec::with_capacity(dependencies.len());
    for dependency in dependencies.drain(..) {
        if !unique.iter().any(|existing| {
            existing.len() == dependency.len()
                && existing
                    .iter()
                    .zip(dependency.iter())
                    .all(|(left, right)| canonical_place_segments_equal(*left, *right))
        }) {
            unique.push(dependency);
        }
    }
    *dependencies = unique;
}
