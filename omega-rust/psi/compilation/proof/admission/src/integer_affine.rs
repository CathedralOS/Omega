/*
    Independently checked integer affine forms, witness replay, bound mapping,
    wrapping evidence, and remainder bounds.

    We replay cited SSA definition and literal equality axioms into canonical
    affine combinations and verify affine bound conversions without search.
*/

mod bound_mapping {
    use crate::integer_rules::fixed_integer::{
        fixed_integer_as_i128, fixed_integer_term, integer_value_as_i128,
    };
    use crate::integer_rules::integer_affine::CheckedIntegerAffineForm;
    use crate::integer_rules::integer_affine::witness_checking::CheckedIntegerEndpointStep;
    use semantic_vocabulary::{
        IntegerMathLiteral, IntegerMathTerm, IntegerSign, IntegerType, IntegerValue, Proposition,
        ScalarTerm, ScalarType,
    };

    mod relative_translation {
        use super::{
            CheckedIntegerAffineForm, CheckedIntegerEndpointStep,
            IntegerAffineBoundConversionError, Proposition, ScalarTerm, push_wrapping_evidence,
        };

        pub(super) fn applies(
            form: &CheckedIntegerAffineForm,
            strict: bool,
            bound: &ScalarTerm,
        ) -> bool {
            !strict && bound == form.root() && bound.integer_value().is_none()
        }

        fn translation(
            form: &CheckedIntegerAffineForm,
        ) -> Result<(i128, Vec<Proposition>), IntegerAffineBoundConversionError> {
            let mut displacement = 0_i128;
            let mut evidence = Vec::new();
            for step in &form.endpoint_steps {
                displacement = match step {
                    CheckedIntegerEndpointStep::Add(literal) => displacement.checked_add(*literal),
                    CheckedIntegerEndpointStep::Subtract(literal) => {
                        displacement.checked_sub(*literal)
                    }
                    CheckedIntegerEndpointStep::WrappingAdd { operand, literal } => {
                        push_wrapping_evidence(
                            &mut evidence,
                            form.integer_type(),
                            operand,
                            *literal,
                        )?;
                        displacement.checked_add(*literal)
                    }
                    CheckedIntegerEndpointStep::WrappingAddBackward { operand, literal } => {
                        push_wrapping_evidence(
                            &mut evidence,
                            form.integer_type(),
                            operand,
                            *literal,
                        )?;
                        displacement.checked_sub(*literal)
                    }
                    _ => return Err(IntegerAffineBoundConversionError::StrictBoundNotTranslation),
                }
                .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?;
            }
            if displacement == 0 {
                return Err(IntegerAffineBoundConversionError::NonProgressingTranslation);
            }
            Ok((displacement, evidence))
        }

        pub(super) fn evidence(
            form: &CheckedIntegerAffineForm,
        ) -> Result<Vec<Proposition>, IntegerAffineBoundConversionError> {
            translation(form).map(|(_, evidence)| evidence)
        }

        pub(super) fn map(
            form: &CheckedIntegerAffineForm,
            provided: &[&Proposition],
        ) -> Result<Proposition, IntegerAffineBoundConversionError> {
            let (displacement, mut required) = translation(form)?;
            for member in provided {
                let Some(position) = required.iter().position(|required| required == *member)
                else {
                    return Err(IntegerAffineBoundConversionError::WrappingEvidenceUnexpected);
                };
                required.remove(position);
            }
            if !required.is_empty() {
                return Err(IntegerAffineBoundConversionError::WrappingEvidenceMissing);
            }
            Ok(if displacement < 0 {
                Proposition::LessThan(form.target().clone(), form.root().clone())
            } else {
                Proposition::LessThan(form.root().clone(), form.target().clone())
            })
        }

        #[cfg(test)]
        mod tests {
            use crate::{
                IntegerAffineBoundConversionError, IntegerAffineWitness, PrimitiveJudgment,
                ProofNode, ProofRule, affine_witness, check_certificate,
                check_integer_affine_witness, integer_affine_wrapping_evidence,
                map_integer_affine_bound, proof_node, value_context,
            };
            use semantic_vocabulary::{
                IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext,
                ScalarTerm, ScalarType, ValueId,
            };
            use std::iter::once;

            fn value(carrier: IntegerType, raw: u64) -> ScalarTerm {
                ScalarTerm::value(ValueId::new(raw).unwrap(), ScalarType::Integer(carrier))
            }
            fn literal(carrier: IntegerType, raw: u64) -> ScalarTerm {
                ScalarTerm::integer(
                    carrier,
                    IntegerValue::for_sign(carrier.sign(), i128::from(raw)).unwrap(),
                )
                .unwrap()
            }
            fn context(carrier: IntegerType) -> PropositionContext {
                value_context(ScalarType::Integer(carrier), 4)
            }
            fn operation(
                carrier: IntegerType,
                operand: u64,
                amount: u64,
                wrapping: bool,
                subtract: bool,
            ) -> ScalarTerm {
                let left = Box::new(value(carrier, operand));
                let right = Box::new(literal(carrier, amount));
                match (wrapping, subtract) {
                    (true, false) => ScalarTerm::WrappingIntegerAdd {
                        scalar_type: carrier,
                        left,
                        right,
                    },
                    (true, true) => ScalarTerm::WrappingIntegerSubtract {
                        scalar_type: carrier,
                        left,
                        right,
                    },
                    (false, false) => ScalarTerm::ExactIntegerAdd {
                        scalar_type: carrier,
                        left,
                        right,
                    },
                    (false, true) => ScalarTerm::ExactIntegerSubtract {
                        scalar_type: carrier,
                        left,
                        right,
                    },
                }
            }
            fn witness(carrier: IntegerType, end: u64, count: usize) -> IntegerAffineWitness {
                affine_witness(
                    value(carrier, 1),
                    value(carrier, end),
                    (0..count).collect(),
                    vec![None; count],
                )
            }
            fn reflexive(root: &ScalarTerm) -> ProofNode {
                ProofNode {
                    conclusion: Proposition::LessOrEqual(root.clone(), root.clone()),
                    rule: ProofRule::IntegerOrderWeakening {
                        relation: Box::new(
                            proof_node!(Proposition::Equal(root.clone(), root.clone()) => ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality)),
                        ),
                    },
                }
            }
            fn certificate(
                witness: IntegerAffineWitness,
                guards: &[Proposition],
                goal: Proposition,
            ) -> ProofNode {
                let base = reflexive(&witness.root);
                let root_bound = if guards.is_empty() {
                    base
                } else {
                    let mut children = vec![base];
                    children.extend(guards.iter().enumerate().map(
                |(index, guard)| proof_node!(guard.clone() => ProofRule::Assumption { index }),
            ));
                    ProofNode {
                        conclusion: Proposition::Conjunction(
                            children
                                .iter()
                                .map(|child| child.conclusion.clone())
                                .collect(),
                        ),
                        rule: ProofRule::ConjunctionIntroduction(children),
                    }
                };
                ProofNode {
                    conclusion: goal,
                    rule: ProofRule::IntegerAffineBound {
                        root_bound: Box::new(root_bound),
                        witness,
                    },
                }
            }
            fn bounded_root(root: &ScalarTerm, guards: &[Proposition]) -> Proposition {
                let base = Proposition::LessOrEqual(root.clone(), root.clone());
                if guards.is_empty() {
                    base
                } else {
                    Proposition::Conjunction(once(base).chain(guards.iter().cloned()).collect())
                }
            }

            #[test]
            fn relative_additive_translation_certifies_signed_and_unsigned_progress() {
                for sign in [IntegerSign::Signed, IntegerSign::Unsigned] {
                    let carrier = IntegerType::new(sign, 64).unwrap();
                    for wrapping in [false, true] {
                        for subtract in [false, true] {
                            let context = context(carrier);
                            let axioms = [Proposition::Equal(
                                value(carrier, 2),
                                operation(carrier, 1, 1, wrapping, subtract),
                            )];
                            let witness = witness(carrier, 2, 1);
                            let form =
                                check_integer_affine_witness(&context, &axioms, &witness).unwrap();
                            let guards = integer_affine_wrapping_evidence(
                                &form,
                                &reflexive(&witness.root).conclusion,
                            )
                            .unwrap();
                            assert_eq!(guards.len(), usize::from(wrapping));
                            let goal = map_integer_affine_bound(
                                &form,
                                &bounded_root(&witness.root, &guards),
                            )
                            .unwrap();
                            let expected = if subtract {
                                Proposition::LessThan(witness.target.clone(), witness.root.clone())
                            } else {
                                Proposition::LessThan(witness.root.clone(), witness.target.clone())
                            };
                            assert_eq!(goal, expected);
                            let proof = certificate(witness, &guards, goal);
                            check_certificate(
                                &context,
                                &proof.conclusion,
                                &guards,
                                &axioms,
                                &proof,
                            )
                            .unwrap();
                            if wrapping {
                                assert!(
                                    check_certificate(
                                        &context,
                                        &proof.conclusion,
                                        &[],
                                        &axioms,
                                        &proof
                                    )
                                    .is_err()
                                );
                            }
                        }
                    }
                }
            }

            #[test]
            fn relative_additive_translation_requires_each_intermediate_wrapping_guard() {
                let carrier = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
                let context = context(carrier);
                let axioms = [
                    Proposition::Equal(value(carrier, 2), operation(carrier, 1, 2, true, false)),
                    Proposition::Equal(value(carrier, 3), operation(carrier, 2, 3, true, true)),
                ];
                let witness = witness(carrier, 3, 2);
                let form = check_integer_affine_witness(&context, &axioms, &witness).unwrap();
                let guards =
                    integer_affine_wrapping_evidence(&form, &reflexive(&witness.root).conclusion)
                        .unwrap();
                assert_eq!(guards.len(), 2);
                assert_eq!(
                    guards[1],
                    Proposition::LessOrEqual(literal(carrier, 3), value(carrier, 2))
                );
                let goal =
                    map_integer_affine_bound(&form, &bounded_root(&witness.root, &guards)).unwrap();
                let proof = certificate(witness.clone(), &guards, goal.clone());
                check_certificate(&context, &goal, &guards, &axioms, &proof).unwrap();
                for dropped in 0..2 {
                    let mut incomplete = guards.clone();
                    incomplete.remove(dropped);
                    let forged = certificate(witness.clone(), &incomplete, goal.clone());
                    assert!(
                        check_certificate(&context, &goal, &incomplete, &axioms, &forged).is_err()
                    );
                }
                let mut wrong = guards.clone();
                wrong[1] = Proposition::LessOrEqual(literal(carrier, 3), value(carrier, 1));
                let forged = certificate(witness, &wrong, goal.clone());
                assert!(check_certificate(&context, &goal, &wrong, &axioms, &forged).is_err());
            }

            #[test]
            fn relative_additive_translation_inverse_keeps_the_original_operand_guard() {
                let carrier = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
                let context = context(carrier);
                let axioms = [Proposition::Equal(
                    value(carrier, 2),
                    operation(carrier, 1, 1, true, false),
                )];
                let mut witness = witness(carrier, 1, 1);
                witness.root = value(carrier, 2);
                let form = check_integer_affine_witness(&context, &axioms, &witness).unwrap();
                let guards =
                    integer_affine_wrapping_evidence(&form, &reflexive(&witness.root).conclusion)
                        .unwrap();
                let goal =
                    map_integer_affine_bound(&form, &bounded_root(&witness.root, &guards)).unwrap();
                assert_eq!(
                    goal,
                    Proposition::LessThan(value(carrier, 1), value(carrier, 2))
                );
                let proof = certificate(witness, &guards, goal);
                check_certificate(&context, &proof.conclusion, &guards, &axioms, &proof).unwrap();
            }

