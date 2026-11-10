/*
Reusing a saved proof: an experiment the owner asked to see before anything is
built on it. No production path calls this module.

A consumer never trusts a saved proof's statement of what it proves. It
computes the goal and the hypotheses from its own lowered program and tables,
exactly as `discharge` does, and takes from the saved evidence only the
certificate. The proof kernel then decides that certificate for that goal
under those hypotheses (`verify`, the same acceptance path as `discharge`).
Nothing here searches: this module cannot reach the prover.

A stale proof (the goal changed), a proof whose assumption is gone, or a proof
attached to the wrong obligation or machine fails because the kernel checks
the certificate's conclusion against the consumer's goal and each cited
hypothesis against the consumer's own.
*/

use proof_admission::{
    AcceptedFact, CertificateEnvelope, EvidenceRoute, IntegerAffineWitness, Obligation,
    ObligationClass, ProofNode, ProofRule, verify_obligation,
};
use semantic_vocabulary::{
    IntegerMathTerm, MachineId, ObligationId, Proposition, PropositionContext, ScalarTerm,
    ScalarType, ValueId,
};
use structure_elaboration::Elaboration;
use terminal_check_tables::{
    Checked, Construct, InvocationId, MachineTables, RefusalReason, UnprovedCause,
};

use crate::{Discharge, Evidence, Knowledge, refusal};

/// Re-verifies saved evidence for `obligation` of `machine`: the goal and
/// hypotheses are this consumer's own, the certificate is the saved one.
pub fn reverify(
    elaboration: &mut Elaboration,
    machine: MachineId,
    obligation: ObligationId,
    saved: &CertificateEnvelope,
) -> Checked<Evidence> {
    let module = &elaboration.module;
    let tables = elaboration
        .tables
        .machines
        .get_mut(machine)
        .ok_or_else(|| {
            refusal(
                RefusalReason::ForeignIdentity,
                machine,
                Construct::Machine,
                "no such machine",
            )
        })?;
    let prepared = crate::prepare(
        &module.machines,
        &module.structural_types,
        machine,
        tables,
        Knowledge::All,
    )?;
    let row = *terminal_check_tables::row(
        &tables.obligations,
        obligation,
        machine,
        Construct::Obligation(obligation),
    )?;
    let lowered = crate::terminal_machine(&module.machines, machine, machine)?;
    let goal = crate::goal(
        &module.machines,
        lowered,
        machine,
        obligation,
        &row.kind,
        tables,
        &prepared.types,
    )?;
    let hypotheses =
        crate::hypotheses(&prepared.graph, tables, machine, obligation, row.created_at)?;
    crate::verify(
        &prepared.context,
        machine,
        obligation,
        goal,
        hypotheses,
        saved.clone(),
    )
}

