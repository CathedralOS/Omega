use psi_build_time_evaluation::{
    BuildEvaluationSponsor, BuildEvaluationSponsorLimits, BuildMachineEvaluationError,
    BuildMachineExecutionMode, BuildMachineFilesystemAccess, BuildTimeValue,
    PreparedBuildMachineProgram, evaluate_build_machine_arguments_measured,
    evaluate_build_machine_arguments_measured_with_sponsor, evaluate_const_array_lengths,
    evaluate_zero_argument_machine,
};
use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_tokens_to_syntax_trees::parse_syntax_trees;

const SOURCE: &str = r#"
    boundary trait FilesystemHost {
        machine create(path: &[u8], mode: i32) -> i32
        reaches FilesystemHost;
    }

    data Build { freestanding: bool; }

    machine pure_build(value: &mut Build) {
        value.freestanding = true;
    }

    machine policy_value() -> u16 {
        7
    }

    machine raw_bytes() -> [u8; 2] {
        "\x80A"
    }

    data Snapshot [copy] { ok: bool; bytes: [u8; 2]; }
    machine snapshot() -> Snapshot {
        Snapshot { ok: true, bytes: "\x01\x02" }
    }

    data Choice [copy] { case Ready(bytes: [u8; 2]); case Empty; }
    machine choice() -> Choice {
        (Choice::Ready { bytes: "\x03\x04" })
    }

    data Stager { filesystem: FilesystemHost; result: i32; }
    machine Stager::build(&mut self, value: &mut Build)
    reaches FilesystemHost
    {
        self.result = self.filesystem.create("artifact.bin", 438);
        value.freestanding = self.result >= 0;
    }

    data StatementStager { filesystem: FilesystemHost; }
    machine StatementStager::build(&mut self, value: &mut Build)
    reaches FilesystemHost
    {
        _ = self.filesystem.create("artifact.bin", 438);
        value.freestanding = true;
    }
"#;

#[test]
fn admission_plan_owns_result_machine_lookup_gate_and_evaluation() {
    let typed = typed(SOURCE);
    let admission = psi_build_time_evaluation::BuildTimeAdmissionPlan::infer(&typed);

    let value = admission
        .evaluate_const_evaluable_machine(&typed, "policy_value", vec![])
        .expect("an admitted result machine should evaluate");
    assert_eq!(value, BuildTimeValue::Int(7));

    let error = admission
        .evaluate_const_evaluable_machine(&typed, "Stager::build", vec![])
        .expect_err("admission must reject reached services before interpretation");
    assert!(error.contains("service reach [FilesystemHost]"), "{error}");
}

#[test]
fn invocation_const_boundary_admits_an_exact_boolean_snapshot() {
    let typed = typed(
        r#"
        machine is_positive(value: u64) -> bool { value > 0 }
        "#,
    );
    let admission = psi_build_time_evaluation::BuildTimeAdmissionPlan::infer(&typed);

    let value = admission
        .evaluate_const_evaluable_machine_for_invocation(
            &typed,
            "is_positive",
            vec![BuildTimeValue::Int(4)],
            psi_build_time_evaluation::BuildTimeInvocationCustody::Source(Default::default()),
        )
        .expect("a Boolean proof-position result should cross as an owned snapshot");
    assert_eq!(value, BuildTimeValue::Bool(true));
}

#[test]
fn exact_width_quoted_literal_evaluates_as_an_owned_raw_byte_array() {
    let typed = typed(SOURCE);
    let admission = psi_build_time_evaluation::BuildTimeAdmissionPlan::infer(&typed);

    let value = admission
        .evaluate_const_evaluable_machine(&typed, "raw_bytes", vec![])
        .expect("exact-width owned bytes should evaluate");
    assert_eq!(
        value,
        BuildTimeValue::Array(vec![BuildTimeValue::Int(0x80), BuildTimeValue::Int(0x41)])
    );
}

