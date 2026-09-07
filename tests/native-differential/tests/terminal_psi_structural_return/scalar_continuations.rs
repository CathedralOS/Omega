//! Native scalar inputs retain their value across real result cleanup edges.

use super::*;

#[path = "scalar_continuations/observation.rs"]
mod observation;

#[test]
fn scalar_parameter_survives_projected_result_cleanup_continuation() {
    let source = "data Token { value: u64; }
        data Pair { left: Token; right: Token; }
        data Root {} data Sink {}
        machine Root::forward(value: Pair) -> Pair { value }
        machine Sink::take(value: Token) {}
        machine Sink::number(value: u64) {}
        machine Root::enter(number: u64, value: Pair) {
            Sink::take(Root::forward(value).right);
            Sink::number(number);
        }";
    let tokens = Lexer::new(source).tokenize().unwrap();
    let syntax = parse_syntax_trees(&tokens).unwrap();
    let resolved = lower_syntax_trees(&syntax).unwrap();
    let typed = lower_symbol_resolved_trees(&resolved).unwrap();
    let checked = lower_typed_trees(typed).unwrap();
    let terminal = lower_machine(&checked, "Root::enter").unwrap();
    let semantic = encode_module(&terminal.semantic_module).unwrap();
    let proof = encode_proof_bundle(&terminal.proof_bundle).unwrap();
    let plan = lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default()).unwrap();
    let observer = plan
        .functions
        .iter()
        .find(|function| function.machine == plan.entry)
        .unwrap()
        .operations
        .iter()
        .find_map(|operation| match operation {
            AbstractOperation::CallUnit {
                callee,
                arguments,
                structural_arguments,
                ..
            } if !arguments.is_empty() && structural_arguments.is_empty() => Some(*callee),
            _ => None,
        })
        .unwrap();
    let target = lower_to_target_operations(&plan, NativeTarget::host())
        .expect("scalar parameter reaches its post-cleanup consumer");
    let assigned = assign_registers(&target).unwrap();
    let emitted = emit_machine_code(&assigned).unwrap();
    let object = build_object_artifact(&emitted).unwrap();
    for mutation in 0..9 {
        let mut changed = emitted.clone();
        let caller = changed
            .functions
            .iter_mut()
            .find(|function| function.machine == plan.entry)
            .unwrap();
        let spill = caller
            .unit_scalar_abi
            .as_ref()
            .unwrap()
            .entry_register_spills[0];
        match mutation {
            0 => caller
                .unit_scalar_abi
                .as_mut()
                .unwrap()
                .entry_register_spills
                .clear(),
            1 => {
                caller
                    .unit_scalar_abi
                    .as_mut()
                    .unwrap()
                    .entry_register_spills[0]
                    .byte_offset = 0
            }
            2 => {
                caller
                    .unit_scalar_abi
                    .as_mut()
                    .unwrap()
                    .entry_register_spills[0]
                    .code_offset += 4
            }
            3 => {
                caller
                    .unit_scalar_abi
                    .as_mut()
                    .unwrap()
                    .entry_register_spills[0]
                    .source_value = semantic_vocabulary::ValueId::new(99999).unwrap()
            }
            4 => {
                caller
                    .unit_scalar_abi
                    .as_mut()
                    .unwrap()
                    .entry_register_spills[0]
                    .parameter_index += 1
            }
            5 => caller.bytes[spill.code_offset] ^= 1,
            6 => {
                let argument = &mut caller
                    .internal_unit_calls
                    .iter_mut()
                    .find(|call| !call.scalar_arguments.is_empty())
                    .unwrap()
                    .scalar_arguments[0];
                let machine_code::InternalUnitScalarArgumentSourceRecord::Parameter {
                    location,
                    ..
                } = &mut argument.source
                else {
                    unreachable!()
                };
                *location =
                    machine_code::UnitScalarParameterLocationRecord::Register(spill.register);
            }
            7 => caller.unit_continuations.clear(),
            8 => {
                // The first structural staging store must not overwrite the
                // scalar home after the correctly recorded entry spill.
                let offset = spill.code_offset + spill.byte_count;
                if NativeTarget::host().architecture == target::Architecture::Aarch64 {
                    let original =
                        u32::from_le_bytes(caller.bytes[offset..offset + 4].try_into().unwrap());
                    let changed = (original & !(0xfff << 10)) | ((spill.byte_offset / 8) << 10);
                    assert_ne!(original, changed);
                    caller.bytes[offset..offset + 4].copy_from_slice(&changed.to_le_bytes());
                } else {
                    caller.bytes[offset] ^= 1;
                }
            }
            _ => unreachable!(),
        }
        assert!(
            build_object_artifact(&changed).is_err(),
            "entry custody mutation {mutation}"
        );
    }
    let image = emit_executable_image(&object, 3).unwrap();
    let record = build_installation_record(&image, ProfileDecisionId::new(1).unwrap()).unwrap();
    for mutation in 0..5 {
        let mut changed = record.clone();
        let caller = changed
            .functions_mut_for_test()
            .iter_mut()
            .find(|function| function.machine == plan.entry)
            .unwrap();
        let abi = caller.unit_scalar_abi.as_mut().unwrap();
        match mutation {
            0 => abi.entry_register_spills.clear(),
            1 => abi.entry_register_spills[0].byte_offset = 0,
            2 => abi.entry_register_spills[0].code_offset += 4,
            3 => {
                abi.entry_register_spills[0].source_value =
                    semantic_vocabulary::ValueId::new(99999).unwrap()
            }
            4 => abi.entry_register_spills[0].parameter_index += 1,
            _ => unreachable!(),
        }
        assert!(
            encode_installation_record(&changed).is_err(),
            "installed entry custody mutation {mutation}"
        );
    }
    observation::execute(&object, &image, plan.entry, observer);
}

