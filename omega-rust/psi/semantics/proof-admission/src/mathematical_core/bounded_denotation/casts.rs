//! Checked exact-cast bound transport.
//!
//! A cast chain's every edge is an exact widen or partial cast — the
//! checked chain's own invariant is that the mathematical integer is
//! preserved — so each cast operation carries one fixed identity law
//! `Π(x : Int). Id Int (op x) x`, interned per operation constructor and
//! machine types like every other fixed roster member. The bound on the
//! chain's root (or, under a `Truth` root, the tightest carrier's
//! membership bound) then walks the cited definition equalities through
//! the shared endpoint-substitution laws instead of a per-instance
//! `rule_axiom`. Any shape the transport cannot express keeps the
//! instance fallback — the relation check already re-decided it, so the
//! fallback remains a checked judgment.

use semantic_vocabulary::{IntegerType, IntegerValue, Proposition, ScalarTerm};

use crate::CheckedIntegerCastChain;

use super::integer_operations::IntegerOperation;
use super::{BoundedDenotationError, Declaration, Denotation, Term, TermHandle};

/// `(minimum, maximum)` of one fixed native carrier, matching the chain
/// checker's own interval gate — addresses and non-{8,16,32,64} widths
/// have no representable interval.
fn carrier_interval(integer_type: IntegerType) -> Option<(i128, i128)> {
    if integer_type.is_address() || !matches!(integer_type.bits(), 8 | 16 | 32 | 64) {
        return None;
    }
    let as_i128 = |value: IntegerValue| match value {
        IntegerValue::Signed(value) => Some(value),
        IntegerValue::Unsigned(value) => i128::try_from(value).ok(),
    };
    Some((
        as_i128(integer_type.minimum_value())?,
        as_i128(integer_type.maximum_value())?,
    ))
}

/// One checked chain edge in denoted form: the `next` value and its
/// exact-cast definition, with the operation identity the identity law
/// keys on and the context variable citing the axiom.
struct CastEdge {
    operation: IntegerOperation,
    previous: TermHandle,
    next: TermHandle,
    /// `⟦op·prev⟧` — the denoted cast application the definition names.
    application: TermHandle,
    /// `d : Id Int next (op·prev)` — the cited axiom's context variable.
    definition: TermHandle,
}

impl Denotation {
    /// `Π(x : Int). Id Int (op x) x` — one assumption constant per exact
    /// cast operation. The chain checker only admits edges whose cast is
    /// exact, so the law states precisely the invariant it verified.
    fn cast_identity_law(
        &mut self,
        operation: IntegerOperation,
    ) -> Result<u32, BoundedDenotationError> {
        if let Some(&position) = self.cast_identities.get(&operation) {
            return Ok(position);
        }
        let integer = self.integer_constant()?;
        let operation_position = self.integer_operation(operation)?;
        let function = self.constant(operation_position);
        let operand = self.arena.insert(Term::Variable(0));
        let applied = self.arena.insert(Term::Apply {
            function,
            argument: operand,
        });
        let identity = self.arena.insert(Term::Id {
            ty: integer,
            left: applied,
            right: operand,
        });
        let ty = self.arena.insert(Term::Pi {
            domain: integer,
            codomain: identity,
        });
        let position = self.position()?;
        self.declarations.push(Declaration::assumption(0, ty));
        self.cast_identities.insert(operation, position);
        Ok(position)
    }

    /// `cast_id prev'` : `Id Int (op·prev') prev'` — the law instantiated
    /// at one chain edge's source value.
    fn cast_identity_application(
        &mut self,
        operation: IntegerOperation,
        operand: TermHandle,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let position = self.cast_identity_law(operation)?;
        let law = self.constant(position);
        Ok(self.arena.insert(Term::Apply {
            function: law,
            argument: operand,
        }))
    }

