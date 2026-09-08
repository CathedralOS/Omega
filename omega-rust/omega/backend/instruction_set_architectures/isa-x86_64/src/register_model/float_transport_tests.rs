use super::*;
use register_model::validate_physical_register_model;
#[test]
fn transport_and_mixed_call_constraints_reject_forged_classes_and_clobbers() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let catalog = x86_64_register_constraint_catalog(&physical);
    let mut wrong_transport = catalog.clone();
    let transport = wrong_transport
        .constraints
        .iter_mut()
        .find(|row| row.key == X86_64_FLOAT32_TO_BITS)
        .unwrap();
    transport.operands[0].class = GPR64;
    assert!(validate_x86_64_register_constraint_catalog(wrong_transport, &physical).is_err());
    let mut wrong_call = catalog;
    let call_key = x86_64_system_v_mixed_unit_call_keys()[0];
    let call = wrong_call
        .constraints
        .iter_mut()
        .find(|row| row.key == call_key)
        .unwrap();
    assert!(call.clobbers.pop().is_some());
    assert!(validate_x86_64_register_constraint_catalog(wrong_call, &physical).is_err());
}
