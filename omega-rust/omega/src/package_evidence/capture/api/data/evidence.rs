use crate::package_evidence::capture::PackageReviewInput;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecheckedDataDefinitionFact {
    pub(crate) data_symbol: SymbolHandle,
    pub(crate) fact:
        arena::Handle<symbol_resolved_trees_to_typed_trees::typed_trees::domain::ProofFact>,
    pub(crate) semantic_fact: RecheckedSemanticFact,
    pub(crate) dependencies: Vec<RecheckedDataDefinitionFactDependency>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecheckedDataDefinitionFactDependency {
    pub(crate) expression:
        symbol_resolved_trees_to_typed_trees::typed_trees::expression::ExpressionHandle,
    pub(crate) place: RecheckedFactPlace,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecheckedSemanticFact {
    pub(crate) place: RecheckedSemanticFactPlace,
    pub(crate) point: typed_trees_to_checked_trees::fact_plan::ProgramPoint,
    pub(crate) origin: typed_trees_to_checked_trees::fact_plan::FactOrigin,
    pub(crate) evidence: typed_trees_to_checked_trees::fact_plan::QualificationEvidence,
    pub(crate) payload: typed_trees_to_checked_trees::fact_plan::FactPayload,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RecheckedSemanticFactPlace {
    Unknown,
    Place(RecheckedFactPlace),
    Symbol(SymbolHandle),
    Expression(symbol_resolved_trees_to_typed_trees::typed_trees::expression::ExpressionHandle),
    TypeReference(symbol_resolved_trees_to_typed_trees::typed_trees::types::TypeReferenceHandle),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecheckedFactPlace {
    pub(crate) root: typed_trees_to_checked_trees::fact_plan::PlaceRoot,
    pub(crate) segments: Vec<typed_trees_to_checked_trees::fact_plan::PlaceSegment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecheckedDataDefinitionEvidence {
    pub(crate) definitions: Vec<RecheckedDataDefinitionFact>,
    pub(crate) semantic_facts: Vec<RecheckedSemanticFact>,
    pub(crate) refs: Vec<RecheckedSemanticFact>,
    pub(crate) contexts: Vec<RecheckedDataFactContext>,
    pub(crate) symbol_sets: Vec<RecheckedDataSymbolFactSet>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecheckedDataFactContext {
    pub(crate) point: typed_trees_to_checked_trees::fact_plan::ProgramPoint,
    pub(crate) facts: Vec<RecheckedSemanticFact>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecheckedDataSymbolFactSet {
    pub(crate) symbol: SymbolHandle,
    pub(crate) facts: Vec<RecheckedSemanticFact>,
}

pub(crate) fn require_rederived_data_definition_facts(
    compilation: &PackageReviewInput<'_>,
) -> Result<(), Vec<Diagnostic>> {
    let rederived =
        typed_trees_to_checked_trees::validation::build_definition_fact_plan(&compilation.typed);
    let data_symbols = compilation
        .data_definitions()
        .iter()
        .map(|definition| definition.symbol)
        .collect::<Vec<_>>();
    let Some(expected) = rechecked_data_definition_evidence(&rederived, &data_symbols) else {
        return Err(vec![Diagnostic::error(
            "compiler-rederived data invariant evidence is internally malformed",
        )]);
    };
    let Some(retained) =
        rechecked_data_definition_evidence(&compilation.facts.semantic, &data_symbols)
    else {
        return Err(vec![Diagnostic::error(
            "retained checked data invariant evidence is internally malformed",
        )]);
    };
    if retained != expected {
        return Err(vec![Diagnostic::error(
            "retained checked data invariant evidence disagrees with the compiler-rederived typed program",
        )]);
    }
    Ok(())
}

pub(crate) fn rechecked_data_definition_evidence(
    facts: &typed_trees_to_checked_trees::fact_plan::FactPlan,
    data_symbols: &[SymbolHandle],
) -> Option<RecheckedDataDefinitionEvidence> {
    fact_plan_arena_links_are_well_formed(facts).then_some(())?;
    let data_symbol_set: std::collections::HashSet<SymbolHandle> =
        data_symbols.iter().copied().collect();
    let definitions = facts
        .data_definition_facts
        .iter()
        .map(|(_, record)| {
            let semantic_fact = rechecked_semantic_fact(facts, record.semantic_fact)?;
            let dependencies = record
                .dependencies
                .iter()
                .map(|dependency| {
                    Some(RecheckedDataDefinitionFactDependency {
                        expression: dependency.expression,
                        place: rechecked_fact_place(facts, dependency.place)?,
                    })
                })
                .collect::<Option<Vec<_>>>()?;
            Some(RecheckedDataDefinitionFact {
                data_symbol: record.data_symbol,
                fact: record.fact,
                semantic_fact,
                dependencies,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    let semantic_facts = facts
        .facts
        .iter()
        .filter_map(|(_, fact)| {
            matches!(
                fact.origin,
                typed_trees_to_checked_trees::fact_plan::FactOrigin::DataDefinition { .. }
            )
            .then_some(rechecked_semantic_fact_value(facts, fact))
        })
        .collect::<Option<Vec<_>>>()?;
    let refs = facts
        .refs
        .iter()
        .filter_map(|(_, fact_ref)| {
            let fact = facts
                .facts
                .is_valid(fact_ref.fact)
                .then(|| facts.facts.get(fact_ref.fact))?;
            matches!(
                fact.origin,
                typed_trees_to_checked_trees::fact_plan::FactOrigin::DataDefinition { .. }
            )
            .then_some(rechecked_semantic_fact_value(facts, fact))
        })
        .collect::<Option<Vec<_>>>()?;
    let contexts = facts
        .contexts
        .iter()
        .filter_map(|(_, context)| {
            let at_data_definition = matches!(
                context.point,
                typed_trees_to_checked_trees::fact_plan::ProgramPoint::Definition { symbol }
                    if data_symbol_set.contains(&symbol)
            );
            let references = match facts.refs.span(context.facts) {
                Some(references) => references,
                None if at_data_definition => return Some(None),
                None => return None,
            };
            let contains_data_fact = references.iter().any(|fact_ref| {
                facts.facts.is_valid(fact_ref.fact)
                    && matches!(
                        facts.facts.get(fact_ref.fact).origin,
                        typed_trees_to_checked_trees::fact_plan::FactOrigin::DataDefinition { .. }
                    )
            });
            (at_data_definition || contains_data_fact).then(|| {
                Some(RecheckedDataFactContext {
                    point: context.point,
                    facts: references
                        .iter()
                        .map(|fact_ref| rechecked_semantic_fact(facts, fact_ref.fact))
                        .collect::<Option<Vec<_>>>()?,
                })
            })
        })
        .collect::<Option<Vec<_>>>()?;
    let symbol_sets = facts
        .symbol_sets
        .iter()
        .filter_map(|(_, set)| {
            let references = match facts.refs.span(set.facts) {
                Some(references) => references,
                None if data_symbol_set.contains(&set.symbol) => return Some(None),
                None => return None,
            };
            let contains_data_fact = references.iter().any(|fact_ref| {
                facts.facts.is_valid(fact_ref.fact)
                    && matches!(
                        facts.facts.get(fact_ref.fact).origin,
                        typed_trees_to_checked_trees::fact_plan::FactOrigin::DataDefinition { .. }
                    )
            });
            (data_symbol_set.contains(&set.symbol) || contains_data_fact).then(|| {
                Some(RecheckedDataSymbolFactSet {
                    symbol: set.symbol,
                    facts: references
                        .iter()
                        .map(|fact_ref| rechecked_semantic_fact(facts, fact_ref.fact))
                        .collect::<Option<Vec<_>>>()?,
                })
            })
        })
        .collect::<Option<Vec<_>>>()?;
    Some(RecheckedDataDefinitionEvidence {
        definitions,
        semantic_facts,
        refs,
        contexts,
        symbol_sets,
    })
}

pub(crate) fn fact_plan_arena_links_are_well_formed(
    facts: &typed_trees_to_checked_trees::fact_plan::FactPlan,
) -> bool {
    facts
        .places
        .iter()
        .all(|(_, place)| facts.place_segments.span(place.segments).is_some())
        && facts.facts.iter().all(|(_, fact)| match fact.place {
            typed_trees_to_checked_trees::fact_plan::FactPlace::Place(place) => {
                facts.places.is_valid(place)
            }
            typed_trees_to_checked_trees::fact_plan::FactPlace::Unknown
            | typed_trees_to_checked_trees::fact_plan::FactPlace::Symbol(_)
            | typed_trees_to_checked_trees::fact_plan::FactPlace::Expression(_)
            | typed_trees_to_checked_trees::fact_plan::FactPlace::TypeReference(_) => true,
        })
        && facts
            .refs
            .iter()
            .all(|(_, fact_ref)| facts.facts.is_valid(fact_ref.fact))
        && facts
            .contexts
            .iter()
            .all(|(_, context)| facts.refs.span(context.facts).is_some())
        && facts
            .symbol_sets
            .iter()
            .all(|(_, set)| facts.refs.span(set.facts).is_some())
}

pub(crate) fn rechecked_semantic_fact(
    facts: &typed_trees_to_checked_trees::fact_plan::FactPlan,
    fact_handle: typed_trees_to_checked_trees::fact_plan::FactHandle,
) -> Option<RecheckedSemanticFact> {
    if !facts.facts.is_valid(fact_handle) {
        return None;
    }
    let fact = facts.facts.get(fact_handle);
    rechecked_semantic_fact_value(facts, fact)
}

pub(crate) fn rechecked_semantic_fact_value(
    facts: &typed_trees_to_checked_trees::fact_plan::FactPlan,
    fact: &typed_trees_to_checked_trees::fact_plan::Fact,
) -> Option<RecheckedSemanticFact> {
    Some(RecheckedSemanticFact {
        place: rechecked_semantic_fact_place(facts, fact.place)?,
        point: fact.point,
        origin: fact.origin,
        evidence: fact.evidence,
        payload: fact.payload,
    })
}

pub(crate) fn rechecked_semantic_fact_place(
    facts: &typed_trees_to_checked_trees::fact_plan::FactPlan,
    place: typed_trees_to_checked_trees::fact_plan::FactPlace,
) -> Option<RecheckedSemanticFactPlace> {
    Some(match place {
        typed_trees_to_checked_trees::fact_plan::FactPlace::Unknown => {
            RecheckedSemanticFactPlace::Unknown
        }
        typed_trees_to_checked_trees::fact_plan::FactPlace::Place(place) => {
            RecheckedSemanticFactPlace::Place(rechecked_fact_place(facts, place)?)
        }
        typed_trees_to_checked_trees::fact_plan::FactPlace::Symbol(symbol) => {
            RecheckedSemanticFactPlace::Symbol(symbol)
        }
        typed_trees_to_checked_trees::fact_plan::FactPlace::Expression(expression) => {
            RecheckedSemanticFactPlace::Expression(expression)
        }
        typed_trees_to_checked_trees::fact_plan::FactPlace::TypeReference(type_reference) => {
            RecheckedSemanticFactPlace::TypeReference(type_reference)
        }
    })
}

pub(crate) fn rechecked_fact_place(
    facts: &typed_trees_to_checked_trees::fact_plan::FactPlan,
    place_handle: typed_trees_to_checked_trees::fact_plan::PlaceHandle,
) -> Option<RecheckedFactPlace> {
    if !facts.places.is_valid(place_handle) {
        return None;
    }
    let place = facts.places.get(place_handle);
    Some(RecheckedFactPlace {
        root: place.root,
        segments: facts.place_segments.span(place.segments)?.to_vec(),
    })
}
