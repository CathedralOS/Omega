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
use psi_core::{ClaimId, OperationId, PlaceId, ProfileDecisionId, StructuralPlaceKind};
use psi_proof_kernel::AdmissionProfile;
use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_terminal::{
    StructuralPathSegment, StructuralPlaceDeclaration, StructuralTypeShape,
    TerminalAffineCleanupAction,
};
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

const WIDE_PARTIAL_AFFINE_SOURCE: &str = r#"
    data LeftToken { value: u32; }
    data MiddleToken { value: u64; }
    data RightToken { value: u64; }
    data Triple { left: LeftToken; middle: MiddleToken; right: RightToken; }
    data Helper {}
    machine Helper::take(token: RightToken) {}
    data Root {}
    machine Root::enter(triple: Triple) {
        Helper::take(triple.right);
    }
"#;

const NOMINAL_AFFINE_SOURCE: &str = r#"
    data Token { first: u64; second: u64; third: u64; fourth: u64; fifth: u64; }
    data FirstCleanupHelper {}
    machine FirstCleanupHelper::run() {}
    data SecondCleanupHelper {}
    machine SecondCleanupHelper::run() {}
    data ThirdCleanupHelper {}
    machine ThirdCleanupHelper::run() {}
    machine Token::drop(&mut self) {
        FirstCleanupHelper::run();
        SecondCleanupHelper::run();
        ThirdCleanupHelper::run();
    }
    data Root {}
    machine Root::enter(token: Token) {}
"#;

const TWO_EMPTY_NOMINAL_AFFINE_SOURCE: &str = r#"
    data Token {}
    machine Token::drop(&mut self) {}
    data Root {}
    machine Root::enter(first: Token, second: Token) {}
"#;

const FIRST_EXECUTABLE_NOMINAL_AFFINE_SOURCE: &str = r#"
    data Helper {}
    machine Helper::touch() {}
    data First { value: u32; }
    machine First::drop(&mut self) { Helper::touch(); }
    data Second { value: u64; }
    machine Second::drop(&mut self) {}
    data Root {}
    machine Root::enter(first: First, second: Second) {}
"#;

const SECOND_EXECUTABLE_NOMINAL_AFFINE_SOURCE: &str = r#"
    data Helper {}
    machine Helper::touch() {}
    data First { value: u32; }
    machine First::drop(&mut self) {}
    data Second { value: u64; }
    machine Second::drop(&mut self) { Helper::touch(); }
    data Root {}
    machine Root::enter(first: First, second: Second) {}
"#;

const TWO_EXECUTABLE_NOMINAL_AFFINE_SOURCE: &str = r#"
    data Helper {}
    machine Helper::touch() {}
    data First { value: u32; }
    machine First::drop(&mut self) { Helper::touch(); }
    data Second { value: u64; }
    machine Second::drop(&mut self) { Helper::touch(); }
    data Root {}
    machine Root::enter(first: First, second: Second) {}
"#;

const SHARED_EXECUTABLE_NOMINAL_AFFINE_SOURCE: &str = r#"
    data Helper {}
    machine Helper::touch() {}
    data Token { value: u32; }
    machine Token::drop(&mut self) { Helper::touch(); }
    data Root {}
    machine Root::enter(first: Token, second: Token) {}
"#;

const THREE_SHARED_EXECUTABLE_NOMINAL_AFFINE_SOURCE: &str = r#"
    data Helper {}
    machine Helper::touch() {}
    data Token { value: u32; }
    machine Token::drop(&mut self) { Helper::touch(); }
    data Root {}
    machine Root::enter(first: Token, second: Token, third: Token) {}
"#;

const FIVE_SHARED_EXECUTABLE_NOMINAL_AFFINE_SOURCE: &str = r#"
    data FirstHelper {}
    machine FirstHelper::touch() {}
    data SecondHelper {}
    machine SecondHelper::touch() {}
    data ThirdHelper {}
    machine ThirdHelper::touch() {}
    data FourthHelper {}
    machine FourthHelper::touch() {}
    data FifthHelper {}
    machine FifthHelper::touch() {}
    data Token { value: u32; }
    machine Token::drop(&mut self) {
        FirstHelper::touch();
        SecondHelper::touch();
        ThirdHelper::touch();
        FourthHelper::touch();
        FifthHelper::touch();
    }
    data Root {}
    machine Root::enter(
        first: Token,
        second: Token,
        third: Token,
        fourth: Token,
        fifth: Token
    ) {}
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

