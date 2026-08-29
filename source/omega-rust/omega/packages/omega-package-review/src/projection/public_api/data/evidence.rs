use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecheckedDataDefinitionFact {
    pub(crate) data_symbol: SymbolHandle,
    pub(crate) fact: psi_arena::Handle<psi_typed_trees::domain::ProofFact>,
    pub(crate) semantic_fact: RecheckedSemanticFact,
    pub(crate) dependencies: Vec<RecheckedDataDefinitionFactDependency>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecheckedDataDefinitionFactDependency {
    pub(crate) expression: psi_typed_trees::expression::ExpressionHandle,
    pub(crate) place: RecheckedFactPlace,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecheckedSemanticFact {
    pub(crate) place: RecheckedSemanticFactPlace,
    pub(crate) point: psi_facts::ProgramPoint,
    pub(crate) origin: psi_facts::FactOrigin,
    pub(crate) evidence: psi_facts::QualificationEvidence,
    pub(crate) payload: psi_facts::FactPayload,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RecheckedSemanticFactPlace {
    Unknown,
    Place(RecheckedFactPlace),
    Symbol(SymbolHandle),
    Expression(psi_typed_trees::expression::ExpressionHandle),
    TypeReference(psi_typed_trees::types::TypeReferenceHandle),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecheckedFactPlace {
    pub(crate) root: psi_facts::PlaceRoot,
    pub(crate) segments: Vec<psi_facts::PlaceSegment>,
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
    pub(crate) point: psi_facts::ProgramPoint,
    pub(crate) facts: Vec<RecheckedSemanticFact>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecheckedDataSymbolFactSet {
    pub(crate) symbol: SymbolHandle,
    pub(crate) facts: Vec<RecheckedSemanticFact>,
}

pub(crate) fn require_rederived_data_definition_facts(
    compilation: &CheckedCompilation,
) -> Result<(), Vec<Diagnostic>> {
    let rederived = psi_facts::build_definition_fact_plan(&compilation.typed);
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
    facts: &psi_facts::FactPlan,
    data_symbols: &[SymbolHandle],
) -> Option<RecheckedDataDefinitionEvidence> {
    fact_plan_arena_links_are_well_formed(facts).then_some(())?;
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
            matches!(fact.origin, psi_facts::FactOrigin::DataDefinition { .. })
                .then_some(rechecked_semantic_fact_value(facts, fact))
        })
        .collect::<Option<Vec<_>>>()?;
    let refs = facts
        .refs
        .iter()
        .filter_map(|(_, fact_ref)| {
            let fact = facts
                .facts
                .iter()
                .find_map(|(handle, fact)| (handle == fact_ref.fact).then_some(fact))?;
            matches!(fact.origin, psi_facts::FactOrigin::DataDefinition { .. })
                .then_some(rechecked_semantic_fact_value(facts, fact))
        })
        .collect::<Option<Vec<_>>>()?;
    let contexts = facts
        .contexts
        .iter()
        .filter_map(|(_, context)| {
            let at_data_definition = matches!(
                context.point,
                psi_facts::ProgramPoint::Definition { symbol }
                    if data_symbols.contains(&symbol)
            );
            let references = match facts.refs.span(context.facts) {
                Some(references) => references,
                None if at_data_definition => return Some(None),
                None => return None,
            };
            let contains_data_fact = references.iter().any(|fact_ref| {
                facts.facts.iter().any(|(handle, fact)| {
                    handle == fact_ref.fact
                        && matches!(fact.origin, psi_facts::FactOrigin::DataDefinition { .. })
                })
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
                None if data_symbols.contains(&set.symbol) => return Some(None),
                None => return None,
            };
            let contains_data_fact = references.iter().any(|fact_ref| {
                facts.facts.iter().any(|(handle, fact)| {
                    handle == fact_ref.fact
                        && matches!(fact.origin, psi_facts::FactOrigin::DataDefinition { .. })
                })
            });
            (data_symbols.contains(&set.symbol) || contains_data_fact).then(|| {
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

pub(crate) fn fact_plan_arena_links_are_well_formed(facts: &psi_facts::FactPlan) -> bool {
    facts
        .places
        .iter()
        .all(|(_, place)| facts.place_segments.span(place.segments).is_some())
        && facts.facts.iter().all(|(_, fact)| match fact.place {
            psi_facts::FactPlace::Place(place) => facts.places.is_valid(place),
            psi_facts::FactPlace::Unknown
            | psi_facts::FactPlace::Symbol(_)
            | psi_facts::FactPlace::Expression(_)
            | psi_facts::FactPlace::TypeReference(_) => true,
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
    facts: &psi_facts::FactPlan,
    fact_handle: psi_facts::FactHandle,
) -> Option<RecheckedSemanticFact> {
    let fact = facts
        .facts
        .iter()
        .find_map(|(handle, fact)| (handle == fact_handle).then_some(fact))?;
    rechecked_semantic_fact_value(facts, fact)
}

pub(crate) fn rechecked_semantic_fact_value(
    facts: &psi_facts::FactPlan,
    fact: &psi_facts::Fact,
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
    facts: &psi_facts::FactPlan,
    place: psi_facts::FactPlace,
) -> Option<RecheckedSemanticFactPlace> {
    Some(match place {
        psi_facts::FactPlace::Unknown => RecheckedSemanticFactPlace::Unknown,
        psi_facts::FactPlace::Place(place) => {
            RecheckedSemanticFactPlace::Place(rechecked_fact_place(facts, place)?)
        }
        psi_facts::FactPlace::Symbol(symbol) => RecheckedSemanticFactPlace::Symbol(symbol),
        psi_facts::FactPlace::Expression(expression) => {
            RecheckedSemanticFactPlace::Expression(expression)
        }
        psi_facts::FactPlace::TypeReference(type_reference) => {
            RecheckedSemanticFactPlace::TypeReference(type_reference)
        }
    })
}

pub(crate) fn rechecked_fact_place(
    facts: &psi_facts::FactPlan,
    place_handle: psi_facts::PlaceHandle,
) -> Option<RecheckedFactPlace> {
    let place = facts
        .places
        .iter()
        .find_map(|(handle, place)| (handle == place_handle).then_some(place))?;
    Some(RecheckedFactPlace {
        root: place.root,
        segments: facts.place_segments.span(place.segments)?.to_vec(),
    })
}
