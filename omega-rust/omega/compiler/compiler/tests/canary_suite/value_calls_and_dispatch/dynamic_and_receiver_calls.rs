use super::assert_forwarded_dynamic_result_canary;
use super::assert_native_exit_code;
use super::fixture_roster;
use crate::{
    CanaryCompileProduct, CanaryCompileSpec, Command, compile, compile_reviewed_repository_fixture,
    compile_rooted_backend_canary_without_output_for_target,
    compile_rooted_backend_canary_without_output_for_target_and_permission_policy,
    compile_rooted_canary_for_native_host, executable_name, fs, interpret, pass_canary,
};
use compiler::CheckedCompileRequest;

#[test]
fn runtime_local_named_dyn_mutable_pass_through_exit_canary_runs() {
    assert_forwarded_dynamic_result_canary(
        fixture_roster::RUNTIME_LOCAL_NAMED_DYN_MUTABLE_PASS_THROUGH_EXIT,
        1,
        "mutable forwarded named dynamic descriptor canary",
        "the indirect slot must preserve exclusive access and select the rebound Item instance",
    );
}

#[test]
fn runtime_local_named_dyn_boolean_pass_through_exit_canary_runs() {
    assert_forwarded_dynamic_result_canary(
        fixture_roster::RUNTIME_LOCAL_NAMED_DYN_BOOLEAN_PASS_THROUGH_EXIT,
        1,
        "forwarded named dynamic Boolean result canary",
        "the indirect slot must preserve the selected true result through its durable Boolean home",
    );
}

#[test]
fn runtime_local_named_dyn_mutable_boolean_pass_through_exit_canary_runs() {
    assert_forwarded_dynamic_result_canary(
        fixture_roster::RUNTIME_LOCAL_NAMED_DYN_MUTABLE_BOOLEAN_PASS_THROUGH_EXIT,
        1,
        "mutable forwarded named dynamic Boolean descriptor canary",
        "the indirect slot must store true into the rebound Item before returning its independent code field",
    );
}

#[test]
fn runtime_local_named_dyn_mutable_projected_boolean_pass_through_exit_canary_runs() {
    assert_forwarded_dynamic_result_canary(
        fixture_roster::RUNTIME_LOCAL_NAMED_DYN_MUTABLE_PROJECTED_BOOLEAN_PASS_THROUGH_EXIT,
        1,
        "projected mutable forwarded named dynamic Boolean descriptor canary",
        "the indirect slot must store true through the nested Envelope/Flags path before returning the selected Item code",
    );
}

#[test]
fn runtime_local_named_dyn_multi_hop_pass_through_exit_canary_runs() {
    assert_forwarded_dynamic_result_canary(
        fixture_roster::RUNTIME_LOCAL_NAMED_DYN_MULTI_HOP_PASS_THROUGH_EXIT,
        2,
        "multi-hop forwarded named dynamic descriptor canary",
        "both unchanged descriptor handoffs must reach the selected Item and return 99 to the caller's exit diamond",
    );
}

