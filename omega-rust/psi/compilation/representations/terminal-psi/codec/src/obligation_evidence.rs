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
use crate::term_wire::integer_math_term_wire::encode_integer_math_term;

const DOMAIN: &[u8] = b"psi-obligation-evidence-fingerprint\0";

/// Deeper certificates are refused rather than encoded on the host stack.
const MAX_CERTIFICATE_DEPTH: usize = 1024;

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
        encode_proposition(&mut writer, &self.goal, 0)?;
        for list in [&self.assumptions, &self.axioms] {
            writer.len("evidence hypotheses", list.len())?;
            for proposition in list {
                encode_proposition(&mut writer, proposition, 0)?;
            }
        }
        writer.id(self.certificate.identity);
        writer.u16(self.certificate.proof_system_marker.get());
        encode_proof_node(&mut writer, &self.certificate.proof, 0)?;
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
    encode_proof_node(&mut writer, proof, 0)?;
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

fn nodes(writer: &mut Writer, children: &[ProofNode], depth: usize) -> Result<(), CodecError> {
    writer.len("certificate children", children.len())?;
    for child in children {
        encode_proof_node(writer, child, depth + 1)?;
    }
    Ok(())
}

fn branch(writer: &mut Writer, branch: &CorrelatedAffineBranchWitness) -> Result<(), CodecError> {
    encode_scalar_term(writer, &branch.root, 0)?;
    encode_scalar_term(writer, &branch.target, 0)?;
    writer.len("correlated branch steps", branch.steps.len())?;
    for step in &branch.steps {
        index(writer, step.definition_axiom)?;
        optional_index(writer, step.literal_axiom)?;
    }
    Ok(())
}

