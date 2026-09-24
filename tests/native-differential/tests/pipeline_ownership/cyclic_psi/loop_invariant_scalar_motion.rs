//! Optimizer module role: test leaf. General single-entry loop-invariant scalar motion.

use super::{
    AbstractOperation, OptimizationUnitValidationError, VerifiedPsiOptimizationSession,
    build_verified_psi_optimization_unit, lower_artifact,
};
use abstract_operations_to_abstract_operations::test_support::{
    LoopInvariantScalarMotionError, apply_loop_invariant_scalar_motion,
    propose_loop_invariant_scalar_motion, validate_loop_invariant_scalar_motion,
};
use abstract_operations_to_abstract_operations::validation::{
    validate_transformed_psi_cycle_components, validate_transformed_psi_optimization_unit,
};
use checked_trees_to_lowered_psi::TerminalMachineSelection;
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

/// `scale` is an invariant loop parameter: the entry edge binds it from the
/// function parameter and the recursive edge binds it to itself, while the
/// `entries` measure still descends. `scale + scale` is therefore a
/// side-effect-free scalar computation the loop recomputes identically every
/// iteration — the non-constant family this boundary now relocates.
const INVARIANT_COMPUTATION_SOURCE: &str = r#"
    data Root {}

    machine Root::scan(scale: u64 in Wrapping, entries: &[u8])
    terminates by entries -> Slice::Length;
    {
        let doubled: u64 in Wrapping = scale + scale;
        transition entries.len > 0 {
            true -> scan(scale, entries[1..])
            _ -> done(doubled)
        }
        state done(r: u64 in Wrapping) {}
    }
"#;

/// `doubled` is invariant through the `scale` parameter and `quadrupled`
/// chains on it: its only member-internal operand is `doubled`'s result, so
/// the same atomic run relocates both computations and keeps the producer
/// ahead of the consumer.
const CHAINED_COMPUTATION_SOURCE: &str = r#"
    data Root {}

    machine Root::scan(scale: u64 in Wrapping, entries: &[u8])
    terminates by entries -> Slice::Length;
    {
        let doubled: u64 in Wrapping = scale + scale;
        let quadrupled: u64 in Wrapping = doubled + doubled;
        transition entries.len > 0 {
            true -> scan(scale, entries[1..])
            _ -> done(quadrupled)
        }
        state done(r: u64 in Wrapping) {}
    }
"#;

/// Same computation shape, but the recursive edge advances `scale`, so the
/// loop parameter is genuinely loop-carried and `scale + scale` must stay
/// inside the component.
const VARIANT_COMPUTATION_SOURCE: &str = r#"
    data Root {}

    machine Root::scan(scale: u64 in Wrapping, entries: &[u8])
    terminates by entries -> Slice::Length;
    {
        let doubled: u64 in Wrapping = scale + scale;
        transition entries.len > 0 {
            true -> scan(scale + 1, entries[1..])
            _ -> done(doubled)
        }
        state done(r: u64 in Wrapping) {}
    }
"#;

fn lowered_unit(
    source: &str,
    label: &str,
) -> (
    terminal_psi::TerminalModule,
    terminal_psi_to_abstract_operations::VerifiedPsiOptimizationUnit,
) {
    let checked = crate::front_end::checked_program(source);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::scan"),
    )
    .unwrap_or_else(|error| panic!("lower {label}: {error:?}"));
    let semantic = terminal_codec::encode_module(&lowered.semantic_module)
        .unwrap_or_else(|error| panic!("encode {label} semantics: {error:?}"));
    let proof =
        terminal_codec::encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
            .unwrap_or_else(|error| panic!("encode {label} proof: {error:?}"));
    let input = lower_artifact(
        terminal_psi_to_abstract_operations::ArtifactSections {
            semantic_bytes: &semantic,
            proof_bytes: &proof,
            obligation_ledger_bytes: None,
        },
        &proof_admission::AdmissionProfile::default(),
    )
    .map(|admitted| {
        admitted
            .into_optimization_artifact()
            .into_optimization_input()
    })
    .unwrap_or_else(|error| panic!("optimizer-only {label} admission: {error:?}"));
    let verified = build_verified_psi_optimization_unit(
        input,
        terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .unwrap_or_else(|error| panic!("build {label} optimizer unit: {error:?}"));
    (lowered.semantic_module, verified)
}

