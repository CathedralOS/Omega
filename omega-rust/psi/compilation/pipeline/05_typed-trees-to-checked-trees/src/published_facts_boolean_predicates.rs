/*
    True relational formulas over exact current storage. Formal coordinates
    identify observations; explicit bindings supply their current namespace.
    Flow carries scratch rows across sweeps and allocates fresh handles on seed.
*/

use validation::CanonicalPlace;

mod availability {
    use facts::{FactContextHandle, FactPlan, ProgramPoint};
    use std::cell::OnceCell;
    use validation::CanonicalPlace;

    /*
        A payload membership is a conditional invariant, not an unconditional
        Boolean premise. Expansion requires selected tags in the consumer scope;
        a fact's publication point may still name another contract's telescope.
        A batch is valid only for these immutable consumer contexts and scope.
        Templates may be shared across subjects; selected-case verdicts may not.
    */
    pub(crate) struct CasePremiseAvailability<'program> {
        program: &'program dyn validation::CheckingInput,
        semantic: &'program FactPlan,
        contexts: &'program [FactContextHandle],
        point: ProgramPoint,
        evidence: OnceCell<Option<crate::flow::CaseEvidence<'program>>>,
    }

    impl<'program> CasePremiseAvailability<'program> {
        pub(crate) fn new(
            program: &'program dyn validation::CheckingInput,
            semantic: &'program FactPlan,
            contexts: &'program [FactContextHandle],
            point: ProgramPoint,
        ) -> Self {
            Self {
                program,
                semantic,
                contexts,
                point,
                evidence: OnceCell::new(),
            }
        }

        pub(crate) fn allows(&self, subject: &CanonicalPlace) -> bool {
            !subject
                .segments
                .iter()
                .any(|segment| matches!(segment, facts::PlaceSegment::Case { .. }))
                || self
                    .evidence
                    .get_or_init(|| {
                        consumer_scope(self.program, self.point).and_then(
                            |(machine, state, statement)| {
                                crate::flow::CaseEvidence::prepare(
                                    self.program,
                                    self.semantic,
                                    self.contexts,
                                    machine,
                                    state,
                                    statement,
                                )
                            },
                        )
                    })
                    .as_ref()
                    .is_some_and(|evidence| evidence.cases_are_selected(subject))
        }
    }

    fn consumer_scope(
        program: &dyn validation::CheckingInput,
        point: ProgramPoint,
    ) -> Option<(symbols::SymbolHandle, symbols::SymbolHandle, usize)> {
        Some(match point {
            ProgramPoint::State {
                machine_symbol,
                state_symbol,
            } => (machine_symbol, state_symbol, 0),
            ProgramPoint::Statement {
                machine_symbol,
                state_symbol,
                statement_index,
            }
            | ProgramPoint::Call {
                machine_symbol,
                state_symbol,
                statement_index,
                ..
            }
            | ProgramPoint::CallRequires {
                machine_symbol,
                state_symbol,
                statement_index,
                ..
            }
            | ProgramPoint::CallEnsures {
                machine_symbol,
                state_symbol,
                statement_index,
                ..
            }
            | ProgramPoint::TransitionArm {
                machine_symbol,
                state_symbol,
                statement_index,
                ..
            }
            | ProgramPoint::Exit {
                machine_symbol,
                state_symbol,
                statement_index,
                ..
            } => (machine_symbol, state_symbol, statement_index),
            ProgramPoint::Machine { machine_symbol } => {
                let machine = program.machine_by_symbol(machine_symbol)?;
                let state = program.machine_states(machine).first()?;
                (machine_symbol, state.symbol, 0)
            }
            ProgramPoint::Global | ProgramPoint::Definition { .. } => return None,
        })
    }
}
mod bindings {
    /*
        Bind each genuine observation in its declared value telescope.
        Scalar ordinals and structural subject ordinals are different namespaces;
        Result is an explicit value binding, never a guessed parameter position.
    */
    use super::{
        BoundObservation, CanonicalPlace, Operand, Scalar, SymbolHandle, TypeReferenceHandle,
    };
    use crate::flow::canonical_place_from_symbol;
    use crate::values::occupies_scalar_position;
    use crate::values::resolve_structural_parameter_path;
    use crate::values::scalar::structural_fields::resolve_structural_path;
    use checked_trees::{CheckedStructuralCaseRoot, CheckedStructuralCaseSubject};

    pub(super) fn parameter(
        program: &dyn validation::CheckingInput,
        parameters: &[typed_trees::signature::StateParameter],
        result: Option<(&CanonicalPlace, TypeReferenceHandle)>,
        operand: &Operand,
    ) -> Option<(CanonicalPlace, TypeReferenceHandle)> {
        match operand {
            Operand::Scalar(operand) => scalar_parameter(program, parameters, result, operand),
            Operand::CaseSubject(subject) => case_parameter(program, parameters, result, subject),
        }
    }

