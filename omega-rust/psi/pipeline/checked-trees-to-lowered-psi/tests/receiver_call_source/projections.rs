//! Exact mutable receiver subloans from plain record fields.

use super::*;
use checked_trees::CheckedUnitStructuralPathSegment;
use terminal_psi::StructuralPathSegment;

#[path = "projections/rejections.rs"]
mod rejections;

fn projected_source(
    nested: bool,
    self_caller: bool,
    from_parameter: bool,
    caller_first: bool,
) -> (String, &'static str, Vec<&'static str>) {
    let caller_name = if self_caller {
        "Container::forward"
    } else {
        "forward"
    };
    let root = if self_caller { "self" } else { "container" };
    let path = if nested {
        vec!["inner", "record"]
    } else {
        vec!["record"]
    };
    let fields = if nested {
        "inner: Inner;"
    } else {
        "record: Record;"
    };
    let signature = match (self_caller, from_parameter) {
        (false, false) => "container: &mut Container",
        (false, true) => "replacement: u16, container: &mut Container",
        (true, false) => "&mut self",
        (true, true) => "&mut self, replacement: u16",
    };
    let scalar_formal = if from_parameter {
        ", replacement: u16"
    } else {
        ""
    };
    let replacement = if from_parameter { "replacement" } else { "17" };
    let argument = if from_parameter { "replacement" } else { "" };
    let callee = format!(
        "machine Record::replace(&mut self{scalar_formal}) {{ self.value = {replacement}; }}"
    );
    let caller = format!(
        "machine {caller_name}({signature}) {{ {root}.{}.replace({argument}); }}",
        path.join(".")
    );
    let declarations = if caller_first {
        format!("{caller}\n{callee}")
    } else {
        format!("{callee}\n{caller}")
    };
    (
        format!(
            "data Record {{ value: u16; }}
             data Inner {{ record: Record; }}
             data Container {{ {fields} }}
             {declarations}"
        ),
        caller_name,
        path,
    )
}

