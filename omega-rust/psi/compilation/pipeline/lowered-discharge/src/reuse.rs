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
    ObligationClass, ProofNode, ProofRule, ProofSystemMarker, verify_obligation,
};
use semantic_vocabulary::{
    EvidenceIdentity, IdTable, IntegerMathTerm, MachineId, ObligationId, Proposition,
    PropositionContext, ScalarTerm, ScalarType, ValueId,
};
use structure_elaboration::Elaboration;
use terminal_check_tables::edges::Callee;
use terminal_check_tables::obligations::ObligationKind;
use terminal_check_tables::values::Justification;
use terminal_check_tables::{
    Checked, Construct, InvocationId, KnowledgeId, MachineTables, Refusal, RefusalReason,
    UnprovedCause,
};
use terminal_psi::TerminalMachine;

use crate::{Discharge, Discharged, Evidence, Hypotheses, Knowledge, refusal};

/// Re-verifies saved evidence for `obligation` of `machine`: the goal and
/// hypotheses are this consumer's own, the certificate is the saved one.
pub fn reverify(
    elaboration: &mut Elaboration,
    machine: MachineId,
    obligation: ObligationId,
    saved: &CertificateEnvelope,
) -> Checked<Evidence> {
    let module = &elaboration.module;
    let domains = &elaboration.domains;
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
        domains,
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
    let storage = crate::places::Storage::new(
        machine,
        lowered,
        &module.structural_types,
        domains,
        &prepared.graph,
        tables,
        &prepared.types,
    )?;
    let goal = crate::goal(
        &module.machines,
        lowered,
        &storage,
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
    // Scalar formals only: a clause over a field of a record formal is
    // refused by these bindings, not partly stated.
    let instance = |clause: &Proposition| {
        crate::substitute(
            clause,
            &mut crate::ScalarActuals {
                actuals: &actuals,
                result: None,
                tables,
            },
        )
        .map_err(|unbound| unbound.refusal(caller, concerns))
    };
    let goal = instance(&ensures.proposition)?;
    let assumptions = contract
        .requires
        .iter()
        .map(instance)
        .collect::<Checked<Vec<_>>>()?;
    // Formals become this call's actuals; the hypotheses the certificate
    // cites keep their positions, because the assumptions given to the
    // kernel below are the callee's `requires` in clause order.
    let proof = Transport {
        value: &|value, scalar_type| formal_actual(&actuals, tables, value, scalar_type),
        assumption: &|index| Ok(Cited::Index(index)),
        axiom: &|index| Ok(index),
        refused: std::cell::RefCell::new(None),
    }
    .certificate(&saved.proof)
    .map_err(|refused| {
        refused.unwrap_or_else(|| {
            unsupported("the certificate names a callee value that is not a formal")
        })
    })?;
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

/// The caller's actual for callee formal `value` (callee value `i + 1` is
/// actual `i`), when the actual has the formal's type.
fn formal_actual(
    actuals: &[ValueId],
    tables: &MachineTables,
    value: ValueId,
    scalar_type: ScalarType,
) -> Option<ValueId> {
    let actual = *actuals.get(usize::try_from(value.get() - 1).ok()?)?;
    (tables.values.get(actual)?.scalar_type == scalar_type).then_some(actual)
}

/// What a hypothesis a certificate cites becomes where the certificate is
/// moved to: the position of the same hypothesis there, or, where the
/// destination does not assume it, a proof of it from what the destination
/// does assume.
enum Cited {
    Index(usize),
    Proof(ProofNode),
}

/// Moves a certificate into another program: every value it names becomes
/// the destination's value, and every hypothesis it cites the destination's.
/// The result is only a candidate; the kernel decides it against the
/// destination's own goal and hypotheses. A value with no counterpart, or a
/// term or rule form this does not list, makes the whole move fail: a
/// partial rename would leave the certificate naming the source's values.
struct Transport<'a> {
    value: &'a dyn Fn(ValueId, ScalarType) -> Option<ValueId>,
    assumption: &'a dyn Fn(usize) -> Checked<Cited>,
    axiom: &'a dyn Fn(usize) -> Checked<usize>,
    /// Why a cited hypothesis could not be moved, when that is what failed.
    refused: std::cell::RefCell<Option<Refusal>>,
}

impl Transport<'_> {
    /// The moved certificate, or the refusal of a hypothesis it cites;
    /// `None` when a value, term or rule form has no counterpart.
    fn certificate(&self, node: &ProofNode) -> Result<ProofNode, Option<Refusal>> {
        self.node(node).ok_or_else(|| self.refused.take())
    }

    fn value(&self, value: ValueId, scalar_type: ScalarType) -> Option<ValueId> {
        (self.value)(value, scalar_type)
    }

    fn cited_axiom(&self, index: usize) -> Option<usize> {
        match (self.axiom)(index) {
            Ok(index) => Some(index),
            Err(refused) => {
                self.refused.replace(Some(refused));
                None
            }
        }
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
            ProofRule::Primitive(_) => node.rule.clone(),
            ProofRule::SemanticAxiom { index } => ProofRule::SemanticAxiom {
                index: self.cited_axiom(*index)?,
            },
            ProofRule::Assumption { index } => match (self.assumption)(*index) {
                Ok(Cited::Index(index)) => ProofRule::Assumption { index },
                // The destination does not assume this hypothesis; the proof
                // of it stands where the citation stood, and must conclude
                // exactly what was cited.
                Ok(Cited::Proof(proof)) => {
                    return (proof.conclusion == self.proposition(&node.conclusion)?)
                        .then_some(proof);
                }
                Err(refused) => {
                    self.refused.replace(Some(refused));
                    return None;
                }
            },
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
                    definition_axioms: witness
                        .definition_axioms
                        .iter()
                        .map(|index| self.cited_axiom(*index))
                        .collect::<Option<_>>()?,
                    literal_axioms: witness
                        .literal_axioms
                        .iter()
                        .map(|index| match index {
                            Some(index) => self.cited_axiom(*index).map(Some),
                            None => Some(None),
                        })
                        .collect::<Option<_>>()?,
                },
            },
            ProofRule::IntegerExactAddDefinitionBound {
                left_bound,
                right_bound,
                definition_axiom,
            } => ProofRule::IntegerExactAddDefinitionBound {
                left_bound: child(left_bound)?,
                right_bound: child(right_bound)?,
                definition_axiom: self.cited_axiom(*definition_axiom)?,
            },
            ProofRule::IntegerExactSubtractDefinitionBound {
                left_bound,
                right_bound,
                definition_axiom,
            } => ProofRule::IntegerExactSubtractDefinitionBound {
                left_bound: child(left_bound)?,
                right_bound: child(right_bound)?,
                definition_axiom: self.cited_axiom(*definition_axiom)?,
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

/// The implementation selected for one machine parameter of a generic body,
/// with the proof that it refines the requirement.
///
/// The proof is a conformance witness: a module whose machine 1 has the
/// requirement's formals, result and contract, and whose body is one call of
/// the implementation with those formals, returning its result. Discharging
/// it is the refinement check, by the route every machine takes: its call
/// requirements say the requirement's `requires` imply the implementation's,
/// and its guarantees say the implementation's `ensures` imply the
/// requirement's (machines.md, "Machines and contract refinement"). It is
/// found once per implementation, not once per use.
pub struct Selection<'a> {
    /// The requirement, as the generic check form names it.
    pub requirement: MachineId,
    pub witness: &'a Elaboration,
    pub refinement: &'a Discharge,
}

