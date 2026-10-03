//! True relational formulas over exact current storage. Formal coordinates
//! identify observations; explicit bindings supply their current namespace.
//! Flow carries scratch rows across sweeps and allocates fresh handles on seed.
use crate::flow::CanonicalPlace;
mod paths;
use arena::HandleSpan;
use checked_trees::{CheckedBooleanExpression as Boolean, CheckedScalarExpression as Scalar};
use facts::{
    Fact, FactContextHandle, FactOrigin, FactPayload, FactPlace, FactPlan, Place, ProgramPoint,
    ScalarValue,
};
pub(crate) use paths::{
    EntryPaths, capture_entry_guard, conjunction, entry_path_condition, seed_entry_paths,
};
use symbols::SymbolHandle;
use typed_trees::{
    TypedTrees,
    types::{PrimitiveType, TypeReferenceHandle},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BoundPredicate {
    pub machine: SymbolHandle,
    pub entry_state: SymbolHandle,
    pub expression: Boolean,
    pub bindings: Vec<BoundObservation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BoundObservation {
    pub operand: Scalar,
    pub place: CanonicalPlace,
    pub type_reference: TypeReferenceHandle,
}

/// Bind an invocation formula once. Dense primitive positions and authored
/// structural positions deliberately use separate lookups.
pub(crate) fn bind_entry(
    program: &TypedTrees,
    machine: SymbolHandle,
    expression: Boolean,
) -> Option<BoundPredicate> {
    let owner = program.machine_by_symbol(machine)?;
    let entry =
        crate::checks::termination::named_transition_target_state_index(program, owner, machine)?;
    let state = program.machine_states(owner).get(entry)?;
    let parameters = program.state_parameters(state);
    let mut bindings = Vec::new();
    let mut inspection = expression.clone();
    map_observations(&mut inspection, &mut |operand| {
        if bindings
            .iter()
            .any(|binding: &BoundObservation| binding.operand == *operand)
        {
            return Some(());
        }
        let (place, type_reference) = match operand {
            Scalar::Parameter { position, .. } => {
                let parameter = parameters
                    .iter()
                    .filter(|parameter| crate::values::occupies_scalar_position(program, parameter))
                    .nth(*position)?;
                (
                    crate::flow::canonical_place_from_symbol(parameter.symbol)?,
                    parameter.type_reference,
                )
            }
            Scalar::StructuralParameterField {
                parameter_position,
                path,
                ..
            } => {
                let (root, segments, reference, _) =
                    crate::values::resolve_structural_parameter_path(
                        program,
                        parameters,
                        *parameter_position,
                        path,
                    )?;
                (
                    CanonicalPlace {
                        root: facts::PlaceRoot::Symbol(root),
                        segments,
                    },
                    reference,
                )
            }
            _ => return None,
        };
        if program.primitive_type_reference(type_reference) != operand.primitive_type() {
            return None;
        }
        bindings.push(BoundObservation {
            operand: operand.clone(),
            place,
            type_reference,
        });
        Some(())
    })?;
    Some(BoundPredicate {
        machine,
        entry_state: state.symbol,
        expression,
        bindings,
    })
}

/// Install one assertion, never separately assert its subexpressions. Every
/// observed projection belongs to the same invalidation context.
pub(crate) fn seed(
    semantic: &mut FactPlan,
    predicate: &BoundPredicate,
    point: ProgramPoint,
    origin: FactOrigin,
) -> FactContextHandle {
    let mut bindings = HandleSpan::empty();
    let mut places = Vec::new();
    for binding in &predicate.bindings {
        let segments = semantic
            .place_segments
            .insert_many(binding.place.segments.iter().copied());
        let place = semantic.append_place(Place {
            root: binding.place.root,
            segments,
        });
        semantic.boolean_predicate_bindings.append_to_span(
            &mut bindings,
            facts::BooleanPredicateBinding {
                operand: binding.operand.clone(),
                place,
                type_reference: binding.type_reference,
            },
        );
        places.push(place);
    }
    let predicate = semantic.boolean_predicates.append(facts::BooleanPredicate {
        role: facts::BooleanPredicateRole::CurrentObservation,
        machine: predicate.machine,
        entry_state: predicate.entry_state,
        expression: predicate.expression.clone(),
        bindings,
    });
    let fact = semantic.append_fact(Fact {
        place: FactPlace::Unknown,
        point,
        origin,
        evidence: Default::default(),
        payload: FactPayload::BooleanPredicate { predicate },
    });
    let mut refs = HandleSpan::empty();
    semantic.append_ref(&mut refs, fact);
    for place in places {
        let dependency = semantic.append_fact(Fact {
            place: FactPlace::Place(place),
            point,
            origin,
            evidence: Default::default(),
            payload: FactPayload::StorageDependency { dependent: fact },
        });
        semantic.append_ref(&mut refs, dependency);
    }
    semantic.append_context(point, refs)
}

pub(crate) fn retained(
    program: &TypedTrees,
    semantic: &FactPlan,
    contexts: &[FactContextHandle],
) -> Vec<BoundPredicate> {
    let mut rows = Vec::new();
    for context in contexts {
        for fact in semantic
            .context_view(semantic.contexts.get(*context))
            .facts()
        {
            let FactPayload::BooleanPredicate { predicate } = fact.payload else {
                continue;
            };
            if !semantic.boolean_predicates.is_valid(predicate) {
                continue;
            }
            let predicate = semantic.boolean_predicates.get(predicate);
            if predicate.role != facts::BooleanPredicateRole::CurrentObservation {
                continue;
            }
            let Some(bindings) = semantic.boolean_predicate_bindings.span(predicate.bindings)
            else {
                continue;
            };
            let Some(bindings) = bindings
                .iter()
                .map(|binding| {
                    if !semantic.places.is_valid(binding.place) {
                        return None;
                    }
                    Some(BoundObservation {
                        operand: binding.operand.clone(),
                        type_reference: binding.type_reference,
                        place: crate::flow::canonical_place_from_semantic_place(
                            program,
                            semantic,
                            semantic.places.get(binding.place),
                        )?,
                    })
                })
                .collect::<Option<Vec<_>>>()
            else {
                continue;
            };
            let row = BoundPredicate {
                machine: predicate.machine,
                entry_state: predicate.entry_state,
                expression: predicate.expression.clone(),
                bindings,
            };
            if !rows.contains(&row) {
                rows.push(row);
            }
        }
    }
    rows
}

struct Values<'a, Resolve> {
    predicate: &'a BoundPredicate,
    resolve: Resolve,
}
impl<Resolve: FnMut(&CanonicalPlace) -> Option<ScalarValue>> crate::values::ScalarValueSource
    for Values<'_, Resolve>
{
    fn binding(&mut self, position: usize) -> Option<ScalarValue> {
        let binding = self.predicate.bindings.iter().find(|binding| matches!(&binding.operand, Scalar::Parameter { position: candidate, .. } | Scalar::Local { position: candidate, .. } if *candidate == position))?;
        (self.resolve)(&binding.place)
    }
    fn storage(&mut self, symbol: SymbolHandle) -> Option<ScalarValue> {
        let binding = self.predicate.bindings.iter().find(|binding| {
            matches!(&binding.operand,
            Scalar::StorageRead { symbol: candidate, .. } if *candidate == symbol)
        })?;
        (self.resolve)(&binding.place)
    }

    fn structural_field(
        &mut self,
        parameter_position: u32,
        path: &[checked_trees::CheckedStructuralPredicatePathSegment],
    ) -> Option<ScalarValue> {
        let binding = self.predicate.bindings.iter().find(|binding| matches!(&binding.operand, Scalar::StructuralParameterField { parameter_position: candidate, path: candidate_path, .. } if *candidate == parameter_position && candidate_path == path))?;
        (self.resolve)(&binding.place)
    }
}

pub(crate) fn evaluate(
    predicate: &BoundPredicate,
    resolve: impl FnMut(&CanonicalPlace) -> Option<ScalarValue>,
) -> Option<bool> {
    match crate::values::evaluate_checked_scalar(
        &Scalar::Boolean(Box::new(predicate.expression.clone())),
        &mut Values { predicate, resolve },
    )? {
        ScalarValue::Boolean(value) => Some(value),
        _ => None,
    }
}

/// A finite closure over the Boolean storage observations already in live
/// formulas. A round reads only previously established values; unknown cycles
/// cannot bootstrap themselves. Contradictory polarities grant no conclusion.
pub(crate) fn inferred_booleans(
    predicates: &[BoundPredicate],
    mut ordinary: impl FnMut(&CanonicalPlace) -> Option<ScalarValue>,
) -> Vec<(CanonicalPlace, bool)> {
    let mut subjects = Vec::new();
    for predicate in predicates {
        for binding in &predicate.bindings {
            if binding.operand.primitive_type() == Some(PrimitiveType::Bool)
                && !subjects.contains(&binding.place)
            {
                subjects.push(binding.place.clone());
            }
        }
    }
    let mut known = Vec::<(CanonicalPlace, bool)>::new();
    for subject in &subjects {
        if let Some(ScalarValue::Boolean(value)) = ordinary(subject) {
            known.push((subject.clone(), value));
        }
    }
    loop {
        let mut additions = Vec::new();
        for subject in &subjects {
            if known.iter().any(|(place, _)| place == subject) {
                continue;
            }
            let contradictory = [false, true].map(|candidate| {
                predicates.iter().any(|predicate| {
                    evaluate(predicate, |place| {
                        if place == subject {
                            return Some(ScalarValue::Boolean(candidate));
                        }
                        known
                            .iter()
                            .find(|(known, _)| known == place)
                            .map(|(_, value)| ScalarValue::Boolean(*value))
                            .or_else(|| ordinary(place))
                    }) == Some(false)
                })
            });
            match contradictory {
                [true, false] => additions.push((subject.clone(), true)),
                [false, true] => additions.push((subject.clone(), false)),
                [true, true] => return Vec::new(),
                [false, false] => {}
            }
        }
        if additions.is_empty() {
            break;
        }
        known.extend(additions);
        if predicates.iter().any(|predicate| {
            evaluate(predicate, |place| {
                known
                    .iter()
                    .find(|(known, _)| known == place)
                    .map(|(_, value)| ScalarValue::Boolean(*value))
                    .or_else(|| ordinary(place))
            }) == Some(false)
        }) {
            return Vec::new();
        }
    }
    known
}

/// Fold exact Boolean observations before moving a relation to a named state.
/// An unforwarded selector can disappear once the selected edge knows its value.
pub(crate) fn fold(
    predicate: &BoundPredicate,
    mut resolve: impl FnMut(&CanonicalPlace) -> Option<ScalarValue>,
) -> Option<BoundPredicate> {
    let mut result = predicate.clone();
    map_observations(&mut result.expression, &mut |operand| {
        let binding = predicate
            .bindings
            .iter()
            .find(|binding| binding.operand == *operand)?;
        if let Some(ScalarValue::Boolean(value)) = resolve(&binding.place)
            && operand.primitive_type() == Some(PrimitiveType::Bool)
        {
            *operand = Scalar::Boolean(Box::new(Boolean::Constant(value)));
        }
        Some(())
    })?;
    simplify(&mut result.expression);
    let mut used = Vec::new();
    map_observations(&mut result.expression, &mut |operand| {
        used.push(operand.clone());
        Some(())
    })?;
    result
        .bindings
        .retain(|binding| used.contains(&binding.operand));
    Some(result)
}

fn simplify(expression: &mut Boolean) {
    match expression {
        Boolean::Not(operand) => {
            simplify(operand);
            if let Boolean::Constant(value) = operand.as_ref() {
                *expression = Boolean::Constant(!value);
            }
        }
        Boolean::And { left, right }
        | Boolean::Or { left, right }
        | Boolean::Equal { left, right } => {
            simplify(left);
            simplify(right);
            let replacement = match &*expression {
                Boolean::And { left, right } => match (left.as_ref(), right.as_ref()) {
                    (Boolean::Constant(false), _) | (_, Boolean::Constant(false)) => {
                        Some(Boolean::Constant(false))
                    }
                    (Boolean::Constant(true), other) | (other, Boolean::Constant(true)) => {
                        Some(other.clone())
                    }
                    _ => None,
                },
                Boolean::Or { left, right } => match (left.as_ref(), right.as_ref()) {
                    (Boolean::Constant(true), _) | (_, Boolean::Constant(true)) => {
                        Some(Boolean::Constant(true))
                    }
                    (Boolean::Constant(false), other) | (other, Boolean::Constant(false)) => {
                        Some(other.clone())
                    }
                    _ => None,
                },
                Boolean::Equal { left, right } => match (left.as_ref(), right.as_ref()) {
                    (Boolean::Constant(a), Boolean::Constant(b)) => Some(Boolean::Constant(a == b)),
                    _ => None,
                },
                _ => None,
            };
            if let Some(replacement) = replacement {
                *expression = replacement;
            }
        }
        _ => {}
    }
}

/// Traverse the existing selected algebra. This does not infer a primitive
/// meaning for authored operators or treat opaque observations as values.
pub(crate) fn map_observations(
    expression: &mut Boolean,
    map: &mut impl FnMut(&mut Scalar) -> Option<()>,
) -> Option<()> {
    match expression {
        Boolean::Constant(_) => {}
        Boolean::Parameter { position } => {
            let mut scalar = Scalar::Parameter {
                position: *position,
                primitive_type: PrimitiveType::Bool,
            };
            map(&mut scalar)?;
            *expression = boolean_operand(scalar)?;
        }
        Boolean::Local { position } => {
            let mut scalar = Scalar::Local {
                position: *position,
                primitive_type: PrimitiveType::Bool,
            };
            map(&mut scalar)?;
            *expression = boolean_operand(scalar)?;
        }
        Boolean::StorageRead { symbol } => {
            let mut scalar = Scalar::StorageRead {
                symbol: *symbol,
                primitive_type: PrimitiveType::Bool,
            };
            map(&mut scalar)?;
            *expression = boolean_operand(scalar)?;
        }
        Boolean::StructuralParameterField {
            parameter_position,
            path,
        } => {
            let mut scalar = Scalar::StructuralParameterField {
                parameter_position: *parameter_position,
                path: path.clone(),
                primitive_type: PrimitiveType::Bool,
            };
            map(&mut scalar)?;
            *expression = boolean_operand(scalar)?;
        }
        Boolean::Not(operand) => map_observations(operand, map)?,
        Boolean::And { left, right }
        | Boolean::Or { left, right }
        | Boolean::Equal { left, right } => {
            map_observations(left, map)?;
            map_observations(right, map)?;
        }
        Boolean::IntegerComparison { left, right, .. }
        | Boolean::ScalarIeeeFloatComparison { left, right, .. } => {
            map_scalar(left, map)?;
            map_scalar(right, map)?;
        }
        Boolean::IntegerMathComparison { left, right, .. } => {
            for expression in [left, right] {
                let handles = expression
                    .terms
                    .iter()
                    .map(|(handle, _)| handle)
                    .collect::<Vec<_>>();
                for handle in handles {
                    if let checked_trees::CheckedIntegerMathTerm::Embed { operand, .. } =
                        expression.terms.get_mut(handle)
                    {
                        map_scalar(operand, map)?;
                    }
                }
            }
        }
        // These observations need non-scalar storage bindings, not an invented
        // scalar read. Their owner must supply that relation before seeding.
        Boolean::ErasedParameter { .. }
        | Boolean::IeeeFloatComparison { .. }
        | Boolean::ByteSequenceEqual { .. }
        | Boolean::ByteSequencePredicate { .. }
        | Boolean::PayloadlessSumEqual { .. }
        | Boolean::StructuralCaseMembership { .. } => return None,
    }
    Some(())
}

fn boolean_operand(operand: Scalar) -> Option<Boolean> {
    match operand {
        Scalar::Parameter {
            position,
            primitive_type: PrimitiveType::Bool,
        } => Some(Boolean::Parameter { position }),
        Scalar::StructuralParameterField {
            parameter_position,
            path,
            primitive_type: PrimitiveType::Bool,
        } => Some(Boolean::StructuralParameterField {
            parameter_position,
            path,
        }),
        Scalar::Local {
            position,
            primitive_type: PrimitiveType::Bool,
        } => Some(Boolean::Local { position }),
        Scalar::StorageRead {
            symbol,
            primitive_type: PrimitiveType::Bool,
        } => Some(Boolean::StorageRead { symbol }),
        Scalar::Boolean(expression) => Some(*expression),
        _ => None,
    }
}

pub(crate) fn map_scalar(
    expression: &mut Scalar,
    map: &mut impl FnMut(&mut Scalar) -> Option<()>,
) -> Option<()> {
    match expression {
        Scalar::Parameter { .. }
        | Scalar::StructuralParameterField { .. }
        | Scalar::Local { .. }
        | Scalar::StorageRead { .. } => map(expression)?,
        Scalar::Boolean(expression) => map_observations(expression, map)?,
        Scalar::IntegerBinary { left, right, .. } => {
            map_scalar(left, map)?;
            map_scalar(right, map)?;
        }
        Scalar::IntegerBitwiseNot { operand, .. }
        | Scalar::IntegerWiden { operand, .. }
        | Scalar::IntegerExactCast { operand, .. }
        | Scalar::IntegerWrappingCast { operand, .. }
        | Scalar::IntegerSaturatingCast { operand, .. }
        | Scalar::IntegerTrappingCast { operand, .. }
        | Scalar::RepresentationView { operand, .. } => map_scalar(operand, map)?,
        Scalar::IntegerLiteral { .. } | Scalar::IeeeFloatLiteral { .. } => {}
        Scalar::StructuralParameterByteLength { .. }
        | Scalar::ErasedParameter { .. }
        | Scalar::StructuralParameterIndexedRead { .. } => return None,
    }
    Some(())
}

/// Prove a whole bound formula, including an implication whose selector is
/// still unknown. Retained formulas compare only after their observations are
/// normalized through exact current places, not declaration positions.
pub(crate) fn proves(
    predicates: &[BoundPredicate],
    required: &BoundPredicate,
    mut ordinary: impl FnMut(&CanonicalPlace) -> Option<ScalarValue>,
) -> bool {
    let inferred = inferred_booleans(predicates, &mut ordinary);
    if evaluate(required, |place| {
        inferred
            .iter()
            .find(|(known, _)| known == place)
            .map(|(_, value)| ScalarValue::Boolean(*value))
            .or_else(|| ordinary(place))
    }) == Some(true)
    {
        return true;
    }
    let mut observations = Vec::new();
    let mut normalize = |predicate: &BoundPredicate| -> Option<Boolean> {
        let mut expression = predicate.expression.clone();
        map_observations(&mut expression, &mut |operand| {
            let binding = predicate
                .bindings
                .iter()
                .find(|binding| binding.operand == *operand)?;
            let primitive_type = operand.primitive_type()?;
            let key = (binding.place.clone(), primitive_type);
            let position = observations
                .iter()
                .position(|known| known == &key)
                .unwrap_or_else(|| {
                    let position = observations.len();
                    observations.push(key);
                    position
                });
            *operand = Scalar::Parameter {
                position,
                primitive_type,
            };
            Some(())
        })?;
        Some(expression)
    };
    let Some(required) = normalize(required) else {
        return false;
    };
    predicates
        .iter()
        .filter_map(&mut normalize)
        .any(|retained| formula_implies(&retained, &required))
}

fn formula_implies(retained: &Boolean, required: &Boolean) -> bool {
    if retained == required || matches!(required, Boolean::Constant(true)) {
        return true;
    }
    if let Boolean::And { left, right } = retained
        && (formula_implies(left, required) || formula_implies(right, required))
    {
        return true;
    }
    match required {
        Boolean::And { left, right } => {
            formula_implies(retained, left) && formula_implies(retained, right)
        }
        Boolean::Or { left, right } => {
            formula_implies(retained, left) || formula_implies(retained, right)
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (
        TypedTrees,
        SymbolHandle,
        Vec<checked_trees::CheckedStructuralPredicatePathSegment>,
    ) {
        let program = crate::tests::front_end::typed_program(
            "data Cell { ready: bool; } machine probe(flag: bool, first: Cell, second: Cell) {}",
        );
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "probe")
            .unwrap()
            .symbol;
        let identity = program
            .data_definitions()
            .iter()
            .flat_map(|data| program.data_members(data))
            .find_map(|member| match member {
                typed_trees::data::DataMember::Field(field) if field.name.as_str() == "ready" => {
                    Some(field.path_identity())
                }
                _ => None,
            })
            .unwrap();
        (
            program,
            machine,
            vec![checked_trees::CheckedStructuralPredicatePathSegment::Field(
                identity,
            )],
        )
    }

    fn implication(antecedent: Boolean, consequence: Boolean) -> Boolean {
        Boolean::Or {
            left: Box::new(Boolean::Not(Box::new(antecedent))),
            right: Box::new(consequence),
        }
    }

    fn place(predicate: &BoundPredicate, scalar: bool) -> CanonicalPlace {
        predicate
            .bindings
            .iter()
            .find(|binding| matches!(binding.operand, Scalar::Parameter { .. }) == scalar)
            .unwrap()
            .place
            .clone()
    }

    #[test]
    fn relational_predicate_selects_only_the_guarded_consequence() {
        let (program, machine, path) = fixture();
        let flag = Boolean::Parameter { position: 0 };
        let first = Boolean::StructuralParameterField {
            parameter_position: 1,
            path: path.clone(),
        };
        let second = Boolean::StructuralParameterField {
            parameter_position: 2,
            path,
        };
        let first = bind_entry(
            &program,
            machine,
            implication(Boolean::Not(Box::new(flag.clone())), first),
        )
        .unwrap();
        let second = bind_entry(&program, machine, implication(flag, second)).unwrap();
        let flag = place(&first, true);
        let first_place = place(&first, false);
        let second_place = place(&second, false);
        let rows = [first, second];
        assert!(inferred_booleans(&rows, |_| None).is_empty());
        for value in [false, true] {
            let inferred = inferred_booleans(&rows, |place| {
                (place == &flag).then_some(ScalarValue::Boolean(value))
            });
            let (dropped, returned) = if value {
                (&second_place, &first_place)
            } else {
                (&first_place, &second_place)
            };
            assert!(inferred.contains(&(dropped.clone(), true)));
            assert!(!inferred.iter().any(|(place, _)| place == returned));
        }
    }

    #[test]
    fn relational_predicate_cycles_and_opposite_conclusions_supply_no_values() {
        let (program, machine, path) = fixture();
        let flag = Boolean::Parameter { position: 0 };
        let ready = Boolean::StructuralParameterField {
            parameter_position: 1,
            path,
        };
        let cycle = [
            bind_entry(&program, machine, implication(flag.clone(), ready.clone())).unwrap(),
            bind_entry(&program, machine, implication(ready.clone(), flag)).unwrap(),
        ];
        assert!(inferred_booleans(&cycle, |_| None).is_empty());
        let contradictory = [
            bind_entry(&program, machine, ready.clone()).unwrap(),
            bind_entry(&program, machine, Boolean::Not(Box::new(ready))).unwrap(),
        ];
        assert!(inferred_booleans(&contradictory, |_| None).is_empty());
    }

    #[test]
    fn relational_predicate_forwarding_matches_places_not_formal_positions() {
        let (program, machine, path) = fixture();
        let retained = bind_entry(
            &program,
            machine,
            implication(
                Boolean::Parameter { position: 0 },
                Boolean::StructuralParameterField {
                    parameter_position: 2,
                    path,
                },
            ),
        )
        .unwrap();
        let mut required = retained.clone();
        let remap = |operand: &mut Scalar| {
            match operand {
                Scalar::Parameter { position, .. } => *position += 7,
                Scalar::StructuralParameterField {
                    parameter_position, ..
                } => *parameter_position += 9,
                _ => unreachable!(),
            }
            Some(())
        };
        map_observations(&mut required.expression, &mut |operand| remap(operand)).unwrap();
        for binding in &mut required.bindings {
            remap(&mut binding.operand).unwrap();
        }
        assert!(proves(std::slice::from_ref(&retained), &required, |_| None));
        let flag = place(&required, true);
        let owner = program.machine_by_symbol(machine).unwrap();
        let entry = crate::checks::termination::named_transition_target_state_index(
            &program, owner, machine,
        )
        .unwrap();
        let first = program.state_parameters(&program.machine_states(owner)[entry])[1].symbol;
        required
            .bindings
            .iter_mut()
            .find(|binding| binding.place != flag)
            .unwrap()
            .place
            .root = facts::PlaceRoot::Symbol(first);
        assert!(!proves(&[retained], &required, |_| None));
    }

    #[test]
    fn relational_predicate_invalidation_retires_every_read_dependency_together() {
        let (program, machine, path) = fixture();
        let predicate = bind_entry(
            &program,
            machine,
            implication(
                Boolean::Parameter { position: 0 },
                Boolean::StructuralParameterField {
                    parameter_position: 2,
                    path: path.clone(),
                },
            ),
        )
        .unwrap();
        let unrelated = bind_entry(
            &program,
            machine,
            Boolean::StructuralParameterField {
                parameter_position: 1,
                path,
            },
        )
        .unwrap()
        .bindings[0]
            .place
            .clone();
        let point = ProgramPoint::State {
            machine_symbol: machine,
            state_symbol: predicate.entry_state,
        };
        for (mutation, preserved) in [
            (place(&predicate, true), false),
            (place(&predicate, false), false),
            (unrelated, true),
        ] {
            let mut semantic = FactPlan::default();
            let context = seed(
                &mut semantic,
                &predicate,
                point,
                FactOrigin::StatementTransfer,
            );
            let mut references = arena::Arena::default();
            let source =
                references.insert_many([checked_trees::FlowSemanticContextRef { context }]);
            let surviving = crate::flow::filter_contexts_after_place_mutations(
                &program,
                &semantic,
                &Default::default(),
                &mut references,
                &mut Default::default(),
                &mut Default::default(),
                source,
                &[mutation],
                checked_trees::FlowInvalidationSource::Statement { statement_index: 0 },
            );
            let contexts = references
                .span_or_empty(surviving)
                .iter()
                .map(|reference| reference.context)
                .collect::<Vec<_>>();
            assert_eq!(
                !retained(&program, &semantic, &contexts).is_empty(),
                preserved
            );
        }
    }
}
