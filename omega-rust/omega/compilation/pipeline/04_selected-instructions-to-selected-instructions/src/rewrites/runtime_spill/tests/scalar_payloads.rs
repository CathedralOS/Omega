use super::{
    Arc, IntegerSign, IntegerType, NativeTarget, ScalarType, VirtualRegisterId,
    VirtualRegisterOrigin, baseline_target_register_environment, fixture,
    selected_instruction_plan_identity,
};
use crate::RuntimeSpillError;
use crate::ValidatedRuntimeSpill;
use crate::rewrites::runtime_spill::tests::budget;
use crate::spill_selected_runtime_value;
use crate::validate_runtime_spill;

fn typed_fixture(target: NativeTarget, scalar_type: ScalarType) -> ValidatedRuntimeSpill {
    let mut source = fixture(target);
    for register in &mut Arc::make_mut(&mut source.transformed).functions[0].virtual_registers {
        register.scalar_type = scalar_type;
    }
    let identity = selected_instruction_plan_identity(source.transformed());
    source.receipt.source_selected = identity;
    source.receipt.transformed_selected = identity;
    source
}

#[test]
fn spill_admission_admits_address_carriers_and_rejects_non_gpr_integer_widths() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    for sign in [IntegerSign::Unsigned, IntegerSign::Signed] {
        for bits in [1, 7, 24, 65, 128] {
            let source = typed_fixture(
                target,
                ScalarType::Integer(IntegerType::new(sign, bits).unwrap()),
            );
            assert_eq!(
                spill_selected_runtime_value(
                    &source,
                    0,
                    VirtualRegisterId(1),
                    &environment,
                    budget(),
                )
                .unwrap_err(),
                RuntimeSpillError::UnsupportedValue
            );
        }
    }
    // The address carrier alone never widens the admitted widths: payloads
    // that do not fit one GPR keep rejecting.
    for bits in [1, 7, 24, 65, 128] {
        let source = typed_fixture(
            target,
            ScalarType::Integer(IntegerType::address(bits).unwrap()),
        );
        assert_eq!(
            spill_selected_runtime_value(&source, 0, VirtualRegisterId(1), &environment, budget(),)
                .unwrap_err(),
            RuntimeSpillError::UnsupportedValue
        );
    }
    // At a GPR width the carrier no longer excludes the victim: the spill and
    // its independent replay accept, and the reload registers retain the exact
    // address scalar type.
    for bits in [8, 16, 32, 64] {
        let scalar_type = ScalarType::Integer(IntegerType::address(bits).unwrap());
        let source = typed_fixture(target, scalar_type);
        let result =
            spill_selected_runtime_value(&source, 0, VirtualRegisterId(1), &environment, budget())
                .unwrap();
        assert!(
            result.transformed().functions[0]
                .virtual_registers
                .iter()
                .skip(5)
                .all(|register| matches!(
                    register.origin,
                    VirtualRegisterOrigin::SpillAddress { .. }
                ) || register.scalar_type == scalar_type)
        );
        assert!(
            validate_runtime_spill(
                &source,
                0,
                VirtualRegisterId(1),
                &environment,
                budget(),
                result.transformed().clone()
            )
            .is_ok()
        );
    }
}
