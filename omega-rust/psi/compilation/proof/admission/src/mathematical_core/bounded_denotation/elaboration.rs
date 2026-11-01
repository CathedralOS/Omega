use super::*;
use equality_rules::predicate_denotation_relation;
use integer_order_rules::math_order_translation;
use integer_order_rules::order_antisymmetry;

/// The denotation of one proof tree, tracking the scoped premise roster
/// exactly as `accept_certificate`'s traversal does: `roster` holds the
/// ambient assumptions plus the hypotheses implication and case-analysis
/// rules push and pop, `positions` records the de Bruijn binder each
/// roster entry's evidence binds at, and `depth` counts every enclosing
/// binder — including the intermediate ones a nested `caseTwo` adds that
/// are not themselves cited premises.
pub(super) struct Elaboration<'a> {
    pub(super) denotation: Denotation,
    context: &'a PropositionContext,
    /// The ambient assumption roster — the shared checkers read it for
    /// the citations the bound rules record.
    ambient_assumptions: &'a [Proposition],
    axioms: &'a [Proposition],
    /// The verifier-reconstructed scalar signature roots the
    /// parameter-custody witness rules check against.
    machine_parameter_values: &'a BTreeSet<ValueId>,
    /// Ambient assumption count — citations at or beyond it name
    /// discharged branch hypotheses, never recorded premises.
    ambient: usize,
    context_bindings: Vec<TermHandle>,
    expected: TermHandle,
    roster: Vec<Proposition>,
    positions: Vec<u32>,
    depth: u32,
    steps: u64,
    rules: BTreeSet<AcceptedProofRule>,
    assumptions: PremiseRoster,
    semantic_axioms: PremiseRoster,
}

macro_rules! certificate {
    ($error:expr) => {
        BoundedDenotationError::Certificate($error)
    };
}

mod walk;

impl<'a> Elaboration<'a> {
    pub(super) fn new(
        context: &'a PropositionContext,
        goal: &Proposition,
        assumptions: &'a [Proposition],
        semantic_axioms: &'a [Proposition],
        machine_parameter_values: &'a BTreeSet<ValueId>,
    ) -> Result<Self, BoundedDenotationError> {
        let mut denotation = Denotation::new();
        let mut context_bindings = Vec::with_capacity(assumptions.len() + semantic_axioms.len());
        for proposition in assumptions.iter().chain(semantic_axioms) {
            context_bindings.push(denotation.denote(proposition)?);
        }
        let expected = denotation.denote(goal)?;
        let depth = u32::try_from(assumptions.len() + semantic_axioms.len())
            .map_err(|_| BoundedDenotationError::DepthLimitExceeded)?;
        let positions = (0..assumptions.len())
            .map(|index| u32::try_from(index).expect("roster position fits the context depth"))
            .collect();
        Ok(Elaboration {
            denotation,
            context,
            ambient_assumptions: assumptions,
            axioms: semantic_axioms,
            machine_parameter_values,
            ambient: assumptions.len(),
            context_bindings,
            expected,
            roster: assumptions.to_vec(),
            positions,
            depth,
            steps: 0,
            rules: BTreeSet::new(),
            assumptions: PremiseRoster::default(),
            semantic_axioms: PremiseRoster::default(),
        })
    }

    /// The variable naming the premise bound at binder position
    /// `position` — de Bruijn `depth - 1 - position` at the current depth.
    fn variable(&mut self, position: u32) -> TermHandle {
        let index = self.depth - 1 - position;
        self.denotation.arena.insert(Term::Variable(index))
    }

    pub(super) fn node(&mut self, proof: &ProofNode) -> Result<TermHandle, BoundedDenotationError> {
        walk::elaborate(self, proof)
    }

