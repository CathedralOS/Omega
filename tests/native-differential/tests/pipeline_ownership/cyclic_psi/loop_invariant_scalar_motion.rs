//! Optimizer module role: test leaf. General single-entry loop-invariant scalar motion.

use super::*;

use abstract_operations_to_abstract_operations::validation::{
    validate_transformed_psi_cycle_components, validate_transformed_psi_optimization_unit,
};
use abstract_operations_to_abstract_operations::{
    LoopInvariantScalarMotionError, apply_loop_invariant_scalar_motion,
    propose_loop_invariant_scalar_motion, validate_loop_invariant_scalar_motion,
};
use optimization_unit::{
    ProvenanceDisposition, PsiProvenance, PsiRealizationSite,
    recompute_psi_optimization_unit_identity,
};

const NATURAL_LOOP_SOURCE: &str = r#"
    data Root {}

    machine Root::scan(entries: &[u8])
    terminates by entries -> Slice::Length;
    {
        transition entries.len > 0 {
            true -> scan(entries[1..])
            _ -> done()
        }
        state done() {}
    }
"#;

fn natural_loop_unit() -> (
    terminal_psi::TerminalModule,
    terminal_psi_to_abstract_operations::VerifiedPsiOptimizationUnit,
) {
    let tokens = Lexer::new(NATURAL_LOOP_SOURCE)
        .tokenize()
        .expect("tokenize natural loop");
    let syntax = parse_syntax_trees(&tokens).expect("parse natural loop");
    let resolved = lower_syntax_trees(&syntax).expect("resolve natural loop");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type natural loop");
    let checked = lower_typed_trees(typed).expect("check natural loop");
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::scan")
        .expect("lower natural loop");
    let semantic = terminal_codec::encode_module(&lowered.semantic_module)
        .expect("encode natural-loop semantics");
    let proof =
        terminal_codec::encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
            .expect("encode natural-loop proof");
    let input = lower_artifact_for_optimization(
        terminal_psi_to_abstract_operations::ArtifactSections {
            semantic_bytes: &semantic,
            proof_bytes: &proof,
            obligation_ledger_bytes: None,
        },
        &proof_admission::AdmissionProfile::default(),
    )
    .and_then(|admitted| admitted.try_into_optimization_input())
    .expect("optimizer-only natural admission");
    let verified = build_verified_psi_optimization_unit(
        input,
        terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .expect("build natural-loop optimizer unit");
    (lowered.semantic_module, verified)
}

fn preheader(session: &VerifiedPsiOptimizationSession) -> semantic_vocabulary::BlockId {
    let [component] = session.cycle_components().components() else {
        panic!("one natural loop component")
    };
    let [entry] = component.entries.as_slice() else {
        panic!("one entry edge")
    };
    entry.source
}

fn block<'unit>(
    unit: &'unit optimization_unit::PsiOptimizationUnit,
    id: semantic_vocabulary::BlockId,
) -> &'unit optimization_unit::OptimizationBlock {
    unit.functions
        .iter()
        .flat_map(|function| &function.blocks)
        .find(|candidate| candidate.id == id)
        .expect("block exists")
}

fn take_operation(
    unit: &mut optimization_unit::PsiOptimizationUnit,
    operation: semantic_vocabulary::OperationId,
) -> optimization_unit::OptimizationNode {
    for function in &mut unit.functions {
        for block in &mut function.blocks {
            if let Some(index) = block.nodes.iter().position(|node| {
                node.provenance.first() == Some(&PsiProvenance::Operation(operation))
            }) {
                return block.nodes.remove(index);
            }
        }
    }
    panic!("operation {operation:?} exists")
}

fn find_operation_mut(
    unit: &mut optimization_unit::PsiOptimizationUnit,
    operation: semantic_vocabulary::OperationId,
) -> &mut optimization_unit::OptimizationNode {
    unit.functions
        .iter_mut()
        .flat_map(|function| &mut function.blocks)
        .flat_map(|block| &mut block.nodes)
        .find(|node| node.provenance.first() == Some(&PsiProvenance::Operation(operation)))
        .expect("operation exists")
}

fn refresh_coordinates_and_effects(unit: &mut optimization_unit::PsiOptimizationUnit) {
    for function in &mut unit.functions {
        let mut effect = 0u64;
        for block in &mut function.blocks {
            for (node_index, node) in block.nodes.iter_mut().enumerate() {
                let node_index = u32::try_from(node_index).expect("test fixture fits u32");
                for definition in &mut node.definitions {
                    definition.site = optimization_unit::ValueDefinitionSite::Node {
                        block: block.id,
                        node: node_index,
                    };
                }
                for value_use in &mut node.uses {
                    value_use.block = block.id;
                    value_use.node = node_index;
                }
                node.effect = optimization_unit::EffectLink {
                    input: effect,
                    output: effect + 1,
                };
                effect += 1;
            }
        }
        let operation_order = function
            .blocks
            .iter()
            .flat_map(|block| &block.nodes)
            .enumerate()
            .filter_map(|(position, node)| match node.provenance.first() {
                Some(PsiProvenance::Operation(operation)) => Some((*operation, position)),
                _ => None,
            })
            .collect::<std::collections::BTreeMap<_, _>>();
        function.facts.sort_by_key(|fact| {
            let support = match fact {
                optimization_unit::OptimizationFact::OperationObligationReference {
                    support,
                    ..
                }
                | optimization_unit::OptimizationFact::BooleanConstant { support, .. }
                | optimization_unit::OptimizationFact::IntegerConstant { support, .. } => support,
            };
            operation_order.get(support).copied()
        });
    }
    unit.identity = recompute_psi_optimization_unit_identity(unit);
}