/// A rule tag, its fields in declaration order, then the conclusion. The
/// match is exhaustive: a new rule changes this encoding deliberately.
fn encode_proof_node(
    writer: &mut Writer,
    node: &ProofNode,
    depth: usize,
) -> Result<(), CodecError> {
    if depth > MAX_CERTIFICATE_DEPTH {
        return Err(CodecError::CertificateNestingTooDeep);
    }
    let child =
        |writer: &mut Writer, child: &ProofNode| encode_proof_node(writer, child, depth + 1);
    match &node.rule {
        ProofRule::ValueEqualityTransport {
            premise,
            equalities,
        } => {
            writer.u8(0);
            child(writer, premise)?;
            nodes(writer, equalities, depth)?;
        }
        ProofRule::PredicateDenotation { premise } => {
            writer.u8(1);
            child(writer, premise)?;
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
            nodes(writer, children, depth)?;
        }
        ProofRule::ConjunctionElimination {
            conjunction,
            conjunct,
        } => {
            writer.u8(6);
            child(writer, conjunction)?;
            index(writer, *conjunct)?;
        }
        ProofRule::DisjunctionIntroduction {
            disjunct,
            index: position,
        } => {
            writer.u8(7);
            child(writer, disjunct)?;
            index(writer, *position)?;
        }
        ProofRule::DisjunctionElimination {
            disjunction,
            branches,
        } => {
            writer.u8(8);
            child(writer, disjunction)?;
            nodes(writer, branches, depth)?;
        }
        ProofRule::ImplicationIntroduction { body } => {
            writer.u8(9);
            child(writer, body)?;
        }
        ProofRule::ImplicationElimination {
            implication,
            premise,
        } => {
            writer.u8(10);
            child(writer, implication)?;
            child(writer, premise)?;
        }
        ProofRule::EqualityTransitivity {
            left_equals_middle,
            middle_equals_right,
        } => {
            writer.u8(11);
            child(writer, left_equals_middle)?;
            child(writer, middle_equals_right)?;
        }
        ProofRule::EqualitySymmetry { equality } => {
            writer.u8(12);
            child(writer, equality)?;
        }
        ProofRule::IntegerOrderWeakening { relation } => {
            writer.u8(13);
            child(writer, relation)?;
        }
        ProofRule::IntegerOrderDiscreteness { relation } => {
            writer.u8(14);
            child(writer, relation)?;
        }
        ProofRule::IntegerMathOrderTranslation { relation, addend } => {
            writer.u8(15);
            child(writer, relation)?;
            encode_integer_math_term(writer, addend, 0)?;
        }
        ProofRule::IntegerOrderAntisymmetry { forward, backward } => {
            writer.u8(16);
            child(writer, forward)?;
            child(writer, backward)?;
        }
        ProofRule::IntegerSubtractOrder {
            difference,
            positive,
        } => {
            writer.u8(17);
            child(writer, difference)?;
            child(writer, positive)?;
        }
        ProofRule::IntegerAddOrder { sum, positive } => {
            writer.u8(18);
            child(writer, sum)?;
            child(writer, positive)?;
        }
        ProofRule::IntegerSubtractAntitone {
            smaller,
            larger,
            order,
        } => {
            writer.u8(19);
            child(writer, smaller)?;
            child(writer, larger)?;
            child(writer, order)?;
        }
        ProofRule::IntegerLessOrEqualTransitivity {
            left_less_or_equal_middle,
            middle_less_or_equal_right,
        } => {
            writer.u8(20);
            child(writer, left_less_or_equal_middle)?;
            child(writer, middle_less_or_equal_right)?;
        }
        ProofRule::IntegerStrictOrderTransitivity {
            left_to_middle,
            middle_to_right,
        } => {
            writer.u8(21);
            child(writer, left_to_middle)?;
            child(writer, middle_to_right)?;
        }
        ProofRule::IntegerOrderSubstitution {
            relation,
            equality,
            endpoint,
        } => {
            writer.u8(22);
            child(writer, relation)?;
            child(writer, equality)?;
            index(writer, *endpoint)?;
        }
        ProofRule::IntegerAffineBound {
            root_bound,
            witness,
        } => {
            writer.u8(23);
            child(writer, root_bound)?;
            encode_scalar_term(writer, &witness.root, 0)?;
            encode_scalar_term(writer, &witness.target, 0)?;
            indices(writer, &witness.definition_axioms)?;
            writer.len("affine literal axioms", witness.literal_axioms.len())?;
            for literal in &witness.literal_axioms {
                optional_index(writer, *literal)?;
            }
        }
        ProofRule::IntegerExactAddDefinitionBound {
            left_bound,
            right_bound,
            definition_axiom,
        } => {
            writer.u8(24);
            child(writer, left_bound)?;
            child(writer, right_bound)?;
            index(writer, *definition_axiom)?;
        }
        ProofRule::IntegerExactSubtractDefinitionBound {
            left_bound,
            right_bound,
            definition_axiom,
        } => {
            writer.u8(25);
            child(writer, left_bound)?;
            child(writer, right_bound)?;
            index(writer, *definition_axiom)?;
        }
        ProofRule::IntegerCastBound {
            root_bound,
            witness,
        } => {
            writer.u8(26);
            child(writer, root_bound)?;
            encode_scalar_term(writer, &witness.root, 0)?;
            encode_scalar_term(writer, &witness.target, 0)?;
            indices(writer, &witness.definition_axioms)?;
        }
        ProofRule::IntegerCorrelatedForbiddenRoots { witness } => {
            writer.u8(27);
            branch(writer, &witness.dividend)?;
            branch(writer, &witness.divisor)?;
            index(writer, witness.definition_axiom_count)?;
            index(writer, witness.lower_bound_axiom)?;
            index(writer, witness.upper_bound_axiom)?;
            encode_proposition(writer, &witness.conclusion, 0)?;
        }
    }
    encode_proposition(writer, &node.conclusion, 0)
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
    fn an_over_deep_certificate_is_refused() {
        let mut proof = truth();
        for _ in 0..=MAX_CERTIFICATE_DEPTH {
            proof = ProofNode {
                conclusion: Proposition::Truth,
                rule: ProofRule::EqualitySymmetry {
                    equality: Box::new(proof),
                },
            };
        }
        assert_eq!(
            canonical_certificate_bytes(&proof),
            Err(CodecError::CertificateNestingTooDeep)
        );
    }
}