    /// Reconstruct one ordinary rule from its exact, already elaborated child
    /// occurrences. Shared premise/conclusion checks stay in their original
    /// family owners and run at the same post-child point.
    fn finish_regular(
        &mut self,
        proof: &ProofNode,
        operands: &mut walk::ChildEvidence<'_>,
    ) -> Result<TermHandle, BoundedDenotationError> {
        match &proof.rule {
            ProofRule::Primitive(judgment) => {
                decide_primitive(self.context, &proof.conclusion, *judgment)
                    .map_err(|error| certificate!(ProofError::PrimitiveJudgment(error)))?;
                self.rules.insert(AcceptedProofRule::Primitive);
                let denoted = self.denotation.denote(&proof.conclusion)?;
                // Reuse the checked inhabitants used by predicate conversion:
                // reflexive identities close by refl and closed orders derive
                // from the fixed numeral laws. Only unsupported leaves retain
                // the explicit decision assumption; a closed order must not
                // assume the conclusion its numeral derivation establishes.
                if let Some(evidence) = self.denotation.closed_inhabitant(denoted)? {
                    return Ok(evidence);
                }
                self.denotation.decision(denoted)
            }
            ProofRule::Assumption { index } => self.cited(*index, &proof.conclusion),
            ProofRule::SemanticAxiom { index } => {
                let Some(axiom) = self.semantic_axiom(*index) else {
                    return Err(certificate!(ProofError::UnknownSemanticAxiom(*index)));
                };
                if !propositions_match(&axiom, &proof.conclusion) {
                    return Err(certificate!(ProofError::SemanticAxiomConclusionMismatch(
                        *index
                    )));
                }
                self.same_denotation(&axiom, &proof.conclusion)?;
                self.rules.insert(AcceptedProofRule::SemanticAxiom);
                self.semantic_axioms.record(*index, &axiom);
                let position = self.ambient + *index;
                let position = u32::try_from(position)
                    .map_err(|_| BoundedDenotationError::DepthLimitExceeded)?;
                Ok(self.variable(position))
            }
            ProofRule::ConjunctionIntroduction(children) => {
                let Proposition::Conjunction(expected) = &proof.conclusion else {
                    unreachable!("conjunction shape admitted before its children")
                };
                let mut terms = Vec::with_capacity(children.len());
                for child in children {
                    terms.push(operands.take(child));
                }
                for (child, expected) in children.iter().zip(expected.iter()) {
                    if &child.conclusion != expected {
                        return Err(certificate!(ProofError::ConjunctConclusionMismatch));
                    }
                }
                self.rules
                    .insert(AcceptedProofRule::ConjunctionIntroduction);
                let mut term = terms.pop().expect("canonical conjunction arity");
                while let Some(first) = terms.pop() {
                    term = self.denotation.arena.insert(Term::Pair {
                        first,
                        second: term,
                    });
                }
                Ok(term)
            }
            ProofRule::ConjunctionElimination {
                conjunction,
                conjunct,
            } => {
                let pair = operands.take(conjunction);
                let Proposition::Conjunction(conjuncts) = &conjunction.conclusion else {
                    return Err(certificate!(ProofError::RulePremiseMismatch(
                        "conjunction elimination"
                    )));
                };
                let selected = conjuncts
                    .get(*conjunct)
                    .ok_or(certificate!(ProofError::UnknownConjunct(*conjunct)))?;
                if *selected != proof.conclusion {
                    return Err(certificate!(ProofError::ConjunctConclusionMismatch));
                }
                self.rules.insert(AcceptedProofRule::ConjunctionElimination);
                // In `Σ(d₀, Σ(d₁, … dₙ₋₁))` conjunct `i` is `fst` after
                // `i` `snd`s — except the last, which is the bare
                // rightmost component.
                let last = conjuncts.len() - 1;
                let mut term = pair;
                for _ in 0..(*conjunct).min(last) {
                    term = self.denotation.arena.insert(Term::Snd { pair: term });
                }
                if *conjunct < last {
                    term = self.denotation.arena.insert(Term::Fst { pair: term });
                }
                Ok(term)
            }
            ProofRule::DisjunctionIntroduction { disjunct, index } => {
                let Proposition::Disjunction(disjuncts) = &proof.conclusion else {
                    unreachable!("disjunction shape and index admitted before the child")
                };
                let disjunct_term = operands.take(disjunct);
                if disjuncts[*index] != disjunct.conclusion {
                    return Err(certificate!(ProofError::DisjunctConclusionMismatch));
                }
                self.rules
                    .insert(AcceptedProofRule::DisjunctionIntroduction);
                Ok(self.disjunction_intro(disjuncts.len(), *index, disjunct_term))
            }
            ProofRule::DisjunctionElimination {
                disjunction: _,
                branches: _,
            } => {
                unreachable!("disjunction elimination is owned by scoped case continuation frames")
            }
            ProofRule::ImplicationIntroduction { body: _ } => {
                unreachable!("implication introduction is owned by scoped continuation frames")
            }
            ProofRule::ImplicationElimination {
                implication,
                premise,
            } => {
                let function = operands.take(implication);
                let argument = operands.take(premise);
                let Proposition::Implication {
                    premise: required,
                    conclusion,
                } = &implication.conclusion
                else {
                    return Err(certificate!(ProofError::RulePremiseMismatch(
                        "implication elimination"
                    )));
                };
                if premise.conclusion != **required {
                    return Err(certificate!(ProofError::ImplicationPremiseMismatch));
                }
                if &proof.conclusion != conclusion.as_ref() {
                    return Err(certificate!(ProofError::ImplicationConclusionMismatch));
                }
                self.rules.insert(AcceptedProofRule::ImplicationElimination);
                Ok(self
                    .denotation
                    .arena
                    .insert(Term::Apply { function, argument }))
            }
            ProofRule::EqualitySymmetry { equality } => {
                let proof_term = operands.take(equality);
                let Proposition::Equal(left, right) = &equality.conclusion else {
                    return Err(certificate!(ProofError::RulePremiseMismatch(
                        "equality symmetry"
                    )));
                };
                if proof.conclusion != Proposition::Equal(right.clone(), left.clone()) {
                    return Err(certificate!(ProofError::EqualityConclusionMismatch));
                }
                self.rules.insert(AcceptedProofRule::EqualitySymmetry);
                let premise_ty = self.denotation.denote(&equality.conclusion)?;
                let goal_ty = self.denotation.denote(&proof.conclusion)?;
                match self
                    .denotation
                    .symmetry_evidence(premise_ty, goal_ty, proof_term)
                {
                    Some(term) => Ok(term),
                    None => self.rule_instance(
                        AcceptedProofRule::EqualitySymmetry,
                        vec![equality.conclusion.clone()],
                        vec![proof_term],
                        &proof.conclusion,
                    ),
                }
            }
            ProofRule::EqualityTransitivity {
                left_equals_middle,
                middle_equals_right,
            } => {
                let first = operands.take(left_equals_middle);
                let second = operands.take(middle_equals_right);
                equality_rules::equality_transitivity_relation(
                    &left_equals_middle.conclusion,
                    &middle_equals_right.conclusion,
                    &proof.conclusion,
                )
                .map_err(BoundedDenotationError::Certificate)?;
                self.rules.insert(AcceptedProofRule::EqualityTransitivity);
                let first_ty = self.denotation.denote(&left_equals_middle.conclusion)?;
                let second_ty = self.denotation.denote(&middle_equals_right.conclusion)?;
                let goal_ty = self.denotation.denote(&proof.conclusion)?;
                match self
                    .denotation
                    .transitivity_evidence(first_ty, first, second_ty, second, goal_ty)
                {
                    Some(term) => Ok(term),
                    // Every transitivity instance the shared check
                    // accepts denotes `Id` — `Equal`/`IntegerMathEqual`
                    // and `ContentConservation` alike — so `None` is a
                    // defensive fallback for a hypothetical denotation
                    // outside the identity vocabulary, not a live path.
                    None => self.rule_instance(
                        AcceptedProofRule::EqualityTransitivity,
                        vec![
                            left_equals_middle.conclusion.clone(),
                            middle_equals_right.conclusion.clone(),
                        ],
                        vec![first, second],
                        &proof.conclusion,
                    ),
                }
            }
            ProofRule::PredicateDenotation { premise } => {
                let evidence = operands.take(premise);
                predicate_denotation_relation(self.context, &premise.conclusion, &proof.conclusion)
                    .map_err(BoundedDenotationError::Certificate)?;
                self.rules.insert(AcceptedProofRule::PredicateDenotation);
                let premise_ty = self.denotation.denote(&premise.conclusion)?;
                let goal_ty = self.denotation.denote(&proof.conclusion)?;
                // Both propositions reach the same normalized denotation
                // goal; when their terms already agree — a canonical
                // `Equal`/`IntegerMathEqual` pair — the premise evidence
                // inhabits the goal with no axiom at all. A reversal nested
                // inside a connective converts through nested `J`s instead,
                // and the closed Truth/Falsehood crossing derives its
                // inhabitant or refutes its premise through `Empty`.
                if let Some(term) = self
                    .denotation
                    .oriented_evidence(premise_ty, goal_ty, evidence)?
                {
                    return Ok(term);
                }
                self.rule_instance(
                    AcceptedProofRule::PredicateDenotation,
                    vec![premise.conclusion.clone()],
                    vec![evidence],
                    &proof.conclusion,
                )
            }
            ProofRule::ValueEqualityTransport {
                premise,
                equalities,
            } => {
                let premises = operands.take_prepared_premises();
                let mut evidence = Vec::with_capacity(equalities.len() + 1);
                evidence.push(operands.take(premise));
                for equality in equalities {
                    evidence.push(operands.take(equality));
                }
                equality_rules::value_equality_transport_relation(
                    self.context,
                    &premise.conclusion,
                    equalities.iter().map(|equality| &equality.conclusion),
                    &proof.conclusion,
                )
                .map_err(BoundedDenotationError::Certificate)?;
                self.rules.insert(AcceptedProofRule::ValueEqualityTransport);
                if let Some(term) = self.denotation.compositional_transport(
                    &premise.conclusion,
                    evidence[0],
                    equalities,
                    &evidence[1..],
                    &proof.conclusion,
                )? {
                    return Ok(term);
                }
                self.rule_instance(
                    AcceptedProofRule::ValueEqualityTransport,
                    premises,
                    evidence,
                    &proof.conclusion,
                )
            }
            ProofRule::IntegerSubtractOrder {
                difference,
                positive,
            } => {
                let difference_evidence = operands.take(difference);
                let positive_evidence = operands.take(positive);
                subtract_order::check(
                    &difference.conclusion,
                    &positive.conclusion,
                    &proof.conclusion,
                )
                .map_err(BoundedDenotationError::Certificate)?;
                self.rules.insert(AcceptedProofRule::IntegerSubtractOrder);
                self.denotation.subtraction_evidence(
                    &difference.conclusion,
                    difference_evidence,
                    positive_evidence,
                    &proof.conclusion,
                )
            }
            ProofRule::IntegerAddOrder { sum, positive } => {
                let sum_evidence = operands.take(sum);
                let positive_evidence = operands.take(positive);
                add_order::check(&sum.conclusion, &positive.conclusion, &proof.conclusion)
                    .map_err(BoundedDenotationError::Certificate)?;
                self.rules.insert(AcceptedProofRule::IntegerAddOrder);
                match self.denotation.addition_order_evidence(
                    &sum.conclusion,
                    sum_evidence,
                    positive_evidence,
                )? {
                    Some(term) => Ok(term),
                    None => self.rule_instance(
                        AcceptedProofRule::IntegerAddOrder,
                        vec![sum.conclusion.clone(), positive.conclusion.clone()],
                        vec![sum_evidence, positive_evidence],
                        &proof.conclusion,
                    ),
                }
            }
            ProofRule::IntegerSubtractAntitone {
                smaller,
                larger,
                order,
            } => {
                let smaller_evidence = operands.take(smaller);
                let larger_evidence = operands.take(larger);
                let order_evidence = operands.take(order);
                subtract_antitone::check(
                    &smaller.conclusion,
                    &larger.conclusion,
                    &order.conclusion,
                    &proof.conclusion,
                )
                .map_err(BoundedDenotationError::Certificate)?;
                self.rules
                    .insert(AcceptedProofRule::IntegerSubtractAntitone);
                match self.denotation.subtraction_antitone_evidence(
                    &smaller.conclusion,
                    smaller_evidence,
                    &larger.conclusion,
                    larger_evidence,
                    order_evidence,
                )? {
                    Some(term) => Ok(term),
                    None => self.rule_instance(
                        AcceptedProofRule::IntegerSubtractAntitone,
                        vec![
                            smaller.conclusion.clone(),
                            larger.conclusion.clone(),
                            order.conclusion.clone(),
                        ],
                        vec![smaller_evidence, larger_evidence, order_evidence],
                        &proof.conclusion,
                    ),
                }
            }
            ProofRule::IntegerOrderDiscreteness { relation } => {
                let evidence = operands.take(relation);
                order_discreteness::check(&relation.conclusion, &proof.conclusion)
                    .map_err(BoundedDenotationError::Certificate)?;
                self.rules
                    .insert(AcceptedProofRule::IntegerOrderDiscreteness);
                self.denotation.discreteness_evidence(
                    &relation.conclusion,
                    evidence,
                    &proof.conclusion,
                )
            }
            ProofRule::IntegerOrderWeakening { relation } => {
                let evidence = operands.take(relation);
                integer_order_rules::order_weakening(&relation.conclusion, &proof.conclusion)
                    .map_err(BoundedDenotationError::Certificate)?;
                self.rules.insert(AcceptedProofRule::IntegerOrderWeakening);
                let premise_ty = self.denotation.denote(&relation.conclusion)?;
                let goal_ty = self.denotation.denote(&proof.conclusion)?;
                match self
                    .denotation
                    .weakening_evidence(premise_ty, evidence, goal_ty)?
                {
                    Some(term) => Ok(term),
                    None => self.rule_instance(
                        AcceptedProofRule::IntegerOrderWeakening,
                        vec![relation.conclusion.clone()],
                        vec![evidence],
                        &proof.conclusion,
                    ),
                }
            }
            ProofRule::IntegerLessOrEqualTransitivity {
                left_less_or_equal_middle,
                middle_less_or_equal_right,
            } => {
                let first = operands.take(left_less_or_equal_middle);
                let second = operands.take(middle_less_or_equal_right);
                integer_order_rules::less_or_equal_transitivity(
                    &left_less_or_equal_middle.conclusion,
                    &middle_less_or_equal_right.conclusion,
                    &proof.conclusion,
                )
                .map_err(BoundedDenotationError::Certificate)?;
                self.rules
                    .insert(AcceptedProofRule::IntegerLessOrEqualTransitivity);
                let first_ty = self
                    .denotation
                    .denote(&left_less_or_equal_middle.conclusion)?;
                let second_ty = self
                    .denotation
                    .denote(&middle_less_or_equal_right.conclusion)?;
                let goal_ty = self.denotation.denote(&proof.conclusion)?;
                match self
                    .denotation
                    .order_transitivity_evidence(first_ty, first, second_ty, second, goal_ty)?
                {
                    Some(term) => Ok(term),
                    None => self.rule_instance(
                        AcceptedProofRule::IntegerLessOrEqualTransitivity,
                        vec![
                            left_less_or_equal_middle.conclusion.clone(),
                            middle_less_or_equal_right.conclusion.clone(),
                        ],
                        vec![first, second],
                        &proof.conclusion,
                    ),
                }
            }
            ProofRule::IntegerStrictOrderTransitivity {
                left_to_middle,
                middle_to_right,
            } => {
                let first = operands.take(left_to_middle);
                let second = operands.take(middle_to_right);
                strict_order_transitivity::check(
                    &left_to_middle.conclusion,
                    &middle_to_right.conclusion,
                    &proof.conclusion,
                )
                .map_err(BoundedDenotationError::Certificate)?;
                self.rules
                    .insert(AcceptedProofRule::IntegerStrictOrderTransitivity);
                let first_ty = self.denotation.denote(&left_to_middle.conclusion)?;
                let second_ty = self.denotation.denote(&middle_to_right.conclusion)?;
                let goal_ty = self.denotation.denote(&proof.conclusion)?;
                match self
                    .denotation
                    .order_transitivity_evidence(first_ty, first, second_ty, second, goal_ty)?
                {
                    Some(term) => Ok(term),
                    None => self.rule_instance(
                        AcceptedProofRule::IntegerStrictOrderTransitivity,
                        vec![
                            left_to_middle.conclusion.clone(),
                            middle_to_right.conclusion.clone(),
                        ],
                        vec![first, second],
                        &proof.conclusion,
                    ),
                }
            }
            ProofRule::IntegerOrderAntisymmetry { forward, backward } => {
                let forward_evidence = operands.take(forward);
                let backward_evidence = operands.take(backward);
                order_antisymmetry(&forward.conclusion, &backward.conclusion, &proof.conclusion)
                    .map_err(BoundedDenotationError::Certificate)?;
                self.rules
                    .insert(AcceptedProofRule::IntegerOrderAntisymmetry);
                let Proposition::LessOrEqual(left, right) = &forward.conclusion else {
                    unreachable!("checked antisymmetry order");
                };
                let conclusion = self.denotation.denote(&proof.conclusion)?;
                self.denotation.order_antisymmetry_evidence(
                    left,
                    right,
                    forward_evidence,
                    backward_evidence,
                    conclusion,
                )
            }
            ProofRule::IntegerMathOrderTranslation { relation, addend } => {
                let evidence = operands.take(relation);
                math_order_translation(&relation.conclusion, addend, &proof.conclusion)
                    .map_err(BoundedDenotationError::Certificate)?;
                self.rules
                    .insert(AcceptedProofRule::IntegerMathOrderTranslation);
                self.denotation.mathematical_order_translation(
                    &relation.conclusion,
                    addend,
                    evidence,
                )
            }
            ProofRule::IntegerOrderSubstitution {
                relation,
                equality,
                endpoint,
            } => {
                let relation_evidence = operands.take(relation);
                let equality_evidence = operands.take(equality);
                integer_order_rules::endpoint_substitution(
                    &relation.conclusion,
                    &equality.conclusion,
                    *endpoint,
                    &proof.conclusion,
                )
                .map_err(BoundedDenotationError::Certificate)?;
                self.rules
                    .insert(AcceptedProofRule::IntegerOrderSubstitution);
                let relation_ty = self.denotation.denote(&relation.conclusion)?;
                let equality_ty = self.denotation.denote(&equality.conclusion)?;
                let goal_ty = self.denotation.denote(&proof.conclusion)?;
                match self.denotation.order_substitution_evidence(
                    relation_ty,
                    relation_evidence,
                    equality_ty,
                    equality_evidence,
                    *endpoint,
                    goal_ty,
                )? {
                    Some(term) => Ok(term),
                    None => self.rule_instance(
                        AcceptedProofRule::IntegerOrderSubstitution,
                        vec![relation.conclusion.clone(), equality.conclusion.clone()],
                        vec![relation_evidence, equality_evidence],
                        &proof.conclusion,
                    ),
                }
            }
            ProofRule::IntegerAffineBound {
                root_bound,
                witness,
            } => {
                let root = operands.take(root_bound);
                let cited = integer_bound_rules::affine_bound_relation(
                    self.context,
                    self.axioms,
                    &root_bound.conclusion,
                    witness,
                    &proof.conclusion,
                )
                .map_err(BoundedDenotationError::Certificate)?;
                if let Some(evidence) = self.denotation.unsigned_subtract_bound_evidence(
                    &root_bound.conclusion,
                    root,
                    witness,
                    &proof.conclusion,
                )? {
                    self.rules.insert(AcceptedProofRule::IntegerAffineBound);
                    return Ok(evidence);
                }
                let mut definitions = Vec::with_capacity(cited.len());
                for &index in &cited {
                    definitions.push(self.cited_axiom(index)?);
                }
                if let Some(evidence) = self.denotation.correlated_subtract_bound_evidence(
                    &root_bound.conclusion,
                    root,
                    witness,
                    &proof.conclusion,
                    &definitions,
                )? {
                    self.rules.insert(AcceptedProofRule::IntegerAffineBound);
                    return Ok(evidence);
                }
                if let Some(evidence) = self.denotation.correlated_add_bound_evidence(
                    &root_bound.conclusion,
                    root,
                    witness,
                    &proof.conclusion,
                    &definitions,
                )? {
                    self.rules.insert(AcceptedProofRule::IntegerAffineBound);
                    return Ok(evidence);
                }
                if let Some(evidence) = self.denotation.correlated_multiply_bound_evidence(
                    &root_bound.conclusion,
                    root,
                    witness,
                    &proof.conclusion,
                    &definitions,
                )? {
                    self.rules.insert(AcceptedProofRule::IntegerAffineBound);
                    return Ok(evidence);
                }
                if let Some(evidence) = self.denotation.direct_add_bound_evidence(
                    &root_bound.conclusion,
                    root,
                    witness,
                    &proof.conclusion,
                )? {
                    self.rules.insert(AcceptedProofRule::IntegerAffineBound);
                    return Ok(evidence);
                }
                if let Some(evidence) = self.denotation.direct_subtract_bound_evidence(
                    &root_bound.conclusion,
                    root,
                    witness,
                    &proof.conclusion,
                )? {
                    self.rules.insert(AcceptedProofRule::IntegerAffineBound);
                    return Ok(evidence);
                }
                let mut premises = Vec::with_capacity(cited.len() + 1);
                let mut evidence = Vec::with_capacity(cited.len() + 1);
                premises.push(root_bound.conclusion.clone());
                evidence.push(root);
                for (proposition, variable) in definitions {
                    premises.push(proposition);
                    evidence.push(variable);
                }
                self.rule_instance(
                    AcceptedProofRule::IntegerAffineBound,
                    premises,
                    evidence,
                    &proof.conclusion,
                )
            }
            ProofRule::IntegerExactAddDefinitionBound {
                left_bound,
                right_bound,
                definition_axiom,
            }
            | ProofRule::IntegerExactSubtractDefinitionBound {
                left_bound,
                right_bound,
                definition_axiom,
            } => {
                let (operation, rule) = match &proof.rule {
                    ProofRule::IntegerExactAddDefinitionBound { .. } => (
                        integer_bound_rules::ExactDefinitionOperation::Add,
                        AcceptedProofRule::IntegerExactAddDefinitionBound,
                    ),
                    _ => (
                        integer_bound_rules::ExactDefinitionOperation::Subtract,
                        AcceptedProofRule::IntegerExactSubtractDefinitionBound,
                    ),
                };
                let left = operands.take(left_bound);
                let right = operands.take(right_bound);
                integer_bound_rules::exact_definition_bound_relation(
                    self.context,
                    self.axioms,
                    &left_bound.conclusion,
                    &right_bound.conclusion,
                    *definition_axiom,
                    &proof.conclusion,
                    operation,
                )
                .map_err(BoundedDenotationError::Certificate)?;
                let (definition, variable) = self.cited_axiom(*definition_axiom)?;
                let derived = match operation {
                    integer_bound_rules::ExactDefinitionOperation::Add => {
                        self.denotation.exact_add_definition_bound_evidence(
                            &left_bound.conclusion,
                            left,
                            &right_bound.conclusion,
                            right,
                            &definition,
                            variable,
                            &proof.conclusion,
                        )?
                    }
                    integer_bound_rules::ExactDefinitionOperation::Subtract => {
                        self.denotation.exact_subtract_definition_bound_evidence(
                            &left_bound.conclusion,
                            left,
                            &right_bound.conclusion,
                            right,
                            &definition,
                            variable,
                            &proof.conclusion,
                        )?
                    }
                };
                if let Some(evidence) = derived {
                    self.rules.insert(rule);
                    return Ok(evidence);
                }
                self.rule_instance(
                    rule,
                    vec![
                        left_bound.conclusion.clone(),
                        right_bound.conclusion.clone(),
                        definition,
                    ],
                    vec![left, right, variable],
                    &proof.conclusion,
                )
            }
            ProofRule::IntegerCastBound {
                root_bound,
                witness,
            } => {
                let root = operands.take(root_bound);
                let (citations, chain) = integer_bound_rules::cast_bound_relation(
                    self.context,
                    self.axioms,
                    &root_bound.conclusion,
                    witness,
                    &proof.conclusion,
                )
                .map_err(BoundedDenotationError::Certificate)?;
                let mut premises = Vec::with_capacity(citations.len() + 1);
                let mut evidence = Vec::with_capacity(citations.len() + 1);
                premises.push(root_bound.conclusion.clone());
                evidence.push(root);
                for &index in &citations {
                    let (proposition, variable) = self.cited_axiom(index)?;
                    premises.push(proposition);
                    evidence.push(variable);
                }
                if let Some(derived) = self.denotation.cast_bound_evidence(
                    &root_bound.conclusion,
                    evidence[0],
                    &chain,
                    &evidence[1..],
                    self.axioms,
                    &proof.conclusion,
                )? {
                    self.rules.insert(AcceptedProofRule::IntegerCastBound);
                    return Ok(derived);
                }
                self.rule_instance(
                    AcceptedProofRule::IntegerCastBound,
                    premises,
                    evidence,
                    &proof.conclusion,
                )
            }
            ProofRule::IntegerCorrelatedForbiddenRoots { witness } => {
                let (citations, checked) =
                    integer_bound_rules::correlated_forbidden_roots_relation(
                        self.context,
                        self.ambient_assumptions,
                        self.axioms,
                        self.machine_parameter_values,
                        witness,
                        &proof.conclusion,
                    )
                    .map_err(BoundedDenotationError::Certificate)?;
                let mut premises = Vec::with_capacity(
                    citations.semantic_axioms.len() + citations.assumptions.len(),
                );
                let mut evidence = Vec::with_capacity(premises.capacity());
                let mut axiom_evidence = Vec::with_capacity(citations.semantic_axioms.len());
                for index in citations.semantic_axioms {
                    let (proposition, variable) = self.cited_axiom(index)?;
                    axiom_evidence.push((proposition.clone(), variable));
                    premises.push(proposition);
                    evidence.push(variable);
                }
                let mut bound_evidence = Vec::with_capacity(citations.assumptions.len());
                for index in citations.assumptions {
                    let (proposition, variable) = self.ambient_premise(index)?;
                    bound_evidence.push((proposition.clone(), variable));
                    premises.push(proposition);
                    evidence.push(variable);
                }
                if let Some(derived) = self.denotation.correlated_forbidden_roots_evidence(
                    witness,
                    &checked,
                    &axiom_evidence,
                    &bound_evidence,
                    &proof.conclusion,
                )? {
                    self.rules
                        .insert(AcceptedProofRule::IntegerCorrelatedForbiddenRoots);
                    return Ok(derived);
                }
                self.rule_instance(
                    AcceptedProofRule::IntegerCorrelatedForbiddenRoots,
                    premises,
                    evidence,
                    &proof.conclusion,
                )
            }
        }
    }