#[test]
fn closed_copy_record_and_realized_copy_sum_case_cross_as_owned_snapshots() {
    let typed = typed(SOURCE);
    let admission = psi_build_time_evaluation::BuildTimeAdmissionPlan::infer(&typed);

    assert_eq!(
        admission
            .evaluate_const_evaluable_machine(&typed, "snapshot", vec![])
            .expect("a closed copy record should be ConstEvaluable"),
        BuildTimeValue::Struct {
            type_name: "Snapshot".to_owned(),
            fields: vec![
                (
                    "bytes".to_owned(),
                    BuildTimeValue::Array(vec![BuildTimeValue::Int(1), BuildTimeValue::Int(2)]),
                ),
                ("ok".to_owned(), BuildTimeValue::Bool(true)),
            ],
        }
    );
    assert_eq!(
        admission
            .evaluate_const_evaluable_machine(&typed, "choice", vec![])
            .expect("a realized case of a closed copy sum should be ConstEvaluable"),
        BuildTimeValue::Case {
            variant: "Ready".to_owned(),
            payload: vec![(
                "bytes".to_owned(),
                BuildTimeValue::Array(vec![BuildTimeValue::Int(3), BuildTimeValue::Int(4)]),
            )],
        }
    );
}

#[test]
fn opt_in_const_boundary_rejects_an_affine_nominal_record() {
    let typed = typed(
        r#"
        data Receipt { code: u8; }
        machine receipt() -> Receipt { Receipt { code: 1 } }
        "#,
    );
    let admission = psi_build_time_evaluation::BuildTimeAdmissionPlan::infer(&typed);

    assert!(
        admission
            .evaluate_machine(&typed, "receipt", vec![])
            .is_ok(),
        "legacy structured-plan positions retain their own result validation"
    );
    let error = admission
        .evaluate_const_evaluable_machine(&typed, "receipt", vec![])
        .expect_err("an affine nominal record cannot cross the opt-in const result boundary");
    assert!(error.contains("not ConstEvaluable"), "{error}");
    assert!(error.contains("affine or linear type `Receipt`"), "{error}");
}

#[test]
fn zero_argument_integer_position_uses_const_evaluable_admission_and_exact_int_decoding() {
    let mut typed = typed(
        r#"
        data Receipt { code: u8; }
        machine count() -> u64 { 3 }
        machine decision() -> bool { true }
        machine receipt() -> Receipt { Receipt { code: 1 } }
        data Buffer { bytes: [u8; count()]; }
        "#,
    );
    let admission = psi_build_time_evaluation::BuildTimeAdmissionPlan::infer(&typed);

    assert_eq!(
        evaluate_zero_argument_machine(&typed, &admission, "count", "array length"),
        Ok(3)
    );

    let non_integer =
        evaluate_zero_argument_machine(&typed, &admission, "decision", "array length")
            .expect_err("a copyable non-integer snapshot must not cross an integer const position");
    assert_eq!(
        non_integer,
        "machine `decision` returns `bool`, not an integer type"
    );

    let affine = evaluate_zero_argument_machine(&typed, &admission, "receipt", "array length")
        .expect_err("the position helper must apply ConstEvaluable before decoding");
    assert!(affine.contains("not ConstEvaluable"), "{affine}");
    assert!(
        affine.contains("affine or linear type `Receipt`"),
        "{affine}"
    );

    evaluate_const_array_lengths(&mut typed)
        .expect("the fixed-array integration should retain its literal substitution");
    assert!(
        typed
            .type_reference_table
            .fixed_array_lengths()
            .any(|(_, length)| matches!(
                length,
                psi_typed_trees::types::FixedArrayLength::Literal(3)
            ))
    );
}

