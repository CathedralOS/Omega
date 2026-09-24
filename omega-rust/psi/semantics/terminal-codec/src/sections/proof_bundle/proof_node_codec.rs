//! Proof nodes and proof rules on the wire, encoded depth-first and decoded
//! through an explicit pending stack.

use crate::sections::proof_bundle::ProofCodecError;
use crate::sections::proof_bundle::proposition_codec::{decode_proposition, encode_proposition};
use crate::sections::proof_bundle::scalar_term_codec::{
    decode_primitive, decode_scalar_term, encode_primitive, encode_scalar_term,
};
use crate::sections::proof_bundle::wire::{Reader, Writer};
use proof_admission::{
    CorrelatedAffineBranchWitness, CorrelatedAffineStepWitness, IntegerAffineWitness,
    IntegerCastChainWitness, IntegerCorrelatedForbiddenRootWitness, ProofNode, ProofRule,
};
use semantic_vocabulary::Proposition;

pub(crate) const MAX_PROOF_DEPTH: usize = 256;

pub(crate) fn encode_proof_node(
    writer: &mut Writer,
    node: &ProofNode,
    depth: usize,
    format_marker: u16,
) -> Result<(), ProofCodecError> {
    let mut pending = vec![ProofEncodingAction::Node(node, depth)];
    while let Some(action) = pending.pop() {
        match action {
            ProofEncodingAction::Suffix(rule) => {
                encode_proof_rule_suffix(writer, rule, format_marker)?;
            }
            ProofEncodingAction::Branches(branches, depth) => {
                writer.len("disjunction branches", branches.len())?;
                pending.push(ProofEncodingAction::Children(branches, depth));
            }
            ProofEncodingAction::Equalities(equalities, depth) => {
                writer.len("value equality proofs", equalities.len())?;
                pending.push(ProofEncodingAction::Children(equalities, depth));
            }
            ProofEncodingAction::Children(children, depth) => {
                if let Some((first, remaining)) = children.split_first() {
                    pending.push(ProofEncodingAction::Children(remaining, depth));
                    pending.push(ProofEncodingAction::Node(first, depth));
                }
            }
            ProofEncodingAction::Node(node, depth) => {
                if depth > MAX_PROOF_DEPTH {
                    return Err(ProofCodecError::ProofNestingTooDeep);
                }
                encode_proposition(writer, &node.conclusion, 0, format_marker)?;
                pending.push(ProofEncodingAction::Suffix(&node.rule));
                let child_depth = depth + 1;
                match &node.rule {
                    ProofRule::Primitive(_) => writer.u8(1),
                    ProofRule::SemanticAxiom { .. } => writer.u8(2),
                    ProofRule::Assumption { .. } => writer.u8(3),
                    ProofRule::ConjunctionIntroduction(children) => {
                        writer.u8(4);
                        writer.len("conjunction proofs", children.len())?;
                        pending.push(ProofEncodingAction::Children(children, child_depth));
                    }
                    ProofRule::ConjunctionElimination { conjunction, .. } => {
                        writer.u8(5);
                        pending.push(ProofEncodingAction::Node(conjunction, child_depth));
                    }
                    ProofRule::ImplicationIntroduction { body } => {
                        writer.u8(6);
                        pending.push(ProofEncodingAction::Node(body, child_depth));
                    }
                    ProofRule::ImplicationElimination {
                        implication,
                        premise,
                    } => {
                        writer.u8(7);
                        pending.push(ProofEncodingAction::Node(premise, child_depth));
                        pending.push(ProofEncodingAction::Node(implication, child_depth));
                    }
                    ProofRule::EqualitySymmetry { equality } => {
                        writer.u8(17);
                        pending.push(ProofEncodingAction::Node(equality, child_depth));
                    }
                    ProofRule::IntegerOrderWeakening { relation } => {
                        writer.u8(18);
                        pending.push(ProofEncodingAction::Node(relation, child_depth));
                    }
                    ProofRule::IntegerOrderDiscreteness { relation } => {
                        writer.u8(19);
                        pending.push(ProofEncodingAction::Node(relation, child_depth));
                    }
                    ProofRule::IntegerSubtractOrder {
                        difference,
                        positive,
                    } => {
                        writer.u8(20);
                        pending.push(ProofEncodingAction::Node(positive, child_depth));
                        pending.push(ProofEncodingAction::Node(difference, child_depth));
                    }
                    ProofRule::IntegerAddOrder { sum, positive } => {
                        writer.u8(24);
                        pending.push(ProofEncodingAction::Node(positive, child_depth));
                        pending.push(ProofEncodingAction::Node(sum, child_depth));
                    }
                    ProofRule::IntegerSubtractAntitone {
                        smaller,
                        larger,
                        order,
                    } => {
                        writer.u8(25);
                        pending.push(ProofEncodingAction::Node(order, child_depth));
                        pending.push(ProofEncodingAction::Node(larger, child_depth));
                        pending.push(ProofEncodingAction::Node(smaller, child_depth));
                    }
                    ProofRule::EqualityTransitivity {
                        left_equals_middle,
                        middle_equals_right,
                    } => {
                        writer.u8(8);
                        pending.push(ProofEncodingAction::Node(middle_equals_right, child_depth));
                        pending.push(ProofEncodingAction::Node(left_equals_middle, child_depth));
                    }
                    ProofRule::DisjunctionIntroduction { disjunct, .. } => {
                        writer.u8(9);
                        pending.push(ProofEncodingAction::Node(disjunct, child_depth));
                    }
                    ProofRule::IntegerLessOrEqualTransitivity {
                        left_less_or_equal_middle,
                        middle_less_or_equal_right,
                    } => {
                        writer.u8(10);
                        pending.push(ProofEncodingAction::Node(
                            middle_less_or_equal_right,
                            child_depth,
                        ));
                        pending.push(ProofEncodingAction::Node(
                            left_less_or_equal_middle,
                            child_depth,
                        ));
                    }
                    ProofRule::ValueEqualityTransport {
                        premise,
                        equalities,
                    } => {
                        writer.u8(23);
                        pending.push(ProofEncodingAction::Equalities(equalities, child_depth));
                        pending.push(ProofEncodingAction::Node(premise, child_depth));
                    }
                    ProofRule::PredicateDenotation { premise } => {
                        writer.u8(22);
                        pending.push(ProofEncodingAction::Node(premise, child_depth));
                    }
                    ProofRule::IntegerStrictOrderTransitivity {
                        left_to_middle,
                        middle_to_right,
                    } => {
                        writer.u8(21);
                        pending.push(ProofEncodingAction::Node(middle_to_right, child_depth));
                        pending.push(ProofEncodingAction::Node(left_to_middle, child_depth));
                    }
                    ProofRule::IntegerOrderSubstitution {
                        relation, equality, ..
                    } => {
                        writer.u8(11);
                        pending.push(ProofEncodingAction::Node(equality, child_depth));
                        pending.push(ProofEncodingAction::Node(relation, child_depth));
                    }
                    ProofRule::IntegerAffineBound { root_bound, .. } => {
                        writer.u8(12);
                        pending.push(ProofEncodingAction::Node(root_bound, child_depth));
                    }
                    ProofRule::IntegerCastBound { root_bound, .. } => {
                        writer.u8(13);
                        pending.push(ProofEncodingAction::Node(root_bound, child_depth));
                    }
                    ProofRule::IntegerCorrelatedForbiddenRoots { .. } => writer.u8(14),
                    ProofRule::IntegerExactAddDefinitionBound {
                        left_bound,
                        right_bound,
                        ..
                    } => {
                        writer.u8(15);
                        pending.push(ProofEncodingAction::Node(right_bound, child_depth));
                        pending.push(ProofEncodingAction::Node(left_bound, child_depth));
                    }
                    ProofRule::DisjunctionElimination {
                        disjunction,
                        branches,
                    } => {
                        writer.u8(16);
                        pending.push(ProofEncodingAction::Branches(branches, child_depth));
                        pending.push(ProofEncodingAction::Node(disjunction, child_depth));
                    }
                }
            }
        }
    }
    Ok(())
}

