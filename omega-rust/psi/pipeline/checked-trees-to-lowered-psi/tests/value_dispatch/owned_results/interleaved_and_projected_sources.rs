use super::{INTERLEAVED_SOURCE, PARAMETER_SOURCE, PROJECTED_FIELD_SOURCE};
use crate::value_dispatch::{
    TerminalExecutionResult, TerminalScalarValue, check_source, execute, unsigned,
};
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use terminal_interpreter::TerminalStructuralInputs;

#[test]
fn interleaved_live_owner_preserves_mixed_root_cleanup() {
    for selected in [true, false] {
        let (module, execution) = execute(
            INTERLEAVED_SOURCE,
            &[TerminalScalarValue::Boolean(selected), unsigned(91)],
        );
        assert_eq!(
            execution.value(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(selected)),
            "selected={selected}"
        );
        let machine = module
            .machines
            .iter()
            .find(|machine| machine.id == module.entry)
            .unwrap();
        assert_eq!(
            machine
                .blocks
                .iter()
                .filter(|block| !block.structural_parameters.is_empty())
                .count(),
            1,
            "interleaved selection uses one result/residual continuation"
        );
        let cleanup_actions = machine
            .blocks
            .iter()
            .find_map(|block| match &block.terminator {
                terminal_psi::Terminator::Return {
                    cleanup_actions, ..
                } => Some(cleanup_actions.as_slice()),
                _ => None,
            })
            .expect("scalar return cleanup");
        // Reverse joined-frontier order: the selected result, the surviving
        // candidate's residual slot, then the transported interleaved owner.
        assert_eq!(cleanup_actions.len(), 3, "{cleanup_actions:?}");
        assert!(
            cleanup_actions.iter().all(|action| matches!(
                action,
                terminal_psi::TerminalAffineCleanupAction::DiscardRoot(_)
            )),
            "{cleanup_actions:?}"
        );
    }
}

#[test]
fn interleaved_selection_keeps_every_transported_owner_at_its_slot() {
    let source = "data Choice { case Empty; case Some(value: u32); }
        data Marker { tag: u64; }
        machine choose(selected: bool, tag: u64) -> bool {
            let left: Choice = Choice::Some { value: 37 };
            let first: Marker = Marker { tag: tag };
            let right: Choice = Choice::Empty;
            let second: Marker = Marker { tag: 7 };
            let result: Choice = match selected { true -> left, false -> right };
            let observed: bool = result in Choice::Some;
            first.tag == tag && second.tag == 7 && observed
        }";
    for selected in [true, false] {
        let (_, execution) = execute(
            source,
            &[TerminalScalarValue::Boolean(selected), unsigned(91)],
        );
        assert_eq!(
            execution.value(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(selected)),
            "selected={selected}"
        );
    }
}

