/*
Reusing a saved proof. A consumer re-decides a proof found elsewhere instead of
searching for one. The compile reaches this module in one place: a machine
stage 04 generated from a generic machine takes its certificates from that
machine's check form (`Carried`, decided in `discharge`).

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
use terminal_check_tables::edges::Callee;
use terminal_check_tables::obligations::ObligationKind;
use terminal_check_tables::program::Elaboration;
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
    let prepared = crate::prepare(module, domains, machine, tables, Knowledge::All)?;
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
        module,
        domains,
        &prepared.graph,
        tables,
        &prepared.types,
    )?;
    let goal = crate::goal(
        module,
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
    let terminal_psi::TargetMachineCallable::Machine(callee) =
        crate::callee(&elaboration.module, caller, invocation, row)?
    else {
        return Err(refuse(
            RefusalReason::Unsupported,
            "an interface contract has no body proof to reuse",
        ));
    };
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
        value: &|value, scalar_type| {
            formal_actual(&actuals, tables, value, scalar_type).map(Moved::Value)
        },
        assumption: &|index, _| Ok(Cited::Index(index)),
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

/// What a value a certificate names becomes where the certificate is moved
/// to: the destination's value, or, for a value parameter the destination
/// has bound, the literal it is bound to.
enum Moved {
    Value(ValueId),
    Literal(semantic_vocabulary::IntegerValue),
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
    value: &'a dyn Fn(ValueId, ScalarType) -> Option<Moved>,
    /// A cited assumption's position at the source, with what it concludes
    /// once moved.
    assumption: &'a dyn Fn(usize, &Proposition) -> Checked<Cited>,
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

    fn value(&self, value: ValueId, scalar_type: ScalarType) -> Option<Moved> {
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
            ScalarTerm::Value { id, scalar_type } => match self.value(*id, *scalar_type)? {
                Moved::Value(value) => ScalarTerm::value(value, *scalar_type),
                Moved::Literal(value) => ScalarTerm::Integer {
                    scalar_type: scalar_type.as_integer()?,
                    value,
                },
            },
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
            IntegerMathTerm::MathValue { source_type, value } => {
                match self.value(*value, ScalarType::Integer(*source_type))? {
                    Moved::Value(value) => IntegerMathTerm::MathValue {
                        source_type: *source_type,
                        value,
                    },
                    // The mathematical integer a literal embeds as, which is
                    // how the kernel denotes a literal operand.
                    Moved::Literal(value) => IntegerMathTerm::embed(ScalarTerm::Integer {
                        scalar_type: *source_type,
                        value,
                    })?,
                }
            }
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
            ProofRule::Assumption { index } => {
                match (self.assumption)(*index, &self.proposition(&node.conclusion)?) {
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
                }
            }
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

/// The check forms a machine stage 04 generated may take its certificates
/// from: the forms of the generic machine it specializes, one, or one per
/// alternative of a finite family. Machine 1 of each form is the generic
/// body (`typed_trees_to_terminal_psi::elaborate_generic`). The first form the
/// specialization corresponds to is the one it is; which form that is is
/// decided by the correspondence of the two lowered bodies
/// (`Carry::corresponds`), never by a type's spelling.
pub struct Carried<'a> {
    pub specialization: MachineId,
    pub forms: Vec<CarriedForm<'a>>,
}

/// One check form, discharged once against its requirements alone, and the
/// selection made for each machine parameter it names.
pub struct CarriedForm<'a> {
    pub generic: &'a Elaboration,
    pub found: &'a Discharge,
    pub selections: Vec<Selection<'a>>,
}

impl Carried<'_> {
    /// The carrier for this specialization from the first form it
    /// corresponds to; `None` when it corresponds to none. Then nothing is
    /// carried and the machine is decided as any closed machine is.
    pub(crate) fn carrier<'s>(
        &'s self,
        tables: &'s MachineTables,
        lowered: &'s TerminalMachine,
    ) -> Option<Carry<'s>> {
        let template = MachineId::new(1)?;
        self.forms.iter().find_map(|form| {
            Carry::new(
                form.generic,
                form.found,
                template,
                self.specialization,
                &form.selections,
                tables,
                lowered,
            )
            .ok()
        })
    }
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
/// A certificate cites a judgment of the requirement's call. A routed
/// specialization keeps that public judgment, so its own identical call fact
/// is used directly. When the consumer instead exposes a different contract,
/// the existing refinement proof must derive the exact missing judgment.
/// Witness formals become this call's actuals and its result becomes this
/// call's result. Clause ordinals in different contracts are never treated as
/// correspondence; the final kernel still checks every moved conclusion and
/// assumption against this consumer's actual goal and hypotheses.
///
/// A value parameter of the generic body (`const Units: u64`) is one of its
/// formals there and a literal in the specialization, which stage 04 put
/// wherever the body named the binder. So each such formal in a carried
/// proof becomes that literal, and every other value keeps its place among
/// the values. Which literal is worked out here, from the two lowered
/// contracts alone (`Instantiation::of`): it is the one at which the generic
/// contract is the specialization's. The typed trees record the argument
/// only as its spelling, which is not read. The literal is only what the
/// proof is instantiated at: the goals and hypotheses it must then fit are
/// the specialization's own, so a wrong one yields a proof the kernel
/// refuses.
///
/// Returns only conditional mathematical evidence per obligation, not an
/// accepted compilation or proof of a selected implementation's identity.
/// `judge_compilation` independently judges every actual conformance witness
/// emitted by source preparation; carrying cannot replace that admission.
/// A proof with no matching premise refuses before kernel entry, and every
/// reconstructed candidate still reaches the consumer's actual kernel goal.
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
    let prepared = crate::prepare(module, domains, specialization, tables, Knowledge::All)?;
    let tables = &*tables;
    let lowered = crate::terminal_machine(&module.machines, specialization, specialization)?;
    let storage = crate::places::Storage::new(
        specialization,
        lowered,
        module,
        domains,
        &prepared.graph,
        tables,
        &prepared.types,
    )?;
    let carry = Carry::new(
        generic,
        found,
        template,
        specialization,
        selections,
        tables,
        lowered,
    )?;
    let mut verdicts = Vec::new();
    for (obligation, row) in tables.obligations.iter() {
        if let ObligationKind::PublishedGuarantee { .. } = row.kind {
            continue;
        }
        let verdict = crate::goal(
            module,
            lowered,
            &storage,
            specialization,
            obligation,
            &row.kind,
            tables,
            &prepared.types,
        )
        .and_then(|goal| {
            carry.certify(
                &prepared.context,
                &prepared.graph,
                obligation,
                &row.kind,
                row.created_at,
                goal,
            )
        });
        verdicts.push((obligation, verdict));
    }
    Ok(verdicts)
}

/// How the values of a generic body are the values of one specialization of
/// it. The generic body's scalar formals are the specialization's
/// (`authored` of them), then its value parameters; every value after those
/// is the specialization's value that many places earlier.
struct Instantiation {
    /// The concrete type the specialization has for each type parameter the
    /// generic body leaves rigid, read off the two machines' formals and
    /// result: each formal of a rigid type there is a formal of this type
    /// here, the same type at every one of them.
    types: Vec<(terminal_check_tables::values::RigidType, ScalarType)>,
    authored: usize,
    /// The literal each value parameter is in the specialization, in
    /// declaration order; `None` for one neither contract mentions, which
    /// no proof can then be moved through.
    bound: Vec<Option<ScalarTerm>>,
}

impl Instantiation {
    /// Works the instantiation out from the two lowered contracts: the
    /// literals at which each `ensures` clause of `generic` is the
    /// specialization's clause at the same position, and its `requires`
    /// clauses are the specialization's, in some order (they are held in
    /// canonical order, which a literal in place of a formal can change).
    /// Refused when there is no such instantiation: `specialization` is then
    /// not that body's contract at any value of its parameters.
    fn of(
        generic: &TerminalMachine,
        generic_tables: &MachineTables,
        rigid: usize,
        specialization: &TerminalMachine,
        tables: &MachineTables,
        machine: MachineId,
    ) -> Checked<Self> {
        let refuse = |detail| {
            Err(refusal(
                RefusalReason::Unsupported,
                machine,
                Construct::Machine,
                detail,
            ))
        };
        let authored = specialization.parameters.len();
        if generic.parameters.len() != authored + rigid {
            return refuse("the specialization's formals are not the generic body's");
        }
        let mut found = Self {
            types: Vec::new(),
            authored,
            bound: vec![None; rigid],
        };
        // The type each rigid type parameter is here: the type of the
        // specialization's value in the place of each value of it. Two
        // values of one parameter must agree, or the specialization is not
        // one application of the body.
        for (value, theirs) in generic_tables.values.iter() {
            let Some(parameter) = theirs.rigid else {
                continue;
            };
            let Some(mine) = (match found.moved(value) {
                Some(Moved::Value(mine)) => tables.values.get(mine),
                Some(Moved::Literal(_)) | None => None,
            }) else {
                return refuse("a value of a type parameter has no value in the specialization");
            };
            match found.types.iter().find(|(bound, _)| *bound == parameter) {
                None => found.types.push((parameter, mine.scalar_type)),
                Some((_, scalar_type)) if *scalar_type == mine.scalar_type => {}
                Some(_) => {
                    return refuse("one type parameter is two types in the specialization");
                }
            }
        }
        if rigid == 0 {
            return Ok(found);
        }
        let (theirs, mine) = (&generic.contract, &specialization.contract);
        if theirs.ensures.len() != mine.ensures.len()
            || theirs.requires.len() != mine.requires.len()
        {
            return refuse("the specialization's contract is not the generic body's");
        }
        for (theirs, mine) in theirs.ensures.iter().zip(&mine.ensures) {
            if !found.unify(tables, &theirs.proposition, &mine.proposition) {
                return refuse("a guarantee of the specialization is not the generic body's");
            }
        }
        // ponytail: tries every pairing of the `requires` clauses, which is
        // factorial in their number; contracts hold a handful.
        if theirs.requires.len() > 8 {
            return refuse("too many requires clauses to pair");
        }
        let mut taken = vec![false; mine.requires.len()];
        if !found.pair(tables, &theirs.requires, &mine.requires, &mut taken) {
            return refuse("the requirements of the specialization are not the generic body's");
        }
        Ok(found)
    }

    /// Pairs each of `theirs` with a distinct clause of `mine` it is under
    /// one instantiation, extending `self.bound`; restores it on failure.
    fn pair(
        &mut self,
        tables: &MachineTables,
        theirs: &[Proposition],
        mine: &[Proposition],
        taken: &mut [bool],
    ) -> bool {
        let Some((first, rest)) = theirs.split_first() else {
            return true;
        };
        for candidate in 0..mine.len() {
            if taken[candidate] {
                continue;
            }
            let saved = self.bound.clone();
            taken[candidate] = true;
            if self.unify(tables, first, &mine[candidate]) && self.pair(tables, rest, mine, taken) {
                return true;
            }
            taken[candidate] = false;
            self.bound = saved;
        }
        false
    }

    /// Whether `theirs` is `mine` once each value parameter is a literal,
    /// binding the parameters `theirs` names to the literals `mine` has in
    /// their place. A parameter already bound must agree.
    fn unify(&mut self, tables: &MachineTables, theirs: &Proposition, mine: &Proposition) -> bool {
        let (mut generic_leaves, mut leaves) = (Vec::new(), Vec::new());
        if !leaf_terms(theirs, &mut generic_leaves) || !leaf_terms(mine, &mut leaves) {
            return false;
        }
        if generic_leaves.len() != leaves.len() {
            return false;
        }
        for (theirs, mine) in generic_leaves.into_iter().zip(leaves) {
            let (
                ScalarTerm::Value { id, scalar_type },
                ScalarTerm::Integer {
                    scalar_type: literal_type,
                    ..
                },
            ) = (theirs, mine)
            else {
                continue;
            };
            let Some(parameter) = usize::try_from(id.get() - 1)
                .ok()
                .and_then(|position| position.checked_sub(self.authored))
                .filter(|parameter| *parameter < self.bound.len())
            else {
                continue;
            };
            if *scalar_type != ScalarType::Integer(*literal_type) {
                return false;
            }
            match &self.bound[parameter] {
                None => self.bound[parameter] = Some(mine.clone()),
                Some(bound) if bound == mine => {}
                Some(_) => return false,
            }
        }
        // The leaves only propose the literals. That the whole clause is the
        // specialization's is decided by moving it.
        let moved = Transport {
            value: &|value, scalar_type| self.moved_as(tables, value, scalar_type),
            assumption: &|index, _| Ok(Cited::Index(index)),
            axiom: &|index| Ok(index),
            refused: std::cell::RefCell::new(None),
        }
        .proposition(theirs);
        moved.as_ref() == Some(mine)
    }

    /// What the generic body's value `value` is in the specialization.
    fn moved(&self, value: ValueId) -> Option<Moved> {
        let position = usize::try_from(value.get() - 1).ok()?;
        if position < self.authored {
            return Some(Moved::Value(value));
        }
        match self.bound.get(position - self.authored) {
            Some(Some(ScalarTerm::Integer { value, .. })) => Some(Moved::Literal(*value)),
            Some(_) => None,
            None => ValueId::new(value.get() - self.bound.len() as u64).map(Moved::Value),
        }
    }

    /// `moved`, for a value that must have the type `scalar_type` there.
    fn moved_as(
        &self,
        tables: &MachineTables,
        value: ValueId,
        scalar_type: ScalarType,
    ) -> Option<Moved> {
        match self.moved(value)? {
            Moved::Value(mine) => {
                (tables.values.get(mine)?.scalar_type == scalar_type).then_some(Moved::Value(mine))
            }
            Moved::Literal(literal) => {
                let position = usize::try_from(value.get() - 1).ok()? - self.authored;
                let Some(ScalarTerm::Integer {
                    scalar_type: bound_type,
                    ..
                }) = self.bound.get(position)?
                else {
                    return None;
                };
                (ScalarType::Integer(*bound_type) == scalar_type).then_some(Moved::Literal(literal))
            }
        }
    }
}

/// The value and literal terms of a contract clause, in one fixed order of
/// its structure; `false` for a form a contract clause of this subset does
/// not have. Two clauses of one shape list their terms position for
/// position.
fn leaf_terms<'p>(proposition: &'p Proposition, out: &mut Vec<&'p ScalarTerm>) -> bool {
    fn term<'p>(term: &'p ScalarTerm, out: &mut Vec<&'p ScalarTerm>) -> bool {
        match term {
            ScalarTerm::Value { .. } | ScalarTerm::Integer { .. } | ScalarTerm::Boolean(_) => {
                out.push(term);
                true
            }
            _ => false,
        }
    }
    match proposition {
        Proposition::Truth | Proposition::Falsehood => true,
        Proposition::Equal(left, right)
        | Proposition::LessThan(left, right)
        | Proposition::LessOrEqual(left, right) => term(left, out) && term(right, out),
        Proposition::Conjunction(parts) | Proposition::Disjunction(parts) => {
            parts.iter().all(|part| leaf_terms(part, out))
        }
        _ => false,
    }
}

/// The proofs a specialization takes from its generic body and from each
/// selected implementation's conformance witness.
pub(crate) struct Carry<'a> {
    specialization: MachineId,
    instantiation: Instantiation,
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

impl<'a> Carry<'a> {
    /// The carrier of `generic`'s certificates for its machine `template`
    /// into `specialization`, whose tables and lowered form are given.
    /// Refused as `Unsupported` when `specialization` is not that body
    /// (`corresponds`) or its contract is not the body's at any value of the
    /// body's parameters (`Instantiation::of`).
    #[allow(clippy::too_many_arguments)]
    fn new(
        generic: &'a Elaboration,
        found: &'a Discharge,
        template: MachineId,
        specialization: MachineId,
        selections: &'a [Selection<'a>],
        tables: &'a MachineTables,
        lowered: &'a TerminalMachine,
    ) -> Checked<Self> {
        let rigid = generic.value_parameters.get(template).copied().unwrap_or(0);
        let generic_tables = terminal_check_tables::row(
            &generic.tables.machines,
            template,
            specialization,
            Construct::Machine,
        )?;
        let instantiation = Instantiation::of(
            crate::terminal_machine(&generic.module.machines, template, specialization)?,
            generic_tables,
            rigid,
            lowered,
            tables,
            specialization,
        )?;
        let carry = Carry {
            specialization,
            instantiation,
            generic,
            template: generic_tables,
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
        Ok(carry)
    }

    /// Decides one obligation of the specialization from the carried
    /// proof: the goal is the specialization's own, the hypotheses are
    /// those its graph makes available at `point`, and the kernel decides
    /// the moved certificate against them. No search runs.
    pub(crate) fn certify(
        &self,
        context: &PropositionContext,
        graph: &crate::graph::Graph,
        obligation: ObligationId,
        kind: &ObligationKind,
        point: terminal_check_tables::ProgramPoint,
        goal: Proposition,
    ) -> Checked<Evidence> {
        let hypotheses =
            crate::hypotheses(graph, self.tables, self.specialization, obligation, point)?;
        let proof = self.proof(obligation, kind, &goal, &hypotheses)?;
        let identity = EvidenceIdentity::new(obligation.get()).ok_or_else(|| {
            self.refuse(
                RefusalReason::ForeignIdentity,
                Construct::Obligation(obligation),
                "obligation identity is not an evidence identity",
            )
        })?;
        crate::verify(
            context,
            self.specialization,
            obligation,
            goal,
            hypotheses,
            CertificateEnvelope {
                identity,
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof,
            },
        )
    }
}

impl Carry<'_> {
    fn refuse(&self, reason: RefusalReason, concerns: Construct, detail: &str) -> Refusal {
        refusal(reason, self.specialization, concerns, detail)
    }

    /// Whether the specialization's value row `mine` is the generic body's
    /// row `theirs`. A value of a type parameter there is, here, a value of
    /// the one concrete type the specialization has for that parameter
    /// (`Instantiation::types`); every other row is the same row.
    fn same_value(
        &self,
        theirs: &terminal_check_tables::values::ValueRow,
        mine: &terminal_check_tables::values::ValueRow,
    ) -> bool {
        match theirs.rigid {
            None => theirs == mine,
            Some(rigid) => {
                mine.rigid.is_none()
                    && mine.origin == theirs.origin
                    && self.instantiation.types.iter().any(|(bound, scalar_type)| {
                        *bound == rigid && *scalar_type == mine.scalar_type
                    })
            }
        }
    }

    /// The specialization has the generic body's values and calls, one for
    /// one, except the value parameters it has bound. Selecting a machine
    /// parameter changes only which machine a call names, and binding a
    /// value parameter only removes its formal, so anything else means
    /// `specialization` is not that body. A body that reads a value
    /// parameter as an operand differs here: the specialization evaluates a
    /// constant where the generic body reads its formal, so it has a value
    /// the generic body does not, and is refused.
    fn corresponds(&self) -> Checked<()> {
        let same_values = self.tables.values.len() + self.instantiation.bound.len()
            == self.template.values.len()
            && self.template.values.iter().all(|(value, theirs)| {
                match self.instantiation.moved(value) {
                    Some(Moved::Value(mine)) => self
                        .tables
                        .values
                        .get(mine)
                        .is_some_and(|mine| self.same_value(theirs, mine)),
                    // A value parameter is a formal there, whether or not
                    // the contract mentions it.
                    Some(Moved::Literal(_)) | None => matches!(
                        theirs.origin,
                        terminal_check_tables::values::ValueOrigin::MachineParameter { .. }
                    ),
                }
            });
        let same_actuals =
            |mine: &[terminal_check_tables::edges::Actual],
             theirs: &[terminal_check_tables::edges::Actual]| {
                use terminal_check_tables::edges::Actual;
                mine.len() == theirs.len()
                    && mine.iter().zip(theirs).all(|pair| match pair {
                        (Actual::Value(mine), Actual::Value(theirs)) => {
                            matches!(
                                self.instantiation.moved(*theirs),
                                Some(Moved::Value(moved)) if moved == *mine
                            )
                        }
                        (mine, theirs) => mine == theirs,
                    })
            };
        let same_calls = self.tables.invocations.len() == self.template.invocations.len()
            && self
                .tables
                .invocations
                .iter()
                .zip(self.template.invocations.iter())
                .all(|((_, mine), (_, theirs))| {
                    mine.site == theirs.site && same_actuals(&mine.actuals, &theirs.actuals)
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
        let Callee::Declared(semantic_vocabulary::TargetMachineCallable::Machine(callee)) =
            row.callee
        else {
            return Ok(None);
        };
        // A generic machine applied at this body's type parameters is held
        // to its own contract, which is the contract of the machine the
        // specialization calls: nothing is selected for it, and what a
        // proof cites of it is found in the specialization like any
        // callee's.
        if !self.generic.requirements.contains(&callee)
            || self.generic.applications.contains(&callee)
        {
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
        goal: &Proposition,
        target: &Hypotheses,
    ) -> Checked<ProofNode> {
        let concerns = Construct::Obligation(obligation);
        let same: Box<dyn Fn(&ObligationKind) -> bool> = match *kind {
            ObligationKind::CallRequirement { invocation, .. } => {
                return self.call_requirement(invocation, goal, target, concerns);
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
    /// with the same judgment. An absent public call fact must instead be
    /// derived by its selected member's existing refinement proof.
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
                self.instantiation.moved_as(self.tables, value, scalar_type)
            },
            assumption: &|index, concluded| {
                let row = cited(&evidence.assumptions, index)?;
                let justification = self
                    .template
                    .knowledge
                    .get(row)
                    .map(|row| row.justification);
                if let Some(Justification::CalleeGuarantee { invocation, .. }) = justification {
                    if let Some(index) = self.call_assumption(invocation, concluded, target) {
                        return Ok(Cited::Index(index));
                    }
                    if let Some(selection) = self.selection(invocation)? {
                        return self
                            .requirement_guarantee(
                                selection, invocation, concluded, target, concerns,
                            )
                            .map(Cited::Proof);
                    }
                    return Err(self.refuse(
                        RefusalReason::Unproved(UnprovedCause::NoDerivation),
                        concerns,
                        "the call does not retain the guarantee this proof cites",
                    ));
                }
                // An entry `requires` clause is found by what it says once
                // moved, not by its position: the clauses are in canonical
                // order, which a literal in place of a value parameter can
                // change.
                if let Some(Justification::EntryRequirement { .. }) = justification {
                    return self.entry_requirement(concluded, target, concerns);
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

    /// Where the specialization assumes the entry `requires` clause that
    /// says `concluded`.
    fn entry_requirement(
        &self,
        concluded: &Proposition,
        target: &Hypotheses,
        concerns: Construct,
    ) -> Checked<Cited> {
        target
            .assumption_rows
            .iter()
            .zip(&target.assumptions)
            .position(|(row, assumed)| {
                assumed == concluded
                    && matches!(
                        self.tables.knowledge.get(*row).map(|row| row.justification),
                        Some(Justification::EntryRequirement { .. })
                    )
            })
            .map(Cited::Index)
            .ok_or_else(|| {
                self.refuse(
                    RefusalReason::Unproved(UnprovedCause::NoDerivation),
                    concerns,
                    "an entry requirement the generic proof cites is not assumed here",
                )
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

    /// A public call may already expose the generic requirement's judgment.
    /// Otherwise its refinement proof must derive the actual input judgment.
    /// Clause positions in the public and implementing contracts need not agree.
    fn call_requirement(
        &self,
        invocation: InvocationId,
        goal: &Proposition,
        target: &Hypotheses,
        concerns: Construct,
    ) -> Checked<ProofNode> {
        let selection = self.selection(invocation)?;
        let called = terminal_check_tables::row(
            &self.template.invocations,
            invocation,
            self.specialization,
            concerns,
        )?;
        let public = self.matching_proof(
            self.found,
            called.requirements.iter().copied(),
            goal,
            concerns,
            |evidence| self.carried(evidence, target, concerns),
        );
        match (public, selection) {
            (Ok(proof), _) => Ok(proof),
            (Err(_), Some(selection)) => {
                let witness = self.witness(selection, concerns)?;
                let called = terminal_check_tables::row(
                    &witness.tables.invocations,
                    witness.invocation,
                    self.specialization,
                    concerns,
                )?;
                self.matching_proof(
                    witness.verdicts,
                    called.requirements.iter().copied(),
                    goal,
                    concerns,
                    |evidence| {
                        self.moved_from_witness(&witness, evidence, invocation, target, concerns)
                    },
                )
            }
            (Err(refusal), None) => Err(refusal),
        }
    }

    // Only a fact on this invocation with this exact moved proposition can
    // replace a cited call fact. An equal clause ordinal is not correspondence.
    fn call_assumption(
        &self,
        invocation: InvocationId,
        concluded: &Proposition,
        target: &Hypotheses,
    ) -> Option<usize> {
        target
            .assumption_rows
            .iter()
            .zip(&target.assumptions)
            .position(|(row, proposition)| {
                proposition == concluded
                    && matches!(
                        self.tables.knowledge.get(*row).map(|row| row.justification),
                        Some(Justification::CalleeGuarantee { invocation: actual, .. })
                            if actual == invocation
                    )
            })
    }

    // Match an already-found certificate to the consumer's own judgment.
    // This reconstructs known proofs; it never starts a prover or changes a goal.
    fn matching_proof(
        &self,
        verdicts: &IdTable<ObligationId, Checked<Discharged>>,
        obligations: impl Iterator<Item = ObligationId>,
        goal: &Proposition,
        concerns: Construct,
        moved: impl Fn(&Evidence) -> Checked<ProofNode>,
    ) -> Checked<ProofNode> {
        let mut matched = None;
        let mut refused = None;
        for obligation in obligations {
            let Some(Ok(Discharged::Certificate(evidence))) = verdicts.get(obligation) else {
                continue;
            };
            match moved(evidence) {
                Ok(proof) if proof.conclusion == *goal => {
                    if matched.replace(proof).is_some() {
                        return Err(self.refuse(
                            RefusalReason::Unsupported,
                            concerns,
                            "more than one certificate matches the call judgment",
                        ));
                    }
                }
                Ok(_) => {}
                Err(refusal) => refused = Some(refusal),
            }
        }
        matched.ok_or_else(|| {
            refused.unwrap_or_else(|| {
                self.refuse(
                    RefusalReason::Unproved(UnprovedCause::NoDerivation),
                    concerns,
                    "no carried certificate matches the call's exact judgment",
                )
            })
        })
    }

    fn requirement_guarantee(
        &self,
        selection: &Selection<'_>,
        invocation: InvocationId,
        goal: &Proposition,
        target: &Hypotheses,
        concerns: Construct,
    ) -> Checked<ProofNode> {
        let witness = self.witness(selection, concerns)?;
        self.matching_proof(
            witness.verdicts,
            witness
                .tables
                .obligations
                .iter()
                .filter_map(|(obligation, row)| {
                    matches!(row.kind, ObligationKind::NormalGuarantee { .. }).then_some(obligation)
                }),
            goal,
            concerns,
            |evidence| self.moved_from_witness(&witness, evidence, invocation, target, concerns),
        )
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
                (self.tables.values.get(mine)?.scalar_type == scalar_type)
                    .then_some(Moved::Value(mine))
            },
            assumption: &|index, concluded| {
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
                    Some(Justification::EntryRequirement { .. }) => {
                        let called = terminal_check_tables::row(
                            &self.template.invocations,
                            invocation,
                            self.specialization,
                            concerns,
                        )?;
                        self.matching_proof(
                            self.found,
                            called.requirements.iter().copied(),
                            concluded,
                            concerns,
                            |evidence| self.carried(evidence, target, concerns),
                        )
                        .map(Cited::Proof)
                    }
                    // This witness premise must actually be available at
                    // the consumer's call. A same-numbered public clause
                    // cannot replace a stronger implementation premise.
                    Some(Justification::CalleeGuarantee {
                        invocation: called, ..
                    }) if called == witness.invocation => self
                        .call_assumption(invocation, concluded, target)
                        .map(Cited::Index)
                        .ok_or_else(|| {
                            self.refuse(
                                RefusalReason::Unproved(UnprovedCause::NoDerivation),
                                concerns,
                                "the implementation's exact guarantee is not available here",
                            )
                        }),
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
