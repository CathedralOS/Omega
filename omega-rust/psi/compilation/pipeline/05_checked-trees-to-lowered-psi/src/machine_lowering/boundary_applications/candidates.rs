//! Publish checked body candidates with exact portable declaration identities.
//! Checking owns conformance discovery and specialization; this projection only
//! translates its retained rows into the Terminal vocabulary.

use checked_trees::{CheckedBoundaryOperatorApplicationDemand, CheckedTrees};
use terminal_psi::{
    BoundaryApplication, BoundaryApplicationCandidate, BoundaryBodySpecialization,
    BoundaryCheckedBody, BoundaryNominalIdentity, BoundaryOperatorRequirement,
};

pub(super) fn retain(
    checked: &CheckedTrees,
    sources: &[(source::SourceId, [u8; 32])],
    demand: &CheckedBoundaryOperatorApplicationDemand,
    requirement: &BoundaryOperatorRequirement,
    application: &BoundaryApplication,
    output: &mut Vec<BoundaryApplicationCandidate>,
) -> Result<(), &'static str> {
    let nominal = |symbol| {
        checked
            .exact_owner_nominal_identity(symbol, sources)
            .ok_or("boundary candidate has no exact declaration owner")
            .and_then(|identity| BoundaryNominalIdentity::new(identity.into_string()))
    };
    for candidate in checked
        .facts
        .operators
        .boundary_body_candidates
        .span(demand.body_candidates)
        .ok_or("boundary application has an invalid checked body span")?
    {
        let specialization = candidate
            .specialization
            .map(|(template, commitment)| {
                Ok::<_, &'static str>(BoundaryBodySpecialization {
                    template: nominal(template)?,
                    commitment,
                })
            })
            .transpose()?;
        output.push(BoundaryApplicationCandidate {
            requirement: requirement.clone(),
            application: application.clone(),
            selection_identity: candidate.selection_identity.clone(),
            selection_package: checked.symbols.symbol_package_identity(candidate.selection),
            target: candidate.target.clone(),
            body: BoundaryCheckedBody {
                machine: nominal(candidate.machine)?,
                state: nominal(candidate.entry)?,
                contract: candidate.contract,
                specialization,
            },
        });
    }
    Ok(())
}
