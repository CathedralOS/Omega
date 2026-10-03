//! Content observations retain selective descriptor preparation and exact source custody.
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use terminal_interpreter::{
    AcceptTerminalEffects, TerminalEffect, TerminalExecutionResult, TerminalScalarValue,
    TerminalStructuralInputs, interpret_terminal_artifact_measured,
};

const SOURCE: &str = r#"
    boundary trait Output { machine flag(value: bool) reaches Output; }
    data Helper {}
    machine Helper::inspect(bytes: &[u8], other: &[u8], count: u64) reaches Output {
        Output::flag(count <= bytes.len && bytes[0..count] == "look");
    }
    data Root {}
    machine Root::enter() reaches Output {
        Helper::inspect("look\n", "quit\n", 4);
        Helper::inspect("look", "quit", 4);
        Helper::inspect("quit", "look", 4);
        Helper::inspect("", "look", 4);
        Helper::inspect("", "look", 0);
    }
"#;

fn lower(
    checked: &checked_trees::CheckedTrees,
) -> Result<lowered_psi::LoweredPsi, checked_trees_to_lowered_psi::LoweringError> {
    checked_trees_to_lowered_psi::lower_machine(
        checked,
        TerminalMachineSelection::Name("Root::enter"),
    )
}

#[test]
fn computed_byte_comparisons_evaluate_only_after_the_bound_guard() {
    let checked = crate::front_end::checked_program(SOURCE);
    let lowered = lower(&checked).expect("computed view comparison lowers");
    let result = interpret_terminal_artifact_measured(
        &terminal_codec::encode_module(&lowered.semantic_module).unwrap(),
        &terminal_codec::encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
            .unwrap(),
        &proof_admission::AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs::default(),
        &mut AcceptTerminalEffects,
    )
    .expect("source-free comparison replay and execution");
    assert_eq!(result.value(), TerminalExecutionResult::Unit);
    let flags: Vec<_> = result
        .effects()
        .iter()
        .map(|effect| {
            let TerminalEffect::BoundaryCall { arguments, .. } = effect else {
                panic!("flag effect");
            };
            let [TerminalScalarValue::Boolean(value)] = arguments.as_slice() else {
                panic!("Boolean flag");
            };
            *value
        })
        .collect();
    assert_eq!(flags, [true, true, false, false, false]);
}

#[test]
fn computed_byte_comparisons_reject_a_same_typed_substituted_source() {
    let mut checked = crate::front_end::checked_program(SOURCE);
    let mut altered = 0;
    let nodes: Vec<_> = checked
        .facts
        .values
        .scalar_computations
        .nodes
        .iter()
        .map(|(handle, _)| handle)
        .collect();
    for handle in nodes {
        let node = checked
            .facts
            .values
            .scalar_computations
            .nodes
            .get_mut(handle);
        let checked_trees::CheckedScalarComputationKind::ByteSequenceLiteralComparison {
            subject,
            ..
        } = &mut node.kind
        else {
            continue;
        };
        let checked_trees::CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice {
            root,
            ..
        } = &mut subject.source
        else {
            continue;
        };
        assert_eq!(
            *root,
            checked_trees::CheckedStorageRoot::Parameter { index: 0 }
        );
        *root = checked_trees::CheckedStorageRoot::Parameter { index: 1 };
        altered += 1;
    }
    assert!(altered > 0, "the computed operand was retained");
    let error = lower(&checked).expect_err("equal carrier shape cannot substitute another operand");
    assert!(
        matches!(
            error,
            checked_trees_to_lowered_psi::LoweringError::Unsupported(_)
        ),
        "source correspondence must reject the substitution before Terminal proof: {error:?}"
    );
}

#[test]
fn computed_byte_comparisons_preserve_whole_view_comparison() {
    let source = SOURCE.replace("count <= bytes.len && bytes[0..count]", "bytes");
    let checked = crate::front_end::checked_program(&source);
    lower(&checked).expect("whole views share the content query without descriptor preparation");
}

