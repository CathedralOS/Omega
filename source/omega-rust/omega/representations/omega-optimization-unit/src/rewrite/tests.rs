//! Rewrite model, construction, and identity tests.

use std::collections::BTreeSet;

use super::*;

#[test]
fn literal_fact_identity_binds_revision_definition_value_type_constant_and_support() {
    let revision = OptimizationUnitIdentity::from_canonical_bytes(b"revision-a");
    let machine = MachineId::new(1).unwrap();
    let definition = ValueDefinition {
        value: ValueId::new(2).unwrap(),
        scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap()),
        site: ValueDefinitionSite::Node {
            block: BlockId::new(3).unwrap(),
            node: 4,
        },
    };
    let identity = literal_scalar_constant_fact_identity(
        revision,
        machine,
        definition,
        ScalarConstantValue::Integer(IntegerValue::Unsigned(7)),
        OperationId::new(5).unwrap(),
    )
    .unwrap();
    assert_eq!(
        identity,
        literal_scalar_constant_fact_identity(
            revision,
            machine,
            definition,
            ScalarConstantValue::Integer(IntegerValue::Unsigned(7)),
            OperationId::new(5).unwrap(),
        )
        .unwrap()
    );
    assert_ne!(
        identity,
        literal_scalar_constant_fact_identity(
            OptimizationUnitIdentity::from_canonical_bytes(b"revision-b"),
            machine,
            definition,
            ScalarConstantValue::Integer(IntegerValue::Unsigned(7)),
            OperationId::new(5).unwrap(),
        )
        .unwrap()
    );
    assert_ne!(
        identity,
        literal_scalar_constant_fact_identity(
            revision,
            machine,
            ValueDefinition {
                site: ValueDefinitionSite::Node {
                    block: BlockId::new(3).unwrap(),
                    node: 6,
                },
                ..definition
            },
            ScalarConstantValue::Integer(IntegerValue::Unsigned(7)),
            OperationId::new(5).unwrap(),
        )
        .unwrap()
    );
    assert_ne!(
        identity,
        literal_scalar_constant_fact_identity(
            revision,
            machine,
            definition,
            ScalarConstantValue::Integer(IntegerValue::Unsigned(8)),
            OperationId::new(5).unwrap(),
        )
        .unwrap()
    );
    assert_ne!(
        identity,
        literal_scalar_constant_fact_identity(
            revision,
            machine,
            definition,
            ScalarConstantValue::Integer(IntegerValue::Unsigned(7)),
            OperationId::new(6).unwrap(),
        )
        .unwrap()
    );
    assert!(
        literal_scalar_constant_fact_identity(
            revision,
            machine,
            definition,
            ScalarConstantValue::Boolean(true),
            OperationId::new(5).unwrap(),
        )
        .is_none()
    );
}

#[test]
fn derived_sccp_identity_binds_every_exact_edge_verdict() {
    let revision = OptimizationUnitIdentity::from_canonical_bytes(b"sccp-revision");
    let machine = MachineId::new(10).unwrap();
    let entry = BlockId::new(11).unwrap();
    let merge = BlockId::new(12).unwrap();
    let definition = ValueDefinition {
        value: ValueId::new(13).unwrap(),
        scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap()),
        site: ValueDefinitionSite::BlockParameter {
            block: merge,
            position: 0,
        },
    };
    let snapshot = SccpMachineSnapshot {
        blocks: vec![
            SccpBlockRow {
                block: entry,
                executable: true,
            },
            SccpBlockRow {
                block: merge,
                executable: true,
            },
        ],
        edges: vec![
            SccpEdgeRow {
                source: entry,
                edge: EdgeId::new(14).unwrap(),
                target: merge,
                state: SccpEdgeState::Executable,
            },
            SccpEdgeRow {
                source: entry,
                edge: EdgeId::new(15).unwrap(),
                target: merge,
                state: SccpEdgeState::Inexecutable,
            },
        ],
        values: vec![SccpValueRow {
            definition,
            state: SccpValueState::Integer(IntegerValue::Unsigned(7)),
        }],
    };
    let identity = derived_sccp_scalar_constant_fact_identity(
        revision,
        machine,
        definition,
        ScalarConstantValue::Integer(IntegerValue::Unsigned(7)),
        &snapshot,
    )
    .unwrap();
    let mut changed_verdict = snapshot.clone();
    changed_verdict.edges[1].state = SccpEdgeState::Unknown;
    assert_ne!(
        identity,
        derived_sccp_scalar_constant_fact_identity(
            revision,
            machine,
            definition,
            ScalarConstantValue::Integer(IntegerValue::Unsigned(7)),
            &changed_verdict,
        )
        .unwrap()
    );
    let mut omitted_edge = snapshot.clone();
    omitted_edge.edges.pop();
    assert_ne!(
        identity,
        derived_sccp_scalar_constant_fact_identity(
            revision,
            machine,
            definition,
            ScalarConstantValue::Integer(IntegerValue::Unsigned(7)),
            &omitted_edge,
        )
        .unwrap()
    );
    let mut noncanonical = snapshot;
    noncanonical.edges.reverse();
    assert!(
        derived_sccp_scalar_constant_fact_identity(
            revision,
            machine,
            definition,
            ScalarConstantValue::Integer(IntegerValue::Unsigned(7)),
            &noncanonical,
        )
        .is_none()
    );
}