#[test]
fn scalar_continuations_preserve_register_and_incoming_stack_origins_across_targets() {
    for case in target_cases() {
        for scalar_count in [1, 10] {
            let parameters = (0..scalar_count)
                .map(|position| format!("number{position}: u64"))
                .collect::<Vec<_>>()
                .join(", ");
            let (carrier, projection) = if case.policy == CallingPolicy::MicrosoftX64 {
                ("[Token; 1]", "[0]")
            } else {
                ("Pair", ".right")
            };
            let source = format!(
                "data Token {{ value: u64; }}
                data Pair {{ left: Token; right: Token; }}
                data Root {{}} data Sink {{}}
                machine Root::forward(value: {carrier}) -> {carrier} {{ value }}
                machine Sink::take(value: Token) {{}}
                machine Sink::number(value: u64) {{}}
                machine Root::enter({parameters}, value: {carrier}) {{
                    Sink::take(Root::forward(value){projection});
                    Sink::number(number{});
                    Sink::number(number0);
                }}",
                scalar_count - 1
            );
            let tokens = Lexer::new(&source).tokenize().unwrap();
            let syntax = parse_syntax_trees(&tokens).unwrap();
            let resolved = lower_syntax_trees(&syntax).unwrap();
            let typed = lower_symbol_resolved_trees(&resolved).unwrap();
            let checked = lower_typed_trees(typed).unwrap();
            let terminal = lower_machine(&checked, "Root::enter").unwrap();
            let semantic = encode_module(&terminal.semantic_module).unwrap();
            let proof = encode_proof_bundle(&terminal.proof_bundle).unwrap();
            let plan =
                lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default()).unwrap();
            let target = lower_to_target_operations(&plan, case.target).unwrap_or_else(|error| {
                panic!("{:?}, {scalar_count} scalars: {error:?}", case.target)
            });
            abstract_operations_to_target_operations::validate_abstract_to_target_translation(
                &plan,
                case.target,
                &target,
            )
            .unwrap();
            let assigned = assign_registers(&target).unwrap();
            let emitted = emit_machine_code(&assigned).unwrap();
            let caller = emitted
                .functions
                .iter()
                .find(|function| function.machine == plan.entry)
                .unwrap();
            let arguments = caller
                .internal_unit_calls
                .iter()
                .flat_map(|call| &call.scalar_arguments)
                .collect::<Vec<_>>();
            assert_eq!(arguments.len(), 2);
            let machine_code::InternalUnitScalarArgumentSourceRecord::Parameter {
                location, ..
            } = arguments[0].source
            else {
                panic!("original scalar parameter");
            };
            assert_eq!(
                matches!(
                    location,
                    machine_code::UnitScalarParameterLocationRecord::IncomingStack { .. }
                ),
                scalar_count == 10
            );
            let machine_code::InternalUnitScalarArgumentSourceRecord::Parameter {
                location, ..
            } = arguments[1].source
            else {
                panic!("original first scalar parameter");
            };
            assert!(matches!(
                location,
                machine_code::UnitScalarParameterLocationRecord::FrameSpill { .. }
            ));
            let object = build_object_artifact(&emitted).unwrap();
            let image = emit_executable_image(&object, 3).unwrap();
            let record =
                build_installation_record(&image, ProfileDecisionId::new(1).unwrap()).unwrap();
            let decoded =
                decode_installation_record(&encode_installation_record(&record).unwrap()).unwrap();
            validate_installation_record(&decoded, &image).unwrap();
        }
    }
}
