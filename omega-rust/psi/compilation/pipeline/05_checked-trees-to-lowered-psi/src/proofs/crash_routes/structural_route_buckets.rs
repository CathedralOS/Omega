//! Structural crash route buckets and their root substitution.

use crate::proofs::crash_routes::crash_predicates::{
    boolean_input_budget, charge_boolean_expansion, contains_boolean_connective,
    flatten_checked_boolean_connective,
};
use crate::proofs::crash_routes::scalar_terms::{checked_boolean_scalar_term, checked_scalar_term};
use crate::proofs::crash_routes::structural_members::{
    lower_byte_sequence_field, lower_ieee_float_field, lower_structural_member_term,
    lower_structural_sum_subject,
};
use crate::proofs::{
    CanonicalStructuralPathSegment, CheckedBooleanExpression, CheckedScalarExpression,
    IeeeFloatFormat, LoweringError, PlaceId, PrimitiveType, Proposition, ScalarTerm, ScalarType,
    StructuralParameterDeclaration, StructuralTypeDeclaration, StructuralTypeShape,
    TerminalCrashCause, ValueDeclaration, unsupported,
};

pub(crate) fn lower_structural_crash_route_buckets(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    buckets: &[checked_trees::CrashRouteBucket],
    scalar_parameters: &[ValueDeclaration],
    parameters: &[StructuralParameterDeclaration],
    structural_types: &[StructuralTypeDeclaration],
    runtime_requirements: &[Proposition],
) -> Result<Vec<terminal_psi::CrashRouteBucket>, LoweringError> {
    let terms = RouteTerms {
        scalar_parameters,
        parameters,
        structural_types,
        runtime_requirements,
    };
    buckets
        .iter()
        .map(|bucket| {
            let mut alternatives = bucket
                .alternative_guards()
                .iter()
                .map(|guard| match guard {
                    checked_trees::CrashRouteGuard::Truth => {
                        Ok(terminal_psi::CrashRouteGuard::Truth)
                    }
                    checked_trees::CrashRouteGuard::Predicate(predicate) => {
                        let proposition = if let Some(expression) = predicate.scalar_expression() {
                            let mut remaining = boolean_input_budget(expression)?;
                            terms.lower_proposition(
                                identities,
                                expression,
                                &mut remaining,
                                0,
                            )?
                        } else {
                            let mut path = Vec::new();
                            let parameter_position = predicate
                                .expression()
                                .and_then(|expression| checked_member_path(expression, &mut path))
                                .ok_or(LoweringError::Unsupported(
                                    "structural crash route is outside checked Boolean member lowering",
                                ))?;
                            Proposition::Equal(
                                ScalarTerm::boolean(true),
                                lower_structural_member_term(
                                    identities,
                                    parameter_position,
                                    &path
                                        .into_iter()
                                        .map(
                                            checked_trees::CheckedStructuralPredicatePathSegment::Field,
                                        )
                                        .collect::<Vec<_>>(),
                                    ScalarType::Boolean,
                                    parameters,
                                    structural_types,
                                )?,
                            )
                        };
                        Ok(terminal_psi::CrashRouteGuard::Predicate(
                            terminal_psi::CrashPredicateTerm::new(proposition),
                        ))
                    }
                })
                .collect::<Result<Vec<_>, _>>()?;
            alternatives.sort();
            alternatives.dedup();
            Ok(terminal_psi::CrashRouteBucket {
                cause: match bucket.cause() {
                    checked_trees::CrashCause::Trap => TerminalCrashCause::Trap,
                    checked_trees::CrashCause::Abort => TerminalCrashCause::Abort,
                },
                alternatives,
            })
        })
        .collect()
}

