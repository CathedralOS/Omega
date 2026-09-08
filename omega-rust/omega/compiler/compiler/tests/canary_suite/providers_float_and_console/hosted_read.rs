use super::*;

#[test]
fn hosted_read_returning_branches_replay_distinct_result_homes() {
    let canary = pass_canary(fixture_roster::RUNTIME_CONSOLE_BYTE_BRANCH_RETURN);
    for target in ["linux_x86_64", "linux_arm64", "macos_arm64"] {
        let artifact = compile_rooted_backend_canary_without_output_for_target(&canary, target)
            .unwrap_or_else(|error| panic!("{target} returning read branches compile: {error:?}"))
            .into_retained_native_artifact()
            .expect("complete native artifact");
        artifact.validate().expect("independent native replay");
        let results = artifact
            .object()
            .boundary_settlements()
            .iter()
            .filter(|row| {
                row.settlement.execution
                    == native_realization::BoundaryExecutionRecord::CompilerBuiltin(
                        target_operations::CompilerBuiltinExecution::HostedReadByte,
                    )
            })
            .map(|row| {
                row.settlement
                    .native_result
                    .structural()
                    .expect("owned read result")
            })
            .collect::<Vec<_>>();
        assert_eq!(
            results.len(),
            3,
            "one initial read and one read in each arm"
        );
        for (position, result) in results.iter().enumerate() {
            assert!(
                results[..position]
                    .iter()
                    .all(
                        |other| other.defining_operation != result.defining_operation
                            && other.result.place != result.result.place
                    )
            );
        }
        let identity = artifact.identity();
        let parts = artifact.into_parts();
        assert_eq!(
            native::NativeArtifact::from_replayed_parts(replay_parts(&parts))
                .expect("return-edge cleanup survives native parts replay")
                .identity(),
            identity
        );
    }
}

#[test]
fn hosted_read_returning_branches_execute_two_sequential_reads() {
    if !cfg!(any(
        all(target_os = "macos", target_arch = "aarch64"),
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        )
    )) {
        eprintln!("SKIP: returning byte branches require matching Linux or macOS ARM64 host");
        return;
    }
    use std::io::Seek;
    use std::process::Stdio;
    let canary = pass_canary(fixture_roster::RUNTIME_CONSOLE_BYTE_BRANCH_RETURN);
    let scratch =
        std::env::temp_dir().join(format!("omega-read-branch-return-{}", std::process::id()));
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("returning read branches publish a native executable");
    let executable = compilation
        .checked_native_executable_path()
        .expect("publication receipt");
    for input in [b"".as_slice(), b"A", b"ABC", &[0, 255, 128]] {
        let input_path = scratch.join("input.bin");
        fs::write(&input_path, input).expect("write exact input bytes");
        let mut supplied = fs::File::open(&input_path).expect("open input");
        let output = Command::new(executable)
            .stdin(Stdio::from(
                supplied.try_clone().expect("shared input cursor"),
            ))
            .output()
            .expect("returning branch execution");
        assert_eq!(output.status.code(), Some(0), "input {input:?}: {output:?}");
        assert_eq!(output.stdout, &input[..input.len().min(1)]);
        assert!(output.stderr.is_empty());
        assert_eq!(
            supplied.stream_position().unwrap(),
            input.len().min(2) as u64,
            "each selected branch must consume its second read exactly once"
        );
    }
    fs::remove_dir_all(scratch).expect("remove completed returning branch output");
}

#[test]
fn hosted_read_inspection_executes_every_byte_eof_and_failed_read() {
    if !cfg!(any(
        all(target_os = "macos", target_arch = "aarch64"),
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        )
    )) {
        eprintln!("SKIP: hosted byte input requires a matching Linux or macOS ARM64 host");
        return;
    }
    use std::io::Write;
    use std::process::Stdio;
    let canary = pass_canary(fixture_roster::RUNTIME_CONSOLE_BYTE_INSPECTION_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-hosted-read-{}", std::process::id()));
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("byte inspection compiles to a complete native product");
    // Publication consumes the artifact; the companion replay test retains
    // and mutates it separately. Execute only the checked publication receipt.
    let executable = compilation
        .checked_native_executable_path()
        .expect("executable receipt");
    for byte in 0..=u8::MAX {
        let mut child = Command::new(executable)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn byte inspection");
        child
            .stdin
            .take()
            .expect("stdin pipe")
            .write_all(&[byte])
            .expect("supply byte");
        let output = child.wait_with_output().expect("byte inspection finishes");
        assert_eq!(output.status.code(), Some(70), "byte {byte}: {output:?}");
        assert_eq!(output.stdout, [byte], "byte {byte}");
        assert!(output.stderr.is_empty());
    }
    let eof = Command::new(executable)
        .stdin(Stdio::null())
        .output()
        .expect("EOF execution");
    assert_eq!(eof.status.code(), Some(70));
    assert!(eof.stdout.is_empty());
    assert!(eof.stderr.is_empty());

    // A write-only descriptor is valid to inherit, but read(0, ...) must fail.
    // Failure must trap, not fabricate Eof or commit an owned Byte result.
    let unreadable = fs::OpenOptions::new()
        .write(true)
        .open("/dev/null")
        .expect("write-only null descriptor");
    let failed = Command::new(executable)
        .stdin(Stdio::from(unreadable))
        .output()
        .expect("failed-read execution");
    assert!(
        failed.status.code().is_none(),
        "failed read must trap: {failed:?}"
    );
    assert!(failed.stdout.is_empty());
    fs::remove_dir_all(scratch).expect("remove completed read fixture output");
}

