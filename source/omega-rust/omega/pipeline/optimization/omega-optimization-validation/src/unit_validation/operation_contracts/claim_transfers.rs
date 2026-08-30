use super::*;

pub(crate) fn validate_internal_claim_transfers(
    caller: &PsiOptimizationFunction,
    callee: &PsiOptimizationFunction,
    arguments: &[psi_terminal::StructuralArgument],
    transfers: &[psi_terminal::ClaimTransfer],
) -> bool {
    for (argument, parameter) in arguments.iter().zip(&callee.structural_parameters) {
        let mut caller_paths = caller
            .entry_claim_declarations
            .iter()
            .filter(|claim| claim.input == argument.place && claim.path.starts_with(&argument.path))
            .map(|claim| &claim.path[argument.path.len()..])
            .collect::<Vec<_>>();
        let mut callee_paths = callee
            .entry_claim_declarations
            .iter()
            .filter(|claim| claim.input == parameter.place)
            .map(|claim| claim.path.as_slice())
            .collect::<Vec<_>>();
        caller_paths.sort();
        callee_paths.sort();
        if caller_paths != callee_paths {
            return false;
        }
        if !argument.path.is_empty()
            && (caller
                .content_entry_claims
                .iter()
                .any(|claim| claim.input.root == argument.place)
                || callee
                    .content_entry_claims
                    .iter()
                    .any(|claim| claim.input.root == parameter.place))
        {
            return false;
        }
        let mut caller_content = caller
            .content_entry_claims
            .iter()
            .filter(|claim| claim.input.root == argument.place)
            .map(|claim| (&claim.input.segments, &claim.projections))
            .collect::<Vec<_>>();
        let mut callee_content = callee
            .content_entry_claims
            .iter()
            .filter(|claim| claim.input.root == parameter.place)
            .map(|claim| (&claim.input.segments, &claim.projections))
            .collect::<Vec<_>>();
        caller_content.sort();
        callee_content.sort();
        if caller_content != callee_content {
            return false;
        }
    }
    let callee_claims = callee
        .entry_claim_declarations
        .iter()
        .map(|claim| (claim.claim, claim.input))
        .chain(
            callee
                .content_entry_claims
                .iter()
                .map(|claim| (claim.claim, claim.input.root)),
        )
        .collect::<BTreeMap<_, _>>();
    if transfers.len() != callee_claims.len()
        || transfers.windows(2).any(|pair| pair[0] >= pair[1])
        || transfers
            .iter()
            .map(|transfer| transfer.claim)
            .collect::<BTreeSet<_>>()
            .len()
            != transfers.len()
    {
        return false;
    }
    for transfer in transfers {
        let Some(argument) = arguments.get(transfer.argument_index as usize) else {
            return false;
        };
        let Some((claim_input, claim_path)) = function_claim_input(caller, transfer.claim) else {
            return false;
        };
        let target_place = callee
            .structural_parameters
            .get(transfer.argument_index as usize)
            .map(|parameter| parameter.place);
        let structural_match = claim_path.starts_with(&argument.path)
            && callee.entry_claim_declarations.iter().any(|claim| {
                Some(claim.input) == target_place && claim.path == claim_path[argument.path.len()..]
            });
        let content_match = argument.path.is_empty()
            && caller
                .content_entry_claims
                .iter()
                .any(|claim| claim.claim == transfer.claim && claim.input.root == argument.place)
            && callee
                .content_entry_claims
                .iter()
                .any(|claim| Some(claim.input.root) == target_place);
        if claim_input != argument.place || (!structural_match && !content_match) {
            return false;
        }
    }
    callee_claims.into_values().all(|input| {
        callee
            .structural_parameters
            .iter()
            .position(|parameter| parameter.place == input)
            .is_some_and(|index| {
                transfers
                    .iter()
                    .any(|transfer| transfer.argument_index as usize == index)
            })
    })
}

pub(crate) fn function_claim_input(
    function: &PsiOptimizationFunction,
    claim: ClaimId,
) -> Option<(PlaceId, &[psi_terminal::StructuralPathSegment])> {
    function
        .entry_claim_declarations
        .iter()
        .find_map(|candidate| {
            (candidate.claim == claim).then_some((candidate.input, candidate.path.as_slice()))
        })
        .or_else(|| {
            function.content_entry_claims.iter().find_map(|candidate| {
                (candidate.claim == claim).then_some((
                    candidate.input.root,
                    &[] as &[psi_terminal::StructuralPathSegment],
                ))
            })
        })
}

