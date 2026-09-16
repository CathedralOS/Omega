//! What one installed function row records about scalar custody, and the
//! consistency its retained facts must show.

use super::super::stack_facts::installed_stack_facts_are_canonical;
use crate::installation_record::{
    BoundaryRealization, CallSiteOwner, InstallationRecord, InstalledFunction, MachineId,
    SemanticCodeSite, StructuralMultiplicity, StructuralTypeId, borrowed_structural,
    graph_structural, incoming_structural, installed_function_scalar_transport_is_canonical,
    installed_unit_scalar_transport,
};

/// The row's scalar custody facts.
#[derive(Clone, Copy)]
pub(super) struct ScalarCustodyFacts {
    /// The row carries scalar control-flow affine cleanups.
    pub(super) has_scalar_control_cleanup: bool,
    /// The row carries a scalar affine cleanup of either kind.
    pub(super) has_scalar_cleanup: bool,
    /// The row has scalar custody: a scalar cleanup, a scalar boundary
    /// settlement, a mixed structural/scalar ABI or scalar field stores.
    pub(super) has_scalar_custody: bool,
    /// The structural-call scalar return, if any, is exactly one call and
    /// one empty cleanup attributed to their operation and edge.
    pub(super) structural_call_scalar_result_is_exact: bool,
    /// The mixed ABI's owned structural parameters have no materialized homes.
    pub(super) unmaterialized_owned: bool,
    /// The row's structural roster is a graph-structural roster.
    pub(super) graph_structural_roster: bool,
    /// The mixed structural roster pairs parameters and homes with the ABI.
    pub(super) mixed_structural_roster_is_exact: bool,
}

/// What the row records about scalar custody: which scalar cleanups it
/// carries, whether it has any scalar custody at all, whether its
/// structural-call scalar return is exact, and whether its mixed structural
/// roster is exact (or a graph-structural roster).
pub(super) fn scalar_custody_facts(
    record: &InstallationRecord,
    function: &InstalledFunction,
    function_unit_calls: &[machine_code::InternalUnitCallRecord],
) -> ScalarCustodyFacts {
    let has_scalar_control_cleanup = !function.scalar_control_affine_cleanups.is_empty();
    let has_scalar_cleanup = function.scalar_affine_cleanup.is_some() || has_scalar_control_cleanup;
    let has_scalar_boundary_custody = record.boundary_settlements.iter().any(|settlement| {
        settlement.machine == function.machine
            && matches!(
                settlement.settlement.realization,
                BoundaryRealization::DirectPortReadU8(_)
                    | BoundaryRealization::HostedExitProcessI32(_)
            )
    });
    let has_scalar_custody = has_scalar_cleanup
        || has_scalar_boundary_custody
        || function.mixed_structural_scalar_abi.is_some()
        || !function.scalar_structural_scalar_field_stores.is_empty();
    let structural_call_scalar_result_is_exact =
        function
            .structural_call_scalar_return
            .is_none_or(|returned| {
                let attributions = record
                    .semantic_code_attribution
                    .iter()
                    .filter(|attribution| attribution.machine == function.machine)
                    .map(|attribution| &attribution.attribution)
                    .collect::<Vec<_>>();
                function.unit_body
                    && function.unit_stack.is_some()
                    && function.scalar_stack.is_none()
                    && matches!(
                        (
                            function_unit_calls,
                            attributions.as_slice(),
                            function.unit_affine_cleanup.as_ref(),
                        ),
                        ([call], [call_attribution, return_attribution], Some(cleanup))
                        if call.owner == CallSiteOwner::Operation(returned.psi_operation)
                            && call.target == returned.callee
                            && call.operation_ordinal == 0
                            && call.result == Some(returned.scalar_type)
                            && call.semantic_result.as_ref().is_some_and(|result| {
                                result.value == returned.source_value
                                    && result.scalar_type == returned.scalar_type
                            })
                            && call_attribution.site
                                == SemanticCodeSite::Operation(returned.psi_operation)
                            && call_attribution.operation_ordinal == call.operation_ordinal
                            && call_attribution.code_offset == call.code_offset
                            && call_attribution.byte_count == call.byte_count
                            && return_attribution.site == SemanticCodeSite::Edge(returned.psi_edge)
                            && return_attribution.operation_ordinal == 1
                            && return_attribution.code_offset == cleanup.code_offset
                            && return_attribution.byte_count == cleanup.byte_count
                            && cleanup.psi_edge == returned.psi_edge
                            && cleanup.locals.is_empty()
                            && cleanup.actions.is_empty()
                    )
            });
    // This is a record-shape check, not erasure authority. The installation
    // is joined field-for-field to the independently admitted image below;
    // image replay requires retained source for every missing home.
    let unmaterialized_owned = function.scalar_structural_parameter_homes.is_empty()
        && function
            .mixed_structural_scalar_abi
            .as_ref()
            .is_some_and(|abi| {
                !abi.structural_parameters.is_empty()
                    && abi.structural_parameters.iter().all(|parameter| {
                        parameter.access == terminal_psi::StructuralAccess::Owned
                            && matches!(
                                parameter.multiplicity,
                                StructuralMultiplicity::Affine
                                    | StructuralMultiplicity::Unrestricted
                            )
                            && parameter.projected_qualifications.is_empty()
                            && parameter.shape.class
                                != calling_conventions::ValueClass::BorrowedReference
                    })
            });
    let graph_structural_roster = graph_structural::function_is_exact(function, record.target);
    let mixed_structural_roster_is_exact = graph_structural_roster || function
        .mixed_structural_scalar_abi
        .as_ref()
        .is_none_or(|abi| {
            function.scalar_structural_parameters.len() == abi.structural_parameters.len()
                && (unmaterialized_owned || function.scalar_structural_parameter_homes.len()
                    == abi.structural_parameters.len())
                && function
                    .scalar_structural_parameters
                    .iter()
                    .zip(&abi.structural_parameters)
                    .all(|(parameter, retained)| {
                        parameter.place == retained.place
                            && parameter.structural_type == retained.structural_type
                            && parameter.multiplicity == retained.multiplicity
                            && parameter.access == retained.access
                            && parameter.shape == retained.shape
                    })
                && function.scalar_structural_parameter_homes.iter()
                    .zip(&abi.structural_parameters)
                    .all(|(home, retained)| {
                        home.place == retained.place
                            && home.structural_type == retained.structural_type
                            && home.multiplicity == retained.multiplicity
                            && home.access == retained.access
                            && home.shape == retained.shape
                            && home.source == retained.placement
                            && installed_unit_scalar_transport::mixed_structural_home_is_canonical(home, retained)
                    })
        });
    ScalarCustodyFacts {
        has_scalar_control_cleanup,
        has_scalar_cleanup,
        has_scalar_custody,
        structural_call_scalar_result_is_exact,
        unmaterialized_owned,
        graph_structural_roster,
        mixed_structural_roster_is_exact,
    }
}

