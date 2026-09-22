//! Fixtures shared by the value call and dispatch canaries.

#[path = "value_calls_and_dispatch/dispatch_and_slice_calls.rs"]
mod dispatch_and_slice_calls;
#[path = "value_calls_and_dispatch/dynamic_and_receiver_calls.rs"]
mod dynamic_and_receiver_calls;
#[path = "../fixture_rosters/value_calls_and_dispatch.rs"]
pub(super) mod fixture_roster;
#[path = "value_calls_and_dispatch/runtime_value_calls.rs"]
mod runtime_value_calls;

use crate::{
    Command, CompileReport, compile_reviewed_repository_fixture,
    compile_rooted_backend_canary_without_output_for_target_and_permission_policy,
    compile_rooted_canary_for_native_host, fs, pass_canary,
};
use compiler::CheckedCompileRequest;

fn assert_native_exit_code(
    report: &CompileReport,
    expected: i32,
    fixture: &str,
    expectation: &str,
) {
    let executable = report
        .checked_native_executable_path()
        .unwrap_or_else(|| panic!("{fixture} lost its exact executable publication receipt"));
    let output = Command::new(executable)
        .output()
        .unwrap_or_else(|error| panic!("{fixture} should run: {error}"));
    assert_eq!(
        output.status.code(),
        Some(expected),
        "{expectation}; expected exit {expected}, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_forwarded_dynamic_result_canary(
    fixture: &str,
    expected_transfer_count: usize,
    description: &str,
    native_expectation: &str,
) {
    let _ = native_expectation;
    let canary = pass_canary(fixture);
    for target in ["linux_x86_64", "linux_arm64"] {
        let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
            &canary.join("main.omg"),
            Some(target),
        ))
        .unwrap_or_else(|diagnostics| {
            panic!(
                "{description} should reach checked descriptor custody for {target}:\n{}",
                diagnostics
                    .iter()
                    .map(|diagnostic| diagnostic.message.as_str())
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        });
        assert_eq!(
            checked
                .facts
                .flow
                .terminal_unit_effects
                .dynamic_dispatch
                .transfers
                .len(),
            expected_transfer_count
        );
        assert_eq!(
            checked
                .facts
                .flow
                .terminal_unit_effects
                .dynamic_dispatch
                .rebound_scalar_calls
                .len(),
            1
        );
        let permission_policy = native_realization::terminal_authority_permission_policy_with_rows(
            checked
                .selected_provider_plans()
                .plans()
                .iter()
                .flat_map(|plan| {
                    plan.rows
                        .iter()
                        .filter(|&row| {
                            matches!(
                                row.binding,
                                effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. }
                            )
                        })
                        .map(|row| {
                            native_realization::TerminalAuthorityPermissionPolicyRow::new(
                                plan.schema.identity_digest(),
                                row.requirement_identity.clone(),
                                effects::TerminalAuthorityDisposition::from_classes([
                                    effects::TerminalAuthorityClass::ProcessTermination,
                                ]),
                            )
                        })
                })
                .collect(),
        )
        .expect("exact Console exit permission policy");
        let report = compile_rooted_backend_canary_without_output_for_target_and_permission_policy(
            &canary,
            target,
            permission_policy,
        )
        .unwrap_or_else(|diagnostics| {
            panic!(
                "{target} should retain {description} through its exact adapter:\n{}",
                diagnostics
                    .iter()
                    .map(|diagnostic| diagnostic.message.as_str())
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        });
        if fixture == fixture_roster::RUNTIME_LOCAL_NAMED_DYN_BOOLEAN_PASS_THROUGH_EXIT {
            assert_boolean_forwarded_native_custody(&report, target);
        } else if fixture == fixture_roster::RUNTIME_LOCAL_NAMED_DYN_MUTABLE_PASS_THROUGH_EXIT {
            let native = report
                .retained_native_artifact()
                .expect("fixed-integer mutation keeps native custody");
            let object = native.object();
            let stores = object
                .functions()
                .iter()
                .flat_map(|function| &function.scalar_structural_scalar_field_stores)
                .collect::<Vec<_>>();
            let [integer_store, boolean_store, short_store] = stores.as_slice() else {
                panic!("{target} should retain three ordered realization stores")
            };
            assert!(integer_store.path.is_empty());
            assert_eq!(integer_store.field_byte_offset, 0);
            assert!(matches!(
                integer_store.immediate,
                target_operations::TargetScalarImmediate::Integer {
                    scalar_type,
                    value: semantic_vocabulary::IntegerValue::Unsigned(513),
                } if scalar_type == semantic_vocabulary::IntegerType::new(
                    semantic_vocabulary::IntegerSign::Unsigned,
                    64,
                ).expect("u64 type")
            ));
            assert!(boolean_store.path.is_empty());
            assert_eq!(boolean_store.field_byte_offset, 8);
            assert!(matches!(
                boolean_store.immediate,
                target_operations::TargetScalarImmediate::Boolean(true)
            ));
            assert!(short_store.path.is_empty());
            assert_eq!(short_store.field_byte_offset, 10);
            assert!(matches!(
                short_store.immediate,
                target_operations::TargetScalarImmediate::Integer {
                    scalar_type,
                    value: semantic_vocabulary::IntegerValue::Unsigned(257),
                } if scalar_type == semantic_vocabulary::IntegerType::new(
                    semantic_vocabulary::IntegerSign::Unsigned,
                    16,
                ).expect("u16 type")
            ));
            assert!(integer_store.operation_ordinal < boolean_store.operation_ordinal);
            assert!(boolean_store.operation_ordinal < short_store.operation_ordinal);
            assert_eq!(
                boolean_store.code_offset,
                integer_store.code_offset + integer_store.byte_count
            );
            assert_eq!(
                short_store.code_offset,
                boolean_store.code_offset + boolean_store.byte_count
            );
        } else if fixture
            == fixture_roster::RUNTIME_LOCAL_NAMED_DYN_MUTABLE_PROJECTED_BOOLEAN_PASS_THROUGH_EXIT
        {
            let object = report
                .retained_native_artifact()
                .expect("nested mutation keeps native custody")
                .object();
            let stores = object
                .functions()
                .iter()
                .flat_map(|function| &function.scalar_structural_scalar_field_stores)
                .collect::<Vec<_>>();
            let [store] = stores.as_slice() else {
                panic!("{target} should retain one nested realization store")
            };
            assert_eq!(
                store.path,
                [
                    terminal_psi::StructuralPathSegment::Field("envelope".into()),
                    terminal_psi::StructuralPathSegment::Field("flags".into()),
                ]
            );
            assert_eq!(store.field_byte_offset, 8);
        } else if fixture == fixture_roster::RUNTIME_LOCAL_NAMED_DYN_MULTI_HOP_PASS_THROUGH_EXIT {
            let object = report
                .retained_native_artifact()
                .expect("multi-hop forwarding keeps native custody")
                .object();
            assert_eq!(
                object
                    .functions()
                    .iter()
                    .flat_map(|function| &function.forwarded_dynamic_descriptor_calls)
                    .count(),
                1,
                "{target} must retain the selection-sourced descriptor call"
            );
            assert_eq!(
                object
                    .functions()
                    .iter()
                    .flat_map(|function| &function.forwarded_dynamic_parameter_calls)
                    .count(),
                1,
                "{target} must retain the intermediate parameter-forwarding direct call"
            );
            assert_eq!(
                object
                    .functions()
                    .iter()
                    .flat_map(|function| &function.dynamic_parameter_calls)
                    .count(),
                1,
                "{target} must retain the final parameter-sourced indirect call"
            );
        }
    }
    #[cfg(target_os = "linux")]
    {
        let build_dir = std::env::temp_dir().join(format!(
            "omega-{}-{}",
            fixture.replace(['/', '_'], "-"),
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&build_dir);

        let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
            .unwrap_or_else(|diagnostics| {
                panic!(
                    "{description} should emit its exact private table function:\n{}",
                    diagnostics
                        .iter()
                        .map(|diagnostic| diagnostic.message.as_str())
                        .collect::<Vec<_>>()
                        .join("\n")
                )
            });
        assert_native_exit_code(&compilation, 70, description, native_expectation);

        let _ = fs::remove_dir_all(&build_dir);
    }
}

fn assert_boolean_forwarded_native_custody(report: &CompileReport, target: &str) {
    let native = report
        .retained_native_artifact()
        .expect("Boolean forwarded result keeps native custody");
    let object = native.object();
    let forwarded = object
        .functions()
        .iter()
        .filter_map(|function| {
            let [call] = function.forwarded_dynamic_descriptor_calls.as_slice() else {
                return None;
            };
            Some((function, call))
        })
        .collect::<Vec<_>>();
    let [(caller, call)] = forwarded.as_slice() else {
        panic!("{target} should retain one forwarded Boolean result call")
    };
    let semantic_result = call.semantic_result.expect("Boolean semantic result");
    let result = call.result.as_ref().expect("Boolean physical result");
    assert_eq!(
        semantic_result.scalar_type,
        semantic_vocabulary::ScalarType::Boolean
    );
    assert_eq!(
        result.home.scalar_type,
        semantic_vocabulary::ScalarType::Boolean
    );
    assert_eq!(
        result.home.shape,
        calling_conventions::ValueShape::integer(1, 1)
    );
    assert!(object.functions().iter().any(|function| {
        function
            .mixed_structural_scalar_abi
            .as_ref()
            .is_some_and(|abi| abi.result.scalar_type == semantic_vocabulary::ScalarType::Boolean)
    }));
    let branch_offset = call.code_offset + call.byte_count;
    let bytes = caller.bytes(object);
    match target {
        "linux_x86_64" => assert!(
            bytes[branch_offset..]
                .windows(3)
                .next()
                .is_some_and(|prefix| prefix == [0x40, 0x0f, 0xb6])
                && bytes[branch_offset..branch_offset + 20]
                    .windows(4)
                    .any(|window| window == [0x84, 0xc0, 0x0f, 0x84]),
            "x86-64 must load the one-byte home, test AL, and branch false on zero"
        ),
        "linux_arm64" => {
            let compare = u32::from_le_bytes(
                bytes[branch_offset + 4..branch_offset + 8]
                    .try_into()
                    .expect("AArch64 compare word"),
            );
            let branch = u32::from_le_bytes(
                bytes[branch_offset + 8..branch_offset + 12]
                    .try_into()
                    .expect("AArch64 branch word"),
            );
            assert_eq!(compare, 0x7100_013f, "AArch64 must compare w9 with zero");
            assert_eq!(
                branch & 0xff00_001f,
                0x5400_0000,
                "AArch64 must branch false with b.eq"
            );
        }
        _ => unreachable!("bounded target roster"),
    }
}
