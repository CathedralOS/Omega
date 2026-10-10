//! Publish checked body candidates with exact portable declaration identities.
//! Checking owns conformance discovery and specialization; this projection only
//! translates its retained rows into the Terminal vocabulary.

use checked_trees::{CheckedBoundaryOperatorApplicationDemand, CheckedTrees};
use terminal_psi::{
    BoundaryApplication, BoundaryApplicationCandidate, BoundaryOperatorRequirement,
    CheckedMachineDeclaration, CheckedMachineSpecialization, NominalDeclarationIdentity,
};

pub(super) fn retain(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    sources: &[(source::SourceId, [u8; 32])],
    demand: &CheckedBoundaryOperatorApplicationDemand,
    requirement: &BoundaryOperatorRequirement,
    application: &BoundaryApplication,
    output: &mut Vec<BoundaryApplicationCandidate>,
) -> Result<(), &'static str> {
    let nominal = |identities: &mut terminal_psi::TerminalIdentityTableBuilder, symbol| {
        checked
            .exact_owner_nominal_identity(symbol, sources)
            .ok_or("boundary candidate has no exact declaration owner")
            .map(|identity| {
                NominalDeclarationIdentity::new(
                    identities.intern(&checked.type_identity_text(identity).unwrap_or_default()),
                )
            })
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
                Ok::<_, &'static str>(CheckedMachineSpecialization {
                    template: nominal(identities, template)?,
                    commitment,
                })
            })
            .transpose()?;
        let machine = nominal(identities, candidate.machine)?;
        let state = nominal(identities, candidate.entry)?;
        let selection_identity =
            identities.intern(&checked.callable_identity_text(candidate.selection_identity));
        output.push(BoundaryApplicationCandidate {
            requirement: requirement.clone(),
            application: application.clone(),
            selection_identity,
            selection_package: checked.symbols.symbol_package_identity(candidate.selection),
            target: candidate.target.clone(),
            body: CheckedMachineDeclaration {
                machine,
                state,
                contract: candidate.contract,
                specialization,
            },
        });
    }
    Ok(())
}