#[test]
fn hosted_read_physical_children_retain_result_and_target_custody() {
    let canary = pass_canary(fixture_roster::RUNTIME_CONSOLE_BYTE_INSPECTION_EXIT);
    for target in ["linux_x86_64", "linux_arm64", "macos_arm64"] {
        let artifact = compile_rooted_backend_canary_without_output_for_target(&canary, target)
            .unwrap_or_else(|error| panic!("{target} byte inspection compiles: {error:?}"))
            .into_retained_native_artifact()
            .expect("complete native artifact");
        artifact.validate().expect("independent native replay");
        let evidence = artifact.physical_evidence().expect("physical evidence");
        let reads = evidence
            .children()
            .iter()
            .enumerate()
            .filter_map(|(position, child)| {
                let native::PhysicalChildParent::BoundaryTraitSettlement(parent) = child.parent()
                else {
                    return None;
                };
                matches!(
                    parent.role(),
                    native::BoundaryTraitSettlementRole::CompilerBuiltinStructural {
                        execution: target_operations::CompilerBuiltinExecution::HostedReadByte,
                        ..
                    }
                )
                .then_some((position, child, parent))
            })
            .collect::<Vec<_>>();
        assert_eq!(reads.len(), 1);
        let (position, child, parent) = reads[0];
        assert_eq!(parent.target(), artifact.target());
        assert_eq!(child.projection(), evidence.projection().identity());
        assert_eq!(child.object_span(), child.final_image_span());
        let identity = artifact.identity();
        let parts = artifact.into_parts();
        assert_eq!(
            native::NativeArtifact::from_replayed_parts(replay_parts(&parts))
                .expect("unchanged parts replay")
                .identity(),
            identity
        );
        for mutation in 0..12 {
            let mut changed = replay_parts(&parts);
            let mut evidence = changed.physical_evidence.take().unwrap().into_parts();
            match mutation {
                0 => {
                    evidence.children.remove(position);
                }
                1 => evidence.children.push(evidence.children[position].clone()),
                2 => changed.object.clear_fragment_replay_for_test(),
                _ => {
                    let mut child = evidence.children[position].clone().into_parts();
                    if mutation == 3 {
                        child.object_span = native::NativeByteSpan::from_replayed_parts(
                            child.object_span.offset() + 1,
                            child.object_span.byte_count(),
                        );
                    } else {
                        let native::PhysicalChildParent::BoundaryTraitSettlement(parent) =
                            child.parent
                        else {
                            panic!("read boundary parent")
                        };
                        let mut parent = parent.into_parts();
                        if mutation == 4 {
                            parent.target = if target == "macos_arm64" {
                                target::NativeTarget::linux_arm64()
                            } else {
                                target::NativeTarget::macos_arm64()
                            };
                        } else if mutation == 5 {
                            parent.selected_plan_digest =
                                native::NativeSelectedProviderPlanDigest::from_digest([11; 32]);
                        } else {
                            let native::BoundaryTraitSettlementRole::CompilerBuiltinStructural {
                                result,
                                ..
                            } = &mut parent.role
                            else {
                                panic!("read structural result")
                            };
                            match mutation {
                                6 => result.home_byte_offset += 4,
                                7 => result.layout.payload_byte_offset = 0,
                                8 => {
                                    result.defining_operation =
                                        semantic_vocabulary::OperationId::new(
                                            result.defining_operation.get() + 100,
                                        )
                                        .unwrap()
                                }
                                9 => {
                                    result.result.place = semantic_vocabulary::PlaceId::new(
                                        result.result.place.get() + 100,
                                    )
                                    .unwrap()
                                }
                                10 => result.layout.cases.swap(0, 1),
                                _ => {
                                    result.result.structural_type =
                                        semantic_vocabulary::StructuralTypeId::new(
                                            result.result.structural_type.get() + 100,
                                        )
                                        .unwrap()
                                }
                            }
                        }
                        child.parent = native::PhysicalChildParent::BoundaryTraitSettlement(
                            native::BoundaryTraitSettlement::from_replayed_parts(parent),
                        );
                    }
                    evidence.children[position] =
                        native::NativePhysicalChild::from_replayed_parts(child);
                }
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
