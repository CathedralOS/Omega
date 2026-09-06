use checked_trees_to_lowered_psi::lower_machine;
use proof_admission::AdmissionProfile;
use terminal_codec::{decode_module, encode_module, encode_proof_bundle};
use terminal_fuel::{FuelChargeSite, TerminalFuelMeter};
use terminal_interpreter::{
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus, TerminalStructuralValue,
};
use terminal_psi::{OperationKind, StructuralPathSegment, Terminator};

fn checked(source: &str) -> checked_trees::CheckedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    typed_trees_to_checked_trees::lower_typed_trees(typed).unwrap()
}

#[test]
fn arbitrary_array_residuals_are_derived_from_the_type() {
    for length in [1, 5, 7, 17, 33] {
        let index = length / 2;
        let source = format!("data Token {{ number: u64; }}
        data Sink {{}} machine Sink::take(value: Token) {{}}
        data Root {{}} machine Root::enter(values: [Token; {length}]) {{ Sink::take(values[{index}]); }}");
        let residuals = (0..length)
            .rev()
            .filter(|value| *value != index)
            .map(|value| vec![StructuralPathSegment::FixedIndex(value)])
            .collect::<Vec<_>>();
        assert_source(
            &source,
            &[vec![StructuralPathSegment::FixedIndex(index)]],
            &residuals,
        );
    }
}

fn path(parts: &[&str]) -> Vec<StructuralPathSegment> {
    parts
        .iter()
        .map(|part| match part.parse::<u64>() {
            Ok(index) => StructuralPathSegment::FixedIndex(index),
            Err(_) => StructuralPathSegment::Field((*part).to_owned()),
        })
        .collect()
}

fn assert_source(
    source: &str,
    moved: &[Vec<StructuralPathSegment>],
    residuals: &[Vec<StructuralPathSegment>],
) -> lowered_psi::LoweredPsi {
    let lowered = lower_machine(&checked(source), "Root::enter")
        .expect("finite paths have exact residual cleanup");
    let module = &lowered.semantic_module;
    let caller = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let block = &caller.blocks[0];
    assert_eq!(
        block
            .operations
            .iter()
            .map(|operation| match &operation.kind {
                OperationKind::CallUnit {
                    structural_arguments,
                    ..
                } => structural_arguments[0].path.clone(),
                _ => panic!("only authored projected calls"),
            })
            .collect::<Vec<_>>(),
        moved
    );
    let cleanup = match &block.terminator {
        Terminator::ReturnUnitPartialAffine {
            residual_affine_discards,
            trivial_affine_discards,
            ..
        } => {
            assert!(trivial_affine_discards.is_empty());
            assert_eq!(
                residual_affine_discards
                    .iter()
                    .map(|discard| discard.path.clone())
                    .collect::<Vec<_>>(),
                residuals
            );
            residual_affine_discards.clone()
        }
        Terminator::ReturnUnit {
            trivial_affine_discards,
            ..
        } => {
            assert!(residuals.is_empty());
            assert!(trivial_affine_discards.is_empty());
            Vec::new()
        }
        _ => panic!("ordinary partial-affine exit"),
    };
    let semantic = encode_module(module).unwrap();
    assert_eq!(decode_module(&semantic).unwrap(), *module);
    let proof = encode_proof_bundle(&lowered.proof_bundle).unwrap();
    let verified = terminal_verifier::verify_module(
        module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .unwrap();
    let certificate =
        terminal_fixed_fuel::derive_fixed_entry_fuel(&verified, module.entry).unwrap();
    terminal_fixed_fuel::validate_fixed_entry_fuel(&verified, &certificate).unwrap();
    let fuel = 2 * moved.len() as u64 + 1;
    assert_eq!(certificate.ceiling_units(), fuel);
    let input = TerminalStructuralValue {
        opaque_identity: 123,
        structural_type: caller.structural_parameters[0].structural_type,
        qualifications: Vec::new(),
        path: Vec::new(),
    };
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[input],
    )
    .unwrap();
    let mut meter = TerminalFuelMeter::with_allowance(fuel - 1);
    let paused = execution.resume(&mut meter).unwrap();
    assert!(
        matches!(&paused, TerminalExecutionStatus::SponsorExhausted(exhaustion) if exhaustion.site == FuelChargeSite::Edge(block.terminator.edge()))
    );
    let mut live = execution
        .live_affine_frontier()
        .cloned()
        .collect::<Vec<_>>();
    let mut expected = cleanup;
    live.sort();
    expected.sort();
    assert_eq!(
        live, expected,
        "only maximal untransferred subtrees remain before cleanup"
    );
    assert_eq!(execution.resume(&mut meter).unwrap(), paused);
    assert_eq!(meter.usage().total_units(), fuel - 1);
    meter.replenish(1).unwrap();
    assert_eq!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert!(execution.live_affine_frontier().next().is_none());
    assert!(execution.live_claim_frontier().next().is_none());
    assert_eq!(meter.usage().total_units(), fuel);
    lowered
}

#[test]
fn nested_arrays_keep_untouched_branches_maximal() {
    let source = "data Token { number: u64; }
        data Sink {} machine Sink::take(value: Token) {}
        data Root {} machine Root::enter(values: [[Token; 17]; 3]) {
            Sink::take(values[1][4]); Sink::take(values[1][16]); Sink::take(values[0][3]);
        }";
    let mut residuals = vec![path(&["2"])];
    for outer in (0..2).rev() {
        for inner in (0..17).rev() {
            if (outer == 1 && (inner == 4 || inner == 16)) || (outer == 0 && inner == 3) {
                continue;
            }
            residuals.push(vec![
                StructuralPathSegment::FixedIndex(outer),
                StructuralPathSegment::FixedIndex(inner),
            ]);
        }
    }
    assert_source(
        source,
        &[path(&["1", "4"]), path(&["1", "16"]), path(&["0", "3"])],
        &residuals,
    );
}

#[test]
fn mixed_record_array_paths_share_recursive_reverse_cleanup() {
    let source = "data Token { number: u64; }
        data Row { first: Token; values: [Token; 5]; last: Token; }
        data Outer { leading: Token; rows: [Row; 3]; tail: Token; }
        data Sink {} machine Sink::take(value: Token) {}
        data Root {} machine Root::enter(values: Outer) {
            Sink::take(values.rows[1].values[3]); Sink::take(values.leading);
        }";
    let expected = [
        vec!["tail"],
        vec!["rows", "2"],
        vec!["rows", "1", "last"],
        vec!["rows", "1", "values", "4"],
        vec!["rows", "1", "values", "2"],
        vec!["rows", "1", "values", "1"],
        vec!["rows", "1", "values", "0"],
        vec!["rows", "1", "first"],
        vec!["rows", "0"],
    ]
    .iter()
    .map(|parts| path(parts))
    .collect::<Vec<_>>();
    assert_source(
        source,
        &[path(&["rows", "1", "values", "3"]), path(&["leading"])],
        &expected,
    );
}

#[test]
fn whole_subtree_and_leaf_moves_form_one_disjoint_partition() {
    let source = "data Token { number: u64; }
        data Sink {}
        machine Sink::take(value: Token) {}
        machine Sink::take_row(values: [Token; 3]) {}
        data Root {} machine Root::enter(values: [[Token; 3]; 2]) {
            Sink::take_row(values[1]); Sink::take(values[0][1]);
        }";
    assert_source(
        source,
        &[path(&["1"]), path(&["0", "1"])],
        &[path(&["0", "2"]), path(&["0", "0"])],
    );
}

#[test]
fn complete_moves_leave_no_affine_cleanup_or_whole_root_custody() {
    for (identity, declaration, paths) in [
        ("[Token; 5]", "", vec!["[3]", "[0]", "[4]", "[1]", "[2]"]),
        (
            "Outer",
            "data Outer { first: Token; scalar: u64; last: Token; }",
            vec![".last", ".first"],
        ),
    ] {
        let body = paths
            .iter()
            .map(|path| format!("Sink::take(values{path});"))
            .collect::<String>();
        let source = format!(
            "data Token {{ number: u64; }} {declaration}
            data Sink {{}} machine Sink::take(value: Token) {{}}
            data Root {{}} machine Root::enter(values: {identity}) {{ {body} }}"
        );
        let moved = paths
            .iter()
            .map(|text| path(&[text.trim_matches(['[', ']', '.'])]))
            .collect::<Vec<_>>();
        assert_source(&source, &moved, &[]);
    }
}

#[test]
fn reconstructed_residuals_reject_omissions_reordering_types_and_huge_forged_arrays() {
    let source = "data Token { number: u64; }
        data Sink {} machine Sink::take(value: Token) {}
        data Root {} machine Root::enter(values: [Token; 5]) { Sink::take(values[2]); }";
    let lowered = assert_source(
        source,
        &[path(&["2"])],
        &[path(&["4"]), path(&["3"]), path(&["1"]), path(&["0"])],
    );
    for mutation in 0..8 {
        let mut module = lowered.semantic_module.clone();
        let entry = module
            .machines
            .iter_mut()
            .find(|machine| machine.id == module.entry)
            .unwrap();
        let root_type = entry.structural_parameters[0].structural_type;
        let Terminator::ReturnUnitPartialAffine {
            residual_affine_discards,
            ..
        } = &mut entry.blocks[0].terminator
        else {
            unreachable!()
        };
        match mutation {
            0 => {
                residual_affine_discards.pop();
            }
            1 => residual_affine_discards.reverse(),
            2 => residual_affine_discards.push(residual_affine_discards[0].clone()),
            3 => residual_affine_discards[0].path = path(&["2"]),
            4 => residual_affine_discards[0].path.clear(),
            5 => residual_affine_discards[0].structural_type = root_type,
            6 => residual_affine_discards[0].path = path(&["5"]),
            7 => {
                let declaration = module
                    .structural_types
                    .iter_mut()
                    .find(|declaration| declaration.id == root_type)
                    .unwrap();
                let terminal_psi::StructuralTypeShape::FixedArray { length, .. } =
                    &mut declaration.shape
                else {
                    unreachable!()
                };
                *length = u64::MAX;
            }
            _ => unreachable!(),
        }
        assert!(
            terminal_verifier::verify_module(
                &module,
                &lowered.proof_bundle,
                &AdmissionProfile::default()
            )
            .is_err(),
            "residual mutation {mutation}"
        );
    }
}

#[test]
fn lowering_independently_reconstructs_the_checked_residual_complement() {
    let source = "data Token { number: u64; }
        data Sink {} machine Sink::take(value: Token) {}
        data Root {} machine Root::enter(values: [Token; 5]) { Sink::take(values[2]); }";
    let baseline = checked(source);
    lower_machine(&baseline, "Root::enter").expect("unaltered checked partition lowers");
    for mutation in 0..5 {
        let mut checked = baseline.clone();
        let plans = &mut checked.facts.flow.terminal_partial_affine_unit_cleanups;
        let [plan] = plans.machines.as_mut_slice() else {
            panic!("one checked partial-cleanup entry")
        };
        match mutation {
            0 => {
                plan.residual_affine_discards.pop();
            }
            1 => plan.residual_affine_discards.reverse(),
            2 => plan.residual_affine_discards[0].path.clear(),
            3 => {
                plan.residual_affine_discards[0].type_identity =
                    plan.machine.structural_parameters[0].type_identity.clone()
            }
            4 => {
                let root_identity = &plan.machine.structural_parameters[0].type_identity;
                let declaration = plans
                    .structural_types
                    .iter_mut()
                    .find(|declaration| &declaration.identity == root_identity)
                    .unwrap();
                let checked_trees::CheckedUnitStructuralTypeShape::FixedArray { length, .. } =
                    &mut declaration.shape
                else {
                    unreachable!()
                };
                *length = u64::MAX;
            }
            _ => unreachable!(),
        }
        assert!(
            lower_machine(&checked, "Root::enter").is_err(),
            "checked complement mutation {mutation} must reject before Terminal verification"
        );
    }
}