            #[test]
            fn relative_additive_translation_refuses_zero_scaling_and_carrier_drift() {
                let carrier = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
                let context = context(carrier);
                let zero_axioms = [
                    Proposition::Equal(value(carrier, 2), operation(carrier, 1, 1, true, false)),
                    Proposition::Equal(value(carrier, 3), operation(carrier, 2, 1, true, true)),
                ];
                let witness = witness(carrier, 3, 2);
                let form = check_integer_affine_witness(&context, &zero_axioms, &witness).unwrap();
                assert_eq!(
                    integer_affine_wrapping_evidence(&form, &reflexive(&witness.root).conclusion),
                    Err(IntegerAffineBoundConversionError::NonProgressingTranslation)
                );
                let scaling = [Proposition::Equal(
                    value(carrier, 2),
                    ScalarTerm::ExactIntegerMultiply {
                        scalar_type: carrier,
                        left: Box::new(value(carrier, 1)),
                        right: Box::new(literal(carrier, 2)),
                    },
                )];
                let mut witness = witness;
                witness.target = value(carrier, 2);
                witness.definition_axioms = vec![0];
                witness.literal_axioms = vec![None];
                let form = check_integer_affine_witness(&context, &scaling, &witness).unwrap();
                assert_eq!(
                    integer_affine_wrapping_evidence(&form, &reflexive(&witness.root).conclusion),
                    Err(IntegerAffineBoundConversionError::StrictBoundNotTranslation)
                );
                witness.root = value(IntegerType::new(IntegerSign::Unsigned, 32).unwrap(), 1);
                assert!(check_integer_affine_witness(&context, &scaling, &witness).is_err());
            }
        }
    }

    pub fn check_integer_affine_bound_conversion(
        form: &CheckedIntegerAffineForm,
        root_bound: &Proposition,
        conclusion: &Proposition,
    ) -> Result<(), IntegerAffineBoundConversionError> {
        let expected = map_integer_affine_bound(form, root_bound);
        if expected
            .as_ref()
            .is_ok_and(|expected| conclusion == expected)
        {
            return Ok(());
        }
        if map_integer_affine_bound_with_remainder_policy(
            form,
            root_bound,
            RemainderUpperBound::TotalImage,
        )
        .as_ref()
        .is_ok_and(|legacy| conclusion == legacy)
        {
            return Ok(());
        }
        expected?;
        Err(IntegerAffineBoundConversionError::ConclusionMismatch)
    }

    #[derive(Clone, Copy)]
    enum RemainderUpperBound {
        DividendIntersection,
        TotalImage,
    }

    pub fn map_integer_affine_bound(
        form: &CheckedIntegerAffineForm,
        root_bound: &Proposition,
    ) -> Result<Proposition, IntegerAffineBoundConversionError> {
        map_integer_affine_bound_with_remainder_policy(
            form,
            root_bound,
            RemainderUpperBound::DividendIntersection,
        )
    }

    #[derive(Clone, Copy)]
    enum BoundRoute {
        Remainder,
        RemainderWithinDividend,
        CorrelatedAdd,
        CorrelatedSubtract,
        CorrelatedMultiply,
        DirectAdd,
        DirectSubtract,
        DirectMultiply,
        DirectShift,
        Chain,
    }

    fn bound_route(form: &CheckedIntegerAffineForm) -> BoundRoute {
        match form.endpoint_steps.as_slice() {
            [CheckedIntegerEndpointStep::CorrelatedUnsignedRemainder] => BoundRoute::Remainder,
            [CheckedIntegerEndpointStep::RemainderWithinDividend { .. }] => {
                BoundRoute::RemainderWithinDividend
            }
            [CheckedIntegerEndpointStep::CorrelatedAddLower]
            | [CheckedIntegerEndpointStep::CorrelatedAddUpper] => BoundRoute::CorrelatedAdd,
            [CheckedIntegerEndpointStep::CorrelatedSubtractLower]
            | [CheckedIntegerEndpointStep::CorrelatedSubtractUpper]
            | [CheckedIntegerEndpointStep::CorrelatedUnsignedSubtract] => {
                BoundRoute::CorrelatedSubtract
            }
            [CheckedIntegerEndpointStep::CorrelatedMultiplyMinimum]
            | [CheckedIntegerEndpointStep::CorrelatedMultiplyMaximum] => {
                BoundRoute::CorrelatedMultiply
            }
            [] => match form.target() {
                ScalarTerm::ExactIntegerAdd { .. } => BoundRoute::DirectAdd,
                ScalarTerm::ExactIntegerSubtract { .. } => BoundRoute::DirectSubtract,
                ScalarTerm::ExactIntegerMultiply { .. }
                | ScalarTerm::WrappingIntegerMultiply { .. } => BoundRoute::DirectMultiply,
                ScalarTerm::ExactIntegerShiftLeft { .. } => BoundRoute::DirectShift,
                _ => BoundRoute::Chain,
            },
            _ => BoundRoute::Chain,
        }
    }

    struct ChainWalk {
        mapped: i128,
        reverses_order: bool,
        required_evidence: Vec<Proposition>,
    }

    fn walk_endpoint_steps(
        form: &CheckedIntegerAffineForm,
        strict: bool,
        root_is_lower_endpoint: bool,
        bound: &ScalarTerm,
        remainder_upper_bound: RemainderUpperBound,
    ) -> Result<ChainWalk, IntegerAffineBoundConversionError> {
        let mut mapped = fixed_integer_as_i128(bound, form.integer_type())
            .ok_or(IntegerAffineBoundConversionError::RootBoundNotTypedLiteral)?;
        let mut reverses_order = false;
        let mut required_evidence = Vec::new();
        for step in &form.endpoint_steps {
            let current_is_lower = root_is_lower_endpoint ^ reverses_order;
            if strict && !step.preserves_strict_endpoint() {
                return Err(IntegerAffineBoundConversionError::StrictBoundNotTranslation);
            }
            mapped = match step {
                CheckedIntegerEndpointStep::Add(value) => mapped.checked_add(*value),
                CheckedIntegerEndpointStep::SaturatingAdd(value) => {
                    let minimum = integer_value_as_i128(form.integer_type().minimum_value())
                        .ok_or(IntegerAffineBoundConversionError::MappedBoundOutsideCarrier)?;
                    let maximum = integer_value_as_i128(form.integer_type().maximum_value())
                        .ok_or(IntegerAffineBoundConversionError::MappedBoundOutsideCarrier)?;
                    mapped
                        .checked_add(*value)
                        .map(|endpoint| endpoint.clamp(minimum, maximum))
                }
                CheckedIntegerEndpointStep::Subtract(value) => mapped.checked_sub(*value),
                CheckedIntegerEndpointStep::Multiply(value) => {
                    if *value < 0 {
                        reverses_order = !reverses_order;
                    }
                    mapped.checked_mul(*value)
                }
                CheckedIntegerEndpointStep::Divide(value) => {
                    if *value < 0 {
                        reverses_order = !reverses_order;
                    }
                    mapped.checked_div(*value)
                }
                CheckedIntegerEndpointStep::Remainder(value) => {
                    let magnitude = value
                        .checked_abs()
                        .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?;
                    Some(
                        if current_is_lower && form.integer_type().sign() == IntegerSign::Signed {
                            if mapped >= 0 {
                                0
                            } else {
                                1_i128
                                    .checked_sub(magnitude)
                                    .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?
                            }
                        } else if current_is_lower
                            || (form.integer_type().sign() == IntegerSign::Signed && mapped <= 0)
                        {
                            0
                        } else {
                            let image_maximum = magnitude
                                .checked_sub(1)
                                .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?;
                            if form.integer_type().sign() == IntegerSign::Unsigned
                                && matches!(
                                    remainder_upper_bound,
                                    RemainderUpperBound::DividendIntersection
                                )
                            {
                                mapped.min(image_maximum)
                            } else {
                                image_maximum
                            }
                        },
                    )
                }
                CheckedIntegerEndpointStep::ShiftLeft(count) => mapped.checked_mul(
                    1_i128
                        .checked_shl(*count)
                        .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?,
                ),
                CheckedIntegerEndpointStep::ShiftRight(count) => Some(mapped >> count),
                CheckedIntegerEndpointStep::BitwiseAndMask(mask) => {
                    Some(if current_is_lower { 0 } else { *mask })
                }
                CheckedIntegerEndpointStep::BitwiseAndBoundedMask { upper, .. } => {
                    Some(if current_is_lower { 0 } else { *upper })
                }
                CheckedIntegerEndpointStep::BitwiseOrLiteralImage(literal) => {
                    Some(if current_is_lower {
                        mapped
                    } else if mapped < 0 {
                        -1
                    } else {
                        bitwise_or_image_maximum(mapped, *literal)
                            .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?
                    })
                }
                CheckedIntegerEndpointStep::BitwiseOrBoundedImage { upper, .. } => {
                    Some(if current_is_lower {
                        mapped
                    } else if mapped < 0 {
                        -1
                    } else {
                        bitwise_or_image_maximum(mapped, *upper)
                            .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?
                    })
                }
                CheckedIntegerEndpointStep::WrappingAdd { operand, literal } => {
                    if wrapping_side_needs_evidence(current_is_lower, *literal) {
                        push_wrapping_evidence(
                            &mut required_evidence,
                            form.integer_type(),
                            operand,
                            *literal,
                        )?;
                    }
                    mapped.checked_add(*literal)
                }
                CheckedIntegerEndpointStep::WrappingAddBackward { operand, literal } => {
                    if wrapping_side_needs_evidence(!current_is_lower, *literal) {
                        push_wrapping_evidence(
                            &mut required_evidence,
                            form.integer_type(),
                            operand,
                            *literal,
                        )?;
                    }
                    mapped.checked_sub(*literal)
                }
                CheckedIntegerEndpointStep::CorrelatedAddLower
                | CheckedIntegerEndpointStep::CorrelatedAddUpper
                | CheckedIntegerEndpointStep::CorrelatedSubtractLower
                | CheckedIntegerEndpointStep::CorrelatedSubtractUpper
                | CheckedIntegerEndpointStep::CorrelatedUnsignedSubtract
                | CheckedIntegerEndpointStep::CorrelatedUnsignedRemainder
                | CheckedIntegerEndpointStep::RemainderWithinDividend { .. }
                | CheckedIntegerEndpointStep::CorrelatedMultiplyMinimum
                | CheckedIntegerEndpointStep::CorrelatedMultiplyMaximum => {
                    return Err(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch);
                }
            }
            .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?;
        }
        Ok(ChainWalk {
            mapped,
            reverses_order,
            required_evidence,
        })
    }

    fn map_integer_affine_bound_with_remainder_policy(
        form: &CheckedIntegerAffineForm,
        root_bound: &Proposition,
        remainder_upper_bound: RemainderUpperBound,
    ) -> Result<Proposition, IntegerAffineBoundConversionError> {
        match bound_route(form) {
            BoundRoute::Remainder => return super::remainder::map_bound(form, root_bound),
            BoundRoute::RemainderWithinDividend => {
                return super::remainder::map_dividend_bound(form, root_bound);
            }
            BoundRoute::CorrelatedAdd => return map_correlated_add_bound(form, root_bound),
            BoundRoute::CorrelatedSubtract => {
                return map_correlated_subtract_bound(form, root_bound);
            }
            BoundRoute::CorrelatedMultiply => {
                return map_correlated_multiply_bound(form, root_bound);
            }
            BoundRoute::DirectAdd => return map_direct_add_bound(form, root_bound),
            BoundRoute::DirectSubtract => return map_direct_subtract_bound(form, root_bound),
            BoundRoute::DirectMultiply => return map_direct_multiply_bound(form, root_bound),
            BoundRoute::DirectShift => return map_direct_shift_left_bound(form, root_bound),
            BoundRoute::Chain => {}
        }
        let ChainRootBound {
            strict,
            bound,
            root_is_lower_endpoint,
            evidence_members,
        } = destructure_chain_root_bound(form, root_bound)?;
        if relative_translation::applies(form, strict, bound) {
            return relative_translation::map(form, &evidence_members);
        }
        let mut walk = walk_endpoint_steps(
            form,
            strict,
            root_is_lower_endpoint,
            bound,
            remainder_upper_bound,
        )?;
        for member in evidence_members {
            let Some(position) = walk
                .required_evidence
                .iter()
                .position(|required| required == member)
            else {
                return Err(IntegerAffineBoundConversionError::WrappingEvidenceUnexpected);
            };
            walk.required_evidence.remove(position);
        }
        if !walk.required_evidence.is_empty() {
            return Err(IntegerAffineBoundConversionError::WrappingEvidenceMissing);
        }
        let mapped = fixed_integer_term(form.integer_type(), walk.mapped)
            .ok_or(IntegerAffineBoundConversionError::MappedBoundOutsideCarrier)?;

        let target_is_left = if walk.reverses_order {
            root_is_lower_endpoint
        } else {
            !root_is_lower_endpoint
        };
        Ok(match (strict, target_is_left) {
            (false, true) => Proposition::LessOrEqual(form.target().clone(), mapped),
            (false, false) => Proposition::LessOrEqual(mapped, form.target().clone()),
            (true, true) => Proposition::LessThan(form.target().clone(), mapped),
            (true, false) => Proposition::LessThan(mapped, form.target().clone()),
        })
    }

    struct ChainRootBound<'a> {
        strict: bool,
        bound: &'a ScalarTerm,
        root_is_lower_endpoint: bool,
        evidence_members: Vec<&'a Proposition>,
    }

    fn destructure_chain_root_bound<'a>(
        form: &CheckedIntegerAffineForm,
        root_bound: &'a Proposition,
    ) -> Result<ChainRootBound<'a>, IntegerAffineBoundConversionError> {
        let (relation, evidence_members) = match root_bound {
            Proposition::Conjunction(members) => {
                let Some((relation, evidence)) = members.split_first() else {
                    return Err(IntegerAffineBoundConversionError::RootBoundNotLessOrEqual);
                };
                (relation, evidence.iter().collect::<Vec<_>>())
            }
            _ => (root_bound, Vec::new()),
        };
        let (strict, bound_left, bound_right) = match relation {
            Proposition::LessOrEqual(left, right) => (false, left, right),
            Proposition::LessThan(left, right) => (true, left, right),
            _ => return Err(IntegerAffineBoundConversionError::RootBoundNotLessOrEqual),
        };
        let (bound, root_is_lower_endpoint) = if bound_left == form.root() {
            (bound_right, false)
        } else if bound_right == form.root() {
            (bound_left, true)
        } else {
            return Err(IntegerAffineBoundConversionError::RootBoundMismatch);
        };
        Ok(ChainRootBound {
            strict,
            bound,
            root_is_lower_endpoint,
            evidence_members,
        })
    }

    pub fn integer_affine_wrapping_evidence(
        form: &CheckedIntegerAffineForm,
        relation: &Proposition,
    ) -> Result<Vec<Proposition>, IntegerAffineBoundConversionError> {
        let ChainRootBound {
            strict,
            bound,
            root_is_lower_endpoint,
            evidence_members,
        } = destructure_chain_root_bound(form, relation)?;
        if !evidence_members.is_empty() {
            return Err(IntegerAffineBoundConversionError::WrappingEvidenceUnexpected);
        }
        if relative_translation::applies(form, strict, bound) {
            return relative_translation::evidence(form);
        }
        let BoundRoute::Chain = bound_route(form) else {
            return Ok(Vec::new());
        };
        Ok(walk_endpoint_steps(
            form,
            strict,
            root_is_lower_endpoint,
            bound,
            RemainderUpperBound::DividendIntersection,
        )?
        .required_evidence)
    }

    fn wrapping_side_needs_evidence(current_is_lower: bool, literal: i128) -> bool {
        (literal > 0 && current_is_lower) || (literal < 0 && !current_is_lower)
    }

    fn push_wrapping_evidence(
        required_evidence: &mut Vec<Proposition>,
        integer_type: IntegerType,
        operand: &ScalarTerm,
        literal: i128,
    ) -> Result<(), IntegerAffineBoundConversionError> {
        let evidence = match integer_type.maximum_value() {
            IntegerValue::Unsigned(_) if literal < 0 => {
                let floor = ScalarTerm::integer(
                    integer_type,
                    IntegerValue::Unsigned(literal.unsigned_abs()),
                )
                .map_err(|_| IntegerAffineBoundConversionError::MappedBoundOutsideCarrier)?;
                Proposition::LessOrEqual(floor, operand.clone())
            }
            IntegerValue::Unsigned(maximum) => {
                let headroom = maximum
                    .checked_sub(u128::try_from(literal).unwrap_or(u128::MAX))
                    .ok_or(IntegerAffineBoundConversionError::MappedBoundOutsideCarrier)?;
                let bound = ScalarTerm::integer(integer_type, IntegerValue::Unsigned(headroom))
                    .map_err(|_| IntegerAffineBoundConversionError::MappedBoundOutsideCarrier)?;
                Proposition::LessOrEqual(operand.clone(), bound)
            }
            IntegerValue::Signed(maximum) => {
                let IntegerValue::Signed(minimum) = integer_type.minimum_value() else {
                    return Err(IntegerAffineBoundConversionError::MappedBoundOutsideCarrier);
                };
                let signed_bound = |value: Option<i128>| {
                    value
                        .and_then(|value| {
                            ScalarTerm::integer(integer_type, IntegerValue::Signed(value)).ok()
                        })
                        .ok_or(IntegerAffineBoundConversionError::MappedBoundOutsideCarrier)
                };
                if literal > 0 {
                    Proposition::LessOrEqual(
                        operand.clone(),
                        signed_bound(maximum.checked_sub(literal))?,
                    )
                } else {
                    Proposition::LessOrEqual(
                        signed_bound(minimum.checked_sub(literal))?,
                        operand.clone(),
                    )
                }
            }
        };
        if !required_evidence.contains(&evidence) {
            required_evidence.push(evidence);
        }
        Ok(())
    }

    fn map_correlated_add_bound(
        form: &CheckedIntegerAffineForm,
        evidence: &Proposition,
    ) -> Result<Proposition, IntegerAffineBoundConversionError> {
        let ScalarTerm::ExactIntegerAdd { left, right, .. } = form.target() else {
            return Err(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch);
        };
        let lower = matches!(
            form.endpoint_steps.as_slice(),
            [CheckedIntegerEndpointStep::CorrelatedAddLower]
        );
        let expected = if lower {
            Proposition::LessOrEqual(form.root().clone(), left.as_ref().clone())
        } else {
            Proposition::LessOrEqual(left.as_ref().clone(), form.root().clone())
        };
        if evidence != &expected {
            return Err(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch);
        }
        let sum = directed_op_term(
            form,
            left,
            right,
            IntegerAffineBoundConversionError::DirectAddEvidenceMismatch,
            |left, right| IntegerMathTerm::Add(Box::new(left), Box::new(right)),
        )?;
        Ok(carrier_endpoint_image(form, sum, lower))
    }

    fn map_correlated_subtract_bound(
        form: &CheckedIntegerAffineForm,
        evidence: &Proposition,
    ) -> Result<Proposition, IntegerAffineBoundConversionError> {
        let ScalarTerm::ExactIntegerSubtract { left, right, .. } = form.target() else {
            return Err(IntegerAffineBoundConversionError::DirectSubtractEvidenceMismatch);
        };
        let lower = matches!(
            form.endpoint_steps.as_slice(),
            [CheckedIntegerEndpointStep::CorrelatedSubtractLower]
                | [CheckedIntegerEndpointStep::CorrelatedUnsignedSubtract]
        );
        let unsigned = matches!(
            form.endpoint_steps.as_slice(),
            [CheckedIntegerEndpointStep::CorrelatedUnsignedSubtract]
        );
        let expected = if unsigned {
            Proposition::LessOrEqual(right.as_ref().clone(), left.as_ref().clone())
        } else if lower {
            Proposition::LessOrEqual(form.root().clone(), left.as_ref().clone())
        } else {
            Proposition::LessOrEqual(left.as_ref().clone(), form.root().clone())
        };
        if evidence != &expected {
            return Err(IntegerAffineBoundConversionError::DirectSubtractEvidenceMismatch);
        }
        let difference = directed_op_term(
            form,
            left,
            right,
            IntegerAffineBoundConversionError::DirectSubtractEvidenceMismatch,
            |left, right| IntegerMathTerm::Subtract(Box::new(left), Box::new(right)),
        )?;
        Ok(carrier_endpoint_image(form, difference, lower))
    }

    fn map_correlated_multiply_bound(
        form: &CheckedIntegerAffineForm,
        evidence: &Proposition,
    ) -> Result<Proposition, IntegerAffineBoundConversionError> {
        let ScalarTerm::ExactIntegerMultiply { left, right, .. } = form.target() else {
            return Err(IntegerAffineBoundConversionError::DirectMultiplyEvidenceMismatch);
        };
        let Proposition::Conjunction(parts) = evidence else {
            return Err(IntegerAffineBoundConversionError::DirectMultiplyEvidenceMismatch);
        };
        let [sign_evidence, bound_evidence] = parts.as_slice() else {
            return Err(IntegerAffineBoundConversionError::DirectMultiplyEvidenceMismatch);
        };
        let one = ScalarTerm::integer(form.integer_type(), form.integer_type().one())
            .map_err(|_| IntegerAffineBoundConversionError::DirectMultiplyEvidenceMismatch)?;
        let positive = sign_evidence == &Proposition::LessOrEqual(one, right.as_ref().clone());
        let negative_two = ScalarTerm::integer(form.integer_type(), IntegerValue::Signed(-2)).ok();
        let negative = negative_two.is_some_and(|negative_two| {
            sign_evidence == &Proposition::LessOrEqual(right.as_ref().clone(), negative_two)
        });
        if !positive && !negative {
            return Err(IntegerAffineBoundConversionError::DirectMultiplyEvidenceMismatch);
        }
        let endpoint_minimum = matches!(
            form.endpoint_steps.as_slice(),
            [CheckedIntegerEndpointStep::CorrelatedMultiplyMinimum]
        );
        let lower = endpoint_minimum;
        let expected_bound = if lower == positive {
            Proposition::LessOrEqual(form.root().clone(), left.as_ref().clone())
        } else {
            Proposition::LessOrEqual(left.as_ref().clone(), form.root().clone())
        };
        if bound_evidence != &expected_bound {
            return Err(IntegerAffineBoundConversionError::DirectMultiplyEvidenceMismatch);
        }
        let product = directed_op_term(
            form,
            left,
            right,
            IntegerAffineBoundConversionError::DirectMultiplyEvidenceMismatch,
            |left, right| IntegerMathTerm::Multiply(Box::new(left), Box::new(right)),
        )?;
        Ok(carrier_endpoint_image(form, product, lower))
    }

    fn map_direct_add_bound(
        form: &CheckedIntegerAffineForm,
        evidence: &Proposition,
    ) -> Result<Proposition, IntegerAffineBoundConversionError> {
        let Proposition::Conjunction(parts) = evidence else {
            return Err(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch);
        };
        let [left_evidence, right_evidence] = parts.as_slice() else {
            return Err(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch);
        };
        let ScalarTerm::ExactIntegerAdd {
            scalar_type,
            left,
            right,
        } = form.target()
        else {
            return Err(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch);
        };
        if *scalar_type != form.integer_type() {
            return Err(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch);
        }
        let left_endpoint = direct_operand_endpoint(left, left_evidence, form.integer_type())?;
        let right_endpoint = direct_operand_endpoint(right, right_evidence, form.integer_type())?;
        let lower = match (left_endpoint.orientation(), right_endpoint.orientation()) {
            (Some(left), Some(right)) if left == right => left,
            (None, Some(right)) => right,
            (Some(left), None) => left,
            (None, None) => {
                return Err(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch);
            }
            _ => return Err(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch),
        };
        let left_bound = left_endpoint.literal(form.integer_type(), lower);
        let right_bound = right_endpoint.literal(form.integer_type(), lower);
        let bound = add_math_literals(left_bound, right_bound)?;
        let sum = directed_op_term(
            form,
            left,
            right,
            IntegerAffineBoundConversionError::DirectAddEvidenceMismatch,
            |left, right| IntegerMathTerm::Add(Box::new(left), Box::new(right)),
        )?;
        Ok(literal_endpoint_image(sum, bound, lower))
    }

    fn map_direct_subtract_bound(
        form: &CheckedIntegerAffineForm,
        evidence: &Proposition,
    ) -> Result<Proposition, IntegerAffineBoundConversionError> {
        let Proposition::Conjunction(parts) = evidence else {
            return Err(IntegerAffineBoundConversionError::DirectSubtractEvidenceMismatch);
        };
        let [left_evidence, right_evidence] = parts.as_slice() else {
            return Err(IntegerAffineBoundConversionError::DirectSubtractEvidenceMismatch);
        };
        let ScalarTerm::ExactIntegerSubtract {
            scalar_type,
            left,
            right,
        } = form.target()
        else {
            return Err(IntegerAffineBoundConversionError::DirectSubtractEvidenceMismatch);
        };
        if *scalar_type != form.integer_type() {
            return Err(IntegerAffineBoundConversionError::DirectSubtractEvidenceMismatch);
        }
        let left_endpoint = direct_operand_endpoint(left, left_evidence, form.integer_type())?;
        let right_endpoint = direct_operand_endpoint(right, right_evidence, form.integer_type())?;
        let lower = match (left_endpoint.orientation(), right_endpoint.orientation()) {
            (Some(left), Some(right)) if left != right => left,
            (None, Some(right)) => !right,
            (Some(left), None) => left,
            (None, None)
                if matches!(
                    (&left_endpoint, &right_endpoint),
                    (DirectAddEndpoint::Exact(left), DirectAddEndpoint::Carrier)
                        if left.as_integer_value(form.integer_type())
                            == Some(form.integer_type().maximum_value())
                ) =>
            {
                true
            }
            (None, None)
                if matches!(
                    (&left_endpoint, &right_endpoint),
                    (DirectAddEndpoint::Exact(left), DirectAddEndpoint::Carrier)
                        if left.as_integer_value(form.integer_type())
                            == Some(form.integer_type().minimum_value())
                ) =>
            {
                false
            }
            (None, None) => {
                return Err(IntegerAffineBoundConversionError::DirectSubtractEvidenceMismatch);
            }
            _ => return Err(IntegerAffineBoundConversionError::DirectSubtractEvidenceMismatch),
        };
        let left_bound = left_endpoint.literal(form.integer_type(), lower);
        let right_bound = right_endpoint.literal(form.integer_type(), !lower);
        let bound = subtract_math_literals(left_bound, right_bound)?;
        let difference = directed_op_term(
            form,
            left,
            right,
            IntegerAffineBoundConversionError::DirectSubtractEvidenceMismatch,
            |left, right| IntegerMathTerm::Subtract(Box::new(left), Box::new(right)),
        )?;
        Ok(literal_endpoint_image(difference, bound, lower))
    }

    fn map_direct_multiply_bound(
        form: &CheckedIntegerAffineForm,
        evidence: &Proposition,
    ) -> Result<Proposition, IntegerAffineBoundConversionError> {
        let Proposition::Conjunction(parts) = evidence else {
            return Err(IntegerAffineBoundConversionError::DirectMultiplyEvidenceMismatch);
        };
        let [left_first, left_second, right_first, right_second] = parts.as_slice() else {
            return Err(IntegerAffineBoundConversionError::DirectMultiplyEvidenceMismatch);
        };
        let (ScalarTerm::ExactIntegerMultiply {
            scalar_type,
            left,
            right,
        }
        | ScalarTerm::WrappingIntegerMultiply {
            scalar_type,
            left,
            right,
        }) = form.target()
        else {
            return Err(IntegerAffineBoundConversionError::DirectMultiplyEvidenceMismatch);
        };
        if *scalar_type != form.integer_type() {
            return Err(IntegerAffineBoundConversionError::DirectMultiplyEvidenceMismatch);
        }
        let left_first = direct_operand_endpoint(left, left_first, form.integer_type())?;
        let left_second = direct_operand_endpoint(left, left_second, form.integer_type())?;
        let right_first = direct_operand_endpoint(right, right_first, form.integer_type())?;
        let right_second = direct_operand_endpoint(right, right_second, form.integer_type())?;
        let pair_orientation = |first: &DirectAddEndpoint,
                                second: &DirectAddEndpoint|
         -> Result<Option<bool>, IntegerAffineBoundConversionError> {
            match (first.orientation(), second.orientation()) {
                (Some(first), Some(second)) if first != second => Ok(Some(first)),
                (Some(first), None) if matches!(second, DirectAddEndpoint::Carrier) => {
                    Ok(Some(first))
                }
                (None, Some(second)) if matches!(first, DirectAddEndpoint::Carrier) => {
                    Ok(Some(!second))
                }
                (None, None)
                    if matches!(
                        (first, second),
                        (DirectAddEndpoint::Exact(left), DirectAddEndpoint::Exact(right)) if left == right
                    ) || matches!(
                        (first, second),
                        (DirectAddEndpoint::Carrier, DirectAddEndpoint::Carrier)
                    ) =>
                {
                    Ok(None)
                }
                _ => Err(IntegerAffineBoundConversionError::DirectMultiplyEvidenceMismatch),
            }
        };
        let left_orientation = pair_orientation(&left_first, &left_second)?;
        let right_orientation = pair_orientation(&right_first, &right_second)?;
        let lower = match (left_orientation, right_orientation) {
            (Some(left), Some(right)) if left == right => left,
            (Some(left), None) => left,
            (None, Some(right)) => right,
            _ => return Err(IntegerAffineBoundConversionError::DirectMultiplyEvidenceMismatch),
        };
        let (left_lower, left_upper, right_lower, right_upper) = if lower {
            (
                left_first.literal(form.integer_type(), true),
                left_second.literal(form.integer_type(), false),
                right_first.literal(form.integer_type(), true),
                right_second.literal(form.integer_type(), false),
            )
        } else {
            (
                left_second.literal(form.integer_type(), true),
                left_first.literal(form.integer_type(), false),
                right_second.literal(form.integer_type(), true),
                right_first.literal(form.integer_type(), false),
            )
        };
        let products = [
            multiply_math_literals(left_lower, right_lower)?,
            multiply_math_literals(left_lower, right_upper)?,
            multiply_math_literals(left_upper, right_lower)?,
            multiply_math_literals(left_upper, right_upper)?,
        ];
        let wrapping = matches!(form.target(), ScalarTerm::WrappingIntegerMultiply { .. });
        if wrapping
            && products
                .iter()
                .any(|corner| corner.as_integer_value(*scalar_type).is_none())
        {
            return Err(IntegerAffineBoundConversionError::DirectMultiplyBoundOverflow);
        }
        let bound = products
            .into_iter()
            .reduce(|current, candidate| {
                if (lower && math_literal_less(candidate, current))
                    || (!lower && math_literal_less(current, candidate))
                {
                    candidate
                } else {
                    current
                }
            })
            .expect("four product corners");
        if wrapping {
            let bound = bound
                .as_integer_value(*scalar_type)
                .ok_or(IntegerAffineBoundConversionError::DirectMultiplyBoundOverflow)?;
            let literal = ScalarTerm::integer(*scalar_type, bound)
                .map_err(|_| IntegerAffineBoundConversionError::DirectMultiplyBoundOverflow)?;
            return Ok(if lower {
                Proposition::LessOrEqual(literal, form.target().clone())
            } else {
                Proposition::LessOrEqual(form.target().clone(), literal)
            });
        }
        let product = directed_op_term(
            form,
            left,
            right,
            IntegerAffineBoundConversionError::DirectMultiplyEvidenceMismatch,
            |left, right| IntegerMathTerm::Multiply(Box::new(left), Box::new(right)),
        )?;
        Ok(literal_endpoint_image(product, bound, lower))
    }

    enum DirectAddEndpoint {
        Exact(IntegerMathLiteral),
        Oriented {
            literal: IntegerMathLiteral,
            lower: bool,
        },
        Carrier,
    }

    impl DirectAddEndpoint {
        fn orientation(&self) -> Option<bool> {
            match self {
                Self::Oriented { lower, .. } => Some(*lower),
                Self::Exact(_) | Self::Carrier => None,
            }
        }

        fn literal(&self, integer_type: IntegerType, lower: bool) -> IntegerMathLiteral {
            match self {
                Self::Exact(literal) | Self::Oriented { literal, .. } => *literal,
                Self::Carrier => IntegerMathLiteral::from_integer_value(if lower {
                    integer_type.minimum_value()
                } else {
                    integer_type.maximum_value()
                }),
            }
        }
    }

    fn direct_operand_endpoint(
        operand: &ScalarTerm,
        evidence: &Proposition,
        integer_type: IntegerType,
    ) -> Result<DirectAddEndpoint, IntegerAffineBoundConversionError> {
        if let ScalarTerm::Integer { scalar_type, value } = operand {
            if *scalar_type != integer_type || evidence != &Proposition::Truth {
                return Err(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch);
            }
            return Ok(DirectAddEndpoint::Exact(
                IntegerMathLiteral::from_integer_value(*value),
            ));
        }
        if evidence == &Proposition::Truth {
            return Ok(DirectAddEndpoint::Carrier);
        }
        if let Proposition::Equal(left, right) = evidence {
            let literal = if left == operand {
                right
            } else if right == operand {
                left
            } else {
                return Err(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch);
            };
            let (actual, value) = literal
                .integer_value()
                .ok_or(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch)?;
            if actual != integer_type {
                return Err(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch);
            }
            return Ok(DirectAddEndpoint::Exact(
                IntegerMathLiteral::from_integer_value(value),
            ));
        }
        let Proposition::LessOrEqual(left, right) = evidence else {
            return Err(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch);
        };
        if left == right {
            return Err(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch);
        }
        let (bound, lower) = if right == operand {
            (left, true)
        } else if left == operand {
            (right, false)
        } else {
            return Err(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch);
        };
        let (actual, value) = bound
            .integer_value()
            .ok_or(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch)?;
        if actual != integer_type {
            return Err(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch);
        }
        Ok(DirectAddEndpoint::Oriented {
            literal: IntegerMathLiteral::from_integer_value(value),
            lower,
        })
    }

    fn add_math_literals(
        left: IntegerMathLiteral,
        right: IntegerMathLiteral,
    ) -> Result<IntegerMathLiteral, IntegerAffineBoundConversionError> {
        let (negative, magnitude) = if left.negative() == right.negative() {
            (
                left.negative(),
                left.magnitude()
                    .checked_add(right.magnitude())
                    .ok_or(IntegerAffineBoundConversionError::DirectAddBoundOverflow)?,
            )
        } else {
            match left.magnitude().cmp(&right.magnitude()) {
                std::cmp::Ordering::Less => {
                    (right.negative(), right.magnitude() - left.magnitude())
                }
                std::cmp::Ordering::Equal => (false, 0),
                std::cmp::Ordering::Greater => {
                    (left.negative(), left.magnitude() - right.magnitude())
                }
            }
        };
        IntegerMathLiteral::new(negative, magnitude)
            .map_err(|_| IntegerAffineBoundConversionError::DirectAddBoundOverflow)
    }

    fn subtract_math_literals(
        left: IntegerMathLiteral,
        right: IntegerMathLiteral,
    ) -> Result<IntegerMathLiteral, IntegerAffineBoundConversionError> {
        let (negative, magnitude) = if left.negative() != right.negative() {
            (
                left.negative(),
                left.magnitude()
                    .checked_add(right.magnitude())
                    .ok_or(IntegerAffineBoundConversionError::DirectSubtractBoundOverflow)?,
            )
        } else {
            match left.magnitude().cmp(&right.magnitude()) {
                std::cmp::Ordering::Less => {
                    (!left.negative(), right.magnitude() - left.magnitude())
                }
                std::cmp::Ordering::Equal => (false, 0),
                std::cmp::Ordering::Greater => {
                    (left.negative(), left.magnitude() - right.magnitude())
                }
            }
        };
        IntegerMathLiteral::new(negative, magnitude)
            .map_err(|_| IntegerAffineBoundConversionError::DirectSubtractBoundOverflow)
    }

    fn multiply_math_literals(
        left: IntegerMathLiteral,
        right: IntegerMathLiteral,
    ) -> Result<IntegerMathLiteral, IntegerAffineBoundConversionError> {
        let magnitude = left
            .magnitude()
            .checked_mul(right.magnitude())
            .ok_or(IntegerAffineBoundConversionError::DirectMultiplyBoundOverflow)?;
        IntegerMathLiteral::new(
            magnitude != 0 && left.negative() != right.negative(),
            magnitude,
        )
        .map_err(|_| IntegerAffineBoundConversionError::DirectMultiplyBoundOverflow)
    }

    fn math_literal_less(left: IntegerMathLiteral, right: IntegerMathLiteral) -> bool {
        match (left.negative(), right.negative()) {
            (true, false) => true,
            (false, true) => false,
            (true, true) => left.magnitude() > right.magnitude(),
            (false, false) => left.magnitude() < right.magnitude(),
        }
    }

    pub(crate) fn direct_math_leaf(
        term: &ScalarTerm,
        expected: IntegerType,
    ) -> Option<IntegerMathTerm> {
        match term {
            ScalarTerm::Value {
                id,
                scalar_type: ScalarType::Integer(actual),
            } if *actual == expected => Some(IntegerMathTerm::MathValue {
                source_type: expected,
                value: *id,
            }),
            ScalarTerm::Integer { scalar_type, value } if *scalar_type == expected => {
                Some(IntegerMathTerm::literal(*value))
            }
            _ => None,
        }
    }

    fn directed_op_term(
        form: &CheckedIntegerAffineForm,
        left: &ScalarTerm,
        right: &ScalarTerm,
        error: IntegerAffineBoundConversionError,
        operation: impl FnOnce(IntegerMathTerm, IntegerMathTerm) -> IntegerMathTerm,
    ) -> Result<IntegerMathTerm, IntegerAffineBoundConversionError> {
        let left = direct_math_leaf(left, form.integer_type()).ok_or(error.clone())?;
        let right = direct_math_leaf(right, form.integer_type()).ok_or(error)?;
        Ok(operation(left, right))
    }

    fn carrier_endpoint_image(
        form: &CheckedIntegerAffineForm,
        term: IntegerMathTerm,
        lower: bool,
    ) -> Proposition {
        let integer_type = form.integer_type();
        if lower {
            Proposition::IntegerMathLessOrEqual(
                IntegerMathTerm::literal(integer_type.minimum_value()),
                term,
            )
        } else {
            Proposition::IntegerMathLessOrEqual(
                term,
                IntegerMathTerm::literal(integer_type.maximum_value()),
            )
        }
    }

    fn literal_endpoint_image(
        term: IntegerMathTerm,
        literal: IntegerMathLiteral,
        lower: bool,
    ) -> Proposition {
        if lower {
            Proposition::IntegerMathLessOrEqual(IntegerMathTerm::IntegerLiteral(literal), term)
        } else {
            Proposition::IntegerMathLessOrEqual(term, IntegerMathTerm::IntegerLiteral(literal))
        }
    }

    fn map_direct_shift_left_bound(
        form: &CheckedIntegerAffineForm,
        evidence: &Proposition,
    ) -> Result<Proposition, IntegerAffineBoundConversionError> {
        let ScalarTerm::ExactIntegerShiftLeft {
            value_type,
            count_type,
            value,
            count,
        } = form.target()
        else {
            return Err(IntegerAffineBoundConversionError::DirectShiftEvidenceMismatch);
        };
        let count: &ScalarTerm = count;
        let shifted_operand = direct_math_leaf(value, *value_type)
            .ok_or(IntegerAffineBoundConversionError::DirectShiftEvidenceMismatch)?;
        let shifted_count = direct_math_leaf(count, *count_type)
            .ok_or(IntegerAffineBoundConversionError::DirectShiftEvidenceMismatch)?;
        let parts = match evidence {
            Proposition::Conjunction(parts) => parts.as_slice(),
            proposition => std::slice::from_ref(proposition),
        };
        let mut root_bound = None;
        let mut exact_count = None;
        let mut count_lower = count_type.sign() == IntegerSign::Unsigned;
        let mut count_upper = None;
        for part in parts {
            match part {
                Proposition::LessOrEqual(left, right)
                    if left == form.root() || right == form.root() =>
                {
                    if root_bound.replace(part).is_some() {
                        return Err(
                            IntegerAffineBoundConversionError::AmbiguousDirectShiftEvidence,
                        );
                    }
                }
                Proposition::Equal(left, right) if left == count || right == count => {
                    let literal = if left == count { right } else { left };
                    let (actual, value) = literal.integer_value().ok_or(
                        IntegerAffineBoundConversionError::DirectShiftCountEvidenceMismatch,
                    )?;
                    if actual != *count_type || exact_count.replace(value).is_some() {
                        return Err(
                            IntegerAffineBoundConversionError::AmbiguousDirectShiftEvidence,
                        );
                    }
                }
                Proposition::LessOrEqual(left, right) if right == count => {
                    let (actual, value) = left.integer_value().ok_or(
                        IntegerAffineBoundConversionError::DirectShiftCountEvidenceMismatch,
                    )?;
                    if actual == *count_type && integer_value_to_u128(value) == Some(0) {
                        count_lower = true;
                    }
                }
                Proposition::LessOrEqual(left, right) if left == count => {
                    let (actual, value) = right.integer_value().ok_or(
                        IntegerAffineBoundConversionError::DirectShiftCountEvidenceMismatch,
                    )?;
                    if actual != *count_type || count_upper.replace(value).is_some() {
                        return Err(
                            IntegerAffineBoundConversionError::AmbiguousDirectShiftEvidence,
                        );
                    }
                }
                _ => {
                    return Err(IntegerAffineBoundConversionError::DirectShiftEvidenceMismatch);
                }
            }
        }
        let root_bound =
            root_bound.ok_or(IntegerAffineBoundConversionError::DirectShiftRootBoundMissing)?;
        let (minimum_count, maximum_count) = if let Some((actual, embedded)) = count.integer_value()
        {
            if actual != *count_type || exact_count.is_some() || count_upper.is_some() {
                return Err(IntegerAffineBoundConversionError::DirectShiftCountEvidenceMismatch);
            }
            (embedded, embedded)
        } else if let Some(exact) = exact_count {
            if count_upper.is_some() {
                return Err(IntegerAffineBoundConversionError::AmbiguousDirectShiftEvidence);
            }
            (exact, exact)
        } else if let Some(upper) = count_upper {
            if !count_lower {
                return Err(IntegerAffineBoundConversionError::DirectShiftCountLowerMissing);
            }
            (count_type.zero(), upper)
        } else {
            if !count_lower {
                return Err(IntegerAffineBoundConversionError::DirectShiftCountLowerMissing);
            }
            (count_type.zero(), count_type.maximum_value())
        };
        let minimum_count = integer_value_to_u128(minimum_count)
            .and_then(|count| u32::try_from(count).ok())
            .filter(|count| *count < u32::from(form.integer_type().bits()))
            .ok_or(IntegerAffineBoundConversionError::DirectShiftCountOutsideValueWidth)?;
        let maximum_count = integer_value_to_u128(maximum_count)
            .and_then(|count| u32::try_from(count).ok())
            .filter(|count| *count < u32::from(form.integer_type().bits()))
            .ok_or(IntegerAffineBoundConversionError::DirectShiftCountOutsideValueWidth)?;
        if minimum_count > maximum_count {
            return Err(IntegerAffineBoundConversionError::DirectShiftCountEvidenceMismatch);
        }

        let Proposition::LessOrEqual(bound_left, bound_right) = root_bound else {
            return Err(IntegerAffineBoundConversionError::RootBoundNotLessOrEqual);
        };
        let (bound, root_is_lower_endpoint) = if bound_left == form.root() {
            (bound_right, false)
        } else if bound_right == form.root() {
            (bound_left, true)
        } else {
            return Err(IntegerAffineBoundConversionError::RootBoundMismatch);
        };
        let (actual_type, bound) = bound
            .integer_value()
            .ok_or(IntegerAffineBoundConversionError::RootBoundNotTypedLiteral)?;
        if actual_type != form.integer_type() {
            return Err(IntegerAffineBoundConversionError::RootBoundNotTypedLiteral);
        }
        let negative = matches!(bound, IntegerValue::Signed(value) if value < 0);
        let count = match (root_is_lower_endpoint, negative) {
            (true, true) | (false, false) => maximum_count,
            (true, false) | (false, true) => minimum_count,
        };
        let shifted_bound = shift_math_literal(bound, count)?;
        let shifted = IntegerMathTerm::ShiftLeft {
            value: Box::new(shifted_operand),
            count: Box::new(shifted_count),
        };
        Ok(literal_endpoint_image(
            shifted,
            shifted_bound,
            root_is_lower_endpoint,
        ))
    }

    fn integer_value_to_u128(value: IntegerValue) -> Option<u128> {
        match value {
            IntegerValue::Signed(value) => u128::try_from(value).ok(),
            IntegerValue::Unsigned(value) => Some(value),
        }
    }

    fn bitwise_or_image_maximum(left_upper: i128, right_upper: i128) -> Option<i128> {
        let wider = left_upper.max(right_upper);
        if wider < 0 {
            return None;
        }
        let width = 128 - (wider as u128).leading_zeros();
        i128::try_from((1_u128 << width) - 1).ok()
    }

    fn shift_math_literal(
        value: IntegerValue,
        count: u32,
    ) -> Result<IntegerMathLiteral, IntegerAffineBoundConversionError> {
        let literal = IntegerMathLiteral::from_integer_value(value);
        let magnitude = literal
            .magnitude()
            .checked_shl(count)
            .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?;
        IntegerMathLiteral::new(literal.negative(), magnitude)
            .map_err(|_| IntegerAffineBoundConversionError::MappedBoundOverflow)
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum IntegerAffineBoundConversionError {
        RootBoundNotLessOrEqual,
        RootBoundMismatch,
        RootBoundNotTypedLiteral,
        MappedBoundOverflow,
        MappedBoundOutsideCarrier,
        TruthRootWithoutTotalImage,
        NonTotalDivisionImage,
        DirectShiftEvidenceMismatch,
        DirectShiftRootBoundMissing,
        DirectShiftCountEvidenceMismatch,
        DirectShiftCountLowerMissing,
        DirectShiftCountOutsideValueWidth,
        AmbiguousDirectShiftEvidence,
        DirectAddEvidenceMismatch,
        DirectAddBoundOverflow,
        DirectSubtractEvidenceMismatch,
        DirectSubtractBoundOverflow,
        DirectMultiplyEvidenceMismatch,
        DirectMultiplyBoundOverflow,
        StrictBoundNotTranslation,
        NonProgressingTranslation,
        WrappingEvidenceMissing,
        WrappingEvidenceUnexpected,
        ConclusionMismatch,
    }

    crate::debug_display!(IntegerAffineBoundConversionError);

    impl std::error::Error for IntegerAffineBoundConversionError {}
}
mod remainder {
    use super::witness_checking::CheckedIntegerEndpointStep;
    use super::{
        CheckedIntegerAffineForm, IntegerAffineBoundConversionError, IntegerAffineWitnessError,
    };
    use crate::integer_rules::fixed_integer::{fixed_integer_as_i128, fixed_integer_term};
    use semantic_vocabulary::{
        IntegerCarrier, IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext,
        ScalarTerm, ScalarType,
    };
    use terminal_psi::IntegerAffineWitness;