    /// `IntLe a' b'` — the denoted `IntLe` application over two already
    /// interned endpoints.
    fn integer_less_or_equal_term(
        &mut self,
        left: TermHandle,
        right: TermHandle,
    ) -> Result<TermHandle, BoundedDenotationError> {
        let position = self.integer_less_or_equal()?;
        let relation = self.constant(position);
        let function = self.arena.insert(Term::Apply {
            function: relation,
            argument: left,
        });
        Ok(self.arena.insert(Term::Apply {
            function,
            argument: right,
        }))
    }

    /// Read one chain edge's cited definition axiom: `Equal(next, cast)`
    /// for a checked `IntegerWiden`/`IntegerExactCast` operand.
    fn cast_edge_definition(
        proposition: &Proposition,
    ) -> Option<(&ScalarTerm, &ScalarTerm, IntegerOperation)> {
        let Proposition::Equal(next, definition) = proposition else {
            return None;
        };
        let operation = match definition {
            ScalarTerm::IntegerWiden {
                source_type,
                target_type,
                ..
            } => IntegerOperation::Widen {
                source: *source_type,
                target: *target_type,
            },
            ScalarTerm::IntegerExactCast {
                source_type,
                target_type,
                ..
            } => IntegerOperation::ExactCast {
                source: *source_type,
                target: *target_type,
            },
            _ => return None,
        };
        Some((next, definition, operation))
    }

