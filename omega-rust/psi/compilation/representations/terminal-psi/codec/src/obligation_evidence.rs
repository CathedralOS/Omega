/*
Evidence seals: the canonical bytes and fingerprint of one obligation's
accepted evidence, so a later stage can check that a fact it uses came with a
certificate the proof kernel accepted (`wiki/drafts/designs/stage_trust.md`,
rule 2).

A seal holds the obligation's identity, its goal, the hypotheses passed to
the kernel and the certificate, and a SHA-256 fingerprint over their canonical
encoding. Anyone holding the seal can recompute the fingerprint; a seal whose
parts were changed after sealing no longer matches it.

Seals travel in memory beside the Terminal module. They are not part of the
module's bytes: Terminal Psi's layout (`wiki/spec/terminal-psi/encoding.md`)
has no place for them yet. A seal is therefore lost on any byte round trip and
does not survive a stored artifact.
*/

use semantic_vocabulary::{MachineId, ObligationId, Proposition};
use terminal_psi::{
    CertificateEnvelope, CorrelatedAffineBranchWitness, PrimitiveJudgment, ProofNode, ProofRule,
};

use crate::CodecError;
use crate::semantic_module::{Writer, encode_proposition, encode_scalar_term};
use crate::term_table::{TableChildren, TableTerm, encode_term_table};
use crate::term_wire::integer_math_term_wire::encode_integer_math_term;

const DOMAIN: &[u8] = b"psi-obligation-evidence-fingerprint\0";

/// One accepted obligation's evidence and its fingerprint.
///
/// In memory only: it is not encoded in the Terminal module, so it is lost on
/// a byte round trip and does not survive a stored artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObligationEvidenceSeal {
    pub machine: MachineId,
    pub obligation: ObligationId,
    pub goal: Proposition,
    /// The kernel's assumptions, in the order the certificate cites them.
    pub assumptions: Vec<Proposition>,
    /// The kernel's semantic axioms, in the order the certificate cites them.
    pub axioms: Vec<Proposition>,
    pub certificate: CertificateEnvelope,
    pub fingerprint: [u8; 32],
}

impl ObligationEvidenceSeal {
    /// Seals accepted evidence, computing its fingerprint.
    pub fn seal(
        machine: MachineId,
        obligation: ObligationId,
        goal: Proposition,
        assumptions: Vec<Proposition>,
        axioms: Vec<Proposition>,
        certificate: CertificateEnvelope,
    ) -> Result<Self, CodecError> {
        let mut seal = Self {
            machine,
            obligation,
            goal,
            assumptions,
            axioms,
            certificate,
            fingerprint: [0; 32],
        };
        seal.fingerprint = seal.recompute()?;
        Ok(seal)
    }

    /// The fingerprint of the seal's parts as they are now.
    pub fn recompute(&self) -> Result<[u8; 32], CodecError> {
        let mut writer = Writer::default();
        writer.id(self.machine);
        writer.id(self.obligation);
        encode_proposition(&mut writer, &self.goal)?;
        for list in [&self.assumptions, &self.axioms] {
            writer.len("evidence hypotheses", list.len())?;
            for proposition in list {
                encode_proposition(&mut writer, proposition)?;
            }
        }
        writer.id(self.certificate.identity);
        writer.u16(self.certificate.proof_system_marker.get());
        encode_proof_node(&mut writer, &self.certificate.proof)?;
        let bytes = writer.finish();
        let mut digest = crate::digest_encoder::DomainDigestEncoder::new(DOMAIN);
        let length = u64::try_from(bytes.len()).map_err(|_| CodecError::NonCanonicalEncoding)?;
        digest.raw(&length.to_le_bytes());
        digest.raw(&bytes);
        Ok(digest.finish())
    }

    /// Whether the stored fingerprint is the fingerprint of the stored parts.
    pub fn intact(&self) -> Result<bool, CodecError> {
        Ok(self.recompute()? == self.fingerprint)
    }
}

/// The seals produced beside one in-memory module, bound to that module's
/// identity. A consumer attaches them only to the module they name, so seals
/// from an earlier build never reach a different module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceSeals {
    pub module: crate::TerminalPsiIdentity,
    pub seals: Vec<ObligationEvidenceSeal>,
}

impl EvidenceSeals {
    pub fn for_module(
        module: &terminal_psi::TerminalModule,
        seals: Vec<ObligationEvidenceSeal>,
    ) -> Result<Self, CodecError> {
        Ok(Self {
            module: crate::terminal_psi_identity(module)?,
            seals,
        })
    }
}