fn natural_loop_unit() -> (
    terminal_psi::TerminalModule,
    terminal_psi_to_abstract_operations::VerifiedPsiOptimizationUnit,
) {
    lowered_unit(NATURAL_LOOP_SOURCE, "natural loop")
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

fn block(
    unit: &optimization_unit::PsiOptimizationUnit,
    id: semantic_vocabulary::BlockId,
) -> &optimization_unit::OptimizationBlock {
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
        assert_eq!(relocation.node().location().block, {
            let [component] = session.cycle_components().components() else {
                panic!("one component")
            };
            assert!(
                component
                    .members
                    .contains(&relocation.node().location().block)
            );
            relocation.node().location().block
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
            .find(|row| row.input == PsiRealizationSite::Node(relocation.node().location()))
            .expect("every moved leaf has exact ledger custody");
        assert_eq!(
            row.disposition,
            ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(relocation.destination()))
        );
        assert_eq!(&row.sources, relocation.node().provenance());
        assert_eq!(&row.fuel, relocation.node().fuel());
    }
    for relocation in candidate.relocations() {
        let destination = block(applied.session().unit(), relocation.destination().block);
        let node = &destination.nodes[usize::try_from(relocation.destination().node).unwrap()];
        assert_eq!(
            node.provenance.first(),
            Some(&PsiProvenance::Operation(relocation.node().psi_operation()))
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

#[test]
fn invariant_computation_hoists_rebinding_entry_parameter_to_its_representative() {
    let (_, verified) = lowered_unit(INVARIANT_COMPUTATION_SOURCE, "invariant computation loop");
    let session =
        VerifiedPsiOptimizationSession::new(verified).expect("verified invariant-computation loop");
    let [component] = session.cycle_components().components() else {
        panic!("one natural loop component")
    };
    let [entry] = component.entries.as_slice() else {
        panic!("one entry edge")
    };
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == component.id.machine)
        .expect("component machine exists");
    let [function_parameter] = function.parameters.as_slice() else {
        panic!("one scalar function parameter")
    };
    let function_parameter = function_parameter.value;
    let header = function
        .blocks
        .iter()
        .find(|block| block.id == entry.target)
        .expect("entry target exists");
    let [header_parameter] = header.parameters.as_slice() else {
        panic!("one scalar header parameter")
    };
    let header_parameter = header_parameter.value;
    let preheader = entry.source;
    let addition = component
        .members
        .iter()
        .flat_map(|member| {
            function
                .blocks
                .iter()
                .filter(move |block| block.id == *member)
                .flat_map(|block| &block.nodes)
        })
        .find(|node| matches!(node.operation, AbstractOperation::WrappingIntegerAdd { .. }))
        .expect("invariant computation lives in the loop");
    let addition_operation = match addition.provenance.first() {
        Some(PsiProvenance::Operation(operation)) => *operation,
        _ => panic!("computation carries its operation identity"),
    };

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 1).expect("one exact relocation candidate");
    let [candidate] = candidates.as_slice() else {
        panic!("one component yields one atomic candidate")
    };
    let relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == addition_operation)
        .expect("invariant computation is a planned relocation");
    assert_eq!(
        relocation.node().operand_rewrites(),
        &[(header_parameter, function_parameter)],
    );
    assert_eq!(relocation.destination().block, preheader);
    // The scalar-constant leaves relocate alongside it in the same atomic plan.
    assert!(candidate.relocations().len() >= 3);

    let validated = validate_loop_invariant_scalar_motion(&session, candidate)
        .expect("independent relocation validation");
    let applied = apply_loop_invariant_scalar_motion(session, validated)
        .expect("atomic relocation application");
    assert_eq!(applied.session().unit().identity, candidate.output());

    let destination = block(applied.session().unit(), relocation.destination().block);
    let moved = &destination.nodes[usize::try_from(relocation.destination().node).unwrap()];
    match &moved.operation {
        AbstractOperation::WrappingIntegerAdd {
            result,
            left,
            right,
            ..
        } => {
            assert_eq!(Some(*result), relocation.node().result().scalar_value());
            assert_eq!(*left, function_parameter);
            assert_eq!(*right, function_parameter);
        }
        operation => panic!("relocated computation keeps its operation: {operation:?}"),
    }
    assert_eq!(moved.provenance, relocation.node().provenance());
    assert_eq!(moved.fuel, relocation.node().fuel());
    assert!(
        moved
            .uses
            .iter()
            .all(|value_use| value_use.value == function_parameter)
    );

    let [record] = applied.ledger().records() else {
        panic!("one atomic relocation has one ledger record")
    };
    let row = record
        .provenance
        .iter()
        .find(|row| row.input == PsiRealizationSite::Node(relocation.node().location()))
        .expect("moved computation has exact ledger custody");
    assert_eq!(
        row.disposition,
        ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(relocation.destination()))
    );
    assert_eq!(&row.sources, relocation.node().provenance());
    assert_eq!(&row.fuel, relocation.node().fuel());

    // Every moved node is now outside the member roster; the loop-carried
    // comparison and slice measure stay inside, so the plan is a fixed point.
    assert!(
        propose_loop_invariant_scalar_motion(applied.session(), 1)
            .expect("relocated session is an exact fixed point")
            .is_empty()
    );
}