    /// The denoted evidence for a checked `IntegerCastBound`/`Truth` cast
    /// conclusion, or `None` when any step's shape leaves the transport
    /// vocabulary — the caller's `rule_instance` then records the
    /// instance axiom exactly as before.
    ///
    /// `axiom_evidence` aligns with `chain.definition_axioms()` in order:
    /// the context variable citing each `Equal(next, cast(prev))`.
    pub(super) fn cast_bound_evidence(
        &mut self,
        root_bound: &Proposition,
        root_evidence: TermHandle,
        chain: &CheckedIntegerCastChain,
        axiom_evidence: &[TermHandle],
        semantic_axioms: &[Proposition],
        conclusion: &Proposition,
    ) -> Result<Option<TermHandle>, BoundedDenotationError> {
        let goal = self.denote(conclusion)?;
        let Some(super::IntegerRelation::LessOrEqual {
            left: goal_left,
            right: goal_right,
        }) = self.integer_relation(goal)
        else {
            return Ok(None);
        };

        // The denoted chain edges, in chain order.
        let mut edges = Vec::with_capacity(chain.definition_axioms().len());
        let mut values = Vec::with_capacity(chain.definition_axioms().len() + 1);
        values.push(chain.root().clone());
        let mut previous = self.fixed_scalar_term(chain.root())?;
        for (&index, &axiom) in chain.definition_axioms().iter().zip(axiom_evidence.iter()) {
            let Some(proposition) = semantic_axioms.get(index) else {
                return Ok(None);
            };
            let Some((next, definition, operation)) = Self::cast_edge_definition(proposition)
            else {
                return Ok(None);
            };
            let application = self.fixed_scalar_term(definition)?;
            let next_denoted = self.fixed_scalar_term(next)?;
            edges.push(CastEdge {
                operation,
                previous,
                next: next_denoted,
                application,
                definition: axiom,
            });
            previous = next_denoted;
            values.push(next.clone());
        }
        let target = self.fixed_scalar_term(chain.target())?;

        // `d : Id next (op·prev)` composed with `cast_id prev :
        // Id (op·prev) prev` gives `Id prev next` after `sym` — the edge
        // equality direction the substitution laws consume.
        let step_equality = |this: &mut Self,
                             edge: &CastEdge|
         -> Result<(TermHandle, TermHandle), BoundedDenotationError> {
            let integer = this.integer_constant()?;
            let identity = this.cast_identity_application(edge.operation, edge.previous)?;
            let forward = this.transitivity(
                integer,
                edge.next,
                edge.application,
                edge.previous,
                edge.definition,
                identity,
            );
            let ty = this.arena.insert(Term::Id {
                ty: integer,
                left: edge.next,
                right: edge.previous,
            });
            Ok((ty, forward))
        };

        // The initial bound, its type, the fixed endpoint, the moving
        // side's law direction (endpoint 0 = left, 1 = right), and the
        // chain index transport starts at.
        let (mut bound, mut bound_ty, endpoint, moving_endpoint, start) =
            if *root_bound == Proposition::Truth {
                // `min ≤ target` / `target ≤ max` for the surviving interval:
                // the endpoint originates at the carrier whose own minimum
                // (maximum) is the interval's, then transports forward.
                let lower = self.arena.structurally_equal(goal_right, target);
                let upper = self.arena.structurally_equal(goal_left, target);
                if lower == upper {
                    return Ok(None);
                }
                if values.len() != chain.carriers().len() {
                    return Ok(None);
                }
                let mut selected: Option<usize> = None;
                let mut selected_bound = 0i128;
                for (index, carrier) in chain.carriers().iter().enumerate() {
                    let Some((minimum, maximum)) = carrier_interval(*carrier) else {
                        return Ok(None);
                    };
                    let candidate = if lower { minimum } else { maximum };
                    let better = match selected {
                        None => true,
                        Some(_) => {
                            if lower {
                                candidate > selected_bound
                            } else {
                                candidate < selected_bound
                            }
                        }
                    };
                    if better {
                        selected = Some(index);
                        selected_bound = candidate;
                    }
                }
                let Some(origin) = selected else {
                    return Ok(None);
                };
                let (endpoint_term, evidence) =
                    self.carrier_bound(&values[origin], lower, &chain.carriers()[origin])?;
                let endpoint = self.fixed_scalar_term(&endpoint_term)?;
                let value_term = self.fixed_scalar_term(&values[origin])?;
                let bound_ty = if lower {
                    self.integer_less_or_equal_term(endpoint, value_term)?
                } else {
                    self.integer_less_or_equal_term(value_term, endpoint)?
                };
                // `min ≤ v` moves the right endpoint, `v ≤ max` the left.
                (evidence, bound_ty, endpoint, usize::from(lower), origin)
            } else {
                let root_ty = self.denote(root_bound)?;
                let Some(super::IntegerRelation::LessOrEqual { left, right }) =
                    self.integer_relation(root_ty)
                else {
                    return Ok(None);
                };
                let root = self.fixed_scalar_term(chain.root())?;
                let (endpoint, moving_endpoint) = if self.arena.structurally_equal(right, root) {
                    // `lit ≤ root` — the value is the right endpoint.
                    (left, 1usize)
                } else if self.arena.structurally_equal(left, root) {
                    // `root ≤ lit` — the value is the left endpoint.
                    (right, 0usize)
                } else {
                    return Ok(None);
                };
                (root_evidence, root_ty, endpoint, moving_endpoint, 0)
            };

        for edge in &edges[start..] {
            let (equality_ty, equality) = step_equality(self, edge)?;
            let step_goal = if moving_endpoint == 1 {
                self.integer_less_or_equal_term(endpoint, edge.next)?
            } else {
                self.integer_less_or_equal_term(edge.next, endpoint)?
            };
            let Some(next_bound) = self.order_substitution_evidence(
                bound_ty,
                bound,
                equality_ty,
                equality,
                moving_endpoint,
                step_goal,
            )?
            else {
                return Ok(None);
            };
            bound = next_bound;
            bound_ty = step_goal;
        }

        // The transported bound must name exactly the checked conclusion —
        // the shared relation already verified the literal retypes to the
        // same mathematical integer, so a mismatch means the shape left
        // the transport vocabulary rather than producing a wrong term.
        if !self.arena.structurally_equal(bound_ty, goal) {
            return Ok(None);
        }
        Ok(Some(bound))
    }
}

#[cfg(test)]
mod tests;
