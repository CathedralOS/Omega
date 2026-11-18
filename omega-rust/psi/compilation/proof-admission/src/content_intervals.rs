//! Replay interval conservation from checked endpoint order and equality.
//!
//! A projection's concrete endpoints come from the checking context, which
//! rejoins its definition and storage observations. The certificate supplies
//! only an ordering and ordinary integer proofs. It supplies no projection
//! meaning, content claim or access permission. This rule is provisional under
//! foundation.md#admitting-an-arithmetic-rule; the mathematical core does not
//! yet derive interval normalization.

use std::collections::{BTreeMap, BTreeSet};

use semantic_vocabulary::{
    CanonicalIntervalSet, ContentAlgebraKind, ContentConservation, ContentTerm, IntegerMathTerm,
    IntegerValue, NaturalInterval, Proposition, PropositionContext,
};

/// The ordinary integer propositions a proposed endpoint ordering must prove.
/// Equal members share a rank only after proving both inequality directions.
/// Distinct ranks may coincide numerically; this can refuse a merge but cannot
/// create overlap or establish a false equality.
pub fn endpoint_requirements(
    groups: &[Vec<IntegerMathTerm>],
) -> Result<Vec<Proposition>, &'static str> {
    let mut requirements = Vec::new();
    let mut previous = None;
    for members in groups {
        let Some(representative) = members.first() else {
            return Err("interval endpoint equality group is empty");
        };
        if members.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err("interval endpoint group is not canonical and unique");
        }
        let low = previous.unwrap_or_else(|| IntegerMathTerm::literal(IntegerValue::Unsigned(0)));
        requirements.push(Proposition::IntegerMathLessOrEqual(
            low,
            representative.clone(),
        ));
        for member in &members[1..] {
            requirements.push(Proposition::IntegerMathLessOrEqual(
                representative.clone(),
                member.clone(),
            ));
            requirements.push(Proposition::IntegerMathLessOrEqual(
                member.clone(),
                representative.clone(),
            ));
        }
        previous = Some(representative.clone());
    }
    Ok(requirements)
}

pub fn check<'premise>(
    context: &PropositionContext,
    conclusion: &Proposition,
    groups: &[Vec<IntegerMathTerm>],
    premises: impl IntoIterator<Item = &'premise Proposition>,
) -> Result<(), &'static str> {
    let Proposition::ContentConservation(equation) = conclusion else {
        return Err("interval normalization does not conclude content conservation");
    };
    if equation.algebra().kind != ContentAlgebraKind::IntervalSet {
        return Err("interval normalization changes the content algebra");
    }
    let required = endpoint_requirements(groups)?;
    let supplied = premises.into_iter().collect::<Vec<_>>();
    if required.len() != supplied.len()
        || required
            .iter()
            .zip(supplied)
            .any(|(required, actual)| required != actual)
    {
        return Err("interval normalization lacks its exact endpoint proofs");
    }
    let mut ranks = BTreeMap::new();
    for (rank, group) in groups.iter().enumerate() {
        for term in group {
            if ranks.insert(term.clone(), rank).is_some() {
                return Err("interval endpoint occurs in more than one equality group");
            }
        }
    }
    let left = intervals(context, equation, equation.left())?;
    let right = intervals(context, equation, equation.right())?;
    let endpoints = left
        .iter()
        .chain(&right)
        .flat_map(|(start, end)| [start, end])
        .collect::<BTreeSet<_>>();
    if endpoints.len() != ranks.len() || endpoints.iter().any(|term| !ranks.contains_key(*term)) {
        return Err("interval ordering does not cover exactly the projected endpoints");
    }
    let normalized = |members: &[(IntegerMathTerm, IntegerMathTerm)]| {
        let intervals = members
            .iter()
            .map(|(start, end)| {
                let start = bignum::BigInt::from_u64(ranks[start] as u64);
                let end = bignum::BigInt::from_u64(ranks[end] as u64);
                NaturalInterval::new(start, end)
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| "content interval has reversed endpoints")?;
        CanonicalIntervalSet::new(intervals)
            .map_err(|_| "content separation contains overlapping intervals")
    };
    if normalized(&left)? != normalized(&right)? {
        return Err("content intervals leave a gap or change the conserved set");
    }
    Ok(())
}

