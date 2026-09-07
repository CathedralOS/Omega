//! Generalized continuation segments do not widen sole-return root disposal.

use super::*;

#[test]
fn sole_projected_result_return_cannot_omit_an_unrelated_affine_parameter() {
    let source = "data Token { value: u64; }
        data Pair { left: Token; right: Token; }
        data Root {} data Sink {}
        machine Root::forward(value: Pair) -> Pair { value }
        machine Sink::take(value: Token) {}
        machine Root::enter(value: Pair) {
            Sink::take(Root::forward(value).right);
        }";
    let mut plan = source_plan(source);
    let target = NativeTarget::linux_x64();
    assert!(lower_to_target_operations(&plan, target).is_ok());
    let entry = plan
        .functions
        .iter_mut()
        .find(|function| function.machine == plan.entry)
        .unwrap();
    let mut unrelated = entry.structural_parameters[0].clone();
    unrelated.place = PlaceId::new(u64::from(u32::MAX) - 1).unwrap();
    unrelated.position = 1;
    entry.structural_parameters.push(unrelated);
    assert!(matches!(
        lower_to_target_operations(&plan, target),
        Err(LoweringError::UnsupportedOperationInUnitFunction(_))
    ));
}

#[test]
fn continuation_scalar_aliases_retain_original_sources_and_exact_bindings() {
    let source = "data Token { value: u64; } data Pair { left: Token; right: Token; }
        data Root {} data Sink {}
        machine Root::forward(value: Pair) -> Pair { value }
        machine Sink::take(value: Token) {}
        machine Sink::number(value: u16) {}
        machine Root::enter(first: u16, second: u16, value: Pair) {
            Sink::take(Root::forward(value).right);
            Sink::number(second); Sink::number(first);
        }";
    let plan = source_plan(source);
    let native = NativeTarget::linux_x64();
    let target = lower_to_target_operations(&plan, native).unwrap();
    validate_abstract_to_target_translation(&plan, native, &target).unwrap();
    let caller = plan
        .functions
        .iter()
        .find(|function| function.machine == plan.entry)
        .unwrap();
    let target_caller = target
        .functions
        .iter()
        .find(|function| function.machine == plan.entry)
        .unwrap();
    let TargetOperation::UnitBody(body) = &target_caller.operation else {
        panic!("Unit caller");
    };
    let observed = body
        .operations
        .iter()
        .filter_map(|operation| match operation {
            TargetUnitOperation::Call {
                scalar_arguments, ..
            } if !scalar_arguments.is_empty() => Some(scalar_arguments[0].source.source_value()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        observed,
        vec![caller.parameters[1].value, caller.parameters[0].value]
    );
    for mutation in 0..4 {
        let mut changed = target.clone();
        let target_caller = changed
            .functions
            .iter_mut()
            .find(|function| function.machine == plan.entry)
            .unwrap();
        let TargetOperation::UnitBody(body) = &mut target_caller.operation else {
            panic!("Unit caller");
        };
        let bindings = body
            .operations
            .iter_mut()
            .find_map(|operation| match operation {
                TargetUnitOperation::Continue { bindings, .. } => Some(bindings),
                _ => None,
            })
            .unwrap();
        assert_eq!(bindings.len(), 2);
        match mutation {
            0 => {
                bindings.pop();
            }
            1 => bindings[0].argument = ValueId::new(99999).unwrap(),
            2 => bindings[0].scalar_type = ScalarType::Boolean,
            3 => {
                let first = bindings[0].argument;
                bindings[0].argument = bindings[1].argument;
                bindings[1].argument = first;
            }
            _ => unreachable!(),
        }
        assert!(validate_abstract_to_target_translation(&plan, native, &changed).is_err());
    }
}

fn source_plan(source: &str) -> AbstractOperationPlan {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed).unwrap();
    let terminal = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter").unwrap();
    let semantic = terminal_codec::encode_module(&terminal.semantic_module).unwrap();
    let proof = terminal_codec::encode_proof_bundle(&terminal.proof_bundle).unwrap();
    terminal_psi_to_abstract_operations::lower_artifact_sections(
        &semantic,
        &proof,
        &proof_admission::AdmissionProfile::default(),
    )
    .unwrap()
}

#[test]
fn scalar_only_unit_leaf_retains_its_real_incoming_abi() {
    let plan = source_plan("data Root {} machine Root::enter(first: u16, second: u64) {}");
    let caller = plan
        .functions
        .iter()
        .find(|function| function.machine == plan.entry)
        .unwrap();
    assert_eq!(caller.parameters.len(), 2);
    assert!(caller.block_entries[0].parameters.is_empty());
    for native in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let target = lower_to_target_operations(&plan, native).unwrap();
        let function = target
            .functions
            .iter()
            .find(|function| function.machine == plan.entry)
            .unwrap();
        let TargetOperation::UnitBody(body) = &function.operation else {
            panic!("actual Unit body");
        };
        assert_eq!(body.scalar_parameters.len(), 2);
        assert_eq!(body.call_plan.parameters.len(), 2);
        assert!(body.call_plan.result.is_none());
        for (position, scalar) in body.scalar_parameters.iter().enumerate() {
            assert_eq!(scalar.value, caller.parameters[position].value);
            assert_eq!(scalar.scalar_type, caller.parameters[position].scalar_type);
            assert_eq!(scalar.placement, body.call_plan.parameters[position]);
        }
        assert!(
            matches!(body.operations.as_slice(), [TargetUnitOperation::Return { cleanup_actions, .. }] if cleanup_actions.is_empty())
        );
    }
}