#[test]
fn loop_carried_computation_is_not_relocated() {
    let (_, verified) = lowered_unit(VARIANT_COMPUTATION_SOURCE, "variant computation loop");
    let session =
        VerifiedPsiOptimizationSession::new(verified).expect("verified variant-computation loop");
    // Scalar-constant leaves still relocate; the loop-carried `scale + scale`
    // and `scale + 1` computations must not.
    let candidates =
        propose_loop_invariant_scalar_motion(&session, 1).expect("exact relocation candidates");
    let [candidate] = candidates.as_slice() else {
        panic!("one component yields one atomic candidate")
    };
    for relocation in candidate.relocations() {
        assert!(
            relocation.node().operand_rewrites().is_empty(),
            "a variant computation must never be planned"
        );
    }
    let validated = validate_loop_invariant_scalar_motion(&session, candidate)
        .expect("independent relocation validation");
    let applied = apply_loop_invariant_scalar_motion(session, validated)
        .expect("atomic relocation application");
    let [component] = applied.session().cycle_components().components() else {
        panic!("one natural loop component")
    };
    let additions_inside = component
        .members
        .iter()
        .flat_map(|member| {
            applied
                .session()
                .unit()
                .functions
                .iter()
                .flat_map(|function| &function.blocks)
                .filter(move |block| block.id == *member)
                .flat_map(|block| &block.nodes)
        })
        .filter(|node| matches!(node.operation, AbstractOperation::WrappingIntegerAdd { .. }))
        .count();
    assert!(
        additions_inside > 0,
        "the loop-carried computation remains inside the component"
    );
}

#[test]
fn relocating_a_variant_computation_is_rejected_by_the_freeze_fence() {
    let (_, verified) = lowered_unit(INVARIANT_COMPUTATION_SOURCE, "invariant computation loop");
    let session =
        VerifiedPsiOptimizationSession::new(verified).expect("verified invariant-computation loop");
    let [component] = session.cycle_components().components() else {
        panic!("one natural loop component")
    };
    let [entry] = component.entries.as_slice() else {
        panic!("one entry edge")
    };
    let machine = component.id.machine;
    let member = component.members[0];
    let preheader = entry.source;
    let (input, mut unit) = session.into_parts();
    let comparison = block(&unit, member)
        .nodes
        .iter()
        .find(|node| matches!(node.operation, AbstractOperation::IntegerLessThan { .. }))
        .expect("loop-carried comparison exists")
        .provenance
        .first()
        .copied()
        .expect("comparison carries its operation identity");
    let operation = match comparison {
        PsiProvenance::Operation(operation) => operation,
        _ => panic!("comparison carries its operation identity"),
    };
    // Hand-move a computation whose operands are defined inside the component:
    // the relocation fence must reject it because no invariant substitution
    // exists, not merely because the shape differs.
    let moved = take_operation(&mut unit, operation);
    let preheader_block = unit
        .functions
        .iter_mut()
        .flat_map(|function| &mut function.blocks)
        .find(|candidate| candidate.id == preheader)
        .expect("preheader exists");
    let terminator = preheader_block.nodes.len() - 1;
    preheader_block.nodes.insert(terminator, moved);
    refresh_coordinates_and_effects(&mut unit);
    assert!(matches!(
        VerifiedPsiOptimizationSession::from_transformed(input, unit),
        Err(
            OptimizationUnitValidationError::RankedCycleFrozenBlockMismatch {
                machine: rejected_machine,
                block
            }
        ) if rejected_machine == machine && block == member
    ));
}

