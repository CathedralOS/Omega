//! Caller observation of source-produced literal indexed write-only receivers.

use abstract_operations_to_target_operations::lower_to_target_operations;
use calling_conventions::{
    IndirectPointerLocation, MachineRegister, ValueClass, ValueLocation, ValuePlacement,
};
use image_emission::{
    build_installation_record, build_object_artifact, decode_installation_record,
    emit_executable_image, emit_object_container, encode_installation_record,
    validate_executable_image, validate_installation_record,
};
use machine_emission::emit_machine_code;
use proof_admission::AdmissionProfile;
use semantic_vocabulary::ProfileDecisionId;
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use target::NativeTarget;
use target_operations::{TargetOperation, TargetUnitOperation};
use target_operations_to_assigned_target_operations::assign_registers;
use terminal_codec::{decode_module, decode_proof_bundle, encode_module};
use terminal_psi::{OperationKind, StructuralAccess, StructuralPathSegment};
use terminal_psi_to_abstract_operations::lower_artifact_sections;
use terminal_verifier::verify_module;
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees_to_checked_trees::lower_typed_trees;

#[cfg(unix)]
use std::{
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    time::SystemTime,
};

// Reuse the host compiler/temporary-directory protocol without changing its owner.
#[cfg(unix)]
#[allow(dead_code)]
#[path = "terminal_psi_structural_return/affine_call_result/host.rs"]
mod host;

#[cfg(unix)]
#[path = "terminal_psi_indexed_receivers/observation.rs"]
mod observation;

#[cfg(unix)]
static NEXT_SCRATCH_DIRECTORY: AtomicU64 = AtomicU64::new(1);

#[cfg(unix)]
struct ScratchDirectory(PathBuf);

#[cfg(unix)]
impl Drop for ScratchDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct Fixture {
    declarations: &'static str,
    root_type: &'static str,
    receiver: &'static str,
    path: Vec<StructuralPathSegment>,
    root_bytes: u16,
    selected_offset: u32,
}

fn array_fixture() -> Fixture {
    Fixture {
        declarations: "",
        root_type: "[Record; 2]",
        receiver: "records[1]",
        path: vec![StructuralPathSegment::FixedIndex(1)],
        root_bytes: 4,
        selected_offset: 2,
    }
}

#[test]
fn literal_indexed_write_only_receiver_changes_the_callers_selected_record() {
    check_fixture(array_fixture(), StructuralAccess::WriteOnlyBorrow, false);
}

#[test]
fn mutable_array_forwards_a_write_only_indexed_receiver() {
    check_fixture(array_fixture(), StructuralAccess::MutableBorrow, false);
}

#[test]
fn nested_literal_indexes_preserve_the_callers_storage() {
    check_fixture(
        Fixture {
            declarations: "",
            root_type: "[[Record; 2]; 2]",
            receiver: "records[1][0]",
            path: vec![
                StructuralPathSegment::FixedIndex(1),
                StructuralPathSegment::FixedIndex(0),
            ],
            root_bytes: 8,
            selected_offset: 4,
        },
        StructuralAccess::WriteOnlyBorrow,
        false,
    );
}

#[test]
fn interleaved_fields_and_indexes_preserve_every_other_byte() {
    check_fixture(
        Fixture {
            declarations: "
                data Entry [copy] { before: u16; records: [Record; 2]; after: u16; }
                data Container [copy] { before: u16; entries: [Entry; 2]; after: u16; }",
            root_type: "Container",
            receiver: "records.entries[1].records[1]",
            path: vec![
                StructuralPathSegment::Field("entries".into()),
                StructuralPathSegment::FixedIndex(1),
                StructuralPathSegment::Field("records".into()),
                StructuralPathSegment::FixedIndex(1),
            ],
            root_bytes: 20,
            selected_offset: 14,
        },
        StructuralAccess::WriteOnlyBorrow,
        false,
    );
}

#[test]
fn scalar_replacement_prefix_retains_the_write_only_receiver_pointer() {
    check_fixture(array_fixture(), StructuralAccess::WriteOnlyBorrow, true);
}

#[test]
fn scalar_replacement_prefix_retains_the_mutable_source_pointer() {
    check_fixture(array_fixture(), StructuralAccess::MutableBorrow, true);
}

