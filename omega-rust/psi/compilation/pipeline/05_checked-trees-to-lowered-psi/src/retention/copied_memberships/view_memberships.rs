//! Introduce universal view contracts from every actual source payload.
use super::*;

pub(super) fn retained(
    module: &TerminalModule,
    machine: &terminal_psi::TerminalMachine,
    operation: &terminal_psi::Operation,
    position: (semantic_vocabulary::BlockId, u32),
    analysis: &mut descriptors::DescriptorAnalysis<'_>,
) -> Option<PayloadMembershipContract> {
    let result = operation.result.structural()?;
    let terminal_psi::StructuralTypeShape::ElementView { element, .. } =
        terminal_psi::find_structural_type(&module.structural_types, result.structural_type)?.shape
    else {
        return None;
    };
    let source = match &operation.kind {
        OperationKind::EstablishElementView { source, .. }
        | OperationKind::EstablishMutableElementView { source, .. }
        | OperationKind::ElementViewSubslice { source, .. } => source,
        _ => return None,
    };
    let evidence = descriptors::element_payload_memberships(
        &module.structural_types,
        &module.structural_domains,
        machine,
        analysis,
        position,
        source,
        element,
    )?;
    let mut projected_qualifications = Vec::new();
    match evidence {
        descriptors::ElementPayloadMemberships::Present(contract) => {
            terminal_psi::StructuralPathQualification::append_under(
                &contract.qualifications,
                &contract.projected_qualifications,
                &[terminal_psi::StructuralPathSegment::AllElements],
                &mut projected_qualifications,
            );
        }
        descriptors::ElementPayloadMemberships::Empty => {
            // These source declarations propose contracts; proved emptiness,
            // not the annotation, authenticates each universal introduction.
            let same_element_carrier = |candidate| {
                terminal_semantics::borrowed_referent_presentation(
                    module.structural_types.iter(),
                    result.structural_type,
                    candidate,
                ) == Some(terminal_semantics::BorrowedReferentPresentation::Unchanged)
            };
            let valid_universal = |row: &&terminal_psi::StructuralPathQualification| {
                matches!(
                    row.path.first(),
                    Some(terminal_psi::StructuralPathSegment::AllElements)
                ) && terminal_semantics::qualification_path_carrier(
                    module.structural_types.iter(),
                    result.structural_type,
                    &row.path,
                )
                .is_some_and(|carrier| {
                    module
                        .structural_domains
                        .iter()
                        .any(|domain| domain.id == row.domain && domain.carrier == carrier)
                })
            };
            for machine in &module.machines {
                for parameter in machine.structural_parameters.iter().chain(
                    machine
                        .blocks
                        .iter()
                        .flat_map(|block| block.structural_parameters.iter()),
                ) {
                    if same_element_carrier(parameter.structural_type) {
                        projected_qualifications.extend(
                            parameter
                                .projected_qualifications
                                .iter()
                                .filter(valid_universal)
                                .cloned(),
                        );
                    }
                }
                if let Some(expected) = machine.result.structural()
                    && same_element_carrier(expected.structural_type)
                {
                    projected_qualifications.extend(
                        expected
                            .projected_qualifications
                            .iter()
                            .filter(valid_universal)
                            .cloned(),
                    );
                }
            }
        }
    }
    projected_qualifications.sort();
    projected_qualifications.dedup();
    Some(PayloadMembershipContract {
        scalar_observations: Vec::new(),
        scalar_requirements: Vec::new(),
        carrier: result.structural_type,
        qualifications: Vec::new(),
        projected_qualifications,
    })
}