#[test]
fn phi_translated_candidate_identity_binds_canonical_incoming_leaders() {
    let machine = MachineId::new(100).unwrap();
    let join = BlockId::new(200).unwrap();
    let left = BlockId::new(201).unwrap();
    let right = BlockId::new(202).unwrap();
    let redundant = NodeLocation {
        machine,
        block: join,
        node: 0,
    };
    let incoming = vec![
        PhiTranslatedScalarIncoming {
            source: left,
            edge: EdgeId::new(301).unwrap(),
            leader: NodeLocation {
                machine,
                block: left,
                node: 0,
            },
            leader_operation: OperationId::new(401).unwrap(),
            leader_result: ValueId::new(501).unwrap(),
        },
        PhiTranslatedScalarIncoming {
            source: right,
            edge: EdgeId::new(302).unwrap(),
            leader: NodeLocation {
                machine,
                block: right,
                node: 0,
            },
            leader_operation: OperationId::new(402).unwrap(),
            leader_result: ValueId::new(502).unwrap(),
        },
    ];
    let contract = OptimizationRuleContract::new(
        OptimizationRuleIdentity::from_canonical_bytes(b"phi-rule"),
        omega_optimization_core::OptimizationPassIdentity::from_canonical_bytes(b"phi-pass"),
        1,
        AnalysisSet::default(),
        AnalysisInvalidationSet::default(),
        OptimizationSafetyClass::ExactOperationSemantics,
    )
    .unwrap();
    let source = PsiProvenance::Operation(OperationId::new(403).unwrap());
    let provenance = vec![ProvenanceRewrite {
        input: PsiRealizationSite::Node(redundant),
        disposition: ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(redundant)),
        sources: vec![source],
        fuel: vec![FuelSettlement {
            site: source,
            units: 1,
        }],
    }];
    let patch = PhiTranslatedScalarGvnRewrite {
        redundant,
        redundant_operation: OperationId::new(404).unwrap(),
        redundant_result: ValueId::new(503).unwrap(),
        scalar_type: ScalarType::Boolean,
        parameter_position: 1,
        incoming,
    };
    let candidate = PsiRewriteCandidate::new_phi_translated_scalar_common_subexpression(
        OptimizationUnitIdentity::from_canonical_bytes(b"phi-input"),
        contract,
        vec![join, left, right],
        provenance.clone(),
        -1,
        patch.clone(),
    )
    .unwrap();
    let mut changed = patch;
    changed.incoming[1].leader_result = ValueId::new(504).unwrap();
    let changed = PsiRewriteCandidate::new_phi_translated_scalar_common_subexpression(
        OptimizationUnitIdentity::from_canonical_bytes(b"phi-input"),
        contract,
        vec![join, left, right],
        provenance,
        -1,
        changed,
    )
    .unwrap();
    assert_ne!(candidate.identity(), changed.identity());
}

