//! True relational formulas over exact current storage. Formal coordinates
//! identify observations; explicit bindings supply their current namespace.
//! Flow carries scratch rows across sweeps and allocates fresh handles on seed.

use validation::CanonicalPlace;

mod availability;
pub(crate) use availability::CasePremiseAvailability;
mod normalization;
mod paths;
mod retention;
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
pub(crate) use retention::retained;
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

    /// A positive conjunction can establish an exact scalar equation. A
    /// disjunction supplies no chosen branch. Resolve the opposite leaf only
    /// from ordinary snapshots in this predicate's surviving context: using
    /// this equation (or another guarantee) recursively would invent evidence.
    /// Every binding is a storage dependency of the whole retained predicate.
    pub(crate) fn constant_at_place(
        &self,
        place: &CanonicalPlace,
        mut ordinary: impl FnMut(&CanonicalPlace) -> Option<ScalarValue>,
    ) -> Option<ScalarValue> {
        fn equation(
            predicate: &BoundPredicate,
            expression: &Boolean,
            place: &CanonicalPlace,
            ordinary: &mut impl FnMut(&CanonicalPlace) -> Option<ScalarValue>,
        ) -> Option<ScalarValue> {
            match expression {
                Boolean::And { left, right } => equation(predicate, left, place, ordinary)
                    .or_else(|| equation(predicate, right, place, ordinary)),
                Boolean::IntegerComparison {
                    kind: checked_trees::CheckedIntegerComparisonKind::Equal,
                    left,
                    right,
                } => [(left, right), (right, left)]
                    .into_iter()
                    .find_map(|(subject, value)| {
                        (predicate.binding(subject)?.place == *place).then_some(())?;
                        crate::values::evaluate_checked_scalar(
                            value,
                            &mut Values {
                                predicate,
                                resolve: &mut *ordinary,
                            },
                        )
                    }),
                _ => None,
            }
        }
        equation(self, &self.expression, place, &mut ordinary)
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
            observation: Default::default(),
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
        None,
        expression,
    )
}

