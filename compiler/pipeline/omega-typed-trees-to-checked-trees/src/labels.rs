use omega_checked_trees::{ContractProofFactKind, ProofFactKind};
use omega_core::symbols::SymbolHandle;
use omega_facts::{
    ContractFactKind as SemanticContractFactKind, Fact, FactPayload, FactPlan,
    ProofObligationKind as SemanticProofObligationKind,
};

pub(crate) fn semantic_fact_requirement_label(
    program: &omega_typed_trees::TypedTrees,
    semantic: &FactPlan,
    fact: &Fact,
) -> String {
    match fact.payload {
        FactPayload::ContractDomainMembership { domain_symbol, .. }
        | FactPayload::DomainMembership { domain_symbol, .. } => {
            let place = match fact.place {
                omega_facts::FactPlace::Place(place) => place,
                _ => return "unknown domain membership".to_owned(),
            };
            let place = semantic.places.get(place);
            format!(
                "{} in {}",
                requirement_place_label(program, semantic, place),
                symbol_name(program, domain_symbol)
            )
        }
        FactPayload::ContractBooleanExpression { expression, .. }
        | FactPayload::BooleanExpression(expression) => {
            program.expression_table.display_name(expression)
        }
        _ => "unknown contract fact".to_owned(),
    }
}

pub(crate) fn requirement_place_label(
    program: &omega_typed_trees::TypedTrees,
    semantic: &FactPlan,
    place: &omega_facts::Place,
) -> String {
    canonical_place_label(program, semantic, place)
}

pub(crate) fn canonical_place_label(
    program: &omega_typed_trees::TypedTrees,
    semantic: &FactPlan,
    place: &omega_facts::Place,
) -> String {
    canonical_place_label_from_parts(
        program,
        place.root,
        semantic.place_segments.span_or_empty(place.segments),
    )
}

pub(crate) fn joined_place_label(
    program: &omega_typed_trees::TypedTrees,
    semantic: &FactPlan,
    place: &omega_facts::Place,
    extra_segments: &[omega_facts::PlaceSegment],
) -> String {
    let mut segments: Vec<_> = semantic
        .place_segments
        .span_or_empty(place.segments)
        .iter()
        .copied()
        .collect();
    segments.extend(extra_segments.iter().copied());
    canonical_place_label_from_parts(program, place.root, &segments)
}

pub(crate) fn canonical_place_label_from_parts(
    program: &omega_typed_trees::TypedTrees,
    root: omega_facts::PlaceRoot,
    segments: &[omega_facts::PlaceSegment],
) -> String {
    let mut label = match root {
        omega_facts::PlaceRoot::Unknown => "unknown".to_owned(),
        omega_facts::PlaceRoot::Symbol(symbol) => symbol_name(program, symbol),
        omega_facts::PlaceRoot::Expression(expression) => {
            program.expression_table.display_name(expression)
        }
        omega_facts::PlaceRoot::TypeReference(type_reference) => {
            program.display_type_reference(type_reference)
        }
    };

    for segment in segments {
        match segment {
            omega_facts::PlaceSegment::Field { symbol } => {
                label.push('.');
                label.push_str(&symbol_name(program, *symbol));
            }
            omega_facts::PlaceSegment::Index { expression } => {
                label.push('[');
                label.push_str(&program.expression_table.display_name(*expression));
                label.push(']');
            }
        }
    }

    label
}

pub(crate) fn machine_name(
    program: &omega_typed_trees::TypedTrees,
    machine_symbol: SymbolHandle,
) -> String {
    program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
        .map(|machine| machine.name.as_str().to_owned())
        .unwrap_or_else(|| symbol_name(program, machine_symbol))
}

pub(crate) fn call_target_label(
    program: &omega_typed_trees::TypedTrees,
    target_symbol: SymbolHandle,
) -> String {
    program
        .machines()
        .iter()
        .flat_map(|machine| program.machine_states(machine).iter())
        .find(|state| state.symbol == target_symbol)
        .map(|state| state.name.as_str().to_owned())
        .unwrap_or_else(|| symbol_name(program, target_symbol))
}

pub(crate) fn symbol_name(
    program: &omega_typed_trees::TypedTrees,
    symbol: SymbolHandle,
) -> String {
    if !symbol.is_valid() {
        return "unknown".to_owned();
    }

    if let Some(machine) = program.machines().iter().find(|machine| machine.symbol == symbol) {
        return machine.name.as_str().to_owned();
    }

    if let Some(state) = program
        .machines()
        .iter()
        .flat_map(|machine| program.machine_states(machine).iter())
        .find(|state| state.symbol == symbol)
    {
        return state.name.as_str().to_owned();
    }

    if let Some(field) = program
        .data_definitions()
        .iter()
        .flat_map(|data| program.data_members(data).iter())
        .find_map(|member| match member {
            omega_typed_trees::data::DataMember::Field(field) if field.symbol == symbol => {
                Some(field.name.as_str().to_owned())
            }
            _ => None,
        })
    {
        return field;
    }

    program
        .symbols
        .name(symbol)
        .to_string()
}

pub(crate) fn semantic_contract_fact_kind(
    kind: ContractProofFactKind,
) -> SemanticContractFactKind {
    match kind {
        ContractProofFactKind::Requires => SemanticContractFactKind::Requires,
        ContractProofFactKind::Ensures => SemanticContractFactKind::Ensures,
        ContractProofFactKind::Trusted => SemanticContractFactKind::Trusted,
    }
}

pub(crate) fn semantic_proof_obligation_kind(
    kind: ProofFactKind,
) -> SemanticProofObligationKind {
    match kind {
        ProofFactKind::BoundedAssignment => SemanticProofObligationKind::BoundedAssignment,
        ProofFactKind::BoundedCallArgument => SemanticProofObligationKind::BoundedCallArgument,
        ProofFactKind::BoundedInitializer => SemanticProofObligationKind::BoundedInitializer,
        ProofFactKind::BoundedStateReturn => SemanticProofObligationKind::BoundedStateReturn,
        ProofFactKind::BoundedValue => SemanticProofObligationKind::BoundedValue,
        ProofFactKind::BoundedTransitionArgument => {
            SemanticProofObligationKind::BoundedTransitionArgument
        }
        ProofFactKind::GuardedTransition => SemanticProofObligationKind::GuardedTransition,
    }
}