fn invariant_leaves(
    session: &VerifiedPsiOptimizationSession,
) -> Vec<semantic_vocabulary::OperationId> {
    let [component] = session.cycle_components().components() else {
        panic!("one natural loop component")
    };
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == component.id.machine)
        .expect("component machine exists");
    let mut leaves = Vec::new();
    for member in &component.members {
        let block = function
            .blocks
            .iter()
            .find(|block| block.id == *member)
            .expect("member exists");
        for node in &block.nodes {
            if matches!(
                node.operation,
                AbstractOperation::IntegerConstant { .. }
                    | AbstractOperation::BooleanConstant { .. }
                    | AbstractOperation::IeeeFloatConstant { .. }
            ) {
                let Some(PsiProvenance::Operation(operation)) = node.provenance.first() else {
                    panic!("constant leaf carries its operation identity")
                };
                leaves.push(*operation);
            }
        }
    }
    leaves
}

#[test]
fn natural_loop_hoists_every_invariant_scalar_leaf_and_ledgers_source_custody() {
    let (_, verified) = natural_loop_unit();
    let session = VerifiedPsiOptimizationSession::new(verified).expect("verified natural loop");
    let leaves = invariant_leaves(&session);
    assert_eq!(leaves.len(), 2);
    let preheader = preheader(&session);
    let input = session.unit().identity;
    let candidates =
        propose_loop_invariant_scalar_motion(&session, 1).expect("one exact relocation candidate");
    let [candidate] = candidates.as_slice() else {
        panic!("one natural loop component yields one atomic candidate")
    };
    assert_eq!(candidate.input(), input);
    assert_eq!(candidate.relocations().len(), 2);
    for relocation in candidate.relocations() {
        assert_eq!(relocation.destination().block, preheader);
        assert_eq!(relocation.leaf().location().block, {
            let [component] = session.cycle_components().components() else {
                panic!("one component")
            };
            assert!(
                component
                    .members
                    .contains(&relocation.leaf().location().block)
            );
            relocation.leaf().location().block
        });
    }
    let validated = validate_loop_invariant_scalar_motion(&session, candidate)
        .expect("independent relocation validation");
    let applied = apply_loop_invariant_scalar_motion(session, validated)
        .expect("atomic relocation application");

    assert_eq!(applied.candidate(), candidate);
    assert_eq!(applied.session().unit().identity, candidate.output());
    let [record] = applied.ledger().records() else {
        panic!("one atomic relocation has one ledger record")
    };
    assert_eq!(record.input, candidate.input());
    assert_eq!(record.output, candidate.output());
    assert_eq!(record.candidate, candidate.identity());
    assert!(record.pruned_machines.is_empty());
    for relocation in candidate.relocations() {
        let row = record
            .provenance
            .iter()
            .find(|row| row.input == PsiRealizationSite::Node(relocation.leaf().location()))
            .expect("every moved leaf has exact ledger custody");
        assert_eq!(
            row.disposition,
            ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(relocation.destination()))
        );
        assert_eq!(&row.sources, relocation.leaf().provenance());
        assert_eq!(&row.fuel, relocation.leaf().fuel());
    }
    for relocation in candidate.relocations() {
        let destination = block(applied.session().unit(), relocation.destination().block);
        let node = &destination.nodes[usize::try_from(relocation.destination().node).unwrap()];
        assert_eq!(
            node.provenance.first(),
            Some(&PsiProvenance::Operation(relocation.leaf().psi_operation()))
        );
    }
    assert!(
        propose_loop_invariant_scalar_motion(applied.session(), 1)
            .expect("relocated session is an exact fixed point")
            .is_empty()
    );
}

#[test]
fn natural_loop_motion_is_deterministic_and_budget_failure_publishes_nothing() {
    let (_, first_verified) = natural_loop_unit();
    let first =
        VerifiedPsiOptimizationSession::new(first_verified).expect("first verified natural loop");
    assert_eq!(
        propose_loop_invariant_scalar_motion(&first, 0),
        Err(LoopInvariantScalarMotionError::CandidateBudgetExhausted {
            required: 1,
            limit: 0,
        })
    );
    let after_failure = propose_loop_invariant_scalar_motion(&first, 1)
        .expect("budget failure leaves the immutable session untouched");

    let (_, second_verified) = natural_loop_unit();
    let second =
        VerifiedPsiOptimizationSession::new(second_verified).expect("second verified natural loop");
    let repeated = propose_loop_invariant_scalar_motion(&second, 1).expect("repeat exact proposal");
    assert_eq!(after_failure, repeated);
}

