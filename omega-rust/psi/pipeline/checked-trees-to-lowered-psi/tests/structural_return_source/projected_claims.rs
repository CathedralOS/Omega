use super::*;

pub(super) fn checked(length: usize) -> checked_trees::CheckedTrees {
    // The same customer is also run through the CLI with the bundled library.
    // The stage-local harness supplies only its imported content vocabulary.
    let source = include_str!(concat!(env!("CARGO_MANIFEST_DIR"),
        "/../../../../tests/omega/pass/effects/structural_callback_reach/projected.omg"))
        .replace("use omega::language::core::content;", "data CountedQuantity<Unit> { magnitude: u64; } trait Content<A> { machine project(subject: &Self) -> A; }")
        .replace("embed(region.length) as Nat", "region.length")
        .replace("; 2]", &format!("; {length}]"));
    let tokens = Lexer::new(&source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).expect("check")
}

#[test]
fn projected_claims_survive_nominal_mixed_call_chains_without_source() {
    for length in [1, 2, 3] {
        let checked = checked(length);
        let artifact =
            terminal_production::TerminalProductionRequest::new(&checked, "Main::demand")
                .produce_artifact()
                .expect("publish projected return custody");
        drop(checked);
        let module = decode_module(artifact.semantic_bytes()).expect("reload semantics");
        let caller = module
            .machines
            .iter()
            .find(|machine| machine.id == module.entry)
            .unwrap();
        assert_eq!(caller.entry_claims.len(), length);
        assert_eq!(caller.content_entry_claims.len(), length);
        assert_eq!(caller.content_identity_reshuffles.len(), length);
        let parameter = &caller.structural_parameters[0];
        assert!(parameter.qualifications.is_empty());
        assert_eq!(parameter.projected_qualifications.len(), length);
        for (index, claim) in caller.entry_claims.iter().enumerate() {
            let path = vec![terminal_psi::StructuralPathSegment::FixedIndex(
                index as u64,
            )];
            assert_eq!(claim.path, path);
            assert_eq!(parameter.projected_qualifications[index].path, path);
            assert_eq!(caller.content_entry_claims[index].claim, claim.claim);
            assert_eq!(
                caller.content_entry_claims[index].input.segments,
                vec![semantic_vocabulary::ContentPlaceSegment::FixedIndex(
                    index as u64
                )]
            );
        }
        let argument = TerminalStructuralValue {
            opaque_identity: 0x5eed,
            structural_type: parameter.structural_type,
            qualifications: parameter.qualifications.clone(),
            path: Vec::new(),
        };
        let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &AdmissionProfile::default(),
            &[],
            std::slice::from_ref(&argument),
        )
        .expect("start source-free artifact");
        let mut meter = TerminalFuelMeter::with_allowance(32);
        assert_eq!(
            execution.resume(&mut meter).unwrap(),
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Structural(
                TerminalStructuralResult {
                    value: argument,
                    claims: caller
                        .entry_claims
                        .iter()
                        .map(|claim| claim.claim)
                        .collect()
                }
            ))
        );
    }
}

#[test]
fn projected_claims_reject_changed_checked_custody() {
    let checked = checked(2);
    let root = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::demand")
        .unwrap()
        .symbol;
    for mutation in [
        "missing_entry",
        "entry_path",
        "missing_qualification",
        "qualification_path",
        "result_qualification",
        "missing_transfer",
        "returned_path",
        "swapped_claims",
    ] {
        let mut invalid = checked.clone();
        let plan = invalid
            .facts
            .flow
            .terminal_unit_effects
            .composed_machines
            .iter_mut()
            .find(|plan| plan.machine == root)
            .expect("shared composed plan");
        match mutation {
            "missing_entry" => {
                plan.states[0].entry_claims.pop();
            }
            "entry_path" => {
                plan.states[0].entry_claims[0].path = plan.states[0].entry_claims[1].path.clone();
            }
            "missing_qualification" => {
                plan.states[0].structural_parameters[0]
                    .projected_qualifications
                    .pop();
            }
            "qualification_path" => {
                plan.states[0].structural_parameters[0].projected_qualifications[0]
                    .path
                    .clear();
            }
            "result_qualification" => {
                let checked_trees::CheckedControlResultPlan::Structural(result) = &mut plan.result
                else {
                    panic!("structural result")
                };
                result.projected_qualifications.clear();
            }
            _ => {
                let operation = plan
                    .states
                    .iter_mut()
                    .flat_map(|state| &mut state.operations)
                    .find(|operation| {
                        matches!(
                            operation,
                            checked_trees::CheckedUnitEffectOperationPlan::StructuralCall { .. }
                        )
                    })
                    .unwrap();
                let checked_trees::CheckedUnitEffectOperationPlan::StructuralCall {
                    custody, ..
                } = operation
                else {
                    unreachable!()
                };
                match mutation {
                    "missing_transfer" => {
                        custody.claim_transfers.pop();
                    }
                    "returned_path" => {
                        custody.returned_claim_transfers[0].path =
                            custody.returned_claim_transfers[1].path.clone();
                    }
                    "swapped_claims" => {
                        let first = custody.returned_claim_transfers[0].caller_claim;
                        custody.returned_claim_transfers[0].caller_claim =
                            custody.returned_claim_transfers[1].caller_claim;
                        custody.returned_claim_transfers[1].caller_claim = first;
                    }
                    _ => unreachable!(),
                }
            }
        }
        assert!(
            terminal_production::TerminalProductionRequest::new(&invalid, "Main::demand")
                .produce_artifact()
                .is_err(),
            "changed {mutation} must reject"
        );
    }
}

#[test]
fn projected_claims_reject_incomplete_portable_return_frontiers() {
    let checked = checked(2);
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "Main::demand")
        .produce_artifact()
        .expect("publish projected returns");
    drop(checked);
    let module = decode_module(artifact.semantic_bytes()).unwrap();
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    for mutation in [
        "missing_claim",
        "duplicate_path",
        "whole_path",
        "out_of_bounds",
        "qualification",
        "content",
    ] {
        let mut invalid = module.clone();
        let machine = invalid
            .machines
            .iter_mut()
            .find(|machine| machine.id == invalid.entry)
            .unwrap();
        if mutation == "content" {
            // Root guarantees may be omitted from a weaker public contract.
            // A consumer cannot retain its guarantee after its producer loses
            // the per-path content premise on which that guarantee depends.
            invalid
                .machines
                .iter_mut()
                .find(|machine| machine.parameters.len() == 1
                    && machine.content_identity_reshuffles.len() == 2)
                .unwrap()
                .content_identity_reshuffles
                .pop();
        } else {
            let result = machine
                .blocks
                .iter_mut()
                .flat_map(|block| &mut block.operations)
                .find_map(|operation| match &mut operation.result {
                    terminal_psi::OperationResult::Structural(result) => Some(result),
                    _ => None,
                })
                .expect("ordinary structural producer");
            match mutation {
                "missing_claim" => {
                    result.claims.pop();
                }
                "duplicate_path" => {
                    result.claims[0].path = result.claims[1].path.clone();
                }
                "whole_path" => {
                    result.claims[0].path.clear();
                }
                "out_of_bounds" => {
                    result.claims[0].path =
                        vec![terminal_psi::StructuralPathSegment::FixedIndex(2)];
                }
                "qualification" => {
                    result.projected_qualifications.pop();
                }
                _ => unreachable!(),
            }
        }
        assert!(
            terminal_verifier::verify_module(&invalid, &proof, &AdmissionProfile::default())
                .is_err(),
            "portable {mutation} must reject"
        );
    }
}
