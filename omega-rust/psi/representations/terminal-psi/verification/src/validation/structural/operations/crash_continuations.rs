//! Unit call crash continuations and crash route places.

use crate::validation::structural::operations::structural_paths::structural_argument_canonical_prefix;
use crate::validation::{
    BTreeMap, BTreeSet, CanonicalStructuralPathSegment, CrashPredicateTerm, CrashRouteBucket,
    CrashRouteGuard, ModuleError, OperationId, PlaceId, ScalarTerm, StructuralArgument,
    StructuralPlaceKind, StructuralTypeId, TerminalMachine, TerminalModule, ValueId, propositions,
    substitute_proposition_structural_places,
};

pub(crate) fn validate_unit_call_crash_continuations(
    module: &TerminalModule,
    caller: &TerminalMachine,
    callee: &TerminalMachine,
    scalar_arguments: &[ValueId],
    arguments: &[StructuralArgument],
    continuations: &[CrashRouteBucket],
    operation: OperationId,
) -> Result<(), ModuleError> {
    let boolean_roots = callee
        .contract
        .crash_routes
        .iter()
        .flat_map(|bucket| &bucket.alternatives)
        .filter_map(|guard| match guard {
            CrashRouteGuard::Truth => None,
            CrashRouteGuard::Predicate(predicate) => Some(predicate.proposition()),
        })
        .flat_map(propositions::proposition_boolean_field_roots)
        .collect::<BTreeSet<_>>();
    let substitutions = callee
        .structural_parameters
        .iter()
        .zip(arguments)
        .map(|(parameter, argument)| {
            let prefix = structural_argument_canonical_prefix(module, caller, argument);
            if prefix.is_none() && boolean_roots.contains(&parameter.place) {
                return Err(
                    ModuleError::ProjectedUnitCallContractUsesStructuralParameter {
                        operation,
                        callee: callee.id,
                        place: parameter.place,
                    },
                );
            }
            Ok((
                parameter.place,
                (argument.place, prefix.unwrap_or_default()),
            ))
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    let mut expected = substitute_crash_route_places(&callee.contract.crash_routes, &substitutions);
    if !callee.parameters.is_empty() {
        let scalar_substitutions = callee
            .parameters
            .iter()
            .zip(scalar_arguments)
            .map(|(parameter, argument)| {
                (
                    parameter.id,
                    ScalarTerm::value(*argument, parameter.scalar_type),
                )
            })
            .collect::<BTreeMap<_, _>>();
        expected =
            crate::validation::crash::substitute_crash_routes(&expected, &scalar_substitutions);
    }
    if !crate::validation::crash::crash_routes_match(continuations, &expected) {
        return Err(ModuleError::CallCrashContinuationsMismatch {
            operation,
            callee: callee.id,
        });
    }
    // Coverage of the exact continuations is a reconstructed crash obligation
    // answered by the proof bundle at verification, not searched here.
    Ok(())
}

fn substitute_crash_route_places(
    routes: &[CrashRouteBucket],
    substitutions: &BTreeMap<PlaceId, (PlaceId, Vec<CanonicalStructuralPathSegment>)>,
) -> Vec<CrashRouteBucket> {
    routes
        .iter()
        .map(|bucket| {
            let mut alternatives = bucket
                .alternatives
                .iter()
                .map(|guard| match guard {
                    CrashRouteGuard::Truth => CrashRouteGuard::Truth,
                    CrashRouteGuard::Predicate(predicate) => CrashRouteGuard::Predicate(
                        CrashPredicateTerm::new(substitute_proposition_structural_places(
                            predicate.proposition(),
                            substitutions,
                        )),
                    ),
                })
                .collect::<Vec<_>>();
            alternatives.sort();
            alternatives.dedup();
            if alternatives.contains(&CrashRouteGuard::Truth) {
                alternatives = vec![CrashRouteGuard::Truth];
            }
            CrashRouteBucket {
                cause: bucket.cause,
                alternatives,
            }
        })
        .collect()
}

/// Resolve the declared structural type behind a caller place. Field-store
/// destinations and structural call arguments share the same root inventory:
/// machine parameters, block-view parameters, then declared structural places.
pub(crate) fn caller_structural_root_type(
    caller: &TerminalMachine,
    root: PlaceId,
) -> Option<StructuralTypeId> {
    caller
        .structural_parameters
        .iter()
        .find_map(|parameter| (parameter.place == root).then_some(parameter.structural_type))
        .or_else(|| {
            crate::validation::block_views::parameter(caller, root)
                .map(|parameter| parameter.structural_type)
        })
        .or_else(|| {
            caller
                .structural_places
                .iter()
                .find_map(|place| match place.kind {
                    StructuralPlaceKind::ByteSequenceLiteral {
                        structural_type, ..
                    }
                    | StructuralPlaceKind::TrivialAffineLocal {
                        structural_type, ..
                    }
                    | StructuralPlaceKind::OperationResult {
                        structural_type, ..
                    } if place.id == root => Some(structural_type),
                    _ => None,
                })
        })
}