enum ProofEncodingAction<'proof> {
    Node(&'proof ProofNode, usize),
    Children(&'proof [ProofNode], usize),
    Branches(&'proof [ProofNode], usize),
    Equalities(&'proof [ProofNode], usize),
    Suffix(&'proof ProofRule),
}

fn encode_proof_rule_suffix(
    writer: &mut Writer,
    rule: &ProofRule,
    format_marker: u16,
) -> Result<(), ProofCodecError> {
    match rule {
        ProofRule::Primitive(judgment) => {
            encode_primitive(writer, *judgment);
        }
        ProofRule::SemanticAxiom { index } => {
            writer.index("semantic axiom index", *index)?;
        }
        ProofRule::Assumption { index } => {
            writer.index("assumption index", *index)?;
        }
        ProofRule::ConjunctionElimination { conjunct, .. } => {
            writer.index("conjunct index", *conjunct)?;
        }
        ProofRule::DisjunctionIntroduction { index, .. } => {
            writer.index("disjunct index", *index)?;
        }
        ProofRule::ValueEqualityTransport { .. }
        | ProofRule::PredicateDenotation { .. }
        | ProofRule::ConjunctionIntroduction(_)
        | ProofRule::DisjunctionElimination { .. }
        | ProofRule::ImplicationIntroduction { .. }
        | ProofRule::ImplicationElimination { .. }
        | ProofRule::EqualityTransitivity { .. }
        | ProofRule::EqualitySymmetry { .. }
        | ProofRule::IntegerOrderWeakening { .. }
        | ProofRule::IntegerOrderDiscreteness { .. }
        | ProofRule::IntegerSubtractOrder { .. }
        | ProofRule::IntegerAddOrder { .. }
        | ProofRule::IntegerSubtractAntitone { .. }
        | ProofRule::IntegerLessOrEqualTransitivity { .. }
        | ProofRule::IntegerStrictOrderTransitivity { .. } => {}
        ProofRule::IntegerOrderSubstitution { endpoint, .. } => {
            writer.index("integer order substitution endpoint", *endpoint)?;
        }
        ProofRule::IntegerAffineBound { witness, .. } => {
            encode_scalar_term(writer, &witness.root, 0, format_marker)?;
            encode_scalar_term(writer, &witness.target, 0, format_marker)?;
            writer.len(
                "integer affine definition axioms",
                witness.definition_axioms.len(),
            )?;
            for &index in &witness.definition_axioms {
                writer.index("integer affine definition axiom", index)?;
            }
            writer.len(
                "integer affine literal axioms",
                witness.literal_axioms.len(),
            )?;
            for &index in &witness.literal_axioms {
                match index {
                    None => writer.u8(0),
                    Some(index) => {
                        writer.u8(1);
                        writer.index("integer affine literal axiom", index)?;
                    }
                }
            }
        }
        ProofRule::IntegerCastBound { witness, .. } => {
            encode_scalar_term(writer, &witness.root, 0, format_marker)?;
            encode_scalar_term(writer, &witness.target, 0, format_marker)?;
            writer.len(
                "integer cast definition axioms",
                witness.definition_axioms.len(),
            )?;
            for &index in &witness.definition_axioms {
                writer.index("integer cast definition axiom", index)?;
            }
        }
        ProofRule::IntegerCorrelatedForbiddenRoots { witness } => {
            encode_correlated_affine_branch(writer, &witness.dividend, format_marker)?;
            encode_correlated_affine_branch(writer, &witness.divisor, format_marker)?;
            writer.index(
                "integer correlated definition axiom count",
                witness.definition_axiom_count,
            )?;
            writer.index(
                "integer correlated lower-bound axiom",
                witness.lower_bound_axiom,
            )?;
            writer.index(
                "integer correlated upper-bound axiom",
                witness.upper_bound_axiom,
            )?;
            encode_proposition(writer, &witness.conclusion, 0, format_marker)?;
        }
        ProofRule::IntegerExactAddDefinitionBound {
            definition_axiom, ..
        } => {
            writer.index("integer exact-add definition axiom", *definition_axiom)?;
        }
    }
    Ok(())
}

fn encode_correlated_affine_branch(
    writer: &mut Writer,
    branch: &CorrelatedAffineBranchWitness,
    format_marker: u16,
) -> Result<(), ProofCodecError> {
    encode_scalar_term(writer, &branch.root, 0, format_marker)?;
    encode_scalar_term(writer, &branch.target, 0, format_marker)?;
    writer.len("integer correlated affine steps", branch.steps.len())?;
    for step in &branch.steps {
        writer.index(
            "integer correlated affine definition axiom",
            step.definition_axiom,
        )?;
        match step.literal_axiom {
            None => writer.u8(0),
            Some(index) => {
                writer.u8(1);
                writer.index("integer correlated affine literal axiom", index)?;
            }
        }
    }
    Ok(())
}

pub(crate) fn decode_proof_node(
    reader: &mut Reader<'_>,
    depth: usize,
    format_marker: u16,
) -> Result<ProofNode, ProofCodecError> {
    // Pending nodes consume heap scratch rather than one large decoder stack
    // frame per proof level. Counts never preallocate untrusted child storage.
    let mut pending: Vec<PendingProofNode> = Vec::new();
    loop {
        if depth + pending.len() > MAX_PROOF_DEPTH {
            return Err(ProofCodecError::ProofNestingTooDeep);
        }
        let conclusion = decode_proposition(reader, 0, format_marker)?;
        let tag = reader.u8()?;
        let remaining = match tag {
            1..=3 | 14 => 0,
            4 => reader.count()?,
            5 | 6 | 9 | 12 | 13 | 16 | 17 | 18 | 19 | 22 | 23 => 1,
            7 | 8 | 10 | 11 | 15 | 20 | 21 | 24 => 2,
            25 => 3,
            tag => return Err(ProofCodecError::InvalidTag("ProofRule", tag)),
        };
        let node = PendingProofNode {
            conclusion,
            tag,
            remaining,
            children: Vec::new(),
        };
        if remaining != 0 {
            pending.push(node);
            continue;
        }
        let mut completed = node.finish(reader, format_marker)?;
        loop {
            let Some(mut parent) = pending.pop() else {
                return Ok(completed);
            };
            parent.children.push(completed);
            parent.remaining -= 1;
            // Counted children follow the leading proof, not the node header.
            if matches!(parent.tag, 16 | 23) && parent.children.len() == 1 {
                parent.remaining = reader.count()?;
            }
            if parent.remaining != 0 {
                pending.push(parent);
                break;
            }
            completed = parent.finish(reader, format_marker)?;
        }
    }
}

struct PendingProofNode {
    conclusion: Proposition,
    tag: u8,
    remaining: u32,
    children: Vec<ProofNode>,
}

impl PendingProofNode {
    fn finish(
        self,
        reader: &mut Reader<'_>,
        format_marker: u16,
    ) -> Result<ProofNode, ProofCodecError> {
        let rule = decode_proof_rule(reader, format_marker, self.tag, self.children.into_iter())?;
        Ok(ProofNode {
            conclusion: self.conclusion,
            rule,
        })
    }
}

fn decode_proof_rule(
    reader: &mut Reader<'_>,
    format_marker: u16,
    tag: u8,
    mut children: std::vec::IntoIter<ProofNode>,
) -> Result<ProofRule, ProofCodecError> {
    Ok(match tag {
        1 => ProofRule::Primitive(decode_primitive(reader)?),
        2 => ProofRule::SemanticAxiom {
            index: reader.index()?,
        },
        3 => ProofRule::Assumption {
            index: reader.index()?,
        },
        4 => ProofRule::ConjunctionIntroduction(children.collect()),
        5 => ProofRule::ConjunctionElimination {
            conjunction: Box::new(children.next().expect("decoded conjunction child")),
            conjunct: reader.index()?,
        },
        6 => ProofRule::ImplicationIntroduction {
            body: Box::new(children.next().expect("decoded implication body")),
        },
        7 => ProofRule::ImplicationElimination {
            implication: Box::new(children.next().expect("decoded implication child")),
            premise: Box::new(children.next().expect("decoded implication premise")),
        },
        17 => ProofRule::EqualitySymmetry {
            equality: Box::new(children.next().expect("decoded equality symmetry child")),
        },
        20 => ProofRule::IntegerSubtractOrder {
            difference: Box::new(children.next().expect("decoded subtraction equation child")),
            positive: Box::new(children.next().expect("decoded positive decrement child")),
        },
        24 => ProofRule::IntegerAddOrder {
            sum: Box::new(children.next().expect("decoded addition equation child")),
            positive: Box::new(children.next().expect("decoded positive increment child")),
        },
        25 => ProofRule::IntegerSubtractAntitone {
            smaller: Box::new(children.next().expect("decoded smaller difference child")),
            larger: Box::new(children.next().expect("decoded larger difference child")),
            order: Box::new(children.next().expect("decoded subtrahend order child")),
        },
        19 => ProofRule::IntegerOrderDiscreteness {
            relation: Box::new(
                children
                    .next()
                    .expect("decoded integer order discreteness child"),
            ),
        },
        18 => ProofRule::IntegerOrderWeakening {
            relation: Box::new(
                children
                    .next()
                    .expect("decoded integer order weakening child"),
            ),
        },
        8 => ProofRule::EqualityTransitivity {
            left_equals_middle: Box::new(children.next().expect("decoded first equality")),
            middle_equals_right: Box::new(children.next().expect("decoded second equality")),
        },
        9 => ProofRule::DisjunctionIntroduction {
            disjunct: Box::new(children.next().expect("decoded disjunct")),
            index: reader.index()?,
        },
        10 => ProofRule::IntegerLessOrEqualTransitivity {
            left_less_or_equal_middle: Box::new(children.next().expect("decoded first order")),
            middle_less_or_equal_right: Box::new(children.next().expect("decoded second order")),
        },
        23 => ProofRule::ValueEqualityTransport {
            premise: Box::new(children.next().ok_or(ProofCodecError::UnexpectedEnd)?),
            equalities: children.collect(),
        },
        22 => ProofRule::PredicateDenotation {
            premise: Box::new(children.next().ok_or(ProofCodecError::UnexpectedEnd)?),
        },
        21 => ProofRule::IntegerStrictOrderTransitivity {
            left_to_middle: Box::new(children.next().ok_or(ProofCodecError::UnexpectedEnd)?),
            middle_to_right: Box::new(children.next().ok_or(ProofCodecError::UnexpectedEnd)?),
        },
        11 => ProofRule::IntegerOrderSubstitution {
            relation: Box::new(children.next().expect("decoded relation")),
            equality: Box::new(children.next().expect("decoded equality")),
            endpoint: reader.index()?,
        },
        12 => {
            let root_bound = Box::new(children.next().expect("decoded affine root bound"));
            let root = decode_scalar_term(reader, 0, format_marker)?;
            let target = decode_scalar_term(reader, 0, format_marker)?;
            let definition_count = reader.count()?;
            let mut definition_axioms = Vec::new();
            for _ in 0..definition_count {
                definition_axioms.push(reader.index()?);
            }
            let literal_count = reader.count()?;
            let mut literal_axioms = Vec::new();
            for _ in 0..literal_count {
                literal_axioms.push(match reader.u8()? {
                    0 => None,
                    1 => Some(reader.index()?),
                    tag => return Err(ProofCodecError::UnknownIntegerAffineLiteralTag(tag)),
                });
            }
            ProofRule::IntegerAffineBound {
                root_bound,
                witness: IntegerAffineWitness {
                    root,
                    target,
                    definition_axioms,
                    literal_axioms,
                },
            }
        }
        13 => {
            let root_bound = Box::new(children.next().expect("decoded cast root bound"));
            let root = decode_scalar_term(reader, 0, format_marker)?;
            let target = decode_scalar_term(reader, 0, format_marker)?;
            let definition_count = reader.count()?;
            let mut definition_axioms = Vec::new();
            for _ in 0..definition_count {
                definition_axioms.push(reader.index()?);
            }
            ProofRule::IntegerCastBound {
                root_bound,
                witness: IntegerCastChainWitness {
                    root,
                    target,
                    definition_axioms,
                },
            }
        }
        14 => {
            let dividend = decode_correlated_affine_branch(reader, format_marker)?;
            let divisor = decode_correlated_affine_branch(reader, format_marker)?;
            let definition_axiom_count = reader.index()?;
            let lower_bound_axiom = reader.index()?;
            let upper_bound_axiom = reader.index()?;
            let conclusion = decode_proposition(reader, 0, format_marker)?;
            ProofRule::IntegerCorrelatedForbiddenRoots {
                witness: IntegerCorrelatedForbiddenRootWitness {
                    dividend,
                    divisor,
                    definition_axiom_count,
                    lower_bound_axiom,
                    upper_bound_axiom,
                    conclusion,
                },
            }
        }
        15 => ProofRule::IntegerExactAddDefinitionBound {
            left_bound: Box::new(children.next().expect("decoded left bound")),
            right_bound: Box::new(children.next().expect("decoded right bound")),
            definition_axiom: reader.index()?,
        },
        16 => ProofRule::DisjunctionElimination {
            disjunction: Box::new(children.next().expect("decoded disjunction")),
            branches: children.collect(),
        },
        tag => return Err(ProofCodecError::InvalidTag("ProofRule", tag)),
    })
}

fn decode_correlated_affine_branch(
    reader: &mut Reader<'_>,
    format_marker: u16,
) -> Result<CorrelatedAffineBranchWitness, ProofCodecError> {
    let root = decode_scalar_term(reader, 0, format_marker)?;
    let target = decode_scalar_term(reader, 0, format_marker)?;
    let step_count = reader.count()?;
    let mut steps = Vec::new();
    for _ in 0..step_count {
        let definition_axiom = reader.index()?;
        let literal_axiom = match reader.u8()? {
            0 => None,
            1 => Some(reader.index()?),
            tag => {
                return Err(ProofCodecError::UnknownIntegerCorrelatedAffineLiteralTag(
                    tag,
                ));
            }
        };
        steps.push(CorrelatedAffineStepWitness {
            definition_axiom,
            literal_axiom,
        });
    }
    Ok(CorrelatedAffineBranchWitness {
        root,
        target,
        steps,
    })
}