/// Site guards and published routes observe the same invocation namespace.
/// A later state's current parameters are not the values captured by its
/// machine's published predicate.
pub(crate) fn lower_structural_crash_predicates(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    predicates: &[CheckedBooleanExpression],
    scalar_parameters: &[ValueDeclaration],
    parameters: &[StructuralParameterDeclaration],
    structural_types: &[StructuralTypeDeclaration],
    runtime_requirements: &[Proposition],
) -> Result<Vec<terminal_psi::CrashPredicateTerm>, LoweringError> {
    let terms = RouteTerms {
        scalar_parameters,
        parameters,
        structural_types,
        runtime_requirements,
    };
    let mut lowered = predicates
        .iter()
        .map(|predicate| {
            let mut remaining = boolean_input_budget(predicate)?;
            terms
                .lower_proposition(identities, predicate, &mut remaining, 0)
                .map(terminal_psi::CrashPredicateTerm::new)
        })
        .collect::<Result<Vec<_>, _>>()?;
    lowered.sort();
    lowered.dedup();
    Ok(lowered)
}

/// The four rosters every structural crash-route term resolves against.
struct RouteTerms<'a> {
    scalar_parameters: &'a [ValueDeclaration],
    parameters: &'a [StructuralParameterDeclaration],
    structural_types: &'a [StructuralTypeDeclaration],
    runtime_requirements: &'a [Proposition],
}