#[test]
fn runtime_local_named_dyn_unit_multi_hop_return_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_LOCAL_NAMED_DYN_UNIT_MULTI_HOP_RETURN);
    for target in ["linux_x86_64", "linux_arm64"] {
        let report = compile_rooted_backend_canary_without_output_for_target(&canary, target)
            .unwrap_or_else(|diagnostics| {
                panic!(
                    "Unit descriptor chain should reach native custody for {target}:\n{}",
                    diagnostics
                        .iter()
                        .map(|diagnostic| diagnostic.message.as_str())
                        .collect::<Vec<_>>()
                        .join("\n")
                )
            });
        let object = report
            .retained_native_artifact()
            .expect("Unit forwarding keeps native custody")
            .object();
        assert_eq!(
            object
                .functions()
                .iter()
                .flat_map(|function| &function.forwarded_dynamic_descriptor_calls)
                .count(),
            1,
            "{target} must retain the selection-sourced Unit descriptor call"
        );
        let forwarded = object
            .functions()
            .iter()
            .flat_map(|function| &function.forwarded_dynamic_parameter_calls)
            .collect::<Vec<_>>();
        let [call] = forwarded.as_slice() else {
            panic!("{target} should retain one Unit parameter-forwarding call")
        };
        assert!(call.source_value.is_none());
        assert!(call.scalar_type.is_none());
        assert!(matches!(
            call.call_stack,
            machine_code::ForwardedDynamicParameterCallStackEvidence::Unit(_)
        ));
        assert_eq!(
            object
                .functions()
                .iter()
                .flat_map(|function| &function.dynamic_parameter_calls)
                .count(),
            1,
            "{target} must retain the final Unit parameter-sourced indirect call"
        );
    }
    #[cfg(target_os = "linux")]
    {
        let build_dir = std::env::temp_dir().join(format!(
            "omega-runtime-local-named-dyn-unit-multi-hop-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&build_dir);
        let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
            .expect("Unit descriptor chain should emit a native image");
        assert_native_exit_code(
            &compilation,
            0,
            "Unit descriptor multi-hop canary",
            "the result-less descriptor must cross both helpers and return normally",
        );
        let _ = fs::remove_dir_all(&build_dir);
    }
}

#[test]
fn runtime_local_named_dyn_rebound_direct_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_LOCAL_NAMED_DYN_REBOUND_DIRECT_EXIT);
    for target in ["linux_x86_64", "linux_arm64"] {
        let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
            &canary.join("main.omg"),
            Some(target),
        ))
        .expect("rebound fixture should reach checked provider custody");
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
        compile_rooted_backend_canary_without_output_for_target_and_permission_policy(
            &canary,
            target,
            permission_policy,
        )
        .unwrap_or_else(|diagnostics| {
            panic!(
                "{target} should lower the rebound dynamic call and its exact exit diamond:\n{}",
                diagnostics
                    .iter()
                    .map(|diagnostic| diagnostic.message.as_str())
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        });
    }

    // The closed compiler-builtin catalog currently realizes exit_process on
    // Linux. Keep the machine-code construction checks above cross-target and
    // execute the artifact on Linux hosts where that exact settlement exists.
    #[cfg(target_os = "linux")]
    {
        let build_dir = std::env::temp_dir().join(format!(
            "omega-runtime-local-named-dyn-rebound-direct-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&build_dir);

        let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
            .expect("a rebound named dynamic value should emit its exact private table function");
        assert_native_exit_code(
            &compilation,
            70,
            "rebound named dynamic descriptor canary",
            "the indirect slot must execute against the rebound Item instance, not the original decoy",
        );

        let _ = fs::remove_dir_all(&build_dir);
    }
}

#[test]
fn runtime_local_named_dyn_stored_return_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_LOCAL_NAMED_DYN_STORED_RETURN);
    for target in ["linux_x86_64", "linux_arm64"] {
        let report = compile_rooted_backend_canary_without_output_for_target(&canary, target)
            .unwrap_or_else(|diagnostics| {
                panic!(
                    "{target} should lower the stored descriptor call:\n{}",
                    diagnostics
                        .iter()
                        .map(|diagnostic| diagnostic.message.as_str())
                        .collect::<Vec<_>>()
                        .join("\n")
                )
            });
        assert_eq!(
            report
                .retained_native_artifact()
                .expect("stored descriptor keeps native custody")
                .object()
                .functions()
                .iter()
                .flat_map(|function| &function.stored_dynamic_calls)
                .count(),
            1,
            "{target} must retain the stored descriptor call"
        );
    }

    #[cfg(target_os = "linux")]
    {
        let build_dir = std::env::temp_dir().join(format!(
            "omega-runtime-local-named-dyn-stored-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&build_dir);
        let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
            .expect("a stored named dynamic value should emit a native image");
        assert_native_exit_code(
            &compilation,
            0,
            "stored named dynamic descriptor canary",
            "the later field reload and indirect call must return normally",
        );
        let _ = fs::remove_dir_all(&build_dir);
    }
}

#[test]
fn runtime_local_named_dyn_stored_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_LOCAL_NAMED_DYN_STORED_EXIT);
    for target in ["linux_x86_64", "linux_arm64"] {
        let report = compile_rooted_backend_canary_without_output_for_target(&canary, target)
            .unwrap_or_else(|diagnostics| {
                panic!(
                    "{target} should lower stored descriptor result control:\n{}",
                    diagnostics
                        .iter()
                        .map(|diagnostic| diagnostic.message.as_str())
                        .collect::<Vec<_>>()
                        .join("\n")
                )
            });
        assert_eq!(
            report
                .retained_native_artifact()
                .expect("stored descriptor control keeps native custody")
                .object()
                .functions()
                .iter()
                .flat_map(|function| &function.stored_dynamic_calls)
                .count(),
            1,
            "{target} must retain the stored descriptor result call"
        );
    }

    #[cfg(target_os = "linux")]
    {
        let build_dir = std::env::temp_dir().join(format!(
            "omega-runtime-local-named-dyn-stored-exit-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&build_dir);
        let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
            .expect("stored named dynamic result control should emit a native image");
        assert_native_exit_code(
            &compilation,
            70,
            "stored named dynamic descriptor result canary",
            "the reloaded descriptor result must drive the good exit arm",
        );
        let _ = fs::remove_dir_all(&build_dir);
    }
}

#[test]
fn runtime_dyn_two_impl_dispatch_exit_canary_runs() {
    // TWO data types satisfy Shape, so the `&mut dyn Shape` receiver cannot
    // devirtualize; the call is monomorphized over the trait's closed world and
    // each call site's receiver type picks the impl: Circle::code() == 9 then
    // Square::code() == 4 -> n == 94 -> exit 70. Mirrors the interpreter
    // coverage test dyn_two_impl_dispatch_selects_impl_by_runtime_type.
    let canary = pass_canary(fixture_roster::RUNTIME_DYN_TWO_IMPL_DISPATCH_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-dyn-two-impl-dispatch-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("dyn two-impl dispatch canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("dyn two-impl dispatch canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("dyn two-impl dispatch canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `&mut dyn Shape` with TWO impls to dispatch by the call site's \
         receiver type (Circle 9, Square 4 -> n == 94 -> exit 70); an unresolved \
         dyn call returns 0 for both (exit 71). got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_dyn_two_impl_dispatch_swapped_exit_canary_runs() {
    // Same two impls, call order swapped: Square (4) first, then Circle (9)
    // -> n == 49 -> exit 70. A dispatcher that always picks the lexically-first
    // impl cannot pass both this and the unswapped canary (it scores 99 twice).
    let canary = pass_canary(fixture_roster::RUNTIME_DYN_TWO_IMPL_DISPATCH_SWAPPED_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-dyn-two-impl-dispatch-swapped-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("dyn two-impl swapped dispatch canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("dyn two-impl swapped dispatch canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("dyn two-impl swapped dispatch canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the swapped call order to dispatch Square (4) then Circle (9) \
         -> n == 49 -> exit 70. got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_alias_write_through_guarded_transition_exit_canary_runs() {
    // A `&mut` param forwarded through a GUARDED transition into a sub-state that
    // writes through it must reach the caller's object. When the callee inlines as a
    // branching leaf, by-value args bind as `mut <literal>` (e.g. `mut 2`), so the
    // leaf guard `key < 4` carried a `Mutable(Integer)` operand the value resolvers
    // didn't see through -- the arm (guard + its alias write) was dropped entirely.
    let canary = pass_canary(fixture_roster::RUNTIME_ALIAS_WRITE_THROUGH_GUARDED_TRANSITION_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-alias-write-guarded-transition-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("alias-write-through-guarded-transition canary should compile");

    let executable = compilation.checked_native_executable_path().expect(
        "alias-write-through-guarded-transition canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("alias-write-through-guarded-transition canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a &mut alias written in a sub-state reached via a guarded transition \
         to reach the caller (exit 70), got {:?} (71 = the guarded arm's alias write was \
         dropped)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_reference_param_forwarded_through_loop_exit_canary_runs() {
    // Forwarding a `&mut` param to another `&mut` param through a (self-looping)
    // dispatch transition must copy the POINTER VALUE, not the pointee. The materializer
    // dereferenced whenever the referent size equalled the target slot size; for a
    // pointer-sized referent it wrote room data into the pointer slot, so the next write
    // through it faulted. The deref branch now fires only for VALUE targets.
    let canary = pass_canary(fixture_roster::RUNTIME_REFERENCE_PARAM_FORWARDED_THROUGH_LOOP_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-reference-param-forwarded-loop-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("reference-param-forwarded-through-loop canary should compile");

    let executable = compilation.checked_native_executable_path().expect(
        "reference-param-forwarded-through-loop canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("reference-param-forwarded-through-loop canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a &mut param forwarded to another &mut param through a loop to copy the \
         pointer value (exit 70), got {:?} (139/segfault = pointee written into pointer slot)\
         \nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_value_call_through_alias_in_dispatch_exit_canary_runs() {
    // A value-returning inline-branching call written through a `&mut` alias inside a
    // DISPATCHED callee must yield the matched arm's value. The guard's forward-skip
    // distance must cover the per-arm pointee copy; `is_guarded_effect` was missing
    // RuntimeStorageCopyToRuntimePointee / RuntimePointee{Integer,Binary}Write, so a
    // skipped arm ran the pointee copy unconditionally and stranded the matched arm.
    let canary = pass_canary(fixture_roster::RUNTIME_VALUE_CALL_THROUGH_ALIAS_IN_DISPATCH_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-value-call-alias-dispatch-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("value-call-through-alias-in-dispatch canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("value-call-through-alias-in-dispatch canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("value-call-through-alias-in-dispatch canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a branching-call result written through a &mut alias in a dispatched callee \
         to yield the matched arm (exit 70), got {:?} (71 = a skipped arm's pointee copy ran \
         unconditionally and stranded the match)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_nested_value_call_in_substate_exit_canary_runs() {
    // Two-level nested call from a sub-state: hall1 -> carve (statement) -> room_mut
    // (in `let x = self.f()` position). carve was misclassified as a leaf (leaf check
    // only scanned `OperationKind::Call` ops, missing the call in a `let` initializer),
    // so its nested room_mut was dropped -> null `&mut Room` -> fault. The classifier
    // now treats a state that sources any non-host call as non-leaf (InlineExpansion).
    let canary = pass_canary(fixture_roster::RUNTIME_NESTED_VALUE_CALL_IN_SUBSTATE_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-nested-value-call-substate-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("nested-value-call-in-substate canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("nested-value-call-in-substate canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("nested-value-call-in-substate canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a 2-level nested call (sub-state -> helper -> value-position call) to be \
         expanded (exit 70), got {:?} (139/71 = the helper was treated as a leaf and its \
         nested call dropped)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_call_in_inlined_substate_exit_canary_runs() {
    // A transition target (sub-state) that calls in `let x = self.f()` position must
    // lower as a straight-line branch so the nested call is expanded. It was
    // misclassified as a leaf (leaf check looked only for Statement-role calls), and
    // leaf expansion can't carry a nested call -> the call was dropped, leaving its
    // `&mut`/value result null -> the next use faulted. Dungeon generation shape.
    let canary = pass_canary(fixture_roster::RUNTIME_CALL_IN_INLINED_SUBSTATE_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-call-in-inlined-substate-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("call-in-inlined-substate canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("call-in-inlined-substate canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("call-in-inlined-substate canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a `let x = self.f()` call in a transition sub-state to be expanded (exit 70), \
         got {:?} (139/71 = the sub-state was treated as a leaf and the call dropped)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_alias_indexed_read_through_transition_exit_canary_runs() {
    // An inlined leaf reading `items[key].field` (constant-index element of a
    // forwarded slice) through a forwarded `&mut` alias: the inlined by-value `key`
    // binds as `mut 2`, so `items[key]` became `items[mut 2]` and the index-path
    // resolvers rejected the `Mutable`-wrapped index, dropping the copy. The `mut`
    // is now stripped on a resolved leaf index. Mirrors the dungeon find_room shape.
    let canary = pass_canary(fixture_roster::RUNTIME_ALIAS_INDEXED_READ_THROUGH_TRANSITION_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-alias-indexed-read-transition-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("alias-indexed-read-through-transition canary should compile");

    let executable = compilation.checked_native_executable_path().expect(
        "alias-indexed-read-through-transition canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("alias-indexed-read-through-transition canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `items[key].field` (constant index) copied through a &mut alias in a \
         guarded sub-state to resolve (exit 70), got {:?} (71 = `mut`-wrapped index rejected)\
         \nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_dispatch_binary_call_argument_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_DISPATCH_BINARY_CALL_ARGUMENT_EXIT);
    let main_path = canary.join("main.omg");
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("binary-local call-argument canary should compile to checked trees");
    let interpreted = interpret(&checked, &[]);
    assert_eq!(interpreted.error, None);
    assert_eq!(interpreted.exit_code, 70);

    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-dispatch-binary-call-arg-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime dispatch binary call argument canary should compile");

    let executable = compilation.checked_native_executable_path().expect(
        "runtime dispatch binary call argument canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("runtime dispatch binary call argument canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a binary-initialized local passed as a call argument (`carve(level, tag)`) \
         from the proof-bearing guarded state to copy its slot and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// A dispatched value call whose result binds to a FIELD (no frame result
// slot): the return-write resolves the caller's Assignment target to its
// machine-region place. Was a live silent-wrong (field stayed ZII).

#[test]
fn runtime_dispatch_result_field_binding_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_DISPATCH_RESULT_FIELD_BINDING_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-dispatch-result-field-binding-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("dispatch result field-binding canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("dispatch result field-binding canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("dispatch result field-binding canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the dispatched call's terminal to write the caller's FIELD \
         (self.total == 5 -> exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// A threaded mutable receiver crosses two calls, an outer guard, and a
// trailing-state mutation. Each transition guard must observe its authored
// phase rather than a flattened descendant predicate.

#[test]
fn runtime_trailing_state_mut_param_phase_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_TRAILING_STATE_MUT_PARAM_PHASE_EXIT);
    let main_path = canary.join("main.omg");
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("threaded mutable receiver phase canary should compile to checked trees");
    let interpreted = interpret(&checked, &[]);
    assert_eq!(interpreted.error, None);
    assert_eq!(interpreted.exit_code, 70);

    let build_dir = std::env::temp_dir().join(format!(
        "omega-trailing-mut-param-phase-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile(CanaryCompileSpec {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("threaded mutable receiver phase canary should compile natively");
    let executable = compilation
        .checked_native_executable_path()
        .expect("threaded mutable receiver phase canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("threaded mutable receiver phase canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected outer and trailing guards to observe calls 2 and 3 (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

// The ORIGINAL same-type receiver aliasing repro, now serving:
// self.b.increment() mutates b (was: mutated a via by-type resolution).

#[test]
fn runtime_same_type_second_receiver_mutation_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_SAME_TYPE_SECOND_RECEIVER_MUTATION_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-same-type-second-receiver-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("second-receiver mutation canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("second-receiver mutation canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("second-receiver mutation canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected b.value == 1 after a x1 + b x1 (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

// A dispatched FLOAT place terminal round-trips through the result slot
// (the type-agnostic place-copy serve; the old "floats bail" row was
// stale). Float BINARY terminals remain unserved.

#[test]
fn runtime_dispatch_float_terminal_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_DISPATCH_FLOAT_TERMINAL_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-dispatch-float-terminal-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("dispatch float terminal canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("dispatch float terminal canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the float place terminal to round-trip 1.5 (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

// std::time Duration core through NON-FIRST same-type receivers (the
// inline-route per-instance fix): sum.checked_subtract resolves against
// sum's storage, not the first Duration's.

#[test]
fn runtime_value_machine_receiver_field_postentry_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_VALUE_MACHINE_RECEIVER_FIELD_POSTENTRY_EXIT);
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        None,
    ))
    .expect("receiver-field postentry canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(outcome.error, None, "should interpret cleanly");
    assert_eq!(outcome.exit_code, 70, "interp oracle should exit 70");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-receiver-field-postentry-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("receiver-field postentry canary should compile natively");
    let executable = compilation
        .checked_native_executable_path()
        .expect("receiver-field postentry canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("receiver-field postentry canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected exact Duration math through the third same-type receiver (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

// A method through the SECOND same-type NESTED leaf (self.p.b.get())
// reads b's 9, not the first leaf's 5 (the inline nested-receiver fix).

#[test]
fn runtime_nested_receiver_same_type_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_NESTED_RECEIVER_SAME_TYPE_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-nested-receiver-same-type-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("nested same-type receiver canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("nested same-type receiver canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("nested same-type receiver canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected p.b.get() == 9 (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

// PER-INSTANCE receiver dispatch: a looping value machine called through
// the SECOND same-type contained receiver runs on that receiver's storage
// (21 = 3 iterations of second.count 7; by-type resolution read first's
// 100 and delivered 300).

#[test]
fn runtime_dispatch_second_receiver_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_DISPATCH_SECOND_RECEIVER_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-dispatch-second-receiver-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("second-receiver dispatch canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("second-receiver dispatch canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("second-receiver dispatch canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the SECOND receiver's storage to drive the loop (21 -> exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_dispatch_sibling_value_calls_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_DISPATCH_SIBLING_VALUE_CALLS_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-dispatch-sibling-value-calls-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("sibling dispatched value calls should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("sibling dispatched value calls should retain their executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("sibling dispatched value calls should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected both dispatched calls to retain receiver/result identity (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_inline_repeated_receiver_value_calls_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_INLINE_REPEATED_RECEIVER_VALUE_CALLS_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-inline-repeated-receiver-value-calls-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("repeated inline value calls on one receiver should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("repeated inline value calls should retain their executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("repeated inline value calls on one receiver should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected each inline call occurrence to retain its result slot (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

// RECEIVER SLICE 2: the second-receiver dispatch shape with a NON-ENTRY
// caller (Holder under Main). The per-dispatch base composes through the
// parent-context chain (Main->holder@0, +second@4); also pins the
// dispatch-index table's ARENA-index (1-based) alignment -- the positional
// table read the next state's base and this is the shape that exposes it.

#[test]
fn runtime_nonentry_second_receiver_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_NONENTRY_SECOND_RECEIVER_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-nonentry-second-receiver-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("non-entry second-receiver canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("non-entry second-receiver canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("non-entry second-receiver canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the SECOND receiver's storage through a NON-entry caller (21 -> exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

// RECEIVER SLICE 2, SELF-CALL HOP: the dispatching caller is reached
// through a machine-to-machine SELF call (Main -> holder.run() ->
// self.step() -> second.drain()). Self-call contexts inherit the parent's
// composed base, so the named-receiver hop downstream keeps composing.

#[test]
fn runtime_selfcall_chain_second_receiver_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_SELFCALL_CHAIN_SECOND_RECEIVER_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-selfcall-chain-second-receiver-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("self-call chain second-receiver canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("self-call chain second-receiver canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("self-call chain second-receiver canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the SECOND receiver's storage through a self-call hop (21 -> exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

// NESTED INLINE VALUE-CALL CHAIN with COLLIDING LOCAL NAMES: the outer
// leaf terminal-write must resolve the arm's arg in the CALL-TARGET scope
// first -- the case-wide name fallback previously copied Main's same-named
// (unwritten) local and delivered ZII.

#[test]
fn runtime_nested_inline_chain_result_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_NESTED_INLINE_CHAIN_RESULT_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-nested-inline-chain-result-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("nested inline chain result canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("nested inline chain result canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("nested inline chain result canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the chained inline result to deliver 7 (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

// RECEIVER SLICE 2, INLINE ROUTE: a non-looping value machine spliced
// through the SECOND same-type receiver from a NON-entry caller (two-hop
// splice; chain-walk recovery + call-target-first leaf-write resolution).

/// A guarded arm's value call is evaluated inside the selected arm at its
/// authored point: resolution synthesizes no continuation state, the callee
/// runs only when the arm is taken, and both the interpreter and the native
/// artifact deliver the arm's result. Exit 70 is the fixtures' own witness.
fn assert_guarded_value_call_arm_canary_exits_70(fixture: &str, description: &str) {
    let canary = pass_canary(fixture);
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        None,
    ))
    .unwrap_or_else(|diagnostics| {
        panic!("{description} should compile to checked trees: {diagnostics:?}")
    });
    let interpreted = interpret(&checked, &[]);
    assert_eq!(
        interpreted.error, None,
        "{description} should interpret cleanly"
    );
    assert_eq!(
        interpreted.exit_code, 70,
        "{description}: the interpreter must take the guarded arm's call result"
    );
    let build_dir = std::env::temp_dir().join(format!(
        "omega-{}-{}",
        fixture
            .rsplit('/')
            .next()
            .unwrap_or(fixture)
            .replace('_', "-"),
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .unwrap_or_else(|diagnostics| {
            panic!("{description} should compile natively: {diagnostics:?}")
        });
    assert_native_exit_code(
        &compilation,
        70,
        description,
        "the native artifact must evaluate the guarded arm's call at its authored point",
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn guarded_value_call_arm_exit_canary_runs() {
    assert_guarded_value_call_arm_canary_exits_70(
        fixture_roster::GUARDED_VALUE_CALL_ARM_EXIT,
        "guarded value-call arm canary",
    );
}

#[test]
fn guarded_value_call_computed_argument_exit_canary_runs() {
    assert_guarded_value_call_arm_canary_exits_70(
        fixture_roster::GUARDED_VALUE_CALL_COMPUTED_ARGUMENT_EXIT,
        "guarded value-call computed-argument canary",
    );
}