#[test]
fn chained_invariant_computations_relocate_together_in_def_order() {
    let (_, verified) = lowered_unit(CHAINED_COMPUTATION_SOURCE, "chained computation loop");
    let session =
        VerifiedPsiOptimizationSession::new(verified).expect("verified chained-computation loop");
    let [component] = session.cycle_components().components() else {
        panic!("one natural loop component")
    };
    let [entry] = component.entries.as_slice() else {
        panic!("one entry edge")
    };
    let machine = component.id.machine;
    let member = component.members[0];
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("component machine exists");
    let [function_parameter] = function.parameters.as_slice() else {
        panic!("one scalar function parameter")
    };
    let function_parameter = function_parameter.value;
    let header = function
        .blocks
        .iter()
        .find(|block| block.id == entry.target)
        .expect("entry target exists");
    let [header_parameter] = header.parameters.as_slice() else {
        panic!("one scalar header parameter")
    };
    let header_parameter = header_parameter.value;
    let preheader = entry.source;
    let member_block = function
        .blocks
        .iter()
        .find(|block| block.id == member)
        .expect("member block exists");
    let doubled = member_block
        .nodes
        .iter()
        .find(|node| {
            matches!(
                &node.operation,
                AbstractOperation::WrappingIntegerAdd { left, right, .. }
                    if *left == header_parameter && *right == header_parameter
            )
        })
        .expect("`scale + scale` lives in the loop");
    let doubled_result = doubled.definitions[0].value;
    let doubled_operation = match doubled.provenance.first() {
        Some(PsiProvenance::Operation(operation)) => *operation,
        _ => panic!("computation carries its operation identity"),
    };
    let quadrupled = member_block
        .nodes
        .iter()
        .find(|node| {
            matches!(
                &node.operation,
                AbstractOperation::WrappingIntegerAdd { left, right, .. }
                    if *left == doubled_result && *right == doubled_result
            )
        })
        .expect("`doubled + doubled` lives in the loop");
    let quadrupled_operation = match quadrupled.provenance.first() {
        Some(PsiProvenance::Operation(operation)) => *operation,
        _ => panic!("computation carries its operation identity"),
    };

    let candidates =
        propose_loop_invariant_scalar_motion(&session, 1).expect("one exact relocation candidate");
    let [candidate] = candidates.as_slice() else {
        panic!("one component yields one atomic candidate")
    };
    let doubled_relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == doubled_operation)
        .expect("the invariant producer is a planned relocation");
    let quadrupled_relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == quadrupled_operation)
        .expect("the chained consumer is a planned relocation");
    assert_eq!(
        doubled_relocation.node().operand_rewrites(),
        &[(header_parameter, function_parameter)],
    );
    // The chained operand needs no rewrite: the run preserves the producer's
    // result identity, so only the run order must keep the producer ahead of
    // the consumer.
    assert_eq!(quadrupled_relocation.node().operand_rewrites(), &[]);
    assert_eq!(doubled_relocation.destination().block, preheader);
    assert_eq!(quadrupled_relocation.destination().block, preheader);
    assert!(
        quadrupled_relocation.destination().node > doubled_relocation.destination().node,
        "the consumer lands strictly after its relocated producer"
    );

    let validated = validate_loop_invariant_scalar_motion(&session, candidate)
        .expect("independent relocation validation");
    let applied = apply_loop_invariant_scalar_motion(session, validated)
        .expect("atomic relocation application");
    assert_eq!(applied.session().unit().identity, candidate.output());

    let destination = block(
        applied.session().unit(),
        quadrupled_relocation.destination().block,
    );
    let moved =
        &destination.nodes[usize::try_from(quadrupled_relocation.destination().node).unwrap()];
    match &moved.operation {
        AbstractOperation::WrappingIntegerAdd { left, right, .. } => {
            assert_eq!(*left, doubled_result);
            assert_eq!(*right, doubled_result);
        }
        operation => panic!("relocated consumer keeps its operation: {operation:?}"),
    }
    assert!(
        moved
            .uses
            .iter()
            .all(|value_use| value_use.value == doubled_result)
    );
    assert_eq!(moved.provenance, quadrupled_relocation.node().provenance());
    assert_eq!(moved.fuel, quadrupled_relocation.node().fuel());

    let [record] = applied.ledger().records() else {
        panic!("one atomic relocation has one ledger record")
    };
    for relocation in [doubled_relocation, quadrupled_relocation] {
        let row = record
            .provenance
            .iter()
            .find(|row| row.input == PsiRealizationSite::Node(relocation.node().location()))
            .expect("every chained relocation has exact ledger custody");
        assert_eq!(
            row.disposition,
            ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(relocation.destination()))
        );
        assert_eq!(&row.sources, relocation.node().provenance());
        assert_eq!(&row.fuel, relocation.node().fuel());
    }
    assert!(
        propose_loop_invariant_scalar_motion(applied.session(), 1)
            .expect("relocated session is an exact fixed point")
            .is_empty()
    );
}