/// Whether the row's retained facts disagree with one another: stack facts
/// and scalar transport must be canonical, rosters and homes must pair up
/// exactly unless the roster is graph-structural, a Unit body has exactly
/// one Unit cleanup, scalar cleanups exclude a Unit body, and structural
/// parameters need scalar custody.
pub(super) fn row_facts_disagree(
    record: &InstallationRecord,
    function: &InstalledFunction,
    attachments: &std::collections::BTreeMap<MachineId, Option<StructuralTypeId>>,
    facts: &ScalarCustodyFacts,
) -> bool {
    let ScalarCustodyFacts {
        has_scalar_control_cleanup,
        has_scalar_cleanup,
        has_scalar_custody,
        structural_call_scalar_result_is_exact,
        unmaterialized_owned,
        graph_structural_roster,
        mixed_structural_roster_is_exact,
        ..
    } = *facts;
    !installed_stack_facts_are_canonical(function, attachments)
        || !installed_function_scalar_transport_is_canonical(function, record.target)
        || !structural_call_scalar_result_is_exact
        || !mixed_structural_roster_is_exact
        || (!graph_structural_roster
            && function.unit_parameters.len() != function.unit_parameter_homes.len())
        || function.unit_body != function.unit_affine_cleanup.is_some()
        || (incoming_structural::has_incoming(function)
            && !incoming_structural::function_is_exact(record, function))
        || (!graph_structural_roster
            && borrowed_structural::has_borrowed(function)
            && !borrowed_structural::function_is_exact(record, function))
        || (!graph_structural_roster
            && !incoming_structural::has_incoming(function)
            && !borrowed_structural::has_borrowed(function)
            && !function.unit_body
            && !has_scalar_cleanup
            && (!function.unit_parameters.is_empty() || !function.unit_parameter_homes.is_empty()))
        || (!graph_structural_roster
            && !unmaterialized_owned
            && function.scalar_structural_parameters.len()
                != function.scalar_structural_parameter_homes.len())
        || (!function.scalar_control_affine_cleanups.is_empty()
            && function.scalar_control_affine_cleanups.len() < 2)
        || (function.scalar_affine_cleanup.is_some() && has_scalar_control_cleanup)
        || (has_scalar_cleanup && function.unit_body)
        || (!graph_structural_roster
            && function
                .scalar_structural_parameters
                .iter()
                .zip(&function.scalar_structural_parameter_homes)
                .any(|(parameter, home)| {
                    parameter.place != home.place
                        || parameter.structural_type != home.structural_type
                        || parameter.multiplicity != home.multiplicity
                        || parameter.access != home.access
                        || parameter.shape != home.shape
                }))
        || (!has_scalar_custody
            && (!function.scalar_structural_parameters.is_empty()
                || !function.scalar_structural_parameter_homes.is_empty()))
        || (!graph_structural_roster
            && function
                .unit_parameters
                .iter()
                .zip(&function.unit_parameter_homes)
                .any(|(parameter, home)| {
                    parameter.place != home.place
                        || parameter.structural_type != home.structural_type
                        || parameter.multiplicity != home.multiplicity
                        || parameter.access != home.access
                        || parameter.shape != home.shape
                }))
}