#[test]
fn computed_byte_comparisons_retain_unannotated_fixed_array_views() {
    let checked = crate::front_end::checked_program(
        r#"
        boundary trait Output { machine flag(value: bool) reaches Output; }
        data Root {}
        machine Root::enter() reaches Output {
            let bytes: [u8; 4] = [108, 111, 111, 107];
            Output::flag(bytes[0..4] == "look");
        }
    "#,
    );
    let lowered =
        lower(&checked).expect("a computed descriptor needs no unrelated view annotation");
    let result = interpret_terminal_artifact_measured(
        &terminal_codec::encode_module(&lowered.semantic_module).unwrap(),
        &terminal_codec::encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
            .unwrap(),
        &proof_admission::AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs::default(),
        &mut AcceptTerminalEffects,
    )
    .expect("fixed-array view executes through source-free replay");
    assert!(
        matches!(result.effects(), [TerminalEffect::BoundaryCall { arguments, .. }]
        if arguments.as_slice() == [TerminalScalarValue::Boolean(true)])
    );
}

#[test]
fn computed_byte_comparisons_observe_indexed_literal_field_stores() {
    let checked = crate::front_end::checked_program(
        r#"
        domain [u8]::Utf8(bytes) requires valid_utf8(bytes);
        boundary trait Output { machine flag(value: bool) reaches Output; }
        data Room { name: &[u8] in Utf8; }
        data Root { rooms: [Room; 3]; }
        machine Root::enter(root: &mut Root) reaches Output {
            let index: u64 = 1;
            let room_slice: &mut [Room] = root.rooms.as_mut_slice();
            room_slice[index].name = "expected";
            transition room_slice[index].name == "expected" {
                true -> finish(root)
                false -> fail(root)
            }
            state finish(root: &mut Root) { Output::flag(true); }
            state fail(root: &mut Root) { Output::flag(false); }
        }
        "#,
    );
    let lowered = lower(&checked).expect("indexed descriptor replacement and observation lower");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("indexed store authority and observation bounds replay");
    let encoded = terminal_codec::encode_module(&lowered.semantic_module)
        .expect("indexed field operation encodes");
    let decoded = terminal_codec::decode_module(&encoded).expect("indexed field operation reloads");
    assert_eq!(decoded, lowered.semantic_module);
    let entry = &decoded.machines[0];
    let input = [terminal_interpreter::TerminalStructuralValue {
        opaque_identity: 991,
        structural_type: entry.structural_parameters[0].structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    }];
    let interpreted = interpret_terminal_artifact_measured(
        &encoded,
        &terminal_codec::encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
            .unwrap(),
        &proof_admission::AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &input,
            ..Default::default()
        },
        &mut AcceptTerminalEffects,
    )
    .expect("indexed descriptor store and guarded observation execute after reload");
    assert!(
        matches!(interpreted.effects(), [TerminalEffect::BoundaryCall { arguments, .. }] if arguments.as_slice() == [TerminalScalarValue::Boolean(true)])
    );

    for observation in [false, true] {
        let mut forged = lowered.semantic_module.clone();
        let mut altered = false;
        for operation in forged
            .machines
            .iter_mut()
            .flat_map(|machine| &mut machine.blocks)
            .flat_map(|block| &mut block.operations)
        {
            let path =
                match &mut operation.kind {
                    terminal_psi::OperationKind::StructuralByteViewFieldStore { path, .. }
                        if !observation =>
                    {
                        path
                    }
                    terminal_psi::OperationKind::StructuralByteSequenceFieldLength {
                        path, ..
                    } if observation => path,
                    _ => continue,
                };
            if let Some(segment) = path.iter_mut().find(|segment| {
                matches!(
                    segment,
                    terminal_psi::StructuralPathSegment::RuntimeIndex { .. }
                )
            }) {
                *segment = terminal_psi::StructuralPathSegment::FixedIndex(1);
                altered = true;
                break;
            }
        }
        assert!(altered, "dynamic view selection carries its bound");
        assert!(
            terminal_verifier::verify_module(
                &forged,
                &lowered.proof_bundle,
                &proof_admission::AdmissionProfile::default()
            )
            .is_err(),
            "a literal view index cannot erase its runtime bound"
        );
    }

    let mut changed = checked.clone();
    let handles: Vec<_> = changed
        .facts
        .values
        .scalar_computations
        .nodes
        .iter()
        .map(|(handle, _)| handle)
        .collect();
    let mut altered = false;
    for handle in handles {
        let node = changed
            .facts
            .values
            .scalar_computations
            .nodes
            .get_mut(handle);
        let checked_trees::CheckedScalarComputationKind::ByteSequenceLiteralComparison {
            subject,
            ..
        } = &mut node.kind
        else {
            continue;
        };
        for segment in &mut subject.path {
            if let checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(index) = segment {
                assert_eq!(*index, 1);
                *index = 0;
                altered = true;
            }
        }
    }
    assert!(
        altered,
        "the projected comparison retains its exact element"
    );
    assert!(
        matches!(
            lower(&changed),
            Err(checked_trees_to_lowered_psi::LoweringError::Unsupported(_))
        ),
        "same-typed sibling storage cannot replace the source operand"
    );
}

