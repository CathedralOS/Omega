use checked_trees::{DomainDependencyFact, DomainDependencyPathFact, DomainFacts};
use facts::{FactPayload, FactPlan};
use std::collections::BTreeSet;
use symbols::SymbolHandle;
mod expression;

pub(crate) use expression::relative_place_segments_from_expression;
use expression::{collect_dependency_paths_from_expression, dedupe_dependency_segments};

#[derive(Debug, Clone, Default)]
pub(crate) struct DomainDependencyCache {
    pub(crate) by_domain: Vec<DomainDependencyCacheEntry>,
}

#[derive(Debug, Clone)]
pub(crate) struct DomainDependencyCacheEntry {
    pub(crate) domain_symbol: SymbolHandle,
    pub(crate) dependencies: Vec<Vec<facts::PlaceSegment>>,
}

/// Content and descriptor coordinates are distinct. A complete nominal theory
/// can depend on the descriptor's inline values without depending on the
/// separately backed content. Checked mathematical applications retain their
/// complete source captures; unknown observations remain conservative. An empty
/// ordinary dependency roster is not this proof: it can mean unknown.
pub(in crate::flow) fn qualification_is_content_independent(
    program: &dyn validation::CheckingInput,
    domain: SymbolHandle,
) -> bool {
    fn visit(
        program: &dyn validation::CheckingInput,
        symbol: SymbolHandle,
        visiting: &mut Vec<SymbolHandle>,
        completed: &mut Vec<SymbolHandle>,
    ) -> bool {
        if completed.contains(&symbol) {
            return true;
        }
        if visiting.contains(&symbol) {
            return false;
        }
        let Some(domain) = program.domain_definition(symbol) else {
            return false;
        };
        if domain.classification.is_some()
            || !domain.semantic_roles.is_empty()
            || !domain.operators.is_empty()
            || (domain.predicate_body.is_present() && program.proof_facts(domain).is_empty())
        {
            return false;
        }
        visiting.push(symbol);
        let aliases = domain.alias.as_ref().is_none_or(|alias| {
            !alias.constituents.is_empty()
                && alias.constituents.iter().all(|constituent| {
                    visit(program, constituent.domain_symbol, visiting, completed)
                })
        });
        let facts = program.proof_facts(domain).iter().all(|fact| match fact {
            typed_trees::domain::ProofFact::Membership(membership) => {
                typed_trees::domain::domain_subject_reference(program, domain, membership.value)
                    && visit(program, membership.domain_symbol, visiting, completed)
            }
            typed_trees::domain::ProofFact::Expression(expression) => {
                mathematical_captures_are_inline(program, domain, *expression)
            }
            typed_trees::domain::ProofFact::Proposition(_) => false,
        });
        visiting.pop();
        if aliases && facts {
            completed.push(symbol);
            true
        } else {
            false
        }
    }
    visit(program, domain, &mut Vec::new(), &mut Vec::new())
}

fn mathematical_captures_are_inline(
    program: &dyn validation::CheckingInput,
    domain: &typed_trees::domain::DomainDefinition,
    expression: typed_trees::expression::ExpressionHandle,
) -> bool {
    let Some(mathematics) = program.prepared_mathematics() else {
        return false;
    };
    let Some(application) = mathematics.application(expression) else {
        return false;
    };
    mathematics
        .applications
        .captures
        .span_or_empty(application.captures)
        .iter()
        .all(|capture| {
            // A scalar loaded through a reference is still a content observation.
            // Require every prefix to be inline, not merely a scalar leaf type.
            let Some(segments) =
                relative_place_segments_from_expression(program, capture.expression, domain)
            else {
                return false;
            };
            // A dynamic selector can itself observe referenced storage even
            // when the selected element is inline. The projected type alone
            // proves nothing about that observation; retain invalidation when
            // selector dependencies have not been established.
            if segments.iter().any(|segment| {
                matches!(
                    segment,
                    facts::PlaceSegment::Index { .. } | facts::PlaceSegment::OpaqueRange
                )
            }) {
                return false;
            }
            (0..=segments.len()).all(|length| {
                validation::project_type_reference_from_segments(
                    program,
                    domain.target_type,
                    &segments[..length],
                )
                .is_some_and(|reference| validation::reference_shell(program, reference).is_none())
            }) && program
                .primitive_type_reference(capture.source_type)
                .is_some()
        })
}

pub(crate) fn build_domain_facts(
    program: &dyn ::validation::CheckingInput,
    semantic: &FactPlan,
) -> DomainFacts {
    let mut cache = DomainDependencyCache::default();
    let mut segments = arena::Arena::new();
    let mut dependency_paths = arena::Arena::new();
    let mut dependencies = arena::Arena::with_capacity(program.domain_definitions().len());

    for domain in program.domain_definitions() {
        let dependency_segments =
            domain_dependency_segments(program, semantic, &mut cache, domain.symbol);
        let mut dependency_span = arena::HandleSpan::empty();
        for dependency in dependency_segments {
            let mut segment_span = arena::HandleSpan::empty();
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
    program: &dyn ::validation::CheckingInput,
    semantic: &FactPlan,
    cache: &'cache mut DomainDependencyCache,
    domain_symbol: SymbolHandle,
) -> &'cache [Vec<facts::PlaceSegment>] {
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
    program: &dyn ::validation::CheckingInput,
    semantic: &FactPlan,
    cache: &mut DomainDependencyCache,
    domain_symbol: SymbolHandle,
    visiting: &mut BTreeSet<u32>,
) -> Vec<Vec<facts::PlaceSegment>> {
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
    let Some(domain) = program.domain_definition(domain_symbol) else {
        visiting.remove(&domain_key);
        return vec![Vec::new()];
    };
    for fact in semantic.facts_for_symbol(domain_symbol) {
        match fact.payload {
            FactPayload::BooleanExpression(expression) => {
                collect_dependency_paths_from_expression(
                    program,
                    expression,
                    domain,
                    &mut dependencies,
                );
            }
            FactPayload::DomainMembership {
                domain_symbol: imported_domain,
                ..
            } => {
                let Some(place_handle) = fact.place.as_place() else {
                    dependencies.push(Vec::new());
                    continue;
                };
                let place = semantic.places.get(place_handle);
                let base_segments: Vec<_> = semantic
                    .place_segments
                    .span_or_empty(place.segments)
                    .to_vec();
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
    dedupe_dependency_segments(program, &mut dependencies);
    dependencies
}