    pub(super) fn check_witness(
        context: &PropositionContext,
        semantic_axioms: &[Proposition],
        witness: &IntegerAffineWitness,
    ) -> Result<Option<CheckedIntegerAffineForm>, IntegerAffineWitnessError> {
        let (expression, definition) = if operands(&witness.target).is_some() {
            (&witness.target, None)
        } else if let [position] = witness.definition_axioms.as_slice()
            && let Some(Proposition::Equal(result, expression)) = semantic_axioms.get(*position)
            && result == &witness.target
            && matches!(result, ScalarTerm::Value { .. })
        {
            (expression, Some(*position))
        } else {
            return Ok(None);
        };
        let Some((integer_type, dividend, divisor)) = operands(expression) else {
            return Ok(None);
        };
        if divisor != &witness.root {
            return Ok(None);
        }
        if integer_type.carrier() != IntegerCarrier::Fixed
            || integer_type.sign() != IntegerSign::Unsigned
        {
            return Ok(None);
        }
        if !matches!(witness.root, ScalarTerm::Value { .. }) {
            return Err(IntegerAffineWitnessError::RootNotValue);
        }
        if !matches!(
            dividend,
            ScalarTerm::Value { .. } | ScalarTerm::Integer { .. }
        ) {
            return Err(IntegerAffineWitnessError::TargetMismatch);
        }
        match definition {
            None if !witness.definition_axioms.is_empty() || !witness.literal_axioms.is_empty() => {
                return Err(IntegerAffineWitnessError::LiteralAxiomCountMismatch);
            }
            Some(position) => {
                if witness.literal_axioms.as_slice() != [None] {
                    return Err(IntegerAffineWitnessError::LiteralAxiomCountMismatch);
                }
                if witness.target == witness.root || &witness.target == dividend {
                    return Err(IntegerAffineWitnessError::DefinitionShapeMismatch(position));
                }
            }
            None => {}
        }
        if witness.root.scalar_type() != ScalarType::Integer(integer_type)
            || witness.target.scalar_type() != ScalarType::Integer(integer_type)
        {
            return Err(IntegerAffineWitnessError::TargetTypeMismatch);
        }
        context
            .validate(&Proposition::Equal(
                witness.root.clone(),
                expression.clone(),
            ))
            .map_err(IntegerAffineWitnessError::MalformedProposition)?;
        if let Some(position) = definition {
            context
                .validate(&semantic_axioms[position])
                .map_err(IntegerAffineWitnessError::MalformedProposition)?;
        }
        Ok(Some(CheckedIntegerAffineForm {
            root: witness.root.clone(),
            target: witness.target.clone(),
            integer_type,
            coefficient: 1,
            offset: 0,
            endpoint_steps: vec![CheckedIntegerEndpointStep::CorrelatedUnsignedRemainder],
        }))
    }

    pub(super) fn map_bound(
        form: &CheckedIntegerAffineForm,
        root_bound: &Proposition,
    ) -> Result<Proposition, IntegerAffineBoundConversionError> {
        let (lower, divisor, expected) = match root_bound {
            Proposition::LessThan(lower, divisor) => (lower, divisor, 0),
            Proposition::LessOrEqual(lower, divisor) => (lower, divisor, 1),
            _ => return Err(IntegerAffineBoundConversionError::RootBoundMismatch),
        };
        if divisor != form.root()
            || !matches!(lower, ScalarTerm::Integer { scalar_type, value }
            if *scalar_type == form.integer_type() && *value == IntegerValue::Unsigned(expected))
        {
            return Err(IntegerAffineBoundConversionError::RootBoundMismatch);
        }
        Ok(Proposition::LessThan(
            form.target().clone(),
            form.root().clone(),
        ))
    }

    /// The remainder-within-dividend form: `result = dividend % divisor`, an
    /// exact remainder whose divisor is a value, rooted at the dividend.
    ///
    /// A truncating remainder is zero or has its dividend's sign, and is no
    /// larger in magnitude, so it lies between zero and the dividend:
    /// `min(dividend, 0) <= result <= max(dividend, 0)`. That holds for every
    /// fixed carrier, signed or unsigned, at any width, wherever the
    /// remainder is defined, and its endpoints are endpoints of the dividend
    /// or zero, so no arithmetic here can overflow. `map_dividend_bound`
    /// requires the divisor proved positive or proved below minus one, which
    /// excludes a zero divisor and, by excluding every `-1`, the signed
    /// `MIN % -1`.
    ///
    /// Provisional (`AcceptedProofRule::IntegerRemainderWithinDividend`): the
    /// kernel trusts this statement; the mathematical core does not derive
    /// it. A divisor that is a literal, or that a cited landing fixes to one,
    /// is the endpoint chain's `Remainder` step and is not matched here.
    pub(super) fn check_dividend_witness(
        context: &PropositionContext,
        semantic_axioms: &[Proposition],
        witness: &IntegerAffineWitness,
    ) -> Result<Option<CheckedIntegerAffineForm>, IntegerAffineWitnessError> {
        let ([position], [None]) = (
            witness.definition_axioms.as_slice(),
            witness.literal_axioms.as_slice(),
        ) else {
            return Ok(None);
        };
        let Some(Proposition::Equal(
            result,
            ScalarTerm::ExactIntegerRemainder {
                scalar_type,
                left,
                right,
            },
        )) = semantic_axioms.get(*position)
        else {
            return Ok(None);
        };
        let (dividend, divisor) = (left.as_ref(), right.as_ref());
        let is_value = |term: &ScalarTerm| matches!(term, ScalarTerm::Value { .. });
        if result != &witness.target
            || dividend != &witness.root
            || !is_value(result)
            || !is_value(dividend)
            || !is_value(divisor)
            || scalar_type.carrier() != IntegerCarrier::Fixed
        {
            return Ok(None);
        }
        if result == dividend || result == divisor || dividend == divisor {
            return Err(IntegerAffineWitnessError::DefinitionShapeMismatch(
                *position,
            ));
        }
        let carrier = ScalarType::Integer(*scalar_type);
        if [result, dividend, divisor]
            .iter()
            .any(|term| term.scalar_type() != carrier)
        {
            return Err(IntegerAffineWitnessError::TargetTypeMismatch);
        }
        context
            .validate(&semantic_axioms[*position])
            .map_err(IntegerAffineWitnessError::MalformedProposition)?;
        Ok(Some(CheckedIntegerAffineForm {
            root: witness.root.clone(),
            target: witness.target.clone(),
            integer_type: *scalar_type,
            coefficient: 1,
            offset: 0,
            endpoint_steps: vec![CheckedIntegerEndpointStep::RemainderWithinDividend {
                divisor: divisor.clone(),
            }],
        }))
    }

    /// The image of one bound on the dividend. The child is the conjunction
    /// of that bound, `dividend <= U` or `L <= dividend` against a literal of
    /// the carrier, and the divisor's premise, `0 < divisor` or
    /// `divisor < -1`, in that order. The image is `result <= max(U, 0)` or
    /// `min(L, 0) <= result`. A strict bound, a missing or weaker divisor
    /// premise (`0 <= divisor`, `divisor < 0`), or a premise about another
    /// value rejects.
    pub(super) fn map_dividend_bound(
        form: &CheckedIntegerAffineForm,
        root_bound: &Proposition,
    ) -> Result<Proposition, IntegerAffineBoundConversionError> {
        let mismatch = IntegerAffineBoundConversionError::RootBoundMismatch;
        let [CheckedIntegerEndpointStep::RemainderWithinDividend { divisor }] =
            form.endpoint_steps.as_slice()
        else {
            return Err(mismatch);
        };
        let Proposition::Conjunction(parts) = root_bound else {
            return Err(mismatch);
        };
        let [dividend_bound, Proposition::LessThan(below, above)] = parts.as_slice() else {
            return Err(mismatch);
        };
        let integer_type = form.integer_type();
        let literal = |term: &ScalarTerm| fixed_integer_as_i128(term, integer_type);
        let positive = literal(below) == Some(0) && above == divisor;
        let below_minus_one = below == divisor && literal(above) == Some(-1);
        if !positive && !below_minus_one {
            return Err(mismatch);
        }
        let Proposition::LessOrEqual(left, right) = dividend_bound else {
            return Err(IntegerAffineBoundConversionError::RootBoundNotLessOrEqual);
        };
        let term = |endpoint: i128| {
            fixed_integer_term(integer_type, endpoint)
                .ok_or(IntegerAffineBoundConversionError::MappedBoundOutsideCarrier)
        };
        let endpoint = |term: &ScalarTerm| {
            literal(term).ok_or(IntegerAffineBoundConversionError::RootBoundNotTypedLiteral)
        };
        if left == form.root() {
            let upper = endpoint(right)?;
            Ok(Proposition::LessOrEqual(
                form.target().clone(),
                term(upper.max(0))?,
            ))
        } else if right == form.root() {
            let lower = endpoint(left)?;
            Ok(Proposition::LessOrEqual(
                term(lower.min(0))?,
                form.target().clone(),
            ))
        } else {
            Err(mismatch)
        }
    }

    fn operands(expression: &ScalarTerm) -> Option<(IntegerType, &ScalarTerm, &ScalarTerm)> {
        match expression {
            ScalarTerm::ExactIntegerRemainder {
                scalar_type,
                left,
                right,
            }
            | ScalarTerm::WrappingIntegerRemainder {
                scalar_type,
                left,
                right,
            }
            | ScalarTerm::SaturatingIntegerRemainder {
                scalar_type,
                left,
                right,
            } => Some((*scalar_type, left, right)),
            _ => None,
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::proof::assumption_proof;
        use crate::{
            ProofNode, ProofRule, accept_certificate, check_certificate,
            check_integer_affine_witness, value_context,
        };
        use semantic_vocabulary::ValueId;

        fn value(position: u64, carrier: IntegerType) -> ScalarTerm {
            ScalarTerm::value(
                ValueId::new(position).unwrap(),
                ScalarType::Integer(carrier),
            )
        }

        fn literal(number: u128, carrier: IntegerType) -> ScalarTerm {
            ScalarTerm::integer(carrier, IntegerValue::Unsigned(number)).unwrap()
        }

        fn context(carrier: IntegerType) -> PropositionContext {
            value_context(ScalarType::Integer(carrier), 4)
        }

        fn proof(witness: IntegerAffineWitness, premise: Proposition) -> ProofNode {
            ProofNode {
                conclusion: Proposition::LessThan(witness.target.clone(), witness.root.clone()),
                rule: ProofRule::IntegerAffineBound {
                    root_bound: Box::new(assumption_proof(premise, 0)),
                    witness,
                },
            }
        }

        #[test]
        fn unsigned_runtime_remainder_checks_direct_and_defined_results_at_full_width() {
            for bits in [1, 8, 32, 64, 128] {
                let carrier = IntegerType::new(IntegerSign::Unsigned, bits).unwrap();
                let context = context(carrier);
                let divisor = value(2, carrier);
                let maximum = ScalarTerm::integer(carrier, carrier.maximum_value()).unwrap();
                for dividend in [value(1, carrier), maximum] {
                    for expression in [
                        ScalarTerm::exact_integer_remainder(
                            carrier,
                            dividend.clone(),
                            divisor.clone(),
                        ),
                        ScalarTerm::wrapping_integer_remainder(
                            carrier,
                            dividend.clone(),
                            divisor.clone(),
                        ),
                        ScalarTerm::saturating_integer_remainder(
                            carrier,
                            dividend.clone(),
                            divisor.clone(),
                        ),
                    ] {
                        let expression = expression.unwrap();
                        for named in [false, true] {
                            let target = if named {
                                value(3, carrier)
                            } else {
                                expression.clone()
                            };
                            let axioms = if named {
                                vec![Proposition::Equal(target.clone(), expression.clone())]
                            } else {
                                Vec::new()
                            };
                            let witness = IntegerAffineWitness {
                                root: divisor.clone(),
                                target,
                                definition_axioms: if named { vec![0] } else { Vec::new() },
                                literal_axioms: if named { vec![None] } else { Vec::new() },
                            };
                            for premise in [
                                Proposition::LessThan(literal(0, carrier), divisor.clone()),
                                Proposition::LessOrEqual(literal(1, carrier), divisor.clone()),
                            ] {
                                let proof = proof(witness.clone(), premise.clone());
                                let accepted = accept_certificate(
                                    &context,
                                    &proof.conclusion,
                                    &[premise],
                                    &axioms,
                                    &proof,
                                )
                                .expect("exact positive divisor bounds the unsigned remainder");
                                assert_eq!(accepted.semantic_axioms.len(), usize::from(named));
                                assert_eq!(accepted.assumptions.len(), 1);
                            }
                        }
                    }
                }
            }
        }

        #[test]
        fn unsigned_runtime_remainder_rejects_missing_or_changed_positive_premises() {
            let carrier = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
            let context = context(carrier);
            let divisor = value(2, carrier);
            let witness = IntegerAffineWitness {
                root: divisor.clone(),
                target: ScalarTerm::wrapping_integer_remainder(
                    carrier,
                    value(1, carrier),
                    divisor.clone(),
                )
                .unwrap(),
                definition_axioms: Vec::new(),
                literal_axioms: Vec::new(),
            };
            let positive = Proposition::LessThan(literal(0, carrier), divisor.clone());
            let valid = proof(witness.clone(), positive.clone());
            assert!(check_certificate(&context, &valid.conclusion, &[], &[], &valid).is_err());
            for premise in [
                Proposition::Truth,
                Proposition::LessOrEqual(literal(0, carrier), divisor.clone()),
                Proposition::LessThan(divisor.clone(), literal(1, carrier)),
                Proposition::LessThan(literal(0, carrier), value(4, carrier)),
                Proposition::Equal(divisor.clone(), literal(0, carrier)),
            ] {
                let invalid = proof(witness.clone(), premise.clone());
                assert!(
                    check_certificate(&context, &invalid.conclusion, &[premise], &[], &invalid)
                        .is_err()
                );
            }
            for conclusion in [
                Proposition::LessThan(value(4, carrier), divisor),
                Proposition::LessThan(witness.target.clone(), value(4, carrier)),
                Proposition::LessThan(witness.target.clone(), literal(0, carrier)),
            ] {
                let invalid = ProofNode {
                    conclusion,
                    ..valid.clone()
                };
                assert!(
                    check_certificate(
                        &context,
                        &invalid.conclusion,
                        std::slice::from_ref(&positive),
                        &[],
                        &invalid
                    )
                    .is_err()
                );
            }
        }

        #[test]
        fn unsigned_runtime_remainder_rejects_definition_and_carrier_drift() {
            let carrier = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
            let context = context(carrier);
            let divisor = value(2, carrier);
            let target = value(3, carrier);
            let expression =
                ScalarTerm::exact_integer_remainder(carrier, value(1, carrier), divisor.clone())
                    .unwrap();
            let axioms = vec![
                Proposition::Equal(value(4, carrier), literal(9, carrier)),
                Proposition::Equal(target.clone(), expression.clone()),
            ];
            let valid = IntegerAffineWitness {
                root: divisor,
                target,
                definition_axioms: vec![1],
                literal_axioms: vec![None],
            };
            check_integer_affine_witness(&context, &axioms, &valid).unwrap();
            for (definitions, landings) in [
                (vec![], vec![]),
                (vec![0], vec![None]),
                (vec![2], vec![None]),
                (vec![1], vec![]),
                (vec![1], vec![Some(0)]),
                (vec![1, 0], vec![None, None]),
                (vec![0, 1], vec![None, None]),
                (vec![1, 1], vec![None, None]),
            ] {
                let invalid = IntegerAffineWitness {
                    definition_axioms: definitions,
                    literal_axioms: landings,
                    ..valid.clone()
                };
                assert!(check_integer_affine_witness(&context, &axioms, &invalid).is_err());
            }
            for replacement in [
                Proposition::Equal(expression, valid.target.clone()),
                Proposition::Equal(
                    valid.target.clone(),
                    ScalarTerm::exact_integer_remainder(
                        carrier,
                        value(1, carrier),
                        value(4, carrier),
                    )
                    .unwrap(),
                ),
                Proposition::Equal(
                    value(4, carrier),
                    ScalarTerm::exact_integer_remainder(
                        carrier,
                        value(1, carrier),
                        valid.root.clone(),
                    )
                    .unwrap(),
                ),
            ] {
                let changed = [axioms[0].clone(), replacement];
                assert!(check_integer_affine_witness(&context, &changed, &valid).is_err());
            }
            for other_carrier in [
                IntegerType::new(IntegerSign::Signed, 64).unwrap(),
                IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
            ] {
                let invalid = IntegerAffineWitness {
                    root: value(2, other_carrier),
                    ..valid.clone()
                };
                assert!(check_integer_affine_witness(&context, &axioms, &invalid).is_err());
            }
        }

        #[test]
        fn unsigned_runtime_remainder_does_not_admit_signed_or_undeclared_operands() {
            let carrier = IntegerType::new(IntegerSign::Signed, 32).unwrap();
            let witness = IntegerAffineWitness {
                root: value(2, carrier),
                target: ScalarTerm::wrapping_integer_remainder(
                    carrier,
                    value(1, carrier),
                    value(2, carrier),
                )
                .unwrap(),
                definition_axioms: Vec::new(),
                literal_axioms: Vec::new(),
            };
            assert!(check_integer_affine_witness(&context(carrier), &[], &witness).is_err());
            let carrier = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
            let witness = IntegerAffineWitness {
                root: value(2, carrier),
                target: ScalarTerm::exact_integer_remainder(
                    carrier,
                    value(99, carrier),
                    value(2, carrier),
                )
                .unwrap(),
                ..witness
            };
            assert!(check_integer_affine_witness(&context(carrier), &[], &witness).is_err());
        }
    }
}
mod truth_bounds {
    use crate::integer_rules::fixed_integer::{fixed_integer_term, integer_value_as_i128};
    use crate::integer_rules::integer_affine::witness_checking::CheckedIntegerEndpointStep;
    use crate::integer_rules::integer_affine::{
        CheckedIntegerAffineForm, IntegerAffineBoundConversionError,
    };
    use semantic_vocabulary::{IntegerSign, Proposition};

    pub fn integer_affine_truth_bounds(
        form: &CheckedIntegerAffineForm,
    ) -> Result<Vec<Proposition>, IntegerAffineBoundConversionError> {
        let mut minimum = integer_value_as_i128(form.integer_type().minimum_value())
            .ok_or(IntegerAffineBoundConversionError::MappedBoundOutsideCarrier)?;
        let mut maximum = integer_value_as_i128(form.integer_type().maximum_value())
            .ok_or(IntegerAffineBoundConversionError::MappedBoundOutsideCarrier)?;
        let carrier_minimum = minimum;
        let carrier_maximum = maximum;
        let mut saw_total_image = false;
        for step in &form.endpoint_steps {
            match step {
                CheckedIntegerEndpointStep::Add(value) => {
                    minimum = minimum
                        .checked_add(*value)
                        .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?;
                    maximum = maximum
                        .checked_add(*value)
                        .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?;
                }
                CheckedIntegerEndpointStep::SaturatingAdd(value) => {
                    minimum = minimum
                        .checked_add(*value)
                        .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?
                        .clamp(carrier_minimum, carrier_maximum);
                    maximum = maximum
                        .checked_add(*value)
                        .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?
                        .clamp(carrier_minimum, carrier_maximum);
                    saw_total_image = true;
                }
                CheckedIntegerEndpointStep::Subtract(value) => {
                    minimum = minimum
                        .checked_sub(*value)
                        .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?;
                    maximum = maximum
                        .checked_sub(*value)
                        .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?;
                }
                CheckedIntegerEndpointStep::Multiply(value) => {
                    let left = minimum
                        .checked_mul(*value)
                        .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?;
                    let right = maximum
                        .checked_mul(*value)
                        .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?;
                    (minimum, maximum) = (left.min(right), left.max(right));
                    if *value == 0 {
                        saw_total_image = true;
                    }
                }
                CheckedIntegerEndpointStep::Divide(value) => {
                    if form.integer_type().sign() == IntegerSign::Signed && *value == -1 {
                        return Err(IntegerAffineBoundConversionError::NonTotalDivisionImage);
                    }
                    let left = carrier_minimum
                        .checked_div(*value)
                        .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?;
                    let right = carrier_maximum
                        .checked_div(*value)
                        .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?;
                    (minimum, maximum) = (left.min(right), left.max(right));
                    saw_total_image = true;
                }
                CheckedIntegerEndpointStep::Remainder(value) => {
                    let magnitude = value
                        .checked_abs()
                        .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?;
                    minimum = if form.integer_type().sign() == IntegerSign::Signed {
                        1_i128
                            .checked_sub(magnitude)
                            .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?
                    } else {
                        0
                    };
                    maximum = magnitude
                        .checked_sub(1)
                        .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?;
                    saw_total_image = true;
                }
                CheckedIntegerEndpointStep::ShiftLeft(count) => {
                    if *count == 0 {
                        minimum = carrier_minimum;
                        maximum = carrier_maximum;
                        saw_total_image = true;
                        continue;
                    }
                    let scale = 1_i128
                        .checked_shl(*count)
                        .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?;
                    minimum = minimum
                        .checked_mul(scale)
                        .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?;
                    maximum = maximum
                        .checked_mul(scale)
                        .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?;
                }
                CheckedIntegerEndpointStep::ShiftRight(count) => {
                    minimum = carrier_minimum >> count;
                    maximum = carrier_maximum >> count;
                    saw_total_image = true;
                }
                CheckedIntegerEndpointStep::BitwiseAndMask(mask) => {
                    minimum = 0;
                    maximum = *mask;
                    saw_total_image = true;
                }
                CheckedIntegerEndpointStep::BitwiseAndBoundedMask { upper, .. } => {
                    minimum = 0;
                    maximum = *upper;
                    saw_total_image = true;
                }
                CheckedIntegerEndpointStep::BitwiseOrLiteralImage(_)
                | CheckedIntegerEndpointStep::BitwiseOrBoundedImage { .. } => {
                    return Err(IntegerAffineBoundConversionError::TruthRootWithoutTotalImage);
                }
                CheckedIntegerEndpointStep::WrappingAdd { .. }
                | CheckedIntegerEndpointStep::WrappingAddBackward { .. }
                | CheckedIntegerEndpointStep::CorrelatedAddLower
                | CheckedIntegerEndpointStep::CorrelatedAddUpper
                | CheckedIntegerEndpointStep::CorrelatedSubtractLower
                | CheckedIntegerEndpointStep::CorrelatedSubtractUpper
                | CheckedIntegerEndpointStep::CorrelatedUnsignedSubtract
                | CheckedIntegerEndpointStep::CorrelatedUnsignedRemainder
                | CheckedIntegerEndpointStep::RemainderWithinDividend { .. }
                | CheckedIntegerEndpointStep::CorrelatedMultiplyMinimum
                | CheckedIntegerEndpointStep::CorrelatedMultiplyMaximum => {
                    return Err(IntegerAffineBoundConversionError::TruthRootWithoutTotalImage);
                }
            }
        }
        if !saw_total_image {
            return Err(IntegerAffineBoundConversionError::TruthRootWithoutTotalImage);
        }
        let minimum = fixed_integer_term(form.integer_type(), minimum)
            .ok_or(IntegerAffineBoundConversionError::MappedBoundOutsideCarrier)?;
        let maximum = fixed_integer_term(form.integer_type(), maximum)
            .ok_or(IntegerAffineBoundConversionError::MappedBoundOutsideCarrier)?;
        Ok(vec![
            Proposition::LessOrEqual(minimum, form.target().clone()),
            Proposition::LessOrEqual(form.target().clone(), maximum),
        ])
    }
}
mod witness_checking {
    use crate::integer_rules::fixed_integer::{fixed_integer_as_i128, integer_value_as_i128};
    use crate::integer_rules::integer_affine::bound_mapping::direct_math_leaf;
    use crate::term_matchers::*;
    use semantic_vocabulary::{
        IntegerCarrier, IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext,
        ScalarTerm, ScalarType,
    };
    use terminal_psi::IntegerAffineWitness;

    mod correlated {
        use super::*;

        fn check_correlated_witness(
            context: &PropositionContext,
            semantic_axioms: &[Proposition],
            witness: &IntegerAffineWitness,
            outer: TermMatcher,
            inner: TermMatcher,
            minimum_step: CheckedIntegerEndpointStep,
            maximum_step: CheckedIntegerEndpointStep,
        ) -> Result<Option<CheckedIntegerAffineForm>, IntegerAffineWitnessError> {
            let Some((scalar_type, left, right)) = outer(&witness.target) else {
                return Ok(None);
            };
            if scalar_type.carrier() != IntegerCarrier::Fixed
                || direct_math_leaf(left, *scalar_type).is_none()
                || direct_math_leaf(right, *scalar_type).is_none()
                || witness.root.scalar_type() != ScalarType::Integer(*scalar_type)
            {
                return Ok(None);
            }
            let (expression, landing) = match (
                witness.definition_axioms.as_slice(),
                witness.literal_axioms.as_slice(),
            ) {
                ([], []) => (&witness.root, None),
                ([index], [landing]) if matches!(witness.root, ScalarTerm::Value { .. }) => {
                    let axiom = checked_axiom(context, semantic_axioms, *index)?;
                    let Proposition::Equal(equal_left, equal_right) = axiom else {
                        return Ok(None);
                    };
                    let expression = if equal_left == &witness.root {
                        equal_right
                    } else if equal_right == &witness.root {
                        equal_left
                    } else {
                        return Ok(None);
                    };
                    (expression, Some((*index, *landing)))
                }
                _ => return Ok(None),
            };
            let Some((inner_type, endpoint, inner_right)) = inner(expression) else {
                return Ok(None);
            };
            if inner_type != scalar_type || inner_right != right {
                return Ok(None);
            }
            let endpoint_value = match (endpoint.integer_value(), landing) {
                (Some((actual, value)), None | Some((_, None))) => {
                    (actual == *scalar_type).then_some(value)
                }
                (None, Some((index, Some(landing_index)))) => {
                    let landed = landed_integer(context, semantic_axioms, index, landing_index)?;
                    (landed.term == *endpoint && landed.integer_type == *scalar_type)
                        .then_some(landed.value)
                }
                _ => return Ok(None),
            };
            let endpoint_step = match endpoint_value {
                Some(value) if value == scalar_type.minimum_value() => minimum_step,
                Some(value) if value == scalar_type.maximum_value() => maximum_step,
                _ => return Ok(None),
            };
            Ok(Some(CheckedIntegerAffineForm {
                root: witness.root.clone(),
                target: witness.target.clone(),
                integer_type: *scalar_type,
                coefficient: 1,
                offset: 0,
                endpoint_steps: vec![endpoint_step],
            }))
        }