/// A guarantee proved once over a callee's formals, re-verified at one call.
///
/// The consumer forms the instance itself from its own module: the callee's
/// `ensures` clause and every `requires` clause with each formal replaced by
/// this call's actual, and the saved certificate with the same replacement.
/// The kernel decides that instance; no search runs. The instance holds only
/// under the callee's `requires`, so the call's own requirement obligations
/// must already be discharged at this site (`discharge`); otherwise the call
/// is refused here.
///
/// This is substitution of actuals for formals, not type-parameter
/// instantiation: the subset has no generics. A certificate that names any
/// callee value other than a formal (a local, a constant's result) cannot be
/// moved to the caller and is refused.
pub fn reverify_at_call(
    elaboration: &Elaboration,
    discharge: &Discharge,
    caller: MachineId,
    invocation: InvocationId,
    clause: u32,
    saved: &CertificateEnvelope,
) -> Checked<AcceptedFact> {
    let concerns = Construct::Invocation(invocation);
    let refuse = |reason, detail| refusal(reason, caller, concerns, detail);
    let tables = terminal_check_tables::row(
        &elaboration.tables.machines,
        caller,
        caller,
        Construct::Machine,
    )?;
    let row = terminal_check_tables::row(&tables.invocations, invocation, caller, concerns)?;
    let callee = crate::callee(caller, invocation, &row.callee)?;
    let actuals = crate::scalar_actuals(&row.actuals);
    let verdicts =
        &terminal_check_tables::row(&discharge.machines, caller, caller, Construct::Machine)?
            .verdicts;
    for requirement in &row.requirements {
        if !matches!(verdicts.get(*requirement), Some(Ok(_))) {
            return Err(refuse(
                RefusalReason::Unproved(UnprovedCause::NoDerivation),
                "the call's requirement is not discharged at this site",
            ));
        }
    }
    let contract = &crate::terminal_machine(&elaboration.module.machines, callee, caller)?.contract;
    let ensures = usize::try_from(clause)
        .ok()
        .and_then(|clause| contract.ensures.get(clause))
        .ok_or_else(|| {
            refuse(
                RefusalReason::ForeignIdentity,
                "callee has no such ensures clause",
            )
        })?;
    let unsupported = |detail| refuse(RefusalReason::Unsupported, detail);
    let goal = crate::substitute(&ensures.proposition, &actuals, tables).map_err(unsupported)?;
    let assumptions = contract
        .requires
        .iter()
        .map(|clause| crate::substitute(clause, &actuals, tables).map_err(unsupported))
        .collect::<Checked<Vec<_>>>()?;
    let replace = Replace {
        actuals: &actuals,
        tables,
    };
    let proof = replace
        .node(&saved.proof)
        .ok_or_else(|| unsupported("the certificate names a callee value that is not a formal"))?;
    let context = PropositionContext::from_value_types(
        tables
            .values
            .iter()
            .map(|(value, row)| (value, row.scalar_type)),
    )
    .map_err(|_| unsupported("value types do not form a proposition context"))?;
    verify_obligation(
        &context,
        &Obligation {
            id: ensures.obligation,
            proposition: goal,
            class: ObligationClass::Derivable,
        },
        &assumptions,
        &[],
        EvidenceRoute::CertificateDerived(CertificateEnvelope {
            identity: saved.identity,
            proof_system_marker: saved.proof_system_marker,
            proof,
        }),
    )
    .map_err(|error| {
        refusal(
            RefusalReason::Unproved(UnprovedCause::KernelRefused),
            caller,
            concerns,
            &format!("kernel {error:?}"),
        )
    })
}

/// Callee formal `i` (callee value `i + 1`) becomes the caller's actual `i`
/// throughout a certificate. Every other value, and every term or rule form
/// this does not list, makes the whole replacement fail: a partial rename
/// would leave the certificate naming the callee's values in the caller.
struct Replace<'a> {
    actuals: &'a [ValueId],
    tables: &'a MachineTables,
}

