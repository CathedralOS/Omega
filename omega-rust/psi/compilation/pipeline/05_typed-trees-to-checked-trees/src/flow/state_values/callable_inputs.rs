//! Callable selection supplies an independent entry under its declared inputs.
//! Each complete applied signature is one alternative arrival. Ordinary flow
//! joins it with every other arrival; requirements from different selections
//! are never conjoined. Ensures describe completed calls and supply no entry fact.

use super::{ArgumentInputs, StateValues, fields, qualifications};
use crate::flow::FlowBuildContext;
use crate::published::facts::boolean_predicates::{self, BoundPredicate};
use checked_trees::CheckedScalarExpression;
use facts::{BooleanPredicateOperand, PlaceRoot};
use typed_trees::domain::ProofFact;
use typed_trees::signature::{SignatureContractKind, StateParameter, StateSignature};
use validation::{CanonicalPlace, StaticMachineContractObligation};

pub(in crate::flow) fn seed_contract_inputs(
    program: &dyn validation::CheckingInput,
    build: &mut FlowBuildContext<'_>,
    machines: &[usize],
) {
    for &position in machines {
        let machine = &program.machines()[position];
        let rows = validation::with_private_invocation_inputs(program, machine, |contracts| {
            contracts
                .iter()
                .filter_map(|input| {
                    let destination =
                        validation::call_target_state(program, input.selected_entry())?;
                    let state = input_state(program, build.operators, destination, input);
                    Some((
                        destination,
                        fields::BoundsSource::callable_input(input),
                        state,
                    ))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
        for (destination, source, state) in rows {
            super::join(program, build, machine, destination, source, state);
        }
    }
}

fn input_state(
    program: &dyn validation::CheckingInput,
    operators: &checked_trees::CheckedOperatorFacts,
    destination: &typed_trees::state::State,
    input: &StaticMachineContractObligation,
) -> StateValues {
    let parameters = program.state_parameters(destination);
    let mut state = StateValues {
        state: destination.symbol,
        values: parameters
            .iter()
            .map(|parameter| (parameter.symbol, facts::ScalarValue::Unknown))
            .collect(),
        bounds: Vec::new(),
        fields: Vec::new(),
        field_sources: Vec::new(),
        qualifications: Vec::new(),
        predicates: Vec::new(),
        entry_paths: boolean_predicates::EntryPaths::unknown(),
        argument_edges: ArgumentInputs::Edges(Vec::new()),
        cases: Vec::new(),
    };
    let Ok(required) = input.required_signature(program) else {
        return state;
    };
    let formals = program.state_signature_parameters(required);
    if formals.len() != parameters.len() {
        return state;
    }
    for (formal, parameter) in formals.iter().zip(parameters) {
        for row in
            validation::field_domains::declared_type_domain_rows(program, formal.type_reference)
        {
            if fixed_path(&row.path) {
                state
                    .qualifications
                    .push(qualifications::QualifiedInput::declared_contract(
                        CanonicalPlace {
                            root: PlaceRoot::Symbol(parameter.symbol),
                            segments: row.path,
                        },
                        row.domain_symbol,
                        row.semantic_domain,
                        destination.symbol,
                    ));
            }
        }
    }
    for contract in program.state_signature_contracts(required) {
        if contract.kind != SignatureContractKind::Requires {
            continue;
        }
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            match fact {
                ProofFact::Expression(expression) => {
                    let Some(expression) = crate::values::lower_parameter_boolean_expression(
                        program,
                        operators,
                        required.symbol,
                        formals,
                        *expression,
                        &operators.exact_integer_casts,
                    ) else {
                        continue;
                    };
                    let Some(predicate) = boolean_predicates::bind_parameters(
                        program,
                        required.symbol,
                        required.symbol,
                        formals,
                        None,
                        expression,
                    ) else {
                        continue;
                    };
                    if let Some(predicate) =
                        rebind_predicate(program, required, destination, predicate)
                    {
                        state.predicates.push(predicate);
                    }
                }
                ProofFact::Membership(membership) => {
                    if let Some(place) =
                        validation::canonical_place_from_expression(program, membership.value)
                            .and_then(|place| rebind_place(formals, parameters, &place))
                    {
                        state.qualifications.push(
                            qualifications::QualifiedInput::declared_contract(
                                place,
                                membership.domain_symbol,
                                membership.semantic_domain,
                                destination.symbol,
                            ),
                        );
                    }
                }
                ProofFact::Proposition(_) => {
                    // An entry proposition needs its existing logical-evidence
                    // owner. Omission weakens this input and grants no theorem.
                }
            }
        }
    }
    // Use the same numeric consequences as ordinary captured predicates.
    // This makes a declared zero input and an ordinary caller passing zero
    // comparable in the existing scalar join, without combining alternatives.
    for parameter in parameters {
        let place = CanonicalPlace {
            root: PlaceRoot::Symbol(parameter.symbol),
            segments: Vec::new(),
        };
        if let Some(value) = state
            .predicates
            .iter()
            .find_map(|predicate| predicate.constant_at_place(&place, |_| None))
            && let Some((_, recorded)) = state
                .values
                .iter_mut()
                .find(|(symbol, _)| *symbol == parameter.symbol)
        {
            *recorded = value;
        }
        let bounds =
            validation::field_domains::readable_type_reference(program, parameter.type_reference)
                .and_then(|reference| program.primitive_type_reference(reference))
                .and_then(|primitive| {
                    let mut common: Option<facts::IntegerRange> = None;
                    for predicate in &state.predicates {
                        let incoming =
                            predicate.integer_interval_at_place(&place, primitive, |_| None)?;
                        common = Some(match common {
                            None => incoming,
                            Some(previous) => facts::IntegerRange {
                                minimum: previous.minimum.max(incoming.minimum),
                                maximum: previous.maximum.min(incoming.maximum),
                            },
                        });
                    }
                    common.filter(|range| range.minimum <= range.maximum)
                });
        state
            .bounds
            .push(super::ParameterBounds::new(parameter.symbol, bounds));
    }
    state
}

fn fixed_path(path: &[facts::PlaceSegment]) -> bool {
    path.iter()
        .all(|segment| !matches!(segment, facts::PlaceSegment::Index { .. }))
}

fn rebind_place(
    formals: &[StateParameter],
    parameters: &[StateParameter],
    place: &CanonicalPlace,
) -> Option<CanonicalPlace> {
    if !fixed_path(&place.segments) {
        return None;
    }
    let symbol = place.root.as_symbol()?;
    let ordinal = formals.iter().position(|formal| formal.symbol == symbol)?;
    Some(CanonicalPlace {
        root: PlaceRoot::Symbol(parameters.get(ordinal)?.symbol),
        segments: place.segments.clone(),
    })
}

fn rebind_predicate(
    program: &dyn validation::CheckingInput,
    required: &StateSignature,
    destination: &typed_trees::state::State,
    mut predicate: BoundPredicate,
) -> Option<BoundPredicate> {
    let formals = program.state_signature_parameters(required);
    let parameters = program.state_parameters(destination);
    let mut bindings = Vec::new();
    boolean_predicates::map_observations(&mut predicate.expression, &mut |operand| {
        let supplied = predicate
            .bindings
            .iter()
            .find(|binding| binding.operand == *operand)?;
        let place = rebind_place(formals, parameters, &supplied.place)?;
        let reference =
            validation::canonical_place_type_reference(program, destination.symbol, 0, &place)?;
        let reference = validation::field_domains::readable_type_reference(program, reference)?;
        if let Some(primitive) = operand.primitive_type() {
            if program.primitive_type_reference(reference) != Some(primitive) {
                return None;
            }
        } else if program.normalized_type_identity(supplied.type_reference)
            != program.normalized_type_identity(reference)
        {
            return None;
        }
        // The source formal selects the destination by identity and ordinal.
        // Canonicalizing the observed scalar also lets two independently
        // declared signatures contribute the same predicate to the meet.
        if let BooleanPredicateOperand::Scalar(CheckedScalarExpression::StorageRead {
            symbol,
            ..
        }) = operand
        {
            let ordinal = formals.iter().position(|formal| formal.symbol == *symbol)?;
            *symbol = parameters.get(ordinal)?.symbol;
        }
        let mut binding = supplied.clone();
        binding.operand = operand.clone();
        binding.place = place;
        binding.type_reference = reference;
        if !bindings.contains(&binding) {
            bindings.push(binding);
        }
        Some(())
    })?;
    predicate.machine = program.machine_holding_state(destination.symbol)?.symbol;
    predicate.entry_state = destination.symbol;
    predicate.bindings = bindings;
    Some(predicate)
}