/// The canonical bytes of one certificate.
pub fn canonical_certificate_bytes(proof: &ProofNode) -> Result<Vec<u8>, CodecError> {
    let mut writer = Writer::default();
    encode_proof_node(&mut writer, proof)?;
    Ok(writer.finish())
}

fn index(writer: &mut Writer, value: usize) -> Result<(), CodecError> {
    writer.u64(u64::try_from(value).map_err(|_| CodecError::NonCanonicalEncoding)?);
    Ok(())
}

fn optional_index(writer: &mut Writer, value: Option<usize>) -> Result<(), CodecError> {
    match value {
        None => writer.u8(0),
        Some(value) => {
            writer.u8(1);
            index(writer, value)?;
        }
    }
    Ok(())
}

fn indices(writer: &mut Writer, values: &[usize]) -> Result<(), CodecError> {
    writer.len("certificate indices", values.len())?;
    for value in values {
        index(writer, *value)?;
    }
    Ok(())
}

fn branch(writer: &mut Writer, branch: &CorrelatedAffineBranchWitness) -> Result<(), CodecError> {
    encode_scalar_term(writer, &branch.root)?;
    encode_scalar_term(writer, &branch.target)?;
    writer.len("correlated branch steps", branch.steps.len())?;
    for step in &branch.steps {
        index(writer, step.definition_axiom)?;
        optional_index(writer, step.literal_axiom)?;
    }
    Ok(())
}

/// A certificate is one postorder table (`crate::term_table`): each row is the
/// rule tag, the rule's non-premise fields in declaration order and the
/// conclusion, then one handle per premise. The match is exhaustive: a new
/// rule changes this encoding deliberately.
fn encode_proof_node(writer: &mut Writer, node: &ProofNode) -> Result<(), CodecError> {
    encode_term_table(writer, node)
}

impl TableTerm for ProofNode {
    const LABEL: &'static str = "certificate node";

    fn children<'term>(&'term self, premises: &mut Vec<&'term Self>) {
        match &self.rule {
            ProofRule::ValueEqualityTransport {
                premise,
                equalities,
            } => {
                premises.push(premise);
                premises.extend(equalities.iter());
            }
            ProofRule::PredicateDenotation { premise } => {
                premises.push(premise);
            }
            ProofRule::ConjunctionIntroduction(children) => {
                premises.extend(children.iter());
            }
            ProofRule::ConjunctionElimination { conjunction, .. } => {
                premises.push(conjunction);
            }
            ProofRule::DisjunctionIntroduction { disjunct, .. } => {
                premises.push(disjunct);
            }
            ProofRule::DisjunctionElimination {
                disjunction,
                branches,
            } => {
                premises.push(disjunction);
                premises.extend(branches.iter());
            }
            ProofRule::ImplicationIntroduction { body } => {
                premises.push(body);
            }
            ProofRule::ImplicationElimination {
                implication,
                premise,
            } => {
                premises.push(implication);
                premises.push(premise);
            }
            ProofRule::EqualityTransitivity {
                left_equals_middle,
                middle_equals_right,
            } => {
                premises.push(left_equals_middle);
                premises.push(middle_equals_right);
            }
            ProofRule::EqualitySymmetry { equality } => {
                premises.push(equality);
            }
            ProofRule::IntegerOrderWeakening { relation } => {
                premises.push(relation);
            }
            ProofRule::IntegerOrderDiscreteness { relation } => {
                premises.push(relation);
            }
            ProofRule::IntegerMathOrderTranslation { relation, .. } => {
                premises.push(relation);
            }
            ProofRule::IntegerOrderAntisymmetry { forward, backward } => {
                premises.push(forward);
                premises.push(backward);
            }
            ProofRule::IntegerSubtractOrder {
                difference,
                positive,
            } => {
                premises.push(difference);
                premises.push(positive);
            }
            ProofRule::IntegerAddOrder { sum, positive } => {
                premises.push(sum);
                premises.push(positive);
            }
            ProofRule::IntegerSubtractAntitone {
                smaller,
                larger,
                order,
            } => {
                premises.push(smaller);
                premises.push(larger);
                premises.push(order);
            }
            ProofRule::IntegerLessOrEqualTransitivity {
                left_less_or_equal_middle,
                middle_less_or_equal_right,
            } => {
                premises.push(left_less_or_equal_middle);
                premises.push(middle_less_or_equal_right);
            }
            ProofRule::IntegerStrictOrderTransitivity {
                left_to_middle,
                middle_to_right,
            } => {
                premises.push(left_to_middle);
                premises.push(middle_to_right);
            }
            ProofRule::IntegerOrderSubstitution {
                relation, equality, ..
            } => {
                premises.push(relation);
                premises.push(equality);
            }
            ProofRule::IntegerAffineBound { root_bound, .. } => {
                premises.push(root_bound);
            }
            ProofRule::IntegerExactAddDefinitionBound {
                left_bound,
                right_bound,
                ..
            } => {
                premises.push(left_bound);
                premises.push(right_bound);
            }
            ProofRule::IntegerExactSubtractDefinitionBound {
                left_bound,
                right_bound,
                ..
            } => {
                premises.push(left_bound);
                premises.push(right_bound);
            }
            ProofRule::IntegerCastBound { root_bound, .. } => {
                premises.push(root_bound);
            }
            _ => {}
        }
    }

    fn write_row(&self, writer: &mut Writer) -> Result<(), CodecError> {
        write_proof_row(writer, self)
    }

    fn read_row(
        _reader: &mut crate::semantic_module::Reader<'_>,
        _children: &mut TableChildren<'_, Self>,
    ) -> Result<Self, CodecError> {
        Err(CodecError::NonCanonicalEncoding)
    }
}

