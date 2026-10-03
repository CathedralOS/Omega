//! A scalar helper borrows an established record without copying its view's
//! backing. Both calls retain the exact authored loan and shortened window;
//! publication must preserve that custody without a synthetic Unit caller.

use checked_trees::{CheckedScalarComputationKind, CheckedScalarComputationStructuralArgument};
use terminal_interpreter::{TerminalExecutionResult, TerminalScalarValue};
use terminal_production::{
    TerminalMachineSelection, TerminalProductionCustody, TerminalProductionRequest,
    TerminalProductionTimings,
};

const SOURCE: &str = r#"
    data Sample { readings: &[i32]; }
    machine inspect(sample: &Sample) -> bool {
        let observed: bool = inspect_values(sample.readings);
        observed
    }
    machine inspect_values(values: &[i32]) -> bool {
        transition 1 < values.len {
            true -> read(values)
            false -> (false)
        }
        state read(values: &[i32]) -> bool
            requires 1 < values.len;
        {
            values.len == 2 && values[0] == 11 && values[1] == 13
        }
    }
    machine run() -> bool {
        let backing: [i32;4] = [7, 11, 13, 17];
        let sample: Sample = Sample { readings: backing[1..3] };
        let observed: bool = inspect(&sample);
        observed
    }
"#;

fn publish(checked: &checked_trees::CheckedTrees) -> terminal_codec::CanonicalTerminalArtifact {
    TerminalProductionRequest::new(checked, TerminalMachineSelection::Name("run"))
        .produce(TerminalProductionCustody::artifact_only(
            &mut TerminalProductionTimings::default(),
        ))
        .expect("scalar calls retain the record and its borrowed window")
        .into_artifact()
}

fn assert_source_free_execution(source: &str) {
    let artifact = {
        let checked = crate::front_end::checked_program(source);
        publish(&checked)
    };
    let reloaded = terminal_codec::CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes())
        .expect("the portable artifact reloads without checked source");
    let profile = proof_admission::AdmissionProfile::default();
    terminal_codec::verify_terminal_artifact_proof(&reloaded, &profile)
        .expect("the reloaded record and view custody verify independently");
    assert_eq!(
        terminal_interpreter::interpret_terminal_artifact(
            reloaded.semantic_bytes(),
            reloaded.proof_bytes(),
            &profile,
            &[],
        )
        .unwrap_or_else(|error| panic!(
            "the retained nonzero-start window executes: {source}: {error:?}"
        )),
        TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(true)),
        "both the start offset and shortened extent survive the calls",
    );
}

#[test]
fn scalar_record_view_calls_execute_the_authored_window_without_source() {
    for source in [SOURCE.to_owned(), SOURCE.replace("i32", "u8")] {
        assert_source_free_execution(&source);
    }
}

#[test]
fn scalar_record_view_calls_narrow_the_forwarded_window_without_source() {
    let source = SOURCE.replace(
        "values.len == 2 && values[0] == 11 && values[1] == 13",
        "inspect_one(values[1..2])",
    ) + r#"
        machine inspect_one(values: &[i32]) -> bool {
            transition 0 < values.len {
                true -> read(values)
                false -> (false)
            }
            state read(values: &[i32]) -> bool requires 0 < values.len; {
                values.len == 1 && values[0] == 13
            }
        }
    "#;
    for source in [source.clone(), source.replace("i32", "u8")] {
        assert_source_free_execution(&source);
    }
}

#[test]
fn scalar_record_view_calls_forward_qualified_byte_fields_without_source() {
    let source = "domain [u8]::Utf8(bytes) requires valid_utf8(bytes);".to_owned()
        + &SOURCE
            .replace("i32", "u8")
            .replace("readings: &[u8];", "readings: &[u8] in Utf8;")
            .replace("let backing: [u8;4] = [7, 11, 13, 17];", "")
            .replace("readings: backing[1..3]", "readings: \"ab\"")
            .replace(
                "values[0] == 11 && values[1] == 13",
                "values[0] == 97 && values[1] == 98",
            );
    assert_source_free_execution(&source);
}