impl Replace<'_> {
    fn value(&self, value: ValueId, scalar_type: ScalarType) -> Option<ValueId> {
        let actual = *self.actuals.get(usize::try_from(value.get() - 1).ok()?)?;
        (self.tables.values.get(actual)?.scalar_type == scalar_type).then_some(actual)
    }

    fn term(&self, term: &ScalarTerm) -> Option<ScalarTerm> {
        Some(match term {
            ScalarTerm::Value { id, scalar_type } => {
                ScalarTerm::value(self.value(*id, *scalar_type)?, *scalar_type)
            }
            ScalarTerm::Integer { .. } | ScalarTerm::Boolean(_) => term.clone(),
            ScalarTerm::ExactIntegerAdd {
                scalar_type,
                left,
                right,
            } => ScalarTerm::ExactIntegerAdd {
                scalar_type: *scalar_type,
                left: Box::new(self.term(left)?),
                right: Box::new(self.term(right)?),
            },
            ScalarTerm::ExactIntegerSubtract {
                scalar_type,
                left,
                right,
            } => ScalarTerm::ExactIntegerSubtract {
                scalar_type: *scalar_type,
                left: Box::new(self.term(left)?),
                right: Box::new(self.term(right)?),
            },
            ScalarTerm::ExactIntegerMultiply {
                scalar_type,
                left,
                right,
            } => ScalarTerm::ExactIntegerMultiply {
                scalar_type: *scalar_type,
                left: Box::new(self.term(left)?),
                right: Box::new(self.term(right)?),
            },
            _ => return None,
        })
    }

    fn math(&self, term: &IntegerMathTerm) -> Option<IntegerMathTerm> {
        Some(match term {
            IntegerMathTerm::IntegerLiteral(_) => term.clone(),
            IntegerMathTerm::MathValue { source_type, value } => IntegerMathTerm::MathValue {
                source_type: *source_type,
                value: self.value(*value, ScalarType::Integer(*source_type))?,
            },
            IntegerMathTerm::Add(left, right) => {
                IntegerMathTerm::Add(Box::new(self.math(left)?), Box::new(self.math(right)?))
            }
            IntegerMathTerm::Subtract(left, right) => {
                IntegerMathTerm::Subtract(Box::new(self.math(left)?), Box::new(self.math(right)?))
            }
            IntegerMathTerm::Multiply(left, right) => {
                IntegerMathTerm::Multiply(Box::new(self.math(left)?), Box::new(self.math(right)?))
            }
            IntegerMathTerm::ShiftLeft { value, count } => IntegerMathTerm::ShiftLeft {
                value: Box::new(self.math(value)?),
                count: Box::new(self.math(count)?),
            },
            IntegerMathTerm::Embed(_) => return None,
        })
    }

    fn proposition(&self, proposition: &Proposition) -> Option<Proposition> {
        let all = |parts: &[Proposition]| {
            parts
                .iter()
                .map(|part| self.proposition(part))
                .collect::<Option<Vec<_>>>()
        };
        Some(match proposition {
            Proposition::Truth | Proposition::Falsehood => proposition.clone(),
            Proposition::Equal(left, right) => {
                Proposition::Equal(self.term(left)?, self.term(right)?)
            }
            Proposition::LessThan(left, right) => {
                Proposition::LessThan(self.term(left)?, self.term(right)?)
            }
            Proposition::LessOrEqual(left, right) => {
                Proposition::LessOrEqual(self.term(left)?, self.term(right)?)
            }
            Proposition::IntegerMathEqual(left, right) => {
                Proposition::IntegerMathEqual(self.math(left)?, self.math(right)?)
            }
            Proposition::IntegerMathLessThan(left, right) => {
                Proposition::IntegerMathLessThan(self.math(left)?, self.math(right)?)
            }
            Proposition::IntegerMathLessOrEqual(left, right) => {
                Proposition::IntegerMathLessOrEqual(self.math(left)?, self.math(right)?)
            }
            Proposition::Conjunction(parts) => Proposition::Conjunction(all(parts)?),
            Proposition::Disjunction(parts) => Proposition::Disjunction(all(parts)?),
            Proposition::Implication {
                premise,
                conclusion,
            } => Proposition::Implication {
                premise: Box::new(self.proposition(premise)?),
                conclusion: Box::new(self.proposition(conclusion)?),
            },
            _ => return None,
        })
    }

    fn node(&self, node: &ProofNode) -> Option<ProofNode> {
        let child = |child: &ProofNode| self.node(child).map(Box::new);
        let children = |children: &[ProofNode]| {
            children
                .iter()
                .map(|child| self.node(child))
                .collect::<Option<Vec<_>>>()
        };
        let rule = match &node.rule {
            ProofRule::Primitive(_)
            | ProofRule::SemanticAxiom { .. }
            | ProofRule::Assumption { .. } => node.rule.clone(),
            ProofRule::ValueEqualityTransport {
                premise,
                equalities,
            } => ProofRule::ValueEqualityTransport {
                premise: child(premise)?,
                equalities: children(equalities)?,
            },
            ProofRule::PredicateDenotation { premise } => ProofRule::PredicateDenotation {
                premise: child(premise)?,
            },
            ProofRule::ConjunctionIntroduction(parts) => {
                ProofRule::ConjunctionIntroduction(children(parts)?)
            }
            ProofRule::ConjunctionElimination {
                conjunction,
                conjunct,
            } => ProofRule::ConjunctionElimination {
                conjunction: child(conjunction)?,
                conjunct: *conjunct,
            },
            ProofRule::DisjunctionIntroduction { disjunct, index } => {
                ProofRule::DisjunctionIntroduction {
                    disjunct: child(disjunct)?,
                    index: *index,
                }
            }
            ProofRule::DisjunctionElimination {
                disjunction,
                branches,
            } => ProofRule::DisjunctionElimination {
                disjunction: child(disjunction)?,
                branches: children(branches)?,
            },
            ProofRule::ImplicationIntroduction { body } => {
                ProofRule::ImplicationIntroduction { body: child(body)? }
            }
            ProofRule::ImplicationElimination {
                implication,
                premise,
            } => ProofRule::ImplicationElimination {
                implication: child(implication)?,
                premise: child(premise)?,
            },
            ProofRule::EqualityTransitivity {
                left_equals_middle,
                middle_equals_right,
            } => ProofRule::EqualityTransitivity {
                left_equals_middle: child(left_equals_middle)?,
                middle_equals_right: child(middle_equals_right)?,
            },
            ProofRule::EqualitySymmetry { equality } => ProofRule::EqualitySymmetry {
                equality: child(equality)?,
            },
            ProofRule::IntegerOrderWeakening { relation } => ProofRule::IntegerOrderWeakening {
                relation: child(relation)?,
            },
            ProofRule::IntegerOrderDiscreteness { relation } => {
                ProofRule::IntegerOrderDiscreteness {
                    relation: child(relation)?,
                }
            }
            ProofRule::IntegerMathOrderTranslation { relation, addend } => {
                ProofRule::IntegerMathOrderTranslation {
                    relation: child(relation)?,
                    addend: self.math(addend)?,
                }
            }
            ProofRule::IntegerOrderAntisymmetry { forward, backward } => {
                ProofRule::IntegerOrderAntisymmetry {
                    forward: child(forward)?,
                    backward: child(backward)?,
                }
            }
            ProofRule::IntegerSubtractOrder {
                difference,
                positive,
            } => ProofRule::IntegerSubtractOrder {
                difference: child(difference)?,
                positive: child(positive)?,
            },
            ProofRule::IntegerAddOrder { sum, positive } => ProofRule::IntegerAddOrder {
                sum: child(sum)?,
                positive: child(positive)?,
            },
            ProofRule::IntegerSubtractAntitone {
                smaller,
                larger,
                order,
            } => ProofRule::IntegerSubtractAntitone {
                smaller: child(smaller)?,
                larger: child(larger)?,
                order: child(order)?,
            },
            ProofRule::IntegerLessOrEqualTransitivity {
                left_less_or_equal_middle,
                middle_less_or_equal_right,
            } => ProofRule::IntegerLessOrEqualTransitivity {
                left_less_or_equal_middle: child(left_less_or_equal_middle)?,
                middle_less_or_equal_right: child(middle_less_or_equal_right)?,
            },
            ProofRule::IntegerStrictOrderTransitivity {
                left_to_middle,
                middle_to_right,
            } => ProofRule::IntegerStrictOrderTransitivity {
                left_to_middle: child(left_to_middle)?,
                middle_to_right: child(middle_to_right)?,
            },
            ProofRule::IntegerOrderSubstitution {
                relation,
                equality,
                endpoint,
            } => ProofRule::IntegerOrderSubstitution {
                relation: child(relation)?,
                equality: child(equality)?,
                endpoint: *endpoint,
            },
            ProofRule::IntegerAffineBound {
                root_bound,
                witness,
            } => ProofRule::IntegerAffineBound {
                root_bound: child(root_bound)?,
                witness: IntegerAffineWitness {
                    root: self.term(&witness.root)?,
                    target: self.term(&witness.target)?,
                    definition_axioms: witness.definition_axioms.clone(),
                    literal_axioms: witness.literal_axioms.clone(),
                },
            },
            ProofRule::IntegerExactAddDefinitionBound {
                left_bound,
                right_bound,
                definition_axiom,
            } => ProofRule::IntegerExactAddDefinitionBound {
                left_bound: child(left_bound)?,
                right_bound: child(right_bound)?,
                definition_axiom: *definition_axiom,
            },
            ProofRule::IntegerExactSubtractDefinitionBound {
                left_bound,
                right_bound,
                definition_axiom,
            } => ProofRule::IntegerExactSubtractDefinitionBound {
                left_bound: child(left_bound)?,
                right_bound: child(right_bound)?,
                definition_axiom: *definition_axiom,
            },
            ProofRule::IntegerCastBound { .. }
            | ProofRule::IntegerCorrelatedForbiddenRoots { .. } => {
                return None;
            }
        };
        Some(ProofNode {
            conclusion: self.proposition(&node.conclusion)?,
            rule,
        })
    }
}
