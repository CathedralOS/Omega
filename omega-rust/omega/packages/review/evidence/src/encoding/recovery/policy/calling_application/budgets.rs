use super::tests::fixture;
use super::*;
use crate::record::*;

// These fixtures have only nominal machine statics, so each machine contract
// charges one node in addition to its outer list element.
pub(super) fn fixture_elements(policy: &PackagePolicyCallingPlan) -> usize {
    let conformance_elements = |application: &PackagePolicyClosedConformanceApplication| {
        application.lifetime_arguments.len()
            + application.type_arguments.len()
            + application.const_arguments.len()
            + application.machine_arguments.len()
            + application.trait_lifetime_arguments.len()
            + application.trait_arguments.len()
            + application.rows.len()
    };
    policy.boundary_arguments.len()
        + policy.requirement_arguments.len()
        + policy.requirement_lifetime_arguments.len()
        + 2 * policy.static_parameters.len()
        + policy.shape_graph.shapes.len()
        + policy.shape_graph.fields.len()
        + policy.shape_graph.parameters.len()
        + policy.semantic_parameters.len()
        + policy.native_parameters.len()
        + policy.callbacks.binders.len()
        + policy.callbacks.demands.len()
        + policy.callbacks.materializations.len()
        + policy.callbacks.layouts.len()
        + policy
            .callbacks
            .layouts
            .iter()
            .map(|layout| conformance_elements(&layout.terminal_slot))
            .sum::<usize>()
        + policy.opaque_uses.len()
        + policy
            .opaque_uses
            .iter()
            .map(|use_| {
                conformance_elements(&use_.application)
                    + use_.occurrences.len()
                    + use_
                        .occurrences
                        .iter()
                        .map(|occurrence| {
                            occurrence.path.len() + occurrence.placement.locations.len()
                        })
                        .sum::<usize>()
            })
            .sum::<usize>()
        + policy.physical.parameters.len()
        + policy
            .physical
            .parameters
            .iter()
            .map(|placement| placement.locations.len())
            .sum::<usize>()
        + policy
            .physical
            .result
            .as_ref()
            .map_or(0, |placement| placement.locations.len())
        + policy.physical.ordinary_clobbers.len()
        + policy.physical.state.interrupted_state.as_slice().len()
        + policy.physical.state.saved_state.as_slice().len()
        + policy.physical.state.restored_state.as_slice().len()
        + policy
            .physical
            .state
            .permitted_transitive_use
            .as_slice()
            .len()
}

#[test]
fn owned_storage_exact_boundary_includes_complete_canonical_scratch() {
    let policy = fixture();
    let bytes = policy.canonical_bytes().unwrap();
    let physical = &policy.physical;
    let states = &physical.state;
    let owned = policy.boundary_trait.path.len()
        + policy.requirement.path.len()
        + policy.requirement_trait.path.len()
        + std::mem::size_of::<PackageReviewBoundaryShape>()
        + std::mem::size_of::<u16>()
        + std::mem::size_of::<PackagePolicyCallingParameter>()
        + "value".len()
        + "u64".len()
        + std::mem::size_of::<PackagePolicyNativeParameter>()
        + "value".len()
        + std::mem::size_of::<PackageReviewBoundaryValuePlacement>()
        + physical.parameters[0].locations.len()
            * std::mem::size_of::<PackageReviewBoundaryValueLocation>()
        + physical.ordinary_clobbers.len() * std::mem::size_of::<PackageReviewMachineRegister>()
        + [
            states.interrupted_state.as_slice(),
            states.saved_state.as_slice(),
            states.restored_state.as_slice(),
            states.permitted_transitive_use.as_slice(),
        ]
        .iter()
        .map(|set| set.len())
        .sum::<usize>()
            * std::mem::size_of::<PackagePolicyMachineState>()
        + bytes.len();
    let limits = |owned| {
        PackagePolicyRecoveryLimits::new(
            bytes.len(),
            usize::MAX,
            fixture_elements(&policy),
            owned,
            usize::MAX,
        )
    };
    assert_eq!(
        PackagePolicyCallingPlan::recover_canonical(&bytes, limits(owned)).unwrap(),
        policy
    );
    assert_eq!(
        PackagePolicyCallingPlan::recover_canonical(&bytes, limits(owned - 1)),
        Err(Error::AllocationLimitExceeded)
    );
}
