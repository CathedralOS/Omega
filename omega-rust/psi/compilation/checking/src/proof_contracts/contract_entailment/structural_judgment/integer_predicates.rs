//! Integer consequences of already captured runtime predicates.
//!
//! Normal-return capture must discharge a selected requirement before importing
//! its guarantees. Its operands may contain Exact arithmetic, so syntactic truth
//! transport alone cannot prove `n - 1 >= 0` from the selected guard `n > 0`.
//! Project observed integer terms into the existing arithmetic engine, with a
//! query-local atom per complete value identity. No source expression is replayed,
//! no caller state is reread, and no future call guarantee enters the premises.
//! Wrapping/saturating operations remain opaque values rather than integer laws;
//! Each expanded operation retains its carrier formation obligation until the
//! authenticated premises establish it, independently of the requested goal.

use super::super::arithmetic_judgment::LowerBoundQueries;
use super::super::{BigInt, Engine, Polynomial};
use super::{StructuralJudge, StructuralJudgment, StructuralTerm};
use std::ops::Range;
use typed_trees::expression::BinaryOperator;
use typed_trees::types::PrimitiveType;

type Comparison = (BinaryOperator, Polynomial, Polynomial);

/// A noninteger predicate has not entered numeric projection. A selected
/// comparison with an unavailable operand may already have registered facts;
/// optional shared queries must discard that entire preparation.
enum IntegerComparisonProjection {
    NotInteger,
    UnavailableOperand,
    Comparison(Comparison),
}

struct ProjectedIntegerPremises {
    comparisons: Vec<(Comparison, Range<usize>)>,
    complete: bool,
}

#[derive(PartialEq, Eq)]
struct IntegerProjectionRegistrations {
    atoms: usize,
    operations: usize,
    carrier_bounds: usize,
    engine: (usize, usize, usize, usize),
}

#[derive(Clone)]
struct ObservedOperation {
    operator: BinaryOperator,
    primitives: [PrimitiveType; 2],
    operands: [Polynomial; 2],
    value: Polynomial,
    children: Range<usize>,
    admitted: bool,
}

/// Intrinsic bounds retain exact selected carrier and polynomial identity.
#[derive(Clone)]
struct ObservedCarrierBounds {
    primitive: PrimitiveType,
    value: Polynomial,
    comparisons: [Comparison; 2],
}

/// Query-local preparation is deliberately separate from admission. Only a
/// formed premise may enter the engine; an equation involving division cannot
/// establish its own nonzero divisor. The requested comparison never enters
/// this premise worklist. Children are admitted before their consuming node,
/// including nodes whose normalized value cancelled out of the comparison.
#[derive(Clone)]
struct IntegerObservations<'program> {
    engine: Engine<'program>,
    atoms: Vec<StructuralTerm>,
    operations: Vec<ObservedOperation>,
    carrier_bounds: Vec<ObservedCarrierBounds>,
    installed_carrier_bounds: usize,
}

/// Every retained comparison and operation has been independently admitted.
/// Cloning the entire semantic engine preserves substitutions, opaque terms,
/// selected meanings and interval state. Requested goals never modify this owner.
struct PreparedIntegerPremises<'program> {
    observations: IntegerObservations<'program>,
}

impl<'program> PreparedIntegerPremises<'program> {
    fn for_goal(&self) -> IntegerObservations<'program> {
        self.observations.clone()
    }
}

