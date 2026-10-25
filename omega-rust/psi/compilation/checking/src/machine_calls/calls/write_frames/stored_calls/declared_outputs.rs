//! Capture signature outputs' lifetime dependencies at the actual pre-call cut.
//! A checked body or provider admission separately establishes backing/access.
//! These debts never locate the output inside an input; ordinary public region
//! coverage is published separately by completed-value flow.
use super::*;
use crate::borrow_contracts::{
    ProviderOutputDestination, ViewReturnInputSource, resolve_provider_outputs,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn effects(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    parameters: &[StateParameter],
    completed: &CompletedArguments,
    context: &CollectedStatementPrefix,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
) -> Option<SlotRelationTransfer> {
    let arguments = &completed.expressions;
    if parameters.len() != arguments.len() {
        return None;
    }
    let relation = resolve_provider_outputs(program, parameters).ok()?;
    let mut effects = Vec::new();
    for output in relation.outputs {
        let ProviderOutputDestination::Parameter(parameter_index) = output.destination;
        let parameter = &parameters[parameter_index];
        let destination = destination_source(
            program,
            machine,
            parameter,
            arguments[parameter_index],
            &owner_segments(&output.owner_path)?,
            completed.contexts.get(parameter_index)?,
            context,
            symbols,
            inference,
        )?;
        let mut sources = Vec::new();
        for dependency in output.dependencies {
            let parameter = parameters.get(dependency.parameter_index)?;
            if parameter.type_reference != dependency.source_type {
                return None;
            }
            let actual = *arguments.get(dependency.parameter_index)?;
            let relative = FrameSourcePlace {
                root: parameter.symbol,
                segments: owner_segments(&dependency.source_path)?,
                builtin_coordinates: true,
                ..Default::default()
            };
            let selected = SlotSource {
                origin: FramePlaceOrigin {
                    path: path(program, &relative),
                    precision: FramePathPrecision::Exact,
                    source: relative.clone(),
                },
                carrier: relative.clone(),
                lifetime_access: Some(dependency.access),
            };
            for captured_context in completed.histories.get(dependency.parameter_index)? {
                let captured = if !super::super::type_reference_is_reference(
                    program,
                    dependency.source_type,
                ) {
                    aggregate_argument_sources(
                        program,
                        machine,
                        parameter,
                        actual,
                        &selected,
                        captured_context,
                        symbols,
                        inference,
                    )?
                } else {
                    let captured = argument_origin(
                        program,
                        machine,
                        parameter,
                        actual,
                        captured_context,
                        symbols,
                        inference,
                    )?;
                    let mut projected = captured.clone();
                    projected.source = captured.source.append_relative(program, &relative);
                    projected.path = path(program, &projected.source);
                    let carrier = FrameSourcePlace::from_expression(program, actual)
                        .append_relative(program, &relative);
                    match dependency.source {
                        // This is the outer reference's referent, not a descriptor
                        // stored at the same empty path inside that referent.
                        ViewReturnInputSource::Referent => vec![SlotSource {
                            origin: projected,
                            carrier,
                            lifetime_access: Some(dependency.access),
                        }],
                        ViewReturnInputSource::Stored => {
                            stored_origins::captured_reference_projection_sources(
                                program,
                                &captured,
                                &projected,
                                &context.stored,
                            )
                            .into_iter()
                            .map(|source| {
                                let source = source?;
                                Some(SlotSource {
                                    origin: source.origin,
                                    carrier: if source.lifetime_access.is_some() {
                                        source.carrier
                                    } else {
                                        carrier.clone()
                                    },
                                    lifetime_access: source
                                        .lifetime_access
                                        .or(Some(dependency.access)),
                                })
                            })
                            .collect::<Option<Vec<_>>>()?
                        }
                    }
                };
                for source in captured {
                    if !sources.contains(&source) {
                        sources.push(source);
                    }
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

pub(super) fn owner_segments(
    path: &[checked_trees::BorrowLoanOwnerSegment],
) -> Option<Vec<PlaceSegment>> {
    path.iter()
        .map(|segment| {
            Some(match segment {
                checked_trees::BorrowLoanOwnerSegment::Field(symbol) => {
                    PlaceSegment::Field { symbol: *symbol }
                }
                checked_trees::BorrowLoanOwnerSegment::Case(variant) => {
                    PlaceSegment::Case { variant: *variant }
                }
                checked_trees::BorrowLoanOwnerSegment::FixedIndex(index) => {
                    PlaceSegment::FixedIndex { index: *index }
                }
                checked_trees::BorrowLoanOwnerSegment::DynamicIndex => return None,
            })
        })
        .collect()
}
