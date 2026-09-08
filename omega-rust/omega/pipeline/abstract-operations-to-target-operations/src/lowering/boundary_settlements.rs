use super::shared::*;

pub(super) fn claim_completion_only_boundary_is_exact(
    function: &AbstractFunction,
    declaration: &terminal_psi::BoundaryMachineDeclaration,
    scalar_arguments: &[ValueId],
    structural_arguments: &[terminal_psi::StructuralArgument],
    completion_claim_sources: &[CompletionClaimSource],
    completion_receipts: &[terminal_psi::CompletionReceipt],
    parameters_by_place: &BTreeMap<PlaceId, &TargetStructuralParameter>,
) -> bool {
    if !scalar_arguments.is_empty()
        || !declaration.scalar_parameters.is_empty()
        || !declaration.result.is_unit()
        || !declaration.program_local_root_introductions.is_empty()
        || !declaration.content_guarantees.is_empty()
        || !declaration.published_service_ceiling.is_empty()
        || structural_arguments.is_empty()
        || structural_arguments.len() != declaration.structural_parameters.len()
        || declaration.requires.iter().any(|requirement| {
            requirement.argument_index as usize >= declaration.structural_parameters.len()
        })
        || completion_receipts.is_empty()
        || completion_claim_sources
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || completion_receipts
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
    {
        return false;
    }

    for (index, (argument, boundary_parameter)) in structural_arguments
        .iter()
        .zip(&declaration.structural_parameters)
        .enumerate()
    {
        let Some(source) = parameters_by_place.get(&argument.place).copied() else {
            return false;
        };
        let Some(caller_parameter) = function
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == argument.place)
        else {
            return false;
        };
        let mut expected_qualifications = boundary_parameter.qualifications.clone();
        expected_qualifications.extend(
            declaration
                .requires
                .iter()
                .filter(|requirement| requirement.argument_index as usize == index)
                .map(|requirement| requirement.domain),
        );
        expected_qualifications.sort_unstable();
        expected_qualifications.dedup();
        if !argument.path.is_empty()
            || argument.access != terminal_psi::StructuralAccess::Owned
            || source.access != terminal_psi::StructuralAccess::Owned
            || boundary_parameter.access != terminal_psi::StructuralAccess::Owned
            || source.multiplicity != terminal_psi::StructuralMultiplicity::Linear
            || boundary_parameter.multiplicity != terminal_psi::StructuralMultiplicity::Linear
            || boundary_parameter.position != index as u32
            || source.structural_type != boundary_parameter.structural_type
            || caller_parameter.qualifications != expected_qualifications
        {
            return false;
        }
    }

    let canonical_sources = function
        .entry_claims
        .iter()
        .cloned()
        .map(|entry| CompletionClaimSource {
            claim: entry.claim,
            entry: Some(entry),
            content: None,
        })
        .collect::<Vec<_>>();
    if completion_claim_sources != canonical_sources {
        return false;
    }

    let expected = structural_arguments
        .iter()
        .enumerate()
        .flat_map(|(argument_index, argument)| {
            completion_claim_sources.iter().filter_map(move |source| {
                (source.input() == argument.place).then_some((argument_index as u32, source.claim))
            })
        })
        .collect::<BTreeSet<_>>();
    let actual = completion_receipts
        .iter()
        .map(|receipt| (receipt.argument_index, receipt.claim))
        .collect::<BTreeSet<_>>();
    expected == actual && actual.len() == completion_receipts.len()
}
