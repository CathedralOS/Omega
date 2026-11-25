//! Prove the boundary's declared content equation from the selected machine's normal returns.
use std::collections::BTreeMap;

use semantic_vocabulary::Proposition;
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
        let required = crate::substitution::substitute_proposition_structural_places(
            &Proposition::ContentConservation(report.conservation.clone()),
            &substitution,
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

type Places = BTreeMap<
    semantic_vocabulary::PlaceId,
    (
        semantic_vocabulary::PlaceId,
        Vec<semantic_vocabulary::CanonicalStructuralPathSegment>,
    ),
>;

fn formal_places(
    boundary: &BoundaryMachineDeclaration,
    body: &TerminalMachine,
    candidate: &ProviderCandidateConformance,
) -> Result<Places, &'static str> {
    let inputs = &boundary.structural_parameters;
    if candidate.refinement.positional_parameters.len() != inputs.len()
        || body.structural_parameters.len() != inputs.len()
    {
        return Err("content provider mapping does not cover its structural parameters");
    }
    let mut places = BTreeMap::new();
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
        places.insert(inputs[index].place, (actual.place, Vec::new()));
    }
    let (formal, carrier) = boundary
        .structural_contract_result()
        .ok_or("content provider has no structural result")?;
    let actual = body
        .result
        .structural()
        .filter(|result| result.structural_type == carrier)
        .ok_or("content provider result has a different carrier")?;
    places.insert(formal, (actual.place, Vec::new()));
    Ok(places)
}