#[test]
fn scalar_record_view_calls_reject_substituted_field_path_and_access() {
    let original = crate::front_end::checked_program(SOURCE);
    let _artifact = publish(&original);
    let inspect = original
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "inspect")
        .expect("the authored record borrower");
    let state = original.machine_states(inspect)[0].symbol;
    let inspect_values = original
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "inspect_values")
        .expect("the authored view consumer")
        .symbol;
    let plans = &original.facts.values.scalar_computations;
    let root = plans
        .root_at(
            state,
            0,
            checked_trees::CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 0 },
        )
        .expect("inspect retains its authored observed initializer");
    let CheckedScalarComputationKind::Call {
        target_machine,
        structural_arguments: arguments,
        ..
    } = plans.nodes.get(root.root).kind
    else {
        panic!("the field loan belongs to the retained initializer call");
    };
    assert_eq!(target_machine, inspect_values);
    assert_eq!(arguments.count(), 1);
    for mutation in ["field path", "borrow access"] {
        let mut changed = original.clone();
        let CheckedScalarComputationStructuralArgument::Place(argument) = changed
            .facts
            .values
            .scalar_computations
            .structural_arguments
            .get_mut(arguments.start())
        else {
            panic!("the view is a borrowed field place");
        };
        assert_eq!(argument.path.len(), 1);
        match mutation {
            "field path" => argument.path.clear(),
            "borrow access" => {
                argument.access = checked_trees::CheckedStructuralAccess::MutableBorrow
            }
            _ => unreachable!(),
        }
        assert!(
            TerminalProductionRequest::new(&changed, TerminalMachineSelection::Name("run"))
                .produce(TerminalProductionCustody::artifact_only(
                    &mut TerminalProductionTimings::default(),
                ))
                .is_err(),
            "publication accepted a substituted {mutation}",
        );
    }
}

#[test]
fn scalar_record_view_calls_reject_coherently_substituted_element_catalogs() {
    let source = SOURCE
        .replace("readings: &[i32];", "readings: &[i32]; other: &[u64];")
        .replace(
            "let sample: Sample = Sample { readings: backing[1..3] };",
            "let other: [u64; 1] = [29];
             let sample: Sample = Sample { readings: backing[1..3], other: other[0..1] };",
        );
    let mut checked = crate::front_end::checked_program(&source);
    let _artifact = publish(&checked);
    let declarations = &checked.facts.flow.terminal_scalar_graphs.structural_types;
    let authored_element = &declarations
        .iter()
        .find(|declaration| {
            declaration.shape
                == checked_trees::CheckedUnitStructuralTypeShape::PrimitiveScalar(
                    checked_trees::types::PrimitiveType::I32,
                )
        })
        .expect("the authored view retains its signed element")
        .identity;
    let view = declarations
        .iter()
        .find(|declaration| {
            matches!(
                &declaration.shape,
                checked_trees::CheckedUnitStructuralTypeShape::BorrowedSliceView { element_type_identity }
                    if element_type_identity == authored_element
            )
        })
        .expect("the scalar callee retains its element-view declaration")
        .identity
        .clone();
    let substituted_element = declarations
        .iter()
        .find(|declaration| {
            matches!(
                declaration.shape,
                checked_trees::CheckedUnitStructuralTypeShape::PrimitiveScalar(
                    checked_trees::types::PrimitiveType::U64
                )
            )
        })
        .expect("the other view retains its distinct nonrecursive scalar element")
        .identity
        .clone();
    let flow = &mut checked.facts.flow;
    let mut changed_catalogs = 0;
    for catalog in [
        &mut flow.terminal_scalar_graphs.structural_types,
        &mut flow.terminal_unit_effects.structural_types,
        &mut flow.terminal_structural_scalar_returns.structural_types,
    ] {
        for declaration in catalog
            .iter_mut()
            .filter(|declaration| declaration.identity == view)
        {
            let checked_trees::CheckedUnitStructuralTypeShape::BorrowedSliceView {
                element_type_identity,
            } = &mut declaration.shape
            else {
                panic!("all catalogs retain the same element-view shape");
            };
            *element_type_identity = substituted_element.clone();
            changed_catalogs += 1;
        }
    }
    assert!(changed_catalogs > 0);
    // Every retained copy agrees, and the substituted identity names a real
    // nonrecursive declaration. Source reconstruction must reject the changed
    // element even though the altered catalog is internally well-founded.
    let error = crate::lower_machine(&checked, crate::TerminalMachineSelection::Name("run"))
        .expect_err("catalog agreement cannot replace the authored element declaration");
    assert!(
        matches!(
            error,
            crate::lowering_error::LoweringError::Unsupported(
                "owned structural catalog differs from its typed declaration"
            )
        ),
        "unexpected rejection: {error:?}",
    );
}

