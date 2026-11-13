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
use crate::semantic_module::{
    Reader, Writer, decode_proposition, decode_scalar_term, encode_proposition, encode_scalar_term,
};
use crate::term_table::{TableChildren, TableTerm, decode_term_table, encode_term_table};
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
        reader: &mut Reader<'_>,
        children: &mut TableChildren<'_, Self>,
    ) -> Result<Self, CodecError> {
        read_proof_row(reader, children)
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

/// A complete ordinary certificate, including its current proof-system marker.
pub(crate) struct Certificate;
impl crate::wire_fields::Field<CertificateEnvelope> for Certificate {
    fn write(writer: &mut Writer, certificate: &CertificateEnvelope) -> Result<(), CodecError> {
        writer.id(certificate.identity);
        writer.u16(certificate.proof_system_marker.get());
        encode_proof_node(writer, &certificate.proof)
    }
    fn read(reader: &mut Reader<'_>) -> Result<CertificateEnvelope, CodecError> {
        let identity = reader.id("certificate identity")?;
        let proof_system_marker = terminal_psi::ProofSystemMarker::new(reader.u16()?).ok_or(
            CodecError::MalformedStructuralFoundation("unsupported proof-system marker"),
        )?;
        Ok(CertificateEnvelope {
            identity,
            proof_system_marker,
            proof: decode_term_table(reader)?,
        })
    }
}

fn read_index(reader: &mut Reader<'_>) -> Result<usize, CodecError> {
    usize::try_from(reader.u64()?).map_err(|_| CodecError::NonCanonicalEncoding)
}
fn read_optional_index(reader: &mut Reader<'_>) -> Result<Option<usize>, CodecError> {
    match reader.u8()? {
        0 => Ok(None),
        1 => read_index(reader).map(Some),
        _ => Err(CodecError::NonCanonicalEncoding),
    }
}
fn read_indices(reader: &mut Reader<'_>) -> Result<Vec<usize>, CodecError> {
    let count = crate::semantic_module::wire::bounded_count(reader, 8)?;
    (0..count).map(|_| read_index(reader)).collect()
}
fn read_branch(reader: &mut Reader<'_>) -> Result<CorrelatedAffineBranchWitness, CodecError> {
    let root = decode_scalar_term(reader)?;
    let target = decode_scalar_term(reader)?;
    let count = crate::semantic_module::wire::bounded_count(reader, 9)?;
    let steps = (0..count)
        .map(|_| {
            Ok(terminal_psi::CorrelatedAffineStepWitness {
                definition_axiom: read_index(reader)?,
                literal_axiom: read_optional_index(reader)?,
            })
        })
        .collect::<Result<Vec<_>, CodecError>>()?;
    Ok(CorrelatedAffineBranchWitness {
        root,
        target,
        steps,
    })
}
fn read_node(
    reader: &mut Reader<'_>,
    children: &mut TableChildren<'_, ProofNode>,
    count: usize,
    rule: impl FnOnce(Vec<ProofNode>) -> ProofRule,
) -> Result<ProofNode, CodecError> {
    let conclusion = decode_proposition(reader)?;
    let children = children.take(reader, count)?;
    Ok(ProofNode {
        conclusion,
        rule: rule(children),
    })
}
fn one(mut children: Vec<ProofNode>) -> Box<ProofNode> {
    Box::new(children.pop().expect("the table reader checked one child"))
}
fn two(children: Vec<ProofNode>) -> (Box<ProofNode>, Box<ProofNode>) {
    let mut children = children.into_iter();
    (
        Box::new(
            children
                .next()
                .expect("the table reader checked two children"),
        ),
        Box::new(
            children
                .next()
                .expect("the table reader checked two children"),
        ),
    )
}
fn read_proof_row(
    reader: &mut Reader<'_>,
    children: &mut TableChildren<'_, ProofNode>,
) -> Result<ProofNode, CodecError> {
    use ProofRule as R;
    match reader.u8()? {
        0 => {
            let count = crate::semantic_module::wire::bounded_count(reader, 4)?;
            let total = count
                .checked_add(1)
                .ok_or(CodecError::NonCanonicalEncoding)?;
            read_node(reader, children, total, |children| {
                let mut children = children.into_iter();
                R::ValueEqualityTransport {
                    premise: Box::new(children.next().expect("checked child count")),
                    equalities: children.collect(),
                }
            })
        }
        1 => read_node(reader, children, 1, |children| R::PredicateDenotation {
            premise: one(children),
        }),
        2 => {
            let judgment = match reader.u8()? {
                0 => PrimitiveJudgment::Truth,
                1 => PrimitiveJudgment::ReflexiveEquality,
                2 => PrimitiveJudgment::ClosedIntegerRelation,
                3 => PrimitiveJudgment::IntegerCarrierBound,
                _ => return Err(CodecError::NonCanonicalEncoding),
            };
            read_node(reader, children, 0, |_| R::Primitive(judgment))
        }
        3 => {
            let index = read_index(reader)?;
            read_node(reader, children, 0, |_| R::SemanticAxiom { index })
        }
        4 => {
            let index = read_index(reader)?;
            read_node(reader, children, 0, |_| R::Assumption { index })
        }
        5 => {
            let count = crate::semantic_module::wire::bounded_count(reader, 4)?;
            read_node(reader, children, count, R::ConjunctionIntroduction)
        }
        6 => {
            let conjunct = read_index(reader)?;
            read_node(reader, children, 1, |children| R::ConjunctionElimination {
                conjunction: one(children),
                conjunct,
            })
        }
        7 => {
            let index = read_index(reader)?;
            read_node(reader, children, 1, |children| R::DisjunctionIntroduction {
                disjunct: one(children),
                index,
            })
        }
        8 => {
            let count = crate::semantic_module::wire::bounded_count(reader, 4)?;
            let total = count
                .checked_add(1)
                .ok_or(CodecError::NonCanonicalEncoding)?;
            read_node(reader, children, total, |children| {
                let mut children = children.into_iter();
                R::DisjunctionElimination {
                    disjunction: Box::new(children.next().expect("checked child count")),
                    branches: children.collect(),
                }
            })
        }
        9 => read_node(reader, children, 1, |children| R::ImplicationIntroduction {
            body: one(children),
        }),
        10 => read_node(reader, children, 2, |children| {
            let (implication, premise) = two(children);
            R::ImplicationElimination {
                implication,
                premise,
            }
        }),
        11 => read_node(reader, children, 2, |children| {
            let (left_equals_middle, middle_equals_right) = two(children);
            R::EqualityTransitivity {
                left_equals_middle,
                middle_equals_right,
            }
        }),
        12 => read_node(reader, children, 1, |children| R::EqualitySymmetry {
            equality: one(children),
        }),
        13 => read_node(reader, children, 1, |children| R::IntegerOrderWeakening {
            relation: one(children),
        }),
        14 => read_node(reader, children, 1, |children| {
            R::IntegerOrderDiscreteness {
                relation: one(children),
            }
        }),
        15 => {
            let addend =
                crate::term_wire::integer_math_term_wire::decode_integer_math_term(reader)?;
            read_node(reader, children, 1, |children| {
                R::IntegerMathOrderTranslation {
                    relation: one(children),
                    addend,
                }
            })
        }
        16 => read_node(reader, children, 2, |children| {
            let (forward, backward) = two(children);
            R::IntegerOrderAntisymmetry { forward, backward }
        }),
        17 => read_node(reader, children, 2, |children| {
            let (difference, positive) = two(children);
            R::IntegerSubtractOrder {
                difference,
                positive,
            }
        }),
        18 => read_node(reader, children, 2, |children| {
            let (sum, positive) = two(children);
            R::IntegerAddOrder { sum, positive }
        }),
        19 => read_node(reader, children, 3, |children| {
            let mut children = children.into_iter();
            R::IntegerSubtractAntitone {
                smaller: Box::new(children.next().expect("checked child count")),
                larger: Box::new(children.next().expect("checked child count")),
                order: Box::new(children.next().expect("checked child count")),
            }
        }),
        20 => read_node(reader, children, 2, |children| {
            let (left_less_or_equal_middle, middle_less_or_equal_right) = two(children);
            R::IntegerLessOrEqualTransitivity {
                left_less_or_equal_middle,
                middle_less_or_equal_right,
            }
        }),
        21 => read_node(reader, children, 2, |children| {
            let (left_to_middle, middle_to_right) = two(children);
            R::IntegerStrictOrderTransitivity {
                left_to_middle,
                middle_to_right,
            }
        }),
        22 => {
            let endpoint = read_index(reader)?;
            read_node(reader, children, 2, |children| {
                let (relation, equality) = two(children);
                R::IntegerOrderSubstitution {
                    relation,
                    equality,
                    endpoint,
                }
            })
        }
        23 => {
            let root = decode_scalar_term(reader)?;
            let target = decode_scalar_term(reader)?;
            let definition_axioms = read_indices(reader)?;
            let count = crate::semantic_module::wire::bounded_count(reader, 1)?;
            let literal_axioms = (0..count)
                .map(|_| read_optional_index(reader))
                .collect::<Result<Vec<_>, _>>()?;
            let witness = terminal_psi::IntegerAffineWitness {
                root,
                target,
                definition_axioms,
                literal_axioms,
            };
            read_node(reader, children, 1, |children| R::IntegerAffineBound {
                root_bound: one(children),
                witness,
            })
        }
        24 => {
            let definition_axiom = read_index(reader)?;
            read_node(reader, children, 2, |children| {
                let (left_bound, right_bound) = two(children);
                R::IntegerExactAddDefinitionBound {
                    left_bound,
                    right_bound,
                    definition_axiom,
                }
            })
        }
        25 => {
            let definition_axiom = read_index(reader)?;
            read_node(reader, children, 2, |children| {
                let (left_bound, right_bound) = two(children);
                R::IntegerExactSubtractDefinitionBound {
                    left_bound,
                    right_bound,
                    definition_axiom,
                }
            })
        }
        26 => {
            let witness = terminal_psi::IntegerCastChainWitness {
                root: decode_scalar_term(reader)?,
                target: decode_scalar_term(reader)?,
                definition_axioms: read_indices(reader)?,
            };
            read_node(reader, children, 1, |children| R::IntegerCastBound {
                root_bound: one(children),
                witness,
            })
        }
        27 => {
            let witness = terminal_psi::IntegerCorrelatedForbiddenRootWitness {
                dividend: read_branch(reader)?,
                divisor: read_branch(reader)?,
                definition_axiom_count: read_index(reader)?,
                lower_bound_axiom: read_index(reader)?,
                upper_bound_axiom: read_index(reader)?,
                conclusion: decode_proposition(reader)?,
            };
            read_node(reader, children, 0, |_| {
                R::IntegerCorrelatedForbiddenRoots { witness }
            })
        }
        _ => Err(CodecError::NonCanonicalEncoding),
    }
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

    fn envelope(proof: ProofNode) -> CertificateEnvelope {
        CertificateEnvelope {
            identity: EvidenceIdentity::new(7).unwrap(),
            proof_system_marker: ProofSystemMarker::CURRENT,
            proof,
        }
    }

    fn encoded_envelope(certificate: &CertificateEnvelope) -> Vec<u8> {
        use crate::wire_fields::Field;
        let mut writer = Writer::default();
        Certificate::write(&mut writer, certificate).unwrap();
        writer.finish()
    }

    #[test]
    fn complete_certificate_rules_and_witness_fields_round_trip() {
        use crate::wire_fields::Field;
        use terminal_psi::{
            CorrelatedAffineStepWitness, IntegerAffineWitness, IntegerCastChainWitness,
            IntegerCorrelatedForbiddenRootWitness,
        };
        let scalar = semantic_vocabulary::ScalarTerm::value(
            semantic_vocabulary::ValueId::new(3).unwrap(),
            semantic_vocabulary::ScalarType::Integer(
                semantic_vocabulary::IntegerType::new(
                    semantic_vocabulary::IntegerSign::Unsigned,
                    64,
                )
                .unwrap(),
            ),
        );
        let branch = || CorrelatedAffineBranchWitness {
            root: scalar.clone(),
            target: scalar.clone(),
            steps: vec![
                CorrelatedAffineStepWitness {
                    definition_axiom: 2,
                    literal_axiom: Some(4),
                },
                CorrelatedAffineStepWitness {
                    definition_axiom: 5,
                    literal_axiom: None,
                },
            ],
        };
        let child = || Box::new(truth());
        let rules = vec![
            ProofRule::ValueEqualityTransport {
                premise: child(),
                equalities: vec![truth(), truth()],
            },
            ProofRule::PredicateDenotation { premise: child() },
            ProofRule::Primitive(PrimitiveJudgment::Truth),
            ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality),
            ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
            ProofRule::Primitive(PrimitiveJudgment::IntegerCarrierBound),
            ProofRule::SemanticAxiom { index: 9 },
            ProofRule::Assumption { index: 11 },
            ProofRule::ConjunctionIntroduction(vec![truth(), truth()]),
            ProofRule::ConjunctionElimination {
                conjunction: child(),
                conjunct: 1,
            },
            ProofRule::DisjunctionIntroduction {
                disjunct: child(),
                index: 2,
            },
            ProofRule::DisjunctionElimination {
                disjunction: child(),
                branches: vec![truth(), truth()],
            },
            ProofRule::ImplicationIntroduction { body: child() },
            ProofRule::ImplicationElimination {
                implication: child(),
                premise: child(),
            },
            ProofRule::EqualityTransitivity {
                left_equals_middle: child(),
                middle_equals_right: child(),
            },
            ProofRule::EqualitySymmetry { equality: child() },
            ProofRule::IntegerOrderWeakening { relation: child() },
            ProofRule::IntegerOrderDiscreteness { relation: child() },
            ProofRule::IntegerMathOrderTranslation {
                relation: child(),
                addend: semantic_vocabulary::IntegerMathTerm::embed(scalar.clone()).unwrap(),
            },
            ProofRule::IntegerOrderAntisymmetry {
                forward: child(),
                backward: child(),
            },
            ProofRule::IntegerSubtractOrder {
                difference: child(),
                positive: child(),
            },
            ProofRule::IntegerAddOrder {
                sum: child(),
                positive: child(),
            },
            ProofRule::IntegerSubtractAntitone {
                smaller: child(),
                larger: child(),
                order: child(),
            },
            ProofRule::IntegerLessOrEqualTransitivity {
                left_less_or_equal_middle: child(),
                middle_less_or_equal_right: child(),
            },
            ProofRule::IntegerStrictOrderTransitivity {
                left_to_middle: child(),
                middle_to_right: child(),
            },
            ProofRule::IntegerOrderSubstitution {
                relation: child(),
                equality: child(),
                endpoint: 1,
            },
            ProofRule::IntegerAffineBound {
                root_bound: child(),
                witness: IntegerAffineWitness {
                    root: scalar.clone(),
                    target: scalar.clone(),
                    definition_axioms: vec![1, 3],
                    literal_axioms: vec![None, Some(2)],
                },
            },
            ProofRule::IntegerExactAddDefinitionBound {
                left_bound: child(),
                right_bound: child(),
                definition_axiom: 7,
            },
            ProofRule::IntegerExactSubtractDefinitionBound {
                left_bound: child(),
                right_bound: child(),
                definition_axiom: 8,
            },
            ProofRule::IntegerCastBound {
                root_bound: child(),
                witness: IntegerCastChainWitness {
                    root: scalar.clone(),
                    target: scalar.clone(),
                    definition_axioms: vec![2, 6],
                },
            },
            ProofRule::IntegerCorrelatedForbiddenRoots {
                witness: IntegerCorrelatedForbiddenRootWitness {
                    dividend: branch(),
                    divisor: branch(),
                    definition_axiom_count: 12,
                    lower_bound_axiom: 8,
                    upper_bound_axiom: 9,
                    conclusion: Proposition::Falsehood,
                },
            },
        ];
        for rule in rules {
            // This checks wire preservation, independently of kernel validity.
            let original = envelope(ProofNode {
                conclusion: Proposition::Truth,
                rule,
            });
            let bytes = encoded_envelope(&original);
            let mut reader = Reader::new(&bytes);
            let recovered = Certificate::read(&mut reader).unwrap();
            assert_eq!(reader.remaining(), 0);
            assert_eq!(recovered, original);
            assert_eq!(encoded_envelope(&recovered), bytes);
        }
    }

    #[test]
    fn certificate_decoder_rejects_marker_and_noncanonical_child_references() {
        use crate::wire_fields::Field;
        let mut bytes = encoded_envelope(&envelope(ProofNode {
            conclusion: Proposition::Truth,
            rule: ProofRule::ImplicationIntroduction {
                body: Box::new(truth()),
            },
        }));
        bytes[8..10].copy_from_slice(&0_u16.to_le_bytes());
        assert!(Certificate::read(&mut Reader::new(&bytes)).is_err());
        let mut bytes = encoded_envelope(&envelope(ProofNode {
            conclusion: Proposition::Truth,
            rule: ProofRule::ImplicationIntroduction {
                body: Box::new(truth()),
            },
        }));
        let last = bytes.len() - 4;
        bytes[last..].copy_from_slice(&1_u32.to_le_bytes());
        assert!(matches!(
            Certificate::read(&mut Reader::new(&bytes)),
            Err(CodecError::TermTableForwardReference(_))
        ));
        let mut bytes = encoded_envelope(&envelope(ProofNode {
            conclusion: Proposition::Truth,
            rule: ProofRule::ConjunctionIntroduction(vec![truth(), truth()]),
        }));
        let last = bytes.len() - 4;
        bytes[last..].copy_from_slice(&0_u32.to_le_bytes());
        assert!(matches!(
            Certificate::read(&mut Reader::new(&bytes)),
            Err(CodecError::NonCanonicalTermTable(_))
        ));
    }
}