/// Carries the certificates of a generic body into one specialization of it
/// and has the kernel decide each again there. No search runs.
///
/// `template` is the body in `generic`, the generic check form, where it was
/// proved once against its requirements' contracts (`found`).
/// `specialization` is the same body in `consumer`, a concrete module in
/// which each machine parameter is the selected implementation. As in
/// `reverify`, each goal and its hypotheses are the consumer's own, computed
/// from its lowered program; only proofs are taken from elsewhere.
///
/// A certificate of the generic body cites what the requirement guarantees
/// after a call. The specialization does not assume that: after the call it
/// knows the implementation's published guarantee. So the citation is
/// replaced by the witness's proof of the requirement's clause, moved to
/// this call: the witness's formals become the call's actuals and its result
/// the call's result. That proof in turn cites the requirement's `requires`,
/// which the generic body proved at the call, and those proofs are carried
/// the same way. The specialization's own call requirements are the
/// implementation's; each is the witness's proof of it, moved likewise.
///
/// Returns one verdict per obligation that takes a certificate, in table
/// order. Nothing here is trusted: a carried proof that does not fit is
/// refused by the kernel, as `Unproved(KernelRefused)`.
pub fn reverify_specialization(
    generic: &Elaboration,
    found: &Discharge,
    template: MachineId,
    consumer: &mut Elaboration,
    specialization: MachineId,
    selections: &[Selection<'_>],
) -> Checked<Vec<(ObligationId, Checked<Evidence>)>> {
    let module = &consumer.module;
    let domains = &consumer.domains;
    let tables = consumer
        .tables
        .machines
        .get_mut(specialization)
        .ok_or_else(|| {
            refusal(
                RefusalReason::ForeignIdentity,
                specialization,
                Construct::Machine,
                "no such machine",
            )
        })?;
    let prepared = crate::prepare(
        &module.machines,
        &module.structural_types,
        domains,
        specialization,
        tables,
        Knowledge::All,
    )?;
    let tables = &*tables;
    let lowered = crate::terminal_machine(&module.machines, specialization, specialization)?;
    let storage = crate::places::Storage::new(
        specialization,
        lowered,
        &module.structural_types,
        domains,
        &prepared.graph,
        tables,
        &prepared.types,
    )?;
    let carry = Carry {
        specialization,
        generic,
        template: terminal_check_tables::row(
            &generic.tables.machines,
            template,
            specialization,
            Construct::Machine,
        )?,
        found: &terminal_check_tables::row(
            &found.machines,
            template,
            specialization,
            Construct::Machine,
        )?
        .verdicts,
        tables,
        lowered,
        selections,
    };
    carry.corresponds()?;
    let mut verdicts = Vec::new();
    for (obligation, row) in tables.obligations.iter() {
        if let ObligationKind::PublishedGuarantee { .. } = row.kind {
            continue;
        }
        let verdict = (|| {
            let goal = crate::goal(
                &module.machines,
                lowered,
                &storage,
                specialization,
                obligation,
                &row.kind,
                tables,
                &prepared.types,
            )?;
            let hypotheses = crate::hypotheses(
                &prepared.graph,
                tables,
                specialization,
                obligation,
                row.created_at,
            )?;
            let proof = carry.proof(obligation, &row.kind, &hypotheses)?;
            let identity = EvidenceIdentity::new(obligation.get()).ok_or_else(|| {
                carry.refuse(
                    RefusalReason::ForeignIdentity,
                    Construct::Obligation(obligation),
                    "obligation identity is not an evidence identity",
                )
            })?;
            crate::verify(
                &prepared.context,
                specialization,
                obligation,
                goal,
                hypotheses,
                CertificateEnvelope {
                    identity,
                    proof_system_marker: ProofSystemMarker::CURRENT,
                    proof,
                },
            )
        })();
        verdicts.push((obligation, verdict));
    }
    Ok(verdicts)
}

/// The proofs a specialization takes from its generic body and from each
/// selected implementation's conformance witness.
struct Carry<'a> {
    specialization: MachineId,
    generic: &'a Elaboration,
    /// The generic body's tables and its verdicts.
    template: &'a MachineTables,
    found: &'a IdTable<ObligationId, Checked<Discharged>>,
    /// The specialization's tables and lowered form.
    tables: &'a MachineTables,
    lowered: &'a TerminalMachine,
    selections: &'a [Selection<'a>],
}

