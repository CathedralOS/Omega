//! A shared descriptor keeps the exact qualified owner's storage identity.
use super::{NativeTarget, StructuralAccess, TargetUnitOperation, mutate_call_row, source_plan};
use target_operations::TargetStructuralArgumentSource;

#[test]
fn qualified_byte_owner_calls_replay_entry_and_arrival_storage() {
    let source = source_plan(
        r#"
        domain [u8;16]::Utf8 requires valid_utf8(self);
        data Main {}
        machine inspect(left: &[u8], right: &[u8], output: &mut u64) {
            transition left == "L" && right == "R" {
                true -> good(output)
                false -> bad(output)
            }
            state good(output: &mut u64) { output = 70; }
            state bad(output: &mut u64) { output = 1; }
        }
        machine Main::run(left: &mut [u8;16] in Utf8,
                          right: &mut [u8;16] in Utf8, output: &mut u64) {
            inspect(left, right, output);
            transition { _ -> arrived(left, right, output) }
            state arrived(left: &mut [u8;16] in Utf8,
                          right: &mut [u8;16] in Utf8, output: &mut u64) {
                inspect(left, right, output);
            }
        }
        "#,
    );
    for native in [NativeTarget::macos_arm64(), NativeTarget::windows_x64()] {
        let target =
            crate::lower_to_target_operations(&source, crate::TargetLoweringRequest::new(native))
                .expect("qualified owners present shared views at entry and state arrival");
        crate::validate_abstract_to_target_translation(&source, native, &target)
            .expect("independent receiving replay");
        let calls = target
            .functions
            .iter()
            .flat_map(|function| &function.graph.blocks)
            .flat_map(|block| &block.operations)
            .filter_map(|operation| {
                let TargetUnitOperation::Call {
                    psi_operation,
                    arguments,
                    ..
                } = operation
                else {
                    return None;
                };
                (arguments.len() == 3 && arguments[0].access == StructuralAccess::SharedBorrow)
                    .then_some((*psi_operation, &arguments[0], &arguments[1]))
            })
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 2, "entry and state calls retain both owners");
        assert!(calls.iter().any(|(_, argument, _)| matches!(
            argument.source,
            TargetStructuralArgumentSource::Placement(_)
        )));
        assert!(calls.iter().any(|(_, argument, _)| matches!(
            argument.source,
            TargetStructuralArgumentSource::BlockParameter { .. }
        )));
        for (identity, argument, sibling) in calls {
            assert_eq!(argument.root_structural_type, sibling.root_structural_type);
            assert_ne!(argument.place, sibling.place);
            assert_ne!(argument.source, sibling.source);
            for corruption in ["sibling source", "offset", "access"] {
                let changed = mutate_call_row(
                    &target,
                    |operation| matches!(operation, TargetUnitOperation::Call { psi_operation, .. } if *psi_operation == identity),
                    |operation| {
                        let TargetUnitOperation::Call { arguments, .. } = operation else {
                            unreachable!();
                        };
                        match corruption {
                            "sibling source" => arguments[0].source = arguments[1].source.clone(),
                            "offset" => arguments[0].source_byte_offset += 8,
                            "access" => arguments[0].access = StructuralAccess::MutableBorrow,
                            _ => unreachable!(),
                        }
                    },
                );
                assert!(
                    crate::validate_abstract_to_target_translation(&source, native, &changed)
                        .is_err(),
                    "{native:?}, {identity:?}, {corruption}"
                );
            }
        }
    }
}
