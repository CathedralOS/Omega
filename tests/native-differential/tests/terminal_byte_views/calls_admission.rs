//! Mixed-call receiving checks retain scalar identity and borrowed-reference custody.

use legalized_operations::{LegalizedScalarArgument, LegalizedScalarInstructionKind};
use semantic_vocabulary::ValueId;
use target::NativeTarget;
use target_operations_to_selected_instructions::{
    legalize_target_operations, validate_legalized_operations,
};
use terminal_psi::StructuralAccess;

use super::{
    byte_view_target,
    fixtures::{byte_view_read_call_module, byte_view_read_proof},
};

#[test]
fn mixed_call_replay_rejects_changed_scalar_and_reference_custody() {
    let module = byte_view_read_call_module();
    let proof = byte_view_read_proof(&module);
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let compiled = byte_view_target(&module, &proof, target);
        let native = compiled.target_operations();
        let abstracted = compiled.optimized().plan();
        let unit = compiled.optimized().unit();
        let admitted = legalize_target_operations(native, abstracted, unit).unwrap();
        for corruption in 0..8 {
            let mut proposed = admitted.plan().clone();
            let caller = proposed
                .scalar_functions
                .iter_mut()
                .find(|function| function.machine == module.entry)
                .unwrap();
            let call = caller
                .blocks
                .iter_mut()
                .flat_map(|block| &mut block.instructions)
                .find_map(|instruction| match &mut instruction.kind {
                    LegalizedScalarInstructionKind::Call(call) => Some(call),
                    _ => None,
                })
                .expect("mixed call instruction");
            match corruption {
                0 => {
                    let LegalizedScalarArgument::Scalar { source, .. } = &mut call.arguments[0]
                    else {
                        panic!("scalar prefix")
                    };
                    *source = ValueId::new(106).unwrap();
                }
                1 => {
                    let LegalizedScalarArgument::Scalar { placement, .. } = &mut call.arguments[0]
                    else {
                        panic!("scalar prefix")
                    };
                    *placement = call.call_plan.parameters[1].clone();
                }
                2 => {
                    call.arguments.pop();
                }
                3 => call.arguments.swap(0, 1),
                4 => {
                    let LegalizedScalarArgument::Structural { semantic, target } =
                        &mut call.arguments[1]
                    else {
                        panic!("structural suffix")
                    };
                    semantic.access = StructuralAccess::MutableBorrow;
                    target.access = semantic.access;
                }
                5 => {
                    let LegalizedScalarArgument::Structural { target, .. } = &mut call.arguments[1]
                    else {
                        panic!("structural suffix")
                    };
                    target.source = call.result_placement.clone().unwrap().into();
                }
                6 => call.call_plan.parameters.swap(0, 1),
                7 => {
                    let LegalizedScalarArgument::Structural { target, .. } = &mut call.arguments[1]
                    else {
                        panic!("structural suffix")
                    };
                    target.destination = call.call_plan.parameters[0].clone();
                }
                _ => unreachable!(),
            }
            assert!(
                validate_legalized_operations(native, abstracted, unit, proposed).is_err(),
                "legalized substitution {corruption} on {target:?}"
            );
        }
    }
}