/// The conformance witness's one call, read from its tables.
struct Witness<'a> {
    tables: &'a MachineTables,
    verdicts: &'a IdTable<ObligationId, Checked<Discharged>>,
    invocation: InvocationId,
    /// How many scalar formals the witness forwards, in order.
    formals: usize,
    /// The value its call defines, when the member returns a scalar.
    result: Option<ValueId>,
}

impl Carry<'_> {
    fn refuse(&self, reason: RefusalReason, concerns: Construct, detail: &str) -> Refusal {
        refusal(reason, self.specialization, concerns, detail)
    }

    /// The specialization has the generic body's values and calls, one for
    /// one. Selecting a machine parameter changes only which machine a call
    /// names, so anything else means `specialization` is not that body.
    fn corresponds(&self) -> Checked<()> {
        let same_values = self.tables.values.len() == self.template.values.len()
            && self
                .tables
                .values
                .iter()
                .zip(self.template.values.iter())
                .all(|((_, mine), (_, theirs))| mine == theirs);
        let same_calls = self.tables.invocations.len() == self.template.invocations.len()
            && self
                .tables
                .invocations
                .iter()
                .zip(self.template.invocations.iter())
                .all(|((_, mine), (_, theirs))| {
                    mine.site == theirs.site && mine.actuals == theirs.actuals
                });
        if same_values && same_calls {
            Ok(())
        } else {
            Err(self.refuse(
                RefusalReason::Unsupported,
                Construct::Machine,
                "the specialization's values and calls are not the generic body's",
            ))
        }
    }

    /// The selection for the machine parameter `invocation` calls in the
    /// generic body; `None` when that call names an ordinary machine.
    fn selection(&self, invocation: InvocationId) -> Checked<Option<&Selection<'_>>> {
        let concerns = Construct::Invocation(invocation);
        let row = terminal_check_tables::row(
            &self.template.invocations,
            invocation,
            self.specialization,
            concerns,
        )?;
        let Callee::Machine(callee) = row.callee else {
            return Ok(None);
        };
        if !self.generic.requirements.contains(&callee) {
            return Ok(None);
        }
        self.selections
            .iter()
            .find(|selection| selection.requirement == callee)
            .map(Some)
            .ok_or_else(|| {
                self.refuse(
                    RefusalReason::Unsupported,
                    concerns,
                    "no implementation is selected for this machine parameter",
                )
            })
    }

    fn witness<'s>(
        &self,
        selection: &'s Selection<'_>,
        concerns: Construct,
    ) -> Checked<Witness<'s>> {
        let unsupported = |detail| self.refuse(RefusalReason::Unsupported, concerns, detail);
        let entry = MachineId::new(1).ok_or_else(|| unsupported("machine identity"))?;
        let tables = selection
            .witness
            .tables
            .machines
            .get(entry)
            .ok_or_else(|| unsupported("the conformance witness has no machine"))?;
        let verdicts = &selection
            .refinement
            .machines
            .get(entry)
            .ok_or_else(|| unsupported("the conformance witness was not discharged"))?
            .verdicts;
        let lowered = crate::terminal_machine(&selection.witness.module.machines, entry, entry)?;
        let mut invocations = tables.invocations.iter();
        let (Some((invocation, row)), None) = (invocations.next(), invocations.next()) else {
            return Err(unsupported(
                "the conformance witness is not one call of the implementation",
            ));
        };
        let actuals = crate::scalar_actuals(&row.actuals);
        let forwards = actuals
            .iter()
            .enumerate()
            .all(|(position, actual)| actual.get() == position as u64 + 1)
            && actuals.len() == lowered.parameters.len();
        if !forwards {
            return Err(unsupported(
                "the conformance witness does not pass its formals on in order",
            ));
        }
        let result = match row.site {
            semantic_vocabulary::InvocationSite::Operation(operation) => {
                crate::operation_result(lowered, operation)
            }
            _ => None,
        };
        Ok(Witness {
            tables,
            verdicts,
            invocation,
            formals: actuals.len(),
            result,
        })
    }

    /// The certificate found for the first obligation of `tables` that
    /// `select` picks.
    fn certificate<'t>(
        &self,
        tables: &MachineTables,
        verdicts: &'t IdTable<ObligationId, Checked<Discharged>>,
        select: impl Fn(&ObligationKind) -> bool,
        concerns: Construct,
    ) -> Checked<&'t Evidence> {
        let mut picked = tables
            .obligations
            .iter()
            .filter(|(_, row)| select(&row.kind))
            .map(|(obligation, _)| obligation);
        let (Some(obligation), None) = (picked.next(), picked.next()) else {
            return Err(self.refuse(
                RefusalReason::Unsupported,
                concerns,
                "no single obligation to take a proof from",
            ));
        };
        match verdicts.get(obligation) {
            Some(Ok(Discharged::Certificate(evidence))) => Ok(evidence),
            // Nothing was proved once, so there is nothing to carry: the
            // obligation stays unproved here and nothing searches for it.
            _ => Err(self.refuse(
                RefusalReason::Unproved(UnprovedCause::NoDerivation),
                concerns,
                "no proof of this was found to carry",
            )),
        }
    }

    /// The proof to offer for one obligation of the specialization.
    fn proof(
        &self,
        obligation: ObligationId,
        kind: &ObligationKind,
        target: &Hypotheses,
    ) -> Checked<ProofNode> {
        let concerns = Construct::Obligation(obligation);
        let same: Box<dyn Fn(&ObligationKind) -> bool> = match *kind {
            ObligationKind::CallRequirement { invocation, clause } => {
                if let Some(selection) = self.selection(invocation)? {
                    return self.implementation_requirement(
                        selection, invocation, clause, target, concerns,
                    );
                }
                Box::new(move |other| {
                    *other == ObligationKind::CallRequirement { invocation, clause }
                })
            }
            // The published identity is this machine's own; the exit and
            // the clause say which guarantee it is.
            ObligationKind::NormalGuarantee { exit, clause, .. } => Box::new(move |other| {
                matches!(other, ObligationKind::NormalGuarantee { exit: e, clause: c, .. }
                    if *e == exit && *c == clause)
            }),
            ObligationKind::OperationSafety { .. } | ObligationKind::DataDomain { .. } => {
                let kind = *kind;
                Box::new(move |other| *other == kind)
            }
            _ => {
                return Err(self.refuse(
                    RefusalReason::Unsupported,
                    concerns,
                    "obligation kind outside this route",
                ));
            }
        };
        let evidence = self.certificate(self.template, self.found, same, concerns)?;
        self.carried(evidence, target, concerns)
    }

    /// A certificate of the generic body, moved into the specialization for
    /// an obligation whose hypotheses are `target`. Values keep their
    /// identities; each cited hypothesis becomes the specialization's row
    /// with the same justification, except a requirement's guarantee, which
    /// the specialization does not assume and the witness proves.
    fn carried(
        &self,
        evidence: &Evidence,
        target: &Hypotheses,
        concerns: Construct,
    ) -> Checked<ProofNode> {
        let cited = |rows: &[KnowledgeId], index: usize| {
            rows.get(index).copied().ok_or_else(|| {
                self.refuse(
                    RefusalReason::ForeignIdentity,
                    concerns,
                    "the proof cites a hypothesis its evidence does not list",
                )
            })
        };
        Transport {
            value: &|value, scalar_type| {
                (self.tables.values.get(value)?.scalar_type == scalar_type).then_some(value)
            },
            assumption: &|index| {
                let row = cited(&evidence.assumptions, index)?;
                if let Some(Justification::CalleeGuarantee { invocation, clause }) = self
                    .template
                    .knowledge
                    .get(row)
                    .map(|row| row.justification)
                    && let Some(selection) = self.selection(invocation)?
                {
                    return self
                        .requirement_guarantee(selection, invocation, clause, target, concerns)
                        .map(Cited::Proof);
                }
                self.position(row, &target.assumption_rows, concerns)
                    .map(Cited::Index)
            },
            axiom: &|index| {
                self.position(
                    cited(&evidence.axioms, index)?,
                    &target.axiom_rows,
                    concerns,
                )
            },
            refused: std::cell::RefCell::new(None),
        }
        .certificate(&evidence.certificate.proof)
        .map_err(|refused| {
            refused.unwrap_or_else(|| {
                self.refuse(
                    RefusalReason::Unsupported,
                    concerns,
                    "the generic body's proof names a value or rule that cannot be carried",
                )
            })
        })
    }

    /// Where, among `rows`, the specialization holds the hypothesis that the
    /// generic body's knowledge row `row` is: the row with the same
    /// justification, and the same place among the rows that share it.
    fn position(
        &self,
        row: KnowledgeId,
        rows: &[KnowledgeId],
        concerns: Construct,
    ) -> Checked<usize> {
        let unavailable = || {
            self.refuse(
                RefusalReason::Unproved(UnprovedCause::NoDerivation),
                concerns,
                "a hypothesis the generic proof cites is not available here",
            )
        };
        let justification = self
            .template
            .knowledge
            .get(row)
            .ok_or_else(unavailable)?
            .justification;
        // ponytail: two scans of the knowledge tables per citation; a
        // per-justification column if machines grow.
        let nth = self
            .template
            .knowledge
            .iter()
            .filter(|(_, other)| other.justification == justification)
            .position(|(other, _)| other == row)
            .ok_or_else(unavailable)?;
        let mine = self
            .tables
            .knowledge
            .iter()
            .filter(|(_, other)| other.justification == justification)
            .map(|(other, _)| other)
            .nth(nth)
            .ok_or_else(unavailable)?;
        rows.iter()
            .position(|candidate| *candidate == mine)
            .ok_or_else(unavailable)
    }

    /// The specialization's call of `invocation`: its scalar actuals and the
    /// value it defines.
    fn call(&self, invocation: InvocationId) -> Checked<(Vec<ValueId>, Option<ValueId>)> {
        let row = terminal_check_tables::row(
            &self.tables.invocations,
            invocation,
            self.specialization,
            Construct::Invocation(invocation),
        )?;
        let result = match row.site {
            semantic_vocabulary::InvocationSite::Operation(operation) => {
                crate::operation_result(self.lowered, operation)
            }
            _ => None,
        };
        Ok((crate::scalar_actuals(&row.actuals), result))
    }

    /// A proof that the requirement's `ensures` clause `clause` holds of the
    /// specialization's call `invocation`: the witness's proof of that
    /// clause, with its formals this call's actuals and its result this
    /// call's result. Where the witness cites the implementation's
    /// guarantee, the specialization assumes the same after its call; where
    /// it cites the requirement's `requires`, the generic body proved that
    /// at this call.
    fn requirement_guarantee(
        &self,
        selection: &Selection<'_>,
        invocation: InvocationId,
        clause: u32,
        target: &Hypotheses,
        concerns: Construct,
    ) -> Checked<ProofNode> {
        let witness = self.witness(selection, concerns)?;
        let evidence = self.certificate(
            witness.tables,
            witness.verdicts,
            |kind| matches!(kind, ObligationKind::NormalGuarantee { clause: c, .. } if *c == clause),
            concerns,
        )?;
        self.moved_from_witness(&witness, evidence, invocation, target, concerns)
    }

    /// A proof of the implementation's `requires` clause `clause` at the
    /// specialization's call `invocation`: the witness's proof that the
    /// requirement's `requires` imply it, moved to this call.
    fn implementation_requirement(
        &self,
        selection: &Selection<'_>,
        invocation: InvocationId,
        clause: u32,
        target: &Hypotheses,
        concerns: Construct,
    ) -> Checked<ProofNode> {
        let witness = self.witness(selection, concerns)?;
        let evidence = self.certificate(
            witness.tables,
            witness.verdicts,
            |kind| matches!(kind, ObligationKind::CallRequirement { clause: c, .. } if *c == clause),
            concerns,
        )?;
        self.moved_from_witness(&witness, evidence, invocation, target, concerns)
    }

    fn moved_from_witness(
        &self,
        witness: &Witness<'_>,
        evidence: &Evidence,
        invocation: InvocationId,
        target: &Hypotheses,
        concerns: Construct,
    ) -> Checked<ProofNode> {
        let (actuals, result) = self.call(invocation)?;
        let unsupported = |detail| self.refuse(RefusalReason::Unsupported, concerns, detail);
        Transport {
            value: &|value, scalar_type| {
                let position = usize::try_from(value.get() - 1).ok()?;
                let mine = if position < witness.formals {
                    *actuals.get(position)?
                } else if Some(value) == witness.result {
                    result?
                } else {
                    return None;
                };
                (self.tables.values.get(mine)?.scalar_type == scalar_type).then_some(mine)
            },
            assumption: &|index| {
                let row = evidence.assumptions.get(index).copied().ok_or_else(|| {
                    unsupported(
                        "the refinement proof cites a hypothesis its evidence does not list",
                    )
                })?;
                match witness
                    .tables
                    .knowledge
                    .get(row)
                    .map(|row| row.justification)
                {
                    // The requirement's `requires`, which the generic body
                    // proved of these actuals before this call.
                    Some(Justification::EntryRequirement { clause }) => {
                        let evidence = self.certificate(
                            self.template,
                            self.found,
                            |kind| *kind == ObligationKind::CallRequirement { invocation, clause },
                            concerns,
                        )?;
                        self.carried(evidence, target, concerns).map(Cited::Proof)
                    }
                    // The implementation's guarantee after the witness's
                    // call, which the specialization assumes after its own.
                    Some(Justification::CalleeGuarantee {
                        invocation: called,
                        clause,
                    }) if called == witness.invocation => {
                        let mine = self.tables.knowledge.iter().find(|(_, row)| {
                            row.justification
                                == Justification::CalleeGuarantee { invocation, clause }
                        });
                        mine.and_then(|(mine, _)| {
                            target
                                .assumption_rows
                                .iter()
                                .position(|candidate| *candidate == mine)
                        })
                        .map(Cited::Index)
                        .ok_or_else(|| {
                            self.refuse(
                                RefusalReason::Unproved(UnprovedCause::NoDerivation),
                                concerns,
                                "the implementation's guarantee is not available here",
                            )
                        })
                    }
                    _ => Err(unsupported(
                        "the refinement proof cites something other than the two contracts",
                    )),
                }
            },
            // The witness has one operation, its call, which has no result
            // equation; a proof that cites one is not a refinement proof.
            axiom: &|_| Err(unsupported("the refinement proof cites an operation")),
            refused: std::cell::RefCell::new(None),
        }
        .certificate(&evidence.certificate.proof)
        .map_err(|refused| {
            refused.unwrap_or_else(|| {
                unsupported("the refinement proof names a value that is not a formal or the result")
            })
        })
    }
}
