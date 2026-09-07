//! A real call-result home supplies subsequent field transfers.

use super::*;

#[cfg(unix)]
use super::affine_call_result_host as host;

#[test]
fn named_affine_result_projection_preserves_its_native_residual() {
    check("Pair", "result.right", None, 16, true);
    check("[Token; 2]", "result[1]", None, 16, true);
}

#[test]
fn fully_consumed_affine_results_keep_real_storage_and_empty_cleanup() {
    check("Pair", "result.right", Some("result.left"), 16, true);
    check("[Token; 2]", "result[1]", Some("result[0]"), 16, true);
    check("[Token; 1]", "result[0]", None, 8, true);
}

#[test]
fn free_unit_callers_retain_projected_result_cleanup_without_an_attachment() {
    check("Pair", "result.right", None, 16, false);
    check("[Token; 2]", "result[1]", Some("result[0]"), 16, false);
}

fn check(carrier: &str, first: &str, second: Option<&str>, byte_size: u16, attached: bool) {
    check_source(carrier, first, second, byte_size, attached, false);
}

#[test]
fn anonymous_affine_result_projections_retain_native_storage_and_cleanup() {
    for attached in [false, true] {
        check_source("Pair", "result.right", None, 16, attached, true);
        check_source("[Token; 2]", "result[1]", None, 16, attached, true);
        check_source("[Token; 1]", "result[0]", None, 8, attached, true);
    }
}