/// Bind checked scalar coordinates against the exact supplied telescope. The
/// caller retains declaration ownership separately from actual call positions.
/// Results occupy the trailing scalar/structural position in their respective
/// namespaces and bind to an exact invocation value, never a synthetic formal.
pub(crate) fn bind_parameters(
    program: &dyn ::validation::CheckingInput,
    machine: SymbolHandle,
    state: SymbolHandle,
    parameters: &[typed_trees::signature::StateParameter],
    result: Option<(&CanonicalPlace, TypeReferenceHandle)>,
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
                let mut scalars = parameters.iter().filter(|parameter| {
                    crate::values::occupies_scalar_position(program, parameter)
                });
                if let Some(parameter) = scalars.nth(*position) {
                    (
                        crate::flow::canonical_place_from_symbol(parameter.symbol)?,
                        parameter.type_reference,
                    )
                } else if *position
                    == parameters
                        .iter()
                        .filter(|parameter| {
                            crate::values::occupies_scalar_position(program, parameter)
                        })
                        .count()
                {
                    let (place, reference) = result?;
                    (place.clone(), reference)
                } else {
                    return None;
                }
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
            Scalar::StructuralParameterField { .. }
            | Scalar::StructuralParameterIndexedRead { .. } => {
                let selected = operand.parameter_observation_projection()?;
                let parameter_position = selected.parameter_position;
                let path = selected.path;
                if usize::try_from(parameter_position).ok()? == parameters.len() {
                    let (place, reference) = result?;
                    let (segments, reference, _) =
                        crate::values::scalar::structural_fields::resolve_structural_path(
                            program,
                            reference,
                            false,
                            &path,
                            &mut |_, _| Some(()),
                        )?;
                    let reference =
                        validation::field_domains::readable_type_reference(program, reference)?;
                    let mut place = place.clone();
                    place.segments.extend(segments);
                    (place, reference)
                } else {
                    let (root, segments, reference, _) =
                        crate::values::resolve_structural_parameter_path(
                            program,
                            parameters,
                            parameter_position,
                            &path,
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

/// Lower current guard algebra without binding a storage or value epoch.
/// Flow supplies fresh completed observations when the guard actually selects
/// its branch; caching this syntax cannot retain a previous read's address.
pub(crate) fn lower_current_guard(
    program: &dyn ::validation::CheckingInput,
    operators: &checked_trees::CheckedOperatorFacts,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<Boolean> {
    let (machine, state) = validation::find_state(program, state_symbol)?;
    if !validation::has_builtin_bound_expression_meaning(program, machine, Some(state), expression)
    {
        return None;
    }
    let expression = crate::values::lower_unit_scalar_argument(
        program,
        operators,
        state,
        statement_index,
        expression,
        PrimitiveType::Bool,
    )
    .and_then(|value| match value {
        Scalar::Boolean(value) => Some(value),
        _ => None,
    })
    .or_else(|| {
        crate::values::lower_current_state_predicate(
            program,
            operators,
            machine,
            state,
            statement_index,
            expression,
        )
        .map(Box::new)
    })?;
    Some(*expression)
}

/// The selected builtin equality used by a value pattern has the same checked
/// observation bindings as an authored equality. It is not a comparison added
/// to the immutable source tree and grants no meaning to selected operators.
pub(crate) fn bind_current_match(
    program: &dyn ::validation::CheckingInput,
    operators: &checked_trees::CheckedOperatorFacts,
    state_symbol: SymbolHandle,
    statement_index: usize,
    subject: typed_trees::expression::ExpressionHandle,
    pattern: typed_trees::expression::ExpressionHandle,
    matched: bool,
) -> Option<BoundPredicate> {
    let (machine, state) = validation::find_state(program, state_symbol)?;
    let reference = validation::expression_result_type_reference(program, machine, state, subject)?;
    let primitive = program.primitive_type_reference(reference)?;
    let left = crate::values::lower_unit_scalar_argument(
        program,
        operators,
        state,
        statement_index,
        subject,
        primitive,
    )?;
    let right = crate::values::lower_unit_scalar_argument(
        program,
        operators,
        state,
        statement_index,
        pattern,
        primitive,
    )?;
    let expression = if primitive == PrimitiveType::Bool {
        let (Scalar::Boolean(left), Scalar::Boolean(right)) = (left, right) else {
            return None;
        };
        Boolean::Equal { left, right }
    } else {
        // Floating equality and noninteger carriers retain their own selected
        // semantics; they cannot acquire reflexive integer comparison meaning.
        validation::carrier_integer_range(primitive)?;
        Boolean::IntegerComparison {
            kind: checked_trees::CheckedIntegerComparisonKind::Equal,
            left: Box::new(left),
            right: Box::new(right),
        }
    };
    bind_current_expression(program, state_symbol, expression, matched, |operand| {
        declared_observation(program, state_symbol, statement_index, operand)
    })
}

pub(crate) fn bind_current_expression(
    program: &dyn ::validation::CheckingInput,
    state_symbol: SymbolHandle,
    mut expression: Boolean,
    value: bool,
    mut observe: impl FnMut(&Scalar) -> Option<(CanonicalPlace, TypeReferenceHandle)>,
) -> Option<BoundPredicate> {
    let (machine, state) = validation::find_state(program, state_symbol)?;
    let mut bindings = Vec::new();
    map_observations(&mut expression, &mut |operand| {
        if bindings
            .iter()
            .any(|binding: &BoundObservation| binding.operand == *operand)
        {
            return Some(());
        }
        let (place, type_reference) = observe(operand)?;
        // The completed observation keeps its exact place and live storage cut.
        // An empty structural path may read a primitive loan's referent, as
        // parameter-contract binding does; write-only loans remain absent.
        let type_reference =
            validation::field_domains::readable_type_reference(program, type_reference)?;
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
            expression
        } else {
            Boolean::Not(Box::new(expression))
        },
        bindings,
    })
}

/// Declaration-bound observations used when no completed evaluation is being
/// consumed. Runtime indexed reads need their actual Flow observation instead;
/// this conversion never invents a static path or re-evaluates a selector.
pub(crate) fn declared_observation(
    program: &dyn ::validation::CheckingInput,
    state_symbol: SymbolHandle,
    statement_index: usize,
    operand: &Scalar,
) -> Option<(CanonicalPlace, TypeReferenceHandle)> {
    let (_, state) = validation::find_state(program, state_symbol)?;
    let symbols = crate::values::unit_scalar_binding_symbols(program, state, statement_index)?;
    let place = match operand {
        Scalar::Parameter { position, .. } | Scalar::Local { position, .. } => {
            crate::flow::canonical_place_from_symbol(*symbols.get(*position)?)?
        }
        Scalar::StorageRead { symbol, .. } => crate::flow::canonical_place_from_symbol(*symbol)?,
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
    let type_reference =
        validation::canonical_place_type_reference(program, state_symbol, statement_index, &place)?;
    Some((place, type_reference))
}

pub(crate) fn seed(
    semantic: &mut FactPlan,
    predicate: &BoundPredicate,
    point: ProgramPoint,
    origin: FactOrigin,
) -> FactContextHandle {
    seed_with_dependencies(semantic, predicate, point, origin, &[])
}

/// Additional evaluated receivers share the proposition's ordinary storage
/// lifetime; they assert no independent truth about those receiver contents.
pub(crate) fn seed_with_dependencies(
    semantic: &mut FactPlan,
    predicate: &BoundPredicate,
    point: ProgramPoint,
    origin: FactOrigin,
    dependencies: &[CanonicalPlace],
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
    for dependency in dependencies {
        let place =
            crate::semantic::places::append_place(semantic, dependency.root, &dependency.segments);
        if !places.contains(&place) {
            places.push(place);
        }
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

/// Bind a domain's predicates to an independently selected live subject.
/// This supplies goals, not membership premises: admission may use the same
/// binding to veto a refuted claim without assuming the claim under judgment.
pub(crate) fn domain_predicates_for_subject(
    program: &dyn ::validation::CheckingInput,
    operators: &checked_trees::CheckedOperatorFacts,
    domain: &typed_trees::domain::DomainDefinition,
    subject: &CanonicalPlace,
) -> Option<Vec<BoundPredicate>> {
    fn visit(
        program: &dyn ::validation::CheckingInput,
        operators: &checked_trees::CheckedOperatorFacts,
        domain: &typed_trees::domain::DomainDefinition,
        subject: &CanonicalPlace,
        active: &mut Vec<(SymbolHandle, language_semantics::SemanticDomainId)>,
    ) -> Option<Vec<BoundPredicate>> {
        if !domain.type_parameters.is_empty()
            || !domain.index_arguments.is_empty()
            || active.contains(&(domain.symbol, domain.semantic_id))
        {
            return None;
        }
        active.push((domain.symbol, domain.semantic_id));
        let parameters = [typed_trees::signature::StateParameter {
            symbol: domain.subject_symbol,
            name: domain.subject_name.clone(),
            type_reference: domain.target_type,
            ..Default::default()
        }];
        let mut predicates: Vec<BoundPredicate> = program
            .proof_facts(domain)
            .iter()
            .filter_map(|fact| {
                let expression = match fact {
                    typed_trees::domain::ProofFact::Expression(expression) => {
                        crate::values::lower_callable_scalar_contract_predicate(
                            program,
                            operators,
                            domain.symbol,
                            &parameters,
                            TypeReferenceHandle::invalid(),
                            *expression,
                            false,
                            &mut 4096,
                        )?
                    }
                    typed_trees::domain::ProofFact::Membership(membership) => {
                        if typed_trees::domain::domain_subject_reference(
                            program,
                            domain,
                            membership.value,
                        ) {
                            return None;
                        }
                        crate::values::scalar::result_contract::lower_callable_scalar_membership(
                            program,
                            operators,
                            domain.symbol,
                            &parameters,
                            membership,
                            &mut 4096,
                        )?
                    }
                    typed_trees::domain::ProofFact::Proposition(_) => return None,
                };
                let mut predicate = bind_parameters(
                    program,
                    domain.symbol,
                    domain.symbol,
                    &parameters,
                    None,
                    expression,
                )?;
                for binding in &mut predicate.bindings {
                    if binding.place.root != facts::PlaceRoot::Symbol(domain.subject_symbol) {
                        return None;
                    }
                    let mut place = subject.clone();
                    place.extend_segments(&binding.place.segments);
                    binding.place = place;
                }
                Some(predicate)
            })
            .collect();
        // Whole-subject membership retains the parent's complete supported
        // predicate theory at this exact subject. Projecting a record membership
        // through the scalar-only lowerer used to drop every inherited clause.
        // Field memberships keep their separate subjects and ordinary projection.
        for membership in typed_trees::domain::subject_membership_facts(program, domain) {
            if let Some(parent) = program
                .domain_definition_for_instance(
                    membership.domain_symbol,
                    membership.semantic_domain,
                )
                .or_else(|| program.domain_definition(membership.domain_symbol))
                && let Some(inherited) = visit(program, operators, parent, subject, active)
            {
                predicates.extend(inherited);
            }
        }
        if let Some(alias) = &domain.alias {
            for constituent in &alias.constituents {
                if let Some(parent) = program.domain_definition(constituent.domain_symbol)
                    && let Some(inherited) = visit(program, operators, parent, subject, active)
                {
                    predicates.extend(inherited);
                }
            }
        }
        active.pop();
        Some(predicates)
    }
    visit(program, operators, domain, subject, &mut Vec::new())
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
        observation: typed_trees::expression::ExpressionHandle,
        parameter_position: u32,
        path: &[checked_trees::CheckedStructuralPredicatePathSegment],
    ) -> Option<ScalarValue> {
        let binding = self.predicate.bindings.iter().find(|binding| matches!(&binding.operand, Scalar::StructuralParameterField { observation: candidate_observation, parameter_position: candidate, path: candidate_path, .. } if *candidate_observation == observation && *candidate == parameter_position && candidate_path == path))?;
        (self.resolve)(&binding.place)
    }
}

/// Finite truth propagation over the selected Boolean algebra. Integer
/// comparisons remain whole observations: knowing one never supplies a value
/// for either integer operand. Every round uses only already established facts.
/// The requested place must occur as a Boolean observation: inferring all
/// unrelated predicates cannot produce a Boolean value for an integer subject.
pub(crate) fn inferred_boolean_at_place(
    predicates: &[BoundPredicate],
    actual: &CanonicalPlace,
    ordinary: impl FnMut(&CanonicalPlace) -> Option<ScalarValue>,
) -> Option<bool> {
    // Normalization obtains every observation from one exact bound operand.
    // This necessary-condition query declines only; it does not prove a goal,
    // filter the premise set, or ignore a contradiction for a represented Bool.
    if !predicates
        .iter()
        .flat_map(|row| &row.bindings)
        .any(|binding| {
            binding.place == *actual
                && binding.operand.primitive_type() == Some(PrimitiveType::Bool)
        })
    {
        return None;
    }
    let (expressions, observations) = normalize(predicates)?;
    let position = observations
        .iter()
        .position(|(place, primitive)| *place == *actual && *primitive == PrimitiveType::Bool)?;
    let truths = formula_closure(&expressions, &[], &observations, ordinary)?;
    truths.into_iter().find_map(|(expression, value)| {
        (expression == Boolean::Parameter { position }).then_some(value)
    })
}

type Observation = (CanonicalPlace, PrimitiveType);

fn formula_closure(
    assertions: &[Boolean],
    queries: &[Boolean],
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
    // A required formula is a query, not an established premise. Evaluate its
    // normalized observations through the same exact current-place values as
    // retained assertions, including indexed reads. The source-shaped scalar
    // evaluator cannot recover those bindings after normalization.
    for query in queries {
        collect(query, &mut expressions);
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
        // Only Boolean leaves can be replaced here. Integer observations stay
        // symbolic, so resolving their live values (including relational truth
        // closure) cannot affect this fold. Still require every leaf's binding.
        if operand.primitive_type() == Some(PrimitiveType::Bool)
            && let Some(ScalarValue::Boolean(value)) = resolve(&binding.place)
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
            observation,
            parameter_position,
            path,
        } => {
            let mut scalar = Scalar::StructuralParameterField {
                observation: *observation,
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
        | Boolean::StructuralIdentity { .. }
        | Boolean::ByteSequenceEqual { .. }
        | Boolean::ByteSequencePredicate { .. }
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
            observation,
            parameter_position,
            path,
            primitive_type: PrimitiveType::Bool,
        } => Some(Boolean::StructuralParameterField {
            observation,
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
        | Scalar::StructuralParameterIndexedRead { .. }
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
        Scalar::StructuralSequenceLength { .. } | Scalar::ErasedParameter { .. } => return None,
    }
    Some(())
}

/// A conditional edge owes !guard || requirement. Normalize both formulas
/// into one exact observation namespace before constructing that goal; equal
/// operand ordinals from different frames are not equal storage observations.
/// This creates no premise and changes no contradiction/formation judgment.
pub(crate) fn guarded_requirement(
    guard: &BoundPredicate,
    required: &BoundPredicate,
) -> Option<BoundPredicate> {
    let (expressions, observations) = normalize(&[guard.clone(), required.clone()])?;
    let mut expressions = expressions.into_iter();
    let guard_expression = expressions.next()?;
    let required_expression = expressions.next()?;
    let bindings = observations
        .into_iter()
        .enumerate()
        .map(|(position, (place, primitive_type))| {
            let source = guard
                .bindings
                .iter()
                .chain(&required.bindings)
                .find(|binding| {
                    binding.place == place
                        && binding.operand.primitive_type() == Some(primitive_type)
                })?;
            Some(BoundObservation {
                operand: Scalar::Parameter {
                    position,
                    primitive_type,
                },
                place,
                type_reference: source.type_reference,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    Some(BoundPredicate {
        machine: required.machine,
        entry_state: required.entry_state,
        expression: Boolean::Or {
            left: Box::new(Boolean::Not(Box::new(guard_expression))),
            right: Box::new(required_expression),
        },
        bindings,
    })
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
    let truths = formula_closure(
        &expressions,
        std::slice::from_ref(&normalized_required),
        &observations,
        &mut ordinary,
    )?;
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
    known(&normalized_required, &truths)
}
