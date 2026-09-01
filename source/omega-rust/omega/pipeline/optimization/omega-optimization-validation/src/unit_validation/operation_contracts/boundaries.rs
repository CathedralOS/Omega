use super::*;

pub(crate) fn boundary_requirements_match(
    caller: &PsiOptimizationFunction,
    arguments: &[psi_terminal::StructuralArgument],
    boundary: &psi_terminal::BoundaryMachineDeclaration,
    domains: &BTreeMap<StructuralDomainId, &psi_terminal::StructuralDomainDeclaration>,
) -> bool {
    boundary.requires.windows(2).all(|pair| pair[0] < pair[1])
        && boundary.requires.iter().all(|requirement| {
            domains.contains_key(&requirement.domain)
                && arguments
                    .get(requirement.argument_index as usize)
                    .and_then(|argument| structural_source_contract(caller, argument.place, true))
                    .is_some_and(|source| source.carries_qualification(requirement.domain))
        })
}

pub(crate) fn boundary_completion_matches(
    caller: &PsiOptimizationFunction,
    arguments: &[psi_terminal::StructuralArgument],
    sources: &[omega_abstract_operations::CompletionClaimSource],
    receipts: &[psi_terminal::CompletionReceipt],
) -> bool {
    let mut expected_sources = caller
        .entry_claim_declarations
        .iter()
        .cloned()
        .map(|entry| omega_abstract_operations::CompletionClaimSource {
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
            expected_sources.push(omega_abstract_operations::CompletionClaimSource {
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
    actual.len() == receipts.len()
        && actual == expected
        && receipts.iter().all(|receipt| {
            arguments
                .get(receipt.argument_index as usize)
                .and_then(|argument| {
                    function_claim_input(caller, receipt.claim).map(|(input, path)| {
                        input == argument.place
                            && (argument.path.is_empty() || path == argument.path.as_slice())
                    })
                })
                == Some(true)
        })
}