fn wide_partial_affine_plan() -> omega_terminal_abstract_operations::TerminalAbstractOperationPlan {
    let tokens = Lexer::new(WIDE_PARTIAL_AFFINE_SOURCE)
        .tokenize()
        .expect("tokenize wide partial affine source");
    let syntax = parse_syntax_trees(&tokens).expect("parse wide partial affine source");
    let resolved = lower_syntax_trees(&syntax).expect("resolve wide partial affine source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type wide partial affine source");
    let checked = lower_typed_trees(typed).expect("check wide partial affine source");
    let terminal = lower_machine(&checked, "Root::enter").expect("lower wide partial affine Psi");
    let semantics =
        encode_module(&terminal.semantic_module).expect("encode wide partial affine Psi");
    let proof =
        encode_proof_bundle(&terminal.proof_bundle).expect("encode wide partial affine proof");
    lower_artifact_sections(&semantics, &proof, &AdmissionProfile::default())
        .expect("verified wide partial affine artifact enters Omega")
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

fn two_empty_nominal_affine_plan()
-> omega_terminal_abstract_operations::TerminalAbstractOperationPlan {
    let tokens = Lexer::new(TWO_EMPTY_NOMINAL_AFFINE_SOURCE)
        .tokenize()
        .expect("tokenize two nominal affine source");
    let syntax = parse_syntax_trees(&tokens).expect("parse two nominal affine source");
    let resolved = lower_syntax_trees(&syntax).expect("resolve two nominal affine source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type two nominal affine source");
    let checked = lower_typed_trees(typed).expect("check two nominal affine source");
    let terminal = lower_machine(&checked, "Root::enter").expect("lower two nominal affine Psi");
    let semantics =
        encode_module(&terminal.semantic_module).expect("encode two nominal affine Psi");
    let proof = encode_proof_bundle(&terminal.proof_bundle).expect("encode two nominal proof");
    lower_artifact_sections(&semantics, &proof, &AdmissionProfile::default())
        .expect("verified two nominal artifact enters Omega")
}

fn two_nominal_one_executable_plan(
    source: &str,
) -> omega_terminal_abstract_operations::TerminalAbstractOperationPlan {
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize executable nominal-list source");
    let syntax = parse_syntax_trees(&tokens).expect("parse executable nominal-list source");
    let resolved = lower_syntax_trees(&syntax).expect("resolve executable nominal-list source");
    let typed =
        lower_symbol_resolved_trees(&resolved).expect("type executable nominal-list source");
    let checked = lower_typed_trees(typed).expect("check executable nominal-list source");
    let terminal =
        lower_machine(&checked, "Root::enter").expect("lower executable nominal-list Psi");
    let semantics =
        encode_module(&terminal.semantic_module).expect("encode executable nominal-list semantics");
    let proof =
        encode_proof_bundle(&terminal.proof_bundle).expect("encode executable nominal-list proof");
    lower_artifact_sections(&semantics, &proof, &AdmissionProfile::default())
        .expect("verified executable nominal-list artifact enters Omega")
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
                cleanup_actions,
                ..
            } => match cleanup_actions.as_slice() {
                [TerminalAffineCleanupAction::DiscardResidual(residual)] => Some(residual.clone()),
                _ => None,
            },
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
        assert_eq!(
            cleanup.actions,
            [TerminalAffineCleanupAction::DiscardResidual(
                residual.clone()
            )]
        );
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
            cleanup_actions,
            ..
        } = body.operations.last_mut().unwrap()
        else {
            panic!("caller ends in a Unit return")
        };
        *cleanup_actions = vec![TerminalAffineCleanupAction::DiscardRoot(residual.place)];
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
            .actions[0] =
            TerminalAffineCleanupAction::DiscardResidual(psi_terminal::StructuralAffineDiscard {
                path: vec![StructuralPathSegment::Field("right".into())],
                ..residual.clone()
            });
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
            .actions[0] =
            TerminalAffineCleanupAction::DiscardResidual(psi_terminal::StructuralAffineDiscard {
                structural_type: pair_type,
                ..residual.clone()
            });
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
        assert_eq!(
            installed_cleanup.actions,
            [TerminalAffineCleanupAction::DiscardResidual(
                residual.clone()
            )]
        );
        validate_terminal_installation_record(&installation, &image).unwrap();
        let bytes = encode_terminal_installation_record(&installation).unwrap();
        assert_eq!(
            decode_terminal_installation_record(&bytes),
            Ok(installation.clone())
        );
    }
}