fn assert_projected_receiver(nested: bool, self_caller: bool) {
    for from_parameter in [false, true] {
        for caller_first in [false, true] {
            let (source, caller_name, field_names) =
                projected_source(nested, self_caller, from_parameter, caller_first);
            let checked = checked_from_source(&source);
            let caller = unit_plan(&checked, caller_name);
            let callee = unit_plan(&checked, "Record::replace");
            let [caller_receiver] = caller.structural_parameters.as_slice() else {
                panic!("caller retains its container root: {source}")
            };
            let [callee_receiver] = callee.structural_parameters.as_slice() else {
                panic!("callee retains its record receiver: {source}")
            };
            assert_eq!(
                caller_receiver.position,
                u32::from(from_parameter && !self_caller)
            );
            assert_eq!(caller_receiver.is_self, self_caller);
            assert_eq!(callee_receiver.position, 0);
            assert!(callee_receiver.is_self);
            assert_ne!(caller_receiver.type_identity, callee_receiver.type_identity);
            for receiver in [caller_receiver, callee_receiver] {
                assert_eq!(receiver.access, CheckedStructuralAccess::MutableBorrow);
            }
            let [
                CheckedUnitEffectOperationPlan::CallUnit {
                    target_machine,
                    scalar_arguments,
                    structural_arguments,
                    claim_transfers,
                    ..
                },
                CheckedUnitEffectOperationPlan::ReturnUnit { .. },
            ] = caller.operations.as_slice()
            else {
                panic!("one projected receiver call and return: {source}")
            };
            assert_eq!(*target_machine, callee.machine);
            assert!(claim_transfers.is_empty());
            let [argument] = structural_arguments.as_slice() else {
                panic!("one exact receiver operand")
            };
            assert_eq!(
                argument.source,
                CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index: 0 }
            );
            assert_eq!(argument.type_identity, callee_receiver.type_identity);
            assert_eq!(argument.access, CheckedStructuralAccess::MutableBorrow);
            assert_eq!(
                argument.path,
                field_names
                    .iter()
                    .map(|identity| CheckedUnitStructuralPathSegment::Field((*identity).into()))
                    .collect::<Vec<_>>()
            );
            if from_parameter {
                let [caller_scalar] = caller.scalar_parameters.as_slice() else {
                    panic!("one caller scalar")
                };
                let [callee_scalar] = callee.scalar_parameters.as_slice() else {
                    panic!("one callee scalar")
                };
                assert_eq!(caller_scalar.source_position, u32::from(self_caller));
                assert_eq!(callee_scalar.source_position, 1);
                assert!(matches!(
                    scalar_arguments.as_slice(),
                    [CheckedCallScalarArgument::Pure(
                        CheckedScalarExpression::Parameter {
                            position: 0,
                            primitive_type: typed_trees::types::PrimitiveType::U16
                        }
                    )]
                ));
            } else {
                assert!(scalar_arguments.is_empty());
            }

            let artifact = terminal_production::produce_terminal_artifact(&checked, caller_name)
                .expect("projected receiver reaches canonical Terminal production");
            drop(checked);
            let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
            let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
            assert_eq!(
                terminal_codec::encode_module(&module).unwrap(),
                artifact.semantic_bytes()
            );
            let profile = proof_admission::AdmissionProfile::default();
            let verified = terminal_verifier::verify_module(&module, &proof, &profile)
                .expect("decoded projected receiver independently verifies");
            let certificate = terminal_fixed_fuel::derive_fixed_entry_fuel(&verified, module.entry)
                .expect("projected call and store have fixed fuel");
            let caller = module
                .machines
                .iter()
                .find(|machine| machine.id == module.entry)
                .unwrap();
            let [caller_receiver] = caller.structural_parameters.as_slice() else {
                panic!("one container root")
            };
            assert_eq!(caller_receiver.position, 0);
            assert_eq!(caller_receiver.is_self, self_caller);
            let [caller_block] = caller.blocks.as_slice() else {
                panic!("one caller block")
            };
            let [call] = caller_block.operations.as_slice() else {
                panic!("one call")
            };
            let OperationKind::CallUnit {
                callee,
                arguments,
                structural_arguments,
                ..
            } = &call.kind
            else {
                panic!("projected receiver remains an ordinary Unit call")
            };
            assert_eq!(call.result, OperationResult::Unit);
            let [argument] = structural_arguments.as_slice() else {
                panic!("one projected operand")
            };
            assert_eq!(argument.place, caller_receiver.place);
            assert_eq!(argument.access, StructuralAccess::MutableBorrow);
            assert_eq!(
                argument.path,
                field_names
                    .iter()
                    .map(|identity| StructuralPathSegment::Field((*identity).into()))
                    .collect::<Vec<_>>()
            );
            let callee = module
                .machines
                .iter()
                .find(|machine| machine.id == *callee)
                .unwrap();
            let [callee_receiver] = callee.structural_parameters.as_slice() else {
                panic!("one record receiver")
            };
            assert_eq!(callee_receiver.position, 0);
            assert!(callee_receiver.is_self);
            assert_ne!(
                caller_receiver.structural_type,
                callee_receiver.structural_type
            );
            for receiver in [caller_receiver, callee_receiver] {
                assert_eq!(receiver.access, StructuralAccess::MutableBorrow);
                assert_eq!(receiver.multiplicity, StructuralMultiplicity::Unrestricted);
                assert!(receiver.qualifications.is_empty());
                assert!(receiver.projected_qualifications.is_empty());
            }
            let mut projected_type = caller_receiver.structural_type;
            for identity in &field_names {
                let declaration = module
                    .structural_types
                    .iter()
                    .find(|declaration| declaration.id == projected_type)
                    .unwrap();
                let StructuralTypeShape::Record { fields } = &declaration.shape else {
                    panic!("plain record along the exact path")
                };
                let field = fields
                    .iter()
                    .find(|field| field.identity == *identity)
                    .expect("authored field identity");
                let StructuralFieldType::Structural(field_type) = field.field_type else {
                    panic!("receiver projection stays structural")
                };
                projected_type = field_type;
            }
            assert_eq!(projected_type, callee_receiver.structural_type);
            let record = module
                .structural_types
                .iter()
                .find(|declaration| declaration.id == projected_type)
                .unwrap();
            let StructuralTypeShape::Record { fields } = &record.shape else {
                panic!("record receiver target")
            };
            let [field] = fields.as_slice() else {
                panic!("one stored scalar field")
            };
            let integer_type = IntegerType::new(IntegerSign::Unsigned, 16).unwrap();
            assert_eq!(field.identity, "value");
            assert_eq!(
                field.field_type,
                StructuralFieldType::Scalar(ScalarType::Integer(integer_type))
            );
            let [callee_block] = callee.blocks.as_slice() else {
                panic!("one callee block")
            };
            for (machine, block) in [(caller, caller_block), (callee, callee_block)] {
                assert_eq!(machine.result, TerminalMachineResult::Unit);
                assert!(matches!(block.terminator, Terminator::ReturnUnit { .. }));
            }
            assert_eq!(
                callee_block.operations.len(),
                if from_parameter { 1 } else { 2 }
            );
            let store = callee_block.operations.last().unwrap();
            let OperationKind::StructuralScalarFieldStore {
                destination,
                path,
                field: stored_field,
                value,
            } = &store.kind
            else {
                panic!("callee stores exactly its literal or scalar parameter")
            };
            assert_eq!(*destination, callee_receiver.place);
            assert!(path.is_empty());
            assert_eq!(*stored_field, field.id);
            assert_eq!(store.result, OperationResult::Unit);
            let scalar_arguments = if from_parameter {
                let [caller_scalar] = caller.parameters.as_slice() else {
                    panic!("one scalar input")
                };
                let [callee_scalar] = callee.parameters.as_slice() else {
                    panic!("one scalar store source")
                };
                assert_eq!(arguments, &[caller_scalar.id]);
                assert_eq!(*value, callee_scalar.id);
                assert_eq!(caller_scalar.scalar_type, ScalarType::Integer(integer_type));
                assert_eq!(callee_scalar.scalar_type, caller_scalar.scalar_type);
                vec![TerminalScalarValue::Integer {
                    scalar_type: integer_type,
                    value: IntegerValue::Unsigned(37),
                }]
            } else {
                assert!(arguments.is_empty());
                assert!(caller.parameters.is_empty());
                assert!(callee.parameters.is_empty());
                let constant = &callee_block.operations[0];
                assert_eq!(
                    constant.kind,
                    OperationKind::IntegerConstant {
                        value: IntegerValue::Unsigned(17)
                    }
                );
                let OperationResult::Scalar(result) = constant.result else {
                    panic!("typed literal")
                };
                assert_eq!(*value, result.id);
                assert_eq!(result.scalar_type, ScalarType::Integer(integer_type));
                Vec::new()
            };

            // Opaque backing establishes interpreter execution and fuel only;
            // it exposes no public post-return field observation or native entry.
            let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
                artifact.semantic_bytes(),
                artifact.proof_bytes(),
                &profile,
                &scalar_arguments,
                &[TerminalStructuralValue {
                    opaque_identity: 71,
                    structural_type: caller_receiver.structural_type,
                    qualifications: Vec::new(),
                    path: Vec::new(),
                }],
            )
            .expect("interpreter accepts the exact container root");
            let mut meter =
                terminal_fuel::TerminalFuelMeter::with_allowance(certificate.ceiling_units());
            assert_eq!(
                execution.resume(&mut meter).unwrap(),
                TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
            );
            assert_eq!(meter.usage().total_units(), certificate.ceiling_units());
            for operation in [call, store] {
                assert_eq!(
                    meter
                        .usage()
                        .at(terminal_fuel::FuelChargeSite::Operation(operation.id))
                        .expect("call and projected callee store execute")
                        .executions(),
                    1
                );
            }
        }
    }
}