#[test]
fn interleaved_fresh_arm_discards_only_the_displaced_candidate() {
    let source = INTERLEAVED_SOURCE
        .replace("selected: bool", "selected: u64")
        .replace(
            "true -> left,\n        false -> right",
            "0 -> left,\n        1 -> right,\n        _ -> Choice::Empty",
        );
    for (selected, expected) in [(0, true), (1, false), (2, false)] {
        let (module, execution) = execute(&source, &[unsigned(selected), unsigned(91)]);
        assert_eq!(
            execution.value(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(expected)),
            "selected={selected}"
        );
        let machine = module
            .machines
            .iter()
            .find(|machine| machine.id == module.entry)
            .unwrap();
        let discarded = machine
            .blocks
            .iter()
            .filter_map(|block| match &block.terminator {
                terminal_psi::Terminator::Jump {
                    trivial_affine_discards,
                    ..
                } if !trivial_affine_discards.is_empty() => Some(trivial_affine_discards.len()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            discarded,
            [1],
            "the fresh arm edge discards exactly the displaced candidate"
        );
    }
}

#[test]
fn interleaved_selection_rejects_reordered_or_incomplete_return_cleanup() {
    let checked = check_source(INTERLEAVED_SOURCE).unwrap();
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("choose"),
    )
    .unwrap();
    for mutation in 0..3 {
        let mut changed = lowered.semantic_module.clone();
        let machine = changed
            .machines
            .iter_mut()
            .find(|machine| machine.id == changed.entry)
            .unwrap();
        let cleanup_actions = machine
            .blocks
            .iter_mut()
            .find_map(|block| match &mut block.terminator {
                terminal_psi::Terminator::Return {
                    cleanup_actions, ..
                } => Some(cleanup_actions),
                _ => None,
            })
            .expect("scalar return cleanup");
        assert_eq!(cleanup_actions.len(), 3);
        match mutation {
            // Losing the interleaved owner's discard leaks a live root.
            0 => {
                cleanup_actions.pop();
            }
            // Reordering breaks the shared establishment-order schedule.
            1 => cleanup_actions.swap(1, 2),
            // A duplicated residual fabricates a second cleanup obligation.
            _ => cleanup_actions.push(cleanup_actions[1].clone()),
        }
        assert!(
            terminal_verifier::verify_module(
                &changed,
                &lowered.proof_bundle,
                &crate::value_dispatch::AdmissionProfile::default()
            )
            .is_err(),
            "interleaved cleanup mutation {mutation}"
        );
    }
}

#[test]
fn interleaved_selection_rejects_swapped_join_arguments() {
    let checked = check_source(INTERLEAVED_SOURCE).unwrap();
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("choose"),
    )
    .unwrap();
    let mut changed = lowered.semantic_module.clone();
    let machine = changed
        .machines
        .iter_mut()
        .find(|machine| machine.id == changed.entry)
        .unwrap();
    let join = machine
        .blocks
        .iter()
        .find(|block| !block.structural_parameters.is_empty())
        .expect("result/residual continuation")
        .id;
    let mut swapped = 0;
    for block in &mut machine.blocks {
        if let terminal_psi::Terminator::Jump {
            target,
            structural_arguments,
            ..
        } = &mut block.terminator
            && *target == join
        {
            // Each arm edge supplies the transported owner, the surviving
            // candidate's residual, and the selected result positionally.
            assert_eq!(structural_arguments.len(), 3);
            structural_arguments.swap(0, 1);
            swapped += 1;
        }
    }
    assert_eq!(
        swapped, 2,
        "both arm edges transport the interleaved frontier"
    );
    assert!(
        terminal_verifier::verify_module(
            &changed,
            &lowered.proof_bundle,
            &crate::value_dispatch::AdmissionProfile::default()
        )
        .is_err()
    );
}