#[test]
fn wide_partial_affine_cleanup_preserves_reverse_field_order_without_code() {
    let plan = wide_partial_affine_plan();
    let caller_machine = plan.entry;
    let caller = plan
        .functions
        .iter()
        .find(|function| function.machine == caller_machine)
        .expect("entry caller remains present");
    let cleanup_actions = caller
        .operations
        .iter()
        .find_map(|operation| match operation {
            omega_terminal_abstract_operations::TerminalAbstractOperation::ReturnUnit {
                cleanup_actions,
                ..
            } => Some(cleanup_actions.clone()),
            _ => None,
        })
        .expect("wide partial return retains cleanup actions");
    let [
        TerminalAffineCleanupAction::DiscardResidual(middle),
        TerminalAffineCleanupAction::DiscardResidual(left),
    ] = cleanup_actions.as_slice()
    else {
        panic!("wide partial return retains two residual fields")
    };
    assert_eq!(middle.path, [StructuralPathSegment::Field("middle".into())]);
    assert_eq!(left.path, [StructuralPathSegment::Field("left".into())]);
    assert_eq!(middle.place, left.place);
    assert_ne!(middle.structural_type, left.structural_type);

    let mut reordered = plan.clone();
    let reordered_caller = reordered
        .functions
        .iter_mut()
        .find(|function| function.machine == caller_machine)
        .unwrap();
    let omega_terminal_abstract_operations::TerminalAbstractOperation::ReturnUnit {
        cleanup_actions: reordered_actions,
        ..
    } = reordered_caller.operations.last_mut().unwrap()
    else {
        panic!("caller ends in a Unit return")
    };
    reordered_actions.swap(0, 1);
    assert!(lower_to_target_operations(&reordered, NativeTarget::linux_x64()).is_err());

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
            .unwrap();
        assert_eq!(
            emitted.unit_affine_cleanup.as_ref().unwrap().actions,
            cleanup_actions
        );
        let [call] = emitted.internal_unit_calls.as_slice() else {
            panic!("wide partial caller retains one projected call")
        };
        let [argument] = call.arguments.as_slice() else {
            panic!("wide partial call retains one projected argument")
        };
        assert_eq!(
            argument.path,
            [StructuralPathSegment::Field("right".into())]
        );
        assert_eq!(argument.source_byte_offset, 16);

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
            cleanup_actions: root_actions,
            ..
        } = body.operations.last_mut().unwrap()
        else {
            panic!("caller ends in a Unit return")
        };
        *root_actions = vec![TerminalAffineCleanupAction::DiscardRoot(middle.place)];
        let root_cleanup_machine = emit_machine_code(&root_cleanup_assigned).unwrap();
        let root_cleanup_bytes = &root_cleanup_machine
            .functions
            .iter()
            .find(|function| function.machine == caller_machine)
            .unwrap()
            .bytes;
        assert_eq!(
            &emitted.bytes, root_cleanup_bytes,
            "two residual field actions add no runtime instruction bytes"
        );

        let object = build_terminal_object_artifact(&machine).unwrap();
        let image = emit_terminal_executable_image(&object, 3).unwrap();
        let installation =
            build_terminal_installation_record(&image, ProfileDecisionId::new(1).unwrap()).unwrap();
        let installed_actions = &installation
            .functions()
            .iter()
            .find(|function| function.machine == caller_machine)
            .unwrap()
            .unit_affine_cleanup
            .as_ref()
            .unwrap()
            .actions;
        assert_eq!(installed_actions, &cleanup_actions);
        validate_terminal_installation_record(&installation, &image).unwrap();
        let bytes = encode_terminal_installation_record(&installation).unwrap();
        assert_eq!(
            decode_terminal_installation_record(&bytes),
            Ok(installation)
        );
    }
}

#[test]
fn partial_affine_cleanup_rejects_a_residual_before_its_local_cleanup() {
    let mut plan = partial_affine_plan();
    let empty_type = plan
        .structural_types
        .iter()
        .find(|declaration| {
            matches!(&declaration.shape, StructuralTypeShape::Record { fields } if fields.is_empty())
        })
        .cloned()
        .expect("partial-cleanup closure retains an empty record type");
    let local_place = PlaceId::new(10_000).unwrap();
    let local_operation = OperationId::new(10_000).unwrap();
    let entry = plan.entry;
    let return_index = {
        let caller = plan
            .functions
            .iter_mut()
            .find(|function| function.machine == entry)
            .expect("entry caller remains present");
        let return_index = caller
            .operations
            .iter()
            .position(|operation| {
                matches!(
                    operation,
                    omega_terminal_abstract_operations::TerminalAbstractOperation::ReturnUnit { .. }
                )
            })
            .expect("partial-cleanup caller returns Unit");
        caller.operations.insert(
            return_index,
            omega_terminal_abstract_operations::TerminalAbstractOperation::EstablishTrivialAffineLocal {
                psi_operation: local_operation,
                place: StructuralPlaceDeclaration {
                    id: local_place,
                    kind: StructuralPlaceKind::TrivialAffineLocal {
                        declaration_ordinal: 0,
                        structural_type: empty_type.id,
                    },
                },
                structural_type: empty_type,
            },
        );
        let omega_terminal_abstract_operations::TerminalAbstractOperation::ReturnUnit {
            cleanup_actions,
            ..
        } = &mut caller.operations[return_index + 1]
        else {
            unreachable!("located Unit return remains at the next operation")
        };
        let [residual] = cleanup_actions.as_slice() else {
            panic!("partial-cleanup return retains one residual action")
        };
        let residual = residual.clone();
        *cleanup_actions = vec![
            TerminalAffineCleanupAction::DiscardRoot(local_place),
            residual,
        ];
        return_index
    };

    lower_to_target_operations(&plan, NativeTarget::linux_x64())
        .expect("reverse-local cleanup followed by the residual is canonical");
    let caller = plan
        .functions
        .iter_mut()
        .find(|function| function.machine == entry)
        .expect("entry caller remains present");
    let omega_terminal_abstract_operations::TerminalAbstractOperation::ReturnUnit {
        cleanup_actions,
        ..
    } = &mut caller.operations[return_index + 1]
    else {
        unreachable!("located Unit return remains at the next operation")
    };
    cleanup_actions.swap(0, 1);
    assert!(lower_to_target_operations(&plan, NativeTarget::linux_x64()).is_err());
}