fn write_proof_row(writer: &mut Writer, node: &ProofNode) -> Result<(), CodecError> {
    match &node.rule {
        ProofRule::ValueEqualityTransport { equalities, .. } => {
            writer.u8(0);
            writer.len("certificate children", equalities.len())?;
        }
        ProofRule::PredicateDenotation { .. } => {
            writer.u8(1);
        }
        ProofRule::Primitive(judgment) => {
            writer.u8(2);
            writer.u8(match judgment {
                PrimitiveJudgment::Truth => 0,
                PrimitiveJudgment::ReflexiveEquality => 1,
                PrimitiveJudgment::ClosedIntegerRelation => 2,
                PrimitiveJudgment::IntegerCarrierBound => 3,
            });
        }
        ProofRule::SemanticAxiom { index: cited } => {
            writer.u8(3);
            index(writer, *cited)?;
        }
        ProofRule::Assumption { index: cited } => {
            writer.u8(4);
            index(writer, *cited)?;
        }
        ProofRule::ConjunctionIntroduction(children) => {
            writer.u8(5);
            writer.len("certificate children", children.len())?;
        }
        ProofRule::ConjunctionElimination { conjunct, .. } => {
            writer.u8(6);
            index(writer, *conjunct)?;
        }
        ProofRule::DisjunctionIntroduction {
            index: position, ..
        } => {
            writer.u8(7);
            index(writer, *position)?;
        }
        ProofRule::DisjunctionElimination { branches, .. } => {
            writer.u8(8);
            writer.len("certificate children", branches.len())?;
        }
        ProofRule::ImplicationIntroduction { .. } => {
            writer.u8(9);
        }
        ProofRule::ImplicationElimination { .. } => {
            writer.u8(10);
        }
        ProofRule::EqualityTransitivity { .. } => {
            writer.u8(11);
        }
        ProofRule::EqualitySymmetry { .. } => {
            writer.u8(12);
        }
        ProofRule::IntegerOrderWeakening { .. } => {
            writer.u8(13);
        }
        ProofRule::IntegerOrderDiscreteness { .. } => {
            writer.u8(14);
        }
        ProofRule::IntegerMathOrderTranslation { addend, .. } => {
            writer.u8(15);
            encode_integer_math_term(writer, addend)?;
        }
        ProofRule::IntegerOrderAntisymmetry { .. } => {
            writer.u8(16);
        }
        ProofRule::IntegerSubtractOrder { .. } => {
            writer.u8(17);
        }
        ProofRule::IntegerAddOrder { .. } => {
            writer.u8(18);
        }
        ProofRule::IntegerSubtractAntitone { .. } => {
            writer.u8(19);
        }
        ProofRule::IntegerLessOrEqualTransitivity { .. } => {
            writer.u8(20);
        }
        ProofRule::IntegerStrictOrderTransitivity { .. } => {
            writer.u8(21);
        }
        ProofRule::IntegerOrderSubstitution { endpoint, .. } => {
            writer.u8(22);
            index(writer, *endpoint)?;
        }
        ProofRule::IntegerAffineBound { witness, .. } => {
            writer.u8(23);
            encode_scalar_term(writer, &witness.root)?;
            encode_scalar_term(writer, &witness.target)?;
            indices(writer, &witness.definition_axioms)?;
            writer.len("affine literal axioms", witness.literal_axioms.len())?;
            for literal in &witness.literal_axioms {
                optional_index(writer, *literal)?;
            }
        }
        ProofRule::IntegerExactAddDefinitionBound {
            definition_axiom, ..
        } => {
            writer.u8(24);
            index(writer, *definition_axiom)?;
        }
        ProofRule::IntegerExactSubtractDefinitionBound {
            definition_axiom, ..
        } => {
            writer.u8(25);
            index(writer, *definition_axiom)?;
        }
        ProofRule::IntegerCastBound { witness, .. } => {
            writer.u8(26);
            encode_scalar_term(writer, &witness.root)?;
            encode_scalar_term(writer, &witness.target)?;
            indices(writer, &witness.definition_axioms)?;
        }
        ProofRule::IntegerCorrelatedForbiddenRoots { witness } => {
            writer.u8(27);
            branch(writer, &witness.dividend)?;
            branch(writer, &witness.divisor)?;
            index(writer, witness.definition_axiom_count)?;
            index(writer, witness.lower_bound_axiom)?;
            index(writer, witness.upper_bound_axiom)?;
            encode_proposition(writer, &witness.conclusion)?;
        }
    }
    encode_proposition(writer, &node.conclusion)
}