#[test]
fn stored_descriptor_operands_replay_parameter_writeback() {
    let checked = crate::front_end::checked_program(
        r#"
        boundary trait Output { machine flag(value: bool) reaches Output; }
        data Holder { text: &[u8]; }
        machine assign(holder: &mut Holder, bytes: &[u8]) {
            holder.text = bytes;
        }
        data Root {}
        machine Root::enter() reaches Output {
            let holder: Holder;
            holder.text = "";
            assign(&mut holder, "look");
            Output::flag(holder.text == "look");
        }
    "#,
    );
    let lowered = lower(&checked).expect("a parameter descriptor stores through a Unit call");
    let result = interpret_terminal_artifact_measured(
        &terminal_codec::encode_module(&lowered.semantic_module).unwrap(),
        &terminal_codec::encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
            .unwrap(),
        &proof_admission::AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs::default(),
        &mut AcceptTerminalEffects,
    )
    .expect("source-free replay retains the caller's current stored descriptor");
    assert!(
        matches!(result.effects(), [TerminalEffect::BoundaryCall { arguments, .. }]
        if arguments.as_slice() == [TerminalScalarValue::Boolean(true)])
    );
    // A matching record type cannot lend a local whose establishment has not
    // executed. Exercise the source-free verifier after changing operation
    // order, independently of the checked source's definite initialization.
    let mut premature = lowered.semantic_module.clone();
    let mut changed = false;
    for machine in &mut premature.machines {
        for block in &mut machine.blocks {
            let Some(call_index) = block.operations.iter().position(|operation| {
                matches!(operation.kind, terminal_psi::OperationKind::CallUnit { .. })
            }) else {
                continue;
            };
            let Some(birth_index) = block.operations[..call_index].iter().position(|operation| {
                matches!(
                    operation.kind,
                    terminal_psi::OperationKind::EstablishStructuralLocal { .. }
                )
            }) else {
                continue;
            };
            let call = block.operations.remove(call_index);
            block.operations.insert(birth_index, call);
            changed = true;
        }
    }
    assert!(changed, "the ordinary call borrows the local carrier");
    assert!(
        terminal_verifier::verify_module(
            &premature,
            &lowered.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .is_err(),
        "the caller cannot borrow an unestablished record"
    );
}

#[test]
fn stored_descriptor_operands_replay_projected_predicates() {
    let checked = crate::front_end::checked_program(
        r#"
        domain [u8]::Utf8(bytes) requires valid_utf8(bytes);
        boundary trait Output { machine flag(value: bool) reaches Output; }
        data Holder { text: &[u8] in Utf8; }
        machine copy(destination: &mut Holder, source: &Holder) {
            destination.text = source.text;
        }
        data Root {}
        machine Root::enter() reaches Output {
            let source: Holder;
            source.text = "look";
            let destination: Holder;
            destination.text = "";
            copy(&mut destination, &source);
            Output::flag(destination.text == "look");
        }
    "#,
    );
    let lowered = lower(&checked).expect("a projected shared descriptor lowers");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("projected source predicates and backing replay independently");
    let result = interpret_terminal_artifact_measured(
        &terminal_codec::encode_module(&lowered.semantic_module).unwrap(),
        &terminal_codec::encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
            .unwrap(),
        &proof_admission::AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs::default(),
        &mut AcceptTerminalEffects,
    )
    .expect("actual caller backing satisfies projected-copy compatibility");
    assert!(
        matches!(result.effects(), [TerminalEffect::BoundaryCall { arguments, .. }]
        if arguments.as_slice() == [TerminalScalarValue::Boolean(true)])
    );
}

#[test]
fn stored_descriptor_operands_replay_current_subslice() {
    let checked = crate::front_end::checked_program(
        r#"
        data Holder { text: &[u8]; }
        data Root {}
        machine Root::enter(destination: &mut Holder, source: &[u8]) {
            destination.text = source[..];
        }
    "#,
    );
    let lowered = lower(&checked).expect("a current shared subslice descriptor lowers");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("subslice source provenance and destination lifetime replay independently");
}

#[test]
fn stored_descriptor_operands_reject_same_typed_source_substitution() {
    let mut checked = crate::front_end::checked_program(
        r#"
        data Holder { text: &[u8]; }
        data Root {}
        machine Root::enter(destination: &mut Holder, first: &[u8], second: &[u8]) {
            destination.text = first;
        }
    "#,
    );
    lower(&checked).expect("the authored source lowers before substitution");
    let mut altered = 0;
    for machine in &mut checked.facts.flow.terminal_unit_effects.machines {
        for operation in &mut machine.operations {
            let checked_trees::CheckedUnitEffectOperationPlan::ByteSequenceStore(store) = operation
            else {
                continue;
            };
            let checked_trees::CheckedByteSequenceValue::Descriptor { source, .. } =
                store.value.nodes.get_mut(store.value.root)
            else {
                continue;
            };
            if let checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                parameter_index,
            } = &mut source.source
            {
                assert_eq!(*parameter_index, 1);
                *parameter_index = 2;
                altered += 1;
            }
        }
    }
    assert_eq!(altered, 1, "the exact source occurrence was retained");
    assert!(
        lower(&checked).is_err(),
        "matching descriptor shape cannot substitute another source"
    );
}

