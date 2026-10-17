//! Content provenance of the exact caller-local claim consumed by a call.
//!
//! The canonical completion receipt binds identity to current custody. A
//! declaration records where that identity entered the machine, which need
//! not be its current home: moves and stores preserve identity. Select the
//! original claim subtree's checked memberships, never a receiving field's
//! annotation or a sibling's content. Construction may publish the same claim
//! at another home, so repeated bindings must agree rather than being guessed
//! from their order. This is a content-signature join, not ownership replay or
//! a proof of partition arithmetic.

use semantic_vocabulary::{ClaimId, StructuralDomainId};
use terminal_psi::{
    ClaimContentProjection, ContentEntryClaim, StructuralPathQualification, StructuralPathSegment,
    TerminalMachine, TerminalModule,
};

pub(super) fn projections(
    module: &TerminalModule,
    caller: &TerminalMachine,
    claim: ClaimId,
    entry_content: Option<&ContentEntryClaim>,
) -> Result<Vec<ClaimContentProjection>, &'static str> {
    // This is historical content identity, not a demand that the consumed
    // argument still occupies its entry place. Psi supplied the exact current
    // argument-relative receipt before provider selection.
    let mut content = entry_content.map(|content| content.projections.clone());
    for binding in &caller.entry_claims {
        if binding.claim != claim {
            continue;
        }
        let parameter = caller
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == binding.input)
            .ok_or("content claim entry parameter is absent")?;
        join(
            module,
            &mut content,
            &parameter.qualifications,
            &parameter.projected_qualifications,
            &binding.path,
        )?;
    }
    for operation in caller.operations() {
        let Some(result) = operation.result.structural() else {
            continue;
        };
        for binding in &result.claims {
            if binding.claim == claim {
                join(
                    module,
                    &mut content,
                    &result.qualifications,
                    &result.projected_qualifications,
                    &binding.path,
                )?;
            }
        }
    }
    for binding in &caller.block_entry_claims {
        if binding.claim != claim {
            continue;
        }
        let parameter = caller
            .blocks
            .iter()
            .flat_map(|block| &block.structural_parameters)
            .find(|parameter| parameter.place == binding.input)
            .ok_or("content claim block parameter is absent")?;
        join(
            module,
            &mut content,
            &parameter.qualifications,
            &parameter.projected_qualifications,
            &binding.path,
        )?;
    }
    content.ok_or("content claim has no retained content-bearing definition")
}

fn join(
    module: &TerminalModule,
    content: &mut Option<Vec<ClaimContentProjection>>,
    qualifications: &[StructuralDomainId],
    projected: &[StructuralPathQualification],
    path: &[StructuralPathSegment],
) -> Result<(), &'static str> {
    let (selected, _) = StructuralPathQualification::select(qualifications, projected, path);
    let mut projections = Vec::new();
    for domain in selected {
        let declaration = module
            .structural_domain(domain)
            .ok_or("content claim qualification is absent from the module")?;
        if let Some(projection) = &declaration.content_projection {
            projections.push(ClaimContentProjection {
                projection: projection.identity,
                algebra: projection.algebra.clone(),
            });
        }
    }
    projections.sort();
    projections.dedup();
    if projections.is_empty() {
        return Err("content claim definition has no exact-subtree content projection");
    }
    if let Some(previous) = content {
        if *previous != projections {
            return Err("content claim bindings disagree on the content projection");
        }
    } else {
        *content = Some(projections);
    }
    Ok(())
}
