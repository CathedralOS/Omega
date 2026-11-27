//! The exact proof carried by a callable descriptor, encoded in postorder.
use crate::CodecError;
use crate::wire::{Reader, Writer};
use psi_declaration_encoding::terms::{
    integer::encode_integer_math_term,
    proposition::{decode_proposition, encode_proposition},
    scalar::{decode_scalar_term, encode_scalar_term},
    table::{TableChildren, TermEncoding, decode_term_table, encode_term_table},
};
use terminal_psi::{
    CertificateEnvelope, CorrelatedAffineBranchWitness, PrimitiveJudgment, ProofNode, ProofRule,
};
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

/// A certificate is one postorder table : each row is the
/// rule tag, the rule's non-premise fields in declaration order and the
/// conclusion, then one handle per premise. The match is exhaustive: a new
/// rule changes this encoding deliberately.
fn encode_proof_node(writer: &mut Writer, node: &ProofNode) -> Result<(), CodecError> {
    encode_term_table::<CertificateTable>(writer, node)
}

struct CertificateTable;

impl TermEncoding for CertificateTable {
    type Term = ProofNode;
    const LABEL: &'static str = "certificate node";

    fn children<'term>(term: &'term Self::Term, premises: &mut Vec<&'term Self::Term>) {
        match &term.rule {
            ProofRule::FloatIntegerConversion { premise, computed } => {
                premises.push(premise);
                if let Some(computed) = computed {
                    premises.push(computed);
                }
            }
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
            ProofRule::IeeeFloatOrderChain(children)
            | ProofRule::FloatMeaningNormalization(children)
            | ProofRule::ConjunctionIntroduction(children) => {
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
            ProofRule::IntegerPowerOfTwoOrder { order } => {
                premises.push(order);
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

    fn write_row(term: &Self::Term, writer: &mut Writer) -> Result<(), CodecError> {
        write_proof_row(writer, term)
    }

    fn read_row(
        reader: &mut Reader<'_>,
        children: &mut TableChildren<'_, Self::Term>,
    ) -> Result<Self::Term, CodecError> {
        read_proof_row(reader, children)
    }
}

fn write_proof_row(writer: &mut Writer, node: &ProofNode) -> Result<(), CodecError> {
    match &node.rule {
        ProofRule::IntegerFloatExact => writer.u8(32),
        ProofRule::StructuralCaseExhaustion => writer.u8(33),
        ProofRule::FloatIntegerConversion { computed, .. } => {
            writer.u8(31);
            writer.u8(u8::from(computed.is_some()));
        }
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
                PrimitiveJudgment::ClosedIeeeFloatRelation => 4,
                PrimitiveJudgment::ClosedFloatIntegerRelation => 5,
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
        ProofRule::IntegerPowerOfTwoOrder { .. } => {
            writer.u8(28);
        }
        ProofRule::IeeeFloatOrderChain(premises) => {
            writer.u8(29);
            writer.len("certificate children", premises.len())?;
        }
        ProofRule::FloatMeaningNormalization(premises) => {
            writer.u8(30);
            writer.len("certificate children", premises.len())?;
        }
    }
    encode_proposition(writer, &node.conclusion)
}

/// A complete ordinary certificate, including its current proof-system marker.
pub struct Certificate;
impl crate::fields::Field<CertificateEnvelope> for Certificate {
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
            proof: decode_term_table::<CertificateTable>(reader)?,
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
    let count = crate::wire::bounded_count(reader, 8)?;
    (0..count).map(|_| read_index(reader)).collect()
}
fn read_branch(reader: &mut Reader<'_>) -> Result<CorrelatedAffineBranchWitness, CodecError> {
    let root = decode_scalar_term(reader)?;
    let target = decode_scalar_term(reader)?;
    let count = crate::wire::bounded_count(reader, 9)?;
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
fn one<P: From<ProofNode>>(mut children: Vec<ProofNode>) -> P {
    children
        .pop()
        .expect("the table reader checked one child")
        .into()
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
            let count = crate::wire::bounded_count(reader, 4)?;
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
                4 => PrimitiveJudgment::ClosedIeeeFloatRelation,
                5 => PrimitiveJudgment::ClosedFloatIntegerRelation,
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
            let count = crate::wire::bounded_count(reader, 4)?;
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
            let count = crate::wire::bounded_count(reader, 4)?;
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
                psi_declaration_encoding::terms::integer::decode_integer_math_term(reader)?;
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
            let count = crate::wire::bounded_count(reader, 1)?;
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
        28 => read_node(reader, children, 1, |children| R::IntegerPowerOfTwoOrder {
            order: one(children),
        }),
        29 => {
            let count = crate::wire::bounded_count(reader, 4)?;
            read_node(reader, children, count, R::IeeeFloatOrderChain)
        }
        30 => {
            let count = crate::wire::bounded_count(reader, 4)?;
            read_node(reader, children, count, R::FloatMeaningNormalization)
        }
        // The flag selects the one-premise form or the form that also
        // transports a separately checked computed goal.
        31 => {
            let total = match reader.u8()? {
                0 => 1,
                1 => 2,
                _ => return Err(CodecError::NonCanonicalEncoding),
            };
            read_node(reader, children, total, |children| {
                let mut children = children.into_iter();
                R::FloatIntegerConversion {
                    premise: Box::new(children.next().expect("checked child count")),
                    computed: children.next().map(Box::new),
                }
            })
        }
        32 => read_node(reader, children, 0, |_| R::IntegerFloatExact),
        33 => read_node(reader, children, 0, |_| R::StructuralCaseExhaustion),
        _ => Err(CodecError::NonCanonicalEncoding),
    }
}
