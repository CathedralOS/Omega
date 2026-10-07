use super::*;

/// One checked affine definition step, denoted: `Equal(current, op(next,
/// sibling))` with `sibling` the step's literal — closed directly or
/// landed by the cited literal axiom.
#[derive(Clone)]
pub(super) struct ChainStep {
    /// `Id Int current' (op' next' sib')` — the cited definition.
    pub(super) definition: TermHandle,
    /// `Id Int sib' lit'` — the cited literal equation when the sibling
    /// is a landed value rather than a literal.
    pub(super) literal: Option<TermHandle>,
    /// The step's operation.
    pub(super) operation: ChainOperation,
    /// `next'` — the step's inner operand.
    pub(super) next: TermHandle,
    /// `sib'` — the step's right operand as denoted.
    pub(super) sibling: TermHandle,
    /// `lit'` — the step's literal numeral.
    pub(super) literal_term: TermHandle,
    /// The literal's exact checked value.
    pub(super) literal_value: i128,
    /// `op' next' sib'` — the denoted right-hand side.
    pub(super) expression: TermHandle,
    /// `current'` — the step's defined value.
    pub(super) current: TermHandle,
}

/// The step's exact-arithmetic operation: the denoted shared function
/// and its exact `i128` evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ChainOperation {
    Add,
    Subtract,
    Multiply,
}

impl Denotation {
    pub(super) fn chain_operation_terms(
        &mut self,
        operation: ChainOperation,
        left: TermHandle,
        right: TermHandle,
    ) -> Result<TermHandle, BoundedDenotationError> {
        match operation {
            ChainOperation::Add => self.add_terms(left, right),
            ChainOperation::Subtract => self.subtract_terms(left, right),
            ChainOperation::Multiply => self.multiply_terms(left, right),
        }
    }

    /// The exact `i128` evaluation of one chain step's operation, or
    /// `None` on overflow — a defensive refusal the checked witness
    /// never produces.
    pub(super) fn chain_operation_eval(
        operation: ChainOperation,
        left: i128,
        right: i128,
    ) -> Option<i128> {
        match operation {
            ChainOperation::Add => left.checked_add(right),
            ChainOperation::Subtract => left.checked_sub(right),
            ChainOperation::Multiply => left.checked_mul(right),
        }
    }

    /// `J(λ(y : Int). λ(_ : Id Int from y). F y, base, to, proof)` —
    /// dependent identity elimination over `Int` with the family built
    /// under the two binders: `family` receives `y` as a `Variable(1)`
    /// handle and returns the body's type-level shape.

    pub(super) fn chain_sibling_congruence(
        &mut self,
        integer: TermHandle,
        operation: ChainOperation,
        next: TermHandle,
        from: TermHandle,
        to: TermHandle,
        proof: TermHandle,
        expression: TermHandle,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let base = self.arena.insert(Term::Refl {
            ty: integer,
            value: expression,
        });
        self.integer_id_elim(integer, from, to, proof, base, |this, bound| {
            let rewritten = this.chain_operation_terms(operation, next, bound)?;
            Ok(this.arena.insert(Term::Id {
                ty: integer,
                left: expression,
                right: rewritten,
            }))
        })
    }

    /// `p : Id Int inner' v'` rewrites the left operand of `op' inner'
    /// sib'` — the congruence step carrying `Id u' (op' inner' sib')` to
    /// `Id u' (op' v' sib')` through the solved inner equation. Returns
    /// `Id Int (op' inner' sib') (op' v' sib')`.
    pub(super) fn chain_operand_congruence(
        &mut self,
        integer: TermHandle,
        operation: ChainOperation,
        sibling: TermHandle,
        from: TermHandle,
        to: TermHandle,
        proof: TermHandle,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let source = self.chain_operation_terms(operation, from, sibling)?;
        let base = self.arena.insert(Term::Refl {
            ty: integer,
            value: source,
        });
        self.integer_id_elim(integer, from, to, proof, base, |this, bound| {
            let rewritten = this.chain_operation_terms(operation, bound, sibling)?;
            Ok(this.arena.insert(Term::Id {
                ty: integer,
                left: source,
                right: rewritten,
            }))
        })
    }