fn check_source(
    carrier: &str,
    first: &str,
    second: Option<&str>,
    byte_size: u16,
    attached: bool,
    anonymous: bool,
) {
    let root = if attached { "Root::" } else { "" };
    let sink = if attached { "Sink::" } else { "" };
    let second_call = second
        .map(|value| format!("{sink}take({value});"))
        .unwrap_or_default();
    let mut source = format!(
        "data Token {{ value: u64; }}
        data Pair {{ left: Token; right: Token; }}
        data Root {{}} data Sink {{}}
        machine {root}forward(value: {carrier}) -> {carrier} {{ value }}
        machine {sink}take(value: Token) {{}}
        machine {root}enter(value: {carrier}) {{
            let result: {carrier} = {root}forward(value);
            {sink}take({first}); {second_call}
        }}"
    );
    if anonymous {
        assert!(second.is_none());
        let producer = format!("{root}forward(value)");
        source = source
            .replace(&format!("let result: {carrier} = {producer};"), "")
            .replace("result.", &format!("{producer}."))
            .replace("result[", &format!("{producer}["));
    }
    let tokens = Lexer::new(&source).tokenize().unwrap();
    let syntax = parse_syntax_trees(&tokens).unwrap();
    let resolved = lower_syntax_trees(&syntax).unwrap();
    let typed = lower_symbol_resolved_trees(&resolved).unwrap();
    let checked = lower_typed_trees(typed).unwrap();
    let terminal = lower_machine(&checked, &format!("{root}enter")).unwrap();
    let semantic = encode_module(&terminal.semantic_module).unwrap();
    let proof = encode_proof_bundle(&terminal.proof_bundle).unwrap();
    let plan = lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default()).unwrap();
    let entry = terminal.semantic_module.entry;
    drop(checked);
    drop(terminal);
    for case in target_cases() {
        if byte_size == 16 && case.policy == CallingPolicy::MicrosoftX64 {
            assert!(lower_to_target_operations(&plan, case.target).is_err());
            continue;
        }
        let target = lower_to_target_operations(&plan, case.target).unwrap();
        let assigned = assign_registers(&target).unwrap();
        let assigned_index = assigned
            .functions
            .iter()
            .position(|function| function.machine == entry)
            .unwrap();
        for mutation in 0..3 {
            let mut changed = assigned.clone();
            let AssignedOperation::UnitBody(body) =
                &mut changed.functions[assigned_index].operation
            else {
                panic!("Unit caller");
            };
            let assigned_target_operations::AssignedUnitOperation::StructuralResultCall {
                result_home,
                ..
            } = &mut body.operations[0]
            else {
                panic!("leading producer");
            };
            match mutation {
                0 => *result_home = None,
                1 => result_home.as_mut().unwrap().byte_offset = 0,
                2 => {
                    result_home.as_mut().unwrap().requirement.defining_operation =
                        semantic_vocabulary::OperationId::new(999).unwrap()
                }
                _ => unreachable!(),
            }
            assert!(
                emit_machine_code(&changed).is_err(),
                "assigned mutation {mutation}"
            );
        }
        let emitted = emit_machine_code(&assigned).unwrap();
        let caller_index = emitted
            .functions
            .iter()
            .position(|function| function.machine == entry)
            .unwrap();
        let function = &emitted.functions[caller_index];
        assert_eq!(function.unit_parameter_homes.len(), 1);
        let input = function.unit_parameter_homes[0].place;
        let producer = &function.internal_unit_calls[0];
        let result = producer.structural_result.as_ref().unwrap();
        let home = result.result_home.as_ref().unwrap();
        assert_eq!(home.requirement.result, result.operation_result);
        assert_eq!(
            producer.owner,
            CallSiteOwner::Operation(home.requirement.defining_operation)
        );
        assert_ne!(result.operation_result.place, input);
        assert_eq!(
            home.requirement.layout,
            target_operations::TargetStructuralHomeLayout::Aggregate(ValueShape::integer(
                byte_size, 8
            ))
        );
        assert_eq!(home.home_byte_offset, u32::from(byte_size));
        assert_eq!(
            home.code_offset + home.byte_count,
            producer.code_offset + producer.byte_count
        );
        assert_eq!(
            &function.bytes[home.code_offset..home.code_offset + home.byte_count],
            home.bytes
        );
        let consumers = &function.internal_unit_calls[1..];
        assert_eq!(consumers.len(), 1 + usize::from(second.is_some()));
        for (index, consumer) in consumers.iter().enumerate() {
            let [argument] = consumer.arguments.as_slice() else {
                panic!("one projected owner");
            };
            assert_eq!(consumer.operation_ordinal, index + 1);
            assert_eq!(argument.place, result.operation_result.place);
            assert_eq!(
                argument.source_location.stack_byte_offset(),
                Some(home.home_byte_offset)
            );
            assert_eq!(argument.source, result.caller_result_placement);
            assert_eq!(
                argument.source_byte_offset,
                if index == 0 && byte_size == 16 { 8 } else { 0 }
            );
            assert!(argument.code_offset >= home.code_offset + home.byte_count);
        }
        let cleanup = &function.unit_affine_cleanup.as_ref().unwrap().actions;
        if second.is_some() || byte_size == 8 {
            assert!(cleanup.is_empty());
        } else {
            let [TerminalAffineCleanupAction::DiscardResidual(residual)] = cleanup.as_slice()
            else {
                panic!("one exact residual");
            };
            assert_eq!(residual.place, result.operation_result.place);
            assert_ne!(residual.path, consumers[0].arguments[0].path);
        }
        for mutation in 0..7 {
            let mut changed = emitted.clone();
            let function = &mut changed.functions[caller_index];
            corrupt_result(&mut function.internal_unit_calls[0], input, mutation);
            if mutation == 4 {
                // Keep the sidecar and executable bytes consistent: independent
                // store reconstruction must reject the altered instruction.
                function.bytes[home.code_offset] ^= 1;
            }
            assert!(
                build_object_artifact(&changed).is_err(),
                "emitted mutation {mutation} {carrier} {:?}",
                case.target
            );
        }
        for wrong_root in [input, result.operation_result.place] {
            let mut changed = emitted.clone();
            changed.functions[caller_index]
                .unit_affine_cleanup
                .as_mut()
                .unwrap()
                .actions = vec![TerminalAffineCleanupAction::DiscardRoot(wrong_root)];
            assert!(build_object_artifact(&changed).is_err());
        }
        let object = build_object_artifact(&emitted).unwrap();
        assert_eq!(emit_object_container(&object).psi, plan.psi);
        let image = emit_executable_image(&object, 3).unwrap();
        let installation =
            build_installation_record(&image, ProfileDecisionId::new(1).unwrap()).unwrap();
        validate_installation_record(&installation, &image).unwrap();
        let encoded = encode_installation_record(&installation).unwrap();
        let decoded = decode_installation_record(&encoded).unwrap();
        validate_installation_record(&decoded, &image).unwrap();
        let installed_call = installation
            .internal_unit_calls()
            .iter()
            .position(|call| call.machine == entry && call.custody.operation_ordinal == 0)
            .unwrap();
        for mutation in 0..7 {
            let mut changed = installation.clone();
            corrupt_result(
                &mut changed.internal_unit_calls_mut_for_test()[installed_call].custody,
                input,
                mutation,
            );
            assert!(
                validate_installation_record(&changed, &image).is_err(),
                "installed mutation {mutation}"
            );
        }
        #[cfg(unix)]
        if case.target == NativeTarget::host() {
            host::execute(&image, object.entry_function().text_offset, byte_size == 16);
        }
    }
}

fn corrupt_result(
    call: &mut machine_code::InternalUnitCallRecord,
    input: PlaceId,
    mutation: usize,
) {
    let result = call.structural_result.as_mut().unwrap();
    if mutation == 0 {
        result.result_home = None;
        return;
    }
    let home = result.result_home.as_mut().unwrap();
    match mutation {
        1 => home.requirement.result.place = input,
        2 => {
            home.requirement.defining_operation =
                semantic_vocabulary::OperationId::new(999).unwrap()
        }
        3 => home.home_byte_offset = 0,
        4 => home.bytes[0] ^= 1,
        5 => home.code_offset += 1,
        6 => home.byte_count += 1,
        _ => unreachable!(),
    }
}