#[test]
fn two_empty_nominal_cleanups_are_reverse_ordered_and_call_free_on_all_targets() {
    let plan = two_empty_nominal_affine_plan();
    let caller_machine = plan.entry;
    let caller = plan
        .functions
        .iter()
        .find(|function| function.machine == caller_machine)
        .expect("entry caller remains present");
    let parameter_places = caller
        .structural_parameters
        .iter()
        .map(|parameter| parameter.place)
        .collect::<Vec<_>>();
    assert_eq!(parameter_places.len(), 2);
    let cleanup_actions = caller
        .operations
        .iter()
        .find_map(|operation| match operation {
            omega_terminal_abstract_operations::TerminalAbstractOperation::ReturnUnit {
                cleanup_actions,
                ..
            } => Some(cleanup_actions.clone()),
            _ => None,
        })
        .expect("entry return retains cleanup actions");
    let [
        TerminalAffineCleanupAction::InvokeNominal(first),
        TerminalAffineCleanupAction::InvokeNominal(second),
    ] = cleanup_actions.as_slice()
    else {
        panic!("entry return must invoke exactly two nominal cleanups")
    };
    assert_eq!(
        [first.place, second.place],
        [parameter_places[1], parameter_places[0]]
    );
    assert_eq!(first.cleanup_machine, second.cleanup_machine);

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
            .unwrap();
        let emitted_cleanup = emitted.unit_affine_cleanup.as_ref().unwrap();
        assert_eq!(emitted_cleanup.actions, cleanup_actions);
        assert!(
            emitted.internal_unit_calls.is_empty(),
            "two empty cleanups emit no calls for {target:?}"
        );

        let mut swapped = machine.clone();
        swapped
            .functions
            .iter_mut()
            .find(|function| function.machine == caller_machine)
            .unwrap()
            .unit_affine_cleanup
            .as_mut()
            .unwrap()
            .actions
            .swap(0, 1);
        assert!(build_terminal_object_artifact(&swapped).is_err());

        let object = build_terminal_object_artifact(&machine).unwrap();
        let image = emit_terminal_executable_image(&object, 3).unwrap();
        let installation =
            build_terminal_installation_record(&image, ProfileDecisionId::new(1).unwrap()).unwrap();
        let installed = installation
            .functions()
            .iter()
            .find(|function| function.machine == caller_machine)
            .unwrap();
        assert_eq!(
            installed.unit_affine_cleanup.as_ref().unwrap().actions,
            cleanup_actions
        );
        assert!(installation.internal_unit_calls().is_empty());
        validate_terminal_installation_record(&installation, &image).unwrap();
        let bytes = encode_terminal_installation_record(&installation).unwrap();
        assert_eq!(
            decode_terminal_installation_record(&bytes),
            Ok(installation)
        );
    }
}