    /// A premise citation: `Assumption{index}` names `roster[index]`,
    /// which may be an ambient assumption or a discharged branch
    /// hypothesis. The bounded matcher's lift/lower normalization is
    /// honored exactly: `Equal`/`IntegerMathEqual` and the order
    /// relations denote the same `Int`-vocabulary term either side of
    /// the crossing, so a normalized citation binds its premise's own
    /// evidence.
    fn cited(
        &mut self,
        index: usize,
        conclusion: &Proposition,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let Some(assumption) = self.roster.get(index).cloned() else {
            return Err(certificate!(ProofError::UnknownAssumption(index)));
        };
        if !propositions_match(&assumption, conclusion) {
            return Err(certificate!(ProofError::AssumptionConclusionMismatch(
                index
            )));
        }
        self.same_denotation(&assumption, conclusion)?;
        self.rules.insert(AcceptedProofRule::Assumption);
        if index < self.ambient {
            self.assumptions.record(index, &assumption);
        }
        let position = self.positions[index];
        Ok(self.variable(position))
    }

    fn semantic_axiom(&self, index: usize) -> Option<Proposition> {
        self.axioms.get(index).cloned()
    }

    /// Citation pre-check for the normalized-but-not-equal case: the
    /// retained and requested propositions must denote the same term.
    /// Every normalized relation the matcher equates denotes identically
    /// now (`Id Int` either side of `Equal`/`IntegerMathEqual`, the same
    /// `IntLt`/`IntLe` applications across lifted orders), so the refusal
    /// below is defensive — the checked denotation fragment no longer
    /// contains a shape crossing it cannot express.
    /// `Err(Unsupported)` remains for a hypothetical normalized pair
    /// whose denotations diverge.
    fn same_denotation(
        &mut self,
        retained: &Proposition,
        requested: &Proposition,
    ) -> Result<(), BoundedDenotationError> {
        if retained == requested {
            return Ok(());
        }
        let retained = self.denotation.denote(retained)?;
        let requested = self.denotation.denote(requested)?;
        if self
            .denotation
            .arena
            .structurally_equal(retained, requested)
        {
            Ok(())
        } else {
            Err(BoundedDenotationError::Unsupported(
                "integer-math-normalized citation across denotation shapes",
            ))
        }
    }

