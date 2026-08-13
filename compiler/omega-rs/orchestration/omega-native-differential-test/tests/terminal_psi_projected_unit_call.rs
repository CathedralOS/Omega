use omega_calling_conventions::ValueShape;
use omega_target::NativeTarget;
use omega_terminal_abstract_operations_to_target_operations::lower_to_target_operations;
use omega_terminal_image_emission::{
    TerminalInstallationError, build_terminal_installation_record, build_terminal_object_artifact,
    decode_terminal_installation_record, derive_terminal_stack_demand,
    emit_terminal_executable_image, encode_terminal_installation_record,
    validate_terminal_installation_record,
};
use omega_terminal_machine_emission::emit_machine_code;
use omega_terminal_psi_to_abstract_operations::lower_artifact_sections;
use omega_terminal_target_operations::TerminalCallSiteOwner;
use omega_terminal_target_operations::TerminalTargetOperation;
use omega_terminal_target_operations_to_assigned_target_operations::assign_registers;
use psi_checked_trees_to_terminal::lower_machine;
use psi_core::{ClaimId, ProfileDecisionId};
use psi_proof_kernel::AdmissionProfile;
use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_terminal::{StructuralPathSegment, StructuralTypeShape};
use psi_terminal_codec::{encode_module, encode_proof_bundle};
use psi_tokens_to_syntax_trees::parse_syntax_trees;
use psi_typed_trees_to_checked_trees::lower_typed_trees;

const SOURCE: &str = r#"
    boundary trait PortIo {}
    data Receipt [linear] { value: u64; }

    boundary machine Receipt::settle(self)
    reaches PortIo
    ensures true;

    data Helper {}
    machine Helper::run(receipt: Receipt)
    reaches PortIo
    {
        Receipt::settle(receipt);
    }

    data Root {}
    machine Root::enter(receipts: [Receipt; 2])
    reaches PortIo
    {
        Helper::run(receipts[0]);
        Helper::run(receipts[1]);
    }
"#;

const PARTIAL_AFFINE_SOURCE: &str = r#"
    data LeftToken { value: u32; }
    data RightToken { value: u64; }
    data Pair { left: LeftToken; right: RightToken; }
    data Helper {}
    machine Helper::take(token: RightToken) {}
    data Root {}
    machine Root::enter(pair: Pair) {
        Helper::take(pair.right);
    }
"#;

const NOMINAL_AFFINE_SOURCE: &str = r#"
    data Token { first: u64; second: u64; third: u64; fourth: u64; fifth: u64; }
    data CleanupHelper {}
    machine CleanupHelper::run() {}
    machine Token::drop(&mut self) { CleanupHelper::run(); }
    data Root {}
    machine Root::enter(token: Token) {}
"#;

fn verified_plan() -> omega_terminal_abstract_operations::TerminalAbstractOperationPlan {
    let tokens = Lexer::new(SOURCE).tokenize().expect("tokenize source");
    let syntax = parse_syntax_trees(&tokens).expect("parse source");
    let resolved = lower_syntax_trees(&syntax).expect("resolve source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type source");
    let checked = lower_typed_trees(typed).expect("check source");
    let terminal = lower_machine(&checked, "Root::enter").expect("lower terminal Psi");
    let semantics = encode_module(&terminal.semantic_module).expect("encode terminal semantics");
    let proof = encode_proof_bundle(&terminal.proof_bundle).expect("encode terminal proof");
    lower_artifact_sections(&semantics, &proof, &AdmissionProfile::default())
        .expect("canonical terminal artifact verifies and lowers into Omega")
}

fn backend_projection_plan() -> omega_terminal_abstract_operations::TerminalAbstractOperationPlan {
    let mut plan = verified_plan();
    // Provider settlement is covered by the admitted-effect suite and its
    // current native realization is deliberately x86-only. This test isolates
    // the portable internal-call carrier after the canonical Psi-to-Omega seam.
    for function in &mut plan.functions {
        function.operations.retain(|operation| {
            !matches!(
                operation,
                omega_terminal_abstract_operations::TerminalAbstractOperation::BoundaryCallUnit {
                    ..
                }
            )
        });
    }
    plan.boundary_machines.clear();
    plan
}

