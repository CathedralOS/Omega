use optimization_unit::PsiOptimizationFunction;
use semantic_vocabulary::{ContentTerm, PlaceId, Proposition, ScalarTerm};
use std::collections::{BTreeMap, BTreeSet};

/// Project private entry identities into the public argument-relative custody
/// geometry. Identity namespaces never participate in slot agreement.
pub(crate) fn function_entry_claim_paths(
    function: &PsiOptimizationFunction,
) -> Option<Vec<terminal_psi::TerminalDynamicClaimPath>> {
    if !function.content_entry_claims.is_empty() {
        return None;
    }
    let mut paths = Vec::new();
    for claim in &function.entry_claim_declarations {
        let argument_index = function
            .structural_parameters
            .iter()
            .position(|parameter| parameter.place == claim.input)?;
        paths.push(terminal_psi::TerminalDynamicClaimPath {
            argument_index: u32::try_from(argument_index).ok()?,
            path: claim.path.clone(),
        });
    }
    paths.sort();
    if paths.windows(2).any(|pair| pair[0] == pair[1]) {
        return None;
    }
    Some(paths)
}

pub(crate) fn validate_slot_claim_transfers(
    slot: &terminal_psi::TerminalDynamicSlot,
    arguments: &[terminal_psi::StructuralArgument],
    transfers: &[terminal_psi::ClaimTransfer],
) -> bool {
    let paths = transfers
        .iter()
        .map(|transfer| (transfer.argument_index, &transfer.path))
        .collect::<BTreeSet<_>>();
    transfers.windows(2).all(|pair| pair[0] < pair[1])
        && transfers
            .iter()
            .map(|transfer| transfer.claim)
            .collect::<BTreeSet<_>>()
            .len()
            == transfers.len()
        && paths.len() == transfers.len()
        && transfers.iter().all(|transfer| {
            arguments
                .get(transfer.argument_index as usize)
                .is_some_and(|argument| argument.access == terminal_psi::StructuralAccess::Owned)
                && slot.entry_claim_paths.iter().any(|expected| {
                    expected.argument_index == transfer.argument_index
                        && expected.path == transfer.path
                })
        })
        && slot.entry_claim_paths.iter().all(|expected| {
            // Guarded absence needs discriminator evidence in current replay.
            paths.contains(&(expected.argument_index, &expected.path))
                || expected
                    .path
                    .iter()
                    .any(|segment| matches!(segment, terminal_psi::StructuralPathSegment::Case(_)))
        })
}

pub(crate) fn validate_internal_claim_transfers(
    caller: &PsiOptimizationFunction,
    callee: &PsiOptimizationFunction,
    arguments: &[terminal_psi::StructuralArgument],
    transfers: &[terminal_psi::ClaimTransfer],
) -> bool {
    for (argument, parameter) in arguments.iter().zip(&callee.structural_parameters) {
        // Whole owned calls use the current claim frontier, not the original
        // parameter home. Borrowed inputs retain their claim-free restriction.
        if argument.access != terminal_psi::StructuralAccess::Owned {
            let mut caller_paths = caller
                .entry_claim_declarations
                .iter()
                .filter(|claim| {
                    claim.input == argument.place && claim.path.starts_with(&argument.path)
                })
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
        // The current ownership replay authenticates the exact live source
        // and complete callee path roster at this operation.
        let Some(parameter) = callee
            .structural_parameters
            .get(transfer.argument_index as usize)
        else {
            return false;
        };
        if !callee
            .entry_claim_declarations
            .iter()
            .any(|claim| claim.input == parameter.place && claim.path == transfer.path)
            && !(transfer.path.is_empty()
                && callee
                    .content_entry_claims
                    .iter()
                    .any(|claim| claim.input.root == parameter.place))
        {
            return false;
        }
        let _ = argument;
    }
    callee_claims.into_values().all(|input| {
        terminal_psi::structural_parameter_position(&callee.structural_parameters, input)
            .is_some_and(|index| {
                transfers
                    .iter()
                    .any(|transfer| transfer.argument_index as usize == index)
            })
    })
}

pub(crate) fn proposition_structural_roots(proposition: &Proposition) -> BTreeSet<PlaceId> {
    fn scalar_term_roots(term: &ScalarTerm, roots: &mut BTreeSet<PlaceId>) {
        match term {
            ScalarTerm::StructuralRead { root, .. } => {
                roots.insert(*root);
                for operand in term.operands() {
                    scalar_term_roots(operand, roots);
                }
            }
            semantic_vocabulary::scalar_term_predicate_quint_pattern!(root, ..) => {
                roots.insert(*root);
            }
            _ => {
                for operand in term.operands() {
                    scalar_term_roots(operand, roots);
                }
            }
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
            semantic_vocabulary::scalar_comparison_operands!(left, right) => {
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
            Proposition::StructuralIdentity { left, right } => {
                roots.insert(left.root());
                roots.insert(right.root());
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