impl RouteTerms<'_> {
    fn lower_term(
        &self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        expression: &CheckedBooleanExpression,
    ) -> Result<ScalarTerm, LoweringError> {
        match expression {
            CheckedBooleanExpression::Constant(value) => Ok(ScalarTerm::boolean(*value)),
            CheckedBooleanExpression::Parameter { .. } => {
                checked_boolean_scalar_term(identities, expression, self.scalar_parameters, &[])
            }
            CheckedBooleanExpression::StorageRead { .. } => {
                unsupported("crash predicate cannot reconstruct mutable storage")
            }
            CheckedBooleanExpression::StructuralParameterField {
                parameter_position,
                path,
            } => lower_structural_member_term(
                identities,
                *parameter_position,
                path,
                ScalarType::Boolean,
                self.parameters,
                self.structural_types,
            ),
            CheckedBooleanExpression::Not(operand) => {
                ScalarTerm::boolean_not(self.lower_term(identities, operand)?)
                    .map_err(LoweringError::InvalidCrashPredicate)
            }
            CheckedBooleanExpression::Equal { left, right } => ScalarTerm::boolean_equal(
                self.lower_term(identities, left)?,
                self.lower_term(identities, right)?,
            )
            .map_err(LoweringError::InvalidCrashPredicate),
            CheckedBooleanExpression::IntegerMathComparison { .. } => {
                unsupported("mathematical comparison is a proof proposition, not a scalar term")
            }
            CheckedBooleanExpression::IntegerComparison { kind, left, right } => {
                let left = self.lower_scalar_term(identities, left)?;
                let right = self.lower_scalar_term(identities, right)?;
                let Some(integer_type) = left.scalar_type().as_integer() else {
                    return unsupported("structural crash comparison operand is not an integer");
                };
                super::scalar_terms::checked_integer_comparison(*kind, integer_type, left, right)
                    .map_err(LoweringError::InvalidCrashPredicate)
            }
            CheckedBooleanExpression::IeeeFloatComparison { .. } => {
                unsupported("IEEE equality lowers as an atomic proposition")
            }
            CheckedBooleanExpression::ScalarIeeeFloatComparison { .. } => {
                unsupported("scalar IEEE equality lowers as an atomic proposition")
            }
            CheckedBooleanExpression::ByteSequenceEqual { .. } => {
                unsupported("byte-sequence equality lowers as an atomic proposition")
            }
            CheckedBooleanExpression::ByteSequencePredicate { .. } => {
                unsupported("byte-sequence predicate evaluation lowers as an atomic proposition")
            }
            CheckedBooleanExpression::PayloadlessSumEqual { .. } => {
                unsupported("payload-less sum equality lowers through case-membership propositions")
            }
            CheckedBooleanExpression::StructuralCaseMembership { .. } => {
                unsupported("sum membership lowers as an atomic proposition")
            }
            CheckedBooleanExpression::Local { .. }
            | CheckedBooleanExpression::ErasedParameter { .. }
            | CheckedBooleanExpression::And { .. }
            | CheckedBooleanExpression::Or { .. } => {
                unsupported("structural crash route contains an unsupported Boolean term")
            }
        }
    }

    fn lower_scalar_term(
        &self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        expression: &CheckedScalarExpression,
    ) -> Result<ScalarTerm, LoweringError> {
        // Scalar subtrees retain the existing scalar operation and safety
        // policy. Structural observations need the declaration's place/type
        // namespace; they must not redirect unrelated scalar casts or shifts.
        if !uses_structural_storage(expression) {
            return checked_scalar_term(identities, expression, self.scalar_parameters, &[]);
        }
        crate::proofs::integer_predicates::lower(
            expression,
            self.runtime_requirements,
            &mut |expression| match expression {
                expression if !uses_structural_storage(expression) => {
                    checked_scalar_term(identities, expression, self.scalar_parameters, &[])
                }
                CheckedScalarExpression::StructuralParameterField {
                    parameter_position,
                    path,
                    primitive_type,
                } => lower_structural_member_term(
                    identities,
                    *parameter_position,
                    path,
                    crate::emission::scalar_types::terminal_scalar_type(*primitive_type)?,
                    self.parameters,
                    self.structural_types,
                ),
                _ => unsupported(
                    "structural crash integer predicate contains an unsupported operand",
                ),
            },
        )
    }

    fn lower_polarity(
        &self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        expression: &CheckedBooleanExpression,
        positive: bool,
        remaining: &mut usize,
        depth: usize,
    ) -> Result<Proposition, LoweringError> {
        charge_boolean_expansion(remaining, depth)?;
        let mut lower = |operand: &CheckedBooleanExpression, polarity, budget: &mut usize| {
            self.lower_polarity(identities, operand, polarity, budget, depth + 1)
        };
        match expression {
            CheckedBooleanExpression::Not(operand) => lower(operand, !positive, remaining),
            CheckedBooleanExpression::Equal { left, right } => {
                match (left.as_ref(), right.as_ref()) {
                    (CheckedBooleanExpression::Constant(value), operand)
                    | (operand, CheckedBooleanExpression::Constant(value)) => {
                        lower(operand, *value == positive, remaining)
                    }
                    _ if contains_boolean_connective(expression)
                        || contains_structural_atomic_proposition(expression) =>
                    {
                        crate::proofs::contract_predicates::equality_from_polarities(
                            left, right, positive, remaining, lower,
                        )
                    }
                    _ => self.lower_atom_polarity(
                        identities,
                        expression,
                        positive,
                        remaining,
                        depth + 1,
                    ),
                }
            }
            CheckedBooleanExpression::And { left, right }
            | CheckedBooleanExpression::Or { left, right } => {
                crate::proofs::contract_predicates::connective(
                    lower(left, positive, remaining)?,
                    lower(right, positive, remaining)?,
                    matches!(expression, CheckedBooleanExpression::And { .. }) == positive,
                )
            }
            _ => self.lower_atom_polarity(identities, expression, positive, remaining, depth + 1),
        }
    }

    fn lower_atom_polarity(
        &self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        expression: &CheckedBooleanExpression,
        positive: bool,
        remaining: &mut usize,
        depth: usize,
    ) -> Result<Proposition, LoweringError> {
        charge_boolean_expansion(remaining, depth)?;
        if let CheckedBooleanExpression::ScalarIeeeFloatComparison { kind, left, right } =
            expression
        {
            return crate::proofs::contract_predicates::ieee_float_proposition(
                if positive { *kind } else { kind.negated() },
                self.lower_scalar_term(identities, left)?,
                self.lower_scalar_term(identities, right)?,
            );
        }
        if positive || contains_structural_atomic_proposition(expression) {
            let proposition =
                self.lower_proposition(identities, expression, remaining, depth + 1)?;
            return Ok(if positive {
                proposition
            } else {
                Proposition::Implication {
                    premise: Box::new(proposition),
                    conclusion: Box::new(Proposition::Falsehood),
                }
            });
        }
        let term = self.lower_term(identities, expression)?;
        crate::proofs::contract_predicates::canonical_equality(
            ScalarTerm::boolean(true),
            ScalarTerm::boolean_not(term).map_err(LoweringError::InvalidCrashPredicate)?,
        )
    }

    fn lower_proposition(
        &self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,

        expression: &CheckedBooleanExpression,
        remaining: &mut usize,
        depth: usize,
    ) -> Result<Proposition, LoweringError> {
        // IEEE negation includes unordered inputs. Resolve every operand in
        // this route's scalar/structural namespace, then share the comparison
        // meaning with other contracts instead of delegating field leaves to
        // the scalar-only crash reader.
        if matches!(
            expression,
            CheckedBooleanExpression::ScalarIeeeFloatComparison { .. }
        ) {
            return self.lower_atom_polarity(identities, expression, true, remaining, depth);
        }
        if let CheckedBooleanExpression::Not(operand) = expression
            && matches!(
                operand.as_ref(),
                CheckedBooleanExpression::ScalarIeeeFloatComparison { .. }
            )
        {
            return self.lower_atom_polarity(identities, operand, false, remaining, depth);
        }
        charge_boolean_expansion(remaining, depth)?;
        // Existing atomic denotations, including proposition-only negation,
        // remain unchanged. Only composition that cannot be a scalar term
        // expands into logical branches, with one budget for the whole guard.
        if matches!(expression, CheckedBooleanExpression::Equal { .. })
            && (contains_boolean_connective(expression)
                || contains_structural_atomic_proposition(expression))
            || matches!(expression, CheckedBooleanExpression::Not(operand)
                if contains_boolean_connective(operand)
                    && !contains_structural_atomic_proposition(operand))
        {
            return self.lower_polarity(identities, expression, true, remaining, depth + 1);
        }
        if let CheckedBooleanExpression::Not(operand) = expression
            && contains_structural_atomic_proposition(operand)
        {
            return Ok(Proposition::Implication {
                premise: Box::new(self.lower_proposition(
                    identities,
                    operand,
                    remaining,
                    depth + 1,
                )?),
                conclusion: Box::new(Proposition::Falsehood),
            });
        }
        if let CheckedBooleanExpression::IeeeFloatComparison {
            kind,
            primitive_type,
            left,
            right,
        } = expression
        {
            let format = match primitive_type {
                PrimitiveType::F32 => IeeeFloatFormat::Binary32,
                PrimitiveType::F64 => IeeeFloatFormat::Binary64,
                _ => return unsupported("structural IEEE equality has a non-float format"),
            };
            let mut left = lower_ieee_float_field(
                identities,
                left,
                format,
                self.parameters,
                self.structural_types,
            )?;
            let mut right = lower_ieee_float_field(
                identities,
                right,
                format,
                self.parameters,
                self.structural_types,
            )?;
            if kind.is_symmetric() && left > right {
                std::mem::swap(&mut left, &mut right);
            }
            return Ok(Proposition::IeeeFloatComparison {
                kind: *kind,
                format,
                left,
                right,
            });
        }
        if let CheckedBooleanExpression::ByteSequenceEqual { left, right } = expression {
            let left = left.parameter_field().ok_or(LoweringError::Unsupported(
                "byte equality entry fact requires a parameter source",
            ))?;
            let right = right.parameter_field().ok_or(LoweringError::Unsupported(
                "byte equality entry fact requires a parameter source",
            ))?;
            let mut left = lower_byte_sequence_field(
                identities,
                &left,
                self.parameters,
                self.structural_types,
            )?;
            let mut right = lower_byte_sequence_field(
                identities,
                &right,
                self.parameters,
                self.structural_types,
            )?;
            if left > right {
                std::mem::swap(&mut left, &mut right);
            }
            return Ok(Proposition::ByteSequenceEqual { left, right });
        }
        if let CheckedBooleanExpression::StructuralCaseMembership { subject, case } = expression {
            return super::structural_members::lower_structural_case_membership(
                identities,
                subject,
                case,
                self.parameters,
                None,
                self.structural_types,
            );
        }
        if let CheckedBooleanExpression::PayloadlessSumEqual { left, right, cases } = expression {
            let (left, left_type) = lower_structural_sum_subject(
                identities,
                left,
                self.parameters,
                self.structural_types,
            )?;
            let (right, right_type) = lower_structural_sum_subject(
                identities,
                right,
                self.parameters,
                self.structural_types,
            )?;
            if left_type != right_type {
                return unsupported("payload-less sum equality operands have different types");
            }
            if left == right {
                return Ok(Proposition::Truth);
            }
            let StructuralTypeShape::Sum {
                cases: declared_cases,
            } = &terminal_psi::find_structural_type(self.structural_types, left_type)
                .expect("sum subject type was resolved")
                .shape
            else {
                unreachable!("sum subject resolver returned a sum")
            };
            if cases.len() != declared_cases.len()
                || cases.iter().zip(declared_cases).any(|(checked, declared)| {
                    checked.spelling() != identities.text(declared.identity).unwrap_or_default()
                })
            {
                return unsupported("payload-less sum equality case roster was redirected");
            }
            let mut propositions = Vec::with_capacity(cases.len().saturating_mul(2));
            for declared in declared_cases {
                let left_membership = Proposition::StructuralCaseMembership {
                    subject: left.clone(),
                    case: declared.id,
                };
                let right_membership = Proposition::StructuralCaseMembership {
                    subject: right.clone(),
                    case: declared.id,
                };
                propositions.push(Proposition::Implication {
                    premise: Box::new(left_membership.clone()),
                    conclusion: Box::new(right_membership.clone()),
                });
                propositions.push(Proposition::Implication {
                    premise: Box::new(right_membership),
                    conclusion: Box::new(left_membership),
                });
            }
            let mut keyed = propositions
                .into_iter()
                .map(|proposition| {
                    terminal_codec::canonical_proposition_order_key(&proposition)
                        .map(|key| (key, proposition))
                        .map_err(|_| {
                            LoweringError::Unsupported(
                                "payload-less sum equality is not canonically encodable",
                            )
                        })
                })
                .collect::<Result<Vec<_>, _>>()?;
            keyed.sort_by(|left, right| left.0.cmp(&right.0));
            keyed.dedup_by(|left, right| left.0 == right.0);
            let mut propositions = keyed
                .into_iter()
                .map(|(_, proposition)| proposition)
                .collect::<Vec<_>>();
            return Ok(match propositions.len() {
                0 => Proposition::Truth,
                1 => propositions.pop().expect("one proposition"),
                _ => Proposition::Conjunction(propositions),
            });
        }
        if let CheckedBooleanExpression::And { left, right }
        | CheckedBooleanExpression::Or { left, right } = expression
        {
            let conjunction = matches!(expression, CheckedBooleanExpression::And { .. });
            let mut leaves = Vec::new();
            flatten_checked_boolean_connective(left, conjunction, &mut leaves);
            flatten_checked_boolean_connective(right, conjunction, &mut leaves);
            let propositions = leaves
                .into_iter()
                .map(|leaf| self.lower_proposition(identities, leaf, remaining, depth + 1))
                .collect::<Result<Vec<_>, _>>()?;
            let mut flattened = Vec::new();
            for proposition in propositions {
                match proposition {
                    Proposition::Conjunction(nested) if conjunction => flattened.extend(nested),
                    Proposition::Disjunction(nested) if !conjunction => flattened.extend(nested),
                    proposition => flattened.push(proposition),
                }
            }
            let mut keyed = flattened
                .into_iter()
                .map(|proposition| {
                    terminal_codec::canonical_proposition_order_key(&proposition)
                        .map(|key| (key, proposition))
                        .map_err(|_| {
                            LoweringError::Unsupported(
                                "structural crash connective is not canonically encodable",
                            )
                        })
                })
                .collect::<Result<Vec<_>, _>>()?;
            keyed.sort_by(|left, right| left.0.cmp(&right.0));
            keyed.dedup_by(|left, right| left.0 == right.0);
            if keyed.len() == 1 {
                return Ok(keyed.pop().expect("one distinct crash predicate").1);
            }
            let propositions = keyed
                .into_iter()
                .map(|(_, proposition)| proposition)
                .collect();
            return Ok(if conjunction {
                Proposition::Conjunction(propositions)
            } else {
                Proposition::Disjunction(propositions)
            });
        }
        let mut left = ScalarTerm::boolean(true);
        let mut right = self.lower_term(identities, expression)?;
        let order_key = |term: &ScalarTerm| {
            terminal_codec::canonical_scalar_term_order_key(term).map_err(|_| {
                LoweringError::Unsupported("mixed crash Boolean term is not canonically encodable")
            })
        };
        if order_key(&left)? > order_key(&right)? {
            std::mem::swap(&mut left, &mut right);
        }
        Ok(Proposition::Equal(left, right))
    }
}

