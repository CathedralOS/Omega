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
                            let mut path = argument.path.clone();
                            path.extend_from_slice(&requirement.path);
                            source.carries_qualification(&path, requirement.domain)
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
    // Sources authenticate historical provenance only. The existing current
    // ownership judgment checks the receipt's exact argument-relative path
    // and the complete live claim roster after intervening moves.
    receipts.iter().all(|receipt| {
        arguments
            .get(receipt.argument_index as usize)
            .is_some_and(|argument| argument.access == terminal_psi::StructuralAccess::Owned)
    })
}