#[test]
fn mutable_parameter_field_receiver_retains_exact_subloan() {
    assert_projected_receiver(false, false);
}

#[test]
fn mutable_parameter_nested_receiver_retains_ordered_subloan() {
    assert_projected_receiver(true, false);
}

#[test]
fn mutable_self_field_receiver_retains_container_root() {
    assert_projected_receiver(false, true);
}

#[test]
fn mutable_self_nested_receiver_retains_container_root() {
    assert_projected_receiver(true, true);
}

#[test]
fn corrupted_projected_receiver_path_type_and_access_reject() {
    let (source, caller_name, _) = projected_source(true, false, true, false);
    let checked = checked_from_source(&source);
    let artifact = terminal_production::produce_terminal_artifact(&checked, caller_name).unwrap();
    drop(checked);
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    let profile = proof_admission::AdmissionProfile::default();
    terminal_verifier::verify_module(&module, &proof, &profile)
        .expect("uncorrupted source artifact verifies");
    for corruption in [
        "unknown field",
        "reordered path",
        "erased path",
        "target type",
        "argument access",
        "source access",
    ] {
        let mut corrupted = module.clone();
        let caller = corrupted
            .machines
            .iter_mut()
            .find(|machine| machine.id == module.entry)
            .unwrap();
        let OperationKind::CallUnit {
            structural_arguments,
            ..
        } = &mut caller.blocks[0].operations[0].kind
        else {
            panic!("one projected receiver call")
        };
        let argument = &mut structural_arguments[0];
        match corruption {
            "unknown field" => argument.path[0] = StructuralPathSegment::Field("missing".into()),
            "reordered path" => argument.path.reverse(),
            "erased path" => argument.path.clear(),
            "argument access" => argument.access = StructuralAccess::SharedBorrow,
            "source access" => {
                caller.structural_parameters[0].access = StructuralAccess::SharedBorrow
            }
            "target type" => {
                let root_type = caller.structural_parameters[0].structural_type;
                let root = corrupted
                    .structural_types
                    .iter_mut()
                    .find(|declaration| declaration.id == root_type)
                    .unwrap();
                let StructuralTypeShape::Record { fields } = &mut root.shape else {
                    panic!("container root")
                };
                // The same field identity now resolves to a scalar instead of Inner.
                fields[0].field_type = StructuralFieldType::Scalar(ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 16).unwrap(),
                ));
            }
            _ => unreachable!(),
        }
        assert!(
            terminal_verifier::verify_module(&corrupted, &proof, &profile).is_err(),
            "independent verification must reject {corruption}"
        );
    }
}
