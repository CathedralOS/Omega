use super::{PARAMETER_SOURCE, PROJECTED_FIELD_SOURCE};
use checked_trees_to_lowered_psi::TerminalMachineSelection;

#[test]
fn owned_match_projected_children_reject_mutated_edge_evidence() {
    let checked = crate::front_end::checked_program_result(PROJECTED_FIELD_SOURCE).unwrap();
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
fn projected_parameter_roots_reject_mutated_residual_cleanup() {
    let checked = crate::front_end::checked_program_result(PARAMETER_SOURCE)
        .expect("parameter selection checks");
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
        let checked = crate::front_end::checked_program_result(source)
            .unwrap_or_else(|errors| panic!("checking {source}: {errors:#?}"));
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
