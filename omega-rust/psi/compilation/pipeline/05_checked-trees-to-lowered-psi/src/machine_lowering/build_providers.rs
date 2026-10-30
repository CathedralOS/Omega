//! Join checked readiness applications to the producer's boundary catalog.
//!
//! The emitted catalog already knows which checked declaration produced each
//! slot. Retain that relationship here; neither runtime names nor catalog
//! order can reconstruct an exact generic/inherited requirement application.

use crate::lowering_error::LoweringError;
use checked_trees::CheckedTrees;
use lowered_psi::{LoweredBuildProviderRequirement, LoweredSourceCallOccurrence};

pub(super) fn retain(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    sources: &[(source::SourceId, [u8; 32])],
    boundaries: &[(
        symbols::SymbolHandle,
        semantic_vocabulary::BoundaryMachineId,
    )],
    calls: &[LoweredSourceCallOccurrence],
) -> Result<Vec<LoweredBuildProviderRequirement>, LoweringError> {
    let mut requirements: Vec<LoweredBuildProviderRequirement> = Vec::new();
    for occurrence in calls {
        let Some(checked_trees::NominalMachineUseSite::Expression(expression)) =
            occurrence.source_site
        else {
            continue;
        };
        let checked_trees::expression::ExpressionNode::Call(call) =
            checked.expression_table.expression(expression)
        else {
            continue;
        };
        let Some(checked_trees::expression::BuildSelectionRequest::ProviderReadiness(request)) =
            checked
                .expression_table
                .build_selection_request(call.build_selection)
        else {
            continue;
        };
        if requirements.iter().any(|requirement| {
            requirement.selection == call.build_selection
                && requirement.source_state == occurrence.source_state
        }) {
            continue;
        }
        let owner = checked
            .machine_holding_state(occurrence.source_state)
            .ok_or(LoweringError::Unsupported(
                "Build readiness lost its source owner",
            ))?;
        let scope = checked
            .build_provider_requirement_scope(request.requirement, owner.symbol)
            .map_err(LoweringError::Unsupported)?;
        let application = scope
            .portable_application_identity(checked)
            .and_then(|identity| checked.type_identity_text(identity))
            .ok_or(LoweringError::Unsupported(
                "Build readiness requirement has no closed portable application identity",
            ))?;
        let declaration =
            super::declarations::nominal(identities, checked, sources, scope.declaration.symbol)
                .map_err(LoweringError::Unsupported)?;
        let mut slots = Vec::new();
        for (source, boundary) in boundaries {
            let selected =
                if let Some(application) = checked.closed_requirement_application(*source) {
                    scope
                        .declaring_scope(checked, application.declaring_trait)
                        .and_then(|declaring| declaring.application_identity(checked))
                        == Some(application.trait_application_identity)
                } else {
                    checked
                        .traits()
                        .iter()
                        .find(|declaration| {
                            checked
                                .trait_machine_signatures(declaration)
                                .iter()
                                .any(|signature| signature.symbol == *source)
                        })
                        .is_some_and(|declaration| {
                            checked.trait_type_parameters(declaration).is_empty()
                                && declaration.lifetime_parameters.is_empty()
                                && scope.declaring_scope(checked, declaration.symbol).is_some()
                        })
                };
            if selected {
                slots.push(*boundary);
            }
        }
        requirements.push(LoweredBuildProviderRequirement {
            selection: call.build_selection,
            source_state: occurrence.source_state,
            declaration,
            application: identities.intern(&application),
            boundaries: slots,
        });
    }
    Ok(requirements)
}
