use crate::assignment::shared::{MachineRegister, ValueLocation, ValueShape};
use crate::{AssignmentError, assign_registers};
use assigned_target_operations::AssignedOperation;
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use target::NativeTarget;
use target_operations::TargetOperation;
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees_to_checked_trees::lower_typed_trees;

const RANKED_COUNTDOWN_SOURCE: &str = r#"
    data Token { value: i32; }
    data Root {}

    machine Root::countdown(token: Token, remaining: u32)
    terminates by remaining -> Nat::Descending;
    {
        transition remaining > 0 {
            true -> countdown(token, remaining - 1)
            _ -> done(token)
        }
        state done(token: Token) {}
    }
"#;

const RANKED_RECEIVER_COUNTDOWN_SOURCE: &str = r#"
    data Token { value: i32; }
    data Root { token: Token; }

    machine Root::countdown(&mut self, remaining: u32)
    terminates by remaining -> Nat::Descending;
    {
        transition remaining > 0 {
            true -> countdown(remaining - 1)
            _ -> done()
        }
        state done(&mut self) {}
    }
"#;

fn ranked_target(target: NativeTarget) -> target_operations::TargetOperationPlan {
    ranked_target_from(RANKED_COUNTDOWN_SOURCE, target)
}

fn ranked_target_from(
    source: &str,
    target: NativeTarget,
) -> target_operations::TargetOperationPlan {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::countdown")
        .expect("lower terminal countdown");
    let semantic = terminal_codec::encode_module(&lowered.semantic_module).expect("semantic");
    let proof = terminal_codec::encode_proof_bundle(&lowered.proof_bundle).expect("proof");
    let ranked =
        terminal_psi_to_abstract_operations::lower_artifact_sections_for_native_ranked_countdown(
            &semantic,
            &proof,
            &proof_admission::AdmissionProfile::default(),
        )
        .expect("admit native ranked countdown");
    abstract_operations_to_target_operations::lower_ranked_to_target_operations(&ranked, target)
        .expect("lower ranked target")
}

#[test]
fn ranked_receiver_assignment_replays_semantic_identity_and_pointer_placement() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_plan = ranked_target_from(RANKED_RECEIVER_COUNTDOWN_SOURCE, target);
        let assigned = assign_registers(&target_plan).expect("assign persistent receiver");
        let AssignedOperation::RankedU32Countdown(countdown) = &assigned.functions[0].operation
        else {
            panic!("assigned ranked carrier")
        };
        let [replay] = countdown.custody.semantic_replay.machines[0]
            .structural_parameters
            .as_slice()
        else {
            panic!("one replay receiver")
        };
        let [physical] = countdown.structural_parameters.as_slice() else {
            panic!("one physical receiver")
        };
        assert!(replay.is_self);
        assert_eq!(physical.place, replay.place);
        assert_eq!(physical.structural_type, replay.structural_type);
        assert_eq!(physical.multiplicity, replay.multiplicity);
        assert_eq!(physical.access, replay.access);
        assert_eq!(physical.shape, ValueShape::borrowed_reference(4, 4));
        assert!(countdown.cleanup_actions.is_empty());

        let mut forged = target_plan.clone();
        let TargetOperation::RankedU32Countdown(candidate) = &mut forged.functions[0].operation
        else {
            unreachable!()
        };
        candidate.structural_parameters[0].access = terminal_psi::StructuralAccess::Owned;
        let source = candidate.custody.graph.initial_value;
        assert_eq!(
            assign_registers(&forged),
            Err(AssignmentError::RankedCountdownAbiMismatch(source))
        );
    }
}