#[test]
fn one_executable_nominal_cleanup_action_retains_its_exact_ordinal_on_all_targets() {
    for (source, executable_action_ordinal) in [
        (SECOND_EXECUTABLE_NOMINAL_AFFINE_SOURCE, 0_u32),
        (FIRST_EXECUTABLE_NOMINAL_AFFINE_SOURCE, 1_u32),
    ] {
        let plan = two_nominal_one_executable_plan(source);
        let caller_machine = plan.entry;
        let caller = plan
            .functions
            .iter()
            .find(|function| function.machine == caller_machine)
            .expect("entry caller remains present");
        let cleanup_actions = caller
            .operations
            .iter()
            .find_map(|operation| match operation {
                omega_terminal_abstract_operations::TerminalAbstractOperation::ReturnUnit {
                    cleanup_actions,
                    ..
                } => Some(cleanup_actions.clone()),
                _ => None,
            })
            .expect("entry return retains cleanup actions");
        assert_eq!(cleanup_actions.len(), 2);
        let TerminalAffineCleanupAction::InvokeNominal(executable_cleanup) =
            cleanup_actions[usize::try_from(executable_action_ordinal).unwrap()]
        else {
            unreachable!("both ordered actions remain nominal")
        };

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
                .unwrap();
            let emitted_cleanup = emitted.unit_affine_cleanup.as_ref().unwrap();
            assert_eq!(emitted_cleanup.actions, cleanup_actions);
            let [cleanup_call] = emitted.internal_unit_calls.as_slice() else {
                panic!("exactly one ordered cleanup action emits a call for {target:?}")
            };
            let expected_owner = TerminalCallSiteOwner::CleanupAction {
                edge: emitted_cleanup.psi_edge,
                action_ordinal: executable_action_ordinal,
            };
            assert_eq!(cleanup_call.owner, expected_owner);
            assert_eq!(cleanup_call.target, executable_cleanup.cleanup_machine);
            assert!(emitted.internal_calls.iter().any(|call| {
                call.owner == expected_owner && call.target == executable_cleanup.cleanup_machine
            }));

            let mut forged = machine.clone();
            let forged_caller = forged
                .functions
                .iter_mut()
                .find(|function| function.machine == caller_machine)
                .unwrap();
            let forged_owner = TerminalCallSiteOwner::CleanupAction {
                edge: emitted_cleanup.psi_edge,
                action_ordinal: 1 - executable_action_ordinal,
            };
            forged_caller.internal_calls[0].owner = forged_owner;
            forged_caller.internal_unit_calls[0].owner = forged_owner;
            assert!(build_terminal_object_artifact(&forged).is_err());

            let object = build_terminal_object_artifact(&machine).unwrap();
            let image = emit_terminal_executable_image(&object, 3).unwrap();
            let installation =
                build_terminal_installation_record(&image, ProfileDecisionId::new(1).unwrap())
                    .unwrap();
            let installed_call = installation
                .internal_unit_calls()
                .iter()
                .find(|call| call.machine == caller_machine)
                .expect("installed caller cleanup call");
            assert_eq!(installed_call.custody.owner, expected_owner);
            validate_terminal_installation_record(&installation, &image).unwrap();
            let bytes = encode_terminal_installation_record(&installation).unwrap();
            let mut owner_encoding = vec![2, 0, 0, 0];
            owner_encoding.extend_from_slice(&emitted_cleanup.psi_edge.get().to_le_bytes());
            owner_encoding.extend_from_slice(&executable_action_ordinal.to_le_bytes());
            owner_encoding.extend_from_slice(&0_u32.to_le_bytes());
            let owner_offset = bytes
                .windows(owner_encoding.len())
                .position(|window| window == owner_encoding)
                .expect("installation encodes the exact cleanup-action owner");
            let mut forged_ordinal = bytes.clone();
            let ordinal_offset = owner_offset + 12;
            forged_ordinal[ordinal_offset..ordinal_offset + 4]
                .copy_from_slice(&(1 - executable_action_ordinal).to_le_bytes());
            assert!(decode_terminal_installation_record(&forged_ordinal).is_err());
            assert_eq!(
                decode_terminal_installation_record(&bytes),
                Ok(installation)
            );
        }
    }
}