/// Select the term's leaf namespace from checked operations, independent of
/// caller shape or unused formals. Boolean terms are not integer leaves.
fn uses_structural_storage(expression: &CheckedScalarExpression) -> bool {
    let mut pending = vec![expression];
    while let Some(expression) = pending.pop() {
        match expression {
            CheckedScalarExpression::StructuralParameterField { .. }
            | CheckedScalarExpression::StructuralParameterIndexedRead { .. }
            | CheckedScalarExpression::StructuralSequenceLength { .. }
            | CheckedScalarExpression::Boolean(_) => return true,
            CheckedScalarExpression::IntegerBinary { left, right, .. } => {
                pending.extend([left.as_ref(), right.as_ref()]);
            }
            CheckedScalarExpression::IntegerBitwiseNot { operand, .. }
            | CheckedScalarExpression::IntegerWiden { operand, .. }
            | CheckedScalarExpression::IntegerExactCast { operand, .. }
            | CheckedScalarExpression::RepresentationView { operand, .. } => pending.push(operand),
            CheckedScalarExpression::StorageRead { .. }
            | CheckedScalarExpression::Parameter { .. }
            | CheckedScalarExpression::ErasedParameter { .. }
            | CheckedScalarExpression::Local { .. }
            | CheckedScalarExpression::IntegerLiteral { .. }
            | CheckedScalarExpression::IeeeFloatLiteral { .. }
            | CheckedScalarExpression::ImplementationSelection { .. } => {}
        }
    }
    false
}