    /// Parse one checked branch's steps into denoted chain data, in the
    /// witness's innermost-first order, consuming `evidence` in the
    /// relation's citation order — literal axiom then definition axiom
    /// per step. Every definition is verified against the chain shape:
    /// `Equal(current, op(next, sibling))`. `None` keeps the instance
    /// fallback for any other shape.
    pub(super) fn chain_steps(
        &mut self,
        branch: &CorrelatedAffineBranchWitness,
        integer_type: IntegerType,
        evidence: &[(Proposition, TermHandle)],
        cursor: &mut usize,
    ) -> Result<Option<Vec<ChainStep>>, BoundedDenotationError> {
        let mut steps = Vec::with_capacity(branch.steps.len());
        for step in &branch.steps {
            let literal_evidence = if step.literal_axiom.is_some() {
                let Some((proposition, evidence)) = evidence.get(*cursor) else {
                    return Ok(None);
                };
                *cursor += 1;
                Some((proposition.clone(), *evidence))
            } else {
                None
            };
            let Some((definition_proposition, definition_evidence)) = evidence.get(*cursor) else {
                return Ok(None);
            };
            *cursor += 1;
            let Proposition::Equal(current, expression) = definition_proposition else {
                return Ok(None);
            };
            let (operation, next, sibling) = match expression {
                ScalarTerm::ExactIntegerAdd {
                    scalar_type,
                    left,
                    right,
                } if *scalar_type == integer_type => {
                    (ChainOperation::Add, left.as_ref(), right.as_ref())
                }
                ScalarTerm::ExactIntegerSubtract {
                    scalar_type,
                    left,
                    right,
                } if *scalar_type == integer_type => {
                    (ChainOperation::Subtract, left.as_ref(), right.as_ref())
                }
                ScalarTerm::ExactIntegerMultiply {
                    scalar_type,
                    left,
                    right,
                } if *scalar_type == integer_type => {
                    (ChainOperation::Multiply, left.as_ref(), right.as_ref())
                }
                _ => return Ok(None),
            };
            let current_term = self.fixed_scalar_term(current)?;
            let next_term = self.fixed_scalar_term(next)?;
            let sibling_term = self.fixed_scalar_term(sibling)?;
            let expression_term = self.chain_operation_terms(operation, next_term, sibling_term)?;
            // The cited evidence's own denotation fixes the equation's
            // orientation — `directed_equality` re-syms whichever side
            // canonical `IntegerMathEqual` order flipped.
            let denoted = self.denote(definition_proposition)?;
            let Some((_, from, to)) = self.identity_parts(denoted) else {
                return Ok(None);
            };
            let Some(definition) = self.directed_equality(
                from,
                to,
                current_term,
                expression_term,
                *definition_evidence,
            ) else {
                return Ok(None);
            };
            let (literal_value, literal_term, literal_evidence_term) = match literal_evidence {
                None => {
                    let Some(value) = signed_literal(sibling, integer_type) else {
                        return Ok(None);
                    };
                    (value, sibling_term, None)
                }
                Some((proposition, evidence)) => {
                    let Proposition::Equal(left, right) = &proposition else {
                        return Ok(None);
                    };
                    if left != sibling {
                        return Ok(None);
                    }
                    let Some(value) = signed_literal(right, integer_type) else {
                        return Ok(None);
                    };
                    let literal_term = self.integer_numeral(value)?;
                    let denoted = self.denote(&proposition)?;
                    let Some((_, from, to)) = self.identity_parts(denoted) else {
                        return Ok(None);
                    };
                    let Some(evidence) =
                        self.directed_equality(from, to, sibling_term, literal_term, evidence)
                    else {
                        return Ok(None);
                    };
                    (value, literal_term, Some(evidence))
                }
            };
            steps.push(ChainStep {
                definition,
                literal: literal_evidence_term,
                operation,
                next: next_term,
                sibling: sibling_term,
                literal_term,
                literal_value,
                expression: expression_term,
                current: current_term,
            });
        }
        Ok(Some(steps))
    }

    /// One step's evidence lifted under `amount` enclosing λ's: the
    /// variable evidence (definition and literal citations) shifts;
    /// every denoted endpoint is closed so `shift` returns it intact.
    pub(super) fn chain_step_lift(&mut self, step: &ChainStep, amount: u32) -> ChainStep {
        ChainStep {
            definition: shift(&mut self.arena, step.definition, 0, amount),
            literal: step
                .literal
                .map(|literal| shift(&mut self.arena, literal, 0, amount)),
            operation: step.operation,
            next: shift(&mut self.arena, step.next, 0, amount),
            sibling: shift(&mut self.arena, step.sibling, 0, amount),
            literal_term: shift(&mut self.arena, step.literal_term, 0, amount),
            literal_value: step.literal_value,
            expression: shift(&mut self.arena, step.expression, 0, amount),
            current: shift(&mut self.arena, step.current, 0, amount),
        }
    }
}