#[test]
fn two_executable_nominal_cleanup_actions_retain_order_and_custody_on_all_targets() {
    for (source, shared_target) in [
        (TWO_EXECUTABLE_NOMINAL_AFFINE_SOURCE, false),
        (SHARED_EXECUTABLE_NOMINAL_AFFINE_SOURCE, true),
    ] {
        let plan = two_nominal_one_executable_plan(source);
        let caller_machine = plan.entry;
        let caller = plan
            .functions
            .iter()
            .find(|function| function.machine == caller_machine)
            .expect("entry caller remains present");
        let cleanup_actions = caller
            .operations
            .iter()
            .find_map(|operation| match operation {
                omega_terminal_abstract_operations::TerminalAbstractOperation::ReturnUnit {
                    cleanup_actions,
                    ..
                } => Some(cleanup_actions.clone()),
                _ => None,
            })
            .expect("entry return retains cleanup actions");
        let [
            TerminalAffineCleanupAction::InvokeNominal(first),
            TerminalAffineCleanupAction::InvokeNominal(second),
        ] = cleanup_actions.as_slice()
        else {
            panic!("entry return invokes two nominal cleanups")
        };
        assert_eq!(first.place, caller.structural_parameters[1].place);
        assert_eq!(second.place, caller.structural_parameters[0].place);
        assert_eq!(
            first.cleanup_machine == second.cleanup_machine,
            shared_target
        );
        assert_eq!(plan.functions.len(), if shared_target { 3 } else { 4 });

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
                .unwrap();
            let emitted_cleanup = emitted.unit_affine_cleanup.as_ref().unwrap();
            assert_eq!(emitted_cleanup.actions, cleanup_actions);
            assert_eq!(emitted.internal_unit_calls.len(), 2);
            assert_eq!(emitted.internal_calls.len(), 2);
            for (ordinal, (call, cleanup)) in emitted
                .internal_unit_calls
                .iter()
                .zip([first, second])
                .enumerate()
            {
                let expected_owner = TerminalCallSiteOwner::CleanupAction {
                    edge: emitted_cleanup.psi_edge,
                    action_ordinal: u32::try_from(ordinal).unwrap(),
                };
                assert_eq!(call.owner, expected_owner);
                assert_eq!(call.target, cleanup.cleanup_machine);
                assert!(call.arguments.is_empty());
                assert!(call.claim_transfers.is_empty());
                assert_eq!(emitted.internal_calls[ordinal].owner, expected_owner);
                assert_eq!(
                    emitted.internal_calls[ordinal].target,
                    cleanup.cleanup_machine
                );
            }
            assert!(
                emitted.internal_unit_calls[0].code_offset
                    + emitted.internal_unit_calls[0].byte_count
                    <= emitted.internal_unit_calls[1].code_offset
            );

            let mut swapped_owners = machine.clone();
            let forged_caller = swapped_owners
                .functions
                .iter_mut()
                .find(|function| function.machine == caller_machine)
                .unwrap();
            forged_caller.internal_calls[0].owner = TerminalCallSiteOwner::CleanupAction {
                edge: emitted_cleanup.psi_edge,
                action_ordinal: 1,
            };
            forged_caller.internal_calls[1].owner = TerminalCallSiteOwner::CleanupAction {
                edge: emitted_cleanup.psi_edge,
                action_ordinal: 0,
            };
            forged_caller.internal_unit_calls[0].owner = TerminalCallSiteOwner::CleanupAction {
                edge: emitted_cleanup.psi_edge,
                action_ordinal: 1,
            };
            forged_caller.internal_unit_calls[1].owner = TerminalCallSiteOwner::CleanupAction {
                edge: emitted_cleanup.psi_edge,
                action_ordinal: 0,
            };
            assert!(build_terminal_object_artifact(&swapped_owners).is_err());

            let object = build_terminal_object_artifact(&machine).unwrap();
            let image = emit_terminal_executable_image(&object, 3).unwrap();
            let installation =
                build_terminal_installation_record(&image, ProfileDecisionId::new(1).unwrap())
                    .unwrap();
            let installed_calls = installation
                .internal_unit_calls()
                .iter()
                .filter(|call| call.machine == caller_machine)
                .collect::<Vec<_>>();
            assert_eq!(installed_calls.len(), 2);
            for (ordinal, (call, cleanup)) in
                installed_calls.iter().zip([first, second]).enumerate()
            {
                assert_eq!(
                    call.custody.owner,
                    TerminalCallSiteOwner::CleanupAction {
                        edge: emitted_cleanup.psi_edge,
                        action_ordinal: u32::try_from(ordinal).unwrap(),
                    }
                );
                assert_eq!(call.custody.target, cleanup.cleanup_machine);
            }
            assert_eq!(
                installation.internal_unit_calls().len(),
                if shared_target { 3 } else { 4 }
            );
            validate_terminal_installation_record(&installation, &image).unwrap();
            let bytes = encode_terminal_installation_record(&installation).unwrap();
            assert_eq!(
                decode_terminal_installation_record(&bytes),
                Ok(installation)
            );
        }
    }
}