#[test]
fn natural_loop_stale_candidate_cannot_cross_a_successful_revision() {
    let (_, verified) = natural_loop_unit();
    let session = VerifiedPsiOptimizationSession::new(verified).expect("verified natural loop");
    let candidate = propose_loop_invariant_scalar_motion(&session, 1)
        .expect("exact candidate")
        .pop()
        .expect("one candidate");
    let validated = validate_loop_invariant_scalar_motion(&session, &candidate)
        .expect("validated exact candidate");
    let applied =
        apply_loop_invariant_scalar_motion(session, validated).expect("applied exact candidate");
    assert!(matches!(
        validate_loop_invariant_scalar_motion(applied.session(), &candidate),
        Err(LoopInvariantScalarMotionError::StaleCandidateRevision {
            candidate: stale,
            current,
        }) if stale == candidate.input() && current == applied.session().unit().identity
    ));
}

#[test]
fn natural_loop_partial_relocation_normalizes_without_duplicate_ledger_rows() {
    let (_, verified) = natural_loop_unit();
    let session = VerifiedPsiOptimizationSession::new(verified).expect("verified natural loop");
    let leaves = invariant_leaves(&session);
    let preheader = preheader(&session);
    let (input, mut unit) = session.into_parts();
    let moved = take_operation(&mut unit, leaves[0]);
    let preheader_block = unit
        .functions
        .iter_mut()
        .flat_map(|function| &mut function.blocks)
        .find(|candidate| candidate.id == preheader)
        .expect("preheader exists");
    let terminator = preheader_block.nodes.len() - 1;
    preheader_block.nodes.insert(terminator, moved);
    refresh_coordinates_and_effects(&mut unit);

    let session = VerifiedPsiOptimizationSession::from_transformed(input, unit)
        .expect("partial relocation preserves natural-loop custody");
    let candidate = propose_loop_invariant_scalar_motion(&session, 1)
        .expect("remaining exact normalization candidate")
        .pop()
        .expect("partial relocation is not yet a fixed point");
    assert_eq!(candidate.relocations().len(), 1);
    let validated = validate_loop_invariant_scalar_motion(&session, &candidate)
        .expect("partial relocation candidate validates independently");
    let applied = apply_loop_invariant_scalar_motion(session, validated)
        .expect("partial relocation normalizes atomically");
    let rows = &applied.ledger().records()[0].provenance;
    assert!(!rows.is_empty());
    assert!(rows.windows(2).all(|pair| pair[0].input < pair[1].input));
    assert!(
        propose_loop_invariant_scalar_motion(applied.session(), 1)
            .expect("normalized relocation is a fixed point")
            .is_empty()
    );
}

#[test]
fn natural_loop_relocation_does_not_thaw_any_other_component_node() {
    let (_, verified) = natural_loop_unit();
    let session = VerifiedPsiOptimizationSession::new(verified).expect("verified natural loop");
    let candidate = propose_loop_invariant_scalar_motion(&session, 1)
        .expect("exact candidate")
        .pop()
        .expect("one candidate");
    let validated = validate_loop_invariant_scalar_motion(&session, &candidate)
        .expect("validated exact candidate");
    let applied =
        apply_loop_invariant_scalar_motion(session, validated).expect("applied exact candidate");
    let (input, mut unit) = applied.into_session().into_parts();

    // A retained member node stays frozen: mutating its fuel still reaches
    // the unchanged frozen-block fence at its own block. Component custody
    // recomputes from the transformed unit through the public validator.
    let components = validate_transformed_psi_cycle_components(&input, &unit)
        .expect("relocated unit retains exact cycle custody");
    let [component] = components.components() else {
        panic!("one component")
    };
    let member = component.members[0];
    let machine = component.id.machine;
    let retained = block(&unit, member)
        .nodes
        .iter()
        .find(|node| matches!(node.operation, AbstractOperation::IntegerLessThan { .. }))
        .expect("guard comparison remains in the loop")
        .provenance
        .first()
        .copied()
        .expect("comparison carries its operation identity");
    let operation = match retained {
        PsiProvenance::Operation(operation) => operation,
        _ => panic!("comparison carries its operation identity"),
    };
    find_operation_mut(&mut unit, operation).fuel[0].units += 1;
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    assert!(matches!(
        validate_transformed_psi_optimization_unit(&input, &unit),
        Err(
            OptimizationUnitValidationError::RankedCycleFrozenBlockMismatch {
                machine: mutated_machine,
                block
            }
        ) if mutated_machine == machine && block == member
    ));
}