        pub(super) fn check_correlated_add_witness(
            context: &PropositionContext,
            semantic_axioms: &[Proposition],
            witness: &IntegerAffineWitness,
        ) -> Result<Option<CheckedIntegerAffineForm>, IntegerAffineWitnessError> {
            check_correlated_witness(
                context,
                semantic_axioms,
                witness,
                exact_integer_add,
                exact_integer_subtract,
                CheckedIntegerEndpointStep::CorrelatedAddLower,
                CheckedIntegerEndpointStep::CorrelatedAddUpper,
            )
        }

        pub(super) fn check_correlated_subtract_witness(
            context: &PropositionContext,
            semantic_axioms: &[Proposition],
            witness: &IntegerAffineWitness,
        ) -> Result<Option<CheckedIntegerAffineForm>, IntegerAffineWitnessError> {
            check_correlated_witness(
                context,
                semantic_axioms,
                witness,
                exact_integer_subtract,
                exact_integer_add,
                CheckedIntegerEndpointStep::CorrelatedSubtractLower,
                CheckedIntegerEndpointStep::CorrelatedSubtractUpper,
            )
        }

        pub(super) fn check_correlated_multiply_witness(
            context: &PropositionContext,
            semantic_axioms: &[Proposition],
            witness: &IntegerAffineWitness,
        ) -> Result<Option<CheckedIntegerAffineForm>, IntegerAffineWitnessError> {
            check_correlated_witness(
                context,
                semantic_axioms,
                witness,
                exact_integer_multiply,
                exact_integer_divide,
                CheckedIntegerEndpointStep::CorrelatedMultiplyMinimum,
                CheckedIntegerEndpointStep::CorrelatedMultiplyMaximum,
            )
        }
    }
    mod definitions {
        use super::*;

        pub(super) fn apply_definition(
            target: &ScalarTerm,
            expression: &ScalarTerm,
            current: &ScalarTerm,
            integer_type: IntegerType,
            coefficient: i128,
            offset: i128,
            landed: Option<&LandedInteger>,
            prior_axioms: &[Proposition],
        ) -> Option<
            Result<
                (ScalarTerm, i128, i128, bool, CheckedIntegerEndpointStep),
                IntegerAffineWitnessError,
            >,
        > {
            if !matches!(target, ScalarTerm::Value { .. })
                || target.scalar_type() != ScalarType::Integer(integer_type)
            {
                return None;
            }
            let transformed = match expression {
                ScalarTerm::ExactIntegerAdd {
                    scalar_type,
                    left,
                    right,
                } if *scalar_type == integer_type
                    && (left.as_ref() == current || right.as_ref() == current) =>
                {
                    let operand = if left.as_ref() == current {
                        right
                    } else {
                        left
                    };
                    let (literal, used_landing) = signed_literal(operand, integer_type, landed)?;
                    (
                        Some(coefficient),
                        offset.checked_add(literal),
                        used_landing,
                        CheckedIntegerEndpointStep::Add(literal),
                    )
                }
                ScalarTerm::ExactIntegerSubtract {
                    scalar_type,
                    left,
                    right,
                } if *scalar_type == integer_type && left.as_ref() == current => {
                    let (literal, used_landing) = signed_literal(right, integer_type, landed)?;
                    (
                        Some(coefficient),
                        offset.checked_sub(literal),
                        used_landing,
                        CheckedIntegerEndpointStep::Subtract(literal),
                    )
                }
                ScalarTerm::ExactIntegerMultiply {
                    scalar_type,
                    left,
                    right,
                } if *scalar_type == integer_type
                    && (left.as_ref() == current || right.as_ref() == current) =>
                {
                    let sibling = if left.as_ref() == current {
                        right
                    } else {
                        left
                    };
                    let (literal, used_landing) = signed_literal(sibling, integer_type, landed)?;
                    (
                        coefficient.checked_mul(literal),
                        offset.checked_mul(literal),
                        used_landing,
                        CheckedIntegerEndpointStep::Multiply(literal),
                    )
                }
                ScalarTerm::ExactIntegerDivide {
                    scalar_type,
                    left,
                    right,
                }
                | ScalarTerm::WrappingIntegerDivide {
                    scalar_type,
                    left,
                    right,
                }
                | ScalarTerm::ExactIntegerRemainder {
                    scalar_type,
                    left,
                    right,
                }
                | ScalarTerm::WrappingIntegerRemainder {
                    scalar_type,
                    left,
                    right,
                } if *scalar_type == integer_type && left.as_ref() == current => {
                    let (literal, used_landing) = signed_literal(right, integer_type, landed)?;
                    if literal == 0 {
                        return Some(Err(IntegerAffineWitnessError::ZeroDivisionLiteral));
                    }
                    let step = match expression {
                        ScalarTerm::ExactIntegerDivide { .. } => {
                            CheckedIntegerEndpointStep::Divide(literal)
                        }
                        ScalarTerm::WrappingIntegerDivide { .. } => {
                            if integer_type.sign() == IntegerSign::Signed && literal == -1 {
                                return None;
                            }
                            CheckedIntegerEndpointStep::Divide(literal)
                        }
                        _ => CheckedIntegerEndpointStep::Remainder(literal),
                    };
                    (Some(coefficient), Some(offset), used_landing, step)
                }
                ScalarTerm::IntegerBitwiseAnd {
                    scalar_type,
                    left,
                    right,
                } if *scalar_type == integer_type
                    && (left.as_ref() == current || right.as_ref() == current) =>
                {
                    let sibling = if left.as_ref() == current {
                        right
                    } else {
                        left
                    };
                    if let Some((mask, used_landing)) =
                        signed_literal(sibling, integer_type, landed)
                    {
                        if mask < 0 {
                            return Some(Err(IntegerAffineWitnessError::NegativeBitwiseAndMask));
                        }
                        (
                            Some(coefficient),
                            Some(offset),
                            used_landing,
                            CheckedIntegerEndpointStep::BitwiseAndMask(mask),
                        )
                    } else {
                        let (mask, upper) =
                            bounded_bitwise_sibling(sibling, integer_type, prior_axioms)?;
                        (
                            Some(coefficient),
                            Some(offset),
                            false,
                            CheckedIntegerEndpointStep::BitwiseAndBoundedMask {
                                mask: mask.clone(),
                                upper,
                            },
                        )
                    }
                }
                ScalarTerm::IntegerBitwiseOr {
                    scalar_type,
                    left,
                    right,
                } if *scalar_type == integer_type
                    && (left.as_ref() == current || right.as_ref() == current) =>
                {
                    let sibling_operand = if left.as_ref() == current {
                        right
                    } else {
                        left
                    };
                    if let Some((sibling, used_landing)) =
                        signed_literal(sibling_operand, integer_type, landed)
                    {
                        if sibling < 0 {
                            return Some(Err(IntegerAffineWitnessError::NegativeBitwiseOrSibling));
                        }
                        (
                            Some(coefficient),
                            Some(offset),
                            used_landing,
                            CheckedIntegerEndpointStep::BitwiseOrLiteralImage(sibling),
                        )
                    } else {
                        let (operand, upper) =
                            bounded_bitwise_sibling(sibling_operand, integer_type, prior_axioms)?;
                        (
                            Some(coefficient),
                            Some(offset),
                            false,
                            CheckedIntegerEndpointStep::BitwiseOrBoundedImage {
                                operand: operand.clone(),
                                upper,
                            },
                        )
                    }
                }
                ScalarTerm::WrappingIntegerAdd {
                    scalar_type,
                    left,
                    right,
                }
                | ScalarTerm::SaturatingIntegerAdd {
                    scalar_type,
                    left,
                    right,
                } if *scalar_type == integer_type
                    && (left.as_ref() == current || right.as_ref() == current) =>
                {
                    let (sibling, current_operand) = if left.as_ref() == current {
                        (right, left)
                    } else {
                        (left, right)
                    };
                    let (literal, used_landing) = signed_literal(sibling, integer_type, landed)?;
                    (
                        Some(coefficient),
                        offset.checked_add(literal),
                        used_landing,
                        policy_add_step(expression, current_operand, literal),
                    )
                }
                ScalarTerm::WrappingIntegerSubtract {
                    scalar_type,
                    left,
                    right,
                }
                | ScalarTerm::SaturatingIntegerSubtract {
                    scalar_type,
                    left,
                    right,
                } if *scalar_type == integer_type && left.as_ref() == current => {
                    let (literal, used_landing) = signed_literal(right, integer_type, landed)?;
                    (
                        Some(coefficient),
                        offset.checked_sub(literal),
                        used_landing,
                        policy_add_step(expression, left, literal.checked_neg()?),
                    )
                }
                _ => return None,
            };
            Some(match transformed {
                (Some(coefficient), Some(offset), used_landing, endpoint_step) => Ok((
                    target.clone(),
                    coefficient,
                    offset,
                    used_landing,
                    endpoint_step,
                )),
                _ => Err(IntegerAffineWitnessError::CoefficientOverflow),
            })
        }

        fn bounded_bitwise_sibling<'a>(
            operand: &'a ScalarTerm,
            integer_type: IntegerType,
            prior_axioms: &'a [Proposition],
        ) -> Option<(&'a ScalarTerm, i128)> {
            let mut masks = vec![operand];
            for proposition in prior_axioms {
                let Proposition::Equal(left, right) = proposition else {
                    continue;
                };
                if left == operand {
                    masks.push(right);
                } else if right == operand {
                    masks.push(left);
                }
            }
            for mask in masks {
                let Some(upper) = prior_axioms.iter().find_map(|proposition| {
                    let Proposition::LessOrEqual(left, right) = proposition else {
                        return None;
                    };
                    if left != mask {
                        return None;
                    }
                    let upper =
                        fixed_integer_as_i128(right, integer_type).filter(|upper| *upper >= 0)?;
                    Some(upper)
                }) else {
                    continue;
                };
                let nonnegative = matches!(
                    mask.scalar_type(),
                    ScalarType::Integer(kind) if kind.sign() == IntegerSign::Unsigned
                ) || prior_axioms.iter().any(|proposition| {
                    let Proposition::LessOrEqual(left, right) = proposition else {
                        return false;
                    };
                    right == mask && fixed_integer_as_i128(left, integer_type) == Some(0)
                });
                if nonnegative {
                    return Some((mask, upper));
                }
            }
            None
        }

        pub(super) fn apply_shift_definition(
            target: &ScalarTerm,
            expression: &ScalarTerm,
            current: &ScalarTerm,
            integer_type: IntegerType,
            landed: Option<&LandedInteger>,
            definition_index: usize,
        ) -> Option<
            Result<
                (ScalarTerm, i128, i128, bool, CheckedIntegerEndpointStep),
                IntegerAffineWitnessError,
            >,
        > {
            if !matches!(target, ScalarTerm::Value { .. })
                || target.scalar_type() != ScalarType::Integer(integer_type)
                || !matches!(integer_type.bits(), 8 | 16 | 32 | 64)
            {
                return None;
            }
            let (count_type, count, left) = match expression {
                ScalarTerm::ExactIntegerShiftLeft {
                    value_type,
                    count_type,
                    value,
                    count,
                } if *value_type == integer_type && value.as_ref() == current => {
                    (*count_type, count.as_ref(), true)
                }
                ScalarTerm::ExactIntegerShiftRight {
                    value_type,
                    count_type,
                    value,
                    count,
                } if *value_type == integer_type && value.as_ref() == current => {
                    (*count_type, count.as_ref(), false)
                }
                _ => return None,
            };
            let Some((count, used_landing)) = nonnegative_count(count, count_type, landed) else {
                return Some(Err(IntegerAffineWitnessError::ShiftCountNotLanded(
                    definition_index,
                )));
            };
            let Ok(count) = u32::try_from(count) else {
                return Some(Err(IntegerAffineWitnessError::ShiftCountOutsideValueWidth(
                    definition_index,
                )));
            };
            if count >= u32::from(integer_type.bits()) {
                return Some(Err(IntegerAffineWitnessError::ShiftCountOutsideValueWidth(
                    definition_index,
                )));
            }
            let step = if left {
                CheckedIntegerEndpointStep::ShiftLeft(count)
            } else {
                CheckedIntegerEndpointStep::ShiftRight(count)
            };
            Some(Ok((target.clone(), 1, 0, used_landing, step)))
        }

        fn nonnegative_count(
            term: &ScalarTerm,
            count_type: IntegerType,
            landed: Option<&LandedInteger>,
        ) -> Option<(u128, bool)> {
            let (actual_type, value, used_landing) =
                if let Some((actual_type, value)) = term.integer_value() {
                    (actual_type, value, false)
                } else {
                    let landed = landed.filter(|landed| landed.term == *term)?;
                    (landed.integer_type, landed.value, true)
                };
            if actual_type != count_type || actual_type.carrier() != IntegerCarrier::Fixed {
                return None;
            }
            match value {
                IntegerValue::Signed(value) => u128::try_from(value)
                    .ok()
                    .map(|value| (value, used_landing)),
                IntegerValue::Unsigned(value) => Some((value, used_landing)),
            }
        }