fn checked_member_path(
    expression: &checked_trees::CrashPredicateExpression,
    path: &mut Vec<checked_trees::CheckedMemberIdentity>,
) -> Option<u32> {
    match expression {
        checked_trees::CrashPredicateExpression::Parameter(position) => Some(*position),
        checked_trees::CrashPredicateExpression::Member { receiver, member } => {
            let parameter = checked_member_path(receiver, path)?;
            path.push(checked_trees::CheckedMemberIdentity::of_spelling(member));
            Some(parameter)
        }
        _ => None,
    }
}

fn contains_structural_atomic_proposition(expression: &CheckedBooleanExpression) -> bool {
    match expression {
        CheckedBooleanExpression::IeeeFloatComparison { .. }
        | CheckedBooleanExpression::ScalarIeeeFloatComparison { .. }
        | CheckedBooleanExpression::ByteSequenceEqual { .. }
        | CheckedBooleanExpression::ByteSequencePredicate { .. }
        | CheckedBooleanExpression::PayloadlessSumEqual { .. }
        | CheckedBooleanExpression::StructuralCaseMembership { .. } => true,
        CheckedBooleanExpression::Not(operand) => contains_structural_atomic_proposition(operand),
        facts::checked_boolean_triple_pattern!(left, right, ..) => {
            contains_structural_atomic_proposition(left)
                || contains_structural_atomic_proposition(right)
        }
        CheckedBooleanExpression::Constant(_)
        | CheckedBooleanExpression::StorageRead { .. }
        | CheckedBooleanExpression::Parameter { .. }
        | CheckedBooleanExpression::Local { .. }
        | CheckedBooleanExpression::StructuralParameterField { .. }
        | CheckedBooleanExpression::ErasedParameter { .. }
        | CheckedBooleanExpression::IntegerMathComparison { .. }
        | CheckedBooleanExpression::IntegerComparison { .. } => false,
    }
}