#[test]
fn owned_match_projected_children_execute_and_close_root_residuals() {
    let (first, second) = (0x8123456789abcdef_u128, 0xfedcba9876543210_u128);
    for selected in [0_u128, 1] {
        let (module, execution) = execute(
            PROJECTED_FIELD_SOURCE,
            &[unsigned(selected), unsigned(first), unsigned(second)],
        );
        // Arm 0 moves `pair.first`; the fallback arm moves `second` out of the
        // call product `supply(first ^ 255, second ^ 255)`.
        let expected = if selected == 0 {
            first ^ second
        } else {
            (second ^ 255) ^ (first ^ 255)
        };
        assert_eq!(
            execution.value(),
            TerminalExecutionResult::Scalar(unsigned(expected)),
            "selected={selected}"
        );
        let machine = module
            .machines
            .iter()
            .find(|machine| machine.id == module.entry)
            .unwrap();
        let projected = machine
            .blocks
            .iter()
            .filter_map(|block| match &block.terminator {
                terminal_psi::Terminator::Jump {
                    structural_arguments,
                    residual_affine_discards,
                    ..
                } if structural_arguments
                    .iter()
                    .any(|argument| !argument.path.is_empty()) =>
                {
                    Some((
                        structural_arguments.as_slice(),
                        residual_affine_discards.as_slice(),
                    ))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(projected.len(), 2, "each arm edge projects its child");
        // The local arm moves `pair.first`; the call arm moves the product's
        // `second`. Pick the edge whose moved path belongs to the arm this
        // execution selected.
        let moved_field = if selected == 0 { "first" } else { "second" };
        let (arguments, residuals) = projected
            .iter()
            .copied()
            .find(|(arguments, _)| {
                arguments.iter().any(|argument| {
                    argument.path
                        == vec![terminal_psi::StructuralPathSegment::Field(
                            moved_field.into(),
                        )]
                })
            })
            .expect("the executed arm's edge carries its projection");
        let moved = arguments
            .iter()
            .find(|argument| !argument.path.is_empty())
            .expect("selected edge moves a projected child");
        assert_eq!(
            moved.access,
            terminal_psi::StructuralAccess::Owned,
            "the moved child keeps owned access"
        );
        // The untouched sibling residual closes on the same edge, under the
        // same root place: `pair.second` for the local arm, the product's
        // `first` for the call arm.
        let sibling = if selected == 0 { "second" } else { "first" };
        assert!(
            residuals.iter().any(|discard| {
                discard.place == moved.place
                    && discard.path
                        == vec![terminal_psi::StructuralPathSegment::Field(sibling.into())]
            }),
            "the untouched residual sibling dies on the selected edge: {residuals:?}"
        );
    }
}

#[test]
fn owned_match_projected_children_reject_mutated_edge_evidence() {
    let checked = check_source(PROJECTED_FIELD_SOURCE).unwrap();
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("choose"),
    )
    .unwrap();
    for mutation in 0..3 {
        let mut changed = lowered.semantic_module.clone();
        for block in changed
            .machines
            .iter_mut()
            .flat_map(|machine| &mut machine.blocks)
        {
            let terminal_psi::Terminator::Jump {
                structural_arguments,
                residual_affine_discards,
                ..
            } = &mut block.terminator
            else {
                continue;
            };
            match mutation {
                // The moved child path must equal the checked projection.
                0 => {
                    for argument in structural_arguments
                        .iter_mut()
                        .filter(|argument| !argument.path.is_empty())
                    {
                        argument.path[0] = terminal_psi::StructuralPathSegment::from("forged");
                    }
                }
                // Each residual discard must name the exact untouched sibling.
                1 => {
                    for discard in residual_affine_discards.iter_mut() {
                        discard.path[0] = terminal_psi::StructuralPathSegment::from("forged");
                    }
                }
                // Dropping the residual leaves the sibling undisposed.
                _ => residual_affine_discards.clear(),
            }
        }
        assert!(
            terminal_verifier::verify_module(
                &changed,
                &lowered.proof_bundle,
                &crate::value_dispatch::AdmissionProfile::default()
            )
            .is_err(),
            "projected edge mutation {mutation}"
        );
    }
}

#[test]
fn owned_match_parameter_source_returns_the_exact_selected_identity() {
    let checked = check_source(PARAMETER_SOURCE).expect("parameter selection checks");
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("pick"),
    )
    .expect("parameter selection lowers");
    let semantic_bytes =
        terminal_codec::encode_module(&lowered.semantic_module).expect("encode semantics");
    let proof_bytes =
        terminal_codec::encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
            .expect("encode proof");
    let module = terminal_codec::decode_module(&semantic_bytes).expect("decode semantics");
    let proof = terminal_codec::decode_proof_bundle(&proof_bytes).expect("decode proof");
    terminal_verifier::verify_module(
        &module,
        &proof,
        &crate::value_dispatch::AdmissionProfile::default(),
    )
    .expect("independent parameter-source verification");
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .expect("entry");
    assert_eq!(
        entry.structural_parameters.len(),
        2,
        "pick carries both owned parameters"
    );
    let arguments = entry
        .structural_parameters
        .iter()
        .enumerate()
        .map(
            |(index, parameter)| terminal_interpreter::TerminalStructuralValue {
                opaque_identity: 0x1e07 + index as u64,
                structural_type: parameter.structural_type,
                qualifications: Vec::new(),
                path: Vec::new(),
            },
        )
        .collect::<Vec<_>>();
    for (selected, expected) in [(true, 0_usize), (false, 1)] {
        let execution = terminal_interpreter::interpret_terminal_artifact_measured(
            &semantic_bytes,
            &proof_bytes,
            &crate::value_dispatch::AdmissionProfile::default(),
            &[TerminalScalarValue::Boolean(selected)],
            TerminalStructuralInputs {
                arguments: &arguments,
                ..Default::default()
            },
            &mut terminal_interpreter::AcceptTerminalEffects,
        )
        .expect("parameter-source execution");
        assert_eq!(
            execution.value(),
            TerminalExecutionResult::Structural(terminal_interpreter::TerminalStructuralResult {
                value: arguments[expected].clone(),
                claims: Vec::new(),
            }),
            "selected={selected}: the result keeps the selected parameter's exact identity"
        );
    }
}

#[test]
fn owned_match_parameter_sources_reject_mutated_residual_cleanup() {
    let checked = check_source(PARAMETER_SOURCE).expect("parameter selection checks");
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("choose"),
    )
    .expect("parameter selection lowers");
    let semantic_bytes =
        terminal_codec::encode_module(&lowered.semantic_module).expect("encode semantics");
    let proof_bytes =
        terminal_codec::encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
            .expect("encode proof");
    let module = terminal_codec::decode_module(&semantic_bytes).expect("decode semantics");
    let proof = terminal_codec::decode_proof_bundle(&proof_bytes).expect("decode proof");
    terminal_verifier::verify_module(
        &module,
        &proof,
        &crate::value_dispatch::AdmissionProfile::default(),
    )
    .expect("independent parameter-source verification");
    let machine = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .expect("entry");
    // Each arm edge still transports every live owner positionally: the
    // selected parameter and the residual complement share one continuation.
    let join_edges = machine
        .blocks
        .iter()
        .filter(|block| {
            matches!(
                &block.terminator,
                terminal_psi::Terminator::Jump {
                    structural_arguments,
                    ..
                } if !structural_arguments.is_empty()
            )
        })
        .count();
    assert!(
        join_edges >= 2,
        "each selection edge binds the join frontier"
    );
    for mutation in 0..2 {
        let mut changed = lowered.semantic_module.clone();
        let machine = changed
            .machines
            .iter_mut()
            .find(|machine| machine.id == changed.entry)
            .unwrap();
        match mutation {
            // Dropping a residual discard leaks the unselected parameter. The
            // residual dies at its actual death edge: the scalar return's
            // cleanup actions for `choose`, or a selection edge for `pick`.
            0 => {
                let mut cleared = 0;
                for block in &mut machine.blocks {
                    match &mut block.terminator {
                        terminal_psi::Terminator::Jump {
                            trivial_affine_discards,
                            ..
                        }
                        | terminal_psi::Terminator::ReturnStructural {
                            trivial_affine_discards,
                            ..
                        }
                        | terminal_psi::Terminator::ReturnUnit {
                            trivial_affine_discards,
                            ..
                        } if !trivial_affine_discards.is_empty() => {
                            trivial_affine_discards.clear();
                            cleared += 1;
                        }
                        terminal_psi::Terminator::Return {
                            cleanup_actions, ..
                        } if !cleanup_actions.is_empty() => {
                            cleanup_actions.clear();
                            cleared += 1;
                        }
                        _ => {}
                    }
                }
                assert!(cleared > 0, "a selection edge carries residual cleanup");
            }
            // Duplicating an edge argument moves the same owner into two
            // join slots while its real residual slot goes unbound.
            _ => {
                let mut duplicated = 0;
                for block in &mut machine.blocks {
                    if let terminal_psi::Terminator::Jump {
                        structural_arguments,
                        ..
                    } = &mut block.terminator
                        && structural_arguments.len() > 1
                    {
                        structural_arguments[0].place = structural_arguments[1].place;
                        duplicated += 1;
                    }
                }
                assert!(duplicated > 0, "selection edges carry positional arguments");
            }
        }
        assert!(
            terminal_verifier::verify_module(
                &changed,
                &lowered.proof_bundle,
                &crate::value_dispatch::AdmissionProfile::default()
            )
            .is_err(),
            "parameter-source cleanup mutation {mutation}"
        );
    }
}

