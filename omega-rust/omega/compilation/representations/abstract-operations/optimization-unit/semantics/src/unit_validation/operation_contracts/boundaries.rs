use crate::unit_validation::operation_contracts::claim_transfers::function_claim_input;
use crate::unit_validation::operation_contracts::structural_access::structural_source_contract;
use optimization_unit::PsiOptimizationFunction;
use semantic_vocabulary::StructuralDomainId;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn boundary_requirements_match(
    caller: &PsiOptimizationFunction,
    arguments: &[terminal_psi::StructuralArgument],
    boundary: &terminal_psi::BoundaryMachineDeclaration,
    domains: &BTreeMap<StructuralDomainId, &terminal_psi::StructuralDomainDeclaration>,
) -> bool {
    boundary.requires.windows(2).all(|pair| pair[0] < pair[1])
        && boundary.requires.iter().all(|requirement| {
            domains.contains_key(&requirement.domain)
                && arguments
                    .get(requirement.argument_index as usize)
                    .is_some_and(|argument| {
                        structural_source_contract(caller, argument.place).is_some_and(|source| {
                            source.carries_qualification(&argument.path, requirement.domain)
                        })
                    })
        })
}

pub(crate) fn boundary_completion_matches(
    caller: &PsiOptimizationFunction,
    arguments: &[terminal_psi::StructuralArgument],
    sources: &[abstract_operations::CompletionClaimSource],
    receipts: &[terminal_psi::CompletionReceipt],
) -> bool {
    let mut expected_sources = caller
        .entry_claim_declarations
        .iter()
        .cloned()
        .map(|entry| abstract_operations::CompletionClaimSource {
            claim: entry.claim,
            entry: Some(entry),
            content: None,
        })
        .collect::<Vec<_>>();
    for content in &caller.content_entry_claims {
        if let Some(source) = expected_sources
            .iter_mut()
            .find(|source| source.claim == content.claim)
        {
            source.content = Some(content.clone());
        } else {
            expected_sources.push(abstract_operations::CompletionClaimSource {
                claim: content.claim,
                entry: None,
                content: Some(content.clone()),
            });
        }
    }
    expected_sources.sort();
    if sources != expected_sources
        || receipts.windows(2).any(|pair| pair[0] >= pair[1])
        || receipts
            .iter()
            .map(|receipt| receipt.claim)
            .collect::<BTreeSet<_>>()
            .len()
            != receipts.len()
    {
        return false;
    }
    let expected = arguments
        .iter()
        .enumerate()
        .flat_map(|(index, argument)| {
            caller
                .entry_claim_declarations
                .iter()
                .filter_map(move |claim| {
                    (claim.input == argument.place
                        && (argument.path.is_empty() || claim.path == argument.path))
                        .then_some((index as u32, claim.claim))
                })
                .chain(caller.content_entry_claims.iter().filter_map(move |claim| {
                    (claim.input.root == argument.place).then_some((index as u32, claim.claim))
                }))
        })
        .collect::<BTreeSet<_>>();
    let actual = receipts
        .iter()
        .map(|receipt| (receipt.argument_index, receipt.claim))
        .collect::<BTreeSet<_>>();
    // Every entry or content claim an argument carries in is completed, at
    // its own argument. A receipt may also complete a claim this function
    // established itself; the ownership replay checks that claim is live at
    // the argument's place.
    actual.len() == receipts.len()
        && expected.is_subset(&actual)
        && receipts.iter().all(|receipt| {
            let Some(argument) = arguments.get(receipt.argument_index as usize) else {
                return false;
            };
            match function_claim_input(caller, receipt.claim) {
                Some((input, path)) => {
                    expected.contains(&(receipt.argument_index, receipt.claim))
                        && input == argument.place
                        && (argument.path.is_empty() || path == argument.path.as_slice())
                }
                None => argument.access == terminal_psi::StructuralAccess::Owned,
            }
        })
}
