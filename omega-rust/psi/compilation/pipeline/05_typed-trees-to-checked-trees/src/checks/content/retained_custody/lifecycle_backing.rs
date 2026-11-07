//! Resolve a sealed lifecycle's backing transfers against its applied types.
//!
//! Geometry equality cannot conserve authority. Each normal outcome must return
//! every input backing once, with the same content qualification. This prepares
//! exact paths; call checking still joins those paths to consumed/live claims.

use super::structural_sources::{SourceSegment, domain_sources};
use facts::PlaceSegment;
use language_semantics::content::ContentProjectionPlan;
use symbols::SymbolHandle;
use typed_trees::signature::StateParameter;
use typed_trees::typed_trees::AccessExecutionDeclaration;
use typed_trees::types::TypeReferenceHandle;

pub(in crate::checks::content) struct BackingTransfer {
    pub parameter: usize,
    pub input: Vec<PlaceSegment>,
    pub output: Vec<PlaceSegment>,
}

pub(in crate::checks::content) fn lifecycle_backing_transfers(
    program: &dyn validation::CheckingInput,
    callable: SymbolHandle,
    parameters: &[StateParameter],
    return_type: TypeReferenceHandle,
    projection: &ContentProjectionPlan,
) -> Option<Vec<Vec<BackingTransfer>>> {
    let declaration = AccessExecutionDeclaration::authenticate(program, callable).ok()??;
    let outcomes = declaration.backing_outcomes();
    if outcomes.is_empty() {
        return None;
    }
    let output = domain_sources(program, return_type);
    let inputs: Vec<_> = parameters
        .iter()
        .map(|parameter| domain_sources(program, parameter.type_reference))
        .collect();
    if !output.complete || inputs.iter().any(|input| !input.complete) {
        return None;
    }
    let same_domain = |source: &&super::structural_sources::ContentSource| {
        !source.borrowed
            && source.domain.symbol == projection.domain
            && source.domain.semantic_domain == projection.semantic_domain
    };
    let outputs: Vec<_> = output.sources.iter().filter(same_domain).collect();
    let inputs: Vec<_> = inputs
        .iter()
        .enumerate()
        .flat_map(|(position, input)| {
            input
                .sources
                .iter()
                .filter(same_domain)
                .map(move |source| (position, source))
        })
        .collect();
    let mut covered = Vec::new();
    let mut resolved = Vec::with_capacity(outcomes.len());
    for outcome in outcomes {
        let mut transfers = Vec::with_capacity(outcome.transfers.len());
        let mut consumed = Vec::new();
        for transfer in outcome.transfers {
            let mut matching_inputs =
                inputs.iter().enumerate().filter(|(_, (position, source))| {
                    *position == transfer.parameter
                        && fields_match(program, &source.path, transfer.input)
                });
            let (input_index, (_, input)) = matching_inputs.next()?;
            if matching_inputs.next().is_some() || consumed.contains(&input_index) {
                return None;
            }
            consumed.push(input_index);
            let mut matching_outputs = outputs.iter().enumerate().filter(|(_, source)| {
                let Some((SourceSegment::Case(case), fields)) = source.path.split_first() else {
                    return false;
                };
                program.symbols.name(*case) == outcome.case
                    && fields_match(program, fields, transfer.output)
            });
            let (output_index, output) = matching_outputs.next()?;
            if matching_outputs.next().is_some() || covered.contains(&output_index) {
                return None;
            }
            covered.push(output_index);
            transfers.push(BackingTransfer {
                parameter: transfer.parameter,
                input: fact_path(&input.path)?,
                output: fact_path(&output.path)?,
            });
        }
        if consumed.len() != inputs.len() {
            return None;
        }
        resolved.push(transfers);
    }
    (covered.len() == outputs.len()).then_some(resolved)
}

fn fields_match(
    program: &dyn validation::CheckingInput,
    path: &[SourceSegment],
    names: &str,
) -> bool {
    let mut names = names.split('.').filter(|name| !name.is_empty());
    path.iter().all(|segment| {
        matches!(segment, SourceSegment::Field(symbol) if names.next() == Some(program.symbols.name(*symbol)))
    }) && names.next().is_none()
}

fn fact_path(path: &[SourceSegment]) -> Option<Vec<PlaceSegment>> {
    path.iter()
        .map(|segment| match segment {
            SourceSegment::Field(symbol) => Some(PlaceSegment::Field { symbol: *symbol }),
            SourceSegment::Case(variant) => Some(PlaceSegment::Case { variant: *variant }),
            _ => None,
        })
        .collect()
}