#[test]
fn proof_certified_live_identity_candidate_identity_binds_policy_and_side_kind() {
    let machine = MachineId::new(601).unwrap();
    let block = BlockId::new(602).unwrap();
    let location = NodeLocation {
        machine,
        block,
        node: 1,
    };
    let contract = OptimizationRuleContract::new(
        OptimizationRuleIdentity::from_canonical_bytes(b"divide-one-rule"),
        omega_optimization_core::OptimizationPassIdentity::from_canonical_bytes(b"divide-one-pass"),
        1,
        AnalysisSet::default(),
        AnalysisInvalidationSet::default(),
        OptimizationSafetyClass::ProofCertified,
    )
    .unwrap();
    let source_operation = OperationId::new(603).unwrap();
    let source = PsiProvenance::Operation(source_operation);
    let provenance = vec![ProvenanceRewrite {
        input: PsiRealizationSite::Node(location),
        disposition: ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(location)),
        sources: vec![source],
        fuel: vec![FuelSettlement {
            site: source,
            units: 1,
        }],
    }];
    let identities = [
        ProofCertifiedScalarIdentityKind::ExactIntegerDivideOneRight,
        ProofCertifiedScalarIdentityKind::WrappingIntegerDivideOneRight,
        ProofCertifiedScalarIdentityKind::SaturatingIntegerDivideOneRight,
        ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyZeroLeft,
        ProofCertifiedScalarIdentityKind::ExactIntegerMultiplyZeroRight,
        ProofCertifiedScalarIdentityKind::ExactIntegerDivideZeroLeft,
        ProofCertifiedScalarIdentityKind::WrappingIntegerDivideZeroLeft,
        ProofCertifiedScalarIdentityKind::SaturatingIntegerDivideZeroLeft,
        ProofCertifiedScalarIdentityKind::ExactIntegerRemainderZeroLeft,
        ProofCertifiedScalarIdentityKind::WrappingIntegerRemainderZeroLeft,
        ProofCertifiedScalarIdentityKind::SaturatingIntegerRemainderZeroLeft,
        ProofCertifiedScalarIdentityKind::ExactIntegerShiftLeftZeroValue,
        ProofCertifiedScalarIdentityKind::ExactIntegerShiftRightZeroValue,
        ProofCertifiedScalarIdentityKind::ExactIntegerShiftRightNegativeOneValue,
    ]
    .map(|identity| {
        PsiRewriteCandidate::new_proof_certified_scalar_identity(
            OptimizationUnitIdentity::from_canonical_bytes(b"divide-one-input"),
            contract,
            vec![block],
            provenance.clone(),
            ScalarConstantFactIdentity::from_canonical_bytes(b"divide-one-literal"),
            AcceptedObligationFactIdentity::from_canonical_bytes(b"divide-one-proof"),
            -1,
            ProofCertifiedScalarIdentityRewrite {
                location,
                source_operation,
                result: ValueId::new(604).unwrap(),
                replacement: ValueId::new(605).unwrap(),
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
                identity,
            },
        )
        .unwrap()
        .identity()
    });
    assert_eq!(identities.into_iter().collect::<BTreeSet<_>>().len(), 14);
}

