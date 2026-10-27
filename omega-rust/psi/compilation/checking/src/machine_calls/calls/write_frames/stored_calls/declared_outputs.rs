//! Public declared outputs retain lifetime debts at completed operand cuts.
//! A declaration supplies neither performed output identity nor pointer coverage;
//! only a complete closed supplier or admitted provider can establish geometry.
use super::*;
use crate::borrow_contracts::{ProviderOutputDestination, resolve_provider_outputs};

#[allow(clippy::too_many_arguments)]
pub(super) fn effects(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    parameters: &[StateParameter],
    arguments: &[SlotArgumentCapture],
    context: &CollectedStatementPrefix,
    symbols: &TopLevelSymbols<'_>,
) -> Option<SlotRelationTransfer> {
    if parameters.len() != arguments.len() {
        return None;
    }
    let relation = resolve_provider_outputs(program, parameters).ok()?;
    let mut effects = Vec::new();
    for output in relation.outputs {
        let ProviderOutputDestination::Parameter(parameter_index) = output.destination;
        let parameter = &parameters[parameter_index];
        let (destination, _) = destination_source(
            program,
            machine,
            parameter,
            &arguments[parameter_index],
            &owner_segments(&output.owner_path)?,
            context,
            symbols,
        )?;
        let sources = super::output_dependency_sources(
            program,
            machine,
            parameters,
            arguments,
            context,
            symbols,
            &output.dependencies,
            Vec::new(),
        )?;
        effects.push(SlotEffect {
            root: destination.root,
            segments: destination.segments,
            sources: Some(sources),
            incoming: false,
            may_select: false,
            kind: stored_origins::StoredOriginKind::Reference(output.access),
        });
    }
    super::complete_applied_output_transfer(context, relation.readiness, effects)
}

pub(super) use super::opaque_outputs::owner_segments;
