//! Public declared outputs retain lifetime debts at completed operand cuts.
//! A declaration supplies neither performed output identity nor pointer coverage;
//! only a complete closed supplier or admitted provider can establish geometry.
use super::*;
use crate::borrow_contracts::{
    ProviderOutputDestination, ViewReturnInputSource, resolve_provider_outputs,
};

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
        let mut sources = Vec::new();
        for dependency in output.dependencies {
            let parameter = parameters.get(dependency.parameter_index)?;
            if parameter.type_reference != dependency.source_type {
                return None;
            }
            let actual = arguments.get(dependency.parameter_index)?;
            let relative = FrameSourcePlace {
                root: parameter.symbol,
                segments: owner_segments(&dependency.source_path)?,
                builtin_coordinates: true,
                ..Default::default()
            };
            let selected = SlotSource {
                carrier_backing: None,
                origin: FramePlaceOrigin {
                    path: path(program, &relative),
                    precision: FramePathPrecision::Exact,
                    source: relative.clone(),
                },
                carrier: relative.clone(),
                lifetime_access: Some(dependency.access),
            };
            let captured =
                if !super::super::type_reference_is_reference(program, dependency.source_type) {
                    aggregate_argument_sources(
                        program, machine, parameter, actual, &selected, context, symbols,
                    )?
                } else {
                    let root = FrameSourcePlace {
                        root: parameter.symbol,
                        builtin_coordinates: true,
                        ..Default::default()
                    };
                    let captured = aggregate_argument_sources(
                        program,
                        machine,
                        parameter,
                        actual,
                        &SlotSource {
                            carrier_backing: None,
                            origin: FramePlaceOrigin {
                                path: parameter.name.to_string(),
                                precision: FramePathPrecision::Exact,
                                source: root.clone(),
                            },
                            carrier: root,
                            lifetime_access: None,
                        },
                        context,
                        symbols,
                    )?;
                    let mut sources = Vec::new();
                    for captured in captured {
                        if captured.lifetime_access.is_some() {
                            sources.push(captured);
                            continue;
                        }
                        let mut projected = captured.origin.clone();
                        projected.source = projected.source.append_relative(program, &relative);
                        projected.path = path(program, &projected.source);
                        let carrier = captured.carrier.clone();
                        sources.extend(match dependency.source {
                            // This is the outer reference's referent, not a descriptor
                            // stored at the same empty path inside that referent.
                            ViewReturnInputSource::Referent => vec![SlotSource {
                                carrier_backing: None,
                                origin: projected,
                                carrier,
                                lifetime_access: Some(dependency.access),
                            }],
                            ViewReturnInputSource::Stored => {
                                // This dependency reads a descriptor stored inside the
                                // already captured referent. Keep that leaf's own
                                // carrier/backing pair; the outer reference's carrier
                                // is the storage address, not this pointer value.
                                stored_origins::completed_reference_projection_sources(
                                    program,
                                    &captured,
                                    &projected,
                                    &context.stored,
                                )?
                                .into_iter()
                                .map(|mut source| {
                                    source.lifetime_access =
                                        source.lifetime_access.or(Some(dependency.access));
                                    source
                                })
                                .collect()
                            }
                        });
                    }
                    sources
                };
            for source in captured {
                if !sources.contains(&source) {
                    sources.push(source);
                }
            }
        }
        effects.push(SlotEffect {
            root: destination.root,
            segments: destination.segments,
            sources: Some(sources),
            incoming: false,
            may_select: false,
            kind: stored_origins::StoredOriginKind::Reference(output.access),
        });
    }
    if relation.readiness == crate::borrow_contracts::BorrowRelationReadiness::TemplateDependent {
        let mut transfer =
            SlotRelationTransfer::unknown(context, SymbolHandle::invalid(), true, true);
        // The captured destination may not have an incoming inventory row.
        // Its checked coordinate still becomes unknown at this completion.
        for mut effect in effects {
            effect.sources = None;
            if !transfer.effects.iter().any(|prior| {
                prior.root == effect.root
                    && prior.segments == effect.segments
                    && prior.kind == effect.kind
                    && prior.may_select == effect.may_select
            }) {
                transfer.effects.push(effect);
            }
        }
        Some(transfer)
    } else {
        Some(SlotRelationTransfer::complete(effects))
    }
}

pub(super) use super::opaque_outputs::owner_segments;