#[test]
fn chained_consumer_whose_producer_stays_inside_is_rejected_by_the_freeze_fence() {
    let (_, verified) = lowered_unit(CHAINED_COMPUTATION_SOURCE, "chained computation loop");
    let session =
        VerifiedPsiOptimizationSession::new(verified).expect("verified chained-computation loop");
    let [component] = session.cycle_components().components() else {
        panic!("one natural loop component")
    };
    let [entry] = component.entries.as_slice() else {
        panic!("one entry edge")
    };
    let machine = component.id.machine;
    let member = component.members[0];
    let preheader = entry.source;
    let (input, mut unit) = session.into_parts();
    // The second `WrappingIntegerAdd` in the member block is `doubled +
    // doubled`: hand-move it into the preheader while its producer stays
    // inside. The member-internal operand names a result that never
    // relocated, so the fence's seed-derived substitution cannot exist and
    // the custody break rejects before core use/def validation runs.
    let member_block = block(&unit, member);
    let chained = member_block
        .nodes
        .iter()
        .filter(|node| matches!(node.operation, AbstractOperation::WrappingIntegerAdd { .. }))
        .nth(1)
        .expect("the chained consumer lives in the loop")
        .provenance
        .first()
        .copied()
        .expect("computation carries its operation identity");
    let operation = match chained {
        PsiProvenance::Operation(operation) => operation,
        _ => panic!("computation carries its operation identity"),
    };
    let moved = take_operation(&mut unit, operation);
    let preheader_block = unit
        .functions
        .iter_mut()
        .flat_map(|function| &mut function.blocks)
        .find(|candidate| candidate.id == preheader)
        .expect("preheader exists");
    let terminator = preheader_block.nodes.len() - 1;
    preheader_block.nodes.insert(terminator, moved);
    refresh_coordinates_and_effects(&mut unit);
    assert!(matches!(
        VerifiedPsiOptimizationSession::from_transformed(input, unit),
        Err(
            OptimizationUnitValidationError::RankedCycleFrozenBlockMismatch {
                machine: rejected_machine,
                block
            }
        ) if rejected_machine == machine && block == member
    ));
}

