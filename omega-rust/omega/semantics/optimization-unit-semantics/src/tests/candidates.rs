//! Candidate observation, replay, and corruption tests.

use super::*;

#[test]
fn write_only_store_cannot_be_dropped_as_a_dead_scalar_node() {
    let input = write_only_store_unit();
    let function = &input.functions[0];
    let block = &function.blocks[0];
    let location = NodeLocation {
        machine: function.machine,
        block: block.id,
        node: 1,
    };
    let next = NodeLocation {
        machine: function.machine,
        block: block.id,
        node: 2,
    };
    let node = &block.nodes[1];
    let AbstractOperation::WriteOnlyPrimitiveStore {
        psi_operation,
        value,
        ..
    } = node.operation
    else {
        panic!("fixture second node is the write-only store")
    };
    let contract = OptimizationRuleContract::new(
        OptimizationRuleIdentity::from_canonical_bytes(
            b"omega.psi-rule.dead-unused-scalar-literal-elimination.v1",
        ),
        OptimizationPassIdentity::from_canonical_bytes(b"dead-scalar-drop-rejection-test"),
        1,
        AnalysisSet::new([AnalysisKind::ValueLiveness, AnalysisKind::EffectSummaries]),
        AnalysisInvalidationSet::new([AnalysisKind::UseDefinition, AnalysisKind::EffectSummaries]),
        OptimizationSafetyClass::ExactOperationSemantics,
    )
    .unwrap();
    let candidate = PsiRewriteCandidate::new_dead_scalar_node(
        input.identity,
        contract,
        vec![block.id],
        vec![ProvenanceRewrite {
            input: PsiRealizationSite::Node(location),
            disposition: ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(next)),
            sources: node.provenance.clone(),
            fuel: node.fuel.clone(),
        }],
        -1,
        DeadScalarNodeRewrite {
            location,
            source_operation: psi_operation,
            result: value.value,
            scalar_type: value.scalar_type,
        },
    )
    .expect("candidate envelope is structurally well formed");

    assert_eq!(
        validate_dead_scalar_node_candidate(&input, &candidate),
        Err(OptimizationUnitValidationError::CandidatePatchMismatch)
    );
}

