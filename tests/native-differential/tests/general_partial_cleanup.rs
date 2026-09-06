use abstract_operations_to_target_operations::lower_to_target_operations;
use checked_trees_to_lowered_psi::lower_machine;
use image_emission::{
    build_installation_record, build_object_artifact, decode_installation_record,
    emit_executable_image, encode_installation_record, validate_installation_record,
};
use machine_emission::emit_machine_code;
use optimization_unit::reconstruct_psi_optimization_unit_seed;
use optimization_unit_semantics::validate_psi_optimization_unit;
use proof_admission::AdmissionProfile;
use semantic_vocabulary::ProfileDecisionId;
use target::NativeTarget;
use target_operations_to_assigned_target_operations::assign_registers;
use terminal_codec::{encode_module, encode_proof_bundle};
use terminal_fuel::TerminalFuelSchedule;
use terminal_psi_to_abstract_operations::lower_artifact_sections;

fn plan(source: &str) -> abstract_operations::AbstractOperationPlan {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed).unwrap();
    let terminal = lower_machine(&checked, "Root::enter").unwrap();
    lower_artifact_sections(
        &encode_module(&terminal.semantic_module).unwrap(),
        &encode_proof_bundle(&terminal.proof_bundle).unwrap(),
        &AdmissionProfile::default(),
    )
    .unwrap()
}

#[test]
fn five_element_partial_cleanup_reaches_native_installation() {
    check_source(
        "data Token { value: u64; }
        data Helper {} machine Helper::take(token: Token) {}
        data Root {} machine Root::enter(values: [Token; 5]) { Helper::take(values[2]); }",
        &[("2", 16)],
        &["4", "3", "1", "0"],
    );
}

fn path(text: &str) -> Vec<terminal_psi::StructuralPathSegment> {
    text.split('.')
        .map(|part| match part.parse() {
            Ok(index) => terminal_psi::StructuralPathSegment::FixedIndex(index),
            Err(_) => terminal_psi::StructuralPathSegment::Field(part.to_owned()),
        })
        .collect()
}