    fn scalar_parameter(
        program: &dyn validation::CheckingInput,
        parameters: &[typed_trees::signature::StateParameter],
        result: Option<(&CanonicalPlace, TypeReferenceHandle)>,
        operand: &Scalar,
    ) -> Option<(CanonicalPlace, TypeReferenceHandle)> {
        let resolved = match operand {
            Scalar::Parameter { position, .. } => {
                let mut scalars = parameters
                    .iter()
                    .filter(|parameter| occupies_scalar_position(program, parameter));
                if let Some(parameter) = scalars.nth(*position) {
                    (
                        canonical_place_from_symbol(parameter.symbol)?,
                        parameter.type_reference,
                    )
                } else if *position
                    == parameters
                        .iter()
                        .filter(|parameter| occupies_scalar_position(program, parameter))
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
                    canonical_place_from_symbol(*symbol)?,
                    validation::field_domains::readable_type_reference(
                        program,
                        parameter.type_reference,
                    )?,
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
                        resolve_structural_path(program, reference, false, &path, &mut |_, _| {
                            Some(())
                        })?;
                    let reference =
                        validation::field_domains::readable_type_reference(program, reference)?;
                    let mut place = place.clone();
                    place.segments.extend(segments);
                    (place, reference)
                } else {
                    let (root, segments, reference, _) = resolve_structural_parameter_path(
                        program,
                        parameters,
                        parameter_position,
                        &path,
                    )?;
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
        Some(resolved)
    }

    fn case_parameter(
        program: &dyn validation::CheckingInput,
        parameters: &[typed_trees::signature::StateParameter],
        result: Option<(&CanonicalPlace, TypeReferenceHandle)>,
        subject: &CheckedStructuralCaseSubject,
    ) -> Option<(CanonicalPlace, TypeReferenceHandle)> {
        let (place, reference) = match subject.root {
            CheckedStructuralCaseRoot::Parameter { position } => {
                let (root, segments, reference, _) = resolve_structural_parameter_path(
                    program,
                    parameters,
                    position,
                    &subject.path,
                )?;
                (
                    CanonicalPlace {
                        root: facts::PlaceRoot::Symbol(root),
                        segments,
                    },
                    reference,
                )
            }
            CheckedStructuralCaseRoot::Result => {
                let (place, reference) = result?;
                let (segments, selected, _) = resolve_structural_path(
                    program,
                    reference,
                    false,
                    &subject.path,
                    &mut |_, _| Some(()),
                )?;
                let mut place = place.clone();
                place.segments.extend(segments);
                (place, selected)
            }
        };
        let reference = validation::field_domains::readable_type_reference(program, reference)?;
        compatible_type(program, &Operand::CaseSubject(subject.clone()), reference)
            .then_some((place, reference))
    }

    /*
        This is type authentication, not formation, tag selection or access proof.
        Those obligations remain at the actual checked observation and load.
    */
    pub(super) fn compatible_type(
        program: &dyn validation::CheckingInput,
        operand: &Operand,
        reference: TypeReferenceHandle,
    ) -> bool {
        match operand {
            Operand::Scalar(scalar) => scalar.primitive_type().is_some_and(|primitive| {
                program.primitive_type_reference(reference) == Some(primitive)
            }),
            Operand::CaseSubject(_) => {
                program
                    .closed_data_definition(reference)
                    .is_some_and(|data| {
                        program.data_members(data).iter().any(|member| {
                            matches!(member, typed_trees::data::DataMember::Variant(_))
                        })
                    })
            }
        }
    }

    pub(super) fn entry(
        program: &dyn validation::CheckingInput,
        machine: SymbolHandle,
        position: u32,
        path: Vec<checked_trees::CheckedStructuralPredicatePathSegment>,
        operand: &Operand,
    ) -> Option<Operand> {
        match operand {
            Operand::Scalar(scalar) => {
                super::entry_scalar(program, machine, position, path, scalar.primitive_type()?)
                    .map(Operand::Scalar)
            }
            Operand::CaseSubject(_) => {
                let owner = program.machine_by_symbol(machine)?;
                let entry = validation::activation_target_state_index(program, owner, machine)?;
                let parameters =
                    program.state_parameters(program.machine_states(owner).get(entry)?);
                let subject = CheckedStructuralCaseSubject {
                    root: CheckedStructuralCaseRoot::Parameter { position },
                    path,
                };
                case_parameter(program, parameters, None, &subject)?;
                Some(Operand::CaseSubject(subject))
            }
        }
    }

    impl BoundObservation {
        /*
            A matching absent primitive is not type correspondence. Structural
            subjects retain the complete nominal application identity, independent
            of the selected case and without borrowing a scalar representation.
        */
        pub(crate) fn compatible_type(
            &self,
            program: &dyn validation::CheckingInput,
            reference: TypeReferenceHandle,
        ) -> bool {
            let other = Self {
                operand: self.operand.clone(),
                place: self.place.clone(),
                type_reference: reference,
            };
            self.compatible_with(program, &other)
        }

        pub(crate) fn compatible_with(
            &self,
            program: &dyn validation::CheckingInput,
            other: &Self,
        ) -> bool {
            match (&self.operand, &other.operand) {
                (Operand::Scalar(left), Operand::Scalar(right)) => left
                    .primitive_type()
                    .is_some_and(|primitive| right.primitive_type() == Some(primitive)),
                (Operand::CaseSubject(_), Operand::CaseSubject(_)) => {
                    if !compatible_type(program, &self.operand, self.type_reference)
                        || !compatible_type(program, &other.operand, other.type_reference)
                    {
                        return false;
                    }
                    let left = program.type_identity(
                        typed_trees::type_identity::TypeIdentityRequest::ordinary(
                            self.type_reference,
                        ),
                    );
                    let right = program.type_identity(
                        typed_trees::type_identity::TypeIdentityRequest::ordinary(
                            other.type_reference,
                        ),
                    );
                    left != Default::default() && left == right
                }
                _ => false,
            }
        }
    }
}
mod case_subjects {
    /*
        Case atoms consume exact tag evidence without reading inactive payloads.
        Folding happens in the source predicate's authenticated scope, before its
        remaining observations are transported to another value namespace.
    */
    use super::{Boolean, BoundObservation, BoundPredicate, Operand};
    use facts::checked_boolean_triple_pattern;
    use typed_trees::data::DataMemberIdentity;

    pub(crate) fn fold_cases(
        predicate: &mut BoundPredicate,
        mut resolve: impl FnMut(&BoundObservation, &DataMemberIdentity) -> Option<bool>,
    ) {
        fn visit(
            expression: &mut Boolean,
            bindings: &[BoundObservation],
            resolve: &mut impl FnMut(&BoundObservation, &DataMemberIdentity) -> Option<bool>,
        ) {
            match expression {
                Boolean::StructuralCaseMembership { subject, case } => {
                    let operand = Operand::CaseSubject(subject.clone());
                    if let Some(binding) =
                        bindings.iter().find(|binding| binding.operand == operand)
                        && let Some(value) = resolve(binding, case)
                    {
                        *expression = Boolean::Constant(value);
                    }
                }
                Boolean::Not(operand) => visit(operand, bindings, resolve),
                checked_boolean_triple_pattern!(left, right, ..) => {
                    visit(left, bindings, resolve);
                    visit(right, bindings, resolve);
                }
                _ => {}
            }
        }
        visit(&mut predicate.expression, &predicate.bindings, &mut resolve);
        super::simplify(&mut predicate.expression);
    }
}
pub(crate) use availability::CasePremiseAvailability;
pub(crate) use case_subjects::fold_cases;
mod integer_intervals {
    /*
        Numeric consequences of a surviving proposition, over its exact bindings.
        Conjunction intersects; alternatives hull. Unknown clauses keep the carrier,
        and contradictory clauses never supply an arbitrary interval. Opposite
        operands use ordinary completed values, never this proposition recursively.
    */
    use super::BoundPredicate;
    use checked_trees::{
        CheckedBooleanExpression as Boolean, CheckedIntegerComparisonKind as Comparison,
    };
    use facts::{IntegerRange, ScalarValue};
    use numerics::bignum::BigInt;
    use typed_trees::types::PrimitiveType;
    use validation::CanonicalPlace;

    impl BoundPredicate {
        pub(crate) fn integer_interval_at_place(
            &self,
            place: &CanonicalPlace,
            primitive: PrimitiveType,
            mut ordinary: impl FnMut(&CanonicalPlace) -> Option<ScalarValue>,
        ) -> Option<IntegerRange> {
            let carrier = validation::carrier_integer_range(primitive)?;
            if self.bindings.iter().any(|binding| {
                binding.place == *place
                    && binding
                        .operand
                        .scalar()
                        .and_then(|operand| operand.primitive_type())
                        != Some(primitive)
            }) {
                return Some(carrier);
            }
            consequence(
                self,
                &self.expression,
                false,
                place,
                &carrier,
                &mut ordinary,
            )
        }
    }

    fn consequence(
        predicate: &BoundPredicate,
        expression: &Boolean,
        negated: bool,
        place: &CanonicalPlace,
        carrier: &IntegerRange,
        ordinary: &mut impl FnMut(&CanonicalPlace) -> Option<ScalarValue>,
    ) -> Option<IntegerRange> {
        let mut bounds = match expression {
            Boolean::Not(operand) => {
                return consequence(predicate, operand, !negated, place, carrier, ordinary);
            }
            Boolean::And { left, right } | Boolean::Or { left, right } => {
                let left = consequence(predicate, left, negated, place, carrier, ordinary);
                let right = consequence(predicate, right, negated, place, carrier, ordinary);
                let conjunction = matches!(expression, Boolean::And { .. }) != negated;
                if conjunction {
                    let (left, right) = (left?, right?);
                    IntegerRange {
                        minimum: left.minimum.max(right.minimum),
                        maximum: left.maximum.min(right.maximum),
                    }
                } else {
                    match (left, right) {
                        (Some(left), Some(right)) => IntegerRange {
                            minimum: left.minimum.min(right.minimum),
                            maximum: left.maximum.max(right.maximum),
                        },
                        (left, right) => return left.or(right),
                    }
                }
            }
            Boolean::Constant(value) if *value == negated => return None,
            Boolean::IntegerComparison { kind, left, right } => {
                let selected = [(left, right, false), (right, left, true)]
                    .into_iter()
                    .find_map(|(subject, value, reversed)| {
                        let binding = predicate.scalar_binding(subject)?;
                        if binding.place != *place {
                            return None;
                        }
                        let value = crate::values::evaluate_checked_scalar(
                            value,
                            &mut super::Values {
                                predicate,
                                resolve: &mut *ordinary,
                            },
                        )?;
                        let ScalarValue::Integer(_) = value else {
                            return None;
                        };
                        Some((value, reversed))
                    });
                let Some((value, reversed)) = selected else {
                    return Some(carrier.clone());
                };
                let ScalarValue::Integer(value) = value else {
                    return None;
                };
                let one = BigInt::from_u64(1);
                let mut bounds = carrier.clone();
                match (kind, reversed, negated) {
                    (Comparison::Equal, _, false) => {
                        bounds.minimum = value.clone();
                        bounds.maximum = value;
                    }
                    (Comparison::Equal, _, true) => {}
                    (Comparison::LessThan, false, false)
                    | (Comparison::LessOrEqual, true, true) => bounds.maximum = value.sub(&one),
                    (Comparison::LessOrEqual, false, false)
                    | (Comparison::LessThan, true, true) => bounds.maximum = value,
                    (Comparison::LessThan, true, false)
                    | (Comparison::LessOrEqual, false, true) => bounds.minimum = value.add(&one),
                    (Comparison::LessOrEqual, true, false)
                    | (Comparison::LessThan, false, true) => bounds.minimum = value,
                }
                bounds
            }
            _ => carrier.clone(),
        };
        bounds.minimum = bounds.minimum.max(carrier.minimum.clone());
        bounds.maximum = bounds.maximum.min(carrier.maximum.clone());
        (bounds.minimum <= bounds.maximum).then_some(bounds)
    }
}
mod normalization {
    /*
        Formula identity joins exact observations, not lowering proof coverage.
        A guard's decided subtraction and a requirement's undecided subtraction
        denote the same operation on the same values. Normalize cloned scratch
        formulas only, clearing decisions rather than granting them: published
        operations, formation obligations, policies and captured epochs stay intact.
    */
    use super::{Boolean, BoundPredicate, Observation, ObservationKind, Scalar, map_observations};

    pub(super) fn normalize(
        predicates: &[BoundPredicate],
    ) -> Option<(Vec<Boolean>, Vec<Observation>)> {
        let mut observations = Vec::new();
        let expressions = predicates
            .iter()
            .map(|predicate| {
                let mut expression = predicate.expression.clone();
                map_observations(&mut expression, &mut |operand| {
                    let binding = predicate.binding(operand)?;
                    let kind = ObservationKind::of(binding)?;
                    let key = (binding.place.clone(), kind);
                    let position = observations
                        .iter()
                        .position(|known| known == &key)
                        .unwrap_or_else(|| {
                            let position = observations.len();
                            observations.push(key);
                            position
                        });
                    *operand = kind.operand(position)?;
                    Some(())
                })?;
                clear_boolean_formation_decisions(&mut expression);
                Some(expression)
            })
            .collect::<Option<Vec<_>>>()?;
        Some((expressions, observations))
    }

    fn clear_boolean_formation_decisions(expression: &mut Boolean) {
        match expression {
            Boolean::Not(operand) => clear_boolean_formation_decisions(operand),
            facts::checked_boolean_triple_pattern!(left, right, ..) => {
                clear_boolean_formation_decisions(left);
                clear_boolean_formation_decisions(right);
            }
            Boolean::IntegerComparison { left, right, .. }
            | Boolean::ScalarIeeeFloatComparison { left, right, .. } => {
                clear_scalar_formation_decisions(left);
                clear_scalar_formation_decisions(right);
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
                            clear_scalar_formation_decisions(operand);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn clear_scalar_formation_decisions(expression: &mut Scalar) {
        match expression {
            Scalar::IntegerBinary {
                left,
                right,
                formation_decided,
                trap_free,
                ..
            } => {
                *formation_decided = false;
                *trap_free = false;
                clear_scalar_formation_decisions(left);
                clear_scalar_formation_decisions(right);
            }
            Scalar::IntegerExactCast {
                operand,
                formation_decided,
                ..
            } => {
                *formation_decided = false;
                clear_scalar_formation_decisions(operand);
            }
            Scalar::IntegerBitwiseNot { operand, .. }
            | Scalar::IntegerWiden { operand, .. }
            | Scalar::RepresentationView { operand, .. } => {
                clear_scalar_formation_decisions(operand)
            }
            Scalar::Boolean(expression) => clear_boolean_formation_decisions(expression),
            _ => {}
        }
    }
}
mod paths {
    /*
        Finite path conditions over original invocation values. Each clause contains
        signed existing checked guard atoms; arrival joins union clauses and remove
        subsumed clauses. Cycles revisit a finite set of atoms rather than unroll ASTs.
    */
    use super::*;
    use facts::BooleanPredicateRole;

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub(crate) struct EntryPaths {
        known: bool,
        clauses: Vec<Vec<Boolean>>,
    }

    impl EntryPaths {
        pub(crate) fn unconditional() -> Self {
            Self {
                known: true,
                clauses: vec![Vec::new()],
            }
        }
        pub(crate) fn unknown() -> Self {
            Self {
                known: false,
                clauses: Vec::new(),
            }
        }
        fn impossible() -> Self {
            Self {
                known: true,
                clauses: Vec::new(),
            }
        }

        fn insert(&mut self, clause: Vec<Boolean>) -> bool {
            if self
                .clauses
                .iter()
                .any(|old| old.iter().all(|atom| clause.contains(atom)))
            {
                return false;
            }
            self.clauses
                .retain(|old| !clause.iter().all(|atom| old.contains(atom)));
            self.clauses.push(clause);
            true
        }

        pub(crate) fn union(&mut self, incoming: &Self) -> bool {
            if !self.known {
                return false;
            }
            if !incoming.known {
                *self = Self::unknown();
                return true;
            }
            let mut changed = false;
            for clause in &incoming.clauses {
                changed |= self.insert(clause.clone());
            }
            changed
        }

        fn intersect(&self, other: &Self) -> Self {
            if !self.known || !other.known {
                return Self::unknown();
            }
            let mut result = Self::impossible();
            for left in &self.clauses {
                for right in &other.clauses {
                    let mut clause = left.clone();
                    let mut consistent = true;
                    for atom in right {
                        if clause.iter().any(|existing| opposite(existing, atom)) {
                            consistent = false;
                            break;
                        }
                        if !clause.contains(atom) {
                            clause.push(atom.clone());
                        }
                    }
                    if consistent {
                        result.insert(clause);
                    }
                }
            }
            result
        }

        fn from_expression(expression: &Boolean, negated: bool) -> Self {
            match expression {
                Boolean::Constant(value) => {
                    if *value != negated {
                        Self::unconditional()
                    } else {
                        Self::impossible()
                    }
                }
                Boolean::Not(operand) => Self::from_expression(operand, !negated),
                Boolean::And { left, right } | Boolean::Or { left, right } => {
                    let mut left = Self::from_expression(left, negated);
                    let right = Self::from_expression(right, negated);
                    let conjunction = matches!(expression, Boolean::And { .. }) != negated;
                    if conjunction {
                        left.intersect(&right)
                    } else {
                        left.union(&right);
                        left
                    }
                }
                atom => Self {
                    known: true,
                    clauses: vec![vec![if negated {
                        Boolean::Not(Box::new(atom.clone()))
                    } else {
                        atom.clone()
                    }]],
                },
            }
        }

        pub(crate) fn expression(&self) -> Option<Boolean> {
            if !self.known {
                return None;
            }
            Some(
                self.clauses
                    .iter()
                    .map(|clause| {
                        clause
                            .iter()
                            .cloned()
                            .reduce(|left, right| Boolean::And {
                                left: Box::new(left),
                                right: Box::new(right),
                            })
                            .unwrap_or(Boolean::Constant(true))
                    })
                    .reduce(|left, right| Boolean::Or {
                        left: Box::new(left),
                        right: Box::new(right),
                    })
                    .unwrap_or(Boolean::Constant(false)),
            )
        }

        pub(crate) fn retained(
            semantic: &FactPlan,
            contexts: &[FactContextHandle],
            machine: SymbolHandle,
        ) -> Self {
            Self::retained_with_knowledge(semantic, contexts, machine, PathKnowledge::Complete)
        }

        fn retained_with_knowledge(
            semantic: &FactPlan,
            contexts: &[FactContextHandle],
            machine: SymbolHandle,
            knowledge: PathKnowledge,
        ) -> Self {
            let mut result = Self::unconditional();
            for context in contexts {
                for fact in semantic
                    .context_view(semantic.contexts.get(*context))
                    .facts()
                {
                    let FactPayload::BooleanPredicate { predicate } = fact.payload else {
                        continue;
                    };
                    if !semantic.boolean_predicates.is_valid(predicate) {
                        return Self::unknown();
                    }
                    let predicate = semantic.boolean_predicates.get(predicate);
                    if predicate.machine != machine {
                        continue;
                    }
                    match predicate.role {
                        BooleanPredicateRole::CurrentObservation => {}
                        BooleanPredicateRole::UnknownEntryPathCondition => {
                            if matches!(knowledge, PathKnowledge::Complete) {
                                return Self::unknown();
                            }
                        }
                        BooleanPredicateRole::EntryPathCondition => {
                            result = result
                                .intersect(&Self::from_expression(&predicate.expression, false))
                        }
                    }
                }
            }
            result
        }
    }

    #[derive(Clone, Copy)]
    enum PathKnowledge {
        Complete,
        Necessary,
    }

    fn opposite(left: &Boolean, right: &Boolean) -> bool {
        matches!(left, Boolean::Not(operand) if operand.as_ref() == right)
            || matches!(right, Boolean::Not(operand) if operand.as_ref() == left)
    }

    pub(crate) fn entry_path_condition(
        semantic: &FactPlan,
        contexts: &[FactContextHandle],
        machine: SymbolHandle,
    ) -> Option<Boolean> {
        EntryPaths::retained(semantic, contexts, machine).expression()
    }

    /*
        A necessary condition on the live path: execution implies the returned
        formula. Crash possibilities need this overapproximation, not a complete
        reconstruction of every guard. An unknown conjunct adds no restriction;
        current-storage observations add none either. The complete-path query and
        its union across alternative arrivals retain their original semantics.
    */
    pub(crate) fn entry_path_necessary_condition(
        semantic: &FactPlan,
        contexts: &[FactContextHandle],
        machine: SymbolHandle,
    ) -> Option<Boolean> {
        EntryPaths::retained_with_knowledge(semantic, contexts, machine, PathKnowledge::Necessary)
            .expression()
    }

    pub(crate) fn seed_entry_paths(
        semantic: &mut FactPlan,
        machine: SymbolHandle,
        entry_state: SymbolHandle,
        point: ProgramPoint,
        paths: &EntryPaths,
    ) -> FactContextHandle {
        let expression = paths.expression();
        let predicate = semantic.boolean_predicates.append(facts::BooleanPredicate {
            role: if expression.is_some() {
                BooleanPredicateRole::EntryPathCondition
            } else {
                BooleanPredicateRole::UnknownEntryPathCondition
            },
            machine,
            entry_state,
            expression: expression.unwrap_or(Boolean::Constant(false)),
            bindings: HandleSpan::empty(),
        });
        let fact = semantic.append_fact(Fact {
            place: FactPlace::Unknown,
            point,
            origin: FactOrigin::TransitionGuard,
            evidence: Default::default(),
            payload: FactPayload::BooleanPredicate { predicate },
        });
        let mut refs = HandleSpan::empty();
        semantic.append_ref(&mut refs, fact);
        semantic.append_context(point, refs)
    }

    /*
        Capture when the guard runs: later writes do not revise this historical
        proposition, and it never supplies a present-storage observation.
    */
    pub(crate) fn capture_entry_guard(
        program: &dyn ::validation::CheckingInput,
        operators: &checked_trees::CheckedOperatorFacts,
        semantic: &mut FactPlan,
        state_symbol: SymbolHandle,
        expression: typed_trees::expression::ExpressionHandle,
        value: bool,
        point: ProgramPoint,
        subjects: &mut impl FnMut(
            typed_trees::expression::ExpressionHandle,
        ) -> Option<(
            u32,
            Vec<checked_trees::CheckedStructuralPredicatePathSegment>,
        )>,
    ) -> Option<FactContextHandle> {
        let (machine, state) = validation::find_state(program, state_symbol)?;
        let expression = crate::values::lower_entry_predicate(
            program, operators, machine, state, expression, subjects,
        )
        .map(|expression| {
            if value {
                expression
            } else {
                Boolean::Not(Box::new(expression))
            }
        });
        capture_entry_expression(program, semantic, state_symbol, expression, point)
    }

    /*
        Publish an already authenticated original-entry proposition. A failed
        projection stays explicitly unknown, never a false path. Callers that have
        no new observation may omit this capture and keep their earlier facts.
    */
    pub(crate) fn capture_entry_expression(
        program: &dyn ::validation::CheckingInput,
        semantic: &mut FactPlan,
        state_symbol: SymbolHandle,
        expression: Option<Boolean>,
        point: ProgramPoint,
    ) -> Option<FactContextHandle> {
        let (machine, _) = validation::find_state(program, state_symbol)?;
        let entry_index =
            validation::activation_target_state_index(program, machine, machine.symbol)?;
        let entry = program.machine_states(machine).get(entry_index)?;
        let paths = expression
            .map(|expression| EntryPaths::from_expression(&expression, false))
            .unwrap_or_else(EntryPaths::unknown);
        Some(seed_entry_paths(
            semantic,
            machine.symbol,
            entry.symbol,
            point,
            &paths,
        ))
    }

    /*
        Compose path prerequisites using the same finite clause normalization as
        arrival flow. Both arguments are known formulas, so no unknown is minted.
    */
    pub(crate) fn conjunction(left: Boolean, right: Boolean) -> Boolean {
        EntryPaths::from_expression(&left, false)
            .intersect(&EntryPaths::from_expression(&right, false))
            .expression()
            .expect("known checked path conjunction")
    }
}
mod retention {
    /*
        Retain current predicates from independently live facts, without repeating
        domain lowering or growing all-pairs structural equality scans.

        Templates live for this invocation only, under one immutable program and
        operator catalog. Membership identity is checked before lookup. Grouping
        uses necessary observation identities only; complete predicate equality
        still decides duplicates, and original first-appearance order is restored.
    */
    use super::{BoundObservation, BoundPredicate};
    use facts::{FactContextHandle, FactPayload, FactPlan, ProgramPoint};
    use language_semantics::SemanticDomainId;
    use std::cmp::Ordering;
    use symbols::SymbolHandle;
    use validation::CanonicalPlace;

    pub(crate) fn retained(
        program: &dyn validation::CheckingInput,
        operators: &checked_trees::CheckedOperatorFacts,
        semantic: &FactPlan,
        contexts: &[FactContextHandle],
        point: ProgramPoint,
    ) -> Vec<BoundPredicate> {
        retained_rows(program, operators, semantic, contexts, point, None)
    }

    /*
        The rows `retained` returns, less those domain-membership rows that bind
        no storage under `root`. Membership binds only its own subject and the
        places below it, so a consumer that reads a predicate's consequence at
        places under `root` sees no difference. A fixed array declares one
        membership fact per element and context; the rows of every element would
        otherwise be rebuilt for each query about an unrelated scalar.
    */
    pub(crate) fn retained_at_root(
        program: &dyn validation::CheckingInput,
        operators: &checked_trees::CheckedOperatorFacts,
        semantic: &FactPlan,
        contexts: &[FactContextHandle],
        point: ProgramPoint,
        root: facts::PlaceRoot,
    ) -> Vec<BoundPredicate> {
        retained_rows(program, operators, semantic, contexts, point, Some(root))
    }

    fn retained_rows(
        program: &dyn validation::CheckingInput,
        operators: &checked_trees::CheckedOperatorFacts,
        semantic: &FactPlan,
        contexts: &[FactContextHandle],
        point: ProgramPoint,
        only_root: Option<facts::PlaceRoot>,
    ) -> Vec<BoundPredicate> {
        let mut rows = Vec::new();
        let mut domains = Vec::new();
        let availability =
            super::availability::CasePremiseAvailability::new(program, semantic, contexts, point);
        for handle in semantic.unique_context_facts(contexts.iter().copied()) {
            let fact = semantic.facts.get(handle);
            if let Some(root) = only_root
                && fact.payload.domain_membership().is_some()
                && let Some(place) = fact.place.as_place()
                && semantic.places.get(place).root != root
            {
                continue;
            }
            if let Some(predicates) = domain_predicates(
                program,
                operators,
                semantic,
                &availability,
                fact,
                &mut domains,
            ) {
                rows.extend(predicates);
                continue;
            }
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
            rows.push(BoundPredicate {
                machine: predicate.machine,
                entry_state: predicate.entry_state,
                expression: predicate.expression.clone(),
                bindings,
            });
        }
        unique_predicates(rows)
    }

    struct DomainTemplate {
        symbol: SymbolHandle,
        identity: SemanticDomainId,
        predicates: Option<Vec<BoundPredicate>>,
    }

    /*
        Live membership supplies predicates only on its own exact subject. The
        public goal/veto binder remains independent of this membership consumer.
    */
    fn domain_predicates(
        program: &dyn validation::CheckingInput,
        operators: &checked_trees::CheckedOperatorFacts,
        semantic: &FactPlan,
        availability: &super::availability::CasePremiseAvailability<'_>,
        fact: &facts::Fact,
        domains: &mut Vec<DomainTemplate>,
    ) -> Option<Vec<BoundPredicate>> {
        let (_, _, symbol, identity) = fact.payload.domain_membership()?;
        let domain = program.domain_definition_for_instance(symbol, identity)?;
        if domain.semantic_id != identity {
            return None;
        }
        let subject = crate::flow::canonical_place_from_semantic_place(
            program,
            semantic,
            semantic.places.get(fact.place.as_place()?),
        )?;
        if !availability.allows(&subject) {
            return None;
        }
        let position = if let Some(position) = domains
            .iter()
            .position(|row| row.symbol == symbol && row.identity == identity)
        {
            position
        } else {
            let position = domains.len();
            domains.push(DomainTemplate {
                symbol,
                identity,
                predicates: super::domain_predicates_for_subject(
                    program,
                    operators,
                    domain,
                    &CanonicalPlace {
                        root: facts::PlaceRoot::Symbol(domain.subject_symbol),
                        segments: Vec::new(),
                    },
                ),
            });
            position
        };
        Some(
            domains[position]
                .predicates
                .as_ref()?
                .iter()
                .map(|template| {
                    let mut predicate = template.clone();
                    for binding in &mut predicate.bindings {
                        let mut place = subject.clone();
                        place.extend_segments(&binding.place.segments);
                        binding.place = place;
                    }
                    predicate
                })
                .collect(),
        )
    }

    fn binding_order(left: &BoundPredicate, right: &BoundPredicate) -> Ordering {
        let handle_key = |handle: SymbolHandle| (handle.arena_index(), handle.generation());
        handle_key(left.machine)
            .cmp(&handle_key(right.machine))
            .then_with(|| handle_key(left.entry_state).cmp(&handle_key(right.entry_state)))
            .then_with(|| left.bindings.len().cmp(&right.bindings.len()))
            .then_with(|| {
                left.bindings
                    .iter()
                    .zip(&right.bindings)
                    .map(|(left, right)| left.place.compare_identity(&right.place))
                    .find(|order| !order.is_eq())
                    .unwrap_or(Ordering::Equal)
            })
    }

    fn unique_predicates(rows: Vec<BoundPredicate>) -> Vec<BoundPredicate> {
        let mut rows: Vec<_> = rows.into_iter().enumerate().collect();
        rows.sort_by(|(_, left), (_, right)| binding_order(left, right));
        let mut retained: Vec<(usize, BoundPredicate)> = Vec::new();
        let mut group_start = 0;
        for (order, row) in rows {
            if retained
                .last()
                .is_none_or(|(_, previous)| !binding_order(previous, &row).is_eq())
            {
                group_start = retained.len();
            }
            if !retained[group_start..]
                .iter()
                .any(|(_, previous)| *previous == row)
            {
                retained.push((order, row));
            }
        }
        retained.sort_by_key(|(order, _)| *order);
        retained.into_iter().map(|(_, row)| row).collect()
    }

    #[cfg(test)]
    mod tests {
        use super::{BoundObservation, BoundPredicate, CanonicalPlace, unique_predicates};
        use checked_trees::{CheckedBooleanExpression, CheckedScalarExpression};
        use facts::PlaceRoot;
        use symbols::SymbolHandle;
        use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

        #[test]
        fn predicate_buckets_preserve_complete_equality_and_first_appearance() {
            let first = BoundPredicate {
                machine: SymbolHandle::from_parts(10, 1),
                entry_state: SymbolHandle::from_parts(11, 1),
                expression: CheckedBooleanExpression::Constant(true),
                bindings: vec![BoundObservation {
                    operand: CheckedScalarExpression::Parameter {
                        position: 0,
                        primitive_type: PrimitiveType::U64,
                    }
                    .into(),
                    place: CanonicalPlace {
                        root: PlaceRoot::Symbol(SymbolHandle::from_parts(2, 1)),
                        segments: Vec::new(),
                    },
                    type_reference: TypeReferenceHandle::from_parts(3, 1),
                }],
            };
            let mut other_formula = first.clone();
            other_formula.expression = CheckedBooleanExpression::Constant(false);
            let mut earlier_key = first.clone();
            earlier_key.bindings[0].place.root = PlaceRoot::Symbol(SymbolHandle::from_parts(1, 1));
            let mut other_operand = first.clone();
            other_operand.bindings[0].operand = CheckedScalarExpression::Parameter {
                position: 1,
                primitive_type: PrimitiveType::U64,
            }
            .into();
            let mut other_type = first.clone();
            other_type.bindings[0].type_reference = TypeReferenceHandle::from_parts(3, 2);
            let mut other_generation = first.clone();
            other_generation.bindings[0].place.root =
                PlaceRoot::Symbol(SymbolHandle::from_parts(2, 2));
            let mut no_bindings = first.clone();
            no_bindings.bindings.clear();
            let rows = vec![
                first.clone(),
                other_formula.clone(),
                earlier_key,
                first,
                other_operand,
                other_type,
                other_generation,
                no_bindings,
                other_formula,
            ];
            let mut expected = Vec::new();
            for row in &rows {
                if !expected.contains(row) {
                    expected.push(row.clone());
                }
            }
            assert_eq!(unique_predicates(rows), expected);
        }
    }
}
use arena::HandleSpan;
use checked_trees::{CheckedBooleanExpression as Boolean, CheckedScalarExpression as Scalar};
use facts::{
    BooleanPredicateOperand as Operand, Fact, FactContextHandle, FactOrigin, FactPayload,
    FactPlace, FactPlan, Place, ProgramPoint, ScalarValue,
};
use normalization::normalize;
pub(crate) use paths::{
    EntryPaths, capture_entry_expression, capture_entry_guard, conjunction, entry_path_condition,
    entry_path_necessary_condition, seed_entry_paths,
};
pub(crate) use retention::{retained, retained_at_root};
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
    pub(crate) fn binding(&self, operand: &Operand) -> Option<&BoundObservation> {
        self.bindings
            .iter()
            .find(|binding| binding.operand == *operand)
    }

    /*
        `binding` for a scalar operand, read in place rather than cloned into
        an `Operand` just to be compared.
    */
    pub(crate) fn scalar_binding(&self, scalar: &Scalar) -> Option<&BoundObservation> {
        self.bindings
            .iter()
            .find(|binding| binding.operand.scalar() == Some(scalar))
    }

    /*
        A positive conjunction can establish an exact scalar equation. A
        disjunction supplies no chosen branch. Resolve the opposite leaf only
        from ordinary snapshots in this predicate's surviving context: using
        this equation (or another guarantee) recursively would invent evidence.
        Every binding is a storage dependency of the whole retained predicate.
    */
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
                        (predicate.scalar_binding(subject)?.place == *place).then_some(())?;
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
    pub operand: Operand,
    pub place: CanonicalPlace,
    pub type_reference: TypeReferenceHandle,
}

/*
    Rebase a whole current proposition onto authenticated invocation values.
    Every observation must survive the same write boundary; one stable field
    cannot authorize a clause whose other observation was changed or is local.
*/
pub(crate) fn entry_expression(
    program: &dyn ::validation::CheckingInput,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement: usize,
    predicate: &BoundPredicate,
) -> Option<Boolean> {
    entry_expression_with(program, machine, predicate, |place| {
        super::crash_entry_values::entry_parameter_place(program, machine, state, statement, place)
    })
}

/*
    One projection owner for a completed bound proposition. The caller supplies
    the authenticated observation cut; the shared entry scalar adapter retains
    exact authored structural positions and dense primitive positions.
*/
pub(crate) fn entry_expression_with(
    program: &dyn ::validation::CheckingInput,
    machine: SymbolHandle,
    predicate: &BoundPredicate,
    mut subject: impl FnMut(
        &CanonicalPlace,
    ) -> Option<(
        u32,
        Vec<checked_trees::CheckedStructuralPredicatePathSegment>,
    )>,
) -> Option<Boolean> {
    if predicate.machine != machine {
        return None;
    }
    let mut expression = predicate.expression.clone();
    map_observations(&mut expression, &mut |operand| {
        let binding = predicate.binding(operand)?;
        let (position, path) = subject(&binding.place)?;
        *operand = bindings::entry(program, machine, position, path, operand)?;
        Some(())
    })?;
    Some(expression)
}

/*
    Authored structural positions and dense scalar positions share one checked
    entry telescope. Preserve that distinction when publishing a transported
    observation rather than treating a structural index as a scalar index.
*/
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
        let reference = validation::field_domains::readable_type_reference(program, reference)?;
        if program.primitive_type_reference(reference) != Some(primitive) {
            return None;
        }
        return Some(Scalar::StructuralParameterField {
            observation: Default::default(),
            root: checked_trees::CheckedStorageRoot::Parameter { index: position },
            path,
            primitive_type: primitive,
        });
    }
    let position = usize::try_from(position).ok()?;
    let parameter = parameters.get(position)?;
    if let Some((observation, _)) = crate::values::scalar::primitive_reference_read::parameter(
        program,
        parameters,
        parameter.symbol,
    ) {
        return (observation.primitive_type() == Some(primitive)).then_some(observation);
    }
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

/*
    Bind an invocation formula once. Dense primitive positions and authored
    structural positions deliberately use separate lookups.
*/
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

/*
    Bind checked scalar coordinates against the exact supplied telescope. The
    caller retains declaration ownership separately from actual call positions.
    Results occupy the trailing scalar/structural position in their respective
    namespaces and bind to an exact invocation value, never a synthetic formal.
*/
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
        let (place, type_reference) = bindings::parameter(program, parameters, result, operand)?;
        if !bindings::compatible_type(program, operand, type_reference) {
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

/*
    Lower current guard algebra without binding a storage or value epoch.
    Flow supplies fresh completed observations when the guard actually selects
    its branch; caching this syntax cannot retain a previous read's address.
*/
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

/*
    The selected builtin equality used by a value pattern has the same checked
    observation bindings as an authored equality. It is not a comparison added
    to the immutable source tree and grants no meaning to selected operators.
*/
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
    mut observe: impl FnMut(&Operand) -> Option<(CanonicalPlace, TypeReferenceHandle)>,
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
        let type_reference =
            validation::field_domains::readable_type_reference(program, type_reference)?;
        if !bindings::compatible_type(program, operand, type_reference) {
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

/*
    Declaration-bound observations used when no completed evaluation is being
    consumed. Runtime indexed reads need their actual Flow observation instead;
    this conversion never invents a static path or re-evaluates a selector.
*/
pub(crate) fn declared_observation(
    program: &dyn ::validation::CheckingInput,
    state_symbol: SymbolHandle,
    statement_index: usize,
    operand: &Operand,
) -> Option<(CanonicalPlace, TypeReferenceHandle)> {
    let (_, state) = validation::find_state(program, state_symbol)?;
    if matches!(operand, Operand::CaseSubject(_)) {
        return bindings::parameter(program, program.state_parameters(state), None, operand);
    }
    let operand = operand.scalar()?;
    let symbols = crate::values::unit_scalar_binding_symbols(program, state, statement_index)?;
    let place = match operand {
        Scalar::Parameter { position, .. } | Scalar::Local { position, .. } => {
            crate::flow::canonical_place_from_symbol(*symbols.get(*position)?)?
        }
        Scalar::StorageRead { symbol, .. } => crate::flow::canonical_place_from_symbol(*symbol)?,
        Scalar::StructuralParameterField {
            root:
                checked_trees::CheckedStorageRoot::Parameter {
                    index: parameter_position,
                },
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

/*
    Additional evaluated receivers share the proposition's ordinary storage
    lifetime; they assert no independent truth about those receiver contents.
*/
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
        let place = crate::published::semantic::places::append_place(
            semantic,
            dependency.root,
            &dependency.segments,
        );
        crate::shared::collections::push_unique(&mut places, place);
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

/*
    Bind a domain's predicates to an independently selected live subject.
    This supplies goals, not membership premises: admission may use the same
    binding to veto a refuted claim without assuming the claim under judgment.
*/
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
        let binding = self.predicate.bindings.iter().find(|binding| matches!(binding.operand.scalar(), Some(Scalar::Parameter { position: candidate, .. } | Scalar::Local { position: candidate, .. }) if *candidate == position))?;
        (self.resolve)(&binding.place)
    }
    fn storage(&mut self, symbol: SymbolHandle) -> Option<ScalarValue> {
        let binding = self.predicate.bindings.iter().find(|binding| {
            matches!(binding.operand.scalar(),
            Some(Scalar::StorageRead { symbol: candidate, .. }) if *candidate == symbol)
        })?;
        (self.resolve)(&binding.place)
    }

    fn structural_field(
        &mut self,
        observation: typed_trees::expression::ExpressionHandle,
        parameter_position: u32,
        path: &[checked_trees::CheckedStructuralPredicatePathSegment],
    ) -> Option<ScalarValue> {
        let binding = self.predicate.bindings.iter().find(|binding| matches!(binding.operand.scalar(), Some(Scalar::StructuralParameterField { observation: candidate_observation, root: checked_trees::CheckedStorageRoot::Parameter { index: candidate }, path: candidate_path, .. }) if *candidate_observation == observation && *candidate == parameter_position && candidate_path == path))?;
        (self.resolve)(&binding.place)
    }
}

/*
    Finite truth propagation over the selected Boolean algebra. Integer
    comparisons remain whole observations: knowing one never supplies a value
    for either integer operand. Every round uses only already established facts.
    The requested place must occur as a Boolean observation: inferring all
    unrelated predicates cannot produce a Boolean value for an integer subject.
*/
pub(crate) fn inferred_boolean_at_place(
    predicates: &[BoundPredicate],
    actual: &CanonicalPlace,
    ordinary: impl FnMut(&CanonicalPlace) -> Option<ScalarValue>,
) -> Option<bool> {
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
    let position = observations.iter().position(|(place, primitive)| {
        *place == *actual && primitive.primitive() == Some(PrimitiveType::Bool)
    })?;
    let truths = formula_closure(&expressions, &[], &observations, ordinary).ok()?;
    truths.into_iter().find_map(|(expression, value)| {
        (expression == Boolean::Parameter { position }).then_some(value)
    })
}

/*
    The established truths ever disagreed: the premises cannot all hold. Kept
    distinct from "no answer" so a consumer that only needs feasibility can
    read a decided conflict instead of a closed door.
*/
struct Contradiction;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ObservationKind {
    Scalar(PrimitiveType),
    CaseSubject(TypeReferenceHandle),
}

impl ObservationKind {
    fn of(binding: &BoundObservation) -> Option<Self> {
        match &binding.operand {
            Operand::Scalar(scalar) => Some(Self::Scalar(scalar.primitive_type()?)),
            Operand::CaseSubject(_) => binding
                .type_reference
                .is_valid()
                .then_some(Self::CaseSubject(binding.type_reference)),
        }
    }
    fn primitive(self) -> Option<PrimitiveType> {
        match self {
            Self::Scalar(primitive) => Some(primitive),
            Self::CaseSubject(_) => None,
        }
    }
    fn operand(self, position: usize) -> Option<Operand> {
        match self {
            Self::Scalar(primitive_type) => Some(Operand::Scalar(Scalar::Parameter {
                position,
                primitive_type,
            })),
            Self::CaseSubject(_) => Some(Operand::CaseSubject(
                checked_trees::CheckedStructuralCaseSubject {
                    root: checked_trees::CheckedStructuralCaseRoot::Parameter {
                        position: u32::try_from(position).ok()?,
                    },
                    path: Vec::new(),
                },
            )),
        }
    }
}
type Observation = (CanonicalPlace, ObservationKind);

/*
    Whether no assignment satisfies `predicates`. Only a conflict between two
    truths this algebra has already established makes a premise set
    unsatisfiable, so the answer never rests on a leaf value a caller resolved:
    the refuted-arm rule must not conclude `false` from a stale read.
*/
pub(crate) fn unsatisfiable(predicates: &[BoundPredicate]) -> bool {
    let Some((expressions, observations)) = normalize(predicates) else {
        return false;
    };
    formula_closure(&expressions, &[], &observations, |_| None).is_err()
}

fn formula_closure(
    assertions: &[Boolean],
    queries: &[Boolean],
    observations: &[Observation],
    mut ordinary: impl FnMut(&CanonicalPlace) -> Option<ScalarValue>,
) -> Result<Vec<(Boolean, bool)>, Contradiction> {
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
    fn put(
        truths: &mut Vec<(Boolean, bool)>,
        expression: &Boolean,
        value: bool,
    ) -> Result<(), Contradiction> {
        if let Some((_, prior)) = truths.iter().find(|(known, _)| known == expression) {
            return (*prior == value).then_some(()).ok_or(Contradiction);
        }
        truths.push((expression.clone(), value));
        Ok(())
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
        .map(|(place, kind)| kind.primitive().and_then(|_| ordinary(place)))
        .collect::<Vec<_>>();
    let mut expressions = Vec::new();
    let mut truths = Vec::new();
    for assertion in assertions {
        collect(assertion, &mut expressions);
        put(&mut truths, assertion, true)?;
    }
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
                Boolean::IntegerComparison {
                    kind: checked_trees::CheckedIntegerComparisonKind::Equal,
                    left,
                    right,
                } => {
                    if let Some(value) = result {
                        put(
                            &mut additions,
                            &Boolean::IntegerComparison {
                                kind: checked_trees::CheckedIntegerComparisonKind::Equal,
                                left: right.clone(),
                                right: left.clone(),
                            },
                            value,
                        )?;
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
            return Ok(truths);
        }
    }
}

/*
    Fold exact Boolean observations before moving a relation to a named state.
    An unforwarded selector can disappear once the selected edge knows its value.
*/
pub(crate) fn fold(
    predicate: &BoundPredicate,
    mut resolve: impl FnMut(&CanonicalPlace) -> Option<ScalarValue>,
) -> Option<BoundPredicate> {
    let mut result = predicate.clone();
    map_observations(&mut result.expression, &mut |operand| {
        let binding = predicate.binding(operand)?;
        if operand.primitive_type() == Some(PrimitiveType::Bool)
            && let Some(ScalarValue::Boolean(value)) = resolve(&binding.place)
        {
            *operand = Operand::Scalar(Scalar::Boolean(Box::new(Boolean::Constant(value))));
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

/*
    Traverse the existing selected algebra. This does not infer a primitive
    meaning for authored operators or treat opaque observations as values.
*/
pub(crate) fn map_observations(
    expression: &mut Boolean,
    map: &mut impl FnMut(&mut Operand) -> Option<()>,
) -> Option<()> {
    match expression {
        Boolean::Constant(_) => {}
        Boolean::Parameter { position } => {
            let mut scalar = Scalar::Parameter {
                position: *position,
                primitive_type: PrimitiveType::Bool,
            };
            map_scalar_observation(&mut scalar, map)?;
            *expression = boolean_operand(scalar)?;
        }
        Boolean::Local { position } => {
            let mut scalar = Scalar::Local {
                position: *position,
                primitive_type: PrimitiveType::Bool,
            };
            map_scalar_observation(&mut scalar, map)?;
            *expression = boolean_operand(scalar)?;
        }
        Boolean::StorageRead { symbol } => {
            let mut scalar = Scalar::StorageRead {
                symbol: *symbol,
                primitive_type: PrimitiveType::Bool,
            };
            map_scalar_observation(&mut scalar, map)?;
            *expression = boolean_operand(scalar)?;
        }
        Boolean::StructuralParameterField {
            observation,
            parameter_position,
            path,
        } => {
            let mut scalar = Scalar::StructuralParameterField {
                observation: *observation,
                root: checked_trees::CheckedStorageRoot::Parameter {
                    index: *parameter_position,
                },
                path: path.clone(),
                primitive_type: PrimitiveType::Bool,
            };
            map_scalar_observation(&mut scalar, map)?;
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
        Boolean::ErasedParameter { .. }
        | Boolean::IeeeFloatComparison { .. }
        | Boolean::StructuralIdentity { .. }
        | Boolean::ByteSequenceEqual { .. }
        | Boolean::ByteSequencePredicate { .. } => return None,
        Boolean::StructuralCaseMembership { subject, .. } => {
            let mut observation = Operand::CaseSubject(subject.clone());
            map(&mut observation)?;
            let Operand::CaseSubject(mapped) = observation else {
                return None;
            };
            *subject = mapped;
        }
    }
    Some(())
}

/*
    A visitor can rebind observations only within their real value family.
    Scalar computations keep their selected algebra; case subjects keep the
    structural namespace and are never converted to numeric or Boolean loads.
*/
fn map_scalar_observation(
    scalar: &mut Scalar,
    map: &mut impl FnMut(&mut Operand) -> Option<()>,
) -> Option<()> {
    let mut observation = Operand::Scalar(scalar.clone());
    map(&mut observation)?;
    let Operand::Scalar(mapped) = observation else {
        return None;
    };
    *scalar = mapped;
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
            root:
                checked_trees::CheckedStorageRoot::Parameter {
                    index: parameter_position,
                },
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
    map: &mut impl FnMut(&mut Operand) -> Option<()>,
) -> Option<()> {
    match expression {
        Scalar::Parameter { .. }
        | Scalar::StructuralParameterField {
            root: checked_trees::CheckedStorageRoot::Parameter { .. },
            ..
        }
        | Scalar::StructuralParameterIndexedRead { .. }
        | Scalar::Local { .. }
        | Scalar::StorageRead { .. } => map_scalar_observation(expression, map)?,
        Scalar::Boolean(expression) => map_observations(expression, map)?,
        Scalar::IntegerBinary { left, right, .. } => {
            map_scalar(left, map)?;
            map_scalar(right, map)?;
        }
        Scalar::IntegerBitwiseNot { operand, .. }
        | Scalar::IntegerWiden { operand, .. }
        | Scalar::IntegerExactCast { operand, .. }
        | Scalar::RepresentationView { operand, .. } => map_scalar(operand, map)?,
        Scalar::ConstantApplication { .. }
        | Scalar::IntegerLiteral { .. }
        | Scalar::IeeeFloatLiteral { .. }
        | Scalar::ImplementationSelection { .. } => {}
        Scalar::StructuralSequenceLength { .. }
        | Scalar::StructuralParameterField {
            root:
                checked_trees::CheckedStorageRoot::OwnedLocal { .. }
                | checked_trees::CheckedStorageRoot::ViewLocal { .. },
            ..
        }
        | Scalar::ErasedParameter { .. } => return None,
    }
    Some(())
}

/*
    A conditional edge owes !guard || requirement. Normalize both formulas
    into one exact observation namespace before constructing that goal; equal
    operand ordinals from different frames are not equal storage observations.
    This creates no premise and changes no contradiction/formation judgment.
*/
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
        .map(|(position, (place, kind))| {
            let source = guard
                .bindings
                .iter()
                .chain(&required.bindings)
                .find(|binding| {
                    binding.place == place && ObservationKind::of(binding) == Some(kind)
                })?;
            Some(BoundObservation {
                operand: kind.operand(position)?,
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

/*
    Prove a whole bound formula, including an implication whose selector is
    still unknown. Retained formulas compare only after their observations are
    normalized through exact current places, not declaration positions.
*/
pub(crate) fn proves(
    predicates: &[BoundPredicate],
    required: &BoundPredicate,
    ordinary: impl FnMut(&CanonicalPlace) -> Option<ScalarValue>,
) -> bool {
    truth(predicates, required, ordinary) == Some(true)
}

/*
    Recover either truth from the same current-observation closure. Unknown
    remains absent; a failed positive judgment never supplies a false premise.
*/
pub(crate) fn truth(
    predicates: &[BoundPredicate],
    required: &BoundPredicate,
    mut ordinary: impl FnMut(&CanonicalPlace) -> Option<ScalarValue>,
) -> Option<bool> {
    let mut all = predicates.to_vec();
    all.push(required.clone());
    let (mut expressions, observations) = normalize(&all)?;
    let normalized_required = expressions.pop().expect("required formula");
    let truths = formula_closure(
        &expressions,
        std::slice::from_ref(&normalized_required),
        &observations,
        &mut ordinary,
    )
    .ok()?;
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

#[cfg(test)]
mod tests {
    use super::{
        Boolean, BoundObservation, BoundPredicate, CanonicalPlace, Scalar, truth, unsatisfiable,
    };
    use facts::PlaceRoot;
    use symbols::SymbolHandle;
    use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

    /*
        One Boolean observation of one place, asserted true (`value`) or
        asserted false by its negation -- the shape `bind_current_expression`
        publishes for a guard arm.
    */
    fn asserted(value: bool) -> BoundPredicate {
        let observation = Boolean::Parameter { position: 0 };
        BoundPredicate {
            machine: SymbolHandle::from_parts(10, 1),
            entry_state: SymbolHandle::from_parts(11, 1),
            expression: if value {
                observation
            } else {
                Boolean::Not(Box::new(observation))
            },
            bindings: vec![BoundObservation {
                operand: Scalar::Parameter {
                    position: 0,
                    primitive_type: PrimitiveType::Bool,
                }
                .into(),
                place: CanonicalPlace {
                    root: PlaceRoot::Symbol(SymbolHandle::from_parts(2, 1)),
                    segments: Vec::new(),
                },
                type_reference: TypeReferenceHandle::from_parts(3, 1),
            }],
        }
    }

    #[test]
    fn a_refuted_observation_is_unsatisfiable_and_agreement_is_not() {
        assert!(unsatisfiable(&[asserted(true), asserted(false)]));
        assert!(!unsatisfiable(&[asserted(true)]));
        assert!(!unsatisfiable(&[asserted(true), asserted(true)]));
        assert!(!unsatisfiable(&[]));
    }

    #[test]
    fn a_conflict_never_becomes_a_positive_goal_answer() {
        assert_eq!(
            truth(&[asserted(true), asserted(false)], &asserted(true), |_| {
                None
            }),
            None
        );
        assert_eq!(
            truth(&[asserted(true)], &asserted(true), |_| None),
            Some(true)
        );
    }
}
