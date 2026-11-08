//! Content signature of the exact required input subtree at an installed call.
//!
//! The canonical operation and completion receipt authenticate current custody.
//! Installation joins the required formal to the selected provider's formal;
//! neither historical ingress metadata nor every home of a claim describes
//! this call's membership contract. Claim identity survives moves and admitted
//! qualification introduction, so a later home may carry additional independent
//! content projections. Scanning that history would make a future introduction
//! invalidate an earlier call. This signature join does not establish operand
//! ownership, prove conservation, or authorize qualification erasure.

use terminal_psi::{
    ClaimContentProjection, StructuralParameterDeclaration, StructuralPathQualification,
    StructuralPathSegment, TerminalModule,
};

pub(super) fn projections(
    module: &TerminalModule,
    parameter: &StructuralParameterDeclaration,
    path: &[StructuralPathSegment],
) -> Result<Vec<ClaimContentProjection>, &'static str> {
    let (selected, _) = StructuralPathQualification::select(
        &parameter.qualifications,
        &parameter.projected_qualifications,
        path,
    );
    let mut projections = Vec::new();
    for domain in selected {
        let declaration = module
            .structural_domain(domain)
            .ok_or("required content qualification is absent from the module")?;
        if let Some(projection) = &declaration.content_projection {
            projections.push(ClaimContentProjection {
                projection: projection.identity,
                algebra: projection.algebra.clone(),
            });
        }
    }
    projections.sort();
    projections.dedup();
    Ok(projections)
}