fn partial_affine_plan() -> omega_terminal_abstract_operations::TerminalAbstractOperationPlan {
    let tokens = Lexer::new(PARTIAL_AFFINE_SOURCE)
        .tokenize()
        .expect("tokenize partial affine source");
    let syntax = parse_syntax_trees(&tokens).expect("parse partial affine source");
    let resolved = lower_syntax_trees(&syntax).expect("resolve partial affine source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type partial affine source");
    let checked = lower_typed_trees(typed).expect("check partial affine source");
    let terminal = lower_machine(&checked, "Root::enter").expect("lower partial affine Psi");
    let semantics = encode_module(&terminal.semantic_module).expect("encode partial affine Psi");
    let proof = encode_proof_bundle(&terminal.proof_bundle).expect("encode partial affine proof");
    lower_artifact_sections(&semantics, &proof, &AdmissionProfile::default())
        .expect("verified partial affine artifact enters Omega")
}

fn nominal_affine_plan() -> omega_terminal_abstract_operations::TerminalAbstractOperationPlan {
    let tokens = Lexer::new(NOMINAL_AFFINE_SOURCE)
        .tokenize()
        .expect("tokenize nominal affine source");
    let syntax = parse_syntax_trees(&tokens).expect("parse nominal affine source");
    let resolved = lower_syntax_trees(&syntax).expect("resolve nominal affine source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type nominal affine source");
    let checked = lower_typed_trees(typed).expect("check nominal affine source");
    let terminal = lower_machine(&checked, "Root::enter").expect("lower nominal affine Psi");
    let semantics = encode_module(&terminal.semantic_module).expect("encode nominal affine Psi");
    let proof = encode_proof_bundle(&terminal.proof_bundle).expect("encode nominal affine proof");
    lower_artifact_sections(&semantics, &proof, &AdmissionProfile::default())
        .expect("verified nominal affine artifact enters Omega")
}