#[test]
fn byte_predicate_observations_replay_across_loop_arrivals() {
    let checked = crate::front_end::checked_program(
        r#"
        boundary trait Output { machine flag(value: bool) reaches Output; }
        data Helper {}
        machine Helper::inspect(bytes: &[u8]) reaches Output {
            transition { _ -> observe(bytes, 0) }
            state observe(bytes: &[u8], index: u64) {
                Output::flag(valid_utf8(bytes));
                transition index < 2 {
                    true -> observe(bytes, index + 1)
                    false -> done()
                }
            }
            state done() {}
        }
        data Root {}
        machine Root::enter() reaches Output { Helper::inspect("look"); }
        "#,
    );
    let lowered = lower(&checked).expect("byte predicate observations carry no cyclic custody");
    assert!(
        lowered
            .semantic_module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .any(|operation| matches!(
                operation.kind,
                terminal_psi::OperationKind::ByteSequencePredicate { .. }
            ))
    );
    let result = interpret_terminal_artifact_measured(
        &terminal_codec::encode_module(&lowered.semantic_module).unwrap(),
        &terminal_codec::encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
            .unwrap(),
        &proof_admission::AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs::default(),
        &mut AcceptTerminalEffects,
    )
    .expect("source-free loop predicate replay and execution");
    assert_eq!(result.effects().len(), 3);
    assert!(result.effects().iter().all(
        |effect| matches!(effect, TerminalEffect::BoundaryCall { arguments, .. }
            if arguments.as_slice() == [TerminalScalarValue::Boolean(true)])
    ));
}

#[test]
fn stored_local_array_range_retains_its_authored_descriptor_type() {
    let mut checked = crate::front_end::checked_program(
        r#"
        data Holder { text: &[u8]; }
        data Root {}
        machine Root::enter() {
            let backing: [u8;4] = [1,2,3,4];
            let destination: Holder;
            destination.text = backing[..];
        }
        "#,
    );
    let lowered = lower(&checked).expect("a stored range declares its own standalone carrier");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("local array backing and descriptor lifetime remain independently checked");
    assert!(
        lowered
            .semantic_module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .any(|operation| matches!(
                operation.kind,
                terminal_psi::OperationKind::EstablishElementView { .. }
            ))
    );
    let mut altered = 0;
    for machine in &mut checked.facts.flow.terminal_unit_effects.machines {
        for operation in &mut machine.operations {
            let checked_trees::CheckedUnitEffectOperationPlan::ByteSequenceStore(store) = operation
            else {
                continue;
            };
            let checked_trees::CheckedByteSequenceValue::Descriptor { source, .. } =
                store.value.nodes.get_mut(store.value.root)
            else {
                continue;
            };
            if matches!(
                source.source,
                checked_trees::CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice { .. }
            ) {
                source.type_identity = "forged-descriptor-carrier".into();
                altered += 1;
            }
        }
    }
    assert_eq!(altered, 1);
    assert!(
        lower(&checked).is_err(),
        "a shape-compatible descriptor cannot erase exact range type identity"
    );
}
