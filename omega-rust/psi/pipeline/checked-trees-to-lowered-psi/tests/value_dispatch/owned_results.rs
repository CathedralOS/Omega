//! Selected existing owners retain transfer identity through result observation.

use super::{TerminalExecutionResult, TerminalScalarValue, check_source, execute, unsigned};

const SOURCE: &str =
    include_str!("../../../../../../tests/omega/pass/expressions/owned_match_values/main.omg");
const MIXED_SOURCE: &str = include_str!(
    "../../../../../../tests/omega/pass/expressions/owned_match_mixed_values/main.omg"
);

const RECORD_SOURCE: &str = include_str!(
    "../../../../../../tests/omega/pass/expressions/owned_match_record_values/main.omg"
);

#[test]
fn owned_match_nested_record_replays_every_selected_payload() {
    let first = 0x8123456789abcdef;
    let second = 0xfedcba9876543210;
    for selected in 0..4 {
        for inner in [false, true] {
            let (_, execution) = execute(
                RECORD_SOURCE,
                &[
                    unsigned(selected),
                    TerminalScalarValue::Boolean(inner),
                    unsigned(first),
                    unsigned(second),
                ],
            );
            let expected = match selected {
                0 => second,
                1 => first,
                _ => (if inner { first } else { second }) ^ 255,
            };
            assert_eq!(
                execution.value(),
                TerminalExecutionResult::Scalar(unsigned(expected ^ 37))
            );
        }
    }
}

#[test]
fn owned_match_record_rejects_hidden_moves_and_post_selection_reuse() {
    let reused = RECORD_SOURCE.replace("result.payload.get_right()", "left.payload.get_right()");
    let errors = check_source(&reused).expect_err("selected candidate cannot be reused");
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("may have been transferred")),
        "{errors:?}"
    );
    for replacement in ["child", "Payload { left: 1, right: 2 }"] {
        let source = format!("data Payload {{ left: u64; right: u64; }}
            data Pair {{ first: Payload; second: Payload; }}
            machine choose(selected: bool) -> Pair {{
                let child: Payload = Payload {{ left: 3, right: 4 }};
                let original: Pair = Pair {{ first: Payload {{ left: 5, right: 6 }}, second: Payload {{ left: 7, right: 8 }} }};
                let result: Pair = match selected {{
                    true -> original, false -> Pair {{ first: child, second: {replacement} }}
                }};
                let reused: Payload = child;
                result
            }}");
        assert!(
            check_source(&source).is_err(),
            "fresh fields cannot conceal a consumed child: {replacement}"
        );
    }
}

#[test]
fn owned_match_record_frontier_rejects_missing_and_duplicate_disposal() {
    let checked = check_source(RECORD_SOURCE).unwrap();
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "choose").unwrap();
    let module = lowered.semantic_module;
    for missing_discard in [true, false] {
        let mut changed = module.clone();
        let machine = changed
            .machines
            .iter_mut()
            .find(|machine| machine.id == changed.entry)
            .unwrap();
        let mut changed_edges = 0;
        for block in &mut machine.blocks {
            if let terminal_psi::Terminator::Jump {
                trivial_affine_discards,
                structural_arguments,
                ..
            } = &mut block.terminator
                && !trivial_affine_discards.is_empty()
            {
                assert_eq!(trivial_affine_discards.len(), 1);
                if missing_discard {
                    trivial_affine_discards.clear();
                } else {
                    trivial_affine_discards.push(structural_arguments.last().unwrap().place);
                }
                changed_edges += 1;
            }
        }
        assert_eq!(
            changed_edges, 1,
            "only the outer fresh result displaces a candidate"
        );
        assert!(
            terminal_verifier::verify_module(
                &changed,
                &lowered.proof_bundle,
                &super::AdmissionProfile::default()
            )
            .is_err()
        );
    }
}

