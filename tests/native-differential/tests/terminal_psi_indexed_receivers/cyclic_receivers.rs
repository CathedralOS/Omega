//! Ordinary looping callees keep projected receiver storage and caller continuation.
use super::*;

fn source(ranking: &str) -> String {
    format!(
        "data Child {{ value: u64; }}
             data Root {{ before: u64; child: Child; after: u64; }}
             machine Child::walk(&mut self, remaining: u64)
             {ranking}
             {{
                 self.value = remaining;
                 transition remaining > 0 {{
                     true -> walk(remaining - 1)
                     false -> done()
                 }}
                 state done(&mut self) {{}}
             }}
             machine forward(root: &mut Root, remaining: u64) {{
                 root.before = 17;
                 root.child.walk(remaining);
                 root.after = 42;
             }}"
    )
}

#[test]
fn ranked_and_unranked_projected_loops_publish_and_update_caller_storage() {
    for ranking in ["", "terminates by remaining -> Nat::Descending;"] {
        let source = source(ranking);
        for target in [
            NativeTarget::linux_x64(),
            NativeTarget::linux_arm64(),
            NativeTarget::macos_arm64(),
            NativeTarget::windows_x64(),
        ] {
            let placed = native_text(&source, target);
            let semantic_entry = placed.text_section().semantic_entry;
            let container = std::sync::Arc::new(
                object_file::stage_optimized_relocation_free_object_container(placed).unwrap(),
            );
            let object =
                image_emission::build_function_fragment_object_artifact(container).unwrap();
            let caller = object
                .functions()
                .iter()
                .find(|function| function.machine == semantic_entry)
                .unwrap();
            assert_eq!(caller.internal_unit_calls.len(), 1);
            let callee = object
                .functions()
                .iter()
                .find(|function| function.machine == caller.internal_unit_calls[0].target)
                .unwrap();
            let demand = image_emission::derive_stack_demand(&object, semantic_entry).unwrap();
            assert_eq!(
                demand.ceiling_bytes(),
                u64::from(
                    caller.unit_call_stacks[0].caller_live_bytes
                        + callee.unit_stack.unwrap().local_peak_bytes
                )
            );
            let (bytes, entry) = primitive_stores::published_text(&source, target);
            if target != NativeTarget::host() {
                continue;
            }
            #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
            {
                let code = native_execution::Code::new(&bytes);
                for count in 0..9 {
                    let mut frame = [0xa5a5_a5a5_a5a5_a5a5; 5];
                    frame[1..4].fill(0x3c3c_3c3c_3c3c_3c3c);
                    let record: &mut [u64; 3] = (&mut frame[1..4]).try_into().unwrap();
                    code.call_unit_with_record(entry, count, record);
                    assert_eq!(
                        frame,
                        [0xa5a5_a5a5_a5a5_a5a5, 17, 0, 42, 0xa5a5_a5a5_a5a5_a5a5]
                    );
                }
            }
            #[cfg(any(
                all(
                    target_os = "linux",
                    any(target_arch = "x86_64", target_arch = "aarch64")
                ),
                all(target_os = "macos", target_arch = "aarch64")
            ))]
            native_function::assert_c_text(
                &bytes,
                entry,
                r#"
                #include <stdint.h>
                #include <string.h>
                typedef struct { uint64_t value; } Child;
                typedef struct { uint64_t before; Child child; uint64_t after; } Root;
                extern void omega_entry(uint64_t remaining, Root *root);
                int main(void) {
                    for (uint64_t count = 0; count < 9; ++count) {
                        struct { uint64_t before; Root root; uint64_t after; } frame;
                        memset(&frame, 0xa5, sizeof frame);
                        frame.root.before = 17;
                        frame.root.child.value = 0;
                        frame.root.after = 42;
                        unsigned char expected[sizeof frame];
                        memcpy(expected, &frame, sizeof frame);
                        memset(&frame.root, 0x3c, sizeof frame.root);
                        omega_entry(count, &frame.root);
                        if (memcmp(expected, &frame, sizeof frame) != 0) return 1;
                    }
                    return 0;
                }
                "#,
            );
            #[cfg(not(any(
                all(target_os = "windows", target_arch = "x86_64"),
                all(
                    target_os = "linux",
                    any(target_arch = "x86_64", target_arch = "aarch64")
                ),
                all(target_os = "macos", target_arch = "aarch64")
            )))]
            {
                let _ = (bytes, entry);
                eprintln!(
                    "SKIP: cyclic receiver execution requires a supported Linux or macOS host"
                );
            }
        }
    }
}

#[test]
fn cyclic_record_field_store_replay_rejects_type_access_and_source_substitution() {
    let optimized = optimize(&artifact(&source(
        "terminates by remaining -> Nat::Descending;",
    )));
    let target = abstract_operations_to_target_operations::lower_to_target_operations(
        optimized.plan(),
        NativeTarget::windows_x64(),
    )
    .unwrap();
    let legalized = target_operations_to_selected_instructions::legalize_target_operations(
        &target,
        optimized.plan(),
        &optimized,
    )
    .unwrap();
    for mutation in 0..5 {
        let mut candidate = target.clone();
        let store = candidate.functions.iter_mut().find_map(|function| {
            let graph = &mut function.graph;
            graph.blocks.iter_mut().flat_map(|block| &mut block.operations)
                .find(|operation| matches!(operation, target_operations::TargetUnitOperation::StructuralScalarFieldStore { .. }))
        }).unwrap();
        let target_operations::TargetUnitOperation::StructuralScalarFieldStore {
            destination,
            field,
            field_byte_offset,
            source,
            ..
        } = store
        else {
            unreachable!();
        };
        match mutation {
            0 => destination.access = terminal_psi::StructuralAccess::SharedBorrow,
            1 => {
                destination.structural_type =
                    semantic_vocabulary::StructuralTypeId::new(999).unwrap()
            }
            2 => *field = semantic_vocabulary::StructuralFieldId::new(999).unwrap(),
            3 => *field_byte_offset += 1,
            4 => {
                let target_operations::TargetUnitScalarArgumentSource::BlockParameter(parameter) =
                    source
                else {
                    panic!("loop-carried source");
                };
                parameter.scalar_type = semantic_vocabulary::ScalarType::Boolean;
            }
            _ => unreachable!(),
        }
        assert!(
            target_operations_to_selected_instructions::validate_legalized_operations(
                &candidate,
                optimized.plan(),
                &optimized,
                legalized.plan().clone(),
            )
            .is_err(),
            "mutation {mutation}"
        );
    }
}