#[test]
fn redundant_parameter_region_observation_is_canonical_and_axis_complete() {
    let (input, output, patch, affected) = redundant_parameter_region_fixture();
    let normalized = normalize_redundant_parameter_observation_input(&input, patch, &affected)
        .expect("independent input normalization");
    let expected = reconstruct_psi_closed_region_observation(
        &normalized,
        patch.machine,
        &[affected[1], affected[0], affected[1]],
    )
    .expect("canonical normalized region");
    let baseline = reconstruct_psi_closed_region_observation(&output, patch.machine, &affected)
        .expect("canonical output region");
    assert_eq!(expected.semantics, baseline.semantics);
    assert_ne!(input.identity, output.identity);
    assert_eq!(baseline.semantics.blocks.len(), 2);
    assert!(baseline.semantics.incoming_edges.is_empty());
    assert!(baseline.semantics.outgoing_edges.is_empty());
    assert_eq!(baseline.semantics.scalar_live_ins.len(), 3);
    assert!(baseline.semantics.scalar_live_outs.is_empty());
    let merge_only =
        reconstruct_psi_closed_region_observation(&output, patch.machine, &[patch.block])
            .expect("single-block graph cut");
    assert_eq!(merge_only.semantics.incoming_edges.len(), 2);
    assert!(merge_only.semantics.outgoing_edges.is_empty());
    assert_eq!(merge_only.semantics.scalar_live_ins.len(), 2);
    assert!(unchanged_outside_redundant_parameter_region(
        &input,
        &output,
        patch.machine,
        &affected,
    ));
    let mut outside_region = output.clone();
    outside_region.fuel_schedule = FuelScheduleIdentity::new(2).unwrap();
    assert!(!unchanged_outside_redundant_parameter_region(
        &input,
        &outside_region,
        patch.machine,
        &affected,
    ));

    let mut corruptions = Vec::new();

    let mut arithmetic_policy = output.clone();
    let node = &mut arithmetic_policy.functions[0].blocks[1].nodes[0];
    let AbstractOperation::ExactIntegerAdd {
        psi_operation,
        result,
        scalar_type,
        left,
        right,
        ..
    } = node.operation.clone()
    else {
        unreachable!()
    };
    node.operation = AbstractOperation::WrappingIntegerAdd {
        psi_operation,
        result,
        scalar_type,
        left,
        right,
    };
    corruptions.push(("arithmetic policy", arithmetic_policy));

    let mut edge = output.clone();
    let AbstractOperation::Conditional { when_true, .. } =
        &mut edge.functions[0].blocks[0].nodes[0].operation
    else {
        unreachable!()
    };
    when_true.psi_edge = id(799, EdgeId::new);
    corruptions.push(("control edge", edge));

    let mut successor = output.clone();
    successor.functions[0].blocks[0].nodes[0].successors[0].psi_edge = id(796, EdgeId::new);
    corruptions.push(("successor row", successor));

    let mut normal_exit = output.clone();
    let AbstractOperation::Return { psi_edge, .. } =
        &mut normal_exit.functions[0].blocks[1].nodes[1].operation
    else {
        unreachable!()
    };
    *psi_edge = id(798, EdgeId::new);
    corruptions.push(("normal exit", normal_exit));

    let mut effect = output.clone();
    effect.functions[0].blocks[1].nodes[0].effect.output += 1;
    corruptions.push(("effect", effect));

    let mut ownership = output.clone();
    ownership.functions[0].blocks[1].nodes[0]
        .ownership
        .push(OwnershipEvent::ClaimCompletion(Vec::new()));
    corruptions.push(("ownership/cleanup", ownership));

    let mut provenance = output.clone();
    provenance.functions[0].blocks[1].nodes[0]
        .provenance
        .push(PsiProvenance::Edge(id(797, EdgeId::new)));
    corruptions.push(("provenance", provenance));

    let mut fuel = output.clone();
    fuel.functions[0].blocks[1].nodes[0].fuel[0].units += 1;
    corruptions.push(("fuel", fuel));

    let mut call_and_suspension = output.clone();
    call_and_suspension.functions[0].blocks[1].nodes[0].operation = AbstractOperation::Call {
        psi_operation: id(711, OperationId::new),
        result: id(708, ValueId::new),
        scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap()),
        callee: patch.machine,
        arguments: vec![patch.replacement],
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
    };
    corruptions.push(("call/crash/suspension", call_and_suspension));

    let mut live_boundary = output.clone();
    live_boundary.functions[0].blocks[1].nodes[0].uses[0].value = id(704, ValueId::new);
    corruptions.push(("typed scalar boundary", live_boundary));

    let mut frontier = output.clone();
    frontier
        .ownership_frontier_facts
        .push(OwnershipFrontierFact::new(
            frontier.psi,
            patch.machine,
            OwnershipFrontierSite::BlockEntry(affected[0]),
            OwnershipFrontierSnapshot {
                claims: Vec::new(),
                owned_places: Vec::new(),
                partial_custody: Vec::new(),
            },
        ));
    corruptions.push(("verifier frontier", frontier));

    for (axis, corrupted) in corruptions {
        let observed =
            reconstruct_psi_closed_region_observation(&corrupted, patch.machine, &affected)
                .expect("corrupted region remains observable");
        assert_ne!(baseline.semantics, observed.semantics, "{axis}");
    }
}

#[test]
fn independent_integer_rewrite_constructor_accepts_only_declared_evaluation() {
    let input = exact_add_unit();
    let candidate = integer_candidate(&input, IntegerValue::Unsigned(15));
    let replay = integer_candidate(&input, IntegerValue::Unsigned(15));
    assert_eq!(candidate.identity(), replay.identity());
    let input_boundary = reconstruct_closed_scalar_node_boundary(
        &input,
        NodeLocation {
            machine: id(201, MachineId::new),
            block: id(202, BlockId::new),
            node: 2,
        },
    )
    .unwrap();
    let accepted = validate_integer_evaluation_candidate(&input, &candidate).unwrap();
    let output_boundary =
        reconstruct_closed_scalar_node_boundary(accepted.unit(), input_boundary.location).unwrap();
    assert_eq!(input_boundary.live_in.len(), 2);
    assert!(output_boundary.live_in.is_empty());
    assert_eq!(input_boundary.live_out, output_boundary.live_out);
    assert_eq!(accepted.candidate(), candidate.identity());
    assert_ne!(accepted.unit().identity, input.identity);
    assert_eq!(
        accepted.unit().identity,
        recompute_psi_optimization_unit_identity(accepted.unit())
    );
    assert_eq!(
        accepted.unit().functions[0].blocks[0].nodes[2].provenance,
        input.functions[0].blocks[0].nodes[2].provenance
    );
    assert_eq!(
        accepted.unit().functions[0].blocks[0].nodes[2].fuel,
        input.functions[0].blocks[0].nodes[2].fuel
    );
    assert!(matches!(
        accepted.unit().functions[0].blocks[0].nodes[2].operation,
        AbstractOperation::IntegerConstant {
            value: IntegerValue::Unsigned(15),
            ..
        }
    ));
    assert!(matches!(
        accepted.unit().functions[0].facts[2],
        OptimizationFact::IntegerConstant {
            constant: IntegerValue::Unsigned(15),
            ..
        }
    ));
    assert!(matches!(
        input.functions[0].blocks[0].nodes[2].operation,
        AbstractOperation::ExactIntegerAdd { .. }
    ));

    let wrong = integer_candidate(&input, IntegerValue::Unsigned(14));
    assert!(matches!(
        validate_integer_evaluation_candidate(&input, &wrong),
        Err(OptimizationUnitValidationError::CandidateEvaluationMismatch)
    ));

    let foreign_fact = integer_candidate_with_facts(
        &input,
        IntegerValue::Unsigned(15),
        Some(
            optimization_core::ScalarConstantFactIdentity::from_canonical_bytes(
                b"fact from another revision",
            ),
        ),
        None,
    );
    assert!(matches!(
        validate_integer_evaluation_candidate(&input, &foreign_fact),
        Err(OptimizationUnitValidationError::CandidateOperandFactMismatch)
    ));

    let foreign_obligation = integer_candidate_with_facts(
        &input,
        IntegerValue::Unsigned(15),
        None,
        Some(
            optimization_core::AcceptedObligationFactIdentity::from_canonical_bytes(
                b"fact admitted for another operation",
            ),
        ),
    );
    assert!(matches!(
        validate_integer_evaluation_candidate(&input, &foreign_obligation),
        Err(OptimizationUnitValidationError::CandidateAcceptedObligationFactMismatch)
    ));
}