#[test]
fn canonical_verified_artifact_delivers_projected_calls_to_omega() {
    let plan = verified_plan();
    let caller = &plan.functions[0];
    let calls = caller
        .operations
        .iter()
        .filter_map(|operation| match operation {
            omega_terminal_abstract_operations::TerminalAbstractOperation::CallUnit {
                structural_arguments,
                claim_transfers,
                ..
            } => Some((structural_arguments, claim_transfers)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 2);
    for (index, (arguments, transfers)) in calls.into_iter().enumerate() {
        assert_eq!(
            arguments[0].path,
            [StructuralPathSegment::FixedIndex(index as u64)]
        );
        assert_eq!(transfers[0].claim, ClaimId::new(index as u64 + 1).unwrap());
    }
}

#[test]
fn literal_element_calls_retain_native_and_installed_custody_on_all_targets() {
    let plan = backend_projection_plan();
    let caller_parameter = plan.functions[0].structural_parameters[0].structural_type;
    let root_declaration = plan
        .structural_types
        .iter()
        .find(|declaration| declaration.id == caller_parameter)
        .expect("caller array type remains declared");
    let StructuralTypeShape::FixedArray {
        element: element_type,
        length: 2,
    } = &root_declaration.shape
    else {
        panic!("caller parameter must remain a two-element fixed array")
    };
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let target_plan = lower_to_target_operations(&plan, target).unwrap();
        let TerminalTargetOperation::UnitBody(caller) = &target_plan.functions[0].operation else {
            panic!("caller must remain Unit")
        };
        assert_eq!(caller.parameters[0].shape, ValueShape::integer(16, 8));
        let assigned = assign_registers(&target_plan).unwrap();
        let machine = emit_machine_code(&assigned).unwrap();
        let custody = &machine.functions[0].internal_unit_calls;
        assert_eq!(custody.len(), 2);
        assert_eq!(custody[0].arguments[0].source_byte_offset, 0);
        assert_eq!(custody[1].arguments[0].source_byte_offset, 8);
        for (index, call) in custody.iter().enumerate() {
            let argument = &call.arguments[0];
            assert_eq!(
                argument.path,
                [StructuralPathSegment::FixedIndex(index as u64)]
            );
            assert_eq!(argument.root_structural_type, caller_parameter);
            assert_eq!(argument.structural_type, *element_type);
            assert_eq!(argument.shape, ValueShape::integer(8, 8));
            assert_eq!(argument.source.shape, ValueShape::integer(16, 8));
            assert_eq!(argument.fixed_array_length, Some(2));
            assert_eq!(argument.element_stride, Some(8));
            assert!(argument.byte_count != 0);
            if target == NativeTarget::windows_x64() {
                assert!(matches!(
                    argument.source.locations.as_slice(),
                    [omega_calling_conventions::ValueLocation::Indirect { .. }]
                ));
            } else {
                assert!(argument.source.locations.iter().all(|location| !matches!(
                    location,
                    omega_calling_conventions::ValueLocation::Indirect { .. }
                )));
            }
            assert_eq!(
                call.claim_transfers[0].claim,
                ClaimId::new(index as u64 + 1).unwrap()
            );
        }

        let mut changed_offset = machine.clone();
        changed_offset.functions[0].internal_unit_calls[1].arguments[0].source_byte_offset = 0;
        assert!(build_terminal_object_artifact(&changed_offset).is_err());
        let mut changed_path = machine.clone();
        changed_path.functions[0].internal_unit_calls[1].arguments[0].path =
            vec![StructuralPathSegment::FixedIndex(0)];
        assert!(build_terminal_object_artifact(&changed_path).is_err());
        let mut dropped_claim = machine.clone();
        dropped_claim.functions[0].internal_unit_calls[1]
            .claim_transfers
            .clear();
        assert!(build_terminal_object_artifact(&dropped_claim).is_err());
        let mut duplicated_custody = machine.clone();
        duplicated_custody.functions[0].internal_unit_calls[1] =
            duplicated_custody.functions[0].internal_unit_calls[0].clone();
        assert!(build_terminal_object_artifact(&duplicated_custody).is_err());
        let mut changed_copy_byte = machine.clone();
        let copy_offset =
            changed_copy_byte.functions[0].internal_unit_calls[1].arguments[0].code_offset;
        changed_copy_byte.functions[0].bytes[copy_offset] ^= 1;
        assert!(build_terminal_object_artifact(&changed_copy_byte).is_err());
        let mut paired_copy_mutation = machine.clone();
        let copy_offset =
            paired_copy_mutation.functions[0].internal_unit_calls[1].arguments[0].code_offset;
        paired_copy_mutation.functions[0].internal_unit_calls[1].arguments[0].bytes[0] ^= 1;
        paired_copy_mutation.functions[0].bytes[copy_offset] ^= 1;
        assert!(build_terminal_object_artifact(&paired_copy_mutation).is_err());

        let first_copy_offset =
            machine.functions[0].internal_unit_calls[0].arguments[0].code_offset;
        let mut forged_home = machine.clone();
        forged_home.functions[0].internal_unit_calls[0].arguments[0].source_home_byte_offset = 8;
        forged_home.functions[0].internal_unit_calls[0].arguments[0].bytes[0] ^= 1;
        forged_home.functions[0].bytes[first_copy_offset] ^= 1;
        assert!(build_terminal_object_artifact(&forged_home).is_err());
        let mut forged_call_stack = machine.clone();
        forged_call_stack.functions[0].internal_unit_calls[0].arguments[0].call_stack_bytes += 8;
        forged_call_stack.functions[0].internal_unit_calls[0].arguments[0].bytes[0] ^= 1;
        forged_call_stack.functions[0].bytes[first_copy_offset] ^= 1;
        assert!(build_terminal_object_artifact(&forged_call_stack).is_err());

        let object = build_terminal_object_artifact(&machine).unwrap();
        let image = emit_terminal_executable_image(&object, 3).unwrap();
        let installation =
            build_terminal_installation_record(&image, ProfileDecisionId::new(1).unwrap()).unwrap();
        assert_eq!(installation.internal_unit_calls().len(), 2);
        validate_terminal_installation_record(&installation, &image).unwrap();
        let bytes = encode_terminal_installation_record(&installation).unwrap();
        let installed_argument = installation.internal_unit_calls()[1].custody.arguments[0].clone();
        assert_eq!(
            decode_terminal_installation_record(&bytes),
            Ok(installation.clone())
        );
        let mut projection = Vec::new();
        projection.extend_from_slice(&installed_argument.place.get().to_le_bytes());
        projection.extend_from_slice(&1_u32.to_le_bytes());
        projection.extend_from_slice(&[2, 0, 0, 0]);
        projection.extend_from_slice(&1_u64.to_le_bytes());
        projection.extend_from_slice(&installed_argument.root_structural_type.get().to_le_bytes());
        projection.extend_from_slice(&installed_argument.structural_type.get().to_le_bytes());
        projection.extend_from_slice(&[1, 0, 8, 0, 8, 0, 0, 0]);
        projection.extend_from_slice(&installed_argument.source_byte_offset.to_le_bytes());
        let offset = bytes
            .windows(projection.len())
            .position(|window| window == projection)
            .expect("format-9 bytes retain the second resolved projection");
        let mut changed_installation = bytes.clone();
        let source_offset = offset + 48;
        changed_installation[source_offset..source_offset + 4]
            .copy_from_slice(&0_u32.to_le_bytes());
        assert!(decode_terminal_installation_record(&changed_installation).is_err());
    }
}

#[test]
fn partial_affine_field_cleanup_is_zero_code_and_installed_on_all_targets() {
    let plan = partial_affine_plan();
    let caller_machine = plan.entry;
    let caller = plan
        .functions
        .iter()
        .find(|function| function.machine == caller_machine)
        .expect("entry caller remains present");
    let pair_type = caller.structural_parameters[0].structural_type;
    let residual = caller
        .operations
        .iter()
        .find_map(|operation| match operation {
            omega_terminal_abstract_operations::TerminalAbstractOperation::ReturnUnit {
                trivial_affine_discards,
                residual_affine_discards,
                ..
            } => {
                assert!(trivial_affine_discards.is_empty());
                Some(residual_affine_discards[0].clone())
            }
            _ => None,
        })
        .expect("partial return retains one residual cleanup");
    assert_eq!(residual.path, [StructuralPathSegment::Field("left".into())]);

    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let target_plan = lower_to_target_operations(&plan, target).unwrap();
        let assigned = assign_registers(&target_plan).unwrap();
        let machine = emit_machine_code(&assigned).unwrap();
        let emitted = machine
            .functions
            .iter()
            .find(|function| function.machine == caller_machine)
            .expect("caller machine code exists");
        let [call] = emitted.internal_unit_calls.as_slice() else {
            panic!("caller has one projected internal call")
        };
        let [argument] = call.arguments.as_slice() else {
            panic!("call has one argument")
        };
        assert_eq!(argument.root_structural_type, pair_type);
        assert_eq!(
            argument.path,
            [StructuralPathSegment::Field("right".into())]
        );
        assert_eq!(argument.source_byte_offset, 8);
        assert_ne!(argument.structural_type, residual.structural_type);

        let cleanup = emitted
            .unit_affine_cleanup
            .as_ref()
            .expect("caller retains cleanup ledger");
        assert!(cleanup.discards.is_empty());
        assert_eq!(cleanup.residual_discards, [residual.clone()]);
        let mut root_cleanup_assigned = assigned.clone();
        let root_cleanup_caller = root_cleanup_assigned
            .functions
            .iter_mut()
            .find(|function| function.machine == caller_machine)
            .unwrap();
        let omega_terminal_assigned_target_operations::TerminalAssignedOperation::UnitBody(body) =
            &mut root_cleanup_caller.operation
        else {
            panic!("caller remains a Unit body")
        };
        let omega_terminal_assigned_target_operations::TerminalAssignedUnitOperation::Return {
            trivial_affine_discards,
            residual_affine_discards,
            ..
        } = body.operations.last_mut().unwrap()
        else {
            panic!("caller ends in a Unit return")
        };
        *trivial_affine_discards = vec![residual.place];
        residual_affine_discards.clear();
        let root_cleanup_machine = emit_machine_code(&root_cleanup_assigned).unwrap();
        let root_cleanup_bytes = &root_cleanup_machine
            .functions
            .iter()
            .find(|function| function.machine == caller_machine)
            .unwrap()
            .bytes;
        assert_eq!(
            &emitted.bytes, root_cleanup_bytes,
            "path-sensitive cleanup adds no runtime instruction bytes"
        );

        let mut forged_path = machine.clone();
        forged_path
            .functions
            .iter_mut()
            .find(|function| function.machine == caller_machine)
            .unwrap()
            .unit_affine_cleanup
            .as_mut()
            .unwrap()
            .residual_discards[0]
            .path = vec![StructuralPathSegment::Field("right".into())];
        assert!(build_terminal_object_artifact(&forged_path).is_err());
        let mut forged_type = machine.clone();
        forged_type
            .functions
            .iter_mut()
            .find(|function| function.machine == caller_machine)
            .unwrap()
            .unit_affine_cleanup
            .as_mut()
            .unwrap()
            .residual_discards[0]
            .structural_type = pair_type;
        assert!(build_terminal_object_artifact(&forged_type).is_err());

        let object = build_terminal_object_artifact(&machine).unwrap();
        let image = emit_terminal_executable_image(&object, 3).unwrap();
        let installation =
            build_terminal_installation_record(&image, ProfileDecisionId::new(1).unwrap()).unwrap();
        let installed_cleanup = installation
            .functions()
            .iter()
            .find(|function| function.machine == caller_machine)
            .unwrap()
            .unit_affine_cleanup
            .as_ref()
            .unwrap();
        assert_eq!(installed_cleanup.residual_discards, [residual.clone()]);
        validate_terminal_installation_record(&installation, &image).unwrap();
        let bytes = encode_terminal_installation_record(&installation).unwrap();
        assert_eq!(
            decode_terminal_installation_record(&bytes),
            Ok(installation.clone())
        );
    }
}

