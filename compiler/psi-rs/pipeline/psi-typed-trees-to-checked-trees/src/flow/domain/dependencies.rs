mod expression;

use super::*;
use crate::lookup::{machine_by_symbol, machine_symbol_from_type_reference_handle};
use expression::{collect_dependency_paths_from_expression, dedupe_dependency_segments};

#[derive(Debug, Clone, Default)]
pub(crate) struct DomainDependencyCache {
    pub(crate) by_domain: Vec<DomainDependencyCacheEntry>,
}

#[derive(Debug, Clone)]
pub(crate) struct DomainDependencyCacheEntry {
    pub(crate) domain_symbol: SymbolHandle,
    pub(crate) dependencies: Vec<Vec<psi_facts::PlaceSegment>>,
}

pub(crate) fn build_domain_facts(
    program: &psi_typed_trees::TypedTrees,
    semantic: &FactPlan,
) -> DomainFacts {
    let mut cache = DomainDependencyCache::default();
    let mut segments = psi_arena::Arena::new();
    let mut dependency_paths = psi_arena::Arena::new();
    let mut dependencies = psi_arena::Arena::with_capacity(program.domain_definitions().len());

    for domain in program.domain_definitions() {
        let dependency_segments =
            domain_dependency_segments(program, semantic, &mut cache, domain.symbol);
        let mut dependency_span = psi_arena::HandleSpan::empty();
        for dependency in dependency_segments {
            let mut segment_span = psi_arena::HandleSpan::empty();
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

    DomainFacts::with_roots(segments, dependency_paths, dependencies)
}

pub(crate) fn domain_dependency_segments<'cache>(
    program: &psi_typed_trees::TypedTrees,
    semantic: &FactPlan,
    cache: &'cache mut DomainDependencyCache,
    domain_symbol: SymbolHandle,
) -> &'cache [Vec<psi_facts::PlaceSegment>] {
    if !cache
        .by_domain
        .iter()
        .any(|entry| entry.domain_symbol == domain_symbol)
    {
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
    program: &psi_typed_trees::TypedTrees,
    semantic: &FactPlan,
    cache: &mut DomainDependencyCache,
    domain_symbol: SymbolHandle,
    visiting: &mut BTreeSet<u32>,
) -> Vec<Vec<psi_facts::PlaceSegment>> {
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
                        let mut rebased =
                            Vec::with_capacity(base_segments.len().saturating_add(imported.len()));
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
