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
                semantic_declaration: identities.intern(
                    &checked
                        .normalized_hermetic_symbol_identity(candidate.machine)
                        .map_err(|_| "boundary candidate has no hermetic declaration identity")?,
                ),
                parameter_types: checked
                    .state_parameters(
                        checked
                            .typed
                            .state_by_symbol(candidate.entry)
                            .ok_or("boundary candidate state is absent")?,
                    )
                    .iter()
                    .map(|parameter| {
                        identities.intern(
                            &checked.package_qualified_type_identity_text(parameter.type_reference),
                        )
                    })
                    .collect(),
                callable: identities.intern(
                    &checked
                        .typed
                        .machine_callable_identity_text(
                            checked
                                .machine(candidate.machine)
                                .ok_or("boundary candidate machine is absent")?,
                        )
                        .ok_or("boundary candidate callable identity is absent")?,
                ),
                result_type: identities.intern(
                    &checked.typed.package_qualified_type_identity_text(
                        checked
                            .typed
                            .state_by_symbol(candidate.entry)
                            .ok_or("boundary candidate state is absent")?
                            .return_type,
                    ),
                ),
                machine,
                state,
                contract: candidate.contract,
                specialization,
            },
        });
    }
    Ok(())
}