#[cfg(test)]
mod tests {
    use semantic_vocabulary::EvidenceIdentity;
    use terminal_psi::ProofSystemMarker;

    use super::*;

    fn truth() -> ProofNode {
        ProofNode {
            conclusion: Proposition::Truth,
            rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
        }
    }

    fn seal(proof: ProofNode) -> ObligationEvidenceSeal {
        ObligationEvidenceSeal::seal(
            MachineId::new(1).unwrap(),
            ObligationId::new(2).unwrap(),
            Proposition::Truth,
            Vec::new(),
            Vec::new(),
            CertificateEnvelope {
                identity: EvidenceIdentity::new(2).unwrap(),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof,
            },
        )
        .unwrap()
    }

    #[test]
    fn a_changed_part_no_longer_matches_its_fingerprint() {
        let sealed = seal(truth());
        assert!(sealed.intact().unwrap());
        assert_ne!(sealed.fingerprint, [0; 32]);
        let mut goal = sealed.clone();
        goal.goal = Proposition::Falsehood;
        assert!(!goal.intact().unwrap());
        let mut hypotheses = sealed.clone();
        hypotheses.assumptions.push(Proposition::Truth);
        assert!(!hypotheses.intact().unwrap());
        let mut obligation = sealed.clone();
        obligation.obligation = ObligationId::new(3).unwrap();
        assert!(!obligation.intact().unwrap());
        let mut certificate = sealed;
        certificate.certificate.proof = ProofNode {
            conclusion: Proposition::Truth,
            rule: ProofRule::ConjunctionIntroduction(vec![truth(), truth()]),
        };
        assert!(!certificate.intact().unwrap());
    }

    #[test]
    fn equal_certificates_encode_equally_and_distinct_rules_do_not() {
        let assumption = |index| ProofNode {
            conclusion: Proposition::Truth,
            rule: ProofRule::Assumption { index },
        };
        let axiom = ProofNode {
            conclusion: Proposition::Truth,
            rule: ProofRule::SemanticAxiom { index: 0 },
        };
        assert_eq!(
            canonical_certificate_bytes(&assumption(0)).unwrap(),
            canonical_certificate_bytes(&assumption(0)).unwrap()
        );
        assert_ne!(
            canonical_certificate_bytes(&assumption(0)).unwrap(),
            canonical_certificate_bytes(&assumption(1)).unwrap()
        );
        assert_ne!(
            canonical_certificate_bytes(&assumption(0)).unwrap(),
            canonical_certificate_bytes(&axiom).unwrap()
        );
    }

    #[test]
    fn a_deep_certificate_encodes_without_host_stack_recursion() {
        std::thread::spawn(|| {
            let mut proof = truth();
            for _ in 0..4096 {
                proof = ProofNode {
                    conclusion: Proposition::Truth,
                    rule: ProofRule::EqualitySymmetry {
                        equality: Box::new(proof),
                    },
                };
            }
            let bytes = canonical_certificate_bytes(&proof).unwrap();
            assert_eq!(&bytes[..4], &4097_u32.to_le_bytes());
            assert_eq!(canonical_certificate_bytes(&proof).unwrap(), bytes);
        })
        .join()
        .expect("certificate encoding uses an explicit stack");
    }
}
