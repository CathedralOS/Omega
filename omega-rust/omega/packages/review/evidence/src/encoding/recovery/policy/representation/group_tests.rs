//! Every opaque used by a retained application belongs to one complete group.

use super::{fixtures, tests::recover};
use crate::record::*;
use calling_conventions::{
    CallSignature, CallingPolicy, ValueShape, evaluate_ordinary_boundary_entry_plan,
};

fn two_opaque_demands() -> PackagePolicyRepresentation {
    let mut policy = fixtures::complete();
    let mut calling = policy.demands[0].calling.clone();
    let native = evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::SystemVAMD64,
        &CallSignature {
            parameters: vec![ValueShape::integer(8, 8); 2],
            result: None,
        },
    )
    .unwrap();
    calling.physical = PackagePolicyPhysicalCallingContract::from_validated_plan(&native);
    calling.callbacks.binders.clear();
    calling.callbacks.demands.clear();
    calling.callbacks.materializations.clear();
    calling.callbacks.layouts.clear();
    calling.native_parameters.truncate(2);
    calling.native_parameters[1].origin = PackagePolicyNativeParameterOrigin::SemanticFormal {
        formal_ordinal: 1,
        shape_root: 0,
    };
    calling.semantic_parameters[1].shape_root = 0;
    calling.semantic_parameters[1].value_type.canonical = "Unused".into();
    calling.shape_graph.parameters[1] = 0;
    let selection = &policy.selected_availability[1];
    calling.opaque_uses.push(PackagePolicyCallingOpaqueUse {
        opaque: selection.opaque.clone(),
        carrier: selection.carrier.clone(),
        selection_owner: selection.selection_owner,
        application: selection.application.clone(),
        origin: selection.origin,
        lifecycle: selection.lifecycle,
        copy_disposition: selection.copy_disposition,
        occurrences: vec![PackageReviewOpaqueRepresentationOccurrence {
            carrier_shape_root: 0,
            role: PackageReviewOpaqueRepresentationMovementRole::Parameter {
                formal_ordinal: 1,
                native_ordinal: 1,
            },
            path: Vec::new(),
            placement: calling.physical.parameters[1].clone(),
        }],
    });
    calling.validate_canonical_structure().unwrap();
    policy.demands = policy
        .selected_availability
        .iter()
        .map(|selection| PackagePolicyRepresentationDemand {
            opaque: selection.opaque.clone(),
            calling: calling.clone(),
        })
        .collect();
    policy
}

#[test]
fn a_calling_application_requires_all_opaque_demands_and_one_shared_meaning() {
    let policy = two_opaque_demands();
    let bytes = policy.canonical_bytes().unwrap();
    assert_eq!(recover(&bytes).unwrap(), policy);
    crate::encoding::encode::text_test_support::component(
        crate::encoding::encode::text_test_support::Component::Representation(&policy),
    );
    let mut missing = policy.clone();
    missing.demands.pop();
    assert!(missing.canonical_bytes().is_err());
    let mut changed = policy.clone();
    changed.demands[1].calling.static_parameters.clear();
    changed.demands[1]
        .calling
        .validate_canonical_structure()
        .unwrap();
    assert!(changed.canonical_bytes().is_err());
    let mut changed = policy;
    changed.demands.reverse();
    assert!(changed.canonical_bytes().is_err());
}

#[test]
fn the_same_opaque_can_be_used_by_distinct_complete_calling_applications() {
    let mut policy = two_opaque_demands();
    let mut other = policy.demands.clone();
    for demand in &mut other {
        demand.calling.requirement = fixtures::nominal("Boundary::other");
    }
    policy.demands.extend(other);
    policy
        .demands
        .sort_by(PackagePolicyRepresentationDemand::compare_application);
    let bytes = policy.canonical_bytes().unwrap();
    assert_eq!(recover(&bytes).unwrap(), policy);
    crate::encoding::encode::text_test_support::component(
        crate::encoding::encode::text_test_support::Component::Representation(&policy),
    );
    policy.demands.remove(1);
    assert!(policy.canonical_bytes().is_err());
}
