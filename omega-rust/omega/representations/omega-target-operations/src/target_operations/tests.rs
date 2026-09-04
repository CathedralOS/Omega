//! Representation invariants and value-category controls.

use super::CallSiteOwner;
use psi_core::{EdgeId, OperationId};

#[test]
fn call_site_owner_preserves_disjoint_operation_and_cleanup_action_identities() {
    let operation = OperationId::new(7).expect("nonzero operation");
    let edge = EdgeId::new(7).expect("nonzero edge");

    let operation_owner = CallSiteOwner::Operation(operation);
    assert_eq!(operation_owner.operation(), Some(operation));
    assert_eq!(operation_owner.edge(), None);

    let edge_owner = CallSiteOwner::CleanupAction {
        edge,
        action_ordinal: 3,
    };
    assert_eq!(edge_owner.operation(), None);
    assert_eq!(edge_owner.edge(), Some(edge));
    assert_eq!(edge_owner.cleanup_action_ordinal(), Some(3));
    assert_eq!(operation_owner.cleanup_action_ordinal(), None);
    assert_ne!(operation_owner, edge_owner);
}