#[test]
fn owned_match_record_shared_projection_rejects_substituted_custody() {
    let checked = check_source(RECORD_SOURCE).unwrap();
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "choose").unwrap();
    for mutation in 0..4 {
        let mut changed = lowered.semantic_module.clone();
        let machine = changed
            .machines
            .iter_mut()
            .find(|machine| machine.id == changed.entry)
            .unwrap();
        let (block_position, operation_position) = machine
            .blocks
            .iter()
            .enumerate()
            .find_map(|(block_position, block)| {
                block
                    .operations
                    .iter()
                    .position(|operation| {
                        matches!(
                            operation.kind,
                            terminal_psi::OperationKind::CallStructuralScalar { .. }
                        )
                    })
                    .map(|operation_position| (block_position, operation_position))
            })
            .unwrap();
        if mutation == 3 {
            let operation = machine.blocks[block_position]
                .operations
                .remove(operation_position);
            machine
                .blocks
                .iter_mut()
                .find(|block| block.id == machine.entry)
                .unwrap()
                .operations
                .push(operation);
        } else {
            let terminal_psi::OperationKind::CallStructuralScalar {
                structural_arguments,
                ..
            } = &mut machine.blocks[block_position].operations[operation_position].kind
            else {
                unreachable!()
            };
            let argument = &mut structural_arguments[0];
            assert!(!argument.path.is_empty());
            match mutation {
                0 => argument.path.clear(),
                1 => {
                    argument.path[0] =
                        terminal_psi::StructuralPathSegment::Field("missing-field".into())
                }
                2 => argument.access = terminal_psi::StructuralAccess::MutableBorrow,
                _ => unreachable!(),
            }
        }
        assert!(
            terminal_verifier::verify_module(
                &changed,
                &lowered.proof_bundle,
                &super::AdmissionProfile::default()
            )
            .is_err(),
            "changed joined field path/access/availability {mutation}"
        );
    }
}

#[test]
fn owned_match_transfers_only_the_selected_existing_local() {
    for selected in [true, false] {
        let (_, execution) = execute(SOURCE, &[TerminalScalarValue::Boolean(selected)]);
        assert_eq!(
            execution.value(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(selected)),
        );
    }
}

#[test]
fn exclusive_arms_can_transfer_the_same_owned_source() {
    let source = SOURCE.replace("false -> right", "false -> left");
    for selected in [true, false] {
        let (_, execution) = execute(&source, &[TerminalScalarValue::Boolean(selected)]);
        assert_eq!(
            execution.value(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(true)),
        );
    }
}

#[test]
fn nested_selections_share_the_result_continuation() {
    let source = SOURCE
        .replace("selected: bool", "selected: bool, inner: bool")
        .replace(
            "true -> left",
            "true -> match inner { true -> left, false -> right }",
        );
    for selected in [true, false] {
        for inner in [true, false] {
            let (module, execution) = execute(
                &source,
                &[
                    TerminalScalarValue::Boolean(selected),
                    TerminalScalarValue::Boolean(inner),
                ],
            );
            assert_eq!(
                execution.value(),
                TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(selected && inner))
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
                "nested selection uses one result/residual continuation, not a path product"
            );
        }
    }
}

#[test]
fn mixed_fresh_and_existing_arms_transfer_or_discard_the_existing_local() {
    for selected in [true, false] {
        let (module, execution) = execute(MIXED_SOURCE, &[TerminalScalarValue::Boolean(selected)]);
        assert_eq!(
            execution.value(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(selected)),
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
            "mixed selection uses one result/residual continuation"
        );
        assert_eq!(
            machine
                .blocks
                .iter()
                .filter(|block| {
                    matches!(
                        &block.terminator,
                        terminal_psi::Terminator::Jump {
                            trivial_affine_discards,
                            ..
                        } if !trivial_affine_discards.is_empty()
                    )
                })
                .count(),
            1,
            "only the fresh arm edge discards the displaced source"
        );
    }
}