fn check_source(source: &str, moved: &[(&str, u32)], residuals: &[&str]) {
    let plan = plan(source);
    let caller = plan
        .functions
        .iter()
        .find(|function| function.machine == plan.entry)
        .unwrap();
    let cleanup = caller
        .operations
        .iter()
        .find_map(|operation| match operation {
            abstract_operations::AbstractOperation::ReturnUnit {
                cleanup_actions, ..
            } => Some(cleanup_actions.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        cleanup
            .iter()
            .map(|action| match action {
                terminal_psi::TerminalAffineCleanupAction::DiscardResidual(discard) =>
                    discard.path.clone(),
                _ => panic!("no whole-root or nominal cleanup in the projected route"),
            })
            .collect::<Vec<_>>(),
        residuals.iter().map(|text| path(text)).collect::<Vec<_>>()
    );
    let unit =
        reconstruct_psi_optimization_unit_seed(&plan, TerminalFuelSchedule::CURRENT.identity())
            .unwrap();
    validate_psi_optimization_unit(&unit).expect("type-directed optimizer ownership replay");
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let target_plan = lower_to_target_operations(&plan, target).expect("target lowering");
        for mutation in 0..3 {
            let mut altered = target_plan.clone();
            let caller = altered
                .functions
                .iter_mut()
                .find(|function| function.machine == plan.entry)
                .unwrap();
            let target_operations::TargetOperation::UnitBody(body) = &mut caller.operation else {
                panic!("Unit body")
            };
            let target_operations::TargetUnitOperation::Call { arguments, .. } =
                &mut body.operations[0]
            else {
                panic!("projected call")
            };
            match mutation {
                0 => arguments[0].source_byte_offset += 1,
                1 => arguments[0].fixed_array_length = Some(u64::MAX),
                2 => arguments[0].element_stride = Some(u32::MAX),
                _ => unreachable!(),
            }
            assert!(
                assign_registers(&altered).is_err(),
                "assignment independently checks projection mutation {mutation}"
            );
        }
        let assigned = assign_registers(&target_plan).expect("physical assignment");
        let machine = emit_machine_code(&assigned).expect("machine emission");
        let emitted = machine
            .functions
            .iter()
            .find(|function| function.machine == plan.entry)
            .unwrap();
        assert_eq!(emitted.internal_unit_calls.len(), moved.len());
        for (call, (expected_path, offset)) in emitted.internal_unit_calls.iter().zip(moved) {
            let [argument] = call.arguments.as_slice() else {
                panic!("one projected argument")
            };
            assert_eq!(argument.path, path(expected_path));
            assert_eq!(argument.source_byte_offset, *offset);
        }
        assert_eq!(
            emitted.unit_affine_cleanup.as_ref().unwrap().actions,
            cleanup
        );
        let mut altered_assignment = assigned.clone();
        let caller = altered_assignment
            .functions
            .iter_mut()
            .find(|function| function.machine == plan.entry)
            .unwrap();
        let assigned_target_operations::AssignedOperation::UnitBody(body) = &mut caller.operation
        else {
            panic!("assigned Unit body")
        };
        let assigned_target_operations::AssignedUnitOperation::Call { copies, .. } =
            &mut body.operations[0]
        else {
            panic!("assigned projected call")
        };
        copies[0].source_byte_offset += 1;
        assert!(
            emit_machine_code(&altered_assignment).is_err(),
            "emission independently checks a changed copy offset"
        );

        let mut changed_offset = machine.clone();
        changed_offset
            .functions
            .iter_mut()
            .find(|function| function.machine == plan.entry)
            .unwrap()
            .internal_unit_calls[0]
            .arguments[0]
            .source_byte_offset += 1;
        assert!(
            build_object_artifact(&changed_offset).is_err(),
            "object replay rejects changed projection offset"
        );
        for ordinal in 0..moved.len() {
            let mut changed_copy = machine.clone();
            let caller = changed_copy
                .functions
                .iter_mut()
                .find(|function| function.machine == plan.entry)
                .unwrap();
            let argument = &mut caller.internal_unit_calls[ordinal].arguments[0];
            assert!(!argument.bytes.is_empty());
            caller.bytes[argument.code_offset] ^= 1;
            argument.bytes[0] ^= 1;
            assert!(
                build_object_artifact(&changed_copy).is_err(),
                "object replay independently rejects matching forged code and copy evidence"
            );
        }
        if !cleanup.is_empty() {
            let mut missing_cleanup = machine.clone();
            missing_cleanup
                .functions
                .iter_mut()
                .find(|function| function.machine == plan.entry)
                .unwrap()
                .unit_affine_cleanup
                .as_mut()
                .unwrap()
                .actions
                .pop();
            assert!(
                build_object_artifact(&missing_cleanup).is_err(),
                "object replay rejects a missing residual"
            );
        }
        let object = build_object_artifact(&machine)
            .unwrap_or_else(|error| panic!("object replay on {target:?}: {error:?}\n{source}"));
        let image = emit_executable_image(&object, 3).expect("image emission");
        let installation = build_installation_record(&image, ProfileDecisionId::new(1).unwrap())
            .expect("installation replay");
        validate_installation_record(&installation, &image).unwrap();
        assert_eq!(
            decode_installation_record(&encode_installation_record(&installation).unwrap()),
            Ok(installation)
        );
    }
}

#[test]
fn nested_array_and_mixed_record_paths_retain_maximal_residuals() {
    check_source(
        "data Token { value: u64; } data Helper {} machine Helper::take(token: Token) {}
        data Root {} machine Root::enter(values: [[Token; 3]; 3]) {
            Helper::take(values[1][2]); Helper::take(values[1][0]);
        }",
        &[("1.2", 40), ("1.0", 24)],
        &["2", "1.1", "0"],
    );
    check_source(
        "data Token { value: u64; } data Row { head: Token; tail: [Token; 3]; }
        data Outer { prefix: Token; rows: [Row; 2]; suffix: Token; }
        data Helper {} machine Helper::take(token: Token) {}
        data Root {} machine Root::enter(values: Outer) {
            Helper::take(values.rows[1].tail[1]); Helper::take(values.prefix);
        }",
        &[("rows.1.tail.1", 56), ("prefix", 0)],
        &[
            "suffix",
            "rows.1.tail.2",
            "rows.1.tail.0",
            "rows.1.head",
            "rows.0",
        ],
    );
}

#[test]
fn complete_arrays_and_records_retain_empty_cleanup() {
    check_source(
        "data Token { value: u64; } data Helper {} machine Helper::take(token: Token) {}
        data Root {} machine Root::enter(values: [Token; 3]) {
            Helper::take(values[2]); Helper::take(values[0]); Helper::take(values[1]);
        }",
        &[("2", 16), ("0", 0), ("1", 8)],
        &[],
    );
    check_source(
        "data Token { value: u64; } data Outer { first: Token; scalar: u64; last: Token; }
        data Helper {} machine Helper::take(token: Token) {}
        data Root {} machine Root::enter(values: Outer) {
            Helper::take(values.last); Helper::take(values.first);
        }",
        &[("last", 16), ("first", 0)],
        &[],
    );
}

#[test]
fn deeper_paths_whole_subtree_moves_and_wider_dimensions_use_the_same_replay() {
    check_source(
        "data Token { value: u64; } data Helper {} machine Helper::take(token: Token) {}
        data Root {} machine Root::enter(values: [[[Token; 2]; 2]; 2]) {
            Helper::take(values[1][0][1]);
        }",
        &[("1.0.1", 40)],
        &["1.1", "1.0.0", "0"],
    );
    check_source(
        "data Token { value: u64; } data Helper {} machine Helper::take(token: Token) {}
        machine Helper::take_row(values: [Token; 3]) {}
        data Root {} machine Root::enter(values: [[Token; 3]; 2]) {
            Helper::take_row(values[1]); Helper::take(values[0][1]);
        }",
        &[("1", 24), ("0.1", 8)],
        &["0.2", "0.0"],
    );
    for length in [1_u32, 7, 17] {
        let selected = length / 2;
        let source = format!("data Token {{ value: u64; }} data Helper {{}} machine Helper::take(token: Token) {{}}
            data Root {{}} machine Root::enter(values: [Token; {length}]) {{ Helper::take(values[{selected}]); }}");
        let residuals = (0..length)
            .rev()
            .filter(|index| *index != selected)
            .map(|index| index.to_string())
            .collect::<Vec<_>>();
        check_source(
            &source,
            &[(&selected.to_string(), selected * 8)],
            &residuals.iter().map(String::as_str).collect::<Vec<_>>(),
        );
    }
}