fn intervals(
    context: &PropositionContext,
    equation: &ContentConservation,
    term: &ContentTerm,
) -> Result<Vec<(IntegerMathTerm, IntegerMathTerm)>, &'static str> {
    let mut pending = vec![term];
    let mut members = Vec::new();
    while let Some(term) = pending.pop() {
        match term {
            ContentTerm::Separate(children) => pending.extend(children.iter().rev()),
            ContentTerm::Projection {
                projection,
                subject,
            } => {
                let interpretation = context
                    .content_interval_projection(*projection, subject)
                    .ok_or("content projection has no checked endpoint interpretation")?;
                if &interpretation.algebra != equation.algebra() {
                    return Err("content projection changes its coordinate space or algebra");
                }
                members.extend(interpretation.members.iter().cloned());
            }
        }
    }
    Ok(members)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AcceptedProofRule, PrimitiveJudgment, ProofNode, ProofRule, accept_certificate};
    use semantic_vocabulary::{
        ContentAlgebra, ContentDomainId, ContentIntervalInterpretation, ContentPlaceSegment,
        ContentPlaceVersion, ContentProjectionIdentity, ContentStructuralPlace, PlaceId,
        StructuralPlaceKind,
    };

    fn literal(value: u64) -> IntegerMathTerm {
        IntegerMathTerm::literal(IntegerValue::Unsigned(value.into()))
    }

    fn fixture(
        bounds: [(u64, u64); 3],
    ) -> (PropositionContext, Proposition, Vec<Vec<IntegerMathTerm>>) {
        let root = PlaceId::new(1).unwrap();
        let projection = ContentProjectionIdentity {
            domain: ContentDomainId::new(1).unwrap(),
            projection_report_fingerprint: 2,
        };
        let algebra = ContentAlgebra {
            kind: ContentAlgebraKind::IntervalSet,
            parameter: "Nat".into(),
        };
        let subjects = ["whole", "left", "right"].map(|field| ContentStructuralPlace {
            version: ContentPlaceVersion::Current,
            root,
            segments: vec![ContentPlaceSegment::Field(field.into())],
        });
        let interpretations = subjects.iter().zip(bounds).map(|(subject, (start, end))| {
            ContentIntervalInterpretation {
                projection,
                subject: subject.clone(),
                algebra: algebra.clone(),
                members: vec![(literal(start), literal(end))],
            }
        });
        let context = PropositionContext::from_value_types_and_places(
            [],
            [(root, StructuralPlaceKind::Parameter { position: 0 })],
        )
        .unwrap()
        .with_content_intervals(interpretations)
        .unwrap();
        let [whole, left, right] = subjects.map(|subject| ContentTerm::Projection {
            projection,
            subject,
        });
        let goal = Proposition::ContentConservation(ContentConservation::new(
            algebra,
            whole,
            ContentTerm::separate([left, right]).unwrap(),
        ));
        let mut endpoints = bounds
            .into_iter()
            .flat_map(|(start, end)| [start, end])
            .collect::<Vec<_>>();
        endpoints.sort_unstable();
        endpoints.dedup();
        (
            context,
            goal,
            endpoints
                .into_iter()
                .map(|endpoint| vec![literal(endpoint)])
                .collect(),
        )
    }

    fn proof(goal: &Proposition, groups: Vec<Vec<IntegerMathTerm>>) -> ProofNode {
        let premises = endpoint_requirements(&groups)
            .unwrap()
            .into_iter()
            .map(|conclusion| ProofNode {
                conclusion,
                rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
            })
            .collect();
        ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::ContentIntervalNormalization {
                endpoint_groups: groups,
                premises,
            },
        }
    }

    #[test]
    fn interval_replay_matches_every_small_partition_and_overlap() {
        // Compare with direct point membership, not another normalization.
        let mask = |start: u64, end: u64| {
            (0..4).fold(0_u8, |mask, point| {
                mask | (u8::from(start <= point && point < end) << point)
            })
        };
        for whole_start in 0..=4 {
            for whole_end in 0..=4 {
                for left_start in 0..=4 {
                    for left_end in 0..=4 {
                        for right_start in 0..=4 {
                            for right_end in 0..=4 {
                                let bounds = [
                                    (whole_start, whole_end),
                                    (left_start, left_end),
                                    (right_start, right_end),
                                ];
                                let (context, goal, groups) = fixture(bounds);
                                let requirements = endpoint_requirements(&groups).unwrap();
                                let actual = check(&context, &goal, &groups, &requirements).is_ok();
                                let left = mask(left_start, left_end);
                                let right = mask(right_start, right_end);
                                let expected = bounds.iter().all(|(start, end)| start <= end)
                                    && left & right == 0
                                    && mask(whole_start, whole_end) == (left | right);
                                assert_eq!(actual, expected, "{bounds:?}");
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn kernel_checks_interval_premises_and_retains_provisional_status() {
        use crate::ProofError;
        let observed = [
            ([(0, 4), (0, 2), (2, 4)], None),
            ([(0, 4), (2, 4), (0, 2)], None),
            ([(0, 4), (3, 3), (0, 4)], None),
            (
                [(0, 4), (0, 1), (2, 4)],
                Some("content intervals leave a gap or change the conserved set"),
            ),
            (
                [(0, 4), (0, 3), (2, 4)],
                Some("content separation contains overlapping intervals"),
            ),
            (
                [(0, 4), (3, 1), (0, 4)],
                Some("content interval has reversed endpoints"),
            ),
        ]
        .map(|(bounds, expected)| {
            let (context, goal, groups) = fixture(bounds);
            let actual = accept_certificate(&context, &goal, &[], &[], &proof(&goal, groups));
            (bounds, expected, actual)
        });
        eprintln!("interval verdicts: {observed:?}");
        for (bounds, expected, actual) in observed {
            if let Some(expected) = expected {
                assert_eq!(
                    actual,
                    Err(ProofError::RulePremiseMismatch(expected)),
                    "{bounds:?}"
                );
            } else {
                let accepted = actual.unwrap_or_else(|error| panic!("{bounds:?}: {error:?}"));
                assert!(
                    accepted
                        .rules
                        .contains(&AcceptedProofRule::ContentIntervalNormalization)
                );
                assert!(AcceptedProofRule::ContentIntervalNormalization.is_provisional());
            }
        }
        let (context, goal, groups) = fixture([(0, 4), (0, 1), (2, 4)]);
        // Collapsing every endpoint would hide the gap, but the equality
        // premises include false inequalities that the kernel must reject.
        let mut forged = groups.into_iter().flatten().collect::<Vec<_>>();
        forged.sort();
        let certificate = proof(&goal, vec![forged]);
        assert!(matches!(
            accept_certificate(&context, &goal, &[], &[], &certificate),
            Err(ProofError::PrimitiveJudgment(
                crate::KernelError::JudgmentDoesNotEstablishGoal { .. }
            ))
        ));
        let missing = PropositionContext::from_value_types_and_places(
            [],
            [(
                PlaceId::new(1).unwrap(),
                StructuralPlaceKind::Parameter { position: 0 },
            )],
        )
        .unwrap();
        let (_, _, groups) = fixture([(0, 4), (0, 2), (2, 4)]);
        assert_eq!(
            accept_certificate(&missing, &goal, &[], &[], &proof(&goal, groups)),
            Err(ProofError::RulePremiseMismatch(
                "content projection has no checked endpoint interpretation"
            ))
        );
    }
}