#[test]
fn mixed_selection_with_two_existing_sources_discards_only_the_displaced_owner() {
    let source = MIXED_SOURCE
        .replace("selected: bool", "selected: u64")
        .replace(
            "let left: Choice = Choice::Some { value: 37 };",
            "let left: Choice = Choice::Some { value: 37 };\n    let right: Choice = Choice::Some { value: 5 };",
        )
        .replace(
            "true -> left,\n        false -> Choice::Empty",
            "0 -> left,\n        1 -> right,\n        _ -> Choice::Empty",
        );
    for (selected, expected) in [(0, true), (1, true), (2, false)] {
        let (module, execution) = execute(&source, &[unsigned(selected)]);
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
            "the fresh arm edge discards exactly the displaced owner"
        );
    }
}

#[test]
fn possibly_transferred_source_remains_unobservable_after_mixed_selection() {
    let source = MIXED_SOURCE.replace("result in Choice::Some", "left in Choice::Some");
    let errors = check_source(&source).expect_err("possibly transferred source is unavailable");
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("may have been transferred")),
        "{errors:?}"
    );
}

#[test]
fn owned_selection_receipts_reject_changed_origin_arm_roster_and_death() {
    for source in [SOURCE, RECORD_SOURCE] {
        let original = check_source(source).expect("owned selection checks");
        checked_trees_to_lowered_psi::lower_machine(&original, "choose")
            .expect("original selection reaches independent lowering");
        for mutation in 0..6 {
            let mut changed = original.clone();
            let ownership = &mut changed.facts.flow.ownership;
            let (handle, receipt) = ownership.owned_selections.iter().next().unwrap();
            let receipt = receipt.clone();
            let first_transfer = receipt.transfers.start();
            let first_source = receipt.sources.start();
            let other_source = arena::Handle::from_parts(
                first_source.arena_index() + 1,
                first_source.generation(),
            );
            match mutation {
                0 => {
                    ownership.selection_sources.get_mut(first_source).provenance =
                        ownership.selection_sources.get(other_source).provenance
                }
                1 => {
                    ownership
                        .selection_transfers
                        .get_mut(first_transfer)
                        .source_arm = arena::Handle::invalid()
                }
                2 => {
                    let transfer = ownership.selection_transfers.get_mut(first_transfer);
                    transfer.source = if transfer.source == first_source {
                        other_source
                    } else {
                        first_source
                    };
                }
                3 => {
                    ownership.owned_selections.get_mut(handle).transfers =
                        arena::HandleSpan::default()
                }
                4 => {
                    ownership.owned_selections.get_mut(handle).death =
                        language_semantics::PermissionEventSource::Statement { statement_index: 2 }
                }
                5 => ownership.owned_selections = arena::Arena::default(),
                _ => unreachable!(),
            }
            assert!(
                checked_trees_to_lowered_psi::lower_machine(&changed, "choose").is_err(),
                "selection receipt mutation {mutation}"
            );
        }
    }
}

#[test]
fn possibly_moved_sources_cannot_be_observed_after_selection() {
    let source = SOURCE.replace("result in Choice::Some", "left in Choice::Some");
    let errors = check_source(&source).expect_err("possibly transferred source is unavailable");
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("may have been transferred")),
        "{errors:?}"
    );
}