#[test]
fn a_relocated_run_keeps_producers_before_consumers() {
    let (_, verified) = lowered_unit(CHAINED_COMPUTATION_SOURCE, "chained computation loop");
    let session =
        VerifiedPsiOptimizationSession::new(verified).expect("verified chained-computation loop");
    let [component] = session.cycle_components().components() else {
        panic!("one natural loop component")
    };
    let [entry] = component.entries.as_slice() else {
        panic!("one entry edge")
    };
    let machine = component.id.machine;
    let preheader = entry.source;
    let function = session
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("component machine exists");
    let header = function
        .blocks
        .iter()
        .find(|block| block.id == entry.target)
        .expect("entry target exists");
    let [header_parameter] = header.parameters.as_slice() else {
        panic!("one scalar header parameter")
    };
    let header_parameter = header_parameter.value;
    let member_block = function
        .blocks
        .iter()
        .find(|block| block.id == component.members[0])
        .expect("member block exists");
    let doubled = member_block
        .nodes
        .iter()
        .find(|node| {
            matches!(
                &node.operation,
                AbstractOperation::WrappingIntegerAdd { left, right, .. }
                    if *left == header_parameter && *right == header_parameter
            )
        })
        .expect("`scale + scale` lives in the loop");
    let doubled_result = doubled.definitions[0].value;
    let doubled_operation = match doubled.provenance.first() {
        Some(PsiProvenance::Operation(operation)) => *operation,
        _ => panic!("computation carries its operation identity"),
    };
    let quadrupled_operation = match member_block
        .nodes
        .iter()
        .find(|node| {
            matches!(
                &node.operation,
                AbstractOperation::WrappingIntegerAdd { left, right, .. }
                    if *left == doubled_result && *right == doubled_result
            )
        })
        .expect("`doubled + doubled` lives in the loop")
        .provenance
        .first()
    {
        Some(PsiProvenance::Operation(operation)) => *operation,
        _ => panic!("computation carries its operation identity"),
    };

    let candidate = propose_loop_invariant_scalar_motion(&session, 1)
        .expect("exact candidate")
        .pop()
        .expect("one candidate");
    let producer = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == doubled_operation)
        .map(|relocation| usize::try_from(relocation.destination().node).unwrap())
        .expect("the invariant producer is a planned relocation");
    let consumer = candidate
        .relocations()
        .iter()
        .find(|relocation| relocation.node().psi_operation() == quadrupled_operation)
        .map(|relocation| usize::try_from(relocation.destination().node).unwrap())
        .expect("the chained consumer is a planned relocation");
    assert!(consumer > producer);
    let validated = validate_loop_invariant_scalar_motion(&session, &candidate)
        .expect("validated exact candidate");
    let applied =
        apply_loop_invariant_scalar_motion(session, validated).expect("applied exact candidate");
    let (input, mut unit) = applied.into_session().into_parts();

    // Swap the chained pair inside the preheader run. The freeze fence still
    // sees an admitted relocation shape — every moved node retains its
    // source-owned fields — so the def-before-use order itself must be
    // enforced by core validation.
    let preheader_block = unit
        .functions
        .iter_mut()
        .flat_map(|function| &mut function.blocks)
        .find(|candidate| candidate.id == preheader)
        .expect("preheader exists");
    preheader_block.nodes.swap(producer, consumer);
    refresh_coordinates_and_effects(&mut unit);
    assert!(matches!(
        validate_transformed_psi_optimization_unit(&input, &unit),
        Err(OptimizationUnitValidationError::UseBeforeDefinition {
            machine: rejected_machine,
            block,
            value
        }) if rejected_machine == machine && block == preheader && value == doubled_result
    ));
}

#[test]
fn forged_operand_rewrite_is_rejected_by_the_freeze_fence() {
    let (_, verified) = lowered_unit(INVARIANT_COMPUTATION_SOURCE, "invariant computation loop");
    let session =
        VerifiedPsiOptimizationSession::new(verified).expect("verified invariant-computation loop");
    let candidate = propose_loop_invariant_scalar_motion(&session, 1)
        .expect("exact candidate")
        .pop()
        .expect("one candidate");
    let validated = validate_loop_invariant_scalar_motion(&session, &candidate)
        .expect("validated exact candidate");
    let applied =
        apply_loop_invariant_scalar_motion(session, validated).expect("applied exact candidate");
    let [component] = applied.session().cycle_components().components() else {
        panic!("one natural loop component")
    };
    let machine = component.id.machine;
    let member = component.members[0];
    let (input, mut unit) = applied.into_session().into_parts();
    let relocation = candidate
        .relocations()
        .iter()
        .find(|relocation| !relocation.node().operand_rewrites().is_empty())
        .expect("the invariant computation carries an operand rewrite");
    // Forging the rebound operand back to the loop parameter must fail the
    // seed-derived substitution, not just dominance bookkeeping.
    let forged = find_operation_mut(&mut unit, relocation.node().psi_operation());
    if let AbstractOperation::WrappingIntegerAdd { right, .. } = &mut forged.operation {
        *right = relocation.node().operand_rewrites()[0].0;
    }
    forged.uses[1].value = relocation.node().operand_rewrites()[0].0;
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    assert!(matches!(
        validate_transformed_psi_optimization_unit(&input, &unit),
        Err(
            OptimizationUnitValidationError::RankedCycleFrozenBlockMismatch {
                machine: rejected_machine,
                block
            }
        ) if rejected_machine == machine && block == member
    ));
}
