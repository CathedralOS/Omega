use super::{
    X86_64_FLOAT32_TO_BITS, validate_x86_64_register_constraint_catalog,
    x86_64_physical_register_model, x86_64_register_constraint_catalog,
    x86_64_system_v_mixed_unit_call_keys,
};
use crate::register_model::physical_model::GPR64;
use register_model::validate_physical_register_model;
#[test]
fn transport_and_mixed_call_constraints_reject_forged_classes_and_clobbers() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let catalog = x86_64_register_constraint_catalog(&physical);
    let mut wrong_transport = catalog.clone();
    let transport = register_model::find_register_constraint_mut(
        &mut wrong_transport.constraints,
        X86_64_FLOAT32_TO_BITS,
    )
    .unwrap();
    transport.operands[0].class = GPR64;
    assert!(validate_x86_64_register_constraint_catalog(wrong_transport, &physical).is_err());
    let mut wrong_call = catalog;
    let call_key = x86_64_system_v_mixed_unit_call_keys()[0];
    let call = register_model::find_register_constraint_mut(&mut wrong_call.constraints, call_key)
        .unwrap();
    assert!(call.clobbers.pop().is_some());
    assert!(validate_x86_64_register_constraint_catalog(wrong_call, &physical).is_err());
}