    /// One ambient assumption already bound in Γ as a rule-instance
    /// premise: the premise proposition and its context variable, with
    /// the citation recorded exactly as `accept_certificate` reports it.
    fn ambient_premise(
        &mut self,
        index: usize,
    ) -> Result<(Proposition, TermHandle), BoundedDenotationError> {
        let Some(proposition) = self.roster.get(index).cloned() else {
            return Err(certificate!(ProofError::UnknownAssumption(index)));
        };
        if index >= self.ambient {
            return Err(certificate!(ProofError::UnknownAssumption(index)));
        }
        self.assumptions.record(index, &proposition);
        let position = self.positions[index];
        let variable = self.variable(position);
        Ok((proposition, variable))
    }

    /// One ambient semantic axiom already bound in Γ as a rule-instance
    /// premise: axioms bind after the assumptions, so index `i` sits at
    /// binder position `ambient + i`.
    fn cited_axiom(
        &mut self,
        index: usize,
    ) -> Result<(Proposition, TermHandle), BoundedDenotationError> {
        let Some(proposition) = self.axioms.get(index).cloned() else {
            return Err(certificate!(ProofError::UnknownSemanticAxiom(index)));
        };
        self.semantic_axioms.record(index, &proposition);
        let position = u32::try_from(self.ambient + index)
            .map_err(|_| BoundedDenotationError::DepthLimitExceeded)?;
        let variable = self.variable(position);
        Ok((proposition, variable))
    }