#[test]
fn three_shared_executable_cleanup_actions_retain_exact_order_on_all_targets() {
    let plan = two_nominal_one_executable_plan(THREE_SHARED_EXECUTABLE_NOMINAL_AFFINE_SOURCE);
    let caller_machine = plan.entry;
    let caller = plan
        .functions
        .iter()
        .find(|function| function.machine == caller_machine)
        .unwrap();
    let cleanup_actions = caller
        .operations
        .iter()
        .find_map(|operation| match operation {
            omega_terminal_abstract_operations::TerminalAbstractOperation::ReturnUnit {
                cleanup_actions,
                ..
            } => Some(cleanup_actions.clone()),
            _ => None,
        })
        .unwrap();
    let cleanup_targets = cleanup_actions
        .iter()
        .map(|action| match action {
            TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
                (cleanup.place, cleanup.cleanup_machine)
            }
            _ => panic!("all three actions remain nominal"),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        cleanup_targets
            .iter()
            .map(|(place, _)| *place)
            .collect::<Vec<_>>(),
        caller
            .structural_parameters
            .iter()
            .rev()
            .map(|parameter| parameter.place)
            .collect::<Vec<_>>()
    );
    assert!(
        cleanup_targets
            .windows(2)
            .all(|pair| pair[0].1 == pair[1].1)
    );
    assert_eq!(plan.functions.len(), 3);

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
            .unwrap();
        assert_eq!(
            emitted.unit_affine_cleanup.as_ref().unwrap().actions,
            cleanup_actions
        );
        assert_eq!(emitted.internal_unit_calls.len(), 3);
        for (ordinal, call) in emitted.internal_unit_calls.iter().enumerate() {
            assert_eq!(
                call.owner,
                TerminalCallSiteOwner::CleanupAction {
                    edge: emitted.unit_affine_cleanup.as_ref().unwrap().psi_edge,
                    action_ordinal: u32::try_from(ordinal).unwrap(),
                }
            );
            assert_eq!(call.target, cleanup_targets[ordinal].1);
        }
        assert!(
            emitted
                .internal_unit_calls
                .windows(2)
                .all(|pair| { pair[0].code_offset + pair[0].byte_count <= pair[1].code_offset })
        );

        let object = build_terminal_object_artifact(&machine).unwrap();
        let image = emit_terminal_executable_image(&object, 3).unwrap();
        let installation =
            build_terminal_installation_record(&image, ProfileDecisionId::new(1).unwrap()).unwrap();
        assert_eq!(installation.internal_unit_calls().len(), 4);
        validate_terminal_installation_record(&installation, &image).unwrap();
        let bytes = encode_terminal_installation_record(&installation).unwrap();
        assert_eq!(
            decode_terminal_installation_record(&bytes),
            Ok(installation)
        );
    }
}