#[test]
fn total_scalar_identity_codec_binds_all_twenty_six_rows_and_only_the_literal_fact() {
    let machine = MachineId::new(701).unwrap();
    let block = BlockId::new(702).unwrap();
    let location = NodeLocation {
        machine,
        block,
        node: 1,
    };
    let contract = OptimizationRuleContract::new(
        OptimizationRuleIdentity::from_canonical_bytes(b"total-wrapping-identity-rule"),
        omega_optimization_core::OptimizationPassIdentity::from_canonical_bytes(
            b"total-wrapping-identity-pass",
        ),
        1,
        AnalysisSet::default(),
        AnalysisInvalidationSet::default(),
        OptimizationSafetyClass::ExactOperationSemantics,
    )
    .unwrap();
    let source_operation = OperationId::new(703).unwrap();
    let source = PsiProvenance::Operation(source_operation);
    let provenance = vec![ProvenanceRewrite {
        input: PsiRealizationSite::Node(location),
        disposition: ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(location)),
        sources: vec![source],
        fuel: vec![FuelSettlement {
            site: source,
            units: 1,
        }],
    }];
    let constant_fact = ScalarConstantFactIdentity::from_canonical_bytes(b"law-literal");
    let identities = [
        (TotalScalarIdentityKind::WrappingIntegerAddZeroLeft, 1),
        (TotalScalarIdentityKind::WrappingIntegerAddZeroRight, 2),
        (TotalScalarIdentityKind::WrappingIntegerSubtractZeroRight, 3),
        (TotalScalarIdentityKind::WrappingIntegerMultiplyOneLeft, 4),
        (TotalScalarIdentityKind::WrappingIntegerMultiplyOneRight, 5),
        (
            TotalScalarIdentityKind::WrappingIntegerShiftLeftZeroCount,
            6,
        ),
        (
            TotalScalarIdentityKind::WrappingIntegerShiftRightZeroCount,
            7,
        ),
        (TotalScalarIdentityKind::WrappingIntegerMultiplyZeroLeft, 8),
        (TotalScalarIdentityKind::WrappingIntegerMultiplyZeroRight, 9),
        (TotalScalarIdentityKind::SaturatingIntegerAddZeroLeft, 10),
        (TotalScalarIdentityKind::SaturatingIntegerAddZeroRight, 11),
        (
            TotalScalarIdentityKind::SaturatingIntegerSubtractZeroRight,
            12,
        ),
        (
            TotalScalarIdentityKind::SaturatingIntegerMultiplyOneLeft,
            13,
        ),
        (
            TotalScalarIdentityKind::SaturatingIntegerMultiplyOneRight,
            14,
        ),
        (
            TotalScalarIdentityKind::SaturatingIntegerMultiplyZeroLeft,
            15,
        ),
        (
            TotalScalarIdentityKind::SaturatingIntegerMultiplyZeroRight,
            16,
        ),
        (TotalScalarIdentityKind::IntegerBitwiseAndAllOnesLeft, 17),
        (
            TotalScalarIdentityKind::IntegerBitwiseAndAllOnesRight,
            18,
        ),
        (TotalScalarIdentityKind::IntegerBitwiseOrZeroLeft, 19),
        (TotalScalarIdentityKind::IntegerBitwiseOrZeroRight, 20),
        (TotalScalarIdentityKind::IntegerBitwiseXorZeroLeft, 21),
        (TotalScalarIdentityKind::IntegerBitwiseXorZeroRight, 22),
        (TotalScalarIdentityKind::IntegerBitwiseAndZeroLeft, 23),
        (TotalScalarIdentityKind::IntegerBitwiseAndZeroRight, 24),
        (TotalScalarIdentityKind::IntegerBitwiseOrAllOnesLeft, 25),
        (TotalScalarIdentityKind::IntegerBitwiseOrAllOnesRight, 26),
    ]
    .map(|(identity, identity_tag)| {
        let input =
            OptimizationUnitIdentity::from_canonical_bytes(b"total-wrapping-identity-input");
        let candidate = PsiRewriteCandidate::new_total_scalar_identity(
            input,
            contract,
            vec![block],
            provenance.clone(),
            constant_fact,
            -1,
            TotalScalarIdentityRewrite {
                location,
                source_operation,
                result: ValueId::new(704).unwrap(),
                replacement: ValueId::new(705).unwrap(),
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
                identity,
            },
        )
        .unwrap();
        assert_eq!(
            candidate.total_scalar_identity_witness(),
            Some(constant_fact)
        );
        assert_eq!(
            candidate.consumed_facts(),
            [OptimizationFactReference::ScalarConstant(constant_fact)]
        );
        assert!(candidate.accepted_obligation_witness().is_none());
        let canonical = super::codec::encode_candidate(
            input,
            contract,
            candidate.decision_point(),
            candidate.affected_blocks(),
            candidate.substitutions(),
            candidate.provenance(),
            &candidate.witness,
            candidate.predicted_cost_delta(),
            candidate.patch_ref(),
        );
        let fact_position = canonical
            .windows(32)
            .position(|window| window == constant_fact.bytes())
            .unwrap();
        assert_eq!(canonical[fact_position - 1], 7, "appended witness tag");
        let operation = source_operation.get().to_le_bytes();
        let patch_operation_position = canonical
            .windows(8)
            .rposition(|window| window == operation)
            .unwrap();
        assert_eq!(
            canonical[patch_operation_position - 21],
            16,
            "appended patch tag"
        );
        assert_eq!(canonical.last(), Some(&identity_tag));
        candidate.identity()
    });
    assert_eq!(identities.into_iter().collect::<BTreeSet<_>>().len(), 26);
}
