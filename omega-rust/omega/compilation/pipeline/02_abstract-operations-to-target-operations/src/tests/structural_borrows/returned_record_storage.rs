//! Returned record loans retain their checked origins whether their backing
//! storage entered as a parameter or was constructed in this activation.

use super::source_plan;
use semantic_vocabulary::PlaceId;
use target::NativeTarget;
use target_operations::{TargetCallResult, TargetUnitOperation};

#[test]
fn returned_record_writes_use_the_storage_contract_not_the_parameter_roster() {
    for (parameters, setup) in [("bag: &mut Bag", ""), ("", "let mut bag: Bag;")] {
        let source = source_plan(&format!(
            "data Cell {{ value: i32; }}
            data Bag {{ cells: [Cell; 4]; other: i32; }}
            data Main {{}}
            machine pick(bag: &mut Bag) -> &mut Cell {{
                let slots: &mut [Cell] = bag.cells.as_mut_slice();
                transition {{ _ -> &mut slots[2] }}
            }}
            machine Main::run({parameters}) {{
                {setup}
                let cell: &mut Cell = pick(&mut bag);
                cell.value = 7;
                bag.other = 63;
            }}"
        ));
        for native in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let target = crate::lower_to_target_operations(
                &source,
                crate::TargetLoweringRequest::new(native),
            )
            .expect("writes through a returned loan support both storage origins");
            crate::validate_abstract_to_target_translation(&source, native, &target)
                .expect("the checked translation preserves the reference origin");

            let mut corrupted = target.clone();
            let reference = corrupted
                .functions
                .iter_mut()
                .flat_map(|function| &mut function.graph.blocks)
                .flat_map(|block| &mut block.operations)
                .find_map(|operation| match operation {
                    TargetUnitOperation::Call {
                        result:
                            TargetCallResult::Structural {
                                reference_results, ..
                            },
                        ..
                    } => reference_results.first_mut(),
                    _ => None,
                })
                .expect("the result retains reference custody");
            reference.root = PlaceId::new(777).expect("nonzero forged root");
            assert!(
                crate::validate_abstract_to_target_translation(&source, native, &corrupted)
                    .is_err(),
                "a different physical origin must not acquire the returned loan's authority"
            );
        }
    }
}