#[test]
fn a_shared_record_view_does_not_authorize_element_writes() {
    let source = SOURCE.replace(
        "values.len == 2 && values[0] == 11 && values[1] == 13",
        "values[0] = 19; true",
    );
    let diagnostics = crate::front_end::checked_program_result(&source)
        .expect_err("a shared view cannot grant mutable access to its backing");
    assert!(!diagnostics.is_empty());
}

#[test]
fn source_free_scalar_cleanup_retains_exact_live_owner_order() {
    use terminal_psi::{TerminalAffineCleanupAction, Terminator};

    // The second composition keeps another record over the same backing alive
    // until return, making reverse producer order observable to validation.
    let two_owners = SOURCE.replace(
        "let sample: Sample = Sample { readings: backing[1..3] };",
        "let sample: Sample = Sample { readings: backing[1..3] };
         let another: Sample = Sample { readings: backing[1..3] };",
    );
    for (source, owner_count) in [(SOURCE, 1), (two_owners.as_str(), 2)] {
        let artifact = {
            let checked = crate::front_end::checked_program(source);
            publish(&checked)
        };
        let original = terminal_codec::decode_module(artifact.semantic_bytes())
            .expect("decode the source-free semantic module");
        terminal_verifier::validate_module(&original)
            .expect("the original cleanup closes the live owner frontier");
        let entry = original
            .machines
            .iter()
            .position(|machine| machine.id == original.entry)
            .expect("the selected run machine");
        let block = original.machines[entry]
            .blocks
            .iter()
            .position(|block| matches!(block.terminator, Terminator::Return { .. }))
            .expect("run returns the observed scalar");
        let Terminator::Return {
            cleanup_actions, ..
        } = &original.machines[entry].blocks[block].terminator
        else {
            unreachable!();
        };
        assert_eq!(cleanup_actions.len(), owner_count);
        assert!(
            cleanup_actions
                .iter()
                .all(|action| matches!(action, TerminalAffineCleanupAction::DiscardRoot(_)))
        );
        for mutation in ["missing", "duplicate", "reordered"] {
            if mutation == "reordered" && owner_count == 1 {
                continue;
            }
            let mut changed = original.clone();
            let Terminator::Return {
                cleanup_actions, ..
            } = &mut changed.machines[entry].blocks[block].terminator
            else {
                unreachable!();
            };
            match mutation {
                "missing" => {
                    cleanup_actions.pop();
                }
                "duplicate" => {
                    cleanup_actions.push(cleanup_actions[0].clone());
                }
                "reordered" => cleanup_actions.reverse(),
                _ => unreachable!(),
            }
            assert!(
                matches!(
                    terminal_verifier::validate_module(&changed),
                    Err(terminal_verifier::ModuleError::ScalarReturnAffineDiscardsMismatch { .. })
                ),
                "source-free validation accepted {mutation} owner cleanup",
            );
        }

        let mut changed = original.clone();
        let borrower = changed
            .machines
            .iter_mut()
            .find(|machine| {
                machine.structural_parameters.iter().any(|parameter| {
                    parameter.access == terminal_psi::StructuralAccess::SharedBorrow
                }) && machine
                    .blocks
                    .iter()
                    .any(|block| matches!(block.terminator, Terminator::Return { .. }))
            })
            .expect("the retained scalar helper borrows its argument");
        let borrowed = borrower
            .structural_parameters
            .iter()
            .find(|parameter| parameter.access == terminal_psi::StructuralAccess::SharedBorrow)
            .unwrap()
            .place;
        let cleanup = borrower
            .blocks
            .iter_mut()
            .find_map(|block| match &mut block.terminator {
                Terminator::Return {
                    cleanup_actions, ..
                } => Some(cleanup_actions),
                _ => None,
            })
            .unwrap();
        cleanup.push(TerminalAffineCleanupAction::DiscardRoot(borrowed));
        assert!(
            terminal_verifier::validate_module(&changed).is_err(),
            "borrowing the record or view does not authorize discarding its owner",
        );
    }
}
