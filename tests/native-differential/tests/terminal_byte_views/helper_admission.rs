//! Standalone receiving entrances reject coherently substituted borrowed arguments.
use super::*;
use calling_conventions::ValueShape;
use legalized_operations::{LegalizedScalarArgument, LegalizedScalarInstructionKind};
use target_operations_to_selected_instructions::{
    legalize_target_operations, validate_legalized_operations,
};
use terminal_psi::StructuralAccess;

#[test]
fn helper_replay_rejects_substituted_argument_custody() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let compiled = byte_view_target(
            &fixtures::byte_view_length_helper_module(),
            &ProofBundle::default(),
            target,
        );
        let native = compiled.target_operations();
        let abstracted = compiled.optimized().plan();
        let unit = compiled.optimized().unit();
        let admitted = legalize_target_operations(native, abstracted, unit).unwrap();
        for corruption in 0..7 {
            let mut proposed = admitted.plan().clone();
            let caller = proposed
                .scalar_functions
                .iter_mut()
                .find(|function| function.machine == native.entry)
                .unwrap();
            let LegalizedScalarInstructionKind::Call(call) =
                &mut caller.blocks[0].instructions[0].kind
            else {
                panic!("helper call")
            };
            let LegalizedScalarArgument::Structural { semantic, target } = &mut call.arguments[0]
            else {
                panic!("borrowed argument")
            };
            match corruption {
                0 => {
                    semantic.access = StructuralAccess::MutableBorrow;
                    target.access = semantic.access;
                }
                1 => {
                    semantic.place = PlaceId::new(3).unwrap();
                    target.place = semantic.place;
                }
                2 => target.source_byte_offset = 8,
                3 => {
                    target.structural_type = semantic_vocabulary::StructuralTypeId::new(99).unwrap()
                }
                4 => target.source = call.result_placement.clone().unwrap().into(),
                5 => {
                    let expected = calling_conventions::evaluate_call_plan(
                        call.call_plan.policy,
                        &calling_conventions::CallSignature {
                            parameters: vec![ValueShape::integer(16, 8)],
                            result: Some(ValueShape::integer(8, 8)),
                        },
                    )
                    .unwrap();
                    target.shape = expected.parameters[0].shape;
                    target.destination = expected.parameters[0].clone();
                    call.call_plan = expected;
                }
                6 => call.callee = native.entry,
                _ => unreachable!(),
            }
            assert!(
                validate_legalized_operations(native, abstracted, unit, proposed).is_err(),
                "substitution {corruption}"
            );
        }
    }
}
