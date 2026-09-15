use crate::{SemanticDomainId, ServiceReachId, ServiceReachRowId};

#[test]
fn semantic_ids_are_zii_inert() {
    assert!(!SemanticDomainId::default().is_valid());
    assert!(!ServiceReachId::default().is_valid());
    assert!(!ServiceReachRowId::default().is_valid());
}
