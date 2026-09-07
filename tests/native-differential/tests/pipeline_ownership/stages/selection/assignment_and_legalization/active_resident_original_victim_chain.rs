//! Independent ordinary graph custody over the existing admitted source fixture.
use crate::tests::*;

#[test]
fn ordinary_graph_preserves_source_proof_fuel_and_selected_control() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let staged = staged_active_resident_exact_add_original_victim_chain(target);
        assert_ordinary_graph_custody(&staged);
    }
}