fn check_fixture(fixture: Fixture, source_access: StructuralAccess, scalar_prefix: bool) {
    let access = match source_access {
        StructuralAccess::WriteOnlyBorrow => "write",
        StructuralAccess::MutableBorrow => "mut",
        _ => unreachable!("exclusive receiver fixtures"),
    };
    let receiver_parameter = if scalar_prefix {
        ", replacement: u16"
    } else {
        ""
    };
    let caller_parameter = if scalar_prefix {
        "replacement: u16, "
    } else {
        ""
    };
    let replacement = if scalar_prefix { "replacement" } else { "17" };
    let actual = if scalar_prefix { "replacement" } else { "" };
    let source = format!(
        "data Record [copy] {{ value: u16; }}
         {}
         machine Record::replace(&write self{receiver_parameter}) {{ self.value = {replacement}; }}
         machine forward({caller_parameter}records: &{access} {}) {{ {}.replace({actual}); }}",
        fixture.declarations, fixture.root_type, fixture.receiver,
    );
    let tokens = Lexer::new(&source)
        .tokenize()
        .expect("tokenize receiver source");
    let syntax = parse_syntax_trees(&tokens).expect("parse receiver source");
    let resolved = lower_syntax_trees(&syntax).expect("resolve receiver source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type receiver source");
    let checked = lower_typed_trees(typed).expect("check receiver source");
    let artifact = terminal_production::produce_terminal_artifact(&checked, "forward")
        .expect("source producer retains the authored receiver loan");
    drop(checked);
    let module = decode_module(artifact.semantic_bytes()).expect("decode canonical Terminal");
    let proof = decode_proof_bundle(artifact.proof_bytes()).expect("decode canonical proof");
    assert_eq!(encode_module(&module).unwrap(), artifact.semantic_bytes());
    verify_module(&module, &proof, &AdmissionProfile::default()).unwrap();
    let caller = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let [root] = caller.structural_parameters.as_slice() else {
        panic!("one borrowed root")
    };
    assert_eq!(root.access, source_access);
    assert!(!root.is_self);
    assert!(root.qualifications.is_empty() && root.projected_qualifications.is_empty());
    let [block] = caller.blocks.as_slice() else {
        panic!("one caller block")
    };
    let [call] = block.operations.as_slice() else {
        panic!("one receiver call")
    };
    let OperationKind::CallUnit {
        callee,
        arguments,
        structural_arguments,
        claim_transfers,
        ..
    } = &call.kind
    else {
        panic!("ordinary source-produced Unit call")
    };
    assert_eq!(arguments.len(), usize::from(scalar_prefix));
    if scalar_prefix {
        assert_eq!(arguments[0], caller.parameters[0].id);
    }
    assert!(claim_transfers.is_empty());
    let [argument] = structural_arguments.as_slice() else {
        panic!("one receiver argument")
    };
    assert_eq!(argument.place, root.place);
    assert_eq!(argument.path, fixture.path);
    assert_eq!(argument.access, StructuralAccess::WriteOnlyBorrow);
    let receiver_machine = module
        .machines
        .iter()
        .find(|machine| machine.id == *callee)
        .unwrap();
    let [receiver] = receiver_machine.structural_parameters.as_slice() else {
        panic!("one receiver")
    };
    assert!(receiver.is_self);
    assert_eq!(receiver.position, 0);
    assert_eq!(receiver.access, StructuralAccess::WriteOnlyBorrow);
    assert_eq!(receiver_machine.attachment, Some(receiver.structural_type));
    assert!(receiver.qualifications.is_empty() && receiver.projected_qualifications.is_empty());

    let abstract_plan = lower_artifact_sections(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &AdmissionProfile::default(),
    )
    .expect("verified canonical artifact crosses the native boundary");
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let plan = lower_to_target_operations(&abstract_plan, target)
            .unwrap_or_else(|error| panic!("{target:?} receiver target lowering: {error:?}"));
        let function = plan
            .functions
            .iter()
            .find(|function| function.machine == module.entry)
            .unwrap();
        let TargetOperation::UnitBody(body) = &function.operation else {
            panic!("Unit caller")
        };
        let [parameter] = body.parameters.as_slice() else {
            panic!("one native root")
        };
        assert_eq!(parameter.place, root.place);
        assert_eq!(parameter.access, source_access);
        assert_eq!(parameter.shape.class, ValueClass::BorrowedReference);
        assert_eq!(parameter.shape.byte_size, fixture.root_bytes);
        let pointer_register = match (target == NativeTarget::linux_x64(), scalar_prefix) {
            (true, false) => MachineRegister::X86Rdi,
            (true, true) => MachineRegister::X86Rsi,
            (false, false) => MachineRegister::Aarch64X(0),
            (false, true) => MachineRegister::Aarch64X(1),
        };
        assert_pointer(&parameter.placement, pointer_register);
        assert_eq!(
            body.call_plan.parameters.len(),
            1 + usize::from(scalar_prefix)
        );
        assert_eq!(
            body.call_plan.parameters[usize::from(scalar_prefix)],
            parameter.placement
        );
        let TargetUnitOperation::Call {
            psi_operation,
            callee: native_callee,
            arguments: native_arguments,
            scalar_arguments,
            call_plan,
            ..
        } = &body.operations[0]
        else {
            panic!("native receiver call")
        };
        assert_eq!(*psi_operation, call.id);
        assert_eq!(*native_callee, *callee);
        assert_eq!(scalar_arguments.len(), usize::from(scalar_prefix));
        if scalar_prefix {
            assert_eq!(scalar_arguments[0].source.source_value(), arguments[0]);
            assert_eq!(scalar_arguments[0].placement, call_plan.parameters[0]);
        }
        let [native_argument] = native_arguments.as_slice() else {
            panic!("one native receiver")
        };
        assert_eq!(native_argument.place, root.place);
        assert_eq!(native_argument.path, fixture.path);
        assert_eq!(native_argument.access, StructuralAccess::WriteOnlyBorrow);
        assert_eq!(native_argument.root_structural_type, root.structural_type);
        assert_eq!(native_argument.structural_type, receiver.structural_type);
        assert_eq!(native_argument.source_byte_offset, fixture.selected_offset);
        assert_eq!(native_argument.fixed_array_length, None);
        assert_eq!(native_argument.element_stride, None);
        assert_eq!(native_argument.source, parameter.placement);
        assert_eq!(native_argument.shape.class, ValueClass::BorrowedReference);
        assert_eq!(native_argument.shape.byte_size, 2);
        assert_pointer(&native_argument.destination, pointer_register);
        assert_eq!(
            native_argument.destination,
            call_plan.parameters[usize::from(scalar_prefix)]
        );

        let assigned = assign_registers(&plan).expect("assignment replays the indexed loan");
        let emitted = emit_machine_code(&assigned).expect("emit indexed receiver call and store");
        let emitted_caller = emitted
            .functions
            .iter()
            .find(|function| function.machine == module.entry)
            .unwrap();
        let [native_call] = emitted_caller.internal_unit_calls.as_slice() else {
            panic!("one relocated call")
        };
        assert_eq!(native_call.target, *callee);
        let [record] = native_call.arguments.as_slice() else {
            panic!("one retained argument")
        };
        assert_eq!(record.place, native_argument.place);
        assert_eq!(record.path, native_argument.path);
        assert_eq!(record.access, native_argument.access);
        assert_eq!(
            record.root_structural_type,
            native_argument.root_structural_type
        );
        assert_eq!(record.structural_type, native_argument.structural_type);
        assert_eq!(record.source_byte_offset, fixture.selected_offset);
        assert_eq!(record.source, native_argument.source);
        assert_eq!(record.destination, native_argument.destination);

        let object = build_object_artifact(&emitted)
            .expect("object replay validates exact receiver geometry");
        let container = emit_object_container(&object);
        assert_eq!(container.psi, abstract_plan.psi);
        let image = emit_executable_image(&object, 3).expect("relocate the complete native text");
        validate_executable_image(&object, &image)
            .expect("validate relocated image before execution");
        let installation =
            build_installation_record(&image, ProfileDecisionId::new(1).unwrap()).unwrap();
        validate_installation_record(&installation, &image)
            .expect("installed replay validates the receiver");
        let installed_call = installation
            .internal_unit_calls()
            .iter()
            .find(|call| call.machine == module.entry)
            .unwrap();
        assert_eq!(installed_call.custody, *native_call);
        let decoded =
            decode_installation_record(&encode_installation_record(&installation).unwrap())
                .unwrap();
        assert_eq!(decoded, installation);
        validate_installation_record(&decoded, &image).unwrap();

        let mut changed = installation.clone();
        let changed_call = changed
            .internal_unit_calls_mut_for_test()
            .iter_mut()
            .find(|call| call.machine == module.entry)
            .unwrap();
        // The previous sibling is also in bounds, but is not the retained path.
        changed_call.custody.arguments[0].source_byte_offset -= 2;
        assert!(
            validate_installation_record(&changed, &image).is_err(),
            "{target:?} installed replay must reject a substituted receiver offset"
        );

        #[cfg(unix)]
        if observation::host_matches(target) {
            observation::execute(
                &image,
                object.entry_function().text_offset,
                &fixture,
                scalar_prefix,
            );
            continue;
        }
        eprintln!(
            "SKIP native execution of {target:?}: host {}-{} cannot run this fixture; cross-emission and artifact replay completed",
            std::env::consts::OS,
            std::env::consts::ARCH
        );
    }
}

fn assert_pointer(placement: &ValuePlacement, register: MachineRegister) {
    assert_eq!(placement.shape.class, ValueClass::BorrowedReference);
    assert!(
        matches!(placement.locations.as_slice(), [ValueLocation::Indirect {
            pointer: IndirectPointerLocation::Register(actual),
            copy_stack_byte_offset: None,
            byte_size,
            alignment,
        }] if *actual == register
            && *byte_size == placement.shape.byte_size
            && *alignment == placement.shape.alignment)
    );
}
