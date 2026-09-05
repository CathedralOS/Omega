//! Focused write-only lowering and installation controls.

use super::*;

#[test]
fn parameter_sourced_store_and_caller_cover_all_native_fixed_integers() {
    for (source_type, sign, bits) in [
        ("i8", semantic_vocabulary::IntegerSign::Signed, 8),
        ("u8", semantic_vocabulary::IntegerSign::Unsigned, 8),
        ("i16", semantic_vocabulary::IntegerSign::Signed, 16),
        ("u16", semantic_vocabulary::IntegerSign::Unsigned, 16),
        ("i32", semantic_vocabulary::IntegerSign::Signed, 32),
        ("u32", semantic_vocabulary::IntegerSign::Unsigned, 32),
        ("i64", semantic_vocabulary::IntegerSign::Signed, 64),
        ("u64", semantic_vocabulary::IntegerSign::Unsigned, 64),
    ] {
        let source = format!(
            r#"
                data Sink {{}}
                machine Sink::fill(destination: &write {source_type}, replacement: {source_type}) {{
                    destination = replacement;
                }}

                data Root {{}}
                machine Root::enter(destination: &mut {source_type}, replacement: {source_type}) {{
                    Sink::fill(&write destination, replacement);
                }}
            "#,
        );
        let checked = checked(&source);
        let lowered = checked_trees_to_terminal_psi::lower_machine(&checked, "Root::enter")
            .unwrap_or_else(|error| panic!("{source_type} reaches Terminal production: {error:?}"));
        let root = lowered
            .semantic_module
            .machines
            .iter()
            .find(|machine| machine.id == lowered.semantic_module.entry)
            .expect("entry machine is retained");
        let [scalar_parameter] = root.parameters.as_slice() else {
            panic!("{source_type} caller retains one scalar parameter")
        };
        let [structural_parameter] = root.structural_parameters.as_slice() else {
            panic!("{source_type} caller retains one structural parameter")
        };
        let integer =
            semantic_vocabulary::IntegerType::new(sign, bits).expect("native integer type");
        assert_eq!(
            scalar_parameter.scalar_type,
            semantic_vocabulary::ScalarType::Integer(integer)
        );
        let semantic = terminal_codec::encode_module(&lowered.semantic_module)
            .expect("encode fixed-integer caller semantics");
        let proof = terminal_codec::encode_proof_bundle(&lowered.proof_bundle)
            .expect("encode fixed-integer caller proof bundle");
        let replacement_value = match sign {
            semantic_vocabulary::IntegerSign::Signed => {
                semantic_vocabulary::IntegerValue::Signed(23)
            }
            semantic_vocabulary::IntegerSign::Unsigned => {
                semantic_vocabulary::IntegerValue::Unsigned(23)
            }
        };
        let initial_value = match sign {
            semantic_vocabulary::IntegerSign::Signed => {
                semantic_vocabulary::IntegerValue::Signed(1)
            }
            semantic_vocabulary::IntegerSign::Unsigned => {
                semantic_vocabulary::IntegerValue::Unsigned(1)
            }
        };
        let replacement = terminal_interpreter::TerminalScalarValue::Integer {
            scalar_type: integer,
            value: replacement_value,
        };
        let mut handler = terminal_interpreter::AcceptTerminalEffects;
        let executed = terminal_interpreter::interpret_terminal_artifact_with_structural_primitive_values_measured(
            &semantic,
            &proof,
            &proof_admission::AdmissionProfile::default(),
            &[replacement],
            &[terminal_interpreter::TerminalStructuralValue {
                opaque_identity: 17,
                structural_type: structural_parameter.structural_type,
                qualifications: Vec::new(),
                path: Vec::new(),
            }],
            &[terminal_interpreter::TerminalStructuralPrimitiveValue {
                argument_index: 0,
                value: terminal_interpreter::TerminalScalarValue::Integer {
                    scalar_type: integer,
                    value: initial_value,
                },
            }],
            &mut handler,
        )
        .unwrap_or_else(|error| panic!("{source_type} executes through Terminal: {error:?}"));
        assert_eq!(
            executed.structural_primitive_values(),
            &[terminal_interpreter::TerminalStructuralPrimitiveValue {
                argument_index: 0,
                value: replacement,
            }]
        );
        let abstract_plan = terminal_psi_to_abstract_operations::lower_artifact_sections(
            &semantic,
            &proof,
            &proof_admission::AdmissionProfile::default(),
        )
        .unwrap_or_else(|error| panic!("{source_type} reaches Abstract operations: {error:?}"));

        for native_target in [
            target::NativeTarget::linux_x64(),
            target::NativeTarget::linux_arm64(),
        ] {
            let target = abstract_operations_to_target_operations::lower_to_target_operations(
                &abstract_plan,
                native_target,
            )
            .unwrap_or_else(|error| {
                panic!(
                    "{source_type} reaches {:?} Target IR: {error:?}",
                    native_target.architecture
                )
            });
            let assigned =
                target_operations_to_assigned_target_operations::assign_registers(&target)
                    .expect("fixed-integer caller reaches physical assignment");
            let emitted = machine_emission::emit_machine_code(&assigned)
                .expect("fixed-integer caller reaches machine-code custody");
            let store_function = emitted
                .functions
                .iter()
                .find(|function| function.unit_write_only_primitive_stores.len() == 1)
                .expect("callee retains one parameter-sourced store");
            let store = &store_function.unit_write_only_primitive_stores[0];
            assert!(matches!(
                store.source,
                machine_code::UnitWriteOnlyPrimitiveStoreSourceRecord::Parameter {
                    scalar_type,
                    ..
                } if scalar_type == semantic_vocabulary::ScalarType::Integer(integer)
            ));
            assert!(matches!(
                store.destination_type.shape,
                terminal_psi::StructuralTypeShape::PrimitiveScalar(
                    semantic_vocabulary::ScalarType::Integer(scalar_type)
                ) if scalar_type == integer
            ));
            assert!(!store.bytes.is_empty());
            let caller = emitted
                .functions
                .iter()
                .find(|function| function.machine == emitted.entry)
                .expect("emitted caller is retained");
            assert!(matches!(
                caller.internal_unit_calls[0].scalar_arguments.as_slice(),
                [machine_code::InternalUnitScalarCallArgumentRecord {
                    source: machine_code::InternalUnitScalarArgumentSourceRecord::Parameter {
                        scalar_type,
                        ..
                    },
                    ..
                }] if *scalar_type == semantic_vocabulary::ScalarType::Integer(integer)
            ));
            let object = image_emission::build_object_artifact(&emitted)
                .expect("object replay accepts the fixed-integer store and call");
            let image = image_emission::emit_executable_image(&object, 3)
                .expect("fixed-integer store and call reach an executable image");
            let installation = image_emission::build_installation_record(
                &image,
                semantic_vocabulary::ProfileDecisionId::new(1).unwrap(),
            )
            .expect("installation retains fixed-integer store and call custody");
            let installation_bytes = image_emission::encode_installation_record(&installation)
                .expect("encode fixed-integer installation custody");
            let decoded = image_emission::decode_installation_record(&installation_bytes)
                .expect("decode fixed-integer installation custody");
            assert_eq!(decoded, installation);
            image_emission::validate_installation_record(&decoded, &image)
                .expect("installation replays fixed-integer store and call custody");
        }
    }
}
