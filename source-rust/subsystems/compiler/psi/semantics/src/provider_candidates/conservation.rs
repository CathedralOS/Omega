//! Prove the boundary's declared content equation from the selected machine's normal returns.
use semantic_vocabulary::{CanonicalStructuralPathSegment, PlaceId, Proposition};
use terminal_psi::{
    BoundaryContentGuarantee, BoundaryMachineDeclaration, ProviderCandidateConformance,
    TerminalMachine, TerminalModule,
};

use crate::content_transformations::{declarations, forwarding};

/// A candidate's declared guarantee is in its own formal namespace. Check the
/// required equation there, then replay the owned input and each actual return.
pub fn verify(
    module: &TerminalModule,
    candidate: &ProviderCandidateConformance,
) -> Result<(), &'static str> {
    let boundary = module
        .boundary_machine(candidate.boundary)
        .ok_or("content provider requirement is missing")?;
    if !boundary
        .content_guarantees
        .iter()
        .any(|row| matches!(row, BoundaryContentGuarantee::Conservation(_)))
    {
        return Ok(());
    }
    let machine = module
        .machine(candidate.candidate)
        .ok_or("content provider body is missing")?;
    declarations::boundary(module, boundary)?;
    let substitution = formal_places(boundary, machine, candidate)?;
    for row in &boundary.content_guarantees {
        let BoundaryContentGuarantee::Conservation(report) = row else {
            continue;
        };
        let required = crate::substitution::substitute_proposition_structural_places_with(
            &Proposition::ContentConservation(report.conservation.clone()),
            &|place| {
                substitution
                    .binary_search_by_key(&place, |(formal, _)| *formal)
                    .ok()
                    .map(|index| &substitution[index].1)
            },
        )?;
        if !machine
            .ensures
            .iter()
            .any(|clause| clause.proposition == required)
        {
            return Err("selected provider does not promise the exact declared content equation");
        }
    }
    forwarding::checked_returns(module, machine).map(|_| ())
}

type FormalPlace = (PlaceId, (PlaceId, Vec<CanonicalStructuralPathSegment>));

fn formal_places(
    boundary: &BoundaryMachineDeclaration,
    body: &TerminalMachine,
    candidate: &ProviderCandidateConformance,
) -> Result<Vec<FormalPlace>, &'static str> {
    let inputs = &boundary.structural_parameters;
    if candidate.refinement.positional_parameters.len() != inputs.len()
        || body.structural_parameters.len() != inputs.len()
    {
        return Err("content provider mapping does not cover its structural parameters");
    }
    let mut places = Vec::with_capacity(inputs.len() + 1);
    for (index, row) in candidate
        .refinement
        .positional_parameters
        .iter()
        .enumerate()
    {
        // Checked satisfies clauses match the authored parameter order. A
        // serialized permutation cannot change which input the provider owes.
        if row.boundary_index as usize != index || row.candidate_index as usize != index {
            return Err("content provider mapping changes a declared structural parameter");
        }
        let actual = &body.structural_parameters[index];
        if actual.structural_type != inputs[index].structural_type
            || actual.position != inputs[index].position
        {
            return Err("content provider input differs from the required carrier or position");
        }
        places.push((inputs[index].place, (actual.place, Vec::new())));
    }
    let (formal, carrier) = boundary
        .structural_contract_result()
        .ok_or("content provider has no structural result")?;
    let actual = body
        .result
        .structural()
        .filter(|result| result.structural_type == carrier)
        .ok_or("content provider result has a different carrier")?;
    places.push((formal, (actual.place, Vec::new())));
    places.sort_unstable_by_key(|(formal, _)| *formal);
    if places.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return Err("content provider repeats a declared structural place");
    }
    Ok(places)
}