#[test]
fn sum_admission_walks_only_the_realized_case_payload() {
    let typed = typed(
        r#"
        data Decision {
            case Accepted(code: u8);
            case Rejected(reason: &[u8]);
        }
        machine accepted() -> Decision {
            (Decision::Accepted { code: 7 })
        }
        machine rejected() -> Decision {
            (Decision::Rejected { reason: "no" })
        }
        "#,
    );
    let admission = psi_build_time_evaluation::BuildTimeAdmissionPlan::infer(&typed);

    assert_eq!(
        admission
            .evaluate_const_evaluable_machine(&typed, "accepted", vec![])
            .expect("an inactive Text case must not contaminate the realized copy case"),
        BuildTimeValue::Case {
            variant: "Accepted".to_owned(),
            payload: vec![("code".to_owned(), BuildTimeValue::Int(7))],
        }
    );
    let error = admission
        .evaluate_const_evaluable_machine(&typed, "rejected", vec![])
        .expect_err("the realized Text payload must remain fail-closed");
    assert!(error.contains("contains Text"), "{error}");
}

#[test]
fn granted_mode_does_not_authorize_a_package_authored_filesystem_lookalike() {
    let typed = typed(SOURCE);
    let prepared = PreparedBuildMachineProgram::prepare(&typed).expect("prepare build program");
    let argument = BuildTimeValue::Struct {
        type_name: "Build".to_owned(),
        fields: vec![("freestanding".to_owned(), BuildTimeValue::Bool(false))],
    };

    let pure = evaluate_build_machine_arguments_measured(
        &prepared,
        "pure_build",
        vec![argument.clone()],
        BuildMachineExecutionMode::Pure,
    )
    .expect("the pure build-machine service should evaluate");

    let pure_error = evaluate_build_machine_arguments_measured(
        &prepared,
        "Stager::build",
        vec![argument.clone()],
        BuildMachineExecutionMode::Pure,
    )
    .expect_err("the pure mode must not silently grant a filesystem boundary");
    assert!(
        pure_error
            .to_string()
            .contains("unknown value-call target `create`"),
        "{pure_error}"
    );

    let granted_error = evaluate_build_machine_arguments_measured(
        &prepared,
        "Stager::build",
        vec![argument],
        BuildMachineExecutionMode::Granted {
            filesystem: BuildMachineFilesystemAccess::Virtual,
            filesystem_metadata_layout: Default::default(),
        },
    )
    .expect_err("a grant cannot turn a package-authored lookalike into a host operation");

    assert!(pure.usage().fuel_units() > 0);
    assert!(!pure.observations().filesystem_host_observed());
    assert!(
        granted_error
            .to_string()
            .contains("unknown value-call target `create`")
    );
    let statement_error = evaluate_build_machine_arguments_measured(
        &prepared,
        "StatementStager::build",
        vec![BuildTimeValue::Struct {
            type_name: "Build".to_owned(),
            fields: vec![("freestanding".to_owned(), BuildTimeValue::Bool(false))],
        }],
        BuildMachineExecutionMode::Granted {
            filesystem: BuildMachineFilesystemAccess::Virtual,
            filesystem_metadata_layout: Default::default(),
        },
    )
    .expect_err("statement dispatch must reject the same package-authored lookalike");
    assert!(
        statement_error
            .to_string()
            .contains("host boundary call `create` not yet supported"),
        "{statement_error}"
    );
    assert_eq!(
        pure.value(),
        &[BuildTimeValue::Struct {
            type_name: "Build".to_owned(),
            fields: vec![("freestanding".to_owned(), BuildTimeValue::Bool(true))],
        }]
    );
}