#[test]
fn wide_flat_nominal_affine_cleanup_executes_and_is_installed_on_all_targets() {
    let plan = nominal_affine_plan();
    let caller_machine = plan.entry;
    let caller = plan
        .functions
        .iter()
        .find(|function| function.machine == caller_machine)
        .expect("entry caller remains present");
    let cleanup = caller
        .operations
        .iter()
        .find_map(|operation| match operation {
            omega_terminal_abstract_operations::TerminalAbstractOperation::ReturnUnit {
                trivial_affine_discards,
                residual_affine_discards,
                nominal_affine_cleanup,
                ..
            } => {
                assert!(trivial_affine_discards.is_empty());
                assert!(residual_affine_discards.is_empty());
                *nominal_affine_cleanup
            }
            _ => None,
        })
        .expect("entry return retains exact nominal cleanup");
    assert_eq!(caller.structural_parameters.len(), 1);
    assert_eq!(cleanup.place, caller.structural_parameters[0].place);
    assert_eq!(
        cleanup.structural_type,
        caller.structural_parameters[0].structural_type
    );
    let cleanup_function = plan
        .functions
        .iter()
        .find(|function| function.machine == cleanup.cleanup_machine)
        .expect("cleanup closure remains in the Omega plan");
    assert_eq!(cleanup_function.attachment, Some(cleanup.structural_type));

    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let target_plan = lower_to_target_operations(&plan, target).unwrap();
        let target_caller = target_plan
            .functions
            .iter()
            .find(|function| function.machine == caller_machine)
            .unwrap();
        let TerminalTargetOperation::UnitBody(target_body) = &target_caller.operation else {
            panic!("caller remains Unit")
        };
        assert_eq!(target_body.parameters[0].shape, ValueShape::integer(40, 8));
        assert!(!target_body.parameters[0].placement.locations.is_empty());
        let omega_terminal_target_operations::TerminalTargetUnitOperation::Return {
            trivial_affine_discards,
            residual_affine_discards,
            nominal_affine_cleanup,
            ..
        } = target_body.operations.last().unwrap()
        else {
            panic!("caller ends in a Unit return")
        };
        assert!(trivial_affine_discards.is_empty());
        assert!(residual_affine_discards.is_empty());
        assert_eq!(*nominal_affine_cleanup, Some(cleanup));

        let assigned = assign_registers(&target_plan).unwrap();
        let machine = emit_machine_code(&assigned).unwrap();
        let emitted = machine
            .functions
            .iter()
            .find(|function| function.machine == caller_machine)
            .unwrap();
        let emitted_cleanup = emitted.unit_affine_cleanup.as_ref().unwrap();
        assert!(emitted_cleanup.locals.is_empty());
        assert!(emitted_cleanup.discards.is_empty());
        assert!(emitted_cleanup.residual_discards.is_empty());
        assert_eq!(emitted_cleanup.nominal_cleanup, Some(cleanup));
        let cleanup_call = emitted
            .internal_unit_calls
            .iter()
            .find(|call| call.owner == TerminalCallSiteOwner::Edge(emitted_cleanup.psi_edge))
            .expect("cleanup edge owns one native Unit call");
        assert_eq!(cleanup_call.target, cleanup.cleanup_machine);
        assert!(cleanup_call.arguments.is_empty());
        assert!(cleanup_call.claim_transfers.is_empty());
        let relocation = emitted
            .internal_calls
            .iter()
            .find(|call| call.owner == cleanup_call.owner)
            .expect("cleanup call retains a relocation");
        assert_eq!(relocation.target, cleanup.cleanup_machine);
        assert!(relocation.unit_stack.is_some());
        assert!(emitted_cleanup.code_offset <= cleanup_call.code_offset);
        assert!(
            cleanup_call.code_offset + cleanup_call.byte_count
                <= emitted_cleanup.code_offset + emitted_cleanup.byte_count
        );
        assert_eq!(
            machine
                .functions
                .iter()
                .find(|function| function.machine == cleanup.cleanup_machine)
                .unwrap()
                .attachment,
            Some(cleanup.structural_type)
        );

        let mut forged_place = machine.clone();
        forged_place
            .functions
            .iter_mut()
            .find(|function| function.machine == caller_machine)
            .unwrap()
            .unit_affine_cleanup
            .as_mut()
            .unwrap()
            .nominal_cleanup
            .as_mut()
            .unwrap()
            .place = psi_core::PlaceId::new(cleanup.place.get() + 1).unwrap();
        assert!(build_terminal_object_artifact(&forged_place).is_err());
        let mut forged_target = machine.clone();
        forged_target
            .functions
            .iter_mut()
            .find(|function| function.machine == cleanup.cleanup_machine)
            .unwrap()
            .attachment = None;
        assert!(build_terminal_object_artifact(&forged_target).is_err());

        let object = build_terminal_object_artifact(&machine).unwrap();
        let expected_stack_bytes = match target {
            target
                if target == NativeTarget::windows_x64() || target == NativeTarget::uefi_x64() =>
            {
                112
            }
            target if target == NativeTarget::linux_x64() => 80,
            _ => 48,
        };
        assert_eq!(
            derive_terminal_stack_demand(&object, caller_machine)
                .expect("executable cleanup stack closure")
                .ceiling_bytes(),
            expected_stack_bytes,
            "the wide receiver and both nested calls compose exactly for {target:?}"
        );
        let image = emit_terminal_executable_image(&object, 3).unwrap();
        let installation =
            build_terminal_installation_record(&image, ProfileDecisionId::new(1).unwrap()).unwrap();
        let installed = installation
            .functions()
            .iter()
            .find(|function| function.machine == caller_machine)
            .unwrap();
        assert_eq!(
            installed
                .unit_affine_cleanup
                .as_ref()
                .unwrap()
                .nominal_cleanup,
            Some(cleanup)
        );
        assert_eq!(
            installation
                .functions()
                .iter()
                .find(|function| function.machine == cleanup.cleanup_machine)
                .unwrap()
                .attachment,
            Some(cleanup.structural_type)
        );
        validate_terminal_installation_record(&installation, &image).unwrap();
        let bytes = encode_terminal_installation_record(&installation).unwrap();
        assert_eq!(
            decode_terminal_installation_record(&bytes),
            Ok(installation.clone())
        );
        if target == NativeTarget::linux_x64() {
            let native_cleanup = installed.unit_affine_cleanup.as_ref().unwrap();
            let mut encoded_cleanup = vec![1];
            encoded_cleanup.extend_from_slice(&cleanup.place.get().to_le_bytes());
            encoded_cleanup.extend_from_slice(&cleanup.structural_type.get().to_le_bytes());
            encoded_cleanup.extend_from_slice(&cleanup.cleanup_machine.get().to_le_bytes());
            encoded_cleanup.extend_from_slice(&(native_cleanup.code_offset as u64).to_le_bytes());
            encoded_cleanup.extend_from_slice(&(native_cleanup.byte_count as u64).to_le_bytes());
            let matches = bytes
                .windows(encoded_cleanup.len())
                .enumerate()
                .filter(|(_, window)| *window == encoded_cleanup)
                .map(|(offset, _)| offset)
                .collect::<Vec<_>>();
            assert_eq!(matches.len(), 1, "nominal cleanup encoding is unique");
            let offset = matches[0];
            let mut invalid_presence = bytes.clone();
            invalid_presence[offset] = 2;
            assert_eq!(
                decode_terminal_installation_record(&invalid_presence),
                Err(TerminalInstallationError::InvalidPresenceFlag(2))
            );
            let mut zero_place = bytes.clone();
            zero_place[offset + 1..offset + 9].fill(0);
            assert_eq!(
                decode_terminal_installation_record(&zero_place),
                Err(TerminalInstallationError::ZeroStructuralReturnIdentity(
                    "nominal Unit cleanup place"
                ))
            );
        }
    }
}