impl IntegerObservations<'_> {
    fn projection_registrations(&self) -> IntegerProjectionRegistrations {
        IntegerProjectionRegistrations {
            atoms: self.atoms.len(),
            operations: self.operations.len(),
            carrier_bounds: self.carrier_bounds.len(),
            engine: self.engine.integer_projection_registrations(),
        }
    }

    fn install_observed_carrier_bounds(
        &mut self,
        primitive: PrimitiveType,
        value: &Polynomial,
    ) -> Option<()> {
        if primitive != PrimitiveType::Addr
            && let Some(constant) = value.constant_value()
            && let Some((minimum, maximum)) = primitive.integer_bounds()
            && constant >= BigInt::from_i128(minimum)
            && constant <= BigInt::from_i128(maximum)
        {
            return Some(());
        }
        if self
            .carrier_bounds
            .iter()
            .any(|bounds| bounds.primitive == primitive && bounds.value == *value)
        {
            return Some(());
        }
        let (minimum, maximum, upper_operator) = if primitive == PrimitiveType::Addr {
            (
                BigInt::zero(),
                self.engine.address_model_bound()?,
                BinaryOperator::Less,
            )
        } else {
            let (minimum, maximum) = primitive.integer_bounds()?;
            (
                BigInt::from_i128(minimum),
                Polynomial::constant(BigInt::from_i128(maximum)),
                BinaryOperator::LessOrEqual,
            )
        };
        let comparisons = [
            (
                BinaryOperator::GreaterOrEqual,
                value.clone(),
                Polynomial::constant(minimum),
            ),
            (upper_operator, value.clone(), maximum),
        ];
        self.carrier_bounds.push(ObservedCarrierBounds {
            primitive,
            value: value.clone(),
            comparisons,
        });
        Some(())
    }

    fn admitted(&self, operations: Range<usize>) -> bool {
        self.operations[operations]
            .iter()
            .all(|operation| operation.admitted)
    }

    /// Positive evidence and tri-valued judgment share this exact proof body.
    /// Requesting evidence does not require proving the unused opposite.
    fn comparison_holds(
        queries: &LowerBoundQueries<'_, '_>,
        operator: BinaryOperator,
        left: &Polynomial,
        right: &Polynomial,
    ) -> bool {
        let engine = queries.engine();
        let proves = |difference: Polynomial, minimum| {
            queries.prove_at_least(&engine.substituted(&difference), &BigInt::from_i64(minimum))
        };
        match operator {
            BinaryOperator::Less => proves(right.sub(left), 1),
            BinaryOperator::LessOrEqual => proves(right.sub(left), 0),
            BinaryOperator::Greater => proves(left.sub(right), 1),
            BinaryOperator::GreaterOrEqual => proves(left.sub(right), 0),
            BinaryOperator::Equal => proves(left.sub(right), 0) && proves(right.sub(left), 0),
            BinaryOperator::NotEqual => {
                engine.is_known_nonzero(&engine.substituted(&left.sub(right)))
                    || proves(left.sub(right), 1)
                    || proves(right.sub(left), 1)
            }
            _ => false,
        }
    }

    fn judge_comparison(
        queries: &LowerBoundQueries<'_, '_>,
        (operator, left, right): Comparison,
    ) -> StructuralJudgment {
        if Self::comparison_holds(queries, operator, &left, &right) {
            StructuralJudgment::Proven
        } else if operator
            .complement()
            .is_some_and(|complement| Self::comparison_holds(queries, complement, &left, &right))
        {
            StructuralJudgment::Refuted
        } else {
            StructuralJudgment::Unknown
        }
    }

    fn install_premises(&mut self, premises: &mut Vec<(Comparison, Range<usize>)>) -> bool {
        if self.installed_carrier_bounds < self.carrier_bounds.len() {
            let comparisons = self.carrier_bounds[self.installed_carrier_bounds..]
                .iter()
                .flat_map(|bounds| bounds.comparisons.clone())
                .collect();
            if !self.engine.install_hypotheses(comparisons) {
                return false;
            }
            // A failed installation never advances this context's frontier.
            self.installed_carrier_bounds = self.carrier_bounds.len();
        }
        loop {
            let mut changed = false;
            // Admission stays fixed throughout this premise round. Install
            // its already-formed comparisons together, in existing intake order,
            // so every fact does not rebuild the retained difference matrix.
            // Only the later operation pass can admit another child or parent;
            // neither an unformed premise nor the goal joins this batch.
            let admitted_comparisons = premises
                .extract_if(.., |(_, operations)| self.admitted(operations.clone()))
                .map(|(comparison, _)| comparison)
                .collect::<Vec<_>>();
            if !admitted_comparisons.is_empty() {
                if !self.engine.install_hypotheses(admitted_comparisons) {
                    return false;
                }
                changed = true;
            }
            self.engine.refresh_opaque_intervals();
            for index in 0..self.operations.len() {
                let operation = &self.operations[index];
                if !self.admitted(operation.children.clone()) {
                    continue;
                }
                let [left, right] = &operation.operands;
                if operation.admitted
                    || self.engine.integer_operation_lands(
                        operation.operator,
                        operation.primitives,
                        left,
                        right,
                        &operation.value,
                    )
                {
                    changed |= self.engine.install_integer_operation_bounds(
                        operation.operator,
                        left,
                        right,
                        &operation.value,
                    );
                    changed |= !operation.admitted;
                    self.operations[index].admitted = true;
                }
            }
            if !changed {
                return true;
            }
        }
    }
}