#[test]
fn finite_cleanup_lists_and_helper_bodies_retain_exact_order_on_all_targets() {
    let plan = two_nominal_one_executable_plan(FIVE_SHARED_EXECUTABLE_NOMINAL_AFFINE_SOURCE);
    let caller_machine = plan.entry;
    let caller = plan
        .functions
        .iter()
        .find(|function| function.machine == caller_machine)
        .unwrap();
    let cleanup_actions = caller
        .operations
        .iter()
        .find_map(|operation| match operation {
            omega_terminal_abstract_operations::TerminalAbstractOperation::ReturnUnit {
                cleanup_actions,
                ..
            } => Some(cleanup_actions.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(cleanup_actions.len(), 5);
    let TerminalAffineCleanupAction::InvokeNominal(first_cleanup) = cleanup_actions[0] else {
        unreachable!()
    };
    let cleanup_function = plan
        .functions
        .iter()
        .find(|function| function.machine == first_cleanup.cleanup_machine)
        .unwrap();
    assert_eq!(cleanup_function.operations.len(), 6);
    assert_eq!(plan.functions.len(), 7);

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
            .unwrap();
        assert_eq!(emitted.internal_unit_calls.len(), 5);
        for (ordinal, call) in emitted.internal_unit_calls.iter().enumerate() {
            assert_eq!(
                call.owner,
                TerminalCallSiteOwner::CleanupAction {
                    edge: emitted.unit_affine_cleanup.as_ref().unwrap().psi_edge,
                    action_ordinal: u32::try_from(ordinal).unwrap(),
                }
            );
            assert_eq!(call.target, first_cleanup.cleanup_machine);
        }
        let drop = machine
            .functions
            .iter()
            .find(|function| function.machine == first_cleanup.cleanup_machine)
            .unwrap();
        assert_eq!(drop.internal_unit_calls.len(), 5);
        assert!(
            drop.internal_unit_calls
                .iter()
                .enumerate()
                .all(|(ordinal, call)| {
                    call.operation_ordinal == ordinal
                        && matches!(call.owner, TerminalCallSiteOwner::Operation(_))
                })
        );

        let object = build_terminal_object_artifact(&machine).unwrap();
        let image = emit_terminal_executable_image(&object, 3).unwrap();
        let installation =
            build_terminal_installation_record(&image, ProfileDecisionId::new(1).unwrap()).unwrap();
        assert_eq!(installation.internal_unit_calls().len(), 10);
        validate_terminal_installation_record(&installation, &image).unwrap();
        let bytes = encode_terminal_installation_record(&installation).unwrap();
        assert_eq!(
            decode_terminal_installation_record(&bytes),
            Ok(installation)
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
                cleanup_actions,
                ..
            } => match cleanup_actions.as_slice() {
                [TerminalAffineCleanupAction::InvokeNominal(cleanup)] => Some(*cleanup),
                _ => None,
            },
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
    let helper_calls = cleanup_function
        .operations
        .iter()
        .filter_map(|operation| match operation {
            omega_terminal_abstract_operations::TerminalAbstractOperation::CallUnit {
                psi_operation,
                callee,
                structural_arguments,
                claim_transfers,
            } => {
                assert!(structural_arguments.is_empty());
                assert!(claim_transfers.is_empty());
                Some((*psi_operation, *callee))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(helper_calls.len(), 3);
    assert_ne!(helper_calls[0].0, helper_calls[1].0);
    assert_ne!(helper_calls[0].1, helper_calls[1].1);
    assert_ne!(helper_calls[1].0, helper_calls[2].0);
    assert_ne!(helper_calls[1].1, helper_calls[2].1);

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
            cleanup_actions,
            ..
        } = target_body.operations.last().unwrap()
        else {
            panic!("caller ends in a Unit return")
        };
        assert_eq!(
            cleanup_actions,
            &[TerminalAffineCleanupAction::InvokeNominal(cleanup)]
        );

        let assigned = assign_registers(&target_plan).unwrap();
        let machine = emit_machine_code(&assigned).unwrap();
        let emitted = machine
            .functions
            .iter()
            .find(|function| function.machine == caller_machine)
            .unwrap();
        let emitted_cleanup = emitted.unit_affine_cleanup.as_ref().unwrap();
        assert!(emitted_cleanup.locals.is_empty());
        assert_eq!(
            emitted_cleanup.actions,
            [TerminalAffineCleanupAction::InvokeNominal(cleanup)]
        );
        let cleanup_call = emitted
            .internal_unit_calls
            .iter()
            .find(|call| {
                call.owner
                    == TerminalCallSiteOwner::CleanupAction {
                        edge: emitted_cleanup.psi_edge,
                        action_ordinal: 0,
                    }
            })
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
        let emitted_drop = machine
            .functions
            .iter()
            .find(|function| function.machine == cleanup.cleanup_machine)
            .unwrap();
        assert_eq!(
            emitted_drop
                .internal_unit_calls
                .iter()
                .map(|call| (call.owner, call.target))
                .collect::<Vec<_>>(),
            helper_calls
                .iter()
                .map(|(operation, target)| {
                    (TerminalCallSiteOwner::Operation(*operation), *target)
                })
                .collect::<Vec<_>>(),
            "drop helper calls retain source order"
        );
        for (ordinal, call) in emitted_drop.internal_unit_calls.iter().enumerate() {
            assert_eq!(call.operation_ordinal, ordinal);
        }
        assert!(
            emitted_drop
                .internal_unit_calls
                .windows(2)
                .all(|pair| { pair[0].code_offset + pair[0].byte_count <= pair[1].code_offset })
        );

        let mut forged_helper_order = machine.clone();
        let forged_drop = forged_helper_order
            .functions
            .iter_mut()
            .find(|function| function.machine == cleanup.cleanup_machine)
            .unwrap();
        let first_owner = forged_drop.internal_calls[0].owner;
        forged_drop.internal_calls[0].owner = forged_drop.internal_calls[2].owner;
        forged_drop.internal_calls[2].owner = first_owner;
        let first_owner = forged_drop.internal_unit_calls[0].owner;
        forged_drop.internal_unit_calls[0].owner = forged_drop.internal_unit_calls[2].owner;
        forged_drop.internal_unit_calls[2].owner = first_owner;
        assert!(build_terminal_object_artifact(&forged_helper_order).is_err());

        let mut forged_place = machine.clone();
        forged_place
            .functions
            .iter_mut()
            .find(|function| function.machine == caller_machine)
            .unwrap()
            .unit_affine_cleanup
            .as_mut()
            .unwrap()
            .actions[0] =
            TerminalAffineCleanupAction::InvokeNominal(psi_terminal::NominalAffineCleanup {
                place: psi_core::PlaceId::new(cleanup.place.get() + 1).unwrap(),
                ..cleanup
            });
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
        assert_eq!(installation.internal_unit_calls().len(), 4);
        let installed = installation
            .functions()
            .iter()
            .find(|function| function.machine == caller_machine)
            .unwrap();
        assert_eq!(
            installed.unit_affine_cleanup.as_ref().unwrap().actions,
            [TerminalAffineCleanupAction::InvokeNominal(cleanup)]
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
            let mut encoded_cleanup = vec![3, 0, 0, 0];
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
            invalid_presence[offset] = 4;
            assert_eq!(
                decode_terminal_installation_record(&invalid_presence),
                Err(TerminalInstallationError::InvalidCleanupActionTag(4))
            );
            let mut zero_place = bytes.clone();
            zero_place[offset + 4..offset + 12].fill(0);
            assert_eq!(
                decode_terminal_installation_record(&zero_place),
                Err(TerminalInstallationError::ZeroStructuralReturnIdentity(
                    "nominal Unit cleanup place"
                ))
            );
        }
    }
}
