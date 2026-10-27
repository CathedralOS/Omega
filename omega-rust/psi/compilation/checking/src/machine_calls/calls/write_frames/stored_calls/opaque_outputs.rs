//! Capture an admitted output's lifetime dependencies from the same pre-call
//! inventory as ordinary stored-reference effects. The output's pointer is not
//! any of these places: opaque admission, not input geometry, establishes it.
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
    call_site: super::super::caller_aliases::CallerWriteSite<'_>,
    target: SymbolHandle,
    expression: ExpressionHandle,
) -> Option<SlotRelationTransfer> {
    if parameters.len() != arguments.len() {
        return None;
    }
    let relation = resolve_provider_outputs(program, parameters).ok()?;
    let mut effects = Vec::new();
    for output in relation.outputs {
        let ProviderOutputDestination::Parameter(parameter_index) = output.destination;
        let parameter = &parameters[parameter_index];
        let segments = owner_segments(&output.owner_path)?;
        let (destination, _) = destination_source(
            program,
            machine,
            parameter,
            &arguments[parameter_index],
            &segments,
            context,
            symbols,
        )?;
        let identity =
            super::output_identity(program, machine, call_site, target, expression, parameter)?;
        let sources = super::output_dependency_sources(
            program,
            machine,
            parameters,
            arguments,
            context,
            symbols,
            &output.dependencies,
            vec![supplied_source(identity, segments)],
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

/// Formal correspondence for the same complete applied output relation. The
/// dependencies remain lifetime-only. Flow authenticates and retains their
/// actual captured cohort from the performed invocation, not these coordinates.
pub(super) fn formal_effects(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    parameters: &[StateParameter],
    site: super::super::caller_aliases::CallerWriteSite<'_>,
    target: SymbolHandle,
    expression: ExpressionHandle,
) -> Option<Vec<StoredReferenceEffect>> {
    let relation = resolve_provider_outputs(program, parameters).ok()?;
    if relation.readiness != crate::borrow_contracts::BorrowRelationReadiness::Complete {
        return None;
    }
    let mut effects = Vec::new();
    for output in relation.outputs {
        let ProviderOutputDestination::Parameter(position) = output.destination;
        let parameter = parameters.get(position)?;
        let segments = owner_segments(&output.owner_path)?;
        let identity =
            super::output_identity(program, machine, site, target, expression, parameter)?;
        let mut sources = vec![supplied_source(identity, segments.clone())];
        for dependency in output.dependencies {
            let parameter = parameters.get(dependency.parameter_index)?;
            if parameter.type_reference != dependency.source_type {
                return None;
            }
            let source = FrameSourcePlace {
                segments: owner_segments(&dependency.source_path)?,
                ..FrameSourcePlace::parameter_root(parameter)
            };
            sources.push(SlotSource::from_place(
                program,
                source,
                FramePathPrecision::Exact,
                Some(dependency.access),
            ));
        }
        effects.push(SlotEffect {
            root: parameter.symbol,
            segments,
            sources: Some(sources),
            incoming: false,
            may_select: false,
            kind: stored_origins::StoredOriginKind::Reference(output.access),
        });
    }
    super::public_effects(effects)
}

fn supplied_source(
    identity: facts::InvocationOutputIdentity,
    segments: Vec<PlaceSegment>,
) -> SlotSource {
    let supplied = FrameSourcePlace::invocation_output(identity, segments);
    SlotSource {
        carrier_backing: None,
        origin: FramePlaceOrigin {
            path: "invocation output".to_owned(),
            precision: FramePathPrecision::Exact,
            source: supplied.clone(),
        },
        carrier: supplied,
        lifetime_access: None,
    }
}