#[test]
fn sponsored_pure_builds_share_and_exactly_exhaust_one_fuel_account() {
    let typed = typed(SOURCE);
    let prepared = PreparedBuildMachineProgram::prepare(&typed).expect("prepare build program");
    let argument = || BuildTimeValue::Struct {
        type_name: "Build".to_owned(),
        fields: vec![("freestanding".to_owned(), BuildTimeValue::Bool(false))],
    };

    let baseline = evaluate_build_machine_arguments_measured(
        &prepared,
        "pure_build",
        vec![argument()],
        BuildMachineExecutionMode::Pure,
    )
    .expect("baseline pure build");
    let fuel_per_evaluation = baseline.usage().fuel_units();
    assert!(fuel_per_evaluation > 0);
    assert_eq!(baseline.usage().schema().schema_version(), 5);
    assert_eq!(baseline.usage().fuel_ceiling(), 100_000);

    let aggregate_ceiling = fuel_per_evaluation
        .checked_mul(2)
        .expect("small test evaluation");
    let sponsor = BuildEvaluationSponsor::new(
        BuildEvaluationSponsorLimits::new(aggregate_ceiling, 1024, 1024, 64, 1024, 1024)
            .expect("nonzero limits"),
    );
    assert_eq!(sponsor.limits().schema_version(), 5);
    assert_eq!(sponsor.limits().maximum_fuel_units(), aggregate_ceiling);
    let sponsor_clone = sponsor.clone();

    let first = evaluate_build_machine_arguments_measured_with_sponsor(
        &prepared,
        "pure_build",
        vec![argument()],
        BuildMachineExecutionMode::Pure,
        &sponsor,
    )
    .expect("first sponsored evaluation");
    let second = evaluate_build_machine_arguments_measured_with_sponsor(
        &prepared,
        "pure_build",
        vec![argument()],
        BuildMachineExecutionMode::Pure,
        &sponsor_clone,
    )
    .expect("second sponsored evaluation");

    assert_eq!(first.usage().fuel_units(), fuel_per_evaluation);
    assert_eq!(second.usage().fuel_units(), fuel_per_evaluation);
    assert_eq!(sponsor.consumed_fuel_units(), aggregate_ceiling);
    assert_eq!(sponsor_clone.consumed_fuel_units(), aggregate_ceiling);
    assert_eq!(
        sponsor.consumed_result_cells(),
        first.usage().result_cells() + second.usage().result_cells()
    );
    assert_eq!(
        sponsor.consumed_result_text_bytes(),
        first.usage().result_text_bytes() + second.usage().result_text_bytes()
    );

    let error = evaluate_build_machine_arguments_measured_with_sponsor(
        &prepared,
        "pure_build",
        vec![argument()],
        BuildMachineExecutionMode::Pure,
        &sponsor,
    )
    .expect_err("the exactly consumed account must reject another tick");
    assert_eq!(
        error.to_string(),
        format!(
            "build-evaluation aggregate fuel sponsor exhausted at {aggregate_ceiling} fuel units"
        )
    );
    assert_eq!(sponsor.consumed_fuel_units(), aggregate_ceiling);
}

#[test]
fn sponsored_granted_build_uses_the_compiler_ceiling_and_classifies_exhaustion() {
    let typed = typed(SOURCE);
    let prepared = PreparedBuildMachineProgram::prepare(&typed).expect("prepare build program");
    let argument = || BuildTimeValue::Struct {
        type_name: "Build".to_owned(),
        fields: vec![("freestanding".to_owned(), BuildTimeValue::Bool(false))],
    };
    let mode = || BuildMachineExecutionMode::Granted {
        filesystem: BuildMachineFilesystemAccess::Virtual,
        filesystem_metadata_layout: Default::default(),
    };
    let sponsor = BuildEvaluationSponsor::new(
        BuildEvaluationSponsorLimits::new(100_000, 1024, 1024, 64, 1024, 1024)
            .expect("nonzero limits"),
    );

    let evaluated = evaluate_build_machine_arguments_measured_with_sponsor(
        &prepared,
        "pure_build",
        vec![argument()],
        mode(),
        &sponsor,
    )
    .expect("granted entry may execute a pure build");
    assert_eq!(evaluated.usage().fuel_ceiling(), 10_000_000);
    assert_eq!(
        sponsor.consumed_fuel_units(),
        evaluated.usage().fuel_units()
    );

    let exhausted = BuildEvaluationSponsor::new(
        BuildEvaluationSponsorLimits::new(1, 1024, 1024, 64, 1024, 1024).expect("nonzero limits"),
    );
    let error = evaluate_build_machine_arguments_measured_with_sponsor(
        &prepared,
        "pure_build",
        vec![argument()],
        mode(),
        &exhausted,
    )
    .expect_err("one fuel unit is insufficient");
    let BuildMachineEvaluationError::Granted(failure) = error else {
        panic!("granted execution must retain its classified failure");
    };
    assert_eq!(
        failure.kind(),
        psi_checked_interpreter::BuildMachineEvaluationFailureKind::ResourceExhausted
    );
    assert_eq!(
        failure.diagnostic(),
        "build-evaluation aggregate fuel sponsor exhausted at 1 fuel units"
    );
    assert_eq!(failure.usage().expect("partial usage").fuel_units(), 2);
    assert_eq!(exhausted.consumed_fuel_units(), 1);
}