#[test]
fn projected_parameter_roots_move_the_selected_child_with_exact_identity() {
    let checked = check_source(PARAMETER_SOURCE).expect("parameter selection checks");
    checked_trees_to_lowered_psi::lower_machine(&checked, TerminalMachineSelection::Name("peek"))
        .expect("same-root projected parameter selection lowers");
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("select_field"),
    )
    .expect("projected parameter selection lowers");
    let semantic_bytes =
        terminal_codec::encode_module(&lowered.semantic_module).expect("encode semantics");
    let proof_bytes =
        terminal_codec::encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
            .expect("encode proof");
    let module = terminal_codec::decode_module(&semantic_bytes).expect("decode semantics");
    let proof = terminal_codec::decode_proof_bundle(&proof_bytes).expect("decode proof");
    terminal_verifier::verify_module(
        &module,
        &proof,
        &crate::value_dispatch::AdmissionProfile::default(),
    )
    .expect("independent projected-parameter verification");
    let machine = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .expect("entry");
    // Each arm edge moves one child out of its parameter root: the moved path
    // rides the result argument while the root's untouched sibling is that
    // same edge's residual discard.
    let projected = machine
        .blocks
        .iter()
        .filter_map(|block| match &block.terminator {
            terminal_psi::Terminator::Jump {
                structural_arguments,
                residual_affine_discards,
                ..
            } if structural_arguments
                .iter()
                .any(|argument| !argument.path.is_empty()) =>
            {
                Some((
                    structural_arguments.clone(),
                    residual_affine_discards.clone(),
                ))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        projected.len(),
        2,
        "each arm edge moves its projected child"
    );
    for (arguments, residuals) in &projected {
        let moved = arguments
            .iter()
            .find(|argument| !argument.path.is_empty())
            .expect("one projected argument per edge");
        assert_eq!(moved.access, terminal_psi::StructuralAccess::Owned);
        assert!(
            machine
                .structural_parameters
                .iter()
                .any(|parameter| parameter.place == moved.place),
            "the moved root is a signature parameter"
        );
        assert_eq!(
            residuals.len(),
            1,
            "the residual complement dies on the selected edge"
        );
        assert_eq!(residuals[0].place, moved.place);
        assert_eq!(residuals[0].path.len(), 1);
    }
    let payload_type = machine
        .result
        .structural()
        .expect("structural result")
        .structural_type;
    let arguments = machine
        .structural_parameters
        .iter()
        .enumerate()
        .map(
            |(index, parameter)| terminal_interpreter::TerminalStructuralValue {
                opaque_identity: 0x5eed + index as u64,
                structural_type: parameter.structural_type,
                qualifications: Vec::new(),
                path: Vec::new(),
            },
        )
        .collect::<Vec<_>>();
    for (selected, root, field) in [(true, 0_usize, "first"), (false, 1, "second")] {
        let execution = terminal_interpreter::interpret_terminal_artifact_measured(
            &semantic_bytes,
            &proof_bytes,
            &crate::value_dispatch::AdmissionProfile::default(),
            &[TerminalScalarValue::Boolean(selected)],
            TerminalStructuralInputs {
                arguments: &arguments,
                ..Default::default()
            },
            &mut terminal_interpreter::AcceptTerminalEffects,
        )
        .expect("projected parameter execution");
        assert_eq!(
            execution.value(),
            TerminalExecutionResult::Structural(terminal_interpreter::TerminalStructuralResult {
                value: terminal_interpreter::TerminalStructuralValue {
                    opaque_identity: arguments[root].opaque_identity,
                    structural_type: payload_type,
                    qualifications: Vec::new(),
                    path: vec![terminal_psi::StructuralPathSegment::Field(field.into())],
                },
                claims: Vec::new(),
            }),
            "selected={selected}: the result keeps the moved child's exact identity"
        );
    }
}

#[test]
fn projected_parameter_roots_reject_mutated_residual_cleanup() {
    let checked = check_source(PARAMETER_SOURCE).expect("parameter selection checks");
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("select_field"),
    )
    .expect("projected parameter selection lowers");
    for mutation in 0..2 {
        let mut changed = lowered.semantic_module.clone();
        let machine = changed
            .machines
            .iter_mut()
            .find(|machine| machine.id == changed.entry)
            .unwrap();
        let mut touched = 0;
        for block in &mut machine.blocks {
            if let terminal_psi::Terminator::Jump {
                structural_arguments,
                residual_affine_discards,
                ..
            } = &mut block.terminator
                && structural_arguments
                    .iter()
                    .any(|argument| !argument.path.is_empty())
            {
                match mutation {
                    // The residual complement must die on this exact edge.
                    0 => residual_affine_discards.clear(),
                    // A whole binding cannot carry the moved child's custody.
                    _ => structural_arguments
                        .iter_mut()
                        .for_each(|argument| argument.path.clear()),
                }
                touched += 1;
            }
        }
        assert_eq!(touched, 2, "both arm edges carry projected custody");
        assert!(
            terminal_verifier::verify_module(
                &changed,
                &lowered.proof_bundle,
                &crate::value_dispatch::AdmissionProfile::default()
            )
            .is_err(),
            "projected parameter cleanup mutation {mutation}"
        );
    }
}