impl StructuralJudge<'_> {
    /// Refute this prefix from independently formed live premises alone.
    /// A requested operation or its result never contributes to this query.
    /// Incomplete projection or admission means unknown, not an impossible path.
    pub(in crate::proof_contracts::contract_entailment) fn integer_premises_contradictory(
        &self,
    ) -> bool {
        if !self.runtime_body_values || self.case_facts_only {
            return false;
        }
        let Some(prepared) = self.prepare_integer_premises() else {
            return false;
        };
        prepared.observations.engine.requires_unsatisfiable
            || prepared
                .observations
                .engine
                .lower_bound_queries()
                .prove_at_least(&Polynomial::constant(BigInt::from_i64(-1)), &BigInt::zero())
    }

    /// Ask about one completed operand, before introducing its cast result.
    /// The address ceiling remains the builtin symbolic model bound; the
    /// hosted producer's current Addr64 representation is not a premise.
    pub(in crate::proof_contracts::contract_entailment) fn observed_integer_fits(
        &self,
        value: StructuralTerm,
        source_reference: typed_trees::types::TypeReferenceHandle,
        target: PrimitiveType,
    ) -> bool {
        let value = StructuralTerm::IntegerEmbedding {
            reference: source_reference,
            value: Box::new(value),
        };
        self.integer_term_fits(value, target)
    }

    /// The caller supplies an independently formed mathematical integer. This
    /// does not create a runtime carrier or acquire fit from an operation's own
    /// result. Both observed casts and anonymous operand landing share it.
    pub(in crate::proof_contracts::contract_entailment) fn integer_term_fits(
        &self,
        value: StructuralTerm,
        target: PrimitiveType,
    ) -> bool {
        matches!(
            self.integer_term_fit_judgment(value, target),
            super::super::operand_fits::CarrierFit::Proven
        )
    }

    pub(in crate::proof_contracts::contract_entailment) fn integer_term_fit_judgment(
        &self,
        value: StructuralTerm,
        target: PrimitiveType,
    ) -> super::super::operand_fits::CarrierFit {
        use super::super::operand_fits::CarrierFit;
        if !self.runtime_body_values || self.case_facts_only {
            return CarrierFit::Unknown;
        }
        let mut observations = IntegerObservations {
            engine: Engine::for_explicit_bindings(self.program),
            atoms: Vec::new(),
            operations: Vec::new(),
            carrier_bounds: Vec::new(),
            installed_carrier_bounds: 0,
        };
        let Some(value) = self.mathematical_value(&value, &mut observations, 0) else {
            return CarrierFit::Unknown;
        };
        let (minimum, maximum, exclusive) = if target == PrimitiveType::Addr {
            let Some(bound) = self
                .program
                .symbols
                .builtin_type_member_symbol(symbols::BuiltinTypeMember::AddressBound)
            else {
                return CarrierFit::Unknown;
            };
            // Resolve the bound in this captured judge before projecting it.
            // Recreating a fresh raw model atom here would lose an already
            // authenticated replacement such as the prior `addr::Bound == 16`.
            let Some(bound) =
                self.mathematical_value(&StructuralTerm::BoundValue(bound), &mut observations, 0)
            else {
                return CarrierFit::Unknown;
            };
            (BigInt::zero(), bound, true)
        } else {
            let Some((minimum, maximum)) = target.integer_bounds() else {
                return CarrierFit::Unknown;
            };
            (
                BigInt::from_i128(minimum),
                Polynomial::constant(BigInt::from_i128(maximum)),
                false,
            )
        };
        let goal_operations = 0..observations.operations.len();
        let mut premises = self.observed_integer_premises(&mut observations);
        if !observations.install_premises(&mut premises) || !observations.admitted(goal_operations)
        {
            return CarrierFit::Unknown;
        }
        observations.engine.integer_value_fit(
            &value,
            &minimum,
            &maximum,
            exclusive,
            target == PrimitiveType::Addr,
        )
    }

    pub(in crate::proof_contracts::contract_entailment) fn judge_observed_predicate(
        &self,
        predicate: StructuralTerm,
        depth: usize,
    ) -> StructuralJudgment {
        let resolved = self.resolve_at(predicate.clone(), depth + 1);
        match self.judge_equation(
            resolved.clone(),
            super::value_predicates::boolean_term(true),
            depth + 1,
        ) {
            StructuralJudgment::Unknown => {
                let direct = self
                    .observed_float_predicate_judgment(&predicate, depth)
                    .unwrap_or_else(|| self.observed_integer_predicate_judgment(&predicate));
                if matches!(direct, StructuralJudgment::Unknown)
                    && self.proves_observed_integer_boolean(&resolved, depth) == Some(true)
                {
                    StructuralJudgment::Proven
                } else {
                    direct
                }
            }
            judgment => judgment,
        }
    }

    /// Branch-wise truth is incomplete for alternatives such as x < y or
    /// y < x under an already observed disequality. Deny the complete formed
    /// question in a private integer query, and discharge every alternative.
    /// Denial never contributes to operation formation or another proof goal.
    fn proves_observed_integer_boolean(
        &self,
        predicate: &StructuralTerm,
        depth: usize,
    ) -> Option<bool> {
        if !self.runtime_body_values || self.case_facts_only || depth >= 32 {
            return None;
        }
        let mut observations = IntegerObservations {
            engine: Engine::for_explicit_bindings(self.program),
            atoms: Vec::new(),
            operations: Vec::new(),
            carrier_bounds: Vec::new(),
            installed_carrier_bounds: 0,
        };
        let mut remaining = 64usize;
        let alternatives =
            self.integer_boolean_alternatives(predicate, false, &mut observations, &mut remaining)?;
        let goal_operations = 0..observations.operations.len();
        let mut premises = self.observed_integer_premises(&mut observations);
        let retained = premises.clone();
        // Only real prefix facts may admit a partial operand. Install the
        // denied question below, after all of its operations have formed.
        if !observations.install_premises(&mut premises) || !observations.admitted(goal_operations)
        {
            return None;
        }
        let admitted = retained
            .into_iter()
            .filter(|(_, operations)| observations.admitted(operations.clone()))
            .map(|(comparison, _)| comparison)
            .collect::<Vec<_>>();
        for alternative in alternatives {
            let mut comparisons = admitted.clone();
            comparisons.extend(alternative);
            // Intake the complete batch together: an unoriented prior NE must
            // see the denied branch's bounds, without gaining a direction in
            // the original context. The admitted engine keeps operation laws
            // and the exact private captured-value atom namespace.
            let mut engine = observations.engine.clone();
            if !engine.integer_comparisons_unsatisfiable(comparisons) {
                return Some(false);
            }
        }
        Some(true)
    }

    fn integer_boolean_alternatives(
        &self,
        predicate: &StructuralTerm,
        holds: bool,
        observations: &mut IntegerObservations<'_>,
        remaining: &mut usize,
    ) -> Option<Vec<Vec<Comparison>>> {
        *remaining = remaining.checked_sub(1)?;
        let predicate = predicate.semantic_value();
        let truth = super::value_predicates::boolean_term(holds);
        if *predicate == truth {
            return Some(vec![Vec::new()]);
        }
        if *predicate == super::value_predicates::boolean_term(!holds) {
            return Some(Vec::new());
        }
        if let Some((operand, truth)) = self.boolean_equality_operand(predicate, &truth) {
            return self.integer_boolean_alternatives(
                &operand,
                truth == super::value_predicates::boolean_term(true),
                observations,
                remaining,
            );
        }
        if let StructuralTerm::ScalarBinary {
            references,
            operator,
            meaning,
            left,
            right,
        } = predicate
            && matches!(operator, BinaryOperator::And | BinaryOperator::Or)
        {
            if !meaning.iter().all(|(primitive, domain)| {
                *primitive == PrimitiveType::Bool
                    && *domain == numerics::arithmetic::ArithmeticDomain::Exact
            }) || ![left, right]
                .into_iter()
                .zip(references)
                .all(|(operand, reference)| {
                    super::super::structural_terms::scalar_operand_is_observed(
                        self.program,
                        *reference,
                        operand,
                    )
                })
            {
                return None;
            }
            let left = self.integer_boolean_alternatives(left, holds, observations, remaining)?;
            let right = self.integer_boolean_alternatives(right, holds, observations, remaining)?;
            if (*operator == BinaryOperator::And) == holds {
                if left.len().checked_mul(right.len())? > 32 {
                    return None;
                }
                let mut alternatives = Vec::new();
                for first in &left {
                    for second in &right {
                        if first.len().checked_add(second.len())? > 64 {
                            return None;
                        }
                        let mut combined = first.clone();
                        combined.extend(second.iter().cloned());
                        alternatives.push(combined);
                    }
                }
                return Some(alternatives);
            }
            if left.len().checked_add(right.len())? > 32 {
                return None;
            }
            let mut alternatives = left;
            alternatives.extend(right);
            return Some(alternatives);
        }
        let (operator, left, right) = self.integer_comparison(predicate, observations)?;
        Some(vec![vec![(
            if holds {
                operator
            } else {
                operator.complement()?
            },
            left,
            right,
        )]])
    }

    pub(in crate::proof_contracts::contract_entailment) fn prove_observed_integer_ordering_minimum(
        &self,
        left_reference: typed_trees::types::TypeReferenceHandle,
        left: &StructuralTerm,
        right_reference: typed_trees::types::TypeReferenceHandle,
        right: &StructuralTerm,
    ) -> Option<i64> {
        let comparison = |operator| StructuralTerm::MathematicalBinary {
            operator,
            left: Box::new(StructuralTerm::IntegerEmbedding {
                reference: left_reference,
                value: Box::new(left.clone()),
            }),
            right: Box::new(StructuralTerm::IntegerEmbedding {
                reference: right_reference,
                value: Box::new(right.clone()),
            }),
        };
        let greater = comparison(BinaryOperator::Greater);
        let resolved_greater = self.resolve_at(greater.clone(), 1);
        let greater_eq_judgment = self.judge_equation(
            resolved_greater.clone(),
            super::value_predicates::boolean_term(true),
            1,
        );
        if matches!(greater_eq_judgment, StructuralJudgment::Proven) {
            return Some(1);
        }
        let greater_or_equal = comparison(BinaryOperator::GreaterOrEqual);
        let resolved_ge = self.resolve_at(greater_or_equal, 1);
        let ge_eq_judgment = self.judge_equation(
            resolved_ge.clone(),
            super::value_predicates::boolean_term(true),
            1,
        );
        let Some((observations, (_, left_poly, right_poly))) =
            self.integer_predicate_observations(&greater)
        else {
            if matches!(greater_eq_judgment, StructuralJudgment::Unknown)
                && self.proves_observed_integer_boolean(&resolved_greater, 1) == Some(true)
            {
                return Some(1);
            }
            if matches!(ge_eq_judgment, StructuralJudgment::Proven)
                || (matches!(ge_eq_judgment, StructuralJudgment::Unknown)
                    && self.proves_observed_integer_boolean(&resolved_ge, 1) == Some(true))
            {
                return Some(0);
            }
            return None;
        };
        let queries = observations.engine.lower_bound_queries();
        if IntegerObservations::comparison_holds(
            &queries,
            BinaryOperator::Greater,
            &left_poly,
            &right_poly,
        ) || (matches!(greater_eq_judgment, StructuralJudgment::Unknown)
            && !IntegerObservations::comparison_holds(
                &queries,
                BinaryOperator::LessOrEqual,
                &left_poly,
                &right_poly,
            )
            && self.proves_observed_integer_boolean(&resolved_greater, 1) == Some(true))
        {
            Some(1)
        } else if matches!(ge_eq_judgment, StructuralJudgment::Proven)
            || IntegerObservations::comparison_holds(
                &queries,
                BinaryOperator::GreaterOrEqual,
                &left_poly,
                &right_poly,
            )
            || (matches!(ge_eq_judgment, StructuralJudgment::Unknown)
                && !IntegerObservations::comparison_holds(
                    &queries,
                    BinaryOperator::Less,
                    &left_poly,
                    &right_poly,
                )
                && self.proves_observed_integer_boolean(&resolved_ge, 1) == Some(true))
        {
            Some(0)
        } else {
            None
        }
    }

    /// Prepare independently formed premises once, then fork their exact
    /// captured-value namespace for each goal. Goal operations and opaque
    /// registrations remain private to that fork; no requested truth enters
    /// another goal's premises. Preparation without goal-supplied leaf bounds
    /// may lose a sufficient proof, so absent shared evidence retains the
    /// original individual proof. Association consumers do not use refutation.
    pub(in crate::proof_contracts::contract_entailment) fn prove_observed_predicates(
        &self,
        predicates: &[StructuralTerm],
        depth: usize,
    ) -> Vec<bool> {
        let mut proofs = Vec::with_capacity(predicates.len());
        let mut unresolved = Vec::new();
        for (position, predicate) in predicates.iter().enumerate() {
            let judgment = self.judge_equation(
                self.resolve_at(predicate.clone(), depth + 1),
                super::value_predicates::boolean_term(true),
                depth + 1,
            );
            if matches!(judgment, StructuralJudgment::Unknown) {
                unresolved.push(position);
            }
            proofs.push(matches!(judgment, StructuralJudgment::Proven));
        }
        let shared = self.prepared_predicate_proofs(predicates, &unresolved);
        for (index, position) in unresolved.into_iter().enumerate() {
            // Shared false grants no negative fact. Preserve the original
            // goal-first sufficient proof when the goal registered operations.
            proofs[position] = match shared
                .as_ref()
                .and_then(|values| values.get(index))
                .copied()
                .flatten()
            {
                Some(proven) => proven,
                None => self.proves_observed_integer_predicate(&predicates[position]),
            };
        }
        proofs
    }

    fn prepared_predicate_proofs(
        &self,
        predicates: &[StructuralTerm],
        unresolved: &[usize],
    ) -> Option<Vec<Option<bool>>> {
        if !self.runtime_body_values || self.case_facts_only || unresolved.is_empty() {
            return None;
        }
        let prepared = self.prepare_integer_premises()?;
        let base_atoms = prepared.observations.atoms.len();
        let base_bounds = prepared.observations.carrier_bounds.len();
        let base_param_atoms = prepared.observations.engine.parameter_atoms.len();
        let base_engine_reg = prepared
            .observations
            .engine
            .integer_projection_registrations();
        let has_premises = prepared.observations.installed_carrier_bounds > 0
            || base_atoms > 0
            || base_engine_reg != (0, 0, 0, 0);
        let prepared_queries = std::sync::OnceLock::new();
        let queries_and_contradictory = || {
            prepared_queries.get_or_init(|| {
                let queries = prepared.observations.engine.lower_bound_queries();
                let contradictory = queries
                    .prove_at_least(&Polynomial::constant(BigInt::from_i64(-1)), &BigInt::zero());
                (queries, contradictory)
            })
        };
        let mut work = prepared.for_goal();
        Some(
            unresolved
                .iter()
                .map(|position| {
                    work.atoms.truncate(base_atoms);
                    work.carrier_bounds.truncate(base_bounds);
                    work.engine.parameter_atoms.truncate(base_param_atoms);
                    let comparison = match self
                        .project_integer_comparison(&predicates[*position], &mut work)
                    {
                        IntegerComparisonProjection::Comparison(comparison) => comparison,
                        IntegerComparisonProjection::NotInteger
                        | IntegerComparisonProjection::UnavailableOperand => {
                            work.operations.clear();
                            if work.engine.integer_projection_registrations().1 != base_engine_reg.1
                            {
                                work = prepared.for_goal();
                            }
                            return Some(false);
                        }
                    };
                    let (operator, left, right) = comparison;
                    let goal_only_linear_ops = work.operations.iter().all(|op| {
                        matches!(
                            op.operator,
                            BinaryOperator::Add
                                | BinaryOperator::Subtract
                                | BinaryOperator::Multiply
                        )
                    });
                    if work.engine.integer_projection_registrations().1 == base_engine_reg.1
                        && !work.carrier_bounds[base_bounds..]
                            .iter()
                            .any(|bound| bound.primitive == PrimitiveType::Addr)
                    {
                        if work.operations.is_empty()
                            && matches!(
                                operator,
                                BinaryOperator::Equal
                                    | BinaryOperator::LessOrEqual
                                    | BinaryOperator::GreaterOrEqual
                            )
                            && left == right
                        {
                            return Some(true);
                        }
                        if operator == BinaryOperator::Equal
                            && work.atoms.len() > base_atoms
                            && goal_only_linear_ops
                        {
                            let diff = left.sub(&right);
                            if (base_atoms..work.atoms.len()).any(|index| {
                                let name = atom_name(index);
                                diff.terms
                                    .keys()
                                    .any(|monomial| monomial.contains_key(&name))
                            }) && !(has_premises && queries_and_contradictory().1)
                            {
                                work.operations.clear();
                                return Some(false);
                            }
                        }
                        if work.carrier_bounds.len() == base_bounds && goal_only_linear_ops {
                            if !has_premises && operator == BinaryOperator::Equal && left != right {
                                work.operations.clear();
                                return Some(false);
                            }
                            let holds = IntegerObservations::comparison_holds(
                                &queries_and_contradictory().0,
                                operator,
                                &left,
                                &right,
                            );
                            if !holds || work.operations.is_empty() {
                                work.operations.clear();
                                return Some(holds);
                            }
                        }
                    }
                    let mut observations = work.clone();
                    work.operations.clear();
                    if work.engine.integer_projection_registrations().1 != base_engine_reg.1 {
                        work = prepared.for_goal();
                    }
                    let operations = 0..observations.operations.len();
                    let goal_has_no_operations = operations.is_empty();
                    // Only this goal has registered since the sealed premise context.
                    // Even an unavailable projection is discarded with this whole fork.
                    if !observations.install_premises(&mut Vec::new())
                        || !observations.admitted(operations)
                    {
                        return None;
                    }
                    let queries = observations.engine.lower_bound_queries();
                    if IntegerObservations::comparison_holds(&queries, operator, &left, &right) {
                        Some(true)
                    } else if goal_has_no_operations {
                        Some(false)
                    } else {
                        None
                    }
                })
                .collect(),
        )
    }

    fn prepare_integer_premises(&self) -> Option<PreparedIntegerPremises<'_>> {
        let mut observations = IntegerObservations {
            engine: Engine::for_explicit_bindings(self.program),
            atoms: Vec::new(),
            operations: Vec::new(),
            carrier_bounds: Vec::new(),
            installed_carrier_bounds: 0,
        };
        let mut premises = self.project_observed_integer_premises(&mut observations);
        if !premises.complete
            || !observations.install_premises(&mut premises.comparisons)
            || !premises.comparisons.is_empty()
            || !observations.admitted(0..observations.operations.len())
        {
            return None;
        }
        // The engine retains the laws of these independently formed operations.
        // A goal starts a fresh admission worklist and cannot refresh another
        // requested goal's provisional denotation or opaque consequences.
        observations.operations.clear();
        Some(PreparedIntegerPremises { observations })
    }

    fn proves_observed_integer_predicate(&self, predicate: &StructuralTerm) -> bool {
        let Some((observations, (operator, left, right))) =
            self.integer_predicate_observations(predicate)
        else {
            return false;
        };
        let queries = observations.engine.lower_bound_queries();
        IntegerObservations::comparison_holds(&queries, operator, &left, &right)
    }

    /// Refutation owes the same admitted operations and captured premises as
    /// proof. An unknown relation is never converted to its opposite.
    fn observed_integer_predicate_judgment(
        &self,
        predicate: &StructuralTerm,
    ) -> StructuralJudgment {
        let Some((observations, comparison)) = self.integer_predicate_observations(predicate)
        else {
            return StructuralJudgment::Unknown;
        };
        let queries = observations.engine.lower_bound_queries();
        IntegerObservations::judge_comparison(&queries, comparison)
    }

    /// Keep one goal-first projection and admission route for positive evidence
    /// and tri-valued consumers. Neither wrapper can skip child formation.
    fn integer_predicate_observations(
        &self,
        predicate: &StructuralTerm,
    ) -> Option<(IntegerObservations<'_>, Comparison)> {
        if !self.runtime_body_values || self.case_facts_only {
            return None;
        }
        let mut observations = IntegerObservations {
            engine: Engine::for_explicit_bindings(self.program),
            atoms: Vec::new(),
            operations: Vec::new(),
            carrier_bounds: Vec::new(),
            installed_carrier_bounds: 0,
        };
        let comparison = self.integer_comparison(predicate, &mut observations)?;
        let goal_operations = 0..observations.operations.len();
        let mut comparisons = self.observed_integer_premises(&mut observations);
        if !observations.install_premises(&mut comparisons)
            || !observations.admitted(goal_operations)
        {
            return None;
        }
        Some((observations, comparison))
    }

    fn observed_integer_premises(
        &self,
        observations: &mut IntegerObservations<'_>,
    ) -> Vec<(Comparison, Range<usize>)> {
        // Preserve the general query's existing successful-premise intake.
        // Only optional sharing requires complete numeric projection.
        self.project_observed_integer_premises(observations)
            .comparisons
    }

    fn project_observed_integer_premises(
        &self,
        observations: &mut IntegerObservations<'_>,
    ) -> ProjectedIntegerPremises {
        let mut comparisons = Vec::new();
        let mut complete = true;
        for (predicate, truth) in self.rewrites.iter() {
            let truth = self.resolve(truth.clone());
            let StructuralTerm::Constructor { data, case, fields } = truth.semantic_value() else {
                // Selected normal guarantees also retain equations between
                // mathematical values, not just predicates equated to true.
                // Read both sides in the same captured-value namespace: a
                // returned scalar and a field containing it have different
                // source type handles but the same observed value identity.
                // Keep each side's operation obligations on this premise so
                // the equation cannot establish its own formation.
                let start = observations.operations.len();
                let before_atoms = observations.atoms.len();
                let before_bounds = observations.carrier_bounds.len();
                let before_param_atoms = observations.engine.parameter_atoms.len();
                let before_engine = observations.engine.integer_projection_registrations();
                if let (Some(left), Some(right)) = (
                    self.mathematical_value(predicate, observations, 0),
                    self.mathematical_value(&truth, observations, 0),
                ) {
                    comparisons.push((
                        (BinaryOperator::Equal, left, right),
                        start..observations.operations.len(),
                    ));
                } else if observations.operations.len() == start
                    && observations.engine.integer_projection_registrations().1 == before_engine.1
                    && observations.engine.integer_projection_registrations().2 == before_engine.2
                    && observations.engine.integer_projection_registrations().3 == before_engine.3
                {
                    observations.atoms.truncate(before_atoms);
                    observations.carrier_bounds.truncate(before_bounds);
                    observations
                        .engine
                        .parameter_atoms
                        .truncate(before_param_atoms);
                } else {
                    complete = false;
                }
                continue;
            };
            if data != "bool" || !fields.is_empty() {
                continue;
            }
            let Some(holds) = language_semantics::const_value::boolean_literal_spelling(case)
            else {
                continue;
            };
            let start = observations.operations.len();
            let before_atoms = observations.atoms.len();
            let before_bounds = observations.carrier_bounds.len();
            let before_param_atoms = observations.engine.parameter_atoms.len();
            let before_engine = observations.engine.integer_projection_registrations();
            let (operator, left, right) =
                match self.project_integer_comparison(predicate, observations) {
                    IntegerComparisonProjection::Comparison(comparison) => comparison,
                    IntegerComparisonProjection::NotInteger => continue,
                    IntegerComparisonProjection::UnavailableOperand => {
                        if observations.operations.len() == start
                            && observations.engine.integer_projection_registrations().1
                                == before_engine.1
                            && observations.engine.integer_projection_registrations().2
                                == before_engine.2
                            && observations.engine.integer_projection_registrations().3
                                == before_engine.3
                        {
                            observations.atoms.truncate(before_atoms);
                            observations.carrier_bounds.truncate(before_bounds);
                            observations
                                .engine
                                .parameter_atoms
                                .truncate(before_param_atoms);
                        } else {
                            complete = false;
                        }
                        continue;
                    }
                };
            comparisons.push((
                (
                    if holds {
                        operator
                    } else {
                        operator.complement().unwrap_or(operator)
                    },
                    left,
                    right,
                ),
                start..observations.operations.len(),
            ));
        }
        ProjectedIntegerPremises {
            comparisons,
            complete,
        }
    }

    /// Prepared calculus captures are rebound to completed values in the same
    /// integer namespace as runtime predicates. Source syntax supplies identity
    /// and carrier only; it cannot reread the caller's storage.
    pub(in crate::proof_contracts::contract_entailment) fn instantiated_mathematical_predicate(
        &self,
        owner: symbols::SymbolHandle,
        parameters: &[typed_trees::signature::StateParameter],
        expression: typed_trees::expression::ExpressionHandle,
        bindings: &[(String, StructuralTerm)],
        machine_bindings: &[(
            symbols::SymbolHandle,
            typed_trees::types::TypeReferenceHandle,
            typed_trees::expression::StaticMachineArgument,
        )],
    ) -> StructuralJudgment {
        if !self.runtime_body_values || self.case_facts_only {
            return StructuralJudgment::Unknown;
        }
        let Some(view) = self.program.prepared_mathematics() else {
            return StructuralJudgment::Unknown;
        };
        let Some(application) = view.application(expression) else {
            return StructuralJudgment::Unknown;
        };
        let mut observations = IntegerObservations {
            engine: Engine::for_explicit_bindings(self.program),
            atoms: Vec::new(),
            operations: Vec::new(),
            carrier_bounds: Vec::new(),
            installed_carrier_bounds: 0,
        };
        for capture in view
            .applications
            .captures
            .span_or_empty(application.captures)
        {
            let Some(value) = super::super::structural_terms::structural_term(
                self.program,
                None,
                Some((owner, parameters)),
                capture.expression,
                None,
            ) else {
                return StructuralJudgment::Unknown;
            };
            let value = StructuralJudge::substitute_call_term(&value, bindings, machine_bindings);
            let value = StructuralTerm::IntegerEmbedding {
                reference: capture.source_type,
                value: Box::new(value),
            };
            let Some(polynomial) = self.mathematical_value(&value, &mut observations, 0) else {
                return StructuralJudgment::Unknown;
            };
            if !observations
                .engine
                .bind_strict_occurrence(capture.expression, polynomial)
            {
                return StructuralJudgment::Unknown;
            }
        }
        let goal_operations = 0..observations.operations.len();
        let mut premises = self.observed_integer_premises(&mut observations);
        if !observations.install_premises(&mut premises) || !observations.admitted(goal_operations)
        {
            return StructuralJudgment::Unknown;
        }
        match observations
            .engine
            .judge_mathematical_application(expression)
        {
            super::super::arithmetic_judgment::Judgment::Proven => StructuralJudgment::Proven,
            super::super::arithmetic_judgment::Judgment::ConstantFalse
            | super::super::arithmetic_judgment::Judgment::Refuted => StructuralJudgment::Refuted,
            super::super::arithmetic_judgment::Judgment::Unknown { .. } => {
                StructuralJudgment::Unknown
            }
        }
    }

    fn integer_comparison(
        &self,
        predicate: &StructuralTerm,
        observations: &mut IntegerObservations<'_>,
    ) -> Option<Comparison> {
        match self.project_integer_comparison(predicate, observations) {
            IntegerComparisonProjection::Comparison(comparison) => Some(comparison),
            IntegerComparisonProjection::NotInteger
            | IntegerComparisonProjection::UnavailableOperand => None,
        }
    }

    fn project_integer_comparison(
        &self,
        predicate: &StructuralTerm,
        observations: &mut IntegerObservations<'_>,
    ) -> IntegerComparisonProjection {
        // Boolean equality to a literal includes negation. Reuse its checked
        // operand meaning before asking the integer owner for a relation;
        // absence of a positive proof is never evidence of the negative.
        let mut predicate = std::borrow::Cow::Borrowed(predicate.semantic_value());
        let mut positive = true;
        for _ in 0..64 {
            let Some((operand, truth)) = self.boolean_equality_operand(
                predicate.semantic_value(),
                &super::value_predicates::boolean_term(positive),
            ) else {
                break;
            };
            positive = truth == super::value_predicates::boolean_term(true);
            predicate = std::borrow::Cow::Owned(operand);
        }
        if let StructuralTerm::MathematicalBinary {
            operator,
            left,
            right,
        } = predicate.semantic_value()
        {
            if !operator.is_comparison() {
                return IntegerComparisonProjection::NotInteger;
            }
            let before = observations.projection_registrations();
            let Some(left) = self.mathematical_value(left, observations, 0) else {
                let after = observations.projection_registrations();
                return if before == after {
                    IntegerComparisonProjection::NotInteger
                } else {
                    IntegerComparisonProjection::UnavailableOperand
                };
            };
            let Some(right) = self.mathematical_value(right, observations, 0) else {
                return IntegerComparisonProjection::UnavailableOperand;
            };
            return IntegerComparisonProjection::Comparison((
                if positive {
                    *operator
                } else {
                    operator.complement().unwrap_or(*operator)
                },
                left,
                right,
            ));
        }
        let StructuralTerm::ScalarBinary {
            references,
            operator,
            meaning,
            left,
            right,
        } = predicate.semantic_value()
        else {
            return IntegerComparisonProjection::NotInteger;
        };
        if !operator.is_comparison()
            || meaning
                .iter()
                .any(|(primitive, _)| !primitive.accepts_integer_literal())
        {
            return IntegerComparisonProjection::NotInteger;
        }
        let [left, right] = [left, right].map(|operand| self.resolve(operand.as_ref().clone()));
        if ![&left, &right]
            .into_iter()
            .zip(references)
            .all(|(operand, reference)| {
                super::super::structural_terms::scalar_operand_is_observed(
                    self.program,
                    *reference,
                    operand,
                )
            })
        {
            return IntegerComparisonProjection::NotInteger;
        }
        let Some(left) = self.integer_value(&left, meaning[0].0, observations, 0) else {
            return IntegerComparisonProjection::UnavailableOperand;
        };
        let Some(right) = self.integer_value(&right, meaning[1].0, observations, 0) else {
            return IntegerComparisonProjection::UnavailableOperand;
        };
        IntegerComparisonProjection::Comparison((
            if positive {
                *operator
            } else {
                operator.complement().unwrap_or(*operator)
            },
            left,
            right,
        ))
    }

    fn integer_value(
        &self,
        value: &StructuralTerm,
        primitive: PrimitiveType,
        observations: &mut IntegerObservations<'_>,
        depth: usize,
    ) -> Option<Polynomial> {
        if depth >= 64 {
            return None;
        }
        // A completed operand may be described by a public computed-result
        // equation. Preserve its captured leaves before normal arithmetic
        // resolution; no caller expression or private body supplies this value.
        let observed_carrier = match value {
            StructuralTerm::ObservedValue { observed_type, .. } => {
                self.program.primitive_type_reference(*observed_type) == Some(primitive)
            }
            _ => false,
        };
        let value = self.resolve(self.completed_value_identity(value.clone()));
        let value = value.semantic_value();
        if let StructuralTerm::Integer(integer) = value {
            let value = Polynomial::constant(integer.clone());
            // Folding an independently formed operand cannot make it
            // anonymous again. Contextual literal destinations carry no such
            // observation, nor does the operation whose fit is being queried.
            if observed_carrier {
                observations.install_observed_carrier_bounds(primitive, &value)?;
            }
            return Some(value);
        }
        // A completed scalar observation can have a recorded mathematical
        // denotation (for example a window's end minus start). Keep that
        // equation visible to the existing polynomial owner. Executable
        // operands embedded inside it still owe their original formation.
        if matches!(value, StructuralTerm::MathematicalBinary { .. }) {
            return self.mathematical_value(value, observations, depth + 1);
        }
        if let StructuralTerm::ScalarBinary {
            references,
            operator,
            meaning,
            left,
            right,
        } = value
            && Engine::integer_operation_meaning(*operator, *meaning)
        {
            if ![left, right]
                .into_iter()
                .zip(references)
                .all(|(operand, reference)| {
                    super::super::structural_terms::scalar_operand_is_observed(
                        self.program,
                        *reference,
                        operand,
                    )
                })
            {
                return None;
            }
            let start = observations.operations.len();
            // Exact conversions retain this computation's mathematical value.
            // Its formation still belongs to its original selected carriers,
            // independently of the carrier observing the completed result.
            let left = self.integer_value(left, meaning[0].0, observations, depth + 1)?;
            let right = self.integer_value(right, meaning[1].0, observations, depth + 1)?;
            let value = observations.engine.selected_integer_binary_value(
                *operator,
                *meaning,
                left.clone(),
                right.clone(),
            )?;
            observations.operations.push(ObservedOperation {
                operator: *operator,
                primitives: meaning.map(|(primitive, _)| primitive),
                operands: [left, right],
                value: value.clone(),
                children: start..observations.operations.len(),
                admitted: false,
            });
            return Some(value);
        }
        let position =
            if let Some(position) = observations.atoms.iter().position(|atom| atom == value) {
                position
            } else {
                observations.atoms.push(value.clone());
                let position = observations.atoms.len() - 1;
                observations
                    .engine
                    .parameter_atoms
                    .push(atom_name(position));
                position
            };
        let value = Polynomial::atom(atom_name(position));
        // A complete observed value has its selected operand's carrier. These
        // intrinsic bounds apply only to opaque leaves, never to a computation
        // waiting to prove formation or to an anonymous integer literal.
        observations.install_observed_carrier_bounds(primitive, &value)?;
        Some(value)
    }

    fn mathematical_value(
        &self,
        value: &StructuralTerm,
        observations: &mut IntegerObservations<'_>,
        depth: usize,
    ) -> Option<Polynomial> {
        if depth >= 64 {
            return None;
        }
        // Keep mathematical arithmetic visible to the polynomial owner. A
        // rewrite such as base + length = end may replace the complete sum
        // with end; doing that before collecting its operands also erases the
        // equation that connects end to the nonnegative length. Captured
        // scalar operands still resolve normally, with formation checked below.
        let value = match value.semantic_value() {
            StructuralTerm::MathematicalBinary { .. } => value.clone(),
            _ => self.resolve(value.clone()),
        };
        match value.semantic_value() {
            StructuralTerm::Integer(value) => Some(Polynomial::constant(value.clone())),
            // Authenticated descriptor metadata is an observed u64 quantity,
            // independently of the contents behind the descriptor.
            StructuralTerm::CollectionMeasure { .. } => {
                self.integer_value(&value, PrimitiveType::U64, observations, depth + 1)
            }
            StructuralTerm::BoundValue(symbol)
                if self.program.symbols.builtin_type_member(*symbol)
                    == Some(symbols::BuiltinTypeMember::AddressBound) =>
            {
                observations.engine.address_model_bound()
            }
            StructuralTerm::IntegerEmbedding { reference, value } => {
                let (primitive, _) = super::super::structural_terms::scalar_operand_meaning(
                    self.program,
                    *reference,
                    value,
                )?;
                self.integer_value(value, primitive, observations, depth + 1)
            }
            StructuralTerm::MathematicalBinary {
                operator,
                left,
                right,
            } => {
                let left = self.mathematical_value(left, observations, depth + 1)?;
                let right = self.mathematical_value(right, observations, depth + 1)?;
                match operator {
                    BinaryOperator::Add => Some(left.add(&right)),
                    BinaryOperator::Subtract => Some(left.sub(&right)),
                    BinaryOperator::Multiply => left.checked_mul(&right),
                    // Quotient and remainder need their own nonzero formation
                    // evidence. They remain opaque, never cancellation laws.
                    _ => None,
                }
            }
            _ => None,
        }
    }
}

fn atom_name(position: usize) -> String {
    format!("\0captured-integer:{position}")
}