#[test]
fn sponsored_build_rejects_result_custody_above_the_shared_ceiling() {
    let typed = typed(SOURCE);
    let prepared = PreparedBuildMachineProgram::prepare(&typed).expect("prepare build program");
    let sponsor = BuildEvaluationSponsor::new(
        BuildEvaluationSponsorLimits::new(100_000, 1024, 1024, 64, 1, 1024)
            .expect("nonzero limits"),
    );
    let error = evaluate_build_machine_arguments_measured_with_sponsor(
        &prepared,
        "pure_build",
        vec![BuildTimeValue::Struct {
            type_name: "Build".to_owned(),
            fields: vec![("freestanding".to_owned(), BuildTimeValue::Bool(false))],
        }],
        BuildMachineExecutionMode::Pure,
        &sponsor,
    )
    .expect_err("the returned record exceeds one result cell");
    assert_eq!(
        error.to_string(),
        "build-evaluation result-cell sponsor exhausted at 1 cells"
    );
    assert_eq!(sponsor.consumed_result_cells(), 0);
}

#[test]
fn prepared_build_program_specializes_static_machine_helpers() {
    let typed = typed(
        r#"
        data Build { selected: u16; }

        machine chosen(value: u16) -> u16 {
            value
        }

        machine apply<T, machine F>(value: T) -> T
        where machine F(value: T) -> T;
        {
            F(value)
        }

        machine build(value: &mut Build) {
            value.selected = apply<chosen>(70);
        }
        "#,
    );
    let prepared = PreparedBuildMachineProgram::prepare(&typed)
        .expect("Psi should prepare static build-machine selections");
    let evaluated = evaluate_build_machine_arguments_measured(
        &prepared,
        "build",
        vec![BuildTimeValue::Struct {
            type_name: "Build".to_owned(),
            fields: vec![("selected".to_owned(), BuildTimeValue::Int(0))],
        }],
        BuildMachineExecutionMode::Pure,
    )
    .expect("the prepared generic build helper should evaluate");

    assert_eq!(
        evaluated.value(),
        &[BuildTimeValue::Struct {
            type_name: "Build".to_owned(),
            fields: vec![("selected".to_owned(), BuildTimeValue::Int(70))],
        }]
    );
}

#[test]
fn admission_rejects_a_transitive_progress_premise_before_interpretation() {
    let typed = typed(
        r#"
        data SchedulerHandle {}
        domain SchedulerHandle::WeakFair
        satisfies ProgressProfile
        established by SchedulerAdmission::grant;

        boundary trait SchedulerAdmission {
            machine grant(scheduler: SchedulerHandle) -> SchedulerHandle in WeakFair
            ensures result in SchedulerHandle::WeakFair
            terminates;
        }

        boundary trait SchedulerRuntime {
            machine wait(scheduler: SchedulerHandle)
            requires scheduler in WeakFair
            terminates;
        }

        machine build(
            runtime: &mut SchedulerRuntime,
            scheduler: SchedulerHandle
        ) {
            runtime.wait(scheduler);
        }
        "#,
    );
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "build")
        .expect("build machine");
    let error = psi_build_time_evaluation::BuildTimeAdmissionPlan::infer(&typed)
        .require_common_floor(&typed, machine)
        .expect_err("pre-check evaluation has no proof context for the progress premise");

    assert!(
        error.contains("has an authored `requires` premise"),
        "{error}"
    );
    assert!(error.contains("callable contract `wait`"), "{error}");
}

fn typed(source: &str) -> psi_typed_trees::TypedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    lower_symbol_resolved_trees(&resolved).expect("type")
}
