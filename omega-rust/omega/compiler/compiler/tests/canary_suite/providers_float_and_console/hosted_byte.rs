use super::*;
use compiler::CheckedCompileRequest;
use provider_planning::plans::CompilerIntrinsicExecutionIdentity;

#[test]
fn hosted_byte_physical_children_retain_target_operand_and_emission_custody() {
    let canary = pass_canary(fixture_roster::RUNTIME_CONSOLE_BYTE_LITERAL_EXIT);
    for target in ["linux_x86_64", "linux_arm64", "macos_arm64"] {
        let artifact = compile_rooted_backend_canary_without_output_for_target(&canary, target)
            .unwrap_or_else(|error| panic!("{target} byte output compiles: {error:?}"))
            .into_retained_native_artifact()
            .expect("complete native artifact");
        artifact.validate().expect("independent native replay");
        assert!(artifact.provider_executions().is_empty());
        let evidence = artifact
            .physical_evidence()
            .expect("complete physical evidence");
        assert_eq!(evidence.children().len(), 3, "two writes and one exit");
        let writes = evidence
            .children()
            .iter()
            .enumerate()
            .filter_map(|(position, child)| {
                let native::PhysicalChildParent::BoundaryTraitSettlement(parent) = child.parent()
                else {
                    return None;
                };
                (parent.execution()
                    == target_operations::BoundaryExecutionBinding::CompilerBuiltin(
                        target_operations::CompilerBuiltinExecution::HostedWriteByteI32,
                    ))
                .then_some((position, child, parent))
            })
            .collect::<Vec<_>>();
        assert_eq!(writes.len(), 2);
        assert_ne!(writes[0].1.occurrence(), writes[1].1.occurrence());
        for (_, child, parent) in &writes {
            assert_eq!(parent.target(), artifact.target());
            assert_eq!(child.projection(), evidence.projection().identity());
            assert_eq!(child.final_image_span(), child.object_span());
            if target == "macos_arm64" {
                assert_eq!(child.machine_span().byte_count(), 40);
            }
        }
        let write_position = writes[0].0;
        let identity = artifact.identity();
        let parts = artifact.into_parts();
        let replayed = native::NativeArtifact::from_replayed_parts(replay_parts(&parts))
            .expect("unchanged native parts replay");
        assert_eq!(replayed.identity(), identity);
        for mutation in 0..10 {
            let mut changed = replay_parts(&parts);
            let mut evidence = changed.physical_evidence.take().unwrap().into_parts();
            if mutation == 0 {
                evidence.children.remove(write_position);
            } else if mutation == 1 {
                evidence
                    .children
                    .push(evidence.children[write_position].clone());
            } else if mutation == 2 {
                changed.object.clear_fragment_replay_for_test();
            } else {
                let mut child = evidence.children[write_position].clone().into_parts();
                match mutation {
                    3 => {
                        child.object_span = native::NativeByteSpan::from_replayed_parts(
                            child.object_span.offset() + 1,
                            child.object_span.byte_count(),
                        )
                    }
                    4 => child.final_image_bytes_digest[0] ^= 1,
                    _ => {
                        let native::PhysicalChildParent::BoundaryTraitSettlement(parent) =
                            child.parent
                        else {
                            panic!("write retains its exact boundary parent")
                        };
                        let mut parent = parent.into_parts();
                        match mutation {
                            5 => {
                                parent.target = if target == "macos_arm64" {
                                    target::NativeTarget::linux_arm64()
                                } else {
                                    target::NativeTarget::macos_arm64()
                                }
                            }
                            6 => {
                                parent.selected_plan_digest =
                                    native::NativeSelectedProviderPlanDigest::from_digest([11; 32])
                            }
                            _ => {
                                let native::BoundaryTraitSettlementRole::CompilerBuiltinRuntimeScalar { scalar_argument, .. } = &mut parent.role else {
                                    panic!("write retains its runtime scalar argument")
                                };
                                let machine_code::InternalUnitScalarArgumentSourceRecord::SelectedBoundary {
                                    source_value, instruction, scratch_byte_offset, ..
                                } = &mut scalar_argument.source else {
                                    panic!("write input has selected boundary custody")
                                };
                                match mutation {
                                    7 => {
                                        *source_value = semantic_vocabulary::ValueId::new(
                                            source_value.get() + 100,
                                        )
                                        .unwrap()
                                    }
                                    8 => instruction.0 += 1,
                                    _ => *scratch_byte_offset += 1,
                                }
                            }
                        }
                        child.parent = native::PhysicalChildParent::BoundaryTraitSettlement(
                            native::BoundaryTraitSettlement::from_replayed_parts(parent),
                        );
                    }
                }
                evidence.children[write_position] =
                    native::NativePhysicalChild::from_replayed_parts(child);
            }
            changed.physical_evidence = Some(native::NativePhysicalEvidence::from_replayed_parts(
                evidence,
            ));
            assert!(
                native::NativeArtifact::from_replayed_parts(changed).is_err(),
                "{target} accepted mutation {mutation}"
            );
        }
    }
}

#[test]
fn hosted_byte_catalog_retains_exact_target_and_provider_custody() {
    let root = pass_canary(fixture_roster::RUNTIME_ADAPTER_FORWARDING_EXIT).join("main.omg");
    for target in ["linux_x86_64", "linux_arm64", "macos_arm64"] {
        let checked =
            compile_reviewed_repository_fixture(CheckedCompileRequest::new(&root, Some(target)))
                .expect("the target-selected Console provider checks");
        let (plan, retained) = checked
            .selected_provider_plans()
            .plans()
            .iter()
            .zip(checked.selected_provider_provenance())
            .find(|(plan, _)| plan.schema.trait_name == "Console")
            .expect("one selected Console plan");
        assert_eq!(plan.target, target);
        let byte_row = plan
            .rows
            .iter()
            .position(|row| row.method == "write_byte")
            .unwrap();
        assert_eq!(
            retained.row_compiler_intrinsic_executions[byte_row],
            Some(CompilerIntrinsicExecutionIdentity::HostedWriteByteI32),
            "{target}"
        );

        // A matching symbol or signature without the accepted package binding
        // must not manufacture the retained compiler-owned realization.
        for requested_target in [None, Some(target), Some("windows_x86_64")] {
            let projected =
                selected_dispatch::derive_selected_compiler_intrinsic_execution_identity_for_row(
                    &checked,
                    plan,
                    retained.provider.schema,
                    &plan.rows[byte_row],
                    retained.provider.row_requirements[byte_row],
                    retained.provider.row_realizations[byte_row],
                    requested_target,
                )
                .unwrap();
            assert_eq!(
                projected,
                Some(selected_dispatch::SelectedCompilerIntrinsicExecutionIdentity::Unsupported)
            );
        }
        if target == "macos_arm64" {
            let exit = plan
                .rows
                .iter()
                .position(|row| row.method == "exit_process")
                .unwrap();
            assert_eq!(
                retained.row_compiler_intrinsic_executions[exit],
                Some(CompilerIntrinsicExecutionIdentity::HostedExitProcessI32)
            );
            let read = plan
                .rows
                .iter()
                .position(|row| row.method == "read_byte")
                .unwrap();
            assert_eq!(
                retained.row_compiler_intrinsic_executions[read],
                Some(CompilerIntrinsicExecutionIdentity::HostedReadByte)
            );
            let line = plan
                .rows
                .iter()
                .position(|row| row.method == "read_line")
                .unwrap();
            assert_eq!(
                retained.row_compiler_intrinsic_executions[line], None,
                "native byte leaves do not admit the separate read_line catalog role"
            );
        }
    }
}