        #[allow(clippy::too_many_arguments)]
        fn apply_add_inverse(
            defined: &ScalarTerm,
            expression: &ScalarTerm,
            current: &ScalarTerm,
            integer_type: IntegerType,
            coefficient: i128,
            offset: i128,
            landed: Option<&LandedInteger>,
            definition_index: usize,
            matcher: TermMatcher,
            step: fn(&ScalarTerm, i128) -> CheckedIntegerEndpointStep,
        ) -> Option<
            Result<
                (ScalarTerm, i128, i128, bool, CheckedIntegerEndpointStep),
                IntegerAffineWitnessError,
            >,
        > {
            if !matches!(defined, ScalarTerm::Value { .. }) || defined != current {
                return None;
            }
            let (scalar_type, left, right) = matcher(expression)?;
            if *scalar_type != integer_type {
                return None;
            }
            let left_operand = if matches!(left, ScalarTerm::Value { .. }) {
                signed_literal(right, integer_type, landed)
            } else {
                None
            };
            let right_operand = if matches!(right, ScalarTerm::Value { .. }) {
                signed_literal(left, integer_type, landed)
            } else {
                None
            };
            let (operand, literal, used_landing) = match (left_operand, right_operand) {
                (Some((literal, used)), None) => (left, literal, used),
                (None, Some((literal, used))) => (right, literal, used),
                (None, None) => return None,
                (Some(_), Some(_)) => {
                    return Some(Err(IntegerAffineWitnessError::AmbiguousDefinition(
                        definition_index,
                    )));
                }
            };
            Some(match offset.checked_sub(literal) {
                Some(offset) => Ok((
                    operand.clone(),
                    coefficient,
                    offset,
                    used_landing,
                    step(operand, literal),
                )),
                None => Err(IntegerAffineWitnessError::CoefficientOverflow),
            })
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn apply_wrapping_add_inverse(
            defined: &ScalarTerm,
            expression: &ScalarTerm,
            current: &ScalarTerm,
            integer_type: IntegerType,
            coefficient: i128,
            offset: i128,
            landed: Option<&LandedInteger>,
            definition_index: usize,
        ) -> Option<
            Result<
                (ScalarTerm, i128, i128, bool, CheckedIntegerEndpointStep),
                IntegerAffineWitnessError,
            >,
        > {
            apply_add_inverse(
                defined,
                expression,
                current,
                integer_type,
                coefficient,
                offset,
                landed,
                definition_index,
                wrapping_integer_add,
                |operand, literal| CheckedIntegerEndpointStep::WrappingAddBackward {
                    operand: operand.clone(),
                    literal,
                },
            )
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn apply_exact_add_inverse(
            defined: &ScalarTerm,
            expression: &ScalarTerm,
            current: &ScalarTerm,
            integer_type: IntegerType,
            coefficient: i128,
            offset: i128,
            landed: Option<&LandedInteger>,
            definition_index: usize,
        ) -> Option<
            Result<
                (ScalarTerm, i128, i128, bool, CheckedIntegerEndpointStep),
                IntegerAffineWitnessError,
            >,
        > {
            apply_add_inverse(
                defined,
                expression,
                current,
                integer_type,
                coefficient,
                offset,
                landed,
                definition_index,
                exact_integer_add,
                |_, literal| CheckedIntegerEndpointStep::Subtract(literal),
            )
        }

        pub(super) fn both_orientations<T>(
            left: &ScalarTerm,
            right: &ScalarTerm,
            read: impl Fn(&ScalarTerm, &ScalarTerm) -> T,
        ) -> [T; 2] {
            [read(left, right), read(right, left)]
        }
    }
    use correlated::*;
    use definitions::*;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct CheckedIntegerAffineForm {
        pub(crate) root: ScalarTerm,
        pub(crate) target: ScalarTerm,
        pub(crate) integer_type: IntegerType,
        pub(crate) coefficient: i128,
        pub(crate) offset: i128,
        pub(crate) endpoint_steps: Vec<CheckedIntegerEndpointStep>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) enum CheckedIntegerEndpointStep {
        Add(i128),
        Subtract(i128),
        Multiply(i128),
        Divide(i128),
        Remainder(i128),
        ShiftLeft(u32),
        ShiftRight(u32),
        SaturatingAdd(i128),
        WrappingAdd {
            operand: ScalarTerm,
            literal: i128,
        },
        WrappingAddBackward {
            operand: ScalarTerm,
            literal: i128,
        },
        BitwiseAndMask(i128),
        BitwiseAndBoundedMask {
            mask: ScalarTerm,
            upper: i128,
        },
        BitwiseOrLiteralImage(i128),
        BitwiseOrBoundedImage {
            operand: ScalarTerm,
            upper: i128,
        },
        CorrelatedAddLower,
        CorrelatedAddUpper,
        CorrelatedSubtractLower,
        CorrelatedSubtractUpper,
        CorrelatedUnsignedSubtract,
        CorrelatedUnsignedRemainder,
        /// An exact remainder by `divisor`, a value no literal fixes, rooted
        /// at its dividend (`remainder::check_dividend_witness`).
        RemainderWithinDividend {
            divisor: ScalarTerm,
        },
        CorrelatedMultiplyMinimum,
        CorrelatedMultiplyMaximum,
    }

    impl CheckedIntegerEndpointStep {
        pub(crate) fn preserves_strict_endpoint(&self) -> bool {
            matches!(
                self,
                Self::Add(_)
                    | Self::Subtract(_)
                    | Self::WrappingAdd { .. }
                    | Self::WrappingAddBackward { .. }
            )
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct LandedInteger {
        term: ScalarTerm,
        integer_type: IntegerType,
        value: IntegerValue,
    }

    impl CheckedIntegerAffineForm {
        pub const fn integer_type(&self) -> IntegerType {
            self.integer_type
        }

        pub const fn coefficient(&self) -> i128 {
            self.coefficient
        }

        pub const fn offset(&self) -> i128 {
            self.offset
        }

        pub const fn root(&self) -> &ScalarTerm {
            &self.root
        }

        pub const fn target(&self) -> &ScalarTerm {
            &self.target
        }

        /// Whether this is the provisional remainder-within-dividend form,
        /// which an acceptance records under its own rule row.
        pub(crate) fn is_remainder_within_dividend(&self) -> bool {
            matches!(
                self.endpoint_steps.as_slice(),
                [CheckedIntegerEndpointStep::RemainderWithinDividend { .. }]
            )
        }
    }

    fn identity_ring_target(target: &ScalarTerm, root: &ScalarTerm) -> Option<IntegerType> {
        let (integer_type, left, right) = match target {
            ScalarTerm::ExactIntegerMultiply {
                scalar_type,
                left,
                right,
            }
            | ScalarTerm::WrappingIntegerMultiply {
                scalar_type,
                left,
                right,
            } if left.as_ref() == root || right.as_ref() == root => (*scalar_type, left, right),
            ScalarTerm::ExactIntegerAdd {
                scalar_type,
                left,
                right,
            }
            | ScalarTerm::ExactIntegerSubtract {
                scalar_type,
                left,
                right,
            } if left.as_ref() == root => (*scalar_type, left, right),
            _ => return None,
        };
        (integer_type.carrier() == IntegerCarrier::Fixed
            && direct_math_leaf(left, integer_type).is_some()
            && direct_math_leaf(right, integer_type).is_some())
        .then_some(integer_type)
    }

    fn trivial_affine_form(
        witness: &IntegerAffineWitness,
        integer_type: IntegerType,
    ) -> CheckedIntegerAffineForm {
        CheckedIntegerAffineForm {
            root: witness.root.clone(),
            target: witness.target.clone(),
            integer_type,
            coefficient: 1,
            offset: 0,
            endpoint_steps: Vec::new(),
        }
    }

    pub fn check_integer_affine_witness(
        context: &PropositionContext,
        semantic_axioms: &[Proposition],
        witness: &IntegerAffineWitness,
    ) -> Result<CheckedIntegerAffineForm, IntegerAffineWitnessError> {
        if let Some(form) = super::remainder::check_witness(context, semantic_axioms, witness)? {
            return Ok(form);
        }
        if let Some(form) =
            super::remainder::check_dividend_witness(context, semantic_axioms, witness)?
        {
            return Ok(form);
        }
        if let Some(form) = check_correlated_add_witness(context, semantic_axioms, witness)? {
            return Ok(form);
        }
        if let Some(form) = check_correlated_subtract_witness(context, semantic_axioms, witness)? {
            return Ok(form);
        }
        if let Some(form) = check_correlated_multiply_witness(context, semantic_axioms, witness)? {
            return Ok(form);
        }
        if witness.definition_axioms.is_empty()
            && witness.literal_axioms.is_empty()
            && let ScalarTerm::ExactIntegerRemainder {
                scalar_type,
                left,
                right,
            } = &witness.target
            && scalar_type.carrier() == IntegerCarrier::Fixed
            && matches!(witness.root, ScalarTerm::Value { .. })
            && left.as_ref() == &witness.root
            && matches!(right.as_ref(), ScalarTerm::Integer { .. })
            && let Some((divisor, _)) = signed_literal(right, *scalar_type, None)
            && (scalar_type.sign() == IntegerSign::Unsigned || divisor != -1)
        {
            context
                .validate(&Proposition::Equal(
                    witness.root.clone(),
                    witness.target.clone(),
                ))
                .map_err(IntegerAffineWitnessError::MalformedProposition)?;
            if divisor == 0 {
                return Err(IntegerAffineWitnessError::ZeroDivisionLiteral);
            }
            return Ok(CheckedIntegerAffineForm {
                root: witness.root.clone(),
                target: witness.target.clone(),
                integer_type: *scalar_type,
                coefficient: 1,
                offset: 0,
                endpoint_steps: vec![CheckedIntegerEndpointStep::Remainder(divisor)],
            });
        }
        if witness.definition_axioms.is_empty()
            && witness.literal_axioms.is_empty()
            && let ScalarTerm::ExactIntegerSubtract {
                scalar_type,
                left,
                right,
            } = &witness.target
            && scalar_type.carrier() == IntegerCarrier::Fixed
            && scalar_type.sign() == IntegerSign::Unsigned
            && right.as_ref() == &witness.root
            && left.as_ref() != right.as_ref()
            && direct_math_leaf(left, *scalar_type).is_some()
            && direct_math_leaf(right, *scalar_type).is_some()
        {
            return Ok(CheckedIntegerAffineForm {
                root: witness.root.clone(),
                target: witness.target.clone(),
                integer_type: *scalar_type,
                coefficient: 1,
                offset: 0,
                endpoint_steps: vec![CheckedIntegerEndpointStep::CorrelatedUnsignedSubtract],
            });
        }
        if witness.definition_axioms.is_empty()
            && witness.literal_axioms.is_empty()
            && let Some(integer_type) = identity_ring_target(&witness.target, &witness.root)
        {
            return Ok(trivial_affine_form(witness, integer_type));
        }
        if witness.definition_axioms.is_empty()
            && witness.literal_axioms.is_empty()
            && let ScalarTerm::ExactIntegerShiftLeft {
                value_type,
                count_type,
                value,
                count: _,
            } = &witness.target
            && value.as_ref() == &witness.root
            && value_type.carrier() == IntegerCarrier::Fixed
            && count_type.carrier() == IntegerCarrier::Fixed
        {
            return Ok(trivial_affine_form(witness, *value_type));
        }
        if !matches!(witness.root, ScalarTerm::Value { .. }) {
            return Err(IntegerAffineWitnessError::RootNotValue);
        }
        let Some(integer_type) = witness.root.scalar_type().as_integer() else {
            return Err(IntegerAffineWitnessError::RootNotInteger);
        };
        if integer_type.carrier() != IntegerCarrier::Fixed {
            return Err(IntegerAffineWitnessError::UnsupportedCarrier(integer_type));
        }
        if witness.target.scalar_type() != ScalarType::Integer(integer_type) {
            return Err(IntegerAffineWitnessError::TargetTypeMismatch);
        }
        if witness.definition_axioms.is_empty()
            && apply_policy_expression_target(&witness.target, &witness.root, integer_type, 1, 0)
                .is_none()
        {
            return Err(IntegerAffineWitnessError::EmptyDefinitionChain);
        }
        if witness.literal_axioms.len() != witness.definition_axioms.len() {
            return Err(IntegerAffineWitnessError::LiteralAxiomCountMismatch);
        }
        if witness
            .definition_axioms
            .windows(2)
            .any(|indices| indices[0] >= indices[1])
        {
            return Err(IntegerAffineWitnessError::NonCanonicalDefinitionOrder);
        }

        let mut current = witness.root.clone();
        let mut coefficient = 1_i128;
        let mut offset = 0_i128;
        let mut endpoint_steps = Vec::with_capacity(witness.definition_axioms.len());
        for (&index, &literal_index) in witness
            .definition_axioms
            .iter()
            .zip(&witness.literal_axioms)
        {
            let proposition = checked_axiom(context, semantic_axioms, index)?;
            let Proposition::Equal(left, right) = proposition else {
                return Err(IntegerAffineWitnessError::DefinitionNotEquality(index));
            };
            let landed = literal_index
                .map(|literal_index| landed_integer(context, semantic_axioms, index, literal_index))
                .transpose()?;
            let candidates = [
                both_orientations(left, right, |first, second| {
                    apply_definition(
                        first,
                        second,
                        &current,
                        integer_type,
                        coefficient,
                        offset,
                        landed.as_ref(),
                        &semantic_axioms[..index],
                    )
                }),
                both_orientations(left, right, |first, second| {
                    apply_shift_definition(
                        first,
                        second,
                        &current,
                        integer_type,
                        landed.as_ref(),
                        index,
                    )
                }),
                both_orientations(left, right, |first, second| {
                    apply_wrapping_add_inverse(
                        first,
                        second,
                        &current,
                        integer_type,
                        coefficient,
                        offset,
                        landed.as_ref(),
                        index,
                    )
                }),
                both_orientations(left, right, |first, second| {
                    apply_exact_add_inverse(
                        first,
                        second,
                        &current,
                        integer_type,
                        coefficient,
                        offset,
                        landed.as_ref(),
                        index,
                    )
                }),
            ]
            .into_iter()
            .flat_map(|orientations| orientations.into_iter().flatten())
            .collect::<Vec<_>>();
            let (next, next_coefficient, next_offset, used_landing, endpoint_step) =
                match candidates.as_slice() {
                    [next] => next.clone()?,
                    [] => return Err(IntegerAffineWitnessError::DefinitionShapeMismatch(index)),
                    _ => {
                        return Err(IntegerAffineWitnessError::AmbiguousDefinition(index));
                    }
                };
            if landed.is_some() != used_landing {
                return Err(IntegerAffineWitnessError::UnusedLiteralAxiom(index));
            }
            current = next;
            coefficient = next_coefficient;
            offset = next_offset;
            endpoint_steps.push(endpoint_step);
        }
        if current != witness.target {
            let (next_coefficient, next_offset, endpoint_step) = apply_policy_expression_target(
                &witness.target,
                &current,
                integer_type,
                coefficient,
                offset,
            )
            .ok_or(IntegerAffineWitnessError::TargetMismatch)??;
            coefficient = next_coefficient;
            offset = next_offset;
            endpoint_steps.push(endpoint_step);
        }
        Ok(CheckedIntegerAffineForm {
            root: witness.root.clone(),
            target: witness.target.clone(),
            integer_type,
            coefficient,
            offset,
            endpoint_steps,
        })
    }

    fn policy_add_step(
        expression: &ScalarTerm,
        operand: &ScalarTerm,
        literal: i128,
    ) -> CheckedIntegerEndpointStep {
        if matches!(
            expression,
            ScalarTerm::SaturatingIntegerAdd { .. } | ScalarTerm::SaturatingIntegerSubtract { .. }
        ) {
            CheckedIntegerEndpointStep::SaturatingAdd(literal)
        } else {
            CheckedIntegerEndpointStep::WrappingAdd {
                operand: operand.clone(),
                literal,
            }
        }
    }

    fn apply_policy_expression_target(
        target: &ScalarTerm,
        current: &ScalarTerm,
        integer_type: IntegerType,
        coefficient: i128,
        offset: i128,
    ) -> Option<Result<(i128, i128, CheckedIntegerEndpointStep), IntegerAffineWitnessError>> {
        let (operand, literal) = match target {
            ScalarTerm::WrappingIntegerAdd {
                scalar_type,
                left,
                right,
            }
            | ScalarTerm::SaturatingIntegerAdd {
                scalar_type,
                left,
                right,
            } if *scalar_type == integer_type && left.as_ref() == current => {
                (left, signed_literal(right, integer_type, None)?.0)
            }
            ScalarTerm::WrappingIntegerAdd {
                scalar_type,
                left,
                right,
            }
            | ScalarTerm::SaturatingIntegerAdd {
                scalar_type,
                left,
                right,
            } if *scalar_type == integer_type && right.as_ref() == current => {
                (right, signed_literal(left, integer_type, None)?.0)
            }
            ScalarTerm::WrappingIntegerSubtract {
                scalar_type,
                left,
                right,
            }
            | ScalarTerm::SaturatingIntegerSubtract {
                scalar_type,
                left,
                right,
            } if *scalar_type == integer_type && left.as_ref() == current => {
                let (literal, _) = signed_literal(right, integer_type, None)?;
                (left, literal.checked_neg()?)
            }
            _ => return None,
        };
        Some(
            offset
                .checked_add(literal)
                .map(|offset| {
                    (
                        coefficient,
                        offset,
                        policy_add_step(target, operand, literal),
                    )
                })
                .ok_or(IntegerAffineWitnessError::CoefficientOverflow),
        )
    }

    fn signed_literal(
        term: &ScalarTerm,
        integer_type: IntegerType,
        landed: Option<&LandedInteger>,
    ) -> Option<(i128, bool)> {
        if let Some(value) = fixed_integer_as_i128(term, integer_type) {
            return Some((value, false));
        }
        landed
            .filter(|landed| landed.term == *term && landed.integer_type == integer_type)
            .and_then(|landed| integer_value_as_i128(landed.value).map(|value| (value, true)))
    }

    fn landed_integer(
        context: &PropositionContext,
        semantic_axioms: &[Proposition],
        definition_index: usize,
        literal_index: usize,
    ) -> Result<LandedInteger, IntegerAffineWitnessError> {
        if literal_index >= definition_index {
            return Err(IntegerAffineWitnessError::LiteralAxiomNotPrior {
                definition: definition_index,
                literal: literal_index,
            });
        }
        let proposition = checked_axiom(context, semantic_axioms, literal_index)?;
        let Proposition::Equal(left, right) = proposition else {
            return Err(IntegerAffineWitnessError::LiteralAxiomNotEquality(
                literal_index,
            ));
        };
        for (value, literal) in [(left, right), (right, left)] {
            if matches!(value, ScalarTerm::Value { .. })
                && let Some((integer_type, literal)) = literal.integer_value()
            {
                return Ok(LandedInteger {
                    term: value.clone(),
                    integer_type,
                    value: literal,
                });
            }
        }
        Err(IntegerAffineWitnessError::LiteralAxiomShapeMismatch(
            literal_index,
        ))
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum IntegerAffineWitnessError {
        RootNotValue,
        RootNotInteger,
        UnsupportedCarrier(IntegerType),
        TargetTypeMismatch,
        EmptyDefinitionChain,
        LiteralAxiomCountMismatch,
        NonCanonicalDefinitionOrder,
        UnknownSemanticAxiom(usize),
        MalformedProposition(semantic_vocabulary::PropositionError),
        DefinitionNotEquality(usize),
        LiteralAxiomNotPrior { definition: usize, literal: usize },
        LiteralAxiomNotEquality(usize),
        LiteralAxiomShapeMismatch(usize),
        UnusedLiteralAxiom(usize),
        DefinitionShapeMismatch(usize),
        AmbiguousDefinition(usize),
        CoefficientOverflow,
        ZeroDivisionLiteral,
        NegativeBitwiseAndMask,
        NegativeBitwiseOrSibling,
        ShiftCountNotLanded(usize),
        ShiftCountOutsideValueWidth(usize),
        TargetMismatch,
    }

    crate::debug_display!(IntegerAffineWitnessError);
    crate::checked_axiom!(IntegerAffineWitnessError);

    impl std::error::Error for IntegerAffineWitnessError {}
}

pub use bound_mapping::{
    IntegerAffineBoundConversionError, check_integer_affine_bound_conversion,
    integer_affine_wrapping_evidence, map_integer_affine_bound,
};
pub use terminal_psi::IntegerAffineWitness;
pub use truth_bounds::integer_affine_truth_bounds;
pub use witness_checking::{
    CheckedIntegerAffineForm, IntegerAffineWitnessError, check_integer_affine_witness,
};

#[cfg(test)]
mod tests {
    mod bound_mapping {
        use super::{literal, unsigned_literal, value};
        use crate::integer_rules::integer_affine::witness_checking::CheckedIntegerEndpointStep;
        use crate::integer_rules::integer_affine::{
            CheckedIntegerAffineForm, IntegerAffineBoundConversionError, IntegerAffineWitness,
            IntegerAffineWitnessError, check_integer_affine_bound_conversion,
            check_integer_affine_witness, integer_affine_truth_bounds, map_integer_affine_bound,
        };
        use crate::{affine_witness, value_context};
        use semantic_vocabulary::IntegerMathTerm;
        use semantic_vocabulary::IntegerType;
        use semantic_vocabulary::IntegerValue;
        use semantic_vocabulary::Proposition;
        use semantic_vocabulary::PropositionContext;
        use semantic_vocabulary::ScalarTerm;
        use semantic_vocabulary::ScalarType;
        use semantic_vocabulary::{IntegerSign, ValueId};

        macro_rules! check {
            ($form:expr => $premise:expr => $conclusion:expr) => {
                check_integer_affine_bound_conversion($form, $premise, $conclusion)
            };
        }

        mod correlated {
            use super::*;

            macro_rules! math_value {
                ($source_type:expr => $value:expr) => {
                    IntegerMathTerm::MathValue {
                        source_type: $source_type,
                        value: $value,
                    }
                };
            }

            #[test]
            fn correlated_add_bound_replays_exact_complement_identity_and_relation() {
                let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
                let left = value(1, integer_type);
                let right = value(2, integer_type);
                let correlated = value(3, integer_type);
                let redirected = value(4, integer_type);
                let target =
                    ScalarTerm::exact_integer_add(integer_type, left.clone(), right.clone())
                        .expect("exact add");
                let context = value_context(ScalarType::Integer(integer_type), 5);
                for (endpoint, evidence, expected) in [
                    (
                        127,
                        Proposition::LessOrEqual(left.clone(), correlated.clone()),
                        Proposition::IntegerMathLessOrEqual(
                            IntegerMathTerm::Add(
                                Box::new(math_value!(integer_type => ValueId::new(1).unwrap())),
                                Box::new(math_value!(integer_type => ValueId::new(2).unwrap())),
                            ),
                            IntegerMathTerm::literal(IntegerValue::Signed(127)),
                        ),
                    ),
                    (
                        -128,
                        Proposition::LessOrEqual(correlated.clone(), left.clone()),
                        Proposition::IntegerMathLessOrEqual(
                            IntegerMathTerm::literal(IntegerValue::Signed(-128)),
                            IntegerMathTerm::Add(
                                Box::new(math_value!(integer_type => ValueId::new(1).unwrap())),
                                Box::new(math_value!(integer_type => ValueId::new(2).unwrap())),
                            ),
                        ),
                    ),
                ] {
                    let axiom = Proposition::Equal(
                        correlated.clone(),
                        ScalarTerm::exact_integer_subtract(
                            integer_type,
                            literal(integer_type, endpoint),
                            right.clone(),
                        )
                        .unwrap(),
                    );
                    let witness =
                        affine_witness(correlated.clone(), target.clone(), vec![0], vec![None]);
                    let checked = check_integer_affine_witness(
                        &context,
                        std::slice::from_ref(&axiom),
                        &witness,
                    )
                    .expect("correlated add witness");
                    assert_eq!(map_integer_affine_bound(&checked, &evidence), Ok(expected));
                    assert_eq!(
                        map_integer_affine_bound(
                            &checked,
                            &Proposition::LessOrEqual(left.clone(), redirected.clone()),
                        ),
                        Err(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch),
                    );
                    assert!(
                        check_integer_affine_witness(
                            &context,
                            &[Proposition::Equal(
                                correlated.clone(),
                                ScalarTerm::exact_integer_subtract(
                                    integer_type,
                                    literal(integer_type, endpoint),
                                    redirected.clone(),
                                )
                                .unwrap(),
                            )],
                            &witness,
                        )
                        .is_err(),
                        "redirecting the complement operand invalidates the witness",
                    );
                    assert!(
                        check_integer_affine_witness(
                            &context,
                            std::slice::from_ref(&axiom),
                            &IntegerAffineWitness {
                                definition_axioms: Vec::new(),
                                literal_axioms: Vec::new(),
                                ..witness
                            },
                        )
                        .is_err(),
                        "omitting correlated definition custody invalidates the witness",
                    );
                }

                let landed_endpoint = value(5, integer_type);
                let landing =
                    Proposition::Equal(landed_endpoint.clone(), literal(integer_type, 127));
                let complement = Proposition::Equal(
                    correlated.clone(),
                    ScalarTerm::exact_integer_subtract(
                        integer_type,
                        landed_endpoint.clone(),
                        right.clone(),
                    )
                    .unwrap(),
                );
                let landed_witness =
                    affine_witness(correlated.clone(), target.clone(), vec![1], vec![Some(0)]);
                let landed = check_integer_affine_witness(
                    &context,
                    &[landing.clone(), complement.clone()],
                    &landed_witness,
                )
                .expect("landed carrier endpoint and complement are replayed in order");
                assert_eq!(
                    map_integer_affine_bound(
                        &landed,
                        &Proposition::LessOrEqual(left.clone(), correlated.clone()),
                    ),
                    Ok(Proposition::IntegerMathLessOrEqual(
                        IntegerMathTerm::Add(
                            Box::new(math_value!(integer_type => ValueId::new(1).unwrap())),
                            Box::new(math_value!(integer_type => ValueId::new(2).unwrap())),
                        ),
                        IntegerMathTerm::literal(IntegerValue::Signed(127)),
                    )),
                );
                let redirected_complement = Proposition::Equal(
                    correlated.clone(),
                    ScalarTerm::exact_integer_subtract(
                        integer_type,
                        landed_endpoint.clone(),
                        redirected.clone(),
                    )
                    .unwrap(),
                );
                for (mutation, (axioms, witness)) in [
                    (
                        vec![landing.clone(), complement.clone()],
                        IntegerAffineWitness {
                            literal_axioms: vec![None],
                            ..landed_witness.clone()
                        },
                    ),
                    (
                        vec![complement.clone(), landing.clone()],
                        IntegerAffineWitness {
                            definition_axioms: vec![0],
                            literal_axioms: vec![Some(1)],
                            ..landed_witness.clone()
                        },
                    ),
                    (
                        vec![
                            Proposition::Equal(landed_endpoint.clone(), literal(integer_type, 126)),
                            complement.clone(),
                        ],
                        landed_witness.clone(),
                    ),
                    (
                        vec![
                            Proposition::Equal(landed_endpoint.clone(), ScalarTerm::boolean(true)),
                            complement.clone(),
                        ],
                        landed_witness.clone(),
                    ),
                    (
                        vec![
                            Proposition::Equal(redirected, literal(integer_type, 127)),
                            complement,
                        ],
                        landed_witness.clone(),
                    ),
                    (vec![landing, redirected_complement], landed_witness),
                ]
                .into_iter()
                .enumerate()
                {
                    let result = check_integer_affine_witness(&context, &axioms, &witness);
                    assert!(
                        result.is_err(),
                        "reordered, omitted, or stale endpoint landing custody rejects: {mutation}: {result:?}",
                    );
                }
            }

            #[test]
            fn correlated_subtract_replays_complement_definition_and_endpoint_landing() {
                let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
                let left = value(1, integer_type);
                let right = value(2, integer_type);
                let correlated = value(3, integer_type);
                let landed_endpoint = value(4, integer_type);
                let target =
                    ScalarTerm::exact_integer_subtract(integer_type, left.clone(), right.clone())
                        .unwrap();
                let context = value_context(ScalarType::Integer(integer_type), 5);
                let landing =
                    Proposition::Equal(landed_endpoint.clone(), literal(integer_type, -128));
                let complement = Proposition::Equal(
                    correlated.clone(),
                    ScalarTerm::exact_integer_add(
                        integer_type,
                        landed_endpoint.clone(),
                        right.clone(),
                    )
                    .unwrap(),
                );
                let witness =
                    affine_witness(correlated.clone(), target.clone(), vec![1], vec![Some(0)]);
                let checked = check_integer_affine_witness(
                    &context,
                    &[landing.clone(), complement.clone()],
                    &witness,
                )
                .expect("landed MIN plus right complement");
                assert_eq!(
                    map_integer_affine_bound(
                        &checked,
                        &Proposition::LessOrEqual(correlated.clone(), left.clone()),
                    ),
                    Ok(Proposition::IntegerMathLessOrEqual(
                        IntegerMathTerm::literal(IntegerValue::Signed(-128)),
                        IntegerMathTerm::Subtract(
                            Box::new(math_value!(integer_type => ValueId::new(1).unwrap())),
                            Box::new(math_value!(integer_type => ValueId::new(2).unwrap())),
                        ),
                    )),
                );
                assert!(
                    check_integer_affine_witness(
                        &context,
                        &[landing.clone(), complement.clone()],
                        &IntegerAffineWitness {
                            literal_axioms: vec![None],
                            ..witness.clone()
                        },
                    )
                    .is_err(),
                    "omitting the endpoint landing rejects",
                );
                let stale = Proposition::Equal(value(5, integer_type), literal(integer_type, -128));
                assert!(
                    check_integer_affine_witness(&context, &[stale, complement], &witness).is_err(),
                    "redirecting the endpoint landing rejects",
                );
                assert!(
        map_integer_affine_bound(&checked, &Proposition::LessOrEqual(left, correlated),).is_err(),
        "reversing the authored guard rejects",
    );
            }

            #[test]
            fn correlated_negative_multiply_uses_target_endpoint_and_sign_orientation() {
                let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
                let left = value(1, integer_type);
                let right = value(2, integer_type);
                let quotient = value(3, integer_type);
                let context = value_context(ScalarType::Integer(integer_type), 5);
                let target =
                    ScalarTerm::exact_integer_multiply(integer_type, left.clone(), right.clone())
                        .unwrap();
                let negative = Proposition::LessOrEqual(right.clone(), literal(integer_type, -2));
                for (endpoint, comparison, expected) in [
                    (
                        -128,
                        Proposition::LessOrEqual(left.clone(), quotient.clone()),
                        Proposition::IntegerMathLessOrEqual(
                            IntegerMathTerm::literal(IntegerValue::Signed(-128)),
                            IntegerMathTerm::Multiply(
                                Box::new(math_value!(integer_type => ValueId::new(1).unwrap())),
                                Box::new(math_value!(integer_type => ValueId::new(2).unwrap())),
                            ),
                        ),
                    ),
                    (
                        127,
                        Proposition::LessOrEqual(quotient.clone(), left.clone()),
                        Proposition::IntegerMathLessOrEqual(
                            IntegerMathTerm::Multiply(
                                Box::new(math_value!(integer_type => ValueId::new(1).unwrap())),
                                Box::new(math_value!(integer_type => ValueId::new(2).unwrap())),
                            ),
                            IntegerMathTerm::literal(IntegerValue::Signed(127)),
                        ),
                    ),
                ] {
                    let axiom = Proposition::Equal(
                        quotient.clone(),
                        ScalarTerm::exact_integer_divide(
                            integer_type,
                            literal(integer_type, endpoint),
                            right.clone(),
                        )
                        .unwrap(),
                    );
                    let checked = check_integer_affine_witness(
                        &context,
                        std::slice::from_ref(&axiom),
                        &affine_witness(quotient.clone(), target.clone(), vec![0], vec![None]),
                    )
                    .expect("negative quotient witness");
                    let evidence =
                        Proposition::Conjunction(vec![negative.clone(), comparison.clone()]);
                    assert_eq!(map_integer_affine_bound(&checked, &evidence), Ok(expected));
                    let Proposition::LessOrEqual(comparison_left, comparison_right) = &comparison
                    else {
                        unreachable!("correlated comparison is ordered")
                    };
                    assert_eq!(
                        map_integer_affine_bound(
                            &checked,
                            &Proposition::Conjunction(vec![
                                negative.clone(),
                                Proposition::LessOrEqual(
                                    comparison_right.clone(),
                                    comparison_left.clone(),
                                ),
                            ]),
                        ),
                        Err(IntegerAffineBoundConversionError::DirectMultiplyEvidenceMismatch),
                        "for right=-2, lower uses left<=MIN/right (64) and upper uses MAX/right (-63)<=left",
                    );
                }

                let landed_endpoint = value(4, integer_type);
                let landing =
                    Proposition::Equal(landed_endpoint.clone(), literal(integer_type, -128));
                let definition = Proposition::Equal(
                    quotient.clone(),
                    ScalarTerm::exact_integer_divide(
                        integer_type,
                        landed_endpoint.clone(),
                        right.clone(),
                    )
                    .unwrap(),
                );
                let landed_witness = IntegerAffineWitness {
                    root: quotient,
                    target,
                    definition_axioms: vec![1],
                    literal_axioms: vec![Some(0)],
                };
                assert!(
                    check_integer_affine_witness(
                        &context,
                        &[landing.clone(), definition.clone()],
                        &landed_witness,
                    )
                    .is_ok(),
                    "an exact earlier endpoint landing is replayed",
                );
                for (axioms, witness) in [
                    (
                        vec![landing.clone(), definition.clone()],
                        IntegerAffineWitness {
                            literal_axioms: vec![None],
                            ..landed_witness.clone()
                        },
                    ),
                    (
                        vec![definition.clone(), landing.clone()],
                        IntegerAffineWitness {
                            definition_axioms: vec![0],
                            literal_axioms: vec![Some(1)],
                            ..landed_witness.clone()
                        },
                    ),
                    (
                        vec![
                            Proposition::Equal(
                                landed_endpoint.clone(),
                                literal(integer_type, -127),
                            ),
                            definition.clone(),
                        ],
                        landed_witness.clone(),
                    ),
                    (
                        vec![
                            landing,
                            Proposition::Equal(
                                landed_witness.root.clone(),
                                ScalarTerm::exact_integer_divide(
                                    integer_type,
                                    landed_endpoint,
                                    value(5, integer_type),
                                )
                                .unwrap(),
                            ),
                        ],
                        landed_witness,
                    ),
                ] {
                    assert!(check_integer_affine_witness(&context, &axioms, &witness).is_err());
                }
            }
        }
        mod direct {
            use super::*;

            macro_rules! math_value {
                ($source_type:expr => $value:expr) => {
                    IntegerMathTerm::MathValue {
                        source_type: $source_type,
                        value: $value,
                    }
                };
            }

            macro_rules! le {
                ($left:expr => $right:expr) => {
                    Proposition::IntegerMathLessOrEqual($left, $right)
                };
            }

            #[test]
            fn direct_add_bound_replays_both_ordered_endpoints_and_rejects_mutations() {
                let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
                let left = value(1, integer_type);
                let right = value(2, integer_type);
                let target =
                    ScalarTerm::exact_integer_add(integer_type, left.clone(), right.clone())
                        .expect("exact add");
                let context = PropositionContext::from_value_types([
                    (ValueId::new(1).unwrap(), ScalarType::Integer(integer_type)),
                    (ValueId::new(2).unwrap(), ScalarType::Integer(integer_type)),
                    (ValueId::new(3).unwrap(), ScalarType::Integer(integer_type)),
                ])
                .unwrap();
                let checked = check_integer_affine_witness(
                    &context,
                    &[],
                    &IntegerAffineWitness {
                        root: left.clone(),
                        target,
                        definition_axioms: Vec::new(),
                        literal_axioms: Vec::new(),
                    },
                )
                .expect("direct mathematical add endpoint");
                let sum = IntegerMathTerm::Add(
                    Box::new(math_value!(integer_type => ValueId::new(1).unwrap())),
                    Box::new(math_value!(integer_type => ValueId::new(2).unwrap())),
                );
                assert_eq!(
                    map_integer_affine_bound(
                        &checked,
                        &Proposition::Conjunction(vec![
                            Proposition::LessOrEqual(literal(integer_type, -100), left.clone()),
                            Proposition::LessOrEqual(literal(integer_type, 20), right.clone()),
                        ]),
                    ),
                    Ok(le!(IntegerMathTerm::literal(IntegerValue::Signed(-80)) => sum.clone())),
                );
                assert_eq!(
                    map_integer_affine_bound(
                        &checked,
                        &Proposition::Conjunction(vec![
                            Proposition::LessOrEqual(left.clone(), literal(integer_type, 100)),
                            Proposition::LessOrEqual(right.clone(), literal(integer_type, 20)),
                        ]),
                    ),
                    Ok(le!(sum.clone() => IntegerMathTerm::literal(IntegerValue::Signed(120)))),
                );
                assert_eq!(
                    map_integer_affine_bound(
                        &checked,
                        &Proposition::Conjunction(vec![
                            Proposition::Equal(left.clone(), literal(integer_type, 7)),
                            Proposition::LessOrEqual(right.clone(), literal(integer_type, 100)),
                        ]),
                    ),
                    Ok(le!(sum.clone() => IntegerMathTerm::literal(IntegerValue::Signed(107)))),
                );
                assert_eq!(
                    map_integer_affine_bound(
                        &checked,
                        &Proposition::Conjunction(vec![
                            Proposition::Truth,
                            Proposition::LessOrEqual(literal(integer_type, 0), right.clone()),
                        ]),
                    ),
                    Ok(le!(IntegerMathTerm::literal(IntegerValue::Signed(-128)) => sum.clone())),
                );
                assert_eq!(
                    map_integer_affine_bound(
                        &checked,
                        &Proposition::Conjunction(vec![
                            Proposition::Truth,
                            Proposition::LessOrEqual(right.clone(), literal(integer_type, 0)),
                        ]),
                    ),
                    Ok(le!(sum.clone() => IntegerMathTerm::literal(IntegerValue::Signed(127)))),
                );
                assert_eq!(
                    map_integer_affine_bound(
                        &checked,
                        &Proposition::Conjunction(vec![Proposition::Truth, Proposition::Truth]),
                    ),
                    Err(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch),
                );

                let seven = literal(integer_type, 7);
                for (target, root, evidence, expected_sum) in [
                    (
                        ScalarTerm::exact_integer_add(integer_type, left.clone(), seven.clone())
                            .unwrap(),
                        left.clone(),
                        Proposition::Conjunction(vec![
                            Proposition::LessOrEqual(left.clone(), literal(integer_type, 100)),
                            Proposition::Truth,
                        ]),
                        IntegerMathTerm::Add(
                            Box::new(math_value!(integer_type => ValueId::new(1).unwrap())),
                            Box::new(IntegerMathTerm::literal(IntegerValue::Signed(7))),
                        ),
                    ),
                    (
                        ScalarTerm::exact_integer_add(integer_type, seven.clone(), right.clone())
                            .unwrap(),
                        seven,
                        Proposition::Conjunction(vec![
                            Proposition::Truth,
                            Proposition::LessOrEqual(right.clone(), literal(integer_type, 100)),
                        ]),
                        IntegerMathTerm::Add(
                            Box::new(IntegerMathTerm::literal(IntegerValue::Signed(7))),
                            Box::new(math_value!(integer_type => ValueId::new(2).unwrap())),
                        ),
                    ),
                ] {
                    let literal_checked = check_integer_affine_witness(
                        &context,
                        &[],
                        &IntegerAffineWitness {
                            root,
                            target,
                            definition_axioms: Vec::new(),
                            literal_axioms: Vec::new(),
                        },
                    )
                    .expect("direct add with embedded literal");
                    assert_eq!(
                        map_integer_affine_bound(&literal_checked, &evidence),
                        Ok(
                            le!(expected_sum => IntegerMathTerm::literal(IntegerValue::Signed(107)))
                        ),
                    );
                }

                for malformed in [
                    Proposition::LessOrEqual(left.clone(), literal(integer_type, 100)),
                    Proposition::Conjunction(vec![
                        Proposition::LessOrEqual(literal(integer_type, -100), left.clone()),
                        Proposition::LessOrEqual(right.clone(), literal(integer_type, 20)),
                    ]),
                    Proposition::Conjunction(vec![
                        Proposition::LessOrEqual(
                            value(3, integer_type),
                            literal(integer_type, 100),
                        ),
                        Proposition::LessOrEqual(right.clone(), literal(integer_type, 20)),
                    ]),
                    Proposition::Conjunction(vec![
                        Proposition::LessOrEqual(right.clone(), literal(integer_type, 20)),
                        Proposition::LessOrEqual(left.clone(), literal(integer_type, 100)),
                    ]),
                ] {
                    assert_eq!(
                        map_integer_affine_bound(&checked, &malformed),
                        Err(IntegerAffineBoundConversionError::DirectAddEvidenceMismatch),
                    );
                }

                let i128_type = IntegerType::new(IntegerSign::Signed, 128).expect("i128");
                let i128_left = value(4, i128_type);
                let i128_right = value(5, i128_type);
                let i128_context = PropositionContext::from_value_types([
                    (ValueId::new(4).unwrap(), ScalarType::Integer(i128_type)),
                    (ValueId::new(5).unwrap(), ScalarType::Integer(i128_type)),
                ])
                .unwrap();
                let i128_checked = check_integer_affine_witness(
                    &i128_context,
                    &[],
                    &IntegerAffineWitness {
                        root: i128_left.clone(),
                        target: ScalarTerm::exact_integer_add(
                            i128_type,
                            i128_left.clone(),
                            i128_right.clone(),
                        )
                        .unwrap(),
                        definition_axioms: Vec::new(),
                        literal_axioms: Vec::new(),
                    },
                )
                .unwrap();
                assert_eq!(
                    map_integer_affine_bound(
                        &i128_checked,
                        &Proposition::Conjunction(vec![
                            Proposition::LessOrEqual(literal(i128_type, i128::MIN), i128_left,),
                            Proposition::LessOrEqual(literal(i128_type, i128::MIN), i128_right,),
                        ]),
                    ),
                    Err(IntegerAffineBoundConversionError::DirectAddBoundOverflow),
                );
            }

            #[test]
            fn direct_subtract_bound_replays_opposite_ordered_endpoints_and_rejects_mutations() {
                let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
                let left = value(1, integer_type);
                let right = value(2, integer_type);
                let target =
                    ScalarTerm::exact_integer_subtract(integer_type, left.clone(), right.clone())
                        .unwrap();
                let context = value_context(ScalarType::Integer(integer_type), 5);
                let checked = check_integer_affine_witness(
                    &context,
                    &[],
                    &IntegerAffineWitness {
                        root: left.clone(),
                        target,
                        definition_axioms: Vec::new(),
                        literal_axioms: Vec::new(),
                    },
                )
                .expect("direct subtract witness");
                let lower = Proposition::Conjunction(vec![
                    Proposition::LessOrEqual(literal(integer_type, -100), left.clone()),
                    Proposition::LessOrEqual(right.clone(), literal(integer_type, 20)),
                ]);
                assert_eq!(
                    map_integer_affine_bound(&checked, &lower),
                    Ok(Proposition::IntegerMathLessOrEqual(
                        IntegerMathTerm::literal(IntegerValue::Signed(-120)),
                        IntegerMathTerm::Subtract(
                            Box::new(math_value!(integer_type => ValueId::new(1).unwrap())),
                            Box::new(math_value!(integer_type => ValueId::new(2).unwrap())),
                        ),
                    )),
                );
                let upper = Proposition::Conjunction(vec![
                    Proposition::LessOrEqual(left.clone(), literal(integer_type, 100)),
                    Proposition::LessOrEqual(literal(integer_type, -20), right.clone()),
                ]);
                assert!(map_integer_affine_bound(&checked, &upper).is_ok());
                for malformed in [
                    Proposition::Conjunction(vec![
                        Proposition::LessOrEqual(literal(integer_type, -100), left.clone()),
                        Proposition::LessOrEqual(literal(integer_type, -20), right.clone()),
                    ]),
                    Proposition::Conjunction(vec![
                        Proposition::LessOrEqual(right.clone(), literal(integer_type, 20)),
                        Proposition::LessOrEqual(literal(integer_type, -100), left.clone()),
                    ]),
                    Proposition::Conjunction(vec![
                        Proposition::LessOrEqual(
                            literal(integer_type, -100),
                            value(3, integer_type),
                        ),
                        Proposition::LessOrEqual(right.clone(), literal(integer_type, 20)),
                    ]),
                ] {
                    assert!(
                        map_integer_affine_bound(&checked, &malformed).is_err(),
                        "mixed orientation, reordered evidence, and redirected operands reject",
                    );
                }
                assert!(
                    map_integer_affine_bound(
                        &checked,
                        &Proposition::Conjunction(vec![
                            Proposition::Equal(left.clone(), literal(integer_type, 127)),
                            Proposition::Truth,
                        ]),
                    )
                    .is_ok(),
                    "an exact MAX minuend and carrier-wide subtrahend derive the lower endpoint",
                );
                assert!(
                    map_integer_affine_bound(
                        &checked,
                        &Proposition::Conjunction(vec![
                            Proposition::Equal(left, literal(integer_type, 100)),
                            Proposition::Truth,
                        ]),
                    )
                    .is_err(),
                    "a noncarrier exact minuend cannot orient bare subtrahend inclusion",
                );
            }

            #[test]
            fn direct_multiply_replays_four_corners_and_rejects_order_mutations() {
                let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
                let left = value(1, integer_type);
                let right = value(2, integer_type);
                let context = PropositionContext::from_value_types([
                    (ValueId::new(1).unwrap(), ScalarType::Integer(integer_type)),
                    (ValueId::new(2).unwrap(), ScalarType::Integer(integer_type)),
                    (ValueId::new(3).unwrap(), ScalarType::Integer(integer_type)),
                ])
                .unwrap();
                let checked = check_integer_affine_witness(
                    &context,
                    &[],
                    &IntegerAffineWitness {
                        root: left.clone(),
                        target: ScalarTerm::exact_integer_multiply(
                            integer_type,
                            left.clone(),
                            right.clone(),
                        )
                        .unwrap(),
                        definition_axioms: Vec::new(),
                        literal_axioms: Vec::new(),
                    },
                )
                .expect("direct multiply witness");
                let lower = Proposition::Conjunction(vec![
                    Proposition::LessOrEqual(literal(integer_type, -4), left.clone()),
                    Proposition::LessOrEqual(left.clone(), literal(integer_type, 5)),
                    Proposition::LessOrEqual(literal(integer_type, -3), right.clone()),
                    Proposition::LessOrEqual(right.clone(), literal(integer_type, 2)),
                ]);
                let product = IntegerMathTerm::Multiply(
                    Box::new(math_value!(integer_type => ValueId::new(1).unwrap())),
                    Box::new(math_value!(integer_type => ValueId::new(2).unwrap())),
                );
                assert_eq!(
                    map_integer_affine_bound(&checked, &lower),
                    Ok(le!(IntegerMathTerm::literal(IntegerValue::Signed(-15)) => product)),
                );
                for malformed in [
                    Proposition::Conjunction(vec![
                        Proposition::LessOrEqual(left.clone(), literal(integer_type, 5)),
                        Proposition::LessOrEqual(literal(integer_type, -4), left.clone()),
                        Proposition::LessOrEqual(literal(integer_type, -3), right.clone()),
                        Proposition::LessOrEqual(right.clone(), literal(integer_type, 2)),
                    ]),
                    Proposition::Conjunction(vec![
                        Proposition::LessOrEqual(literal(integer_type, -4), left.clone()),
                        Proposition::LessOrEqual(left.clone(), literal(integer_type, 5)),
                        Proposition::LessOrEqual(literal(integer_type, -3), value(3, integer_type)),
                        Proposition::LessOrEqual(right.clone(), literal(integer_type, 2)),
                    ]),
                    Proposition::Conjunction(vec![
                        Proposition::LessOrEqual(literal(integer_type, -4), left.clone()),
                        Proposition::LessOrEqual(left, literal(integer_type, 5)),
                        Proposition::LessOrEqual(literal(integer_type, -3), right),
                    ]),
                ] {
                    assert!(map_integer_affine_bound(&checked, &malformed).is_err());
                }
            }

            #[test]
            fn direct_multiply_requires_oriented_zero_evidence_for_exact_zero_bounds() {
                let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
                let left = value(1, integer_type);
                let right = value(2, integer_type);
                let context = PropositionContext::from_value_types([
                    (ValueId::new(1).unwrap(), ScalarType::Integer(integer_type)),
                    (ValueId::new(2).unwrap(), ScalarType::Integer(integer_type)),
                ])
                .unwrap();
                let checked = check_integer_affine_witness(
                    &context,
                    &[],
                    &IntegerAffineWitness {
                        root: left.clone(),
                        target: ScalarTerm::exact_integer_multiply(
                            integer_type,
                            left.clone(),
                            right.clone(),
                        )
                        .unwrap(),
                        definition_axioms: Vec::new(),
                        literal_axioms: Vec::new(),
                    },
                )
                .expect("direct multiply witness");
                let zero = literal(integer_type, 0);
                let product = IntegerMathTerm::Multiply(
                    Box::new(math_value!(integer_type => ValueId::new(1).unwrap())),
                    Box::new(math_value!(integer_type => ValueId::new(2).unwrap())),
                );
                let lower = Proposition::Conjunction(vec![
                    Proposition::Truth,
                    Proposition::Truth,
                    Proposition::LessOrEqual(zero.clone(), right.clone()),
                    Proposition::LessOrEqual(right.clone(), zero.clone()),
                ]);
                assert_eq!(
                    map_integer_affine_bound(&checked, &lower),
                    Ok(le!(IntegerMathTerm::literal(IntegerValue::Signed(0)) => product.clone())),
                );
                let upper = Proposition::Conjunction(vec![
                    Proposition::Truth,
                    Proposition::Truth,
                    Proposition::LessOrEqual(right.clone(), zero.clone()),
                    Proposition::LessOrEqual(zero, right.clone()),
                ]);
                assert_eq!(
                    map_integer_affine_bound(&checked, &upper),
                    Ok(le!(product => IntegerMathTerm::literal(IntegerValue::Signed(0)))),
                );

                let one_equality = Proposition::Equal(right, literal(integer_type, 1));
                let unoriented_nonzero = Proposition::Conjunction(vec![
                    Proposition::Truth,
                    Proposition::Truth,
                    one_equality.clone(),
                    one_equality,
                ]);
                assert!(map_integer_affine_bound(&checked, &unoriented_nonzero).is_err());
            }

            #[test]
            fn direct_shift_bound_replays_count_range_and_sign_oriented_endpoint() {
                let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
                let count_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8 count");
                let root = value(1, integer_type);
                let count = value(2, count_type);
                let target = ScalarTerm::exact_integer_shift_left(
                    integer_type,
                    count_type,
                    root.clone(),
                    count.clone(),
                )
                .unwrap();
                let context = PropositionContext::from_value_types([
                    (ValueId::new(1).unwrap(), ScalarType::Integer(integer_type)),
                    (ValueId::new(2).unwrap(), ScalarType::Integer(count_type)),
                ])
                .unwrap();
                let checked = check_integer_affine_witness(
                    &context,
                    &[],
                    &IntegerAffineWitness {
                        root: root.clone(),
                        target,
                        definition_axioms: Vec::new(),
                        literal_axioms: Vec::new(),
                    },
                )
                .expect("direct mathematical shift endpoint");
                let lower_count = Proposition::LessOrEqual(literal(count_type, 0), count.clone());
                let upper_count = Proposition::LessOrEqual(count.clone(), literal(count_type, 3));
                let shifted = IntegerMathTerm::ShiftLeft {
                    value: Box::new(math_value!(integer_type => ValueId::new(1).unwrap())),
                    count: Box::new(math_value!(count_type => ValueId::new(2).unwrap())),
                };
                let mapped = |root_bound| {
                    map_integer_affine_bound(
                        &checked,
                        &Proposition::Conjunction(vec![
                            root_bound,
                            lower_count.clone(),
                            upper_count.clone(),
                        ]),
                    )
                };
                assert_eq!(
                    mapped(Proposition::LessOrEqual(
                        literal(integer_type, -16),
                        root.clone(),
                    )),
                    Ok(
                        le!(IntegerMathTerm::literal(IntegerValue::Signed(-128)) => shifted.clone())
                    ),
                );
                assert_eq!(
                    mapped(Proposition::LessOrEqual(
                        root.clone(),
                        literal(integer_type, 15),
                    )),
                    Ok(le!(shifted.clone() => IntegerMathTerm::literal(IntegerValue::Signed(120)))),
                );
                assert_eq!(
                    mapped(Proposition::LessOrEqual(
                        literal(integer_type, 2),
                        root.clone(),
                    )),
                    Ok(le!(IntegerMathTerm::literal(IntegerValue::Signed(2)) => shifted.clone())),
                );
                assert_eq!(
                    mapped(Proposition::LessOrEqual(
                        root.clone(),
                        literal(integer_type, -2),
                    )),
                    Ok(le!(shifted => IntegerMathTerm::literal(IntegerValue::Signed(-2)))),
                );
                assert_eq!(
                    map_integer_affine_bound(
                        &checked,
                        &Proposition::Conjunction(vec![
                            Proposition::LessOrEqual(root.clone(), literal(integer_type, 15)),
                            upper_count,
                        ]),
                    ),
                    Err(IntegerAffineBoundConversionError::DirectShiftCountLowerMissing),
                );
                assert_eq!(
                    map_integer_affine_bound(
                        &checked,
                        &Proposition::Conjunction(vec![
                            Proposition::LessOrEqual(root, literal(integer_type, 15)),
                            lower_count,
                            Proposition::LessOrEqual(count, literal(count_type, 8)),
                        ]),
                    ),
                    Err(IntegerAffineBoundConversionError::DirectShiftCountOutsideValueWidth),
                );
            }
        }

        #[test]

        fn unsigned_remainder_intersects_dividend_bounds_and_accepts_legacy_images() {
            let integer_type = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
            let root = value(1, integer_type);
            let target = value(2, integer_type);
            let context = PropositionContext::from_value_types([
                (ValueId::new(1).unwrap(), ScalarType::Integer(integer_type)),
                (ValueId::new(2).unwrap(), ScalarType::Integer(integer_type)),
            ])
            .unwrap();
            let literal = |number| unsigned_literal(integer_type, number);
            let definition = Proposition::Equal(
                target.clone(),
                ScalarTerm::exact_integer_remainder(integer_type, root.clone(), literal(256))
                    .unwrap(),
            );
            let form = check_integer_affine_witness(
                &context,
                &[definition],
                &affine_witness(root.clone(), target.clone(), vec![0], vec![None]),
            )
            .unwrap();
            for (bound, expected) in [
                (0, 0),
                (57, 57),
                (255, 255),
                (256, 255),
                (u32::MAX as u128, 255),
            ] {
                let premise = Proposition::LessOrEqual(root.clone(), literal(bound));
                let conclusion = Proposition::LessOrEqual(target.clone(), literal(expected));
                assert_eq!(
                    map_integer_affine_bound(&form, &premise),
                    Ok(conclusion.clone())
                );
                assert_eq!(
                    check_integer_affine_bound_conversion(&form, &premise, &conclusion),
                    Ok(())
                );
                assert_eq!(
                    check!(&form => &premise => &Proposition::LessOrEqual(target.clone(), literal(255))),
                    Ok(()),
                    "the previous total-image certificate remains valid",
                );
            }
            for wrong in [56, 58] {
                assert_eq!(
                    check_integer_affine_bound_conversion(
                        &form,
                        &Proposition::LessOrEqual(root.clone(), literal(57)),
                        &Proposition::LessOrEqual(target.clone(), literal(wrong)),
                    ),
                    Err(IntegerAffineBoundConversionError::ConclusionMismatch),
                    "only the tight or legacy mapping is admitted; relaxation needs evidence",
                );
            }
            assert_eq!(
                map_integer_affine_bound(
                    &form,
                    &Proposition::LessOrEqual(literal(57), root.clone())
                ),
                Ok(Proposition::LessOrEqual(literal(0), target)),
            );
            assert_eq!(
                map_integer_affine_bound(&form, &Proposition::LessThan(root, literal(57))),
                Err(IntegerAffineBoundConversionError::StrictBoundNotTranslation),
            );
        }

        #[test]
        fn signed_remainder_keeps_truncation_bounds_for_both_divisor_signs() {
            let integer_type = IntegerType::new(IntegerSign::Signed, 16).unwrap();
            let root = value(1, integer_type);
            let target = value(2, integer_type);
            let context = PropositionContext::from_value_types([
                (ValueId::new(1).unwrap(), ScalarType::Integer(integer_type)),
                (ValueId::new(2).unwrap(), ScalarType::Integer(integer_type)),
            ])
            .unwrap();
            for divisor in [-256, 256] {
                let definition = Proposition::Equal(
                    target.clone(),
                    ScalarTerm::exact_integer_remainder(
                        integer_type,
                        root.clone(),
                        literal(integer_type, divisor),
                    )
                    .unwrap(),
                );
                let form = check_integer_affine_witness(
                    &context,
                    &[definition],
                    &affine_witness(root.clone(), target.clone(), vec![0], vec![None]),
                )
                .unwrap();
                for (bound, upper, expected) in [
                    (-57, true, 0),
                    (0, true, 0),
                    (57, true, 255),
                    (-57, false, -255),
                    (0, false, 0),
                    (57, false, 0),
                ] {
                    let premise = if upper {
                        Proposition::LessOrEqual(root.clone(), literal(integer_type, bound))
                    } else {
                        Proposition::LessOrEqual(literal(integer_type, bound), root.clone())
                    };
                    let conclusion = if upper {
                        Proposition::LessOrEqual(target.clone(), literal(integer_type, expected))
                    } else {
                        Proposition::LessOrEqual(literal(integer_type, expected), target.clone())
                    };
                    assert_eq!(map_integer_affine_bound(&form, &premise), Ok(conclusion));
                }
            }
        }

        #[test]
        fn maps_upper_and_lower_bounds_across_every_coefficient_sign() {
            let integer_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16");
            let root = value(1, integer_type);
            let target = value(2, integer_type);
            let literal = |value| literal(integer_type, value);
            let form = |coefficient, offset| CheckedIntegerAffineForm {
                root: root.clone(),
                target: target.clone(),
                integer_type,
                coefficient,
                offset,
                endpoint_steps: vec![
                    CheckedIntegerEndpointStep::Multiply(coefficient),
                    CheckedIntegerEndpointStep::Add(offset),
                ],
            };
            let upper = Proposition::LessOrEqual(root.clone(), literal(4));
            let lower = Proposition::LessOrEqual(literal(-3), root.clone());

            assert_eq!(
                check!(&form(2, 1) => &upper => &Proposition::LessOrEqual(target.clone(), literal(9))),
                Ok(()),
            );
            assert_eq!(
                check!(&form(2, 1) => &lower => &Proposition::LessOrEqual(literal(-5), target.clone())),
                Ok(()),
            );
            assert_eq!(
                check!(&form(-2, 1) => &upper => &Proposition::LessOrEqual(literal(-7), target.clone())),
                Ok(()),
            );
            assert_eq!(
                check!(&form(-2, 1) => &lower => &Proposition::LessOrEqual(target.clone(), literal(7))),
                Ok(()),
            );
            assert_eq!(
                check!(&form(0, 5) => &upper => &Proposition::LessOrEqual(target.clone(), literal(5))),
                Ok(()),
            );
            assert_eq!(
                check!(&form(0, 5) => &lower => &Proposition::LessOrEqual(literal(5), target.clone())),
                Ok(()),
            );
            assert_eq!(
                integer_affine_truth_bounds(&form(0, 5)),
                Ok(vec![
                    Proposition::LessOrEqual(literal(5), target.clone()),
                    Proposition::LessOrEqual(target, literal(5)),
                ]),
            );
        }

        #[test]
        fn affine_bound_mapping_rejects_shape_direction_and_arithmetic_drift() {
            let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
            let root = value(1, integer_type);
            let target = value(2, integer_type);
            let literal = |value| literal(integer_type, value);
            let form = CheckedIntegerAffineForm {
                root: root.clone(),
                target: target.clone(),
                integer_type,
                coefficient: 2,
                offset: 1,
                endpoint_steps: vec![
                    CheckedIntegerEndpointStep::Multiply(2),
                    CheckedIntegerEndpointStep::Add(1),
                ],
            };
            assert_eq!(
                check_integer_affine_bound_conversion(
                    &form,
                    &Proposition::Equal(root.clone(), literal(4)),
                    &Proposition::LessOrEqual(target.clone(), literal(9)),
                ),
                Err(IntegerAffineBoundConversionError::RootBoundNotLessOrEqual),
            );
            assert_eq!(
                check_integer_affine_bound_conversion(
                    &form,
                    &Proposition::LessOrEqual(value(3, integer_type), literal(4)),
                    &Proposition::LessOrEqual(target.clone(), literal(9)),
                ),
                Err(IntegerAffineBoundConversionError::RootBoundMismatch),
            );
            assert_eq!(
                check_integer_affine_bound_conversion(
                    &form,
                    &Proposition::LessOrEqual(root.clone(), value(3, integer_type)),
                    &Proposition::LessOrEqual(target.clone(), literal(9)),
                ),
                Err(IntegerAffineBoundConversionError::RootBoundNotTypedLiteral),
            );
            assert_eq!(
                check_integer_affine_bound_conversion(
                    &form,
                    &Proposition::LessOrEqual(root.clone(), literal(4)),
                    &Proposition::LessOrEqual(literal(9), target.clone()),
                ),
                Err(IntegerAffineBoundConversionError::ConclusionMismatch),
            );
            assert_eq!(
                check_integer_affine_bound_conversion(
                    &CheckedIntegerAffineForm {
                        endpoint_steps: vec![
                            CheckedIntegerEndpointStep::Multiply(i128::MAX),
                            CheckedIntegerEndpointStep::Add(i128::MAX),
                        ],
                        ..form
                    },
                    &Proposition::LessOrEqual(root, literal(2)),
                    &Proposition::LessOrEqual(target, literal(1)),
                ),
                Err(IntegerAffineBoundConversionError::MappedBoundOverflow),
            );
            assert_eq!(
                check_integer_affine_bound_conversion(
                    &CheckedIntegerAffineForm {
                        root: value(1, integer_type),
                        target: value(2, integer_type),
                        integer_type,
                        coefficient: 2,
                        offset: 1,
                        endpoint_steps: vec![
                            CheckedIntegerEndpointStep::Multiply(2),
                            CheckedIntegerEndpointStep::Add(1),
                        ],
                    },
                    &Proposition::LessOrEqual(value(1, integer_type), literal(100)),
                    &Proposition::LessOrEqual(value(2, integer_type), literal(127)),
                ),
                Err(IntegerAffineBoundConversionError::MappedBoundOutsideCarrier),
            );
        }

        #[test]
        fn maps_mixed_exact_shift_endpoints_and_rejects_witness_tamper() {
            let value_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16");
            let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
            let u16_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16");
            let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
            let root = value(1, value_type);
            let first = value(2, value_type);
            let second = value(3, value_type);
            let target = value(4, value_type);
            let landed_count = value(5, i8_type);
            let integer = |integer_type: IntegerType, value: i128| {
                let value = IntegerValue::for_sign(integer_type.sign(), value).unwrap();
                ScalarTerm::integer(integer_type, value).unwrap()
            };
            let axioms = vec![
                Proposition::Equal(landed_count.clone(), integer(i8_type, 1)),
                Proposition::Equal(
                    first.clone(),
                    ScalarTerm::exact_integer_shift_left(
                        value_type,
                        i8_type,
                        root.clone(),
                        landed_count,
                    )
                    .unwrap(),
                ),
                Proposition::Equal(
                    second.clone(),
                    ScalarTerm::exact_integer_shift_right(
                        value_type,
                        u16_type,
                        first,
                        integer(u16_type, 2),
                    )
                    .unwrap(),
                ),
                Proposition::Equal(
                    target.clone(),
                    ScalarTerm::exact_integer_shift_left(
                        value_type,
                        i32_type,
                        second,
                        integer(i32_type, 3),
                    )
                    .unwrap(),
                ),
            ];
            let context = PropositionContext::from_value_types(
                (1..=4)
                    .map(|id| (ValueId::new(id).unwrap(), ScalarType::Integer(value_type)))
                    .chain([(ValueId::new(5).unwrap(), ScalarType::Integer(i8_type))]),
            )
            .unwrap();
            let witness = affine_witness(
                root.clone(),
                target.clone(),
                vec![1, 2, 3],
                vec![Some(0), None, None],
            );
            let checked = check_integer_affine_witness(&context, &axioms, &witness)
                .expect("ordered mixed shift witness");
            assert_eq!(
                map_integer_affine_bound(
                    &checked,
                    &Proposition::LessOrEqual(root.clone(), integer(value_type, 63)),
                ),
                Ok(Proposition::LessOrEqual(target, integer(value_type, 248),)),
            );
            assert_eq!(
                check_integer_affine_witness(
                    &context,
                    &axioms,
                    &IntegerAffineWitness {
                        definition_axioms: vec![1, 3, 2],
                        ..witness.clone()
                    },
                ),
                Err(IntegerAffineWitnessError::NonCanonicalDefinitionOrder),
            );
            assert_eq!(
                check_integer_affine_witness(
                    &context,
                    &axioms,
                    &IntegerAffineWitness {
                        literal_axioms: vec![None, None, None],
                        ..witness
                    },
                ),
                Err(IntegerAffineWitnessError::ShiftCountNotLanded(1)),
            );
        }

        #[test]
        fn bitwise_and_mask_maps_every_root_endpoint_to_the_mask_image() {
            let integer_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
            let root = value(1, integer_type);
            let target = value(2, integer_type);
            let literal = |value| literal(integer_type, value);
            let context = value_context(ScalarType::Integer(integer_type), 2);
            let definition = Proposition::Equal(
                target.clone(),
                ScalarTerm::integer_bitwise_and(integer_type, root.clone(), literal(15)).unwrap(),
            );
            let witness = affine_witness(root.clone(), target.clone(), vec![0], vec![None]);
            let checked =
                check_integer_affine_witness(&context, std::slice::from_ref(&definition), &witness)
                    .expect("bitwise-and mask witness");

            assert_eq!(
                integer_affine_truth_bounds(&checked),
                Ok(vec![
                    Proposition::LessOrEqual(literal(0), target.clone()),
                    Proposition::LessOrEqual(target.clone(), literal(15)),
                ]),
            );

            assert_eq!(
                map_integer_affine_bound(
                    &checked,
                    &Proposition::LessOrEqual(root.clone(), literal(259)),
                ),
                Ok(Proposition::LessOrEqual(target.clone(), literal(15))),
            );
            assert_eq!(
                map_integer_affine_bound(
                    &checked,
                    &Proposition::LessOrEqual(literal(5), root.clone()),
                ),
                Ok(Proposition::LessOrEqual(literal(0), target.clone())),
            );
            assert_eq!(
                map_integer_affine_bound(
                    &checked,
                    &Proposition::LessThan(root.clone(), literal(259)),
                ),
                Err(IntegerAffineBoundConversionError::StrictBoundNotTranslation),
            );
            assert_eq!(
                check_integer_affine_bound_conversion(
                    &checked,
                    &Proposition::LessOrEqual(root.clone(), literal(259)),
                    &Proposition::LessOrEqual(target.clone(), literal(16)),
                ),
                Err(IntegerAffineBoundConversionError::ConclusionMismatch),
            );
            assert_eq!(
                check_integer_affine_bound_conversion(
                    &checked,
                    &Proposition::LessOrEqual(literal(0), root.clone()),
                    &Proposition::LessOrEqual(literal(1), target.clone()),
                ),
                Err(IntegerAffineBoundConversionError::ConclusionMismatch),
            );

            let widened = value(3, integer_type);
            let context = value_context(ScalarType::Integer(integer_type), 3);
            let axioms = [
                definition,
                Proposition::Equal(
                    widened.clone(),
                    ScalarTerm::exact_integer_subtract(integer_type, target.clone(), literal(3))
                        .unwrap(),
                ),
            ];
            let chained = check_integer_affine_witness(
                &context,
                &axioms,
                &affine_witness(root.clone(), widened.clone(), vec![0, 1], vec![None, None]),
            )
            .expect("mask then subtract chain");
            assert_eq!(
                map_integer_affine_bound(
                    &chained,
                    &Proposition::LessOrEqual(root.clone(), literal(259)),
                ),
                Ok(Proposition::LessOrEqual(widened.clone(), literal(12))),
            );
            assert_eq!(
                map_integer_affine_bound(&chained, &Proposition::LessOrEqual(literal(0), root),),
                Ok(Proposition::LessOrEqual(literal(-3), widened)),
            );
        }

        #[test]
        fn saturating_endpoints_cover_every_byte_input_and_reject_plateau_strictness() {
            for sign in [IntegerSign::Signed, IntegerSign::Unsigned] {
                let integer_type = IntegerType::new(sign, 8).unwrap();
                let (minimum, maximum): (i128, i128) = match sign {
                    IntegerSign::Signed => (-128, 127),
                    IntegerSign::Unsigned => (0, 255),
                };
                let integer = |n: i128| match sign {
                    IntegerSign::Signed => literal(integer_type, n),
                    IntegerSign::Unsigned => unsigned_literal(integer_type, n as u128),
                };
                let root = value(1, integer_type);
                let target = value(2, integer_type);
                let context = value_context(ScalarType::Integer(integer_type), 3);
                for amount in [minimum, 1, maximum] {
                    for operation in 0..3 {
                        let expression = match operation {
                            0 => ScalarTerm::saturating_integer_add(
                                integer_type,
                                root.clone(),
                                integer(amount),
                            ),
                            1 => ScalarTerm::saturating_integer_add(
                                integer_type,
                                integer(amount),
                                root.clone(),
                            ),
                            _ => ScalarTerm::saturating_integer_subtract(
                                integer_type,
                                root.clone(),
                                integer(amount),
                            ),
                        }
                        .unwrap();
                        for inline in [false, true] {
                            let axioms = if inline {
                                vec![]
                            } else {
                                vec![Proposition::Equal(target.clone(), expression.clone())]
                            };
                            let result = if inline {
                                expression.clone()
                            } else {
                                target.clone()
                            };
                            let witness = IntegerAffineWitness {
                                root: root.clone(),
                                target: result.clone(),
                                definition_axioms: if inline { vec![] } else { vec![0] },
                                literal_axioms: if inline { vec![] } else { vec![None] },
                            };
                            let checked =
                                check_integer_affine_witness(&context, &axioms, &witness).unwrap();
                            let execute = |input: i128| match (sign, operation) {
                                (IntegerSign::Signed, 2) => {
                                    (input as i8).saturating_sub(amount as i8) as i128
                                }
                                (IntegerSign::Signed, _) => {
                                    (input as i8).saturating_add(amount as i8) as i128
                                }
                                (IntegerSign::Unsigned, 2) => {
                                    (input as u8).saturating_sub(amount as u8) as i128
                                }
                                (IntegerSign::Unsigned, _) => {
                                    (input as u8).saturating_add(amount as u8) as i128
                                }
                            };
                            for bound in minimum..=maximum {
                                for upper in [false, true] {
                                    let premise = if upper {
                                        Proposition::LessOrEqual(root.clone(), integer(bound))
                                    } else {
                                        Proposition::LessOrEqual(integer(bound), root.clone())
                                    };
                                    let endpoint = execute(bound);
                                    let conclusion = if upper {
                                        Proposition::LessOrEqual(result.clone(), integer(endpoint))
                                    } else {
                                        Proposition::LessOrEqual(integer(endpoint), result.clone())
                                    };
                                    assert_eq!(
                                        map_integer_affine_bound(&checked, &premise),
                                        Ok(conclusion.clone())
                                    );
                                    assert_eq!(check!(&checked => &premise => &conclusion), Ok(()));
                                    for input in minimum..=maximum {
                                        if upper && input <= bound {
                                            assert!(execute(input) <= endpoint);
                                        }
                                        if !upper && bound <= input {
                                            assert!(endpoint <= execute(input));
                                        }
                                    }
                                }
                            }
                            assert_eq!(
                                map_integer_affine_bound(
                                    &checked,
                                    &Proposition::LessThan(root.clone(), integer(maximum))
                                ),
                                Err(IntegerAffineBoundConversionError::StrictBoundNotTranslation),
                            );
                            assert_eq!(
                                integer_affine_truth_bounds(&checked),
                                Ok(vec![
                                    Proposition::LessOrEqual(
                                        integer(execute(minimum)),
                                        result.clone()
                                    ),
                                    Proposition::LessOrEqual(
                                        result.clone(),
                                        integer(execute(maximum))
                                    ),
                                ])
                            );
                            if !inline {
                                assert!(
                                    check_integer_affine_witness(
                                        &context,
                                        &axioms,
                                        &IntegerAffineWitness {
                                            root: target.clone(),
                                            target: root.clone(),
                                            ..witness.clone()
                                        }
                                    )
                                    .is_err()
                                );
                                let changed = Proposition::Equal(
                                    target.clone(),
                                    ScalarTerm::saturating_integer_add(
                                        integer_type,
                                        value(3, integer_type),
                                        integer(amount),
                                    )
                                    .unwrap(),
                                );
                                assert!(
                                    check_integer_affine_witness(&context, &[changed], &witness)
                                        .is_err()
                                );
                            }
                        }
                    }
                }
            }
        }
    }
    mod witness_checking {
        use super::{literal, unsigned_literal, value};
        use crate::integer_rules::integer_affine::{
            IntegerAffineWitness, IntegerAffineWitnessError, check_integer_affine_witness,
            integer_affine_wrapping_evidence, map_integer_affine_bound,
        };
        use crate::proof::assumption_proof;
        use crate::{ProofNode, ProofRule, check_certificate};
        use crate::{affine_witness, value_context};
        use semantic_vocabulary::IntegerType;
        use semantic_vocabulary::Proposition;
        use semantic_vocabulary::PropositionContext;
        use semantic_vocabulary::ScalarTerm;
        use semantic_vocabulary::ScalarType;
        use semantic_vocabulary::{IntegerSign, ValueId};
        use std::slice::from_ref;

        #[test]
        fn direct_literal_remainder_maps_and_certifies_without_invented_definitions() {
            let integer_type = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
            let root = value(1, integer_type);
            let target = ScalarTerm::exact_integer_remainder(
                integer_type,
                root.clone(),
                unsigned_literal(integer_type, 256),
            )
            .unwrap();
            let context = PropositionContext::from_value_types([(
                ValueId::new(1).unwrap(),
                ScalarType::Integer(integer_type),
            )])
            .unwrap();
            let witness = affine_witness(root.clone(), target.clone(), Vec::new(), Vec::new());
            let form = check_integer_affine_witness(&context, &[], &witness).unwrap();
            let premise = Proposition::LessOrEqual(root, unsigned_literal(integer_type, 57));
            let goal = Proposition::LessOrEqual(target.clone(), unsigned_literal(integer_type, 57));
            assert_eq!(map_integer_affine_bound(&form, &premise), Ok(goal.clone()));
            let proof = ProofNode {
                conclusion: goal.clone(),
                rule: ProofRule::IntegerAffineBound {
                    root_bound: Box::new(assumption_proof(premise.clone(), 0)),
                    witness,
                },
            };
            check_certificate(&context, &goal, from_ref(&premise), &[], &proof)
                .expect("the direct expression bound independently checks");
            let false_goal = Proposition::LessOrEqual(target, unsigned_literal(integer_type, 56));
            let false_proof = ProofNode {
                conclusion: false_goal.clone(),
                ..proof
            };
            assert!(
                check_certificate(&context, &false_goal, &[premise], &[], &false_proof).is_err()
            );
        }

        #[test]
        fn direct_remainder_rejects_zero_divisors_and_unbound_or_mismatched_terms() {
            let integer_type = IntegerType::new(IntegerSign::Signed, 16).unwrap();
            let root = value(1, integer_type);
            let other = value(2, integer_type);
            let context = PropositionContext::from_value_types([
                (ValueId::new(1).unwrap(), ScalarType::Integer(integer_type)),
                (ValueId::new(2).unwrap(), ScalarType::Integer(integer_type)),
            ])
            .unwrap();
            let target = |dividend, divisor| {
                ScalarTerm::exact_integer_remainder(integer_type, dividend, divisor).unwrap()
            };
            let witness = |target| IntegerAffineWitness {
                root: root.clone(),
                target,
                definition_axioms: Vec::new(),
                literal_axioms: Vec::new(),
            };
            assert_eq!(
                check_integer_affine_witness(
                    &context,
                    &[],
                    &witness(target(root.clone(), literal(integer_type, 0))),
                ),
                Err(IntegerAffineWitnessError::ZeroDivisionLiteral),
            );
            assert_eq!(
                check_integer_affine_witness(
                    &context,
                    &[],
                    &witness(target(root.clone(), literal(integer_type, -1))),
                ),
                Err(IntegerAffineWitnessError::EmptyDefinitionChain),
                "a direct signed remainder cannot assume MIN / -1 is representable",
            );
            for divisor in [-256, 256] {
                let direct = witness(target(root.clone(), literal(integer_type, divisor)));
                let form = check_integer_affine_witness(&context, &[], &direct).unwrap();
                assert_eq!(
                    map_integer_affine_bound(
                        &form,
                        &Proposition::LessOrEqual(root.clone(), literal(integer_type, -57)),
                    ),
                    Ok(Proposition::LessOrEqual(
                        direct.target.clone(),
                        literal(integer_type, 0)
                    )),
                );
                let phantom_landing = IntegerAffineWitness {
                    literal_axioms: vec![None],
                    ..direct
                };
                assert!(check_integer_affine_witness(&context, &[], &phantom_landing).is_err());
            }
            for invalid_target in [
                target(other.clone(), literal(integer_type, 256)),
                target(literal(integer_type, 256), root.clone()),
                target(root.clone(), other),
            ] {
                assert!(
                    check_integer_affine_witness(&context, &[], &witness(invalid_target)).is_err()
                );
            }
            let empty_context = PropositionContext::from_value_types([]).unwrap();
            assert!(matches!(
                check_integer_affine_witness(
                    &empty_context,
                    &[],
                    &witness(target(root.clone(), literal(integer_type, 256))),
                ),
                Err(IntegerAffineWitnessError::MalformedProposition(_)),
            ));
        }

        #[test]
        fn wrapping_remainder_retains_landed_divisor_and_carrier_specific_bounds() {
            let integer_type = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
            let root = value(1, integer_type);
            let divisor = value(2, integer_type);
            let target = value(3, integer_type);
            let context = value_context(ScalarType::Integer(integer_type), 3);
            let axioms = [
                Proposition::Equal(divisor.clone(), unsigned_literal(integer_type, 10)),
                Proposition::Equal(
                    target.clone(),
                    ScalarTerm::wrapping_integer_remainder(integer_type, root.clone(), divisor)
                        .unwrap(),
                ),
            ];
            let witness = affine_witness(root.clone(), target.clone(), vec![1], vec![Some(0)]);
            let form = check_integer_affine_witness(&context, &axioms, &witness).unwrap();
            for (input_bound, expected) in [(100, 9), (7, 7), (0, 0)] {
                let premise = Proposition::LessOrEqual(
                    root.clone(),
                    unsigned_literal(integer_type, input_bound),
                );
                let goal = Proposition::LessOrEqual(
                    target.clone(),
                    unsigned_literal(integer_type, expected),
                );
                assert_eq!(map_integer_affine_bound(&form, &premise), Ok(goal.clone()));
                let proof = ProofNode {
                    conclusion: goal.clone(),
                    rule: ProofRule::IntegerAffineBound {
                        root_bound: Box::new(assumption_proof(premise.clone(), 0)),
                        witness: witness.clone(),
                    },
                };
                check_certificate(&context, &goal, from_ref(&premise), &axioms, &proof).expect(
                    "unsigned wrapping remainder bounds retain the checked literal landing",
                );
                if expected > 0 {
                    let false_goal = Proposition::LessOrEqual(
                        target.clone(),
                        unsigned_literal(integer_type, expected - 1),
                    );
                    let false_proof = ProofNode {
                        conclusion: false_goal.clone(),
                        ..proof
                    };
                    assert!(
                        check_certificate(&context, &false_goal, &[premise], &axioms, &false_proof)
                            .is_err()
                    );
                }
            }
            let mut zero_divisor = axioms.clone();
            zero_divisor[0] =
                Proposition::Equal(value(2, integer_type), unsigned_literal(integer_type, 0));
            assert_eq!(
                check_integer_affine_witness(&context, &zero_divisor, &witness),
                Err(IntegerAffineWitnessError::ZeroDivisionLiteral),
            );
            let missing_landing = IntegerAffineWitness {
                literal_axioms: vec![None],
                ..witness
            };
            assert!(check_integer_affine_witness(&context, &axioms, &missing_landing).is_err());

            let signed = IntegerType::new(IntegerSign::Signed, 16).unwrap();
            let signed_context = PropositionContext::from_value_types([
                (ValueId::new(1).unwrap(), ScalarType::Integer(signed)),
                (ValueId::new(3).unwrap(), ScalarType::Integer(signed)),
            ])
            .unwrap();
            let signed_axiom = Proposition::Equal(
                value(3, signed),
                ScalarTerm::wrapping_integer_remainder(
                    signed,
                    value(1, signed),
                    literal(signed, 10),
                )
                .unwrap(),
            );
            let signed_witness =
                affine_witness(value(1, signed), value(3, signed), vec![0], vec![None]);
            let signed_axioms = [signed_axiom];
            let signed_form =
                check_integer_affine_witness(&signed_context, &signed_axioms, &signed_witness)
                    .unwrap();
            let premise = Proposition::LessOrEqual(literal(signed, -100), value(1, signed));
            let goal = Proposition::LessOrEqual(literal(signed, -9), value(3, signed));
            assert_eq!(
                map_integer_affine_bound(&signed_form, &premise),
                Ok(goal.clone())
            );
            let mut proof = ProofNode {
                conclusion: goal.clone(),
                rule: ProofRule::IntegerAffineBound {
                    root_bound: Box::new(assumption_proof(premise.clone(), 0)),
                    witness: signed_witness,
                },
            };
            check_certificate(
                &signed_context,
                &goal,
                from_ref(&premise),
                &signed_axioms,
                &proof,
            )
            .unwrap();
            proof.conclusion = Proposition::LessOrEqual(literal(signed, 0), value(3, signed));
            assert!(
                check_certificate(
                    &signed_context,
                    &proof.conclusion,
                    &[premise],
                    &signed_axioms,
                    &proof
                )
                .is_err(),
                "a signed remainder cannot inherit the unsigned nonnegative image"
            );
        }

        #[test]
        fn checks_ordered_add_subtract_multiply_normalization() {
            let integer_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16");
            let root = value(1, integer_type);
            let added = value(2, integer_type);
            let subtracted = value(3, integer_type);
            let target = value(4, integer_type);
            let axioms = vec![
                Proposition::Equal(
                    added.clone(),
                    ScalarTerm::exact_integer_add(
                        integer_type,
                        root.clone(),
                        literal(integer_type, 7),
                    )
                    .unwrap(),
                ),
                Proposition::Equal(
                    ScalarTerm::exact_integer_subtract(
                        integer_type,
                        added.clone(),
                        literal(integer_type, 2),
                    )
                    .unwrap(),
                    subtracted.clone(),
                ),
                Proposition::Equal(
                    target.clone(),
                    ScalarTerm::exact_integer_multiply(
                        integer_type,
                        literal(integer_type, -3),
                        subtracted,
                    )
                    .unwrap(),
                ),
            ];
            let context = value_context(ScalarType::Integer(integer_type), 4);
            let checked = check_integer_affine_witness(
                &context,
                &axioms,
                &affine_witness(
                    root.clone(),
                    target.clone(),
                    vec![0, 1, 2],
                    vec![None, None, None],
                ),
            )
            .expect("ordered affine witness");
            assert_eq!(checked.root(), &root);
            assert_eq!(checked.target(), &target);
            assert_eq!(checked.coefficient(), -3);
            assert_eq!(checked.offset(), -15);
        }

        #[test]
        fn checks_landed_literal_sibling_and_rejects_stale_or_unused_custody() {
            let integer_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16");
            let root = value(1, integer_type);
            let sibling = value(2, integer_type);
            let target = value(3, integer_type);
            let context = value_context(ScalarType::Integer(integer_type), 5);
            let landing = Proposition::Equal(sibling.clone(), literal(integer_type, 7));
            let definition = Proposition::Equal(
                target.clone(),
                ScalarTerm::exact_integer_add(integer_type, root.clone(), sibling.clone()).unwrap(),
            );
            let witness = affine_witness(root.clone(), target.clone(), vec![1], vec![Some(0)]);

            let checked = check_integer_affine_witness(
                &context,
                &[landing.clone(), definition.clone()],
                &witness,
            )
            .expect("an earlier exact landing supplies the affine sibling literal");
            assert_eq!(checked.coefficient(), 1);
            assert_eq!(checked.offset(), 7);

            let missing_alignment = IntegerAffineWitness {
                literal_axioms: Vec::new(),
                ..witness.clone()
            };
            assert_eq!(
                check_integer_affine_witness(
                    &context,
                    &[landing.clone(), definition.clone()],
                    &missing_alignment,
                ),
                Err(IntegerAffineWitnessError::LiteralAxiomCountMismatch),
            );

            let late_landing = IntegerAffineWitness {
                definition_axioms: vec![0],
                literal_axioms: vec![Some(1)],
                ..witness.clone()
            };
            assert_eq!(
                check_integer_affine_witness(&context, &[definition, landing], &late_landing),
                Err(IntegerAffineWitnessError::LiteralAxiomNotPrior {
                    definition: 0,
                    literal: 1,
                }),
            );

            let inline_definition = Proposition::Equal(
                target,
                ScalarTerm::exact_integer_add(integer_type, root, literal(integer_type, 7))
                    .unwrap(),
            );
            assert_eq!(
                check_integer_affine_witness(
                    &context,
                    &[
                        Proposition::Equal(sibling, literal(integer_type, 7)),
                        inline_definition
                    ],
                    &witness,
                ),
                Err(IntegerAffineWitnessError::UnusedLiteralAxiom(1)),
            );
        }

        #[test]
        fn exact_add_backward_maps_bounds_and_rejects_ambiguity() {
            let integer_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16");
            let operand = value(1, integer_type);
            let defined = value(2, integer_type);
            let sibling = value(3, integer_type);
            let context = value_context(ScalarType::Integer(integer_type), 3);
            let landing = Proposition::Equal(sibling.clone(), literal(integer_type, 1));
            let definition = Proposition::Equal(
                defined.clone(),
                ScalarTerm::exact_integer_add(integer_type, operand.clone(), sibling.clone())
                    .unwrap(),
            );
            let checked = check_integer_affine_witness(
                &context,
                &[landing, definition],
                &affine_witness(defined.clone(), operand.clone(), vec![1], vec![Some(0)]),
            )
            .expect("exact add traverses toward its operand");
            assert_eq!(checked.coefficient(), 1);
            assert_eq!(checked.offset(), -1);
            assert_eq!(
                map_integer_affine_bound(
                    &checked,
                    &Proposition::LessThan(defined.clone(), literal(integer_type, 3)),
                ),
                Ok(Proposition::LessThan(
                    operand.clone(),
                    literal(integer_type, 2),
                )),
            );
            assert_eq!(
                map_integer_affine_bound(
                    &checked,
                    &Proposition::LessOrEqual(literal(integer_type, 1), defined.clone()),
                ),
                Ok(Proposition::LessOrEqual(
                    literal(integer_type, 0),
                    operand.clone(),
                )),
            );
            assert_eq!(
                integer_affine_wrapping_evidence(
                    &checked,
                    &Proposition::LessThan(defined.clone(), literal(integer_type, 3)),
                ),
                Ok(Vec::new()),
            );

            let ambiguous = Proposition::Equal(
                defined.clone(),
                ScalarTerm::exact_integer_add(integer_type, operand.clone(), operand.clone())
                    .unwrap(),
            );
            let landed_operand = Proposition::Equal(operand.clone(), literal(integer_type, 4));
            assert_eq!(
                check_integer_affine_witness(
                    &context,
                    &[landed_operand, ambiguous],
                    &affine_witness(defined, operand, vec![1], vec![Some(0)]),
                ),
                Err(IntegerAffineWitnessError::AmbiguousDefinition(1)),
            );
        }

        #[test]
        fn rejects_reordered_stale_and_non_affine_definitions() {
            let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
            let root = value(1, integer_type);
            let target = value(2, integer_type);
            let context = PropositionContext::from_value_types([
                (ValueId::new(1).unwrap(), ScalarType::Integer(integer_type)),
                (ValueId::new(2).unwrap(), ScalarType::Integer(integer_type)),
            ])
            .unwrap();
            let axiom = Proposition::Equal(
                target.clone(),
                ScalarTerm::exact_integer_add(integer_type, root.clone(), literal(integer_type, 1))
                    .unwrap(),
            );
            let witness = |definition_axioms: Vec<usize>| IntegerAffineWitness {
                root: root.clone(),
                target: target.clone(),
                literal_axioms: vec![None; definition_axioms.len()],
                definition_axioms,
            };
            assert_eq!(
                check_integer_affine_witness(&context, from_ref(&axiom), &witness(vec![1])),
                Err(IntegerAffineWitnessError::UnknownSemanticAxiom(1)),
            );
            assert_eq!(
                check_integer_affine_witness(
                    &context,
                    &[axiom.clone(), axiom],
                    &witness(vec![1, 0])
                ),
                Err(IntegerAffineWitnessError::NonCanonicalDefinitionOrder),
            );
            assert_eq!(
                check_integer_affine_witness(
                    &context,
                    &[Proposition::Equal(target.clone(), root.clone())],
                    &witness(vec![0]),
                ),
                Err(IntegerAffineWitnessError::DefinitionShapeMismatch(0)),
            );
        }

        #[test]
        fn rejects_non_value_roots_stale_unsigned_words_and_checked_overflow() {
            let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
            let i8_target = value(1, i8_type);
            let i8_context = PropositionContext::from_value_types([(
                ValueId::new(1).unwrap(),
                ScalarType::Integer(i8_type),
            )])
            .unwrap();
            assert_eq!(
                check_integer_affine_witness(
                    &i8_context,
                    &[Proposition::Equal(
                        i8_target.clone(),
                        ScalarTerm::exact_integer_add(
                            i8_type,
                            literal(i8_type, 0),
                            literal(i8_type, 1),
                        )
                        .unwrap(),
                    )],
                    &affine_witness(literal(i8_type, 0), i8_target, vec![0], vec![None]),
                ),
                Err(IntegerAffineWitnessError::RootNotValue),
            );

            let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
            let u8_root = value(1, u8_type);
            let u8_target = value(2, u8_type);
            let u8_context = PropositionContext::from_value_types([
                (ValueId::new(1).unwrap(), ScalarType::Integer(u8_type)),
                (ValueId::new(2).unwrap(), ScalarType::Integer(u8_type)),
            ])
            .unwrap();
            assert_eq!(
                check_integer_affine_witness(
                    &u8_context,
                    &[],
                    &affine_witness(u8_root, u8_target, vec![0], vec![None]),
                ),
                Err(IntegerAffineWitnessError::UnknownSemanticAxiom(0)),
            );

            let i128_type = IntegerType::new(IntegerSign::Signed, 128).expect("i128");
            let root = value(1, i128_type);
            let intermediate = value(2, i128_type);
            let target = value(3, i128_type);
            let context = value_context(ScalarType::Integer(i128_type), 3);
            let axioms = [
                Proposition::Equal(
                    intermediate.clone(),
                    ScalarTerm::exact_integer_multiply(
                        i128_type,
                        root.clone(),
                        literal(i128_type, i128::MAX),
                    )
                    .unwrap(),
                ),
                Proposition::Equal(
                    target.clone(),
                    ScalarTerm::exact_integer_multiply(
                        i128_type,
                        intermediate,
                        literal(i128_type, 2),
                    )
                    .unwrap(),
                ),
            ];
            assert_eq!(
                check_integer_affine_witness(
                    &context,
                    &axioms,
                    &IntegerAffineWitness {
                        root,
                        target,
                        definition_axioms: vec![0, 1],
                        literal_axioms: vec![None, None],
                    },
                ),
                Err(IntegerAffineWitnessError::CoefficientOverflow),
            );
        }

        #[test]
        fn bitwise_and_mask_witness_rejects_negative_and_unlanded_masks() {
            let integer_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
            let root = value(1, integer_type);
            let sibling = value(2, integer_type);
            let target = value(3, integer_type);
            let context = value_context(ScalarType::Integer(integer_type), 3);
            let witness = || affine_witness(root.clone(), target.clone(), vec![0], vec![None]);

            for mask in [literal(integer_type, -1), literal(integer_type, -256)] {
                for expression in [
                    ScalarTerm::integer_bitwise_and(integer_type, root.clone(), mask.clone())
                        .unwrap(),
                    ScalarTerm::integer_bitwise_and(integer_type, mask.clone(), root.clone())
                        .unwrap(),
                ] {
                    assert_eq!(
                        check_integer_affine_witness(
                            &context,
                            &[Proposition::Equal(target.clone(), expression)],
                            &witness(),
                        ),
                        Err(IntegerAffineWitnessError::NegativeBitwiseAndMask),
                    );
                }
            }

            let unlanded = Proposition::Equal(
                target.clone(),
                ScalarTerm::integer_bitwise_and(integer_type, root.clone(), sibling.clone())
                    .unwrap(),
            );
            assert_eq!(
                check_integer_affine_witness(&context, &[unlanded], &witness()),
                Err(IntegerAffineWitnessError::DefinitionShapeMismatch(0)),
            );

            let landing = Proposition::Equal(sibling, literal(integer_type, 7));
            let definition = Proposition::Equal(
                target.clone(),
                ScalarTerm::integer_bitwise_and(integer_type, root, literal(integer_type, 15))
                    .unwrap(),
            );
            assert_eq!(
                check_integer_affine_witness(
                    &context,
                    &[landing, definition],
                    &IntegerAffineWitness {
                        root: value(1, integer_type),
                        target,
                        definition_axioms: vec![1],
                        literal_axioms: vec![Some(0)],
                    },
                ),
                Err(IntegerAffineWitnessError::UnusedLiteralAxiom(1)),
            );
        }
    }
    mod wrapping_bounds {
        use super::{literal, unsigned_literal, value};
        use crate::integer_rules::integer_affine::{
            IntegerAffineBoundConversionError, IntegerAffineWitnessError,
            check_integer_affine_witness, integer_affine_wrapping_evidence,
            map_integer_affine_bound,
        };
        use crate::{affine_witness, value_context};
        use semantic_vocabulary::IntegerType;
        use semantic_vocabulary::Proposition;
        use semantic_vocabulary::PropositionContext;
        use semantic_vocabulary::ScalarTerm;
        use semantic_vocabulary::ScalarType;
        use semantic_vocabulary::{IntegerSign, ValueId};

        macro_rules! map {
            ($checked:expr => $bound:expr) => {
                map_integer_affine_bound($checked, $bound)
            };
        }

        #[test]
        fn wrapping_product_bounds_require_the_entire_rectangle_to_fit() {
            let signed = [
                (-128, -128),
                (-128, 127),
                (-10, -2),
                (-4, 5),
                (-1, -1),
                (0, 0),
                (1, 9),
                (2, 10),
                (20, 20),
                (127, 127),
            ];
            let unsigned = [
                (0, 0),
                (0, 1),
                (1, 9),
                (2, 10),
                (20, 20),
                (255, 255),
                (0, 255),
            ];
            for (sign, intervals) in [
                (IntegerSign::Signed, signed.as_slice()),
                (IntegerSign::Unsigned, unsigned.as_slice()),
            ] {
                let integer_type = IntegerType::new(sign, 8).unwrap();
                let context = value_context(ScalarType::Integer(integer_type), 2);
                let left = value(1, integer_type);
                let right = value(2, integer_type);
                let product = ScalarTerm::wrapping_integer_multiply(
                    integer_type,
                    left.clone(),
                    right.clone(),
                )
                .unwrap();
                let form = check_integer_affine_witness(
                    &context,
                    &[],
                    &affine_witness(left.clone(), product.clone(), vec![], vec![]),
                )
                .unwrap();
                let integer = |number: i128| {
                    if sign == IntegerSign::Signed {
                        literal(integer_type, number)
                    } else {
                        unsigned_literal(integer_type, number as u128)
                    }
                };
                for &(left_minimum, left_maximum) in intervals {
                    for &(right_minimum, right_maximum) in intervals {
                        let mut fits = true;
                        let mut minimum = i128::MAX;
                        let mut maximum = i128::MIN;
                        for left_value in left_minimum..=left_maximum {
                            for right_value in right_minimum..=right_maximum {
                                let mathematical = left_value * right_value;
                                let stored = if sign == IntegerSign::Signed {
                                    (mathematical as i8) as i128
                                } else {
                                    (mathematical as u8) as i128
                                };
                                fits &= mathematical == stored;
                                minimum = minimum.min(stored);
                                maximum = maximum.max(stored);
                            }
                        }
                        for lower in [true, false] {
                            let mut parts = vec![
                                Proposition::LessOrEqual(integer(left_minimum), left.clone()),
                                Proposition::LessOrEqual(left.clone(), integer(left_maximum)),
                                Proposition::LessOrEqual(integer(right_minimum), right.clone()),
                                Proposition::LessOrEqual(right.clone(), integer(right_maximum)),
                            ];
                            if !lower {
                                parts.swap(0, 1);
                                parts.swap(2, 3);
                            }
                            let mapped = map_integer_affine_bound(
                                &form,
                                &Proposition::Conjunction(parts.clone()),
                            );
                            if !fits {
                                assert_eq!(
                            mapped,
                            Err(IntegerAffineBoundConversionError::DirectMultiplyBoundOverflow)
                        );
                                continue;
                            }
                            assert_eq!(
                                mapped,
                                Ok(if lower {
                                    Proposition::LessOrEqual(integer(minimum), product.clone())
                                } else {
                                    Proposition::LessOrEqual(product.clone(), integer(maximum))
                                })
                            );
                            let mut wrong_operand = parts.clone();
                            wrong_operand[2] = parts[0].clone();
                            assert!(
                                map_integer_affine_bound(
                                    &form,
                                    &Proposition::Conjunction(wrong_operand)
                                )
                                .is_err()
                            );
                            parts.swap(0, 1);
                            assert!(
                                map_integer_affine_bound(&form, &Proposition::Conjunction(parts))
                                    .is_err()
                            );
                        }
                    }
                }
            }
        }

        #[test]
        fn wrapping_add_backward_maps_strict_bounds_with_headroom_evidence() {
            let integer_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16");
            let operand = value(1, integer_type);
            let defined = value(2, integer_type);
            let context = PropositionContext::from_value_types([
                (ValueId::new(1).unwrap(), ScalarType::Integer(integer_type)),
                (ValueId::new(2).unwrap(), ScalarType::Integer(integer_type)),
            ])
            .unwrap();
            let axioms = vec![Proposition::Equal(
                defined.clone(),
                ScalarTerm::wrapping_integer_add(
                    integer_type,
                    operand.clone(),
                    unsigned_literal(integer_type, 1),
                )
                .unwrap(),
            )];
            let checked = check_integer_affine_witness(
                &context,
                &axioms,
                &affine_witness(defined.clone(), operand.clone(), vec![0], vec![None]),
            )
            .expect("wrapping add traverses toward its operand");

            let strict = Proposition::LessThan(defined.clone(), unsigned_literal(integer_type, 2));
            let headroom =
                Proposition::LessOrEqual(operand.clone(), unsigned_literal(integer_type, 65_534));
            assert_eq!(
                integer_affine_wrapping_evidence(&checked, &strict),
                Ok(vec![headroom.clone()]),
            );
            assert_eq!(
                map_integer_affine_bound(&checked, &strict),
                Err(IntegerAffineBoundConversionError::WrappingEvidenceMissing),
            );
            assert_eq!(
                map!(&checked => &Proposition::Conjunction(vec![strict, headroom.clone()])),
                Ok(Proposition::LessThan(
                    operand.clone(),
                    unsigned_literal(integer_type, 1),
                )),
            );

            let upper =
                Proposition::LessOrEqual(defined.clone(), unsigned_literal(integer_type, 2));
            assert_eq!(
                map!(&checked => &Proposition::Conjunction(vec![upper, headroom.clone()])),
                Ok(Proposition::LessOrEqual(
                    operand.clone(),
                    unsigned_literal(integer_type, 1),
                )),
            );

            assert_eq!(
                map_integer_affine_bound(
                    &checked,
                    &Proposition::LessOrEqual(unsigned_literal(integer_type, 1), defined.clone()),
                ),
                Ok(Proposition::LessOrEqual(
                    unsigned_literal(integer_type, 0),
                    operand.clone(),
                )),
            );
            assert_eq!(
                map!(&checked => &Proposition::LessThan(unsigned_literal(integer_type, 1), defined)),
                Ok(Proposition::LessThan(
                    unsigned_literal(integer_type, 0),
                    operand,
                )),
            );
        }

        #[test]
        fn wrapping_add_backward_lands_a_sibling_literal_and_rejects_ambiguity() {
            let integer_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
            let operand = value(1, integer_type);
            let defined = value(2, integer_type);
            let sibling = value(3, integer_type);
            let context = value_context(ScalarType::Integer(integer_type), 4);
            let landing = Proposition::Equal(sibling.clone(), unsigned_literal(integer_type, 3));
            let definition = Proposition::Equal(
                defined.clone(),
                ScalarTerm::wrapping_integer_add(integer_type, operand.clone(), sibling.clone())
                    .unwrap(),
            );
            let checked = check_integer_affine_witness(
                &context,
                &[landing.clone(), definition],
                &affine_witness(defined.clone(), operand.clone(), vec![1], vec![Some(0)]),
            )
            .expect("landed sibling resolves the wrapping operand");
            let upper =
                Proposition::LessOrEqual(defined.clone(), unsigned_literal(integer_type, 10));
            let headroom =
                Proposition::LessOrEqual(operand.clone(), unsigned_literal(integer_type, 252));
            assert_eq!(
                integer_affine_wrapping_evidence(&checked, &upper),
                Ok(vec![headroom.clone()]),
            );
            assert_eq!(
                map_integer_affine_bound(
                    &checked,
                    &Proposition::Conjunction(vec![upper, headroom]),
                ),
                Ok(Proposition::LessOrEqual(
                    operand.clone(),
                    unsigned_literal(integer_type, 7),
                )),
            );

            let ambiguous = Proposition::Equal(
                defined.clone(),
                ScalarTerm::wrapping_integer_add(integer_type, operand.clone(), operand.clone())
                    .unwrap(),
            );
            let landed_operand =
                Proposition::Equal(operand.clone(), unsigned_literal(integer_type, 4));
            assert_eq!(
                check_integer_affine_witness(
                    &context,
                    &[landed_operand, ambiguous],
                    &affine_witness(defined, operand, vec![1], vec![Some(0)]),
                ),
                Err(IntegerAffineWitnessError::AmbiguousDefinition(1)),
            );
        }

        #[test]
        fn wrapping_divide_maps_both_directions_and_rejects_zero() {
            let integer_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
            let root = value(1, integer_type);
            let target = value(2, integer_type);
            let context = PropositionContext::from_value_types([
                (ValueId::new(1).unwrap(), ScalarType::Integer(integer_type)),
                (ValueId::new(2).unwrap(), ScalarType::Integer(integer_type)),
            ])
            .unwrap();
            let axioms = vec![Proposition::Equal(
                target.clone(),
                ScalarTerm::wrapping_integer_divide(
                    integer_type,
                    root.clone(),
                    unsigned_literal(integer_type, 3),
                )
                .unwrap(),
            )];
            let witness = affine_witness(root.clone(), target.clone(), vec![0], vec![None]);
            let checked = check_integer_affine_witness(&context, &axioms, &witness)
                .expect("unsigned wrapping divide traverses forward");
            assert_eq!(
                map_integer_affine_bound(
                    &checked,
                    &Proposition::LessOrEqual(root.clone(), unsigned_literal(integer_type, 9)),
                ),
                Ok(Proposition::LessOrEqual(
                    target.clone(),
                    unsigned_literal(integer_type, 3),
                )),
            );
            assert_eq!(
                map_integer_affine_bound(
                    &checked,
                    &Proposition::LessOrEqual(unsigned_literal(integer_type, 4), root.clone()),
                ),
                Ok(Proposition::LessOrEqual(
                    unsigned_literal(integer_type, 1),
                    target.clone(),
                )),
            );
            assert_eq!(
                map!(&checked => &Proposition::LessThan(root.clone(), unsigned_literal(integer_type, 9))),
                Err(IntegerAffineBoundConversionError::StrictBoundNotTranslation),
            );

            let zero_divisor = vec![Proposition::Equal(
                target.clone(),
                ScalarTerm::wrapping_integer_divide(
                    integer_type,
                    root.clone(),
                    unsigned_literal(integer_type, 0),
                )
                .unwrap(),
            )];
            assert_eq!(
                check_integer_affine_witness(&context, &zero_divisor, &witness),
                Err(IntegerAffineWitnessError::ZeroDivisionLiteral),
            );
        }

        #[test]
        fn signed_wrapping_division_maps_truncating_bounds_except_overflow() {
            let integer_type = IntegerType::new(IntegerSign::Signed, 8).unwrap();
            let root = value(1, integer_type);
            let target = value(2, integer_type);
            let context = PropositionContext::from_value_types([
                (ValueId::new(1).unwrap(), ScalarType::Integer(integer_type)),
                (ValueId::new(2).unwrap(), ScalarType::Integer(integer_type)),
            ])
            .unwrap();
            let witness = affine_witness(root.clone(), target.clone(), vec![0], vec![None]);
            for divisor in [-128, -3, -1, 0, 1, 3, 127] {
                let axioms = [Proposition::Equal(
                    target.clone(),
                    ScalarTerm::wrapping_integer_divide(
                        integer_type,
                        root.clone(),
                        literal(integer_type, divisor),
                    )
                    .unwrap(),
                )];
                let checked = check_integer_affine_witness(&context, &axioms, &witness);
                if matches!(divisor, -1 | 0) {
                    assert!(
                        checked.is_err(),
                        "zero and the MIN/-1 discontinuity are not monotone division"
                    );
                    continue;
                }
                let checked = checked.unwrap();
                for bound in -128..=127 {
                    for lower in [false, true] {
                        let endpoint = literal(integer_type, bound);
                        let premise = if lower {
                            Proposition::LessOrEqual(endpoint, root.clone())
                        } else {
                            Proposition::LessOrEqual(root.clone(), endpoint)
                        };
                        let mapped = literal(integer_type, bound / divisor);
                        let expected = if lower == (divisor > 0) {
                            Proposition::LessOrEqual(mapped, target.clone())
                        } else {
                            Proposition::LessOrEqual(target.clone(), mapped)
                        };
                        assert_eq!(map_integer_affine_bound(&checked, &premise), Ok(expected));
                        for operand in -128..=127 {
                            if (lower && operand >= bound) || (!lower && operand <= bound) {
                                assert!(if lower == (divisor > 0) {
                                    operand / divisor >= bound / divisor
                                } else {
                                    operand / divisor <= bound / divisor
                                });
                            }
                        }
                    }
                }
            }
        }

        #[test]
        fn signed_wrapping_remainder_bounds_include_minimum_by_negative_one() {
            let integer_type = IntegerType::new(IntegerSign::Signed, 8).unwrap();
            let root = value(1, integer_type);
            let target = value(2, integer_type);
            let context = PropositionContext::from_value_types([
                (ValueId::new(1).unwrap(), ScalarType::Integer(integer_type)),
                (ValueId::new(2).unwrap(), ScalarType::Integer(integer_type)),
            ])
            .unwrap();
            let witness = affine_witness(root.clone(), target.clone(), vec![0], vec![None]);
            for divisor in [-128_i128, -3, -1, 0, 1, 3, 127] {
                let axioms = [Proposition::Equal(
                    target.clone(),
                    ScalarTerm::wrapping_integer_remainder(
                        integer_type,
                        root.clone(),
                        literal(integer_type, divisor),
                    )
                    .unwrap(),
                )];
                let checked = check_integer_affine_witness(&context, &axioms, &witness);
                if divisor == 0 {
                    assert_eq!(checked, Err(IntegerAffineWitnessError::ZeroDivisionLiteral));
                    continue;
                }
                let checked = checked.unwrap();
                for bound in -128..=127 {
                    for lower in [false, true] {
                        let premise = if lower {
                            Proposition::LessOrEqual(literal(integer_type, bound), root.clone())
                        } else {
                            Proposition::LessOrEqual(root.clone(), literal(integer_type, bound))
                        };
                        let endpoint = if lower {
                            if bound >= 0 { 0 } else { 1 - divisor.abs() }
                        } else if bound <= 0 {
                            0
                        } else {
                            divisor.abs() - 1
                        };
                        let expected = if lower {
                            Proposition::LessOrEqual(
                                literal(integer_type, endpoint),
                                target.clone(),
                            )
                        } else {
                            Proposition::LessOrEqual(
                                target.clone(),
                                literal(integer_type, endpoint),
                            )
                        };
                        assert_eq!(map_integer_affine_bound(&checked, &premise), Ok(expected));
                        for operand in -128..=127 {
                            if (lower && operand >= bound) || (!lower && operand <= bound) {
                                let remainder = operand % divisor;
                                assert!(if lower {
                                    remainder >= endpoint
                                } else {
                                    remainder <= endpoint
                                });
                            }
                        }
                    }
                }
            }
        }

        fn signed_wrapping_witnesses(
            addend: i128,
        ) -> (
            ScalarTerm,
            ScalarTerm,
            crate::integer_rules::integer_affine::CheckedIntegerAffineForm,
            crate::integer_rules::integer_affine::CheckedIntegerAffineForm,
        ) {
            let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
            let operand = value(1, integer_type);
            let target = value(2, integer_type);
            let context = PropositionContext::from_value_types([
                (ValueId::new(1).unwrap(), ScalarType::Integer(integer_type)),
                (ValueId::new(2).unwrap(), ScalarType::Integer(integer_type)),
            ])
            .unwrap();
            let axioms = vec![Proposition::Equal(
                target.clone(),
                ScalarTerm::wrapping_integer_add(
                    integer_type,
                    operand.clone(),
                    literal(integer_type, addend),
                )
                .unwrap(),
            )];
            let witness = |root: &ScalarTerm, destination: &ScalarTerm| {
                check_integer_affine_witness(
                    &context,
                    &axioms,
                    &affine_witness(root.clone(), destination.clone(), vec![0], vec![None]),
                )
                .expect("signed wrapping add traverses")
            };
            let forward = witness(&operand, &target);
            let backward = witness(&target, &operand);
            (operand, target, forward, backward)
        }

        #[test]
        fn signed_wrapping_add_demands_evidence_on_the_side_its_addend_can_cross() {
            let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
            let bound = |value| literal(integer_type, value);

            let (operand, target, forward, backward) = signed_wrapping_witnesses(1);
            assert_eq!(
                map!(&forward => &Proposition::LessOrEqual(operand.clone(), bound(9))),
                Ok(Proposition::LessOrEqual(target.clone(), bound(10))),
            );
            let lower = Proposition::LessOrEqual(bound(0), operand.clone());
            let headroom = Proposition::LessOrEqual(operand.clone(), bound(126));
            assert_eq!(
                integer_affine_wrapping_evidence(&forward, &lower),
                Ok(vec![headroom.clone()]),
            );
            assert_eq!(
                map_integer_affine_bound(&forward, &lower),
                Err(IntegerAffineBoundConversionError::WrappingEvidenceMissing),
            );
            assert_eq!(
                map!(&forward => &Proposition::Conjunction(vec![lower, headroom.clone()])),
                Ok(Proposition::LessOrEqual(bound(1), target.clone())),
            );
            assert_eq!(
                map!(&backward => &Proposition::LessOrEqual(bound(5), target.clone())),
                Ok(Proposition::LessOrEqual(bound(4), operand.clone())),
            );
            let upper = Proposition::LessOrEqual(target.clone(), bound(5));
            assert_eq!(
                integer_affine_wrapping_evidence(&backward, &upper),
                Ok(vec![headroom]),
            );

            let (operand, target, forward, backward) = signed_wrapping_witnesses(-1);
            assert_eq!(
                map!(&forward => &Proposition::LessOrEqual(bound(2), operand.clone())),
                Ok(Proposition::LessOrEqual(bound(1), target.clone())),
            );
            let upper = Proposition::LessOrEqual(operand.clone(), bound(9));
            let floor = Proposition::LessOrEqual(bound(-127), operand.clone());
            assert_eq!(
                integer_affine_wrapping_evidence(&forward, &upper),
                Ok(vec![floor.clone()]),
            );
            assert_eq!(
                map_integer_affine_bound(&forward, &upper),
                Err(IntegerAffineBoundConversionError::WrappingEvidenceMissing),
            );
            assert_eq!(
                map!(&forward => &Proposition::Conjunction(vec![upper, floor.clone()])),
                Ok(Proposition::LessOrEqual(target.clone(), bound(8))),
            );
            assert_eq!(
                map_integer_affine_bound(
                    &forward,
                    &Proposition::Conjunction(vec![
                        Proposition::LessOrEqual(operand.clone(), bound(9)),
                        Proposition::LessOrEqual(operand.clone(), bound(126)),
                    ]),
                ),
                Err(IntegerAffineBoundConversionError::WrappingEvidenceUnexpected),
            );
            assert_eq!(
                map!(&backward => &Proposition::LessOrEqual(target.clone(), bound(5))),
                Ok(Proposition::LessOrEqual(operand.clone(), bound(6))),
            );
            assert_eq!(
                integer_affine_wrapping_evidence(
                    &backward,
                    &Proposition::LessOrEqual(bound(5), target)
                ),
                Ok(vec![floor]),
            );
        }
    }
    mod wrapping_subtraction {
        use super::{literal, unsigned_literal, value};
        use crate::affine_witness;
        use crate::integer_rules::integer_affine::{
            CheckedIntegerAffineForm, IntegerAffineBoundConversionError, IntegerAffineWitness,
            IntegerAffineWitnessError, check_integer_affine_witness,
            integer_affine_wrapping_evidence, map_integer_affine_bound,
        };
        use semantic_vocabulary::{
            IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext, ScalarTerm,
            ScalarType, ValueId,
        };

        fn context(integer_type: IntegerType, values: u64) -> PropositionContext {
            PropositionContext::from_value_types(
                (1..=values)
                    .map(|id| (ValueId::new(id).unwrap(), ScalarType::Integer(integer_type))),
            )
            .unwrap()
        }

        fn typed_literal(integer_type: IntegerType, number: i128) -> ScalarTerm {
            match integer_type.sign() {
                IntegerSign::Signed => literal(integer_type, number),
                IntegerSign::Unsigned => unsigned_literal(integer_type, number as u128),
            }
        }

        fn subtract(integer_type: IntegerType, left: ScalarTerm, right: ScalarTerm) -> ScalarTerm {
            ScalarTerm::wrapping_integer_subtract(integer_type, left, right).unwrap()
        }

        fn add(integer_type: IntegerType, left: ScalarTerm, right: ScalarTerm) -> ScalarTerm {
            ScalarTerm::wrapping_integer_add(integer_type, left, right).unwrap()
        }

        fn subtraction_row(
            integer_type: IntegerType,
            subtrahend: i128,
        ) -> (ScalarTerm, ScalarTerm, CheckedIntegerAffineForm) {
            let operand = value(1, integer_type);
            let defined = value(2, integer_type);
            let axioms = [Proposition::Equal(
                defined.clone(),
                subtract(
                    integer_type,
                    operand.clone(),
                    typed_literal(integer_type, subtrahend),
                ),
            )];
            let form = check_integer_affine_witness(
                &context(integer_type, 2),
                &axioms,
                &affine_witness(operand.clone(), defined.clone(), vec![0], vec![None]),
            )
            .expect("a wrapping difference of a literal is one checked step");
            (operand, defined, form)
        }

        fn holds(relation: &Proposition, operand: &ScalarTerm, x: i128) -> bool {
            let number = |term: &ScalarTerm| match term.integer_value() {
                Some((_, IntegerValue::Signed(value))) => value,
                Some((_, IntegerValue::Unsigned(value))) => value as i128,
                None => panic!("one side of {relation:?} is a literal"),
            };
            match relation {
                Proposition::LessOrEqual(left, right) if left == operand => x <= number(right),
                Proposition::LessOrEqual(left, right) if right == operand => number(left) <= x,
                Proposition::LessThan(left, right) if left == operand => x < number(right),
                Proposition::LessThan(left, right) if right == operand => number(left) < x,
                other => panic!("unexpected relation {other:?}"),
            }
        }

        fn assert_subtraction_mapping_is_sound(integer_type: IntegerType, subtrahends: &[i128]) {
            let (minimum, maximum) = match integer_type.sign() {
                IntegerSign::Signed => (-128_i128, 127_i128),
                IntegerSign::Unsigned => (0, 255),
            };
            let wrap = |number: i128| match integer_type.sign() {
                IntegerSign::Signed => (number as i8) as i128,
                IntegerSign::Unsigned => (number as u8) as i128,
            };
            for &subtrahend in subtrahends {
                let (operand, defined, form) = subtraction_row(integer_type, subtrahend);
                let mut accepted = 0;
                for bound in minimum..=maximum {
                    for (lower, strict) in
                        [(true, false), (false, false), (true, true), (false, true)]
                    {
                        let endpoint = typed_literal(integer_type, bound);
                        let relation = match (lower, strict) {
                            (true, false) => Proposition::LessOrEqual(endpoint, operand.clone()),
                            (false, false) => Proposition::LessOrEqual(operand.clone(), endpoint),
                            (true, true) => Proposition::LessThan(endpoint, operand.clone()),
                            (false, true) => Proposition::LessThan(operand.clone(), endpoint),
                        };
                        let required = integer_affine_wrapping_evidence(&form, &relation)
                            .expect("one literal relation on the operand");
                        let expected_side = (subtrahend > 0 && !lower) || (subtrahend < 0 && lower);
                        assert_eq!(!required.is_empty(), expected_side, "c={subtrahend}");
                        if !required.is_empty() {
                            assert_eq!(
                                map_integer_affine_bound(&form, &relation),
                                Err(IntegerAffineBoundConversionError::WrappingEvidenceMissing),
                                "c={subtrahend} bound={bound}",
                            );
                        }
                        let premise = if required.is_empty() {
                            relation.clone()
                        } else {
                            Proposition::Conjunction(
                                std::iter::once(relation.clone())
                                    .chain(required.iter().cloned())
                                    .collect(),
                            )
                        };
                        let Ok(conclusion) = map_integer_affine_bound(&form, &premise) else {
                            continue;
                        };
                        accepted += 1;
                        for x in minimum..=maximum {
                            if !holds(&relation, &operand, x)
                                || !required.iter().all(|fact| holds(fact, &operand, x))
                            {
                                continue;
                            }
                            let y = wrap(x - subtrahend);
                            assert!(
                                holds(&conclusion, &defined, y),
                                "{premise:?} admits x={x}, whose difference {y} breaks {conclusion:?}",
                            );
                        }
                    }
                }
                assert!(accepted > 0, "c={subtrahend} maps some bound");
            }
        }

        #[test]
        fn unsigned_wrapping_subtraction_maps_every_accepted_bound_soundly() {
            let integer_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
            assert_subtraction_mapping_is_sound(integer_type, &[0, 1, 5, 200, 255]);
        }

        #[test]
        fn signed_wrapping_subtraction_maps_every_accepted_bound_soundly() {
            let integer_type = IntegerType::new(IntegerSign::Signed, 8).unwrap();
            assert_subtraction_mapping_is_sound(integer_type, &[-128, -3, -1, 0, 1, 5, 127]);
        }

        #[test]
        fn unsigned_wrapping_subtraction_demands_exactly_the_minuend_floor() {
            let integer_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
            let bound = |number| unsigned_literal(integer_type, number);
            let (operand, defined, form) = subtraction_row(integer_type, 1);

            let upper = Proposition::LessOrEqual(operand.clone(), bound(10));
            let floor = Proposition::LessOrEqual(bound(1), operand.clone());
            assert_eq!(
                integer_affine_wrapping_evidence(&form, &upper),
                Ok(vec![floor.clone()])
            );
            assert_eq!(
                map_integer_affine_bound(&form, &upper),
                Err(IntegerAffineBoundConversionError::WrappingEvidenceMissing),
            );
            assert_eq!(
                map_integer_affine_bound(
                    &form,
                    &Proposition::Conjunction(vec![upper.clone(), floor.clone()])
                ),
                Ok(Proposition::LessOrEqual(defined.clone(), bound(9))),
            );
            let strict = Proposition::LessThan(operand.clone(), bound(10));
            assert_eq!(
                map_integer_affine_bound(
                    &form,
                    &Proposition::Conjunction(vec![strict, floor.clone()])
                ),
                Ok(Proposition::LessThan(defined.clone(), bound(9))),
            );

            for wrong in [
                Proposition::LessOrEqual(bound(2), operand.clone()),
                Proposition::LessOrEqual(bound(0), operand.clone()),
                Proposition::LessOrEqual(operand.clone(), bound(254)),
                Proposition::LessOrEqual(bound(1), defined.clone()),
            ] {
                assert_eq!(
                    map_integer_affine_bound(
                        &form,
                        &Proposition::Conjunction(vec![upper.clone(), wrong.clone()])
                    ),
                    Err(IntegerAffineBoundConversionError::WrappingEvidenceUnexpected),
                    "{wrong:?}",
                );
            }

            let lower = Proposition::LessOrEqual(bound(3), operand.clone());
            assert_eq!(integer_affine_wrapping_evidence(&form, &lower), Ok(vec![]));
            assert_eq!(
                map_integer_affine_bound(&form, &lower),
                Ok(Proposition::LessOrEqual(bound(2), defined.clone())),
            );
            assert_eq!(
                map_integer_affine_bound(&form, &Proposition::Conjunction(vec![lower, floor])),
                Err(IntegerAffineBoundConversionError::WrappingEvidenceUnexpected),
            );
            assert_eq!(
                map_integer_affine_bound(
                    &form,
                    &Proposition::LessOrEqual(bound(0), operand.clone())
                ),
                Err(IntegerAffineBoundConversionError::MappedBoundOutsideCarrier),
            );
        }

        #[test]
        fn a_wrapping_target_spelled_in_place_is_its_own_definition() {
            let integer_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
            let bound = |number| unsigned_literal(integer_type, number);
            let operand = value(1, integer_type);
            let context = context(integer_type, 2);
            let in_place = |target: ScalarTerm| {
                check_integer_affine_witness(
                    &context,
                    &[],
                    &IntegerAffineWitness {
                        root: operand.clone(),
                        target,
                        definition_axioms: vec![],
                        literal_axioms: vec![],
                    },
                )
            };

            let difference = subtract(integer_type, operand.clone(), bound(1));
            let form = in_place(difference.clone()).expect("an in-place difference checks");
            let upper = Proposition::LessOrEqual(operand.clone(), bound(4));
            let floor = Proposition::LessOrEqual(bound(1), operand.clone());
            assert_eq!(
                map_integer_affine_bound(&form, &upper),
                Err(IntegerAffineBoundConversionError::WrappingEvidenceMissing),
            );
            assert_eq!(
                map_integer_affine_bound(
                    &form,
                    &Proposition::Conjunction(vec![upper.clone(), floor.clone()])
                ),
                Ok(Proposition::LessOrEqual(difference.clone(), bound(3))),
            );
            assert_eq!(
                map_integer_affine_bound(
                    &form,
                    &Proposition::Conjunction(vec![
                        upper.clone(),
                        Proposition::LessOrEqual(bound(1), difference.clone()),
                    ])
                ),
                Err(IntegerAffineBoundConversionError::WrappingEvidenceUnexpected),
            );

            for sum in [
                add(integer_type, operand.clone(), bound(1)),
                add(integer_type, bound(1), operand.clone()),
            ] {
                let form = in_place(sum.clone()).expect("an in-place sum checks");
                assert_eq!(
                    map_integer_affine_bound(&form, &upper),
                    Ok(Proposition::LessOrEqual(sum.clone(), bound(5))),
                );
                let lower = Proposition::LessOrEqual(bound(2), operand.clone());
                assert_eq!(
                    map_integer_affine_bound(&form, &lower),
                    Err(IntegerAffineBoundConversionError::WrappingEvidenceMissing),
                );
                assert_eq!(
                    map_integer_affine_bound(
                        &form,
                        &Proposition::Conjunction(vec![
                            lower,
                            Proposition::LessOrEqual(operand.clone(), bound(254)),
                        ])
                    ),
                    Ok(Proposition::LessOrEqual(bound(3), sum)),
                );
            }
        }

        #[test]
        fn an_in_place_step_follows_the_chain_it_extends() {
            let integer_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
            let bound = |number| unsigned_literal(integer_type, number);
            let operand = value(1, integer_type);
            let defined = value(2, integer_type);
            let context = context(integer_type, 2);
            let axioms = [Proposition::Equal(
                defined.clone(),
                add(integer_type, operand.clone(), bound(2)),
            )];
            let difference = subtract(integer_type, defined.clone(), bound(1));
            let form = check_integer_affine_witness(
                &context,
                &axioms,
                &affine_witness(operand.clone(), difference.clone(), vec![0], vec![None]),
            )
            .expect("an in-place step extends a cited chain");
            let upper = Proposition::LessOrEqual(operand.clone(), bound(10));
            let floor = Proposition::LessOrEqual(bound(1), defined.clone());
            assert_eq!(
                integer_affine_wrapping_evidence(&form, &upper),
                Ok(vec![floor.clone()])
            );
            assert_eq!(
                map_integer_affine_bound(
                    &form,
                    &Proposition::Conjunction(vec![upper.clone(), floor])
                ),
                Ok(Proposition::LessOrEqual(difference, bound(11))),
            );
            assert_eq!(
                map_integer_affine_bound(
                    &form,
                    &Proposition::Conjunction(vec![
                        upper,
                        Proposition::LessOrEqual(bound(1), operand.clone()),
                    ])
                ),
                Err(IntegerAffineBoundConversionError::WrappingEvidenceUnexpected),
            );

            assert_eq!(
                check_integer_affine_witness(
                    &context,
                    &axioms,
                    &affine_witness(
                        operand.clone(),
                        subtract(integer_type, operand, bound(1)),
                        vec![0],
                        vec![None]
                    ),
                ),
                Err(IntegerAffineWitnessError::TargetMismatch),
            );
        }

        #[test]
        fn in_place_targets_outside_one_literal_step_reject() {
            let integer_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
            let wide = IntegerType::new(IntegerSign::Unsigned, 16).unwrap();
            let bound = |number| unsigned_literal(integer_type, number);
            let operand = value(1, integer_type);
            let other = value(2, integer_type);
            let context = PropositionContext::from_value_types([
                (ValueId::new(1).unwrap(), ScalarType::Integer(integer_type)),
                (ValueId::new(2).unwrap(), ScalarType::Integer(integer_type)),
                (ValueId::new(3).unwrap(), ScalarType::Integer(wide)),
            ])
            .unwrap();
            let in_place = |root: &ScalarTerm, target: ScalarTerm| {
                check_integer_affine_witness(
                    &context,
                    &[],
                    &IntegerAffineWitness {
                        root: root.clone(),
                        target,
                        definition_axioms: vec![],
                        literal_axioms: vec![],
                    },
                )
            };
            for target in [
                subtract(integer_type, bound(10), operand.clone()),
                subtract(integer_type, operand.clone(), other.clone()),
                add(integer_type, operand.clone(), other.clone()),
                add(integer_type, operand.clone(), operand.clone()),
                subtract(integer_type, other.clone(), bound(1)),
            ] {
                assert_eq!(
                    in_place(&operand, target.clone()),
                    Err(IntegerAffineWitnessError::EmptyDefinitionChain),
                    "{target:?}",
                );
            }
            let wide_operand = value(3, wide);
            assert_eq!(
                in_place(
                    &operand,
                    subtract(wide, wide_operand, unsigned_literal(wide, 1))
                ),
                Err(IntegerAffineWitnessError::TargetTypeMismatch),
            );
        }
    }
    use semantic_vocabulary::IntegerType;
    use semantic_vocabulary::IntegerValue;
    use semantic_vocabulary::ScalarTerm;

    pub(crate) use crate::integer_rules::value;

    fn literal(integer_type: IntegerType, value: i128) -> ScalarTerm {
        ScalarTerm::integer(integer_type, IntegerValue::Signed(value)).expect("literal")
    }

    fn unsigned_literal(integer_type: IntegerType, value: u128) -> ScalarTerm {
        ScalarTerm::integer(integer_type, IntegerValue::Unsigned(value)).expect("unsigned literal")
    }
}