#[test]
fn heterogeneous_residual_sources_reject_the_custody_join() {
    // A differently-typed survivor cannot rebind through its sibling's
    // residual slot: on the edge that selects it the survivor sequence
    // shifts, and the moved root would land in the wrong slot. Both the
    // local and parameter forms keep a clean lowering rejection rather than
    // emitting an edge the terminal verifier must refuse.
    let local = r#"
        data Payload { left: u64; right: u64; }
        data Pair { first: Payload; second: Payload; }
        machine choose(selected: bool) -> u64 {
            let pair: Pair = Pair {
                first: Payload { left: 3, right: 4 },
                second: Payload { left: 5, right: 6 }
            };
            let fallback: Payload = Payload { left: 7, right: 8 };
            let result: Payload = match selected {
                true -> pair.first,
                false -> fallback
            };
            result.left ^ result.right
        }
    "#;
    let parameter = r#"
        data Payload { left: u64; right: u64; }
        data Pair { first: Payload; second: Payload; }
        machine choose(selected: bool, pair: Pair, fallback: Payload) -> u64 {
            let result: Payload = match selected {
                true -> pair.first,
                false -> fallback
            };
            result.left ^ result.right
        }
    "#;
    for source in [local, parameter] {
        let checked =
            check_source(source).unwrap_or_else(|errors| panic!("checking {source}: {errors:#?}"));
        let error = checked_trees_to_lowered_psi::lower_machine(
            &checked,
            TerminalMachineSelection::Name("choose"),
        )
        .expect_err("heterogeneous residual custody stays rejected");
        assert!(
            matches!(
                error,
                checked_trees_to_lowered_psi::LoweringError::Unsupported(
                    "owned selection residual sources need uniform custody types"
                )
            ),
            "{error:?}"
        );
    }
}
