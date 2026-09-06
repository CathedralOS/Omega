//! Source-produced whole borrowed receiver calls through canonical Terminal admission.

use checked_trees::{
    CheckedCallScalarArgument, CheckedScalarExpression, CheckedStructuralAccess,
    CheckedUnitEffectMachinePlan, CheckedUnitEffectOperationPlan,
    CheckedUnitStructuralArgumentSourcePlan,
};
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, ScalarType};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use terminal_interpreter::{
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue,
    TerminalStructuralValue,
};
use terminal_psi::{
    OperationKind, OperationResult, StructuralAccess, StructuralFieldType, StructuralMultiplicity,
    StructuralTypeShape, TerminalMachineResult, Terminator,
};
use tokens_to_syntax_trees::parse_syntax_trees;

#[path = "receiver_call_source/forwarding.rs"]
mod forwarding;
#[path = "receiver_call_source/projections.rs"]
mod projections;

fn checked_from_source(source: &str) -> checked_trees::CheckedTrees {
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize receiver call");
    let syntax = parse_syntax_trees(&tokens).expect("parse receiver call");
    let resolved = lower_syntax_trees(&syntax).expect("resolve receiver call");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type receiver call");
    typed_trees_to_checked_trees::lower_typed_trees(typed).expect("check receiver call")
}

fn unit_plan<'a>(
    checked: &'a checked_trees::CheckedTrees,
    name: &str,
) -> &'a CheckedUnitEffectMachinePlan {
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == name)
        .expect("authored machine")
        .symbol;
    checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .find(|plan| plan.machine == machine)
        .expect("authored machine retains its checked Unit plan")
}