pub(crate) fn proposition_structural_roots(proposition: &Proposition) -> BTreeSet<PlaceId> {
    fn scalar_term_roots(term: &ScalarTerm, roots: &mut BTreeSet<PlaceId>) {
        match term {
            ScalarTerm::BooleanField { root, .. } | ScalarTerm::IntegerField { root, .. } => {
                roots.insert(*root);
            }
            ScalarTerm::BooleanNot { operand }
            | ScalarTerm::IntegerBitwiseNot { operand, .. }
            | ScalarTerm::IntegerWiden { operand, .. }
            | ScalarTerm::IntegerExactCast { operand, .. } => scalar_term_roots(operand, roots),
            ScalarTerm::BooleanEqual { left, right }
            | ScalarTerm::IntegerEqual { left, right, .. }
            | ScalarTerm::IntegerLessThan { left, right, .. }
            | ScalarTerm::IntegerLessOrEqual { left, right, .. }
            | ScalarTerm::IntegerBitwiseAnd { left, right, .. }
            | ScalarTerm::IntegerBitwiseOr { left, right, .. }
            | ScalarTerm::IntegerBitwiseXor { left, right, .. }
            | ScalarTerm::ExactIntegerAdd { left, right, .. }
            | ScalarTerm::ExactIntegerSubtract { left, right, .. }
            | ScalarTerm::ExactIntegerMultiply { left, right, .. }
            | ScalarTerm::ExactIntegerDivide { left, right, .. }
            | ScalarTerm::ExactIntegerRemainder { left, right, .. }
            | ScalarTerm::WrappingIntegerDivide { left, right, .. }
            | ScalarTerm::WrappingIntegerRemainder { left, right, .. }
            | ScalarTerm::SaturatingIntegerDivide { left, right, .. }
            | ScalarTerm::SaturatingIntegerRemainder { left, right, .. }
            | ScalarTerm::WrappingIntegerAdd { left, right, .. }
            | ScalarTerm::SaturatingIntegerAdd { left, right, .. }
            | ScalarTerm::WrappingIntegerSubtract { left, right, .. }
            | ScalarTerm::SaturatingIntegerSubtract { left, right, .. }
            | ScalarTerm::WrappingIntegerMultiply { left, right, .. }
            | ScalarTerm::SaturatingIntegerMultiply { left, right, .. } => {
                scalar_term_roots(left, roots);
                scalar_term_roots(right, roots);
            }
            ScalarTerm::WrappingIntegerShiftLeft { value, count, .. }
            | ScalarTerm::WrappingIntegerShiftRight { value, count, .. }
            | ScalarTerm::ExactIntegerShiftLeft { value, count, .. }
            | ScalarTerm::ExactIntegerShiftRight { value, count, .. } => {
                scalar_term_roots(value, roots);
                scalar_term_roots(count, roots);
            }
            ScalarTerm::Value { .. } | ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. } => {}
        }
    }

    fn content_term_roots(term: &ContentTerm, roots: &mut BTreeSet<PlaceId>) {
        match term {
            ContentTerm::Projection { subject, .. } => {
                roots.insert(subject.root);
            }
            ContentTerm::Separate(terms) => {
                for term in terms {
                    content_term_roots(term, roots);
                }
            }
        }
    }

    fn collect(proposition: &Proposition, roots: &mut BTreeSet<PlaceId>) {
        match proposition {
            Proposition::Equal(left, right)
            | Proposition::LessThan(left, right)
            | Proposition::LessOrEqual(left, right) => {
                scalar_term_roots(left, roots);
                scalar_term_roots(right, roots);
            }
            Proposition::IeeeFloatComparison { left, right, .. } => {
                roots.insert(left.root());
                roots.insert(right.root());
            }
            Proposition::ByteSequenceEqual { left, right } => {
                roots.insert(left.root());
                roots.insert(right.root());
            }
            Proposition::StructuralCaseMembership { subject, .. } => {
                roots.insert(subject.root());
            }
            Proposition::ContentConservation(conservation) => {
                content_term_roots(conservation.left(), roots);
                content_term_roots(conservation.right(), roots);
            }
            Proposition::Conjunction(propositions) | Proposition::Disjunction(propositions) => {
                for proposition in propositions {
                    collect(proposition, roots);
                }
            }
            Proposition::Implication {
                premise,
                conclusion,
            } => {
                collect(premise, roots);
                collect(conclusion, roots);
            }
            Proposition::Truth
            | Proposition::Falsehood
            | Proposition::Atom(_)
            | Proposition::IntegerMathEqual(_, _)
            | Proposition::IntegerMathLessThan(_, _)
            | Proposition::IntegerMathLessOrEqual(_, _) => {}
        }
    }

    let mut roots = BTreeSet::new();
    collect(proposition, &mut roots);
    roots
}