#[test]
fn ranked_receiver_assignment_rejects_coherent_shape_and_placement_substitution() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let target_plan = ranked_target_from(RANKED_RECEIVER_COUNTDOWN_SOURCE, target);
        for shape in [
            ValueShape::integer(8, 8),
            ValueShape::integer(4, 4),
            ValueShape::borrowed_reference(8, 8),
            ValueShape::borrowed_reference(4, 2),
        ] {
            let mut forged = target_plan.clone();
            let TargetOperation::RankedU32Countdown(candidate) = &mut forged.functions[0].operation
            else {
                panic!("ranked receiver")
            };
            candidate.call_plan = calling_conventions::evaluate_call_plan(
                calling_conventions::CallingPolicy::native_for_target(target),
                &calling_conventions::CallSignature {
                    parameters: vec![ValueShape::integer(4, 4), shape],
                    result: None,
                },
            )
            .unwrap();
            candidate.structural_parameters[0].shape = shape;
            candidate.structural_parameters[0].placement =
                candidate.call_plan.parameters[1].clone();
            let source = candidate.custody.graph.initial_value;
            assert_eq!(
                assign_registers(&forged),
                Err(AssignmentError::RankedCountdownAbiMismatch(source)),
                "the supplied ABI cannot authorize {shape:?} for the retained receiver"
            );
        }
        let mut forged = target_plan;
        let TargetOperation::RankedU32Countdown(candidate) = &mut forged.functions[0].operation
        else {
            panic!("ranked receiver")
        };
        let source = candidate.custody.graph.initial_value;
        let calling_conventions::ValueLocation::Indirect { pointer, .. } =
            &mut candidate.call_plan.parameters[1].locations[0]
        else {
            panic!("reference ABI")
        };
        *pointer = calling_conventions::IndirectPointerLocation::Register(
            if target == NativeTarget::linux_x64() {
                MachineRegister::X86Rdx
            } else {
                MachineRegister::Aarch64X(2)
            },
        );
        candidate.structural_parameters[0].placement = candidate.call_plan.parameters[1].clone();
        assert_eq!(
            assign_registers(&forged),
            Err(AssignmentError::RankedCountdownAbiMismatch(source))
        );
    }
}

#[test]
fn ranked_countdown_assignment_preserves_custody_and_uses_the_initial_register() {
    for (target, expected_register) in [
        (NativeTarget::linux_x64(), MachineRegister::X86Rdi),
        (NativeTarget::linux_arm64(), MachineRegister::Aarch64X(0)),
    ] {
        let target_plan = ranked_target(target);
        let TargetOperation::RankedU32Countdown(target_countdown) =
            &target_plan.functions[0].operation
        else {
            panic!("target ranked carrier")
        };
        let assigned = assign_registers(&target_plan).expect("assign ranked countdown");
        assert_eq!(
            assigned.functions[0].provenance,
            target_plan.functions[0].provenance
        );
        let AssignedOperation::RankedU32Countdown(countdown) = &assigned.functions[0].operation
        else {
            panic!("assigned ranked carrier")
        };
        assert_eq!(countdown.custody, target_countdown.custody);
        assert_eq!(countdown.call_plan, target_countdown.call_plan);
        assert_eq!(
            countdown.structural_types,
            target_countdown.structural_types
        );
        assert_eq!(
            countdown.structural_parameters,
            target_countdown.structural_parameters
        );
        assert_eq!(countdown.cleanup_actions, target_countdown.cleanup_actions);
        assert_eq!(countdown.rank_home, expected_register);
    }
}

#[test]
fn ranked_countdown_assignment_rejects_stack_and_wrong_architecture_rank_homes() {
    let mut stacked = ranked_target(NativeTarget::linux_x64());
    let TargetOperation::RankedU32Countdown(countdown) = &mut stacked.functions[0].operation else {
        unreachable!()
    };
    let source = countdown.custody.graph.initial_value;
    countdown.call_plan.parameters[0].locations = vec![ValueLocation::Stack {
        stack_byte_offset: 0,
        value_byte_offset: 0,
        byte_size: 4,
        alignment: 4,
    }];
    assert_eq!(
        assign_registers(&stacked),
        Err(AssignmentError::RankedCountdownRequiresRegister(source))
    );

    let mut wrong_arch = ranked_target(NativeTarget::linux_x64());
    let TargetOperation::RankedU32Countdown(countdown) = &mut wrong_arch.functions[0].operation
    else {
        unreachable!()
    };
    countdown.call_plan.parameters[0].locations = vec![ValueLocation::Register {
        register: MachineRegister::Aarch64X(0),
        value_byte_offset: 0,
        byte_size: 4,
    }];
    assert_eq!(
        assign_registers(&wrong_arch),
        Err(AssignmentError::ParameterRegisterArchitectureMismatch {
            value: source,
            register: MachineRegister::Aarch64X(0),
            architecture: target::Architecture::X86_64,
        })
    );

    let mut wrong_same_arch = ranked_target(NativeTarget::linux_x64());
    let TargetOperation::RankedU32Countdown(countdown) =
        &mut wrong_same_arch.functions[0].operation
    else {
        unreachable!()
    };
    countdown.call_plan.parameters[0].locations = vec![ValueLocation::Register {
        register: MachineRegister::X86Rsi,
        value_byte_offset: 0,
        byte_size: 4,
    }];
    assert_eq!(
        assign_registers(&wrong_same_arch),
        Err(AssignmentError::RankedCountdownAbiMismatch(source))
    );
}
