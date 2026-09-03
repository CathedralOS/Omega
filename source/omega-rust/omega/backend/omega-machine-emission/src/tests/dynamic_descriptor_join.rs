use omega_machine_code::{SemanticCodeSite, UnitScalarParameterLocationRecord};
use omega_target::{Architecture, NativeTarget};

use super::forwarded_dynamic_descriptor::assigned_scalar_plan_from_source;

const SOURCE: &str = r#"
    trait Measure { machine measure(&self) -> bool; }
    data Item [copy] { marker: bool; }
    Primary: Item satisfies Measure {
        machine measure(&self) -> bool { transition { _ -> self.marker } }
    }
    Secondary: Item satisfies Measure {
        machine measure(&self) -> bool { transition { _ -> self.marker } }
    }
    data Main [copy] { first: Item; second: Item; }
    machine Main::run(&self, choose_first: bool) {
        transition choose_first {
            true -> take_first()
            _ -> take_second()
        }
        state take_first(&self) {
            let selected: &dyn Measure = &self.first as &dyn Item::Primary;
            let result: bool = finish(selected);
        }
        state take_second(&self) {
            let selected: &dyn Measure = &self.second as &dyn Item::Secondary;
            let result: bool = finish(selected);
        }
    }
    machine finish(erased: &dyn Measure) -> bool {
        let result: bool = erased.measure();
        transition { _ -> result }
    }
"#;

#[test]
fn emits_joined_descriptor_arms_with_one_shared_unit_epilogue() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let assigned = assigned_scalar_plan_from_source(target, SOURCE);
        let emitted = crate::emit_machine_code(&assigned).expect("emit joined descriptor control");
        let caller = emitted
            .functions
            .iter()
            .find(|function| function.machine == emitted.entry)
            .expect("joined entry caller");
        let [first, second] = caller.forwarded_dynamic_descriptor_calls.as_slice() else {
            panic!("both descriptor-bearing leaves reach machine code: {caller:#?}")
        };
        assert_eq!(first.operation_ordinal, 1);
        assert_eq!(second.operation_ordinal, 3);
        assert_eq!(first.callee, second.callee);
        assert_ne!(
            first.dynamic_arguments[0].custody.source,
            second.dynamic_arguments[0].custody.source
        );
        let abi = caller
            .unit_scalar_abi
            .as_ref()
            .expect("joined caller retains its Unit scalar ABI");
        assert_eq!(abi.parameters.len(), 1);
        let expected_location = match target.architecture {
            Architecture::X86_64 => UnitScalarParameterLocationRecord::Register(
                omega_target_operations::MachineRegister::X86Rdi,
            ),
            Architecture::Aarch64 => UnitScalarParameterLocationRecord::Register(
                omega_target_operations::MachineRegister::Aarch64X(0),
            ),
        };
        let parameter_record = caller.unit_scalar_abi.as_ref().expect("joined caller ABI");
        let location = match parameter_record.parameters[0]
            .placement
            .locations
            .as_slice()
        {
            [omega_calling_conventions::ValueLocation::Register { register, .. }] => {
                UnitScalarParameterLocationRecord::Register(*register)
            }
            [
                omega_calling_conventions::ValueLocation::Stack {
                    stack_byte_offset, ..
                },
            ] => UnitScalarParameterLocationRecord::IncomingStack {
                byte_offset: *stack_byte_offset,
            },
            other => panic!("one direct Boolean ABI location expected: {other:#?}"),
        };
        assert_eq!(location, expected_location);
        assert_eq!(caller.semantic_code_attribution.len(), 5);
        assert!(matches!(
            caller.semantic_code_attribution[0].site,
            SemanticCodeSite::Edge(_)
        ));
        assert!(matches!(
            caller.semantic_code_attribution[2].site,
            SemanticCodeSite::Edge(_)
        ));
        assert!(matches!(
            caller.semantic_code_attribution[4].site,
            SemanticCodeSite::Edge(_)
        ));
        let first_return = caller.semantic_code_attribution[2];
        assert_eq!(
            first_return.byte_count,
            match target.architecture {
                Architecture::X86_64 => 5,
                Architecture::Aarch64 => 4,
            }
        );
        let final_return = caller.semantic_code_attribution[4];
        assert!(final_return.byte_count > 0);
        assert_eq!(
            final_return.code_offset + final_return.byte_count,
            caller.bytes.len()
        );
    }
}