#[test]
fn duplicate_prior_receipts_cannot_launder_a_fresh_origin_as_unknown() {
    let source = "data Choice { case Empty; case Some(value: u32); }
        machine choose(selected: bool, other: bool) -> bool {
            let first: Choice = match selected {
                true -> Choice::Some { value: 37 }, false -> Choice::Empty
            };
            let result: Choice = match other { true -> first, false -> first };
            result in Choice::Some
        }";
    let original = super::check_source(source).expect("fresh origin followed by selection checks");
    checked_trees_to_lowered_psi::lower_machine(&original, "choose")
        .expect("unmodified source reaches independent lowering");

    let mut changed = original.clone();
    let (_, selection) = changed
        .facts
        .flow
        .ownership
        .owned_selections
        .iter()
        .next()
        .expect("one existing-owner selection");
    let selection = selection.clone();
    let source_handle = selection.sources.start();
    let incoming = changed
        .facts
        .flow
        .ownership
        .selection_sources
        .get(source_handle)
        .clone();
    assert!(!incoming.origin_selection.is_valid());
    assert!(matches!(
        incoming.provenance,
        language_semantics::PermissionProvenance::Established { .. }
    ));
    let source_expression = changed
        .facts
        .values
        .structural_values
        .root_at(selection.state, incoming.statement_ordinal)
        .expect("fresh source has a retained structural producer")
        .expression;
    let bogus_prior = checked_trees::FlowOwnedSelectionReceipt {
        machine: selection.machine,
        state: selection.state,
        statement_ordinal: incoming.statement_ordinal,
        expression: source_expression,
        destination: incoming.symbol,
        type_reference: selection.type_reference,
        transfers: arena::HandleSpan::empty(),
        sources: arena::HandleSpan::empty(),
        death: language_semantics::PermissionEventSource::StateExit,
    };
    let ownership = &mut changed.facts.flow.ownership;
    let bogus_handle = ownership.owned_selections.append(bogus_prior.clone());
    ownership.owned_selections.append(bogus_prior);
    // Ambiguous lookup used to look absent at the fresh producer, while an
    // unchecked direct handle authorized Unknown at its subsequent consumer.
    assert!(
        ownership
            .owned_selection_at(selection.state, incoming.statement_ordinal)
            .is_none()
    );
    let incoming = ownership.selection_sources.get_mut(source_handle);
    incoming.provenance = language_semantics::PermissionProvenance::Unknown;
    incoming.origin_selection = bogus_handle;

    let error = checked_trees_to_lowered_psi::lower_machine(&changed, "choose")
        .expect_err("duplicate prior receipts cannot fabricate origin authority");
    assert!(
        format!("{error:?}").contains("selected ownership lost its prior selection origin"),
        "expected duplicate-receipt rejection, got {error:?}"
    );
}

#[test]
fn untouched_local_remains_observable_after_selection() {
    let source = SOURCE
        .replace("false -> right", "false -> left")
        .replace("result in Choice::Some", "right in Choice::Empty");
    for selected in [true, false] {
        let (_, execution) = execute(&source, &[TerminalScalarValue::Boolean(selected)]);
        assert_eq!(
            execution.value(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(true))
        );
    }
}

#[test]
fn borrowed_untouched_local_keeps_the_lowering_boundary_explicit() {
    let baseline = r#"
        data Choice { case Empty; case Some(value: u32); }
        machine choose(selected: bool) -> bool {
            let left: Choice = Choice::Some { value: 37 };
            let right: Choice = Choice::Empty;
            let view: &Choice = &right;
            view in Choice::Empty
        }
    "#;
    let selected = baseline.replace(
        "view in Choice::Empty",
        "let result: Choice = match selected { true -> left, false -> left };
         view in Choice::Empty",
    );
    let baseline_checked = check_source(baseline)
        .unwrap_or_else(|errors| panic!("direct borrowed-survivor source: {errors:#?}"));
    let selected_checked = check_source(&selected)
        .unwrap_or_else(|errors| panic!("selected borrowed-survivor source: {errors:#?}"));

    // Both reject at the existing borrowed-local lowering boundary. This is
    // not evidence of address-stable transport, nor a language prohibition.
    for checked in [&baseline_checked, &selected_checked] {
        let error = checked_trees_to_lowered_psi::lower_machine(checked, "choose")
            .expect_err("borrowed-local lowering remains an implementation dependency");
        assert!(
            matches!(
                error,
                checked_trees_to_lowered_psi::LoweringError::Unsupported(
                    "machine has no source-independent checked scalar control plan"
                )
            ),
            "{error:?}"
        );
    }
}