pub(crate) fn substitute_structural_crash_route_roots(
    buckets: &mut [terminal_psi::CrashRouteBucket],
    substitutions: &mut dyn FnMut(
        PlaceId,
    ) -> Result<
        Option<(PlaceId, Vec<CanonicalStructuralPathSegment>)>,
        LoweringError,
    >,
) -> Result<(), LoweringError> {
    fn substitute_term(
        term: &mut ScalarTerm,
        substitutions: &mut dyn FnMut(
            PlaceId,
        ) -> Result<
            Option<(PlaceId, Vec<CanonicalStructuralPathSegment>)>,
            LoweringError,
        >,
    ) -> Result<(), LoweringError> {
        match term {
            ScalarTerm::StructuralRead { root, path, .. } => {
                for projection in path.iter_mut() {
                    if let semantic_vocabulary::ScalarReadProjection::Index(selector) = projection {
                        substitute_term(selector, substitutions)?;
                    }
                }
                if let Some((replacement, prefix)) = substitutions(*root)? {
                    *root = replacement;
                    let mut rebased = prefix
                        .into_iter()
                        .map(semantic_vocabulary::ScalarReadProjection::Static)
                        .collect::<Vec<_>>();
                    rebased.append(path);
                    *path = rebased;
                }
            }

            semantic_vocabulary::scalar_term_predicate_quint_pattern!(root, path, ..) => {
                let Some((replacement, prefix)) = substitutions(*root)? else {
                    return Ok(());
                };
                *root = replacement;
                if !prefix.is_empty() {
                    let mut rebased = Vec::with_capacity(prefix.len() + path.len());
                    rebased.extend(prefix);
                    rebased.append(path);
                    *path = rebased;
                }
            }
            semantic_vocabulary::scalar_term_unary_operand!(operand) => {
                substitute_term(operand, substitutions)?
            }
            semantic_vocabulary::scalar_term_binary_operands!(left, right) => {
                substitute_term(left, substitutions)?;
                substitute_term(right, substitutions)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn substitute_proposition(
        proposition: &mut Proposition,
        substitutions: &mut dyn FnMut(
            PlaceId,
        ) -> Result<
            Option<(PlaceId, Vec<CanonicalStructuralPathSegment>)>,
            LoweringError,
        >,
    ) -> Result<(), LoweringError> {
        match proposition {
            Proposition::Equal(left, right) => {
                substitute_term(left, substitutions)?;
                substitute_term(right, substitutions)?;
            }
            Proposition::ScalarIeeeFloatComparison {
                kind, left, right, ..
            } => {
                substitute_term(left, substitutions)?;
                substitute_term(right, substitutions)?;
                if matches!(
                    kind,
                    semantic_vocabulary::IeeeFloatComparisonKind::Equal
                        | semantic_vocabulary::IeeeFloatComparisonKind::NotEqual
                ) && right < left
                {
                    std::mem::swap(left, right);
                }
            }
            Proposition::Conjunction(propositions) | Proposition::Disjunction(propositions) => {
                for proposition in propositions.iter_mut() {
                    substitute_proposition(proposition, substitutions)?;
                }
            }
            Proposition::Implication {
                premise,
                conclusion,
            } => {
                substitute_proposition(premise, substitutions)?;
                substitute_proposition(conclusion, substitutions)?;
            }
            Proposition::IeeeFloatComparison { left, right, .. } => {
                for field in [left, right] {
                    if let Some((root, prefix)) = substitutions(field.root())? {
                        *field = field.rebase(root, &prefix);
                    }
                }
            }
            Proposition::ByteSequenceEqual { left, right } => {
                for field in [left, right] {
                    if let Some((root, prefix)) = substitutions(field.root())? {
                        *field = field.rebase(root, &prefix);
                    }
                }
            }
            Proposition::StructuralCaseMembership { subject, .. } => {
                if let Some((root, prefix)) = substitutions(subject.root())? {
                    *subject = subject.rebase(root, &prefix);
                }
            }
            _ => {}
        }
        Ok(())
    }

    for bucket in buckets {
        for alternative in &mut bucket.alternatives {
            let terminal_psi::CrashRouteGuard::Predicate(predicate) = alternative else {
                continue;
            };
            let mut proposition = predicate.proposition().clone();
            substitute_proposition(&mut proposition, substitutions)?;
            *predicate = terminal_psi::CrashPredicateTerm::new(proposition);
        }
    }
    Ok(())
}