#[test]
fn candidate_history_does_not_declare_the_accepted_content_identity() {
    let input = exact_add_unit();
    let first =
        integer_candidate_with_facts_and_cost(&input, IntegerValue::Unsigned(15), None, None, -1);
    let second =
        integer_candidate_with_facts_and_cost(&input, IntegerValue::Unsigned(15), None, None, -2);
    assert_ne!(first.identity(), second.identity());

    let first_output = validate_integer_evaluation_candidate(&input, &first).unwrap();
    let second_output = validate_integer_evaluation_candidate(&input, &second).unwrap();
    assert_eq!(first_output.unit(), second_output.unit());
    assert_eq!(
        first_output.unit().identity,
        recompute_psi_optimization_unit_identity(first_output.unit())
    );
}

#[test]
fn corruption_classes_fail_independently() {
    let mut accepted_fact = exact_add_unit();
    accepted_fact.accepted_obligation_facts[0].proof_bundle_fingerprint[0] ^= 1;
    refresh_identity(&mut accepted_fact);
    assert!(matches!(
        validate_psi_optimization_unit(&accepted_fact),
        Err(OptimizationUnitValidationError::AcceptedObligationFactIndexMismatch)
    ));

    let mut provenance = unit();
    provenance.functions[0].blocks[0].nodes[0]
        .provenance
        .clear();
    refresh_identity(&mut provenance);
    assert!(matches!(
        validate_psi_optimization_unit(&provenance),
        Err(OptimizationUnitValidationError::OperationMetadataMismatch { .. })
    ));

    let mut fuel = unit();
    fuel.functions[0].blocks[0].nodes[0].fuel.clear();
    refresh_identity(&mut fuel);
    assert!(matches!(
        validate_psi_optimization_unit(&fuel),
        Err(OptimizationUnitValidationError::FuelDoesNotMatchProvenance { .. })
    ));

    let mut effects = unit();
    effects.functions[0].blocks[0].nodes[1].effect.input = 99;
    refresh_identity(&mut effects);
    assert!(matches!(
        validate_psi_optimization_unit(&effects),
        Err(OptimizationUnitValidationError::BrokenEffectChain { .. })
    ));

    let mut facts = unit();
    facts.functions[0].facts.clear();
    refresh_identity(&mut facts);
    assert!(matches!(
        validate_psi_optimization_unit(&facts),
        Err(OptimizationUnitValidationError::FactIndexMismatch(_))
    ));

    let mut forged_uses = unit();
    let block = forged_uses.functions[0].blocks[0].id;
    forged_uses.functions[0].blocks[0].nodes[1]
        .uses
        .push(ValueUse {
            value: id(99, ValueId::new),
            block,
            node: 1,
        });
    refresh_identity(&mut forged_uses);
    assert!(matches!(
        validate_psi_optimization_unit(&forged_uses),
        Err(OptimizationUnitValidationError::OperationMetadataMismatch { .. })
    ));

    let mut forged_definitions = unit();
    forged_definitions.functions[0].blocks[0].nodes[0]
        .definitions
        .clear();
    refresh_identity(&mut forged_definitions);
    assert!(matches!(
        validate_psi_optimization_unit(&forged_definitions),
        Err(OptimizationUnitValidationError::OperationMetadataMismatch { .. })
    ));

    let mut undefined = unit();
    let unknown = id(99, ValueId::new);
    let AbstractOperation::Return { value, .. } =
        &mut undefined.functions[0].blocks[0].nodes[1].operation
    else {
        panic!("unit ends in return")
    };
    *value = unknown;
    undefined.functions[0].blocks[0].nodes[1].uses = vec![ValueUse {
        value: unknown,
        block,
        node: 1,
    }];
    refresh_identity(&mut undefined);
    assert!(matches!(
        validate_psi_optimization_unit(&undefined),
        Err(OptimizationUnitValidationError::UndefinedValue { .. })
    ));

    let mut place = unit();
    place.functions[0]
        .declared_places
        .insert(id(88, PlaceId::new));
    refresh_identity(&mut place);
    assert!(matches!(
        validate_psi_optimization_unit(&place),
        Err(OptimizationUnitValidationError::UnknownPlace { .. })
    ));

    let mut cleanup = unit();
    cleanup.functions[0].blocks[0].nodes[1].ownership.clear();
    refresh_identity(&mut cleanup);
    assert!(matches!(
        validate_psi_optimization_unit(&cleanup),
        Err(OptimizationUnitValidationError::OperationMetadataMismatch { .. })
    ));

    let mut cfg = unit();
    cfg.functions[0].blocks[0].nodes[1].operation = AbstractOperation::Jump {
        psi_edge: id(5, EdgeId::new),
        target: id(77, BlockId::new),
        bindings: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    cfg.functions[0].blocks[0].nodes[1].successors =
        expected_edges(&cfg.functions[0].blocks[0].nodes[1].operation);
    cfg.functions[0].blocks[0].nodes[1].uses.clear();
    cfg.functions[0].blocks[0].nodes[1].ownership.clear();
    cfg.functions[0].blocks[0].nodes[1].provenance.clear();
    cfg.functions[0].blocks[0].nodes[1].fuel.clear();
    refresh_identity(&mut cfg);
    assert!(matches!(
        validate_psi_optimization_unit(&cfg),
        Err(OptimizationUnitValidationError::UnknownSuccessor { .. })
    ));

    let mut entry_parameters = unit();
    let block = entry_parameters.functions[0].entry;
    entry_parameters.functions[0].blocks[0]
        .parameters
        .push(ValueDefinition {
            value: id(76, ValueId::new),
            scalar_type: ScalarType::Boolean,
            site: ValueDefinitionSite::BlockParameter { block, position: 0 },
        });
    refresh_identity(&mut entry_parameters);
    assert!(matches!(
        validate_psi_optimization_unit(&entry_parameters),
        Err(OptimizationUnitValidationError::EntryBlockHasParameters { .. })
    ));

    let mut unreachable = unit();
    let block = id(75, BlockId::new);
    let mut detached = unreachable.functions[0].blocks[0].clone();
    detached.id = block;
    for (node_index, node) in detached.nodes.iter_mut().enumerate() {
        let node_index = u32::try_from(node_index).unwrap();
        node.definitions = expected_definitions(&node.operation, block, node_index);
        node.uses = expected_uses(&node.operation, block, node_index);
    }
    unreachable.functions[0].blocks.push(detached);
    refresh_identity(&mut unreachable);
    assert!(matches!(
        validate_psi_optimization_unit(&unreachable),
        Err(OptimizationUnitValidationError::UnreachableBlock { .. })
    ));

    let mut cycle = unit();
    let block = cycle.functions[0].entry;
    let operation = AbstractOperation::Jump {
        psi_edge: id(5, EdgeId::new),
        target: block,
        bindings: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    let node = &mut cycle.functions[0].blocks[0].nodes[1];
    node.operation = operation;
    node.provenance = expected_provenance(&node.operation);
    node.uses = expected_uses(&node.operation, block, 1);
    node.successors = expected_edges(&node.operation);
    node.ownership = expected_ownership(&node.operation);
    refresh_identity(&mut cycle);
    assert!(matches!(
        validate_psi_optimization_unit(&cycle),
        Err(OptimizationUnitValidationError::ControlCycle { .. })
    ));
}

#[test]
fn unknown_claim_frontier_is_rejected() {
    let mut unit = unit();
    let claim = id(71, ClaimId::new);
    let edge = id(5, EdgeId::new);
    let operation = AbstractOperation::Crash {
        psi_edge: edge,
        cause: terminal_psi::CrashCause::Trap,
        site_guard: Vec::new(),
        frontier_lower_bound: vec![claim],
    };
    let node = &mut unit.functions[0].blocks[0].nodes[1];
    node.operation = operation;
    node.provenance = expected_provenance(&node.operation);
    node.fuel[0].site = PsiProvenance::Edge(edge);
    node.uses.clear();
    node.successors.clear();
    node.ownership = expected_ownership(&node.operation);
    refresh_identity(&mut unit);
    assert!(matches!(
        validate_psi_optimization_unit(&unit),
        Err(OptimizationUnitValidationError::UnknownClaim { .. })
    ));
}
