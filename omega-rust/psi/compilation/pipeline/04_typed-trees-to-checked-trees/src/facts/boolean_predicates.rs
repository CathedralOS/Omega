//! True relational formulas over exact current storage. Formal coordinates
//! identify observations; explicit bindings supply their current namespace.
//! Flow carries scratch rows across sweeps and allocates fresh handles on seed.

use validation::CanonicalPlace;

mod normalization;
mod paths;
use arena::HandleSpan;
use checked_trees::{CheckedBooleanExpression as Boolean, CheckedScalarExpression as Scalar};
use facts::{
    Fact, FactContextHandle, FactOrigin, FactPayload, FactPlace, FactPlan, Place, ProgramPoint,
    ScalarValue,
};
use normalization::normalize;
pub(crate) use paths::{
    EntryPaths, capture_entry_guard, conjunction, entry_path_condition, seed_entry_paths,
};
use symbols::SymbolHandle;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BoundPredicate {
    pub machine: SymbolHandle,
    pub entry_state: SymbolHandle,
    pub expression: Boolean,
    pub bindings: Vec<BoundObservation>,
}

impl BoundPredicate {
    pub(crate) fn binding(&self, operand: &Scalar) -> Option<&BoundObservation> {
        self.bindings
            .iter()
            .find(|binding| binding.operand == *operand)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BoundObservation {
    pub operand: Scalar,
    pub place: CanonicalPlace,
    pub type_reference: TypeReferenceHandle,
}

/// Rebase a whole current proposition onto authenticated invocation values.
/// Every observation must survive the same write boundary; one stable field
/// cannot authorize a clause whose other observation was changed or is local.
pub(crate) fn entry_expression(
    program: &dyn ::validation::CheckingInput,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement: usize,
    predicate: &BoundPredicate,
) -> Option<Boolean> {
    let mut expression = predicate.expression.clone();
    map_observations(&mut expression, &mut |operand| {
        let binding = predicate.binding(&operand)?;
        let (position, path) = super::crash_entry_values::entry_parameter_place(
            program,
            machine,
            state,
            statement,
            &binding.place,
        )?;
        *operand = entry_scalar(program, machine, position, path, operand.primitive_type()?)?;
        Some(())
    })?;
    Some(expression)
}

/// Authored structural positions and dense scalar positions share one checked
/// entry telescope. Preserve that distinction when publishing a transported
/// observation rather than treating a structural index as a scalar index.
pub(crate) fn entry_scalar(
    program: &dyn ::validation::CheckingInput,
    machine: SymbolHandle,
    position: u32,
    path: Vec<checked_trees::CheckedStructuralPredicatePathSegment>,
    primitive: PrimitiveType,
) -> Option<Scalar> {
    let owner = program.machine_by_symbol(machine)?;
    let entry_position = validation::activation_target_state_index(program, owner, machine)?;
    let entry = program.machine_states(owner).get(entry_position)?;
    let parameters = program.state_parameters(entry);
    if !path.is_empty() {
        let (_, _, reference, _) =
            crate::values::resolve_structural_parameter_path(program, parameters, position, &path)?;
        if program.primitive_type_reference(reference) != Some(primitive) {
            return None;
        }
        return Some(Scalar::StructuralParameterField {
            parameter_position: position,
            path,
            primitive_type: primitive,
        });
    }
    let position = usize::try_from(position).ok()?;
    if program.primitive_type_reference(parameters.get(position)?.type_reference) != Some(primitive)
    {
        return None;
    }
    Some(Scalar::Parameter {
        position: parameters[..position]
            .iter()
            .filter(|parameter| crate::values::occupies_scalar_position(program, parameter))
            .count(),
        primitive_type: primitive,
    })
}

/// Bind an invocation formula once. Dense primitive positions and authored
/// structural positions deliberately use separate lookups.
pub(crate) fn bind_entry(
    program: &dyn ::validation::CheckingInput,
    machine: SymbolHandle,
    expression: Boolean,
) -> Option<BoundPredicate> {
    let owner = program.machine_by_symbol(machine)?;
    let entry = validation::activation_target_state_index(program, owner, machine)?;
    let state = program.machine_states(owner).get(entry)?;
    bind_parameters(
        program,
        machine,
        state.symbol,
        program.state_parameters(state),
        expression,
    )
}

/// Bind checked scalar coordinates against the exact supplied telescope. The
/// caller retains declaration ownership separately from actual call positions.
pub(crate) fn bind_parameters(
    program: &dyn ::validation::CheckingInput,
    machine: SymbolHandle,
    state: SymbolHandle,
    parameters: &[typed_trees::signature::StateParameter],
    expression: Boolean,
) -> Option<BoundPredicate> {
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
            Scalar::StorageRead { symbol, .. } => {
                let parameter = parameters
                    .iter()
                    .find(|parameter| parameter.symbol == *symbol)?;
                (
                    crate::flow::canonical_place_from_symbol(*symbol)?,
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
                // An empty projection can observe a whole primitive loan.
                // Bind its readable referent carrier, as scalar contract
                // lowering does, rather than comparing the reference type
                // itself with the scalar observation. Write-only stays absent.
                let reference =
                    validation::field_domains::readable_type_reference(program, reference)?;
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
        entry_state: state,
        expression,
        bindings,
    })
}

/// Capture a selected current guard using the ordinary checked scalar algebra.
/// The leaf bindings name current storage, including named-state parameters
/// and saved locals; they make no claim about invocation-entry identity.
pub(crate) fn bind_current_guard(
    program: &dyn ::validation::CheckingInput,
    operators: &checked_trees::CheckedOperatorFacts,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: typed_trees::expression::ExpressionHandle,
    value: bool,
) -> Option<BoundPredicate> {
    let (machine, state) = validation::find_state(program, state_symbol)?;
    if !validation::has_builtin_bound_expression_meaning(program, machine, Some(state), expression)
    {
        return None;
    }
    let Scalar::Boolean(mut expression) = crate::values::lower_unit_scalar_argument(
        program,
        operators,
        state,
        statement_index,
        expression,
        PrimitiveType::Bool,
    )?
    else {
        return None;
    };
    let symbols = crate::values::unit_scalar_binding_symbols(program, state, statement_index)?;
    let mut bindings = Vec::new();
    map_observations(&mut expression, &mut |operand| {
        if bindings
            .iter()
            .any(|binding: &BoundObservation| binding.operand == *operand)
        {
            return Some(());
        }
        let place = match operand {
            Scalar::Parameter { position, .. } | Scalar::Local { position, .. } => {
                crate::flow::canonical_place_from_symbol(*symbols.get(*position)?)?
            }
            Scalar::StorageRead { symbol, .. } => {
                crate::flow::canonical_place_from_symbol(*symbol)?
            }
            Scalar::StructuralParameterField {
                parameter_position,
                path,
                ..
            } => {
                let (root, segments, _, _) = crate::values::resolve_structural_parameter_path(
                    program,
                    program.state_parameters(state),
                    *parameter_position,
                    path,
                )?;
                CanonicalPlace {
                    root: facts::PlaceRoot::Symbol(root),
                    segments,
                }
            }
            _ => return None,
        };
        let type_reference = validation::canonical_place_type_reference(
            program,
            state_symbol,
            statement_index,
            &place,
        )?;
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
        machine: machine.symbol,
        entry_state: state.symbol,
        expression: if value {
            *expression
        } else {
            Boolean::Not(expression)
        },
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
    program: &dyn ::validation::CheckingInput,
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

/// Finite truth propagation over the selected Boolean algebra. Integer
/// comparisons remain whole observations: knowing one never supplies a value
/// for either integer operand. Every round uses only already established facts.
pub(crate) fn inferred_booleans(
    predicates: &[BoundPredicate],
    ordinary: impl FnMut(&CanonicalPlace) -> Option<ScalarValue>,
) -> Vec<(CanonicalPlace, bool)> {
    let Some((expressions, observations)) = normalize(predicates) else {
        return Vec::new();
    };
    let Some(truths) = formula_closure(&expressions, &observations, ordinary) else {
        return Vec::new();
    };
    observations
        .into_iter()
        .enumerate()
        .filter_map(|(position, (place, primitive))| {
            (primitive == PrimitiveType::Bool)
                .then(|| {
                    truths.iter().find_map(|(expression, value)| {
                        (*expression == Boolean::Parameter { position })
                            .then_some((place.clone(), *value))
                    })
                })
                .flatten()
        })
        .collect()
}

type Observation = (CanonicalPlace, PrimitiveType);

fn formula_closure(
    assertions: &[Boolean],
    observations: &[Observation],
    mut ordinary: impl FnMut(&CanonicalPlace) -> Option<ScalarValue>,
) -> Option<Vec<(Boolean, bool)>> {
    fn collect(expression: &Boolean, expressions: &mut Vec<Boolean>) {
        if expressions.contains(expression) {
            return;
        }
        expressions.push(expression.clone());
        match expression {
            Boolean::Not(operand) => collect(operand, expressions),
            facts::checked_boolean_triple_pattern!(left, right, ..) => {
                collect(left, expressions);
                collect(right, expressions);
            }
            _ => {}
        }
    }
    fn put(truths: &mut Vec<(Boolean, bool)>, expression: &Boolean, value: bool) -> Option<()> {
        if let Some((_, prior)) = truths.iter().find(|(known, _)| known == expression) {
            return (*prior == value).then_some(());
        }
        truths.push((expression.clone(), value));
        Some(())
    }
    struct Inputs<'a> {
        values: &'a [Option<ScalarValue>],
    }
    impl crate::values::ScalarValueSource for Inputs<'_> {
        fn binding(&mut self, position: usize) -> Option<ScalarValue> {
            self.values.get(position).cloned().flatten()
        }
    }
    let values = observations
        .iter()
        .map(|(place, _)| ordinary(place))
        .collect::<Vec<_>>();
    let mut expressions = Vec::new();
    let mut truths = Vec::new();
    for assertion in assertions {
        collect(assertion, &mut expressions);
        put(&mut truths, assertion, true)?;
    }
    for expression in &expressions {
        if let Some(ScalarValue::Boolean(value)) = crate::values::evaluate_checked_scalar(
            &Scalar::Boolean(Box::new(expression.clone())),
            &mut Inputs { values: &values },
        ) {
            put(&mut truths, expression, value)?;
        }
    }
    loop {
        let mut additions = Vec::new();
        let truth = |expression: &Boolean| {
            truths
                .iter()
                .find_map(|(known, value)| (known == expression).then_some(*value))
        };
        for expression in &expressions {
            let result = truth(expression);
            match expression {
                Boolean::Not(operand) => {
                    if let Some(value) = truth(operand) {
                        put(&mut additions, expression, !value)?;
                    }
                    if let Some(value) = result {
                        put(&mut additions, operand, !value)?;
                    }
                }
                Boolean::And { left, right } | Boolean::Or { left, right } => {
                    let is_and = matches!(expression, Boolean::And { .. });
                    let a = truth(left);
                    let b = truth(right);
                    if a == Some(!is_and) || b == Some(!is_and) {
                        put(&mut additions, expression, !is_and)?;
                    } else if a == Some(is_and) && b == Some(is_and) {
                        put(&mut additions, expression, is_and)?;
                    }
                    if result == Some(is_and) {
                        put(&mut additions, left, is_and)?;
                        put(&mut additions, right, is_and)?;
                    } else if result == Some(!is_and) {
                        if a == Some(is_and) {
                            put(&mut additions, right, !is_and)?;
                        }
                        if b == Some(is_and) {
                            put(&mut additions, left, !is_and)?;
                        }
                    }
                }
                Boolean::Equal { left, right } => {
                    let a = truth(left);
                    let b = truth(right);
                    if let (Some(a), Some(b)) = (a, b) {
                        put(&mut additions, expression, a == b)?;
                    }
                    if let Some(result) = result {
                        if let Some(a) = a {
                            put(&mut additions, right, a == result)?;
                        }
                        if let Some(b) = b {
                            put(&mut additions, left, b == result)?;
                        }
                    }
                }
                _ => {}
            }
        }
        let previous = truths.len();
        for (expression, value) in additions {
            put(&mut truths, &expression, value)?;
        }
        if truths.len() == previous {
            return Some(truths);
        }
    }
}

/// Fold exact Boolean observations before moving a relation to a named state.
/// An unforwarded selector can disappear once the selected edge knows its value.
pub(crate) fn fold(
    predicate: &BoundPredicate,
    mut resolve: impl FnMut(&CanonicalPlace) -> Option<ScalarValue>,
) -> Option<BoundPredicate> {
    let mut result = predicate.clone();
    map_observations(&mut result.expression, &mut |operand| {
        let binding = predicate.binding(&operand)?;
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
        facts::checked_boolean_triple_pattern!(left, right, ..) => {
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
        facts::checked_boolean_triple_pattern!(left, right, ..) => {
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
        | Scalar::RepresentationView { operand, .. } => map_scalar(operand, map)?,
        Scalar::IntegerLiteral { .. }
        | Scalar::IeeeFloatLiteral { .. }
        | Scalar::ImplementationSelection { .. } => {}
        Scalar::StructuralSequenceLength { .. }
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
    ordinary: impl FnMut(&CanonicalPlace) -> Option<ScalarValue>,
) -> bool {
    truth(predicates, required, ordinary) == Some(true)
}

/// Recover either truth from the same current-observation closure. Unknown
/// remains absent; a failed positive judgment never supplies a false premise.
pub(crate) fn truth(
    predicates: &[BoundPredicate],
    required: &BoundPredicate,
    mut ordinary: impl FnMut(&CanonicalPlace) -> Option<ScalarValue>,
) -> Option<bool> {
    let mut all = predicates.to_vec();
    all.push(required.clone());
    let Some((mut expressions, observations)) = normalize(&all) else {
        return None;
    };
    let normalized_required = expressions.pop().expect("required formula");
    let Some(truths) = formula_closure(&expressions, &observations, &mut ordinary) else {
        return None;
    };
    fn known(expression: &Boolean, truths: &[(Boolean, bool)]) -> Option<bool> {
        if let Some((_, value)) = truths.iter().find(|(known, _)| known == expression) {
            return Some(*value);
        }
        match expression {
            Boolean::Constant(value) => Some(*value),
            Boolean::Not(operand) => known(operand, truths).map(|value| !value),
            Boolean::And { left, right } => match (known(left, truths), known(right, truths)) {
                (Some(false), _) | (_, Some(false)) => Some(false),
                (Some(true), Some(true)) => Some(true),
                _ => None,
            },
            Boolean::Or { left, right } => match (known(left, truths), known(right, truths)) {
                (Some(true), _) | (_, Some(true)) => Some(true),
                (Some(false), Some(false)) => Some(false),
                _ => None,
            },
            // Normalization has already joined observations by exact place and
            // primitive type. Invocation binders additionally establish capture
            // epoch agreement before presenting those places to this algebra.
            Boolean::Equal { left, right } if left == right => Some(true),
            Boolean::Equal { left, right } => Some(known(left, truths)? == known(right, truths)?),
            Boolean::IntegerComparison { kind, left, right } if left == right => Some(!matches!(
                kind,
                checked_trees::CheckedIntegerComparisonKind::LessThan
            )),
            _ => None,
        }
    }
    known(&normalized_required, &truths).or_else(|| evaluate(required, &mut ordinary))
}