fn assert_receiver_call(access: StructuralAccess, from_parameter: bool, self_caller: bool) {
    let (borrow, checked_access) = match access {
        StructuralAccess::MutableBorrow => ("mut", CheckedStructuralAccess::MutableBorrow),
        StructuralAccess::WriteOnlyBorrow => ("write", CheckedStructuralAccess::WriteOnlyBorrow),
        _ => panic!("fixture requires a writable borrowed receiver"),
    };
    let scalar_formal = if from_parameter {
        ", replacement: u16"
    } else {
        ""
    };
    let replacement = if from_parameter { "replacement" } else { "17" };
    let callee_source = format!(
        "machine Record::replace(&{borrow} self{scalar_formal}) {{
             self.value = {replacement};
         }}"
    );
    let caller_name = if self_caller {
        "Record::forward"
    } else {
        "forward"
    };
    // A leading scalar makes authored structural position 1 differ from dense
    // structural argument index 0. Callee self is implicit at the call site.
    let caller_signature = if self_caller {
        format!("&{borrow} self{scalar_formal}")
    } else if from_parameter {
        format!("replacement: u16, destination: &{borrow} Record")
    } else {
        format!("destination: &{borrow} Record")
    };
    let root = if self_caller { "self" } else { "destination" };
    let call_argument = if from_parameter { "replacement" } else { "" };
    let caller_source =
        format!("machine {caller_name}({caller_signature}) {{ {root}.replace({call_argument}); }}");

    for caller_first in [false, true] {
        let declarations = if caller_first {
            format!("{caller_source}\n{callee_source}")
        } else {
            format!("{callee_source}\n{caller_source}")
        };
        let source = format!("data Record {{ value: u16; }}\n{declarations}");
        let checked = checked_from_source(&source);
        let caller = unit_plan(&checked, caller_name);
        let callee = unit_plan(&checked, "Record::replace");
        let [caller_receiver] = caller.structural_parameters.as_slice() else {
            panic!("forwarding must retain the caller's whole borrowed place: {source}")
        };
        let [callee_receiver] = callee.structural_parameters.as_slice() else {
            panic!("the callee store must retain its borrowed self: {source}")
        };
        let caller_position = u32::from(from_parameter && !self_caller);
        assert_eq!(caller_receiver.position, caller_position);
        assert_eq!(caller_receiver.is_self, self_caller);
        assert_eq!(caller_receiver.access, checked_access);
        assert_eq!(callee_receiver.position, 0);
        assert!(callee_receiver.is_self);
        assert_eq!(callee_receiver.access, checked_access);
        assert_eq!(caller_receiver.type_identity, callee_receiver.type_identity);

        let [
            CheckedUnitEffectOperationPlan::CallUnit {
                target_machine,
                scalar_arguments,
                structural_arguments,
                ..
            },
            CheckedUnitEffectOperationPlan::ReturnUnit { .. },
        ] = caller.operations.as_slice()
        else {
            panic!("caller retains exactly its Unit call and return: {source}")
        };
        assert_eq!(*target_machine, callee.machine);
        let [argument] = structural_arguments.as_slice() else {
            panic!("the retained callee receiver requires one exact caller argument: {source}")
        };
        assert_eq!(
            argument.source,
            CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index: 0 }
        );
        assert!(argument.path.is_empty());
        assert_eq!(argument.type_identity, caller_receiver.type_identity);
        assert_eq!(argument.access, checked_access);
        if from_parameter {
            let [caller_scalar] = caller.scalar_parameters.as_slice() else {
                panic!("one caller scalar formal")
            };
            let [callee_scalar] = callee.scalar_parameters.as_slice() else {
                panic!("one callee scalar formal")
            };
            assert_eq!(caller_scalar.source_position, u32::from(self_caller));
            assert_eq!(callee_scalar.source_position, 1);
            assert_eq!(
                caller_scalar.primitive_type,
                typed_trees::types::PrimitiveType::U16
            );
            assert_eq!(callee_scalar.primitive_type, caller_scalar.primitive_type);
            assert!(matches!(
                scalar_arguments.as_slice(),
                [CheckedCallScalarArgument::Pure(
                    CheckedScalarExpression::Parameter {
                        position: 0,
                        primitive_type: typed_trees::types::PrimitiveType::U16,
                    }
                )]
            ));
        } else {
            assert!(caller.scalar_parameters.is_empty());
            assert!(callee.scalar_parameters.is_empty());
            assert!(scalar_arguments.is_empty());
        }

        let artifact = terminal_production::produce_terminal_artifact(&checked, caller_name)
            .expect("receiver call reaches canonical Terminal production");
        drop(checked);
        let module = terminal_codec::decode_module(artifact.semantic_bytes())
            .expect("reload canonical receiver call semantics");
        let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes())
            .expect("reload canonical receiver call proof");
        assert_eq!(
            terminal_codec::encode_module(&module).unwrap(),
            artifact.semantic_bytes()
        );
        let profile = proof_admission::AdmissionProfile::default();
        let verified = terminal_verifier::verify_module(&module, &proof, &profile)
            .expect("decoded receiver call independently verifies");
        let certificate = terminal_fixed_fuel::derive_fixed_entry_fuel(&verified, module.entry)
            .expect("receiver call and store have fixed fuel");
        let caller = module
            .machines
            .iter()
            .find(|machine| machine.id == module.entry)
            .expect("Terminal caller entry");
        let [caller_receiver] = caller.structural_parameters.as_slice() else {
            panic!("Terminal caller retains one structural parameter")
        };
        // Terminal positions are dense within the separate structural namespace.
        assert_eq!(caller_receiver.position, 0);
        assert_eq!(caller_receiver.is_self, self_caller);
        assert_eq!(caller_receiver.access, access);
        assert_eq!(caller.result, TerminalMachineResult::Unit);
        let [caller_block] = caller.blocks.as_slice() else {
            panic!("one caller block")
        };
        assert!(matches!(
            caller_block.terminator,
            Terminator::ReturnUnit { .. }
        ));
        let [call] = caller_block.operations.as_slice() else {
            panic!("caller retains exactly the authored call")
        };
        let OperationKind::CallUnit {
            callee,
            arguments,
            structural_arguments,
            ..
        } = &call.kind
        else {
            panic!("ordinary receiver invocation remains a Unit call")
        };
        assert_eq!(call.result, OperationResult::Unit);
        let [argument] = structural_arguments.as_slice() else {
            panic!("Terminal call retains one whole receiver argument")
        };
        assert_eq!(argument.place, caller_receiver.place);
        assert!(argument.path.is_empty());
        assert_eq!(argument.access, access);
        let callee = module
            .machines
            .iter()
            .find(|machine| machine.id == *callee)
            .expect("exact call target");
        let [callee_receiver] = callee.structural_parameters.as_slice() else {
            panic!("callee signature retains self; erasure cannot repair call arity")
        };
        assert_eq!(callee_receiver.position, 0);
        assert!(callee_receiver.is_self);
        assert_eq!(callee_receiver.access, access);
        assert_eq!(
            callee_receiver.structural_type,
            caller_receiver.structural_type
        );
        assert_eq!(
            callee_receiver.multiplicity,
            StructuralMultiplicity::Unrestricted
        );
        assert!(callee_receiver.qualifications.is_empty());
        assert!(callee_receiver.projected_qualifications.is_empty());
        assert_eq!(callee.result, TerminalMachineResult::Unit);
        let record = module
            .structural_types
            .iter()
            .find(|declaration| declaration.id == callee_receiver.structural_type)
            .expect("declared receiver type");
        let StructuralTypeShape::Record { fields } = &record.shape else {
            panic!("receiver retains its record shape")
        };
        let [field] = fields.as_slice() else {
            panic!("one authored record field")
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
        assert!(matches!(
            callee_block.terminator,
            Terminator::ReturnUnit { .. }
        ));
        assert_eq!(
            callee_block.operations.len(),
            if from_parameter { 1 } else { 2 }
        );
        let store = callee_block.operations.last().expect("callee store");
        let OperationKind::StructuralScalarFieldStore {
            destination,
            path,
            field: stored_field,
            value,
        } = &store.kind
        else {
            panic!("callee retains the authored scalar field store")
        };
        assert_eq!(*destination, callee_receiver.place);
        assert!(path.is_empty());
        assert_eq!(*stored_field, field.id);
        assert_eq!(store.result, OperationResult::Unit);
        let scalar_arguments = if from_parameter {
            let [caller_scalar] = caller.parameters.as_slice() else {
                panic!("one caller scalar")
            };
            let [callee_scalar] = callee.parameters.as_slice() else {
                panic!("one callee scalar")
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
                panic!("typed literal result")
            };
            assert_eq!(*value, result.id);
            assert_eq!(result.scalar_type, ScalarType::Integer(integer_type));
            Vec::new()
        };

        // The interpreter accepts an opaque live structural argument for stores.
        // Completion and fuel establish execution, not public post-return field
        // observation or native pointer provisioning. No record constructor is used.
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
        .expect("canonical receiver call accepts its interpreter argument");
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
                    .expect("call and callee store are both charged")
                    .executions(),
                1
            );
        }
    }
}

#[test]
fn mutable_parameter_receiver_call_retains_whole_place() {
    assert_receiver_call(StructuralAccess::MutableBorrow, false, false);
}

#[test]
fn write_only_parameter_receiver_call_retains_whole_place() {
    assert_receiver_call(StructuralAccess::WriteOnlyBorrow, false, false);
}

#[test]
fn mutable_receiver_call_separates_scalar_and_structural_positions() {
    assert_receiver_call(StructuralAccess::MutableBorrow, true, false);
}

#[test]
fn write_only_receiver_call_separates_scalar_and_structural_positions() {
    assert_receiver_call(StructuralAccess::WriteOnlyBorrow, true, false);
}

#[test]
fn mutable_self_caller_retains_forwarded_receiver() {
    assert_receiver_call(StructuralAccess::MutableBorrow, true, true);
}

#[test]
fn write_only_self_caller_retains_forwarded_receiver() {
    assert_receiver_call(StructuralAccess::WriteOnlyBorrow, true, true);
}