    /// The denoted evidence for a rule the kernel does not derive: the
    /// shared relation check has already re-decided the instance, so its
    /// arithmetic or conversion content is a named decision axiom
    /// `Π(_ : ⟦premise₁⟧). … . ⟦conclusion⟧` applied to the premise
    /// evidence in rule order. `premises` and `evidence` are aligned —
    /// child terms and cited ambient bindings alike.
    fn rule_instance(
        &mut self,
        rule: AcceptedProofRule,
        premises: Vec<Proposition>,
        evidence: Vec<TermHandle>,
        conclusion: &Proposition,
    ) -> Result<TermHandle, BoundedDenotationError> {
        debug_assert_eq!(premises.len(), evidence.len());
        let axiom = self.denotation.rule_axiom(&premises, conclusion)?;
        let term = self.denotation.arena.apply_arguments(axiom, &evidence);
        self.rules.insert(rule);
        Ok(term)
    }

    /// `⟨zero, t⟩` for disjunct 0, `⟨one, inner⟩` for a later one — the
    /// tagged-sum introduction shape `Σ(t : Two). caseTwo(M, d₀, rest, t)`.
    /// `remaining` is how many disjuncts the sum at this level still
    /// holds: the `one` branch of a binary sum is the bare last disjunct,
    /// so its payload is `t` itself, while a wider sum nests another
    /// tagged pair for the disjuncts after the first.
    fn disjunction_intro(
        &mut self,
        mut remaining: usize,
        mut index: usize,
        mut term: TermHandle,
    ) -> TermHandle {
        let mut tags = Vec::new();
        loop {
            tags.push(self.denotation.arena.insert(if index == 0 {
                Term::TwoZero
            } else {
                Term::TwoOne
            }));
            if index == 0 || remaining == 2 {
                break;
            }
            remaining -= 1;
            index -= 1;
        }
        while let Some(first) = tags.pop() {
            term = self.denotation.arena.insert(Term::Pair {
                first,
                second: term,
            });
        }
        term
    }

    pub(super) fn finish(self, term: TermHandle) -> BoundedDenotation {
        let Elaboration {
            denotation,
            context_bindings,
            expected,
            rules,
            assumptions,
            semantic_axioms,
            ..
        } = self;
        BoundedDenotation {
            arena: denotation.arena,
            certificate: MathematicalCertificate {
                signature: denotation.declarations,
                level_arity: 0,
                context: context_bindings,
                term,
                expected,
            },
            rules: rules.into_iter().collect(),
            assumptions: assumptions.premises,
            semantic_axioms: semantic_axioms.premises,
        }
    }
}
