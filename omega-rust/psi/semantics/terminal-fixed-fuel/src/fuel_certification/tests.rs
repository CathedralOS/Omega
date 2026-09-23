use super::{
    BlockId, EdgeId, FixedFuelError, FixedSegmentFuelCertificate, MachineId, TerminalFuelSchedule,
    validate_certificate_sequence,
};
use crate::{FuelScheduleIdentity, Proposition};
use terminal_codec::TerminalPsiIdentity;
use terminal_psi::{
    OperationKind, TerminalMachine, TerminalModule, TerminalNaturalCycle, TerminalRankedScc,
    Terminator,
};

fn identity(byte: u8) -> TerminalPsiIdentity {
    TerminalPsiIdentity {
        vocabulary_marker: terminal_psi::VocabularyMarker::CURRENT,
        program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes([byte; 32]),
    }
}

#[test]
fn segment_row_comparison_binds_every_identity_endpoint_and_ceiling() {
    let expected = FixedSegmentFuelCertificate {
        terminal_psi: identity(1),
        schedule: TerminalFuelSchedule::CURRENT.identity(),
        machine: MachineId::new(1).expect("nonzero machine"),
        start_block: BlockId::new(2).expect("nonzero block"),
        end_edge: EdgeId::new(3).expect("nonzero edge"),
        relevant_preconditions: Vec::new(),
        ceiling_units: 4,
    };
    validate_certificate_sequence(
        std::slice::from_ref(&expected),
        std::slice::from_ref(&expected),
    )
    .expect("the exact row matches");

    let mut mutations = Vec::new();
    let mut changed = expected.clone();
    changed.terminal_psi = identity(2);
    mutations.push(changed);
    let mut changed = expected.clone();
    changed.schedule = FuelScheduleIdentity::new(2).expect("nonzero schedule");
    mutations.push(changed);
    let mut changed = expected.clone();
    changed.machine = MachineId::new(2).expect("nonzero machine");
    mutations.push(changed);
    let mut changed = expected.clone();
    changed.start_block = BlockId::new(3).expect("nonzero block");
    mutations.push(changed);
    let mut changed = expected.clone();
    changed.end_edge = EdgeId::new(4).expect("nonzero edge");
    mutations.push(changed);
    let mut changed = expected.clone();
    changed.relevant_preconditions = vec![Proposition::Truth];
    mutations.push(changed);
    let mut changed = expected.clone();
    changed.ceiling_units = 5;
    mutations.push(changed);

    for mutation in mutations {
        assert_eq!(
            validate_certificate_sequence(
                std::slice::from_ref(&expected),
                std::slice::from_ref(&mutation),
            ),
            Err(FixedFuelError::CertificateMismatch)
        );
    }
}

mod machine_bounds {
    use super::super::outcome_bounds::{
        boundary_call_candidates, dynamic_call_targets, maximum_machine_outcomes,
        used_contract_premises,
    };
    use super::super::segment_partition::{PreparedFuelModule, PreparedSegments};
    use super::super::{
        derive_fixed_entry_fuel, derive_fixed_safe_point_segments, derive_fixed_segment_fuel,
        derive_maximum_entry_bound, derive_validated_fixed_safe_point_segments,
        retain_validated_fixed_safe_point_segments, validate_fixed_entry_fuel,
        validate_fixed_segment_fuel, validate_retained_fixed_safe_point_segments,
    };
    use super::{
        BlockId, EdgeId, FixedFuelError, FuelScheduleIdentity, OperationKind, Proposition,
        TerminalFuelSchedule, TerminalMachine, TerminalModule, TerminalNaturalCycle,
        TerminalRankedScc, Terminator, identity,
    };
    use semantic_vocabulary::{
        ContractId, IntegerSign, IntegerType, IntegerValue, ScalarTerm, ScalarType, ValueId,
    };
    use std::collections::{BTreeMap, BTreeSet};
    use terminal_psi::{
        Block, MachineContract, Operation, OperationResult, ProviderCandidateConformance,
        ProviderRefinement, ProviderSignature, TerminalBlockNaturalRank,
        TerminalIndirectDynamicDispatch, TerminalMachineResult, TerminalNaturalRankComparison,
        TerminalNaturalRankEdge, TerminalStoredDynamicDispatch, ValueDeclaration,
    };

    fn id<T: semantic_vocabulary::PsiSemanticId>(raw: u64) -> T {
        T::new(raw).expect("test identities are nonzero")
    }

    fn block(block_id: u64, operations: Vec<Operation>, terminator: Terminator) -> Block {
        Block {
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: id(block_id),
            parameters: Vec::new(),
            operations,
            terminator,
        }
    }

    fn machine(
        machine_id: u64,
        entry_block: u64,
        blocks: Vec<Block>,
        ranked_scc: Option<TerminalRankedScc>,
    ) -> TerminalMachine {
        TerminalMachine {
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
            id: id(machine_id),
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: Vec::new(),
            ranked_scc,
            result: TerminalMachineResult::Unit,
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: id(entry_block),
            blocks,
            contract: MachineContract {
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
                id: id::<ContractId>(machine_id),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }
    }

    fn module(entry: u64, machines: Vec<TerminalMachine>) -> TerminalModule {
        TerminalModule {
            scalar_qualifications: Default::default(),
            scalar_block_invariants: Vec::new(),
            operation_crash_contracts: Vec::new(),
            vocabulary_marker: terminal_psi::VocabularyMarker::CURRENT,
            entry: id(entry),
            structural_types: Vec::new(),
            structural_domains: Vec::new(),
            services: Vec::new(),
            root_service_reach: Default::default(),
            placed_view_inputs: Vec::new(),
            reborrow_root_handoffs: Vec::new(),
            reborrow_restored_call_uses: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            float_meaning_projections: Vec::new(),
            float_meaning_equalities: Vec::new(),
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            evidence_terms: Vec::new(),
            evidence_contract_lanes: Vec::new(),
            proof_output_calls: Vec::new(),
            proof_recursive_components: Vec::new(),
            closed_conformance_applications: Vec::new(),
            dynamic_dispatch: Default::default(),
            suspension_call_plan_count: 0,
            suspension_call_sites: Vec::new(),
            suspension_call_plans: Vec::new(),
            quotient_correspondences: Vec::new(),
            machines,
        }
    }

    fn jump(edge: u64, target: u64) -> Terminator {
        Terminator::Jump {
            edge: id(edge),
            target: id(target),
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
            residual_affine_discards: Vec::new(),
        }
    }

    fn conditional(
        edge_true: u64,
        target_true: u64,
        edge_false: u64,
        target_false: u64,
    ) -> Terminator {
        let successor = |edge, target| terminal_psi::SuccessorEdge {
            edge: id::<EdgeId>(edge),
            target: id::<BlockId>(target),
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
        };
        Terminator::Conditional {
            condition: id(9_000),
            when_true: successor(edge_true, target_true),
            when_false: successor(edge_false, target_false),
        }
    }

    fn return_unit(edge: u64) -> Terminator {
        Terminator::ReturnUnit {
            edge: id(edge),
            trivial_affine_discards: Vec::new(),
        }
    }

    fn call_unit(operation_id: u64, callee: u64) -> Operation {
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: id(operation_id),
            result: OperationResult::Unit,
            kind: OperationKind::CallUnit {
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                callee: id(callee),
                arguments: Vec::new(),
                structural_arguments: Vec::new(),
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        }
    }

    fn dynamic_unit_call(operation_id: u64, descriptor_ordinal: u32) -> Operation {
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: id(operation_id),
            result: OperationResult::Unit,
            kind: OperationKind::CallDynamicUnit {
                descriptor_ordinal,
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        }
    }

    fn dynamic_scalar_call(operation_id: u64, descriptor_ordinal: u32) -> Operation {
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: id(operation_id),
            result: OperationResult::Scalar(ValueDeclaration {
                id: id(20_000 + operation_id),
                scalar_type: ScalarType::Boolean,
                qualifications: Default::default(),
            }),
            kind: OperationKind::CallDynamicScalar {
                descriptor_ordinal,
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        }
    }

    fn parameter_unit_call(operation_id: u64) -> Operation {
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: id(operation_id),
            result: OperationResult::Unit,
            kind: OperationKind::CallDynamicParameterUnit {
                parameter_ordinal: 0,
                requirement_slot: 0,
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        }
    }

    fn boundary_call(operation_id: u64, boundary: u64) -> Operation {
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: id(operation_id),
            result: OperationResult::Unit,
            kind: OperationKind::BoundaryCall {
                boundary: id(boundary),
                arguments: Vec::new(),
                structural_arguments: Vec::new(),
                completion_receipts: Vec::new(),
            },
        }
    }

    fn integer_constant(operation_id: u64, result: u64, value: u64) -> Operation {
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: id(operation_id),
            result: OperationResult::Scalar(ValueDeclaration {
                id: id(result),
                scalar_type: ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 integer type"),
                ),
                qualifications: Default::default(),
            }),
            kind: OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(u128::from(value)),
            },
        }
    }

    fn boolean_constant(operation_id: u64, result: u64, value: bool) -> Operation {
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: id(operation_id),
            result: OperationResult::Scalar(ValueDeclaration {
                id: id(result),
                scalar_type: ScalarType::Boolean,
                qualifications: Default::default(),
            }),
            kind: OperationKind::BooleanConstant { value },
        }
    }

    fn return_scalar(edge: u64, value: u64) -> Terminator {
        Terminator::Return {
            edge: id(edge),
            value: id(value),
            cleanup_actions: Vec::new(),
        }
    }

    fn scalar_machine(machine_id: u64, entry_block: u64, blocks: Vec<Block>) -> TerminalMachine {
        let mut semantic = machine(machine_id, entry_block, blocks, None);
        semantic.result = TerminalMachineResult::Scalar(ValueDeclaration {
            id: id(20_000 + machine_id),
            scalar_type: ScalarType::Boolean,
            qualifications: Default::default(),
        });
        semantic
    }

    fn indirect_dispatch(
        owner: u64,
        operation: u64,
        descriptor_ordinal: u32,
        realization: u64,
    ) -> TerminalIndirectDynamicDispatch {
        TerminalIndirectDynamicDispatch {
            owner: id(owner),
            operation: id(operation),
            descriptor_ordinal,
            declaring_trait_identity: "test::Work".into(),
            public_requirement_identity: "test::Work::run()".into(),
            family_tuple: Vec::new(),
            requirement_identity: "test::Work::run".into(),
            realization_identity: "test::Item::run".into(),
            realization_callable_identity: "test::Item::run#callable".into(),
            realization: id(realization),
        }
    }

    fn stored_dispatch(
        owner: u64,
        operation: u64,
        descriptor_ordinal: u32,
        realization: u64,
    ) -> TerminalStoredDynamicDispatch {
        TerminalStoredDynamicDispatch {
            owner: id(owner),
            operation: id(operation),
            descriptor_ordinal,
            declaring_trait_identity: "test::Work".into(),
            public_requirement_identity: "test::Work::run()".into(),
            family_tuple: Vec::new(),
            requirement_identity: "test::Work::run".into(),
            realization_identity: "test::Item::run".into(),
            realization_callable_identity: "test::Item::run#callable".into(),
            realization: id(realization),
        }
    }

    fn provider_candidate(boundary: u64, candidate: u64) -> ProviderCandidateConformance {
        ProviderCandidateConformance {
            boundary: id(boundary),
            requirement_identity: "test::boundary".into(),
            provider_identity: format!("test::provider::{candidate}"),
            candidate_identity: format!("test::candidate::{candidate}"),
            candidate: id(candidate),
            signature: ProviderSignature {
                parameters: Vec::new(),
            },
            refinement: ProviderRefinement {
                positional_parameters: Vec::new(),
                required_domains: Vec::new(),
                realized_service_ceiling: Vec::new(),
            },
        }
    }

    fn rank_edge(
        edge: u64,
        source: u64,
        target: u64,
        comparison: TerminalNaturalRankComparison,
    ) -> TerminalNaturalRankEdge {
        TerminalNaturalRankEdge {
            edge: id(edge),
            source: id(source),
            target: id(target),
            successor_rank: id::<ValueId>(7_000),
            comparison,
        }
    }

    /// One `Natural` component {2,3}: header 2 conditionally enters work 3
    /// or exits to 4; work 3 jumps back to 2 (strict descent). Entry 1
    /// jumps into the cycle; the cycle exits to return block 4.
    fn cyclic_machine(rank_bits: u16, work_operations: Vec<Operation>) -> TerminalMachine {
        let mut semantic = machine(
            1,
            1,
            vec![
                block(1, Vec::new(), jump(1, 2)),
                block(2, Vec::new(), conditional(2, 3, 3, 4)),
                block(3, work_operations, jump(4, 2)),
                block(4, Vec::new(), return_unit(5)),
            ],
            None,
        );
        semantic.ranked_scc = Some(TerminalRankedScc::Natural(vec![TerminalNaturalCycle {
            rank_type: IntegerType::new(IntegerSign::Unsigned, rank_bits)
                .expect("fixed unsigned rank type"),
            ranks: [2, 3]
                .into_iter()
                .map(|block| TerminalBlockNaturalRank {
                    block: id(block),
                    value: id::<ValueId>(7_000),
                })
                .collect(),
            edges: vec![
                rank_edge(2, 2, 3, TerminalNaturalRankComparison::Preserving),
                rank_edge(4, 3, 2, TerminalNaturalRankComparison::Strict),
            ],
        }]));
        semantic
    }

    #[test]
    fn natural_cycle_bound_replays_actual_operations_and_calls() {
        // walk calls callee (one ReturnUnit block = 1 unit) inside the
        // loop; caller invokes walk once from a straight-line block.
        let walk = cyclic_machine(32, vec![call_unit(10, 5)]);
        let callee = machine(5, 5, vec![block(5, Vec::new(), return_unit(6))], None);
        let caller = machine(
            6,
            6,
            vec![block(6, vec![call_unit(11, 1)], return_unit(7))],
            None,
        );
        let module = module(6, vec![walk, callee, caller]);

        // Member visits: header 1 (conditional edge) + work (1 call op +
        // 1 callee unit + 1 jump edge) = 4. Iterations: u32 rank admits at
        // most 2^32 member visits.
        let component = 4_u128 * (u128::from(u32::MAX) + 1);
        let walk_bound = 1 + component + 1; // entry edge + component + exit edge
        assert_eq!(
            derive_maximum_entry_bound(&module, id(1)),
            Ok(u64::try_from(walk_bound).expect("fits u64"))
        );
        // The caller composes the callee's bound into its own call.
        assert_eq!(
            derive_maximum_entry_bound(&module, id(6)),
            Ok(u64::try_from(2 + walk_bound).expect("fits u64"))
        );
    }

    #[test]
    fn natural_cycle_with_wide_rank_reports_bound_overflow() {
        let walk = cyclic_machine(64, Vec::new());
        let module = module(1, vec![walk]);
        assert_eq!(
            derive_maximum_entry_bound(&module, id(1)),
            Err(FixedFuelError::BoundOverflow)
        );
    }

    #[test]
    fn natural_cycle_calling_itself_still_reports_call_cycle() {
        let walk = cyclic_machine(32, vec![call_unit(10, 1)]);
        let module = module(1, vec![walk]);
        assert_eq!(
            derive_maximum_entry_bound(&module, id(1)),
            Err(FixedFuelError::CallCycle(id(1)))
        );
    }

    /// An unranked cycle reports the verifier-derived component identity and
    /// the `Unranked` cause — not whichever block the traversal revisited —
    /// and the topology-derived name equals the identity a producer ranking
    /// row for that same component would carry.
    #[test]
    fn unranked_cycle_reports_component_identity_and_cause() {
        let mut walk = cyclic_machine(32, Vec::new());
        walk.ranked_scc = None;
        let component = terminal_verifier::cyclic_component_identity(&walk, &[id(2), id(3)]);
        let module = module(1, vec![walk]);
        assert_eq!(
            derive_maximum_entry_bound(&module, id(1)),
            Err(FixedFuelError::UnboundedCycleComponent {
                component,
                cause: crate::UnboundedCycleCause::Unranked,
            })
        );
    }

    /// The topology-derived identity is the canonical name: on a ranked
    /// machine it equals the producer row's `control_cycle_identity`, so an
    /// unranked component's report already names what ranking it would join.
    #[test]
    fn topology_derived_identity_matches_producer_identity() {
        let walk = cyclic_machine(32, Vec::new());
        let Some(TerminalRankedScc::Natural(components)) = &walk.ranked_scc else {
            panic!("cyclic machine is Natural-ranked");
        };
        let members: Vec<BlockId> = components
            .first()
            .expect("one component")
            .ranks
            .iter()
            .map(|rank| rank.block)
            .collect();
        assert_eq!(
            terminal_verifier::cyclic_component_identity(&walk, &members),
            terminal_verifier::control_cycle_identity(&walk, components.first().expect("one")),
        );
    }

    /// A descriptor-dispatched Unit call inside a cyclic member composes its
    /// realization's bound into every member visit, exactly like a direct
    /// call — the `rank_maximum + 1` multiplier covers it per iteration.
    #[test]
    fn natural_cycle_bound_counts_dynamic_unit_realization_per_visit() {
        let walk = cyclic_machine(32, vec![dynamic_unit_call(10, 0)]);
        let realization = machine(5, 5, vec![block(5, Vec::new(), return_unit(6))], None);
        let mut module = module(1, vec![walk, realization]);
        module.dynamic_dispatch.indirect_dispatches = vec![indirect_dispatch(1, 10, 0, 5)];

        // Same accounting as the direct-call case: header edge 1 plus work
        // (1 call op + 1 callee unit + 1 jump edge) per member visit.
        let component = 4_u128 * (u128::from(u32::MAX) + 1);
        let walk_bound = 1 + component + 1;
        assert_eq!(
            derive_maximum_entry_bound(&module, id(1)),
            Ok(u64::try_from(walk_bound).expect("fits u64"))
        );
    }

    /// A stored-descriptor dispatch names the same kind of in-module
    /// realization an indirect row does: its bound composes per call site.
    #[test]
    fn stored_dispatch_realization_contributes_its_bound() {
        let caller = machine(
            1,
            1,
            vec![block(1, vec![dynamic_scalar_call(10, 0)], return_unit(2))],
            None,
        );
        let realization =
            scalar_machine(5, 5, vec![block(5, Vec::new(), return_scalar(6, 20_005))]);
        let mut module = module(1, vec![caller, realization]);
        module.dynamic_dispatch.stored_dispatches = vec![stored_dispatch(1, 10, 0, 5)];

        // 1 call op + realization bound 1 (its Return edge) + ReturnUnit edge 1.
        assert_eq!(derive_maximum_entry_bound(&module, id(1)), Ok(3));
    }

    /// A descriptor call without its dispatch row has no realization to
    /// bound. That is a broken semantic invariant, so derivation fails closed
    /// instead of certifying zero callee work.
    #[test]
    fn descriptor_call_without_dispatch_row_rejects() {
        let walk = cyclic_machine(32, vec![dynamic_unit_call(10, 0)]);
        let module = module(1, vec![walk]);
        assert_eq!(
            derive_maximum_entry_bound(&module, id(1)),
            Err(FixedFuelError::MissingDynamicDispatch {
                owner: id(1),
                operation: id(10),
            })
        );
    }

    /// A dynamic-parameter call receives its callee from the invocation's
    /// descriptor table: the realization set is open, so no fixed ceiling can
    /// cover it and derivation rejects rather than under-approximates. On an
    /// acyclic block there is no component to cite, so the flat operation
    /// report is already the precise absence-of-bound cause.
    #[test]
    fn dynamic_parameter_call_rejects_as_invocation_bound() {
        let walk = machine(
            1,
            1,
            vec![block(1, vec![parameter_unit_call(10)], return_unit(2))],
            None,
        );
        let module = module(1, vec![walk]);
        assert_eq!(
            derive_maximum_entry_bound(&module, id(1)),
            Err(FixedFuelError::InvocationBoundCallee {
                owner: id(1),
                operation: id(10),
            })
        );
    }

    /// An installed boundary call dispatches to whichever checked candidate
    /// the admitted installation binds, so the call charge is the maximum
    /// candidate bound — not the sum and not zero.
    #[test]
    fn boundary_call_composes_the_maximum_candidate_bound() {
        let caller = machine(
            1,
            1,
            vec![block(1, vec![boundary_call(10, 7)], return_unit(2))],
            None,
        );
        let small = machine(5, 5, vec![block(5, Vec::new(), return_unit(6))], None);
        let large = machine(
            6,
            6,
            vec![block(6, vec![integer_constant(11, 12, 0)], return_unit(8))],
            None,
        );
        let mut module = module(1, vec![caller, small, large]);
        module.provider_candidates = vec![provider_candidate(7, 5), provider_candidate(7, 6)];

        // 1 call op + max(candidate bounds 1 and 2) + ReturnUnit edge 1 = 4.
        assert_eq!(derive_maximum_entry_bound(&module, id(1)), Ok(4));
    }

    /// A boundary call with no retained provider candidates completes
    /// through an external handler: it charges only its own operation unit.
    #[test]
    fn boundary_call_without_candidates_charges_only_the_operation() {
        let caller = machine(
            1,
            1,
            vec![block(1, vec![boundary_call(10, 7)], return_unit(2))],
            None,
        );
        let module = module(1, vec![caller]);

        assert_eq!(derive_maximum_entry_bound(&module, id(1)), Ok(2));
    }

    #[test]
    fn complete_catalog_reuses_preparation_but_replays_callee_outcomes() {
        use super::super::outcome_bounds::MACHINE_DERIVATIONS;
        use super::super::segment_partition::{IDENTITIES, PREPARATIONS};

        let caller = machine(
            1,
            1,
            vec![
                block(1, vec![call_unit(1, 2)], jump(1, 2)),
                block(2, vec![call_unit(2, 2)], return_unit(2)),
            ],
            None,
        );
        let callee = machine(2, 3, vec![block(3, Vec::new(), return_unit(3))], None);
        let module = module(1, vec![caller, callee]);
        let verified = terminal_verifier::verify_module(
            &module,
            &terminal_verifier::ProofBundle::default(),
            &proof_admission::AdmissionProfile::default(),
        )
        .expect("repeated unit calls verify");
        PREPARATIONS.with(|count| count.set(0));
        IDENTITIES.with(|count| count.set(0));
        MACHINE_DERIVATIONS.with(|count| count.set(0));

        let catalog = derive_validated_fixed_safe_point_segments(&verified, id(1))
            .expect("complete catalog derives and replays");
        assert_eq!(
            PREPARATIONS.with(std::cell::Cell::get),
            1,
            "one identity and lookup preparation for derivation plus sealing replay"
        );
        assert_eq!(IDENTITIES.with(std::cell::Cell::get), 1);
        assert_eq!(
            MACHINE_DERIVATIONS.with(std::cell::Cell::get),
            2,
            "callee derived once per roster, with fresh replay working state"
        );
        assert_eq!(
            catalog
                .certificates
                .iter()
                .map(|row| row.ceiling_units)
                .collect::<Vec<_>>(),
            vec![3, 3]
        );

        validate_retained_fixed_safe_point_segments(&verified, &catalog)
            .expect("external replay prepares its own subject");
        assert_eq!(PREPARATIONS.with(std::cell::Cell::get), 2);
        assert_eq!(IDENTITIES.with(std::cell::Cell::get), 2);
        assert_eq!(MACHINE_DERIVATIONS.with(std::cell::Cell::get), 3);

        let independently_derived = [
            derive_fixed_segment_fuel(&verified, id(1), id(1), id(1)).expect("first segment"),
            derive_fixed_segment_fuel(&verified, id(1), id(2), id(2)).expect("second segment"),
        ];
        assert_eq!(catalog.certificates, independently_derived);
        for field in 0..7 {
            let mut tampered = catalog.certificates.clone();
            match field {
                0 => tampered[0].terminal_psi = identity(99),
                1 => {
                    tampered[0].schedule = FuelScheduleIdentity::new(99).expect("nonzero schedule")
                }
                2 => tampered[0].machine = id(2),
                3 => tampered[0].start_block = id(2),
                4 => tampered[0].end_edge = id(2),
                5 => tampered[0].relevant_preconditions = vec![Proposition::Truth],
                _ => tampered[0].ceiling_units += 1,
            }
            assert_eq!(
                retain_validated_fixed_safe_point_segments(&verified, id(1), tampered),
                Err(FixedFuelError::CertificateMismatch),
                "whole-roster replay binds certificate field {field}"
            );
        }
    }

    /// A component that contains the machine entry still amplifies each member
    /// by the full rank budget and composes the exit edge.
    #[test]
    fn natural_cycle_entry_block_inside_component_amplifies() {
        // entry = 2 (the header): 2 cond -> 3 or exit 4; 3 work -> 2 strict.
        let mut m = machine(
            1,
            2,
            vec![
                block(2, Vec::new(), conditional(2, 3, 3, 4)),
                block(3, vec![integer_constant(10, 11, 0)], jump(4, 2)),
                block(4, Vec::new(), return_unit(5)),
            ],
            None,
        );
        m.ranked_scc = Some(TerminalRankedScc::Natural(vec![TerminalNaturalCycle {
            rank_type: IntegerType::new(IntegerSign::Unsigned, 8).expect("u8"),
            ranks: [2, 3]
                .into_iter()
                .map(|block| TerminalBlockNaturalRank {
                    block: id(block),
                    value: id::<ValueId>(7_000),
                })
                .collect(),
            edges: vec![
                rank_edge(2, 2, 3, TerminalNaturalRankComparison::Preserving),
                rank_edge(4, 3, 2, TerminalNaturalRankComparison::Strict),
            ],
        }]));
        let module = module(1, vec![m]);
        // member_units = visit2(1) + visit3(1 op + 1 jump) = 3
        // component = 3 * (255+1) = 768; bound = 768 + exit return edge (1).
        assert_eq!(derive_maximum_entry_bound(&module, id(1)), Ok(769));
    }

    /// Two disjoint components sequence through the condensed control DAG.
    #[test]
    fn natural_cycle_two_components_compose() {
        // entry 1 -> component A {2,3} -> mid 9 -> component B {5,6} -> exit 7
        let mut m = machine(
            1,
            1,
            vec![
                block(1, Vec::new(), jump(1, 2)),
                block(2, Vec::new(), conditional(2, 3, 3, 9)),
                block(3, vec![integer_constant(10, 11, 0)], jump(4, 2)),
                block(9, Vec::new(), jump(9, 5)),
                block(5, Vec::new(), conditional(5, 6, 6, 7)),
                block(6, vec![integer_constant(12, 13, 0)], jump(6, 5)),
                block(7, Vec::new(), return_unit(7)),
            ],
            None,
        );
        let comp = |members: [u64; 2], edges: Vec<TerminalNaturalRankEdge>| TerminalNaturalCycle {
            rank_type: IntegerType::new(IntegerSign::Unsigned, 8).expect("u8"),
            ranks: members
                .into_iter()
                .map(|block| TerminalBlockNaturalRank {
                    block: id(block),
                    value: id::<ValueId>(7_000),
                })
                .collect(),
            edges,
        };
        m.ranked_scc = Some(TerminalRankedScc::Natural(vec![
            comp(
                [2, 3],
                vec![
                    rank_edge(2, 2, 3, TerminalNaturalRankComparison::Preserving),
                    rank_edge(4, 3, 2, TerminalNaturalRankComparison::Strict),
                ],
            ),
            comp(
                [5, 6],
                vec![
                    rank_edge(5, 5, 6, TerminalNaturalRankComparison::Preserving),
                    rank_edge(6, 6, 5, TerminalNaturalRankComparison::Strict),
                ],
            ),
        ]));
        let module = module(1, vec![m]);
        // each component member_units = visit_cond(1) + visit_work(1 op + 1 jump) = 3
        // component = 3 * (255+1) = 768
        // bound = 1 (entry) + 768 + 1 (mid jump) + 768 + 1 (exit return) = 1539.
        assert_eq!(derive_maximum_entry_bound(&module, id(1)), Ok(1539));
    }

    /// A boundary call inside a cyclic member composes the maximum provider
    /// candidate bound, amplified per visit.
    #[test]
    fn natural_cycle_boundary_call_amplifies_candidate_maximum() {
        let walk = cyclic_machine(32, vec![boundary_call(10, 7)]);
        let small = machine(5, 5, vec![block(5, Vec::new(), return_unit(6))], None);
        let large = machine(
            6,
            6,
            vec![block(6, vec![integer_constant(11, 12, 0)], return_unit(8))],
            None,
        );
        let mut module = module(1, vec![walk, small, large]);
        module.provider_candidates = vec![provider_candidate(7, 5), provider_candidate(7, 6)];
        // visit3 = 1 boundary op + max(candidate5=1, candidate6=2) + 1 jump = 4;
        // member_units = 1 + 4 = 5; component = 5 * (2^32) ; bound = 1+5*2^32+1.
        let expected = 1 + 5 * (u64::from(u32::MAX) + 1) + 1;
        assert_eq!(derive_maximum_entry_bound(&module, id(1)), Ok(expected));
    }

    /// A cyclic callee's own amplified bound composes into a cyclic caller's
    /// per-visit work, which is itself amplified.
    #[test]
    fn natural_cycle_callee_amplifies_into_caller() {
        let mut inner = cyclic_machine(8, Vec::new());
        inner.id = id(5);
        let walk = cyclic_machine(8, vec![call_unit(10, 5)]);
        let module = module(1, vec![walk, inner]);
        // inner: member_units = visit2(1) + visit3(0 ops + 1 jump) = 2;
        // component = 2*256 = 512; inner bound = 1 + 512 + 1 = 514.
        assert_eq!(derive_maximum_entry_bound(&module, id(5)), Ok(514));
        // walk visit3 = 1 call + inner(514) + 1 jump = 516;
        // member_units = 1 + 516 = 517; component = 517*256 = 132352;
        // bound = 1 + 132352 + 1 = 132354.
        assert_eq!(derive_maximum_entry_bound(&module, id(1)), Ok(132354));
    }

    /// A callee that can only crash still bounds every crash-terminal walk:
    /// member 3's dead traversal removes the only cycle header 2 could be
    /// re-entered through, so the completing interior charges header 2
    /// once, and member 3's crashing visit commits once — its call
    /// operation plus the callee's crash edge — rather than at the rank
    /// multiplier the crash-inclusive ceiling billed.
    #[test]
    fn natural_cycle_crash_only_callee_still_bounds_crash() {
        // callee 5 always crashes (returned: None, crashed: Some(1)).
        let mut callee = machine(5, 5, vec![], None);
        callee.blocks = vec![block(
            5,
            Vec::new(),
            Terminator::Crash {
                edge: id(50),
                cause: terminal_psi::CrashCause::Trap,
                site_guard: Vec::new(),
                frontier_lower_bound: Vec::new(),
            },
        )];
        callee.contract.crash_routes = Vec::new();
        let walk = cyclic_machine(32, vec![call_unit(10, 5)]);
        let module = module(1, vec![walk, callee]);
        // interior = member2 visit (1) crossed once — its strict back-edge
        // rides member 3's dead traversal, so no surviving cycle can
        // re-enter it; crash visit = member3's call op (1) + callee crash
        // edge (1) = 2; bound = 1 entry edge + 1 + 2.
        assert_eq!(derive_maximum_entry_bound(&module, id(1)), Ok(1 + 1 + 2));
    }

    /// An invocation-bound dynamic-parameter call inside a cyclic member
    /// reports the verifier-derived component identity and the `OpenCalleeSet`
    /// cause — the responsible edge plus its cyclic component — rather than a
    /// flat operation report that drops where closure failed.
    #[test]
    fn natural_cycle_dynamic_parameter_call_rejects() {
        let walk = cyclic_machine(32, vec![parameter_unit_call(10)]);
        let component = terminal_verifier::cyclic_component_identity(&walk, &[id(2), id(3)]);
        let module = module(1, vec![walk]);
        assert_eq!(
            derive_maximum_entry_bound(&module, id(1)),
            Err(FixedFuelError::UnboundedCycleComponent {
                component,
                cause: crate::UnboundedCycleCause::OpenCalleeSet { operation: id(10) },
            })
        );
    }

    /// A multi-block segment whose interior crosses a conditional needs no
    /// successor selection: the bound is the maximum charge over the arms
    /// that can commit the endpoint, matching the entry bound's rule.
    #[test]
    fn segment_bound_crosses_a_conditional_at_the_maximum_arm() {
        // 1: [bconst] cond -> 2 | 3; 2: [iconst] jump -> 4; 3: jump -> 4;
        // 4: return edge 40.
        let walker = machine(
            1,
            1,
            vec![
                block(
                    1,
                    vec![boolean_constant(8, 9_000, true)],
                    conditional(10, 2, 11, 3),
                ),
                block(2, vec![integer_constant(12, 13, 0)], jump(20, 4)),
                block(3, Vec::new(), jump(21, 4)),
                block(4, Vec::new(), return_unit(40)),
            ],
            None,
        );
        let module = module(1, vec![walker]);
        let verified = terminal_verifier::verify_module(
            &module,
            &terminal_verifier::ProofBundle::default(),
            &proof_admission::AdmissionProfile::default(),
        )
        .expect("diamond machine verifies");

        let certificate = derive_fixed_segment_fuel(&verified, id(1), id(1), id(40))
            .expect("a conditional interior composes the maximum arm");
        // bconst 1 + cond edge 1 + max(arm 2: iconst+jump 2, arm 3: jump 1)
        // + return edge 1 = 5.
        assert_eq!(certificate.ceiling_units, 5);
        validate_fixed_segment_fuel(&verified, &certificate)
            .expect("independent replay reaches the same bound");
    }

    /// An arm that leaves the machine without committing the endpoint does
    /// not bound this segment: that execution is covered by its own terminal
    /// edge's certificate, so this bound follows the reaching arm alone.
    #[test]
    fn segment_bound_excludes_arms_that_never_commit_the_endpoint() {
        // 1: [bconst] cond -> 2 | 3; 2: jump -> 4; 3: return edge 30;
        // 4: return edge 40.
        let walker = machine(
            1,
            1,
            vec![
                block(
                    1,
                    vec![boolean_constant(8, 9_000, true)],
                    conditional(10, 2, 11, 3),
                ),
                block(2, Vec::new(), jump(20, 4)),
                block(3, Vec::new(), return_unit(30)),
                block(4, Vec::new(), return_unit(40)),
            ],
            None,
        );
        let module = module(1, vec![walker]);
        let verified = terminal_verifier::verify_module(
            &module,
            &terminal_verifier::ProofBundle::default(),
            &proof_admission::AdmissionProfile::default(),
        )
        .expect("early-return arm machine verifies");

        let certificate = derive_fixed_segment_fuel(&verified, id(1), id(1), id(40))
            .expect("the reaching arm still bounds the segment");
        // bconst 1 + cond edge 1 + arm 2 (jump 1 + return edge 1) = 4;
        // arm 3 returns through edge 30 and never commits edge 40.
        assert_eq!(certificate.ceiling_units, 4);
    }

    /// When no walk commits the endpoint, derivation reports the first
    /// terminal edge reached in traversal order — the same displacement the
    /// linear walk reported before arms existed.
    #[test]
    fn segment_with_no_reaching_walk_reports_the_first_terminal() {
        let walker = machine(
            1,
            1,
            vec![
                block(
                    1,
                    vec![boolean_constant(8, 9_000, true)],
                    conditional(10, 2, 11, 3),
                ),
                block(2, Vec::new(), return_unit(20)),
                block(3, Vec::new(), return_unit(30)),
            ],
            None,
        );
        let module = module(1, vec![walker]);
        let verified = terminal_verifier::verify_module(
            &module,
            &terminal_verifier::ProofBundle::default(),
            &proof_admission::AdmissionProfile::default(),
        )
        .expect("two-return machine verifies");

        assert_eq!(
            derive_fixed_segment_fuel(&verified, id(1), id(1), id(99)),
            Err(FixedFuelError::SegmentEndNotReached {
                requested: id(99),
                reached_terminal: id(20),
            })
        );
    }

    /// An arm looping back into the walk is a real cycle, not a bounded
    /// segment; derivation still fails closed rather than overcounting
    /// iterations. Uses the internal surface because an unranked cycle never
    /// reaches verification.
    #[test]
    fn segment_interior_cycle_reports_control_cycle() {
        let walker = machine(
            1,
            1,
            vec![
                block(
                    1,
                    vec![boolean_constant(8, 9_000, true)],
                    conditional(10, 2, 11, 3),
                ),
                block(2, Vec::new(), jump(20, 1)),
                block(3, Vec::new(), jump(21, 4)),
                block(4, Vec::new(), return_unit(40)),
            ],
            None,
        );
        let component = terminal_verifier::cyclic_component_identity(&walker, &[id(1), id(2)]);
        let module = module(1, vec![walker]);
        let subject = PreparedFuelModule::new(&module);
        let prepared = PreparedSegments::new(&subject, id(1)).expect("machine prepares");
        assert_eq!(
            prepared.segment_certificate(id(1), id(40), &mut BTreeMap::new()),
            Err(FixedFuelError::UnboundedCycleComponent {
                component,
                cause: crate::UnboundedCycleCause::Unranked,
            })
        );
    }

    /// The safe-point catalog is unchanged by interior traversal: it still
    /// emits one row per reachable block terminator edge, in canonical order.
    #[test]
    fn branched_catalog_still_partitions_at_every_reachable_edge() {
        let walker = machine(
            1,
            1,
            vec![
                block(
                    1,
                    vec![boolean_constant(8, 9_000, true)],
                    conditional(10, 2, 11, 3),
                ),
                block(2, vec![integer_constant(12, 13, 0)], jump(20, 4)),
                block(3, Vec::new(), jump(21, 4)),
                block(4, Vec::new(), return_unit(40)),
            ],
            None,
        );
        let module = module(1, vec![walker]);
        let verified = terminal_verifier::verify_module(
            &module,
            &terminal_verifier::ProofBundle::default(),
            &proof_admission::AdmissionProfile::default(),
        )
        .expect("diamond machine verifies");

        let rows = derive_fixed_safe_point_segments(&verified, id(1))
            .expect("complete catalog derives over a diamond");
        assert_eq!(
            rows.iter()
                .map(|row| (row.start_block, row.end_edge, row.ceiling_units))
                .collect::<Vec<_>>(),
            vec![
                (id(1), id(10), 2),
                (id(1), id(11), 2),
                (id(2), id(20), 2),
                (id(3), id(21), 1),
                (id(4), id(40), 1),
            ]
        );
    }

    /// A `StructuralCase` member's multi-way branch amplifies inside the
    /// component and its exits route through the condensed DAG.
    #[test]
    fn natural_cycle_structural_case_member_branches() {
        // component {2,3,8}: 2 cond -> 3 or exit 4; 3 case -> 8 (either case);
        // 8 jump -> 2 (strict).
        let mut m = machine(
            1,
            1,
            vec![
                block(1, Vec::new(), jump(1, 2)),
                block(2, Vec::new(), conditional(2, 3, 3, 4)),
                block(
                    3,
                    vec![integer_constant(10, 11, 0)],
                    Terminator::StructuralCase {
                        source: id(60),
                        cases: vec![
                            terminal_psi::StructuralCaseSuccessorEdge {
                                edge: id(30),
                                target: id(8),
                                case: id(70),
                                payload_fields: Vec::new(),
                                trivial_affine_discards: Vec::new(),
                            },
                            terminal_psi::StructuralCaseSuccessorEdge {
                                edge: id(31),
                                target: id(8),
                                case: id(71),
                                payload_fields: Vec::new(),
                                trivial_affine_discards: Vec::new(),
                            },
                        ],
                    },
                ),
                block(8, Vec::new(), jump(8, 2)),
                block(4, Vec::new(), return_unit(5)),
            ],
            None,
        );
        m.ranked_scc = Some(TerminalRankedScc::Natural(vec![TerminalNaturalCycle {
            rank_type: IntegerType::new(IntegerSign::Unsigned, 8).expect("u8"),
            ranks: [2, 3, 8]
                .into_iter()
                .map(|block| TerminalBlockNaturalRank {
                    block: id(block),
                    value: id::<ValueId>(7_000),
                })
                .collect(),
            edges: vec![
                rank_edge(2, 2, 3, TerminalNaturalRankComparison::Preserving),
                rank_edge(30, 3, 8, TerminalNaturalRankComparison::Preserving),
                rank_edge(31, 3, 8, TerminalNaturalRankComparison::Preserving),
                rank_edge(8, 8, 2, TerminalNaturalRankComparison::Strict),
            ],
        }]));
        let module = module(1, vec![m]);
        // members: 2 (cond 1), 3 (1 op + case 1), 8 (jump 1) = 1+2+1 = 4
        // component = 4 * 256 = 1024; bound = 1 + 1024 + 1 = 1026.
        assert_eq!(derive_maximum_entry_bound(&module, id(1)), Ok(1026));
    }

    /// A codec-valid `Natural` countdown: entry 1 passes machine parameter
    /// `initial` into header 2's rank parameter `rank`; 2 conditionally enters
    /// work 3 (preserving) or exits to return block 4; 3 passes the measured
    /// `next` value back to 2 (strict descent). Rank values are real
    /// declarations so the semantic identity validation admits the module.
    fn ranked_countdown_machine(rank_bits: u16) -> TerminalMachine {
        let rank_type =
            IntegerType::new(IntegerSign::Unsigned, rank_bits).expect("fixed unsigned rank");
        let scalar = ScalarType::Integer(rank_type);
        let value = |raw: u64| ValueDeclaration {
            qualifications: Default::default(),
            id: id(raw),
            scalar_type: scalar,
        };
        let rank_constant = |operation: u64, result: u64, value: u64| Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: id(operation),
            result: OperationResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: id(result),
                scalar_type: scalar,
            }),
            kind: OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(u128::from(value)),
            },
        };
        let jump_with = |edge: u64, target: u64, arguments: Vec<ValueId>| Terminator::Jump {
            edge: id(edge),
            target: id(target),
            arguments,
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
            residual_affine_discards: Vec::new(),
        };
        let mut semantic = machine(
            1,
            1,
            vec![
                block(1, Vec::new(), jump_with(1, 2, vec![id(100)])),
                Block {
                    erased_scalar_formals: Vec::new(),
                    erased_proof_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: id(2),
                    parameters: vec![value(200)],
                    operations: vec![boolean_constant(20, 9_000, true)],
                    terminator: conditional(2, 3, 3, 4),
                },
                block(
                    3,
                    vec![rank_constant(30, 300, 0)],
                    jump_with(4, 2, vec![id(300)]),
                ),
                block(4, Vec::new(), return_unit(5)),
            ],
            Some(TerminalRankedScc::Natural(vec![TerminalNaturalCycle {
                rank_type,
                ranks: [2, 3]
                    .into_iter()
                    .map(|block| TerminalBlockNaturalRank {
                        block: id(block),
                        value: id(200),
                    })
                    .collect(),
                edges: vec![
                    TerminalNaturalRankEdge {
                        edge: id(2),
                        source: id(2),
                        target: id(3),
                        successor_rank: id(200),
                        comparison: TerminalNaturalRankComparison::Preserving,
                    },
                    TerminalNaturalRankEdge {
                        edge: id(4),
                        source: id(3),
                        target: id(2),
                        successor_rank: id(300),
                        comparison: TerminalNaturalRankComparison::Strict,
                    },
                ],
            }])),
        );
        semantic.parameters = vec![value(100)];
        semantic
    }

    /// A safe-point row whose endpoint rides the start block's own terminator
    /// is one traversal of that block: a covered backedge or loop exit is an
    /// ordinary per-traversal row, not authority to charge the
    /// rank-multiplied component bound that whole-entry composition uses.
    /// Uses the internal surface because the verifier requires discharged
    /// rank obligations that a hand-built module cannot carry.
    #[test]
    fn natural_cycle_safe_point_catalog_charges_single_edge_traversals() {
        // Per-traversal charges: 1 = jump, 2 = bconst + conditional,
        // 3 = iconst + jump, 4 = return. The u32 rank amplifies whole-entry
        // composition only — the catalog rows stay at each block's own visit.
        let module = module(1, vec![ranked_countdown_machine(32)]);
        let subject = PreparedFuelModule::new(&module);
        let prepared = PreparedSegments::new(&subject, id(1)).expect("machine prepares");
        let rows = prepared.derive_catalog().expect("catalog derives");
        assert_eq!(
            rows.iter()
                .map(|row| (row.start_block, row.end_edge, row.ceiling_units))
                .collect::<Vec<_>>(),
            vec![
                (id(1), id(1), 1),
                (id(2), id(2), 2),
                (id(2), id(3), 2),
                (id(3), id(4), 2),
                (id(4), id(5), 1),
            ]
        );
    }

    /// A u64 rank bound overflows the whole-entry certificate but cannot
    /// overflow a per-traversal row: the catalog still derives and every
    /// ceiling stays at the single-block traversal charge.
    #[test]
    fn natural_cycle_u64_catalog_stays_per_traversal_when_entry_overflows() {
        let module = module(1, vec![ranked_countdown_machine(64)]);
        assert_eq!(
            derive_maximum_entry_bound(&module, id(1)),
            Err(FixedFuelError::BoundOverflow)
        );
        let subject = PreparedFuelModule::new(&module);
        let prepared = PreparedSegments::new(&subject, id(1)).expect("machine prepares");
        let rows = prepared
            .derive_catalog()
            .expect("per-edge segments stay within the u64 schedule");
        assert_eq!(
            rows.iter().map(|row| row.ceiling_units).collect::<Vec<_>>(),
            vec![1, 2, 2, 2, 1]
        );
    }

    /// A mid-component segment whose endpoint commits on a member terminator
    /// still bounds every walk that commits it — but when the component's
    /// non-end-edge internal graph reaching that member is acyclic, every
    /// such walk is a single interior pass and the bound is its member-visit
    /// sum, not the rank-multiplied whole-component charge. In the countdown
    /// component {2,3}: segment (2, edge 4) is exactly 2 then 3; segment
    /// (3, edge 2) is exactly 3 then 2.
    #[test]
    fn natural_cycle_mid_component_segment_bounds_the_interior_pass() {
        let module = module(1, vec![ranked_countdown_machine(8)]);
        let subject = PreparedFuelModule::new(&module);
        let prepared = PreparedSegments::new(&subject, id(1)).expect("machine prepares");
        assert_eq!(
            prepared
                .segment_certificate(id(2), id(4), &mut BTreeMap::new())
                .expect("header-to-backedge segment derives")
                .ceiling_units,
            4,
            "visit(2) + visit(3), not 256 * member_units"
        );
        assert_eq!(
            prepared
                .segment_certificate(id(3), id(2), &mut BTreeMap::new())
                .expect("work-to-header-edge segment derives")
                .ceiling_units,
            4,
            "visit(3) + visit(2), not 256 * member_units"
        );
    }

    /// The same interior-pass bound applies when the walk reaches the
    /// component from an ordinary predecessor: entry 1 jumps into header 2,
    /// so segment (1, edge 4) charges visit(1) + visit(2) + visit(3).
    #[test]
    fn natural_cycle_entry_to_component_segment_bounds_the_interior_pass() {
        let module = module(1, vec![ranked_countdown_machine(8)]);
        let subject = PreparedFuelModule::new(&module);
        let prepared = PreparedSegments::new(&subject, id(1)).expect("machine prepares");
        assert_eq!(
            prepared
                .segment_certificate(id(1), id(4), &mut BTreeMap::new())
                .expect("entry-to-backedge segment derives")
                .ceiling_units,
            5,
            "visit(1) + visit(2) + visit(3), not 1 + 256 * member_units"
        );
    }

    /// The interior bound stays conservative when the endpoint's committing
    /// member can still be reached through a cycle that avoids the endpoint:
    /// a walk committing exit edge 3 may iterate 2 -> 3 -> 2 up to the rank
    /// carrier before leaving, so the segment keeps the rank-multiplied
    /// component charge rather than a single interior pass.
    #[test]
    fn natural_cycle_cyclic_interior_keeps_component_scale_bound() {
        let module = module(1, vec![ranked_countdown_machine(8)]);
        let subject = PreparedFuelModule::new(&module);
        let prepared = PreparedSegments::new(&subject, id(1)).expect("machine prepares");
        assert_eq!(
            prepared
                .segment_certificate(id(3), id(3), &mut BTreeMap::new())
                .expect("work-to-exit segment derives")
                .ceiling_units,
            4 * 256,
            "the surviving 2 -> 3 -> 2 cycle admits rank-bounded revisits"
        );
        assert_eq!(
            prepared
                .segment_certificate(id(1), id(3), &mut BTreeMap::new())
                .expect("entry-to-exit segment derives")
                .ceiling_units,
            1 + 4 * 256,
            "entry edge plus the rank-bounded interior"
        );
    }

    /// Scalar-term operands for contract `requires` clauses:
    /// `parameter_term` names a machine parameter at the rank carrier's
    /// type and `integer_literal` is a same-type unsigned literal — the
    /// only operand shapes an entry-rank ceiling recognizes.
    fn parameter_term(parameter: u64, rank_type: IntegerType) -> ScalarTerm {
        ScalarTerm::Value {
            id: id(parameter),
            scalar_type: ScalarType::Integer(rank_type),
        }
    }

    fn integer_literal(rank_type: IntegerType, value: u128) -> ScalarTerm {
        ScalarTerm::Integer {
            scalar_type: rank_type,
            value: IntegerValue::Unsigned(value),
        }
    }

    /// A jump carrying explicit scalar arguments — the shape an entry edge
    /// needs to bind a component member's rank parameter.
    fn jump_arguments(edge: u64, target: u64, arguments: Vec<ValueId>) -> Terminator {
        Terminator::Jump {
            edge: id(edge),
            target: id(target),
            arguments,
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
            residual_affine_discards: Vec::new(),
        }
    }

    /// A `requires` clause capping the machine parameter that supplies the
    /// component's initial rank tightens the visit multiplier below the
    /// carrier maximum: `initial <= 5` admits at most six rank
    /// observations at the {2,3} countdown's first entry, so its four
    /// member-visit units bill six traversals rather than 256 — and the
    /// certificate binds the exact clause row as its premise.
    #[test]
    fn contract_requires_bound_tightens_the_rank_visit_ceiling() {
        let rank_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let mut walk = ranked_countdown_machine(8);
        let clause = Proposition::LessOrEqual(
            parameter_term(100, rank_type),
            integer_literal(rank_type, 5),
        );
        walk.contract.requires = vec![clause.clone()];
        let tightened = module(1, vec![walk]);
        // member visits: header 2 (bconst + conditional) + work 3
        // (iconst + jump) = 4; the clause admits 6 visits -> 4 * 6 = 24;
        // bound = entry edge + 24 + exit edge.
        assert_eq!(
            derive_maximum_entry_bound(&tightened, id(1)),
            Ok(1 + 4 * 6 + 1)
        );
        assert_eq!(
            used_contract_premises(tightened.machines.first().expect("one machine")),
            vec![clause],
            "the certificate binds exactly the consulted premise"
        );

        // The same clause rescues a carrier whose type maximum cannot fit
        // the u64 ceiling at all: a u64 rank overflows without it.
        let wide = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
        let mut walk = ranked_countdown_machine(64);
        walk.contract.requires = vec![Proposition::LessOrEqual(
            parameter_term(100, wide),
            integer_literal(wide, 5),
        )];
        let rescued = module(1, vec![walk]);
        assert_eq!(
            derive_maximum_entry_bound(&rescued, id(1)),
            Ok(1 + 4 * 6 + 1),
            "the tightened rank ceiling bounds what the carrier's own \
             maximum could not"
        );
    }

    /// `p < k` binds `k - 1` and `p == k` binds `k` — including the
    /// literal-on-the-left equality form — while an unsatisfiable
    /// `p < 0` places no usable ceiling and leaves the carrier maximum
    /// standing rather than fabricating a zero bound.
    #[test]
    fn contract_requires_strict_less_than_and_equality_forms_tighten() {
        let rank_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        for (clause, expected) in [
            (
                Proposition::LessThan(
                    parameter_term(100, rank_type),
                    integer_literal(rank_type, 5),
                ),
                1 + 4 * 5 + 1,
            ),
            (
                Proposition::Equal(
                    parameter_term(100, rank_type),
                    integer_literal(rank_type, 5),
                ),
                1 + 4 * 6 + 1,
            ),
            (
                Proposition::Equal(
                    integer_literal(rank_type, 5),
                    parameter_term(100, rank_type),
                ),
                1 + 4 * 6 + 1,
            ),
            (
                // `p < 1` admits exactly one rank observation.
                Proposition::LessThan(
                    parameter_term(100, rank_type),
                    integer_literal(rank_type, 1),
                ),
                1 + 4 + 1,
            ),
        ] {
            let mut walk = ranked_countdown_machine(8);
            walk.contract.requires = vec![clause];
            let module = module(1, vec![walk]);
            assert_eq!(derive_maximum_entry_bound(&module, id(1)), Ok(expected));
        }
        let mut walk = ranked_countdown_machine(8);
        walk.contract.requires = vec![Proposition::LessThan(
            parameter_term(100, rank_type),
            integer_literal(rank_type, 0),
        )];
        let module = module(1, vec![walk]);
        assert_eq!(
            derive_maximum_entry_bound(&module, id(1)),
            Ok(1 + 4 * 256 + 1),
            "an unsatisfiable strict bound keeps the carrier maximum"
        );
    }

    /// Only an unconditional literal ceiling binds: a disjunctive or
    /// implied ceiling is conditional, a literal on the left of `<=` is a
    /// lower bound, a wrong-typed or signed literal cannot cap the
    /// unsigned carrier, a clause naming another value binds nothing,
    /// and a clause that merely restates the carrier maximum adds no
    /// premise.
    #[test]
    fn contract_requires_non_ceiling_forms_do_not_tighten() {
        let rank_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let wide = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
        let clauses = [
            Proposition::Disjunction(vec![
                Proposition::LessOrEqual(
                    parameter_term(100, rank_type),
                    integer_literal(rank_type, 5),
                ),
                Proposition::Truth,
            ]),
            Proposition::Implication {
                premise: Box::new(Proposition::Truth),
                conclusion: Box::new(Proposition::LessOrEqual(
                    parameter_term(100, rank_type),
                    integer_literal(rank_type, 5),
                )),
            },
            Proposition::LessOrEqual(
                integer_literal(rank_type, 5),
                parameter_term(100, rank_type),
            ),
            Proposition::LessOrEqual(parameter_term(100, rank_type), integer_literal(wide, 5)),
            Proposition::LessOrEqual(
                parameter_term(100, rank_type),
                ScalarTerm::Integer {
                    scalar_type: rank_type,
                    value: IntegerValue::Signed(5),
                },
            ),
            Proposition::LessOrEqual(
                parameter_term(9_999, rank_type),
                integer_literal(rank_type, 5),
            ),
            Proposition::LessOrEqual(
                parameter_term(100, rank_type),
                integer_literal(rank_type, 255),
            ),
        ];
        for clause in clauses {
            let mut walk = ranked_countdown_machine(8);
            walk.contract.requires = vec![clause];
            let module = module(1, vec![walk]);
            assert_eq!(
                derive_maximum_entry_bound(&module, id(1)),
                Ok(1 + 4 * 256 + 1)
            );
            assert!(
                used_contract_premises(module.machines.first().expect("one machine")).is_empty(),
                "a clause the bound never rests on is not a premise"
            );
        }
    }

    /// A ceiling nested inside a `Conjunction` clause still binds —
    /// conjuncts are unconditional — and the certificate names the whole
    /// contract row the ceiling arrived under, in canonical order.
    #[test]
    fn contract_requires_conjunction_row_binds_verbatim() {
        let rank_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let conjunction = Proposition::Conjunction(vec![
            Proposition::LessOrEqual(
                parameter_term(100, rank_type),
                integer_literal(rank_type, 9),
            ),
            Proposition::LessOrEqual(
                parameter_term(100, rank_type),
                integer_literal(rank_type, 5),
            ),
        ]);
        let mut walk = ranked_countdown_machine(8);
        walk.contract.requires = vec![Proposition::Truth, conjunction.clone()];
        let module = module(1, vec![walk]);
        // The tightest conjunct wins: 4 * (5 + 1) = 24.
        assert_eq!(
            derive_maximum_entry_bound(&module, id(1)),
            Ok(1 + 4 * 6 + 1)
        );
        assert_eq!(
            used_contract_premises(module.machines.first().expect("one machine")),
            vec![conjunction],
            "the premise is the contract row, not its flattened conjunct"
        );
    }

    /// Every first-entry arrival must reduce to a clause-bound machine
    /// parameter: an entry edge passing a computed value or a second
    /// unbounded parameter, or one supplying no argument at the rank
    /// parameter's position, leaves the carrier maximum in place rather
    /// than guessing at the arrivals it could not bind.
    #[test]
    fn contract_requires_bound_needs_every_entry_arrival_covered() {
        let rank_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let clause = Proposition::LessOrEqual(
            parameter_term(100, rank_type),
            integer_literal(rank_type, 5),
        );

        // The entry edge passes a computed constant, not the parameter.
        let mut walk = ranked_countdown_machine(8);
        walk.contract.requires = vec![clause.clone()];
        walk.blocks[0].operations = vec![integer_constant(15, 500, 0)];
        walk.blocks[0].terminator = jump_arguments(1, 2, vec![id(500)]);
        let computed = module(1, vec![walk]);
        assert_eq!(
            derive_maximum_entry_bound(&computed, id(1)),
            Ok(2 + 4 * 256 + 1),
            "a computed arrival has no contract ceiling — the entry \
             block's extra operation is the only added charge"
        );
        assert!(used_contract_premises(computed.machines.first().expect("one machine")).is_empty());

        // The entry edge passes a second machine parameter no clause caps.
        let mut walk = ranked_countdown_machine(8);
        walk.contract.requires = vec![clause.clone()];
        walk.parameters.push(ValueDeclaration {
            qualifications: Default::default(),
            id: id(101),
            scalar_type: ScalarType::Integer(rank_type),
        });
        walk.blocks[0].terminator = jump_arguments(1, 2, vec![id(101)]);
        let unbounded = module(1, vec![walk]);
        assert_eq!(
            derive_maximum_entry_bound(&unbounded, id(1)),
            Ok(1 + 4 * 256 + 1),
            "an unbounded parameter arrival keeps the carrier maximum"
        );
        assert!(
            used_contract_premises(unbounded.machines.first().expect("one machine")).is_empty()
        );

        // The entry edge supplies no argument at the rank parameter's
        // position at all.
        let mut walk = ranked_countdown_machine(8);
        walk.contract.requires = vec![clause];
        walk.blocks[0].terminator = jump_arguments(1, 2, Vec::new());
        let missing = module(1, vec![walk]);
        assert_eq!(
            derive_maximum_entry_bound(&missing, id(1)),
            Ok(1 + 4 * 256 + 1),
            "an unbound argument position keeps the carrier maximum"
        );
        assert!(used_contract_premises(missing.machines.first().expect("one machine")).is_empty());
    }

    /// When the machine entry is itself a member, the arriving rank is the
    /// machine-parameter observation the rank row names — a contract
    /// ceiling on that parameter tightens the visit bound exactly as an
    /// edge arrival does.
    #[test]
    fn contract_requires_bound_applies_at_a_member_machine_entry() {
        let rank_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let mut walk = machine(
            1,
            2,
            vec![
                block(2, Vec::new(), conditional(2, 3, 3, 4)),
                block(3, vec![integer_constant(10, 11, 0)], jump(4, 2)),
                block(4, Vec::new(), return_unit(5)),
            ],
            None,
        );
        walk.parameters = vec![ValueDeclaration {
            qualifications: Default::default(),
            id: id(100),
            scalar_type: ScalarType::Integer(rank_type),
        }];
        walk.ranked_scc = Some(TerminalRankedScc::Natural(vec![TerminalNaturalCycle {
            rank_type,
            ranks: vec![
                TerminalBlockNaturalRank {
                    block: id(2),
                    value: id(100),
                },
                TerminalBlockNaturalRank {
                    block: id(3),
                    value: id(7_000),
                },
            ],
            edges: vec![
                rank_edge(2, 2, 3, TerminalNaturalRankComparison::Preserving),
                rank_edge(4, 3, 2, TerminalNaturalRankComparison::Strict),
            ],
        }]));
        let clause = Proposition::LessOrEqual(
            parameter_term(100, rank_type),
            integer_literal(rank_type, 5),
        );
        walk.contract.requires = vec![clause.clone()];
        let module = module(1, vec![walk]);
        // member_units = visit2(1) + visit3(1 op + 1 jump) = 3;
        // 3 * 6 = 18, plus the exit edge.
        assert_eq!(derive_maximum_entry_bound(&module, id(1)), Ok(19));
        assert_eq!(
            used_contract_premises(module.machines.first().expect("one machine")),
            vec![clause]
        );
    }

    /// A contract ceiling on a component the entry cannot reach is no
    /// premise of the whole-entry bound: the condensed derivation never
    /// visits it, so the certificate binds no clause.
    #[test]
    fn contract_premises_stay_off_unreachable_components() {
        let rank_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let mut walk = cyclic_machine(8, Vec::new());
        walk.parameters = vec![ValueDeclaration {
            qualifications: Default::default(),
            id: id(100),
            scalar_type: ScalarType::Integer(rank_type),
        }];
        walk.contract.requires = vec![Proposition::LessOrEqual(
            parameter_term(100, rank_type),
            integer_literal(rank_type, 5),
        )];
        // Unreachable blocks 8 -> 9 feed member 9's rank parameter the
        // boundable machine parameter, but nothing from entry reaches
        // them.
        walk.blocks
            .push(block(8, Vec::new(), jump_arguments(80, 9, vec![id(100)])));
        walk.blocks.push(Block {
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: id(9),
            parameters: vec![ValueDeclaration {
                qualifications: Default::default(),
                id: id(900),
                scalar_type: ScalarType::Integer(rank_type),
            }],
            operations: Vec::new(),
            terminator: jump_arguments(81, 9, vec![id(900)]),
        });
        let Some(TerminalRankedScc::Natural(components)) = &mut walk.ranked_scc else {
            unreachable!("cyclic_machine is Natural-ranked")
        };
        components.push(TerminalNaturalCycle {
            rank_type,
            ranks: vec![TerminalBlockNaturalRank {
                block: id(9),
                value: id(900),
            }],
            edges: vec![rank_edge(81, 9, 9, TerminalNaturalRankComparison::Strict)],
        });
        let module = module(1, vec![walk]);
        // member_units = visit2(1) + visit3(1) = 2 at the carrier maximum.
        assert_eq!(
            derive_maximum_entry_bound(&module, id(1)),
            Ok(1 + 2 * 256 + 1)
        );
        assert!(
            used_contract_premises(module.machines.first().expect("one machine")).is_empty(),
            "an unreachable component's clause is not a bound premise"
        );
    }

    /// A segment row binds only the clauses its own bound consulted: the
    /// endpoint on the start block's own terminator and the acyclic
    /// interior pass to a member's committing edge never touch the rank
    /// bound, while a surviving interior cycle bills the tightened
    /// ceiling and binds the clause.
    #[test]
    fn segment_certificate_binds_only_the_clauses_its_interior_consulted() {
        let rank_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let clause = Proposition::LessOrEqual(
            parameter_term(100, rank_type),
            integer_literal(rank_type, 5),
        );
        let mut walk = ranked_countdown_machine(8);
        walk.contract.requires = vec![clause.clone()];
        let module = module(1, vec![walk]);
        let subject = PreparedFuelModule::new(&module);
        let prepared = PreparedSegments::new(&subject, id(1)).expect("machine prepares");

        let per_traversal = prepared
            .segment_certificate(id(1), id(1), &mut BTreeMap::new())
            .expect("entry-edge row derives");
        assert_eq!(per_traversal.ceiling_units, 1);
        assert!(
            per_traversal.relevant_preconditions.is_empty(),
            "a per-traversal row consults no rank bound"
        );

        // Endpoint edge 4 rides member 3's terminator; excluding it leaves
        // 2 -> 3 acyclic, so the interior is one pass — no rank bound is
        // consulted and no premise is bound.
        let interior_pass = prepared
            .segment_certificate(id(1), id(4), &mut BTreeMap::new())
            .expect("entry-to-backedge row derives");
        assert_eq!(interior_pass.ceiling_units, 5);
        assert!(interior_pass.relevant_preconditions.is_empty());

        // Endpoint edge 3 exits through member 2 with the surviving
        // 2 -> 3 -> 2 cycle billed at the tightened rank ceiling.
        let cyclic = prepared
            .segment_certificate(id(1), id(3), &mut BTreeMap::new())
            .expect("entry-to-exit row derives");
        assert_eq!(cyclic.ceiling_units, 1 + 4 * 6);
        assert_eq!(cyclic.relevant_preconditions, vec![clause.clone()]);
        let mid_component = prepared
            .segment_certificate(id(3), id(3), &mut BTreeMap::new())
            .expect("mid-component cyclic row derives");
        assert_eq!(mid_component.ceiling_units, 4 * 6);
        assert_eq!(mid_component.relevant_preconditions, vec![clause]);
    }

    /// An acyclic machine consults no component rank bound, so its entry
    /// certificate binds no premise even when the contract carries
    /// `requires` clauses — and validation rejects a certificate that
    /// claims one.
    #[test]
    fn entry_certificate_on_acyclic_machine_binds_no_premises() {
        let rank_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let mut walk = machine(1, 1, vec![block(1, Vec::new(), return_unit(2))], None);
        walk.parameters = vec![ValueDeclaration {
            qualifications: Default::default(),
            id: id(100),
            scalar_type: ScalarType::Integer(rank_type),
        }];
        walk.contract.requires = vec![Proposition::LessOrEqual(
            parameter_term(100, rank_type),
            integer_literal(rank_type, 5),
        )];
        let module = module(1, vec![walk]);
        let verified = terminal_verifier::verify_module(
            &module,
            &terminal_verifier::ProofBundle::default(),
            &proof_admission::AdmissionProfile::default(),
        )
        .expect("acyclic machine with a requires clause verifies");
        let certificate =
            derive_fixed_entry_fuel(&verified, id(1)).expect("entry certificate derives");
        assert_eq!(certificate.ceiling_units, 1);
        assert!(certificate.relevant_preconditions.is_empty());
        validate_fixed_entry_fuel(&verified, &certificate)
            .expect("independent replay reaches the same certificate");
        let mut tampered = certificate.clone();
        tampered.relevant_preconditions = vec![Proposition::Truth];
        assert_eq!(
            validate_fixed_entry_fuel(&verified, &tampered),
            Err(FixedFuelError::CertificateMismatch),
            "a claimed premise the derivation never used must mismatch"
        );
    }

    /// A codec-valid `Natural` component with a once-only tail: entry 1
    /// passes machine parameter `initial` into header 2's rank parameter
    /// `rank`; 2 conditionally enters work 3 (preserving) or exits to
    /// return block 4; 3 either passes `next` back to 2 (strict) or
    /// forwards to tail 8 (preserving); 8 passes `last` back to 2
    /// (strict). Removing endpoint edge 6 leaves 2 -> 3 -> 2 cyclic while
    /// 8 cannot be re-entered, so interior walks visit 8 at most once.
    fn cyclic_tail_machine() -> TerminalMachine {
        let rank_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let scalar = ScalarType::Integer(rank_type);
        let value = |raw: u64| ValueDeclaration {
            qualifications: Default::default(),
            id: id(raw),
            scalar_type: scalar,
        };
        let rank_constant = |operation: u64, result: u64| Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: id(operation),
            result: OperationResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: id(result),
                scalar_type: scalar,
            }),
            kind: OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(0),
            },
        };
        let jump_with = |edge: u64, target: u64, arguments: Vec<ValueId>| Terminator::Jump {
            edge: id(edge),
            target: id(target),
            arguments,
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
            residual_affine_discards: Vec::new(),
        };
        let successor =
            |edge: u64, target: u64, arguments: Vec<ValueId>| terminal_psi::SuccessorEdge {
                edge: id::<EdgeId>(edge),
                target: id::<BlockId>(target),
                arguments,
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                structural_arguments: Vec::new(),
                trivial_affine_discards: Vec::new(),
            };
        let mut semantic = machine(
            1,
            1,
            vec![
                block(1, Vec::new(), jump_with(1, 2, vec![id(100)])),
                Block {
                    erased_scalar_formals: Vec::new(),
                    erased_proof_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: id(2),
                    parameters: vec![value(200)],
                    operations: vec![boolean_constant(20, 9_000, true)],
                    terminator: conditional(2, 3, 3, 4),
                },
                block(
                    3,
                    vec![rank_constant(30, 300)],
                    Terminator::Conditional {
                        condition: id(9_000),
                        when_true: successor(4, 2, vec![id(300)]),
                        when_false: successor(5, 8, Vec::new()),
                    },
                ),
                block(4, Vec::new(), return_unit(7)),
                block(
                    8,
                    vec![rank_constant(80, 800)],
                    jump_with(6, 2, vec![id(800)]),
                ),
            ],
            Some(TerminalRankedScc::Natural(vec![TerminalNaturalCycle {
                rank_type,
                ranks: [2, 3, 8]
                    .into_iter()
                    .map(|block| TerminalBlockNaturalRank {
                        block: id(block),
                        value: id(200),
                    })
                    .collect(),
                edges: vec![
                    TerminalNaturalRankEdge {
                        edge: id(2),
                        source: id(2),
                        target: id(3),
                        successor_rank: id(200),
                        comparison: TerminalNaturalRankComparison::Preserving,
                    },
                    TerminalNaturalRankEdge {
                        edge: id(4),
                        source: id(3),
                        target: id(2),
                        successor_rank: id(300),
                        comparison: TerminalNaturalRankComparison::Strict,
                    },
                    TerminalNaturalRankEdge {
                        edge: id(5),
                        source: id(3),
                        target: id(8),
                        successor_rank: id(200),
                        comparison: TerminalNaturalRankComparison::Preserving,
                    },
                    TerminalNaturalRankEdge {
                        edge: id(6),
                        source: id(8),
                        target: id(2),
                        successor_rank: id(800),
                        comparison: TerminalNaturalRankComparison::Strict,
                    },
                ],
            }])),
        );
        semantic.parameters = vec![value(100)];
        semantic
    }

    /// When the endpoint exclusion leaves once-only members beside the
    /// surviving cycle, only the re-enterable members keep the rank
    /// multiplier: component {2,3,8} bounds a segment committing tail
    /// edge 6 by rank * (visit 2 + visit 3) + visit 8 rather than rank *
    /// all three member visits. Uses the internal surface because the
    /// verifier requires discharged rank obligations that a hand-built
    /// module cannot carry.
    #[test]
    fn natural_cycle_once_only_members_leave_the_rank_scale() {
        let module = module(1, vec![cyclic_tail_machine()]);
        let subject = PreparedFuelModule::new(&module);
        let prepared = PreparedSegments::new(&subject, id(1)).expect("machine prepares");
        assert_eq!(
            prepared
                .segment_certificate(id(2), id(6), &mut BTreeMap::new())
                .expect("mid-component segment derives")
                .ceiling_units,
            4 * 256 + 2,
            "the surviving 2 -> 3 -> 2 cycle at rank scale plus once-only member 8"
        );
        assert_eq!(
            prepared
                .segment_certificate(id(1), id(6), &mut BTreeMap::new())
                .expect("entry-to-commit segment derives")
                .ceiling_units,
            1 + 4 * 256 + 2,
            "entry edge plus the tightened interior"
        );
    }

    /// A codec-valid `Natural` component {2,3,8,9} with an upstream
    /// member the entry cannot reach once the endpoint edge is
    /// excluded: entry 1 passes machine parameter `initial` into
    /// header 2's rank parameter `rank`; 2 conditionally enters work 3
    /// (preserving) or exits to return block 4; 3 returns to 2 (strict)
    /// or forwards to 8 (preserving); 8 jumps into 9 (preserving); 9
    /// returns to 8 (strict) or back to 2 (strict). Removing endpoint
    /// edge 2 leaves the 8 <-> 9 inner cycle intact beside member 3,
    /// which an entry at 9 can no longer reach.
    fn upstream_member_cyclic_machine() -> TerminalMachine {
        let rank_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let scalar = ScalarType::Integer(rank_type);
        let value = |raw: u64| ValueDeclaration {
            qualifications: Default::default(),
            id: id(raw),
            scalar_type: scalar,
        };
        let rank_constant = |operation: u64, result: u64| Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: id(operation),
            result: OperationResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: id(result),
                scalar_type: scalar,
            }),
            kind: OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(0),
            },
        };
        let jump_with = |edge: u64, target: u64, arguments: Vec<ValueId>| Terminator::Jump {
            edge: id(edge),
            target: id(target),
            arguments,
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
            residual_affine_discards: Vec::new(),
        };
        let successor =
            |edge: u64, target: u64, arguments: Vec<ValueId>| terminal_psi::SuccessorEdge {
                edge: id::<EdgeId>(edge),
                target: id::<BlockId>(target),
                arguments,
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                structural_arguments: Vec::new(),
                trivial_affine_discards: Vec::new(),
            };
        let mut semantic = machine(
            1,
            1,
            vec![
                block(1, Vec::new(), jump_with(1, 2, vec![id(100)])),
                Block {
                    erased_scalar_formals: Vec::new(),
                    erased_proof_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: id(2),
                    parameters: vec![value(200)],
                    operations: vec![boolean_constant(20, 9_000, true)],
                    terminator: conditional(2, 3, 3, 4),
                },
                block(
                    3,
                    vec![rank_constant(30, 300)],
                    Terminator::Conditional {
                        condition: id(9_000),
                        when_true: successor(4, 2, vec![id(300)]),
                        when_false: successor(5, 8, Vec::new()),
                    },
                ),
                block(4, Vec::new(), return_unit(40)),
                block(8, vec![rank_constant(80, 800)], jump_with(6, 9, Vec::new())),
                block(
                    9,
                    vec![rank_constant(90, 900)],
                    Terminator::Conditional {
                        condition: id(9_000),
                        when_true: successor(7, 8, Vec::new()),
                        when_false: successor(8, 2, vec![id(900)]),
                    },
                ),
            ],
            Some(TerminalRankedScc::Natural(vec![TerminalNaturalCycle {
                rank_type,
                ranks: [2, 3, 8, 9]
                    .into_iter()
                    .map(|block| TerminalBlockNaturalRank {
                        block: id(block),
                        value: id(200),
                    })
                    .collect(),
                edges: vec![
                    TerminalNaturalRankEdge {
                        edge: id(2),
                        source: id(2),
                        target: id(3),
                        successor_rank: id(200),
                        comparison: TerminalNaturalRankComparison::Preserving,
                    },
                    TerminalNaturalRankEdge {
                        edge: id(4),
                        source: id(3),
                        target: id(2),
                        successor_rank: id(300),
                        comparison: TerminalNaturalRankComparison::Strict,
                    },
                    TerminalNaturalRankEdge {
                        edge: id(5),
                        source: id(3),
                        target: id(8),
                        successor_rank: id(200),
                        comparison: TerminalNaturalRankComparison::Preserving,
                    },
                    TerminalNaturalRankEdge {
                        edge: id(6),
                        source: id(8),
                        target: id(9),
                        successor_rank: id(200),
                        comparison: TerminalNaturalRankComparison::Preserving,
                    },
                    TerminalNaturalRankEdge {
                        edge: id(7),
                        source: id(9),
                        target: id(8),
                        successor_rank: id(200),
                        comparison: TerminalNaturalRankComparison::Strict,
                    },
                    TerminalNaturalRankEdge {
                        edge: id(8),
                        source: id(9),
                        target: id(2),
                        successor_rank: id(900),
                        comparison: TerminalNaturalRankComparison::Strict,
                    },
                ],
            }])),
        );
        semantic.parameters = vec![value(100)];
        semantic
    }

    /// The interior pass is charged over the members the entry block can
    /// actually traverse, not every member that can still reach the
    /// endpoint. In the `cyclic_tail_machine` component {2,3,8},
    /// removing endpoint edge 2 disconnects member 3 from an entry at
    /// 8, so segment (8, edge 2) bounds walks 8 -> 2 -> commit at
    /// visit(8) + visit(2); the same endpoint from member 3 — which can
    /// still reach tail 8 — keeps the longer 3 -> 8 -> 2 -> commit
    /// pass, and an entry edge arriving at member 2 bounds that
    /// member's traversal alone. Uses the internal surface because the
    /// verifier requires discharged rank obligations that a hand-built
    /// module cannot carry.
    #[test]
    fn mid_component_segment_bounds_only_members_reachable_from_entry() {
        let module = module(1, vec![cyclic_tail_machine()]);
        let subject = PreparedFuelModule::new(&module);
        let prepared = PreparedSegments::new(&subject, id(1)).expect("machine prepares");
        assert_eq!(
            prepared
                .segment_certificate(id(8), id(2), &mut BTreeMap::new())
                .expect("tail-to-header segment derives")
                .ceiling_units,
            4,
            "visit(8) + visit(2): member 3 cannot be reached from the \
             entry member once the endpoint edge is excluded"
        );
        assert_eq!(
            prepared
                .segment_certificate(id(3), id(2), &mut BTreeMap::new())
                .expect("work-to-header segment derives")
                .ceiling_units,
            6,
            "from member 3 the longer 3 -> 8 -> 2 -> commit pass still bounds"
        );
        assert_eq!(
            prepared
                .segment_certificate(id(1), id(2), &mut BTreeMap::new())
                .expect("entry-to-header segment derives")
                .ceiling_units,
            3,
            "an entry edge arriving at member 2 bounds that member alone"
        );
    }

    /// The same entry restriction applies when the surviving interior
    /// keeps a cycle: segment (9, edge 2) bounds the re-enterable
    /// members reachable from 9 — the surviving 8 <-> 9 inner cycle —
    /// at the rank ceiling plus once-only member 2, while upstream
    /// member 3 drops out of the bound entirely.
    #[test]
    fn mid_component_cyclic_interior_drops_members_behind_the_entry() {
        let module = module(1, vec![upstream_member_cyclic_machine()]);
        let subject = PreparedFuelModule::new(&module);
        let prepared = PreparedSegments::new(&subject, id(1)).expect("machine prepares");
        assert_eq!(
            prepared
                .segment_certificate(id(9), id(2), &mut BTreeMap::new())
                .expect("inner-cycle entry segment derives")
                .ceiling_units,
            4 * 256 + 2,
            "the surviving 8 <-> 9 cycle at rank scale plus once-only \
             member 2 — member 3 stays unreachable from the entry member"
        );
        assert_eq!(
            prepared
                .segment_certificate(id(3), id(2), &mut BTreeMap::new())
                .expect("work-to-header segment derives")
                .ceiling_units,
            4 * 256 + 4,
            "from member 3 every member stays reachable, so the interior \
             keeps member 3's own once-only charge"
        );
    }

    /// A `Natural` component {2,3,4,8,9,10} whose two cyclic branches meet
    /// only at merge member 10: entry 1 passes machine parameter `initial`
    /// into header 2's rank parameter `rank`; 2 conditionally enters work
    /// member 3 (preserving) or work member 8 (preserving); 3 enters 4
    /// (preserving) or exits to heavy tail 11; 4 returns to 3 (strict) or
    /// forwards to 10 (preserving); 8 enters 9 (preserving) or exits to
    /// light tail 12; 9 returns to 8 (strict) or forwards to 10
    /// (preserving); 10's crash-only call means no traversal of it
    /// completes, severing the branches — its strict edge back to 2 can
    /// never commit. The two tails join at return block 13, so a segment
    /// committing 13's return edge leaves through member 3 or member 8.
    fn severed_merge_exit_machine() -> TerminalMachine {
        let rank_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let scalar = ScalarType::Integer(rank_type);
        let value = |raw: u64| ValueDeclaration {
            qualifications: Default::default(),
            id: id(raw),
            scalar_type: scalar,
        };
        let rank_constant = |operation: u64, result: u64| Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: id(operation),
            result: OperationResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: id(result),
                scalar_type: scalar,
            }),
            kind: OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(0),
            },
        };
        let jump_with = |edge: u64, target: u64, arguments: Vec<ValueId>| Terminator::Jump {
            edge: id(edge),
            target: id(target),
            arguments,
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
            residual_affine_discards: Vec::new(),
        };
        let successor =
            |edge: u64, target: u64, arguments: Vec<ValueId>| terminal_psi::SuccessorEdge {
                edge: id::<EdgeId>(edge),
                target: id::<BlockId>(target),
                arguments,
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                structural_arguments: Vec::new(),
                trivial_affine_discards: Vec::new(),
            };
        let branch_conditional = |internal_edge: u64, internal: u64, exit_edge: u64, exit: u64| {
            Terminator::Conditional {
                condition: id(9_000),
                when_true: successor(internal_edge, internal, Vec::new()),
                when_false: successor(exit_edge, exit, Vec::new()),
            }
        };
        // Merge member 10's call republishes callee 5's Trap route
        // verbatim: a call's crash continuations must match the callee
        // contract the semantic identity replays, or the module is
        // malformed before fuel runs.
        let mut crash_call = call_unit(101, 5);
        let OperationKind::CallUnit {
            crash_continuations,
            ..
        } = &mut crash_call.kind
        else {
            unreachable!("call_unit builds a CallUnit operation")
        };
        *crash_continuations = vec![terminal_psi::CrashRouteBucket {
            cause: terminal_psi::CrashCause::Trap,
            alternatives: vec![terminal_psi::CrashRouteGuard::Truth],
        }];
        let mut tail_operations = Vec::new();
        for offset in 0..10_u64 {
            tail_operations.push(rank_constant(110 + offset, 1_100 + offset));
        }
        let mut semantic = machine(
            1,
            1,
            vec![
                block(1, Vec::new(), jump_with(1, 2, vec![id(100)])),
                Block {
                    erased_scalar_formals: Vec::new(),
                    erased_proof_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: id(2),
                    parameters: vec![value(200)],
                    operations: vec![boolean_constant(20, 9_000, true)],
                    terminator: branch_conditional(2, 3, 3, 8),
                },
                block(
                    3,
                    vec![rank_constant(30, 300)],
                    branch_conditional(4, 4, 5, 11),
                ),
                block(
                    4,
                    vec![rank_constant(40, 400)],
                    branch_conditional(6, 3, 7, 10),
                ),
                block(
                    8,
                    vec![rank_constant(80, 800)],
                    branch_conditional(8, 9, 9, 12),
                ),
                block(
                    9,
                    vec![rank_constant(90, 900)],
                    branch_conditional(10, 8, 11, 10),
                ),
                block(
                    10,
                    vec![rank_constant(100, 1_000), crash_call],
                    jump_with(12, 2, vec![id(1_000)]),
                ),
                block(11, tail_operations, jump_with(13, 13, Vec::new())),
                block(12, Vec::new(), jump_with(14, 13, Vec::new())),
                block(13, Vec::new(), return_unit(60)),
            ],
            Some(TerminalRankedScc::Natural(vec![TerminalNaturalCycle {
                rank_type,
                ranks: [2, 3, 4, 8, 9, 10]
                    .into_iter()
                    .map(|block| TerminalBlockNaturalRank {
                        block: id(block),
                        value: id(200),
                    })
                    .collect(),
                edges: vec![
                    TerminalNaturalRankEdge {
                        edge: id(2),
                        source: id(2),
                        target: id(3),
                        successor_rank: id(200),
                        comparison: TerminalNaturalRankComparison::Preserving,
                    },
                    TerminalNaturalRankEdge {
                        edge: id(3),
                        source: id(2),
                        target: id(8),
                        successor_rank: id(200),
                        comparison: TerminalNaturalRankComparison::Preserving,
                    },
                    TerminalNaturalRankEdge {
                        edge: id(4),
                        source: id(3),
                        target: id(4),
                        successor_rank: id(200),
                        comparison: TerminalNaturalRankComparison::Preserving,
                    },
                    TerminalNaturalRankEdge {
                        edge: id(6),
                        source: id(4),
                        target: id(3),
                        successor_rank: id(200),
                        comparison: TerminalNaturalRankComparison::Strict,
                    },
                    TerminalNaturalRankEdge {
                        edge: id(7),
                        source: id(4),
                        target: id(10),
                        successor_rank: id(200),
                        comparison: TerminalNaturalRankComparison::Preserving,
                    },
                    TerminalNaturalRankEdge {
                        edge: id(8),
                        source: id(8),
                        target: id(9),
                        successor_rank: id(200),
                        comparison: TerminalNaturalRankComparison::Preserving,
                    },
                    TerminalNaturalRankEdge {
                        edge: id(10),
                        source: id(9),
                        target: id(8),
                        successor_rank: id(200),
                        comparison: TerminalNaturalRankComparison::Strict,
                    },
                    TerminalNaturalRankEdge {
                        edge: id(11),
                        source: id(9),
                        target: id(10),
                        successor_rank: id(200),
                        comparison: TerminalNaturalRankComparison::Preserving,
                    },
                    TerminalNaturalRankEdge {
                        edge: id(12),
                        source: id(10),
                        target: id(2),
                        successor_rank: id(1_000),
                        comparison: TerminalNaturalRankComparison::Strict,
                    },
                ],
            }])),
        );
        semantic.parameters = vec![value(100)];
        // The caller republishes the Trap route the call propagates:
        // uncovered continuations are malformed before fuel accounting.
        semantic.contract.crash_routes = vec![terminal_psi::CrashRouteBucket {
            cause: terminal_psi::CrashCause::Trap,
            alternatives: vec![terminal_psi::CrashRouteGuard::Truth],
        }];
        semantic
    }

    /// A mid-component exit segment pairs each exit member with the
    /// interior that can still reach it alone, not the union of every
    /// exit-reaching member against the worst tail. In the
    /// `severed_merge_exit_machine` component, merge member 10's dead
    /// traversal severs the branches, so a walk exiting through member 3
    /// traverses only {2,3,4} — its surviving 3 <-> 4 cycle at the rank
    /// ceiling plus once-only header 2 — before the heavy tail, while a
    /// walk exiting through member 8 crosses {2,8,9} before the light
    /// tail: the bound is the worse of the two pairings rather than the
    /// five-member union against the worst tail. Uses the internal
    /// surface because the verifier requires discharged rank obligations
    /// that a hand-built module cannot carry.
    #[test]
    fn natural_cycle_exit_segment_pairs_each_exit_with_its_own_interior() {
        // callee 5 always crashes (returned: None, crashed: Some(1)); the
        // contract publishes the Trap route coverage semantic identity
        // validation requires.
        let mut callee = machine(5, 5, vec![], None);
        callee.blocks = vec![block(
            5,
            Vec::new(),
            Terminator::Crash {
                edge: id(50),
                cause: terminal_psi::CrashCause::Trap,
                site_guard: Vec::new(),
                frontier_lower_bound: Vec::new(),
            },
        )];
        callee.contract.crash_routes = vec![terminal_psi::CrashRouteBucket {
            cause: terminal_psi::CrashCause::Trap,
            alternatives: vec![terminal_psi::CrashRouteGuard::Truth],
        }];
        let module = module(1, vec![severed_merge_exit_machine(), callee]);
        let subject = PreparedFuelModule::new(&module);
        let prepared = PreparedSegments::new(&subject, id(1)).expect("machine prepares");
        // Branch interiors: visit(2) once + 256 * (visit(3) + visit(4))
        // or visit(2) + 256 * (visit(8) + visit(9)) — the severed merge
        // keeps either branch's members off the other's walk. Tails:
        // tail 11 charges ten operations plus its jump and the return
        // edge (12), tail 12 charges the jump and the return (2).
        assert_eq!(
            prepared
                .segment_certificate(id(2), id(60), &mut BTreeMap::new())
                .expect("fork-entry exit segment derives")
                .ceiling_units,
            2 + 4 * 256 + 12,
            "the worse pairing — branch 3 <-> 4 at rank scale plus tail \
             11 — not the five-member union against the worst tail"
        );
        assert_eq!(
            prepared
                .segment_certificate(id(1), id(60), &mut BTreeMap::new())
                .expect("entry-to-exit segment derives")
                .ceiling_units,
            1 + 2 + 4 * 256 + 12,
            "entry edge plus the worst exit pairing"
        );
        assert_eq!(
            prepared
                .segment_certificate(id(3), id(60), &mut BTreeMap::new())
                .expect("branch-entry exit segment derives")
                .ceiling_units,
            4 * 256 + 12,
            "entering inside branch 3 <-> 4 bills that branch alone: \
             members 8, 9, and header 2 stay unreachable"
        );
        assert_eq!(
            prepared
                .segment_certificate(id(8), id(60), &mut BTreeMap::new())
                .expect("other-branch entry segment derives")
                .ceiling_units,
            4 * 256 + 2,
            "entering inside branch 8 <-> 9 bills its own interior plus \
             its own light tail"
        );
    }

    /// A `Natural` countdown whose work member calls a callee that can only
    /// crash: entry 1 passes machine parameter `initial` into header 2's
    /// rank parameter `rank`; 2 conditionally enters work 3 (preserving) or
    /// exits to return block 4; 3 computes `next`, calls crash-only machine
    /// 5, and would pass `next` back to 2 (strict). Every traversal of 3
    /// invokes the call, so a walk that reaches 3 crashes before it can
    /// commit an exit — member 3 cannot participate in an exit-committing
    /// segment at all.
    fn cyclic_crash_member_machine() -> TerminalMachine {
        let rank_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let scalar = ScalarType::Integer(rank_type);
        let value = |raw: u64| ValueDeclaration {
            qualifications: Default::default(),
            id: id(raw),
            scalar_type: scalar,
        };
        let rank_constant = |operation: u64, result: u64| Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: id(operation),
            result: OperationResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: id(result),
                scalar_type: scalar,
            }),
            kind: OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(0),
            },
        };
        let jump_with = |edge: u64, target: u64, arguments: Vec<ValueId>| Terminator::Jump {
            edge: id(edge),
            target: id(target),
            arguments,
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
            residual_affine_discards: Vec::new(),
        };
        // The call republishes callee 5's Trap route verbatim: a call's
        // crash continuations must match the callee contract the semantic
        // identity replays, or the module is malformed before fuel runs.
        let mut crash_call = call_unit(31, 5);
        let OperationKind::CallUnit {
            crash_continuations,
            ..
        } = &mut crash_call.kind
        else {
            unreachable!("call_unit builds a CallUnit operation")
        };
        *crash_continuations = vec![terminal_psi::CrashRouteBucket {
            cause: terminal_psi::CrashCause::Trap,
            alternatives: vec![terminal_psi::CrashRouteGuard::Truth],
        }];
        let mut semantic = machine(
            1,
            1,
            vec![
                block(1, Vec::new(), jump_with(1, 2, vec![id(100)])),
                Block {
                    erased_scalar_formals: Vec::new(),
                    erased_proof_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: id(2),
                    parameters: vec![value(200)],
                    operations: vec![boolean_constant(20, 9_000, true)],
                    terminator: conditional(2, 3, 3, 4),
                },
                block(
                    3,
                    vec![rank_constant(30, 300), crash_call],
                    jump_with(4, 2, vec![id(300)]),
                ),
                block(4, Vec::new(), return_unit(5)),
            ],
            Some(TerminalRankedScc::Natural(vec![TerminalNaturalCycle {
                rank_type,
                ranks: [2, 3]
                    .into_iter()
                    .map(|block| TerminalBlockNaturalRank {
                        block: id(block),
                        value: id(200),
                    })
                    .collect(),
                edges: vec![
                    TerminalNaturalRankEdge {
                        edge: id(2),
                        source: id(2),
                        target: id(3),
                        successor_rank: id(200),
                        comparison: TerminalNaturalRankComparison::Preserving,
                    },
                    TerminalNaturalRankEdge {
                        edge: id(4),
                        source: id(3),
                        target: id(2),
                        successor_rank: id(300),
                        comparison: TerminalNaturalRankComparison::Strict,
                    },
                ],
            }])),
        );
        semantic.parameters = vec![value(100)];
        // The caller republishes the Trap route the call propagates:
        // uncovered continuations are malformed before fuel accounting.
        semantic.contract.crash_routes = vec![terminal_psi::CrashRouteBucket {
            cause: terminal_psi::CrashCause::Trap,
            alternatives: vec![terminal_psi::CrashRouteGuard::Truth],
        }];
        semantic
    }

    /// A `Natural` component whose only exit rides a member no traversal can
    /// complete: header 2 always re-enters work member 3, and 3's call to
    /// crash-only machine 5 ends every walk before the exit edge to return
    /// block 4 can commit. Every admitted walk crashes, so the machine has
    /// no commit-reachable return even though the crash-inclusive ceiling
    /// still certifies the whole entry.
    fn never_returning_cyclic_machine() -> TerminalMachine {
        let rank_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let scalar = ScalarType::Integer(rank_type);
        let value = |raw: u64| ValueDeclaration {
            qualifications: Default::default(),
            id: id(raw),
            scalar_type: scalar,
        };
        let rank_constant = |operation: u64, result: u64| Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: id(operation),
            result: OperationResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: id(result),
                scalar_type: scalar,
            }),
            kind: OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(0),
            },
        };
        let jump_with = |edge: u64, target: u64, arguments: Vec<ValueId>| Terminator::Jump {
            edge: id(edge),
            target: id(target),
            arguments,
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
            residual_affine_discards: Vec::new(),
        };
        let mut crash_call = call_unit(31, 5);
        let OperationKind::CallUnit {
            crash_continuations,
            ..
        } = &mut crash_call.kind
        else {
            unreachable!("call_unit builds a CallUnit operation")
        };
        *crash_continuations = vec![terminal_psi::CrashRouteBucket {
            cause: terminal_psi::CrashCause::Trap,
            alternatives: vec![terminal_psi::CrashRouteGuard::Truth],
        }];
        // Work member 3 carries both the strict backedge and the exit on
        // its own conditional — its crash-only call means no traversal
        // completes it, so neither edge can ever commit. The backedge
        // passes `next` to header 2's rank parameter exactly like the
        // returning cycle's does.
        let exit_conditional = {
            let successor =
                |edge: u64, target: u64, arguments: Vec<ValueId>| terminal_psi::SuccessorEdge {
                    edge: id::<EdgeId>(edge),
                    target: id::<BlockId>(target),
                    arguments,
                    erased_arguments: Vec::new(),
                    erased_proof_arguments: Vec::new(),
                    structural_arguments: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                };
            Terminator::Conditional {
                condition: id(9_001),
                when_true: successor(4, 2, vec![id(300)]),
                when_false: successor(7, 4, vec![]),
            }
        };
        let mut semantic = machine(
            1,
            1,
            vec![
                block(1, Vec::new(), jump_with(1, 2, vec![id(100)])),
                Block {
                    erased_scalar_formals: Vec::new(),
                    erased_proof_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: id(2),
                    parameters: vec![value(200)],
                    operations: vec![boolean_constant(20, 9_000, true)],
                    terminator: conditional(2, 3, 6, 3),
                },
                block(
                    3,
                    vec![
                        rank_constant(30, 300),
                        boolean_constant(32, 9_001, true),
                        crash_call,
                    ],
                    exit_conditional,
                ),
                block(4, Vec::new(), return_unit(5)),
            ],
            Some(TerminalRankedScc::Natural(vec![TerminalNaturalCycle {
                rank_type,
                ranks: [2, 3]
                    .into_iter()
                    .map(|block| TerminalBlockNaturalRank {
                        block: id(block),
                        value: id(200),
                    })
                    .collect(),
                edges: vec![
                    TerminalNaturalRankEdge {
                        edge: id(2),
                        source: id(2),
                        target: id(3),
                        successor_rank: id(200),
                        comparison: TerminalNaturalRankComparison::Preserving,
                    },
                    TerminalNaturalRankEdge {
                        edge: id(4),
                        source: id(3),
                        target: id(2),
                        successor_rank: id(300),
                        comparison: TerminalNaturalRankComparison::Strict,
                    },
                    TerminalNaturalRankEdge {
                        edge: id(6),
                        source: id(2),
                        target: id(3),
                        successor_rank: id(200),
                        comparison: TerminalNaturalRankComparison::Preserving,
                    },
                ],
            }])),
        );
        semantic.parameters = vec![value(100)];
        semantic.contract.crash_routes = vec![terminal_psi::CrashRouteBucket {
            cause: terminal_psi::CrashCause::Trap,
            alternatives: vec![terminal_psi::CrashRouteGuard::Truth],
        }];
        semantic
    }

    /// Machine-level outcome bounds through the same module preparation the
    /// certificate derivations use, so tests can pin `returned` and
    /// `crashed` separately rather than only the merged entry ceiling.
    fn machine_outcomes(
        module: &TerminalModule,
        machine: u64,
    ) -> super::super::outcome_bounds::OutcomeBounds {
        let machines = module
            .machines
            .iter()
            .map(|machine| (machine.id, machine))
            .collect::<BTreeMap<_, _>>();
        maximum_machine_outcomes(
            id(machine),
            &machines,
            &dynamic_call_targets(module),
            &boundary_call_candidates(module),
            TerminalFuelSchedule::CURRENT,
            &mut BTreeMap::new(),
            &mut BTreeSet::new(),
        )
        .expect("machine outcomes derive")
    }

    /// When no member terminator carries the endpoint, a committing walk
    /// leaves the component through an exit edge — so the interior bound
    /// covers only the members a committing walk can still traverse, at the
    /// segment's normal-return accounting. Work member 3 invokes a callee
    /// that can only crash, so it contributes nothing — and its strict
    /// back-edge can never be taken either, since no traversal of 3
    /// completes: header 2 is crossed at most once before the walk exits
    /// or dies inside 3's call, so segment (2, edge 5) is visit(2) + the
    /// exit block, not rank * (visit 2 + visit 3) the way the old
    /// whole-component charge billed it. Uses the internal surface
    /// because the verifier requires discharged rank obligations that a
    /// hand-built module cannot carry.
    #[test]
    fn natural_cycle_exit_segment_drops_members_that_cannot_return() {
        // callee 5 always crashes (returned: None, crashed: Some(1)); the
        // contract publishes the Trap route coverage semantic identity
        // validation requires.
        let mut callee = machine(5, 5, vec![], None);
        callee.blocks = vec![block(
            5,
            Vec::new(),
            Terminator::Crash {
                edge: id(50),
                cause: terminal_psi::CrashCause::Trap,
                site_guard: Vec::new(),
                frontier_lower_bound: Vec::new(),
            },
        )];
        callee.contract.crash_routes = vec![terminal_psi::CrashRouteBucket {
            cause: terminal_psi::CrashCause::Trap,
            alternatives: vec![terminal_psi::CrashRouteGuard::Truth],
        }];
        let module = module(1, vec![cyclic_crash_member_machine(), callee]);
        let subject = PreparedFuelModule::new(&module);
        let prepared = PreparedSegments::new(&subject, id(1)).expect("machine prepares");
        assert_eq!(
            prepared
                .segment_certificate(id(2), id(5), &mut BTreeMap::new())
                .expect("mid-component exit segment derives")
                .ceiling_units,
            3,
            "once-only header 2 plus the exit block; member 3's dead \
             traversal removes its back-edge along with its visit units"
        );
        assert_eq!(
            prepared
                .segment_certificate(id(1), id(5), &mut BTreeMap::new())
                .expect("entry-to-exit segment derives")
                .ceiling_units,
            4,
            "entry edge plus the tightened interior"
        );
    }

    /// A member whose traversal cannot complete commits none of its
    /// terminator edges: member 3's call can only crash, so its strict
    /// backedge 4 is unreachable as an endpoint and a segment naming it
    /// reports the call that ends every traversal — the same dead end the
    /// acyclic walk reports — rather than a fabricated interior bound.
    /// Uses the internal surface because the verifier requires discharged
    /// rank obligations that a hand-built module cannot carry.
    #[test]
    fn natural_cycle_uncompletable_member_cannot_commit_its_edge() {
        let mut callee = machine(5, 5, vec![], None);
        callee.blocks = vec![block(
            5,
            Vec::new(),
            Terminator::Crash {
                edge: id(50),
                cause: terminal_psi::CrashCause::Trap,
                site_guard: Vec::new(),
                frontier_lower_bound: Vec::new(),
            },
        )];
        callee.contract.crash_routes = vec![terminal_psi::CrashRouteBucket {
            cause: terminal_psi::CrashCause::Trap,
            alternatives: vec![terminal_psi::CrashRouteGuard::Truth],
        }];
        let module = module(1, vec![cyclic_crash_member_machine(), callee]);
        let subject = PreparedFuelModule::new(&module);
        let prepared = PreparedSegments::new(&subject, id(1)).expect("machine prepares");
        assert_eq!(
            prepared.segment_certificate(id(2), id(4), &mut BTreeMap::new()),
            Err(FixedFuelError::SegmentEndUnreachableAfterCall {
                block: id(3),
                callee: id(5),
            }),
            "member 3's backedge cannot commit through a crash-only call"
        );
        assert_eq!(
            prepared.segment_certificate(id(1), id(4), &mut BTreeMap::new()),
            Err(FixedFuelError::SegmentEndUnreachableAfterCall {
                block: id(3),
                callee: id(5),
            }),
            "the same rejection holds when the walk enters through the entry"
        );
    }

    /// The whole-entry ceiling keeps every admitted walk — crash-terminal
    /// ones included — while the machine's normal-return outcome charges
    /// only walks that can still commit: member 3's crash-only call drops
    /// out of `returned` exactly the way it drops out of a committing
    /// segment, and a caller composing `.returned` inherits the tighter
    /// figure rather than the crash-inclusive maximum. Uses the internal
    /// surface because the verifier requires discharged rank obligations
    /// that a hand-built module cannot carry.
    #[test]
    fn natural_cycle_returned_bound_drops_members_that_can_only_crash() {
        // callee 5 always crashes (returned: None, crashed: Some(1)); the
        // contract publishes the Trap route coverage semantic identity
        // validation requires.
        let mut callee = machine(5, 5, vec![], None);
        callee.blocks = vec![block(
            5,
            Vec::new(),
            Terminator::Crash {
                edge: id(50),
                cause: terminal_psi::CrashCause::Trap,
                site_guard: Vec::new(),
                frontier_lower_bound: Vec::new(),
            },
        )];
        callee.contract.crash_routes = vec![terminal_psi::CrashRouteBucket {
            cause: terminal_psi::CrashCause::Trap,
            alternatives: vec![terminal_psi::CrashRouteGuard::Truth],
        }];
        // caller 7 is a single call to machine 1 followed by a return edge.
        let caller = machine(
            7,
            7,
            vec![block(7, vec![call_unit(70, 1)], return_unit(7))],
            None,
        );
        let module = module(1, vec![cyclic_crash_member_machine(), callee, caller]);

        let bounds = machine_outcomes(&module, 1);
        assert_eq!(
            bounds.returned,
            Some(1 + 2 + 1),
            "entry edge, member 2 crossed once — member 3's dead traversal \
             removes the only cycle that could re-enter it — exit block"
        );
        assert_eq!(
            bounds.crashed,
            Some(1 + 2 + 3),
            "the crash read keeps the once-only completing interior — \
             member 3's back-edge never commits — then ends inside the \
             component on member 3's one crashing visit: its two \
             operations plus callee 5's crash edge, never the exit block \
             the walk cannot reach"
        );
        assert_eq!(
            derive_maximum_entry_bound(&module, id(1)),
            Ok(1 + 2 + 3),
            "the certificate bound drops to the worse of the two honest \
             outcome lanes"
        );

        let caller_bounds = machine_outcomes(&module, 7);
        assert_eq!(
            caller_bounds.returned,
            Some(1 + (1 + 2 + 1) + 1),
            "a returning caller composes only the callee's returned bound"
        );
        assert_eq!(
            caller_bounds.crashed,
            Some(1 + (1 + 2 + 3)),
            "the crash walk commits the call then the callee's crash"
        );
    }

    /// A member that can complete but cannot reach an exit or return
    /// through other completing members is never on a commit-reachable
    /// walk: chain members 3 and 5 feed only member 6's always-crashing
    /// call, so the returned interior covers header 2 alone while the
    /// crash lane still bills the chain once each before member 6's
    /// crashing visit ends the walk. Uses the internal surface because the
    /// verifier requires discharged rank obligations that a hand-built
    /// module cannot carry.
    #[test]
    fn natural_cycle_members_unreachable_to_the_return_frontier_stay_on_the_crash_lane() {
        // callee 9 always crashes (returned: None, crashed: Some(1)); the
        // contract publishes the Trap route coverage semantic identity
        // validation requires.
        let mut callee = machine(9, 9, vec![], None);
        callee.blocks = vec![block(
            9,
            Vec::new(),
            Terminator::Crash {
                edge: id(50),
                cause: terminal_psi::CrashCause::Trap,
                site_guard: Vec::new(),
                frontier_lower_bound: Vec::new(),
            },
        )];
        callee.contract.crash_routes = vec![terminal_psi::CrashRouteBucket {
            cause: terminal_psi::CrashCause::Trap,
            alternatives: vec![terminal_psi::CrashRouteGuard::Truth],
        }];
        let mut crash_call = call_unit(31, 9);
        let OperationKind::CallUnit {
            crash_continuations,
            ..
        } = &mut crash_call.kind
        else {
            unreachable!("call_unit builds a CallUnit operation")
        };
        *crash_continuations = vec![terminal_psi::CrashRouteBucket {
            cause: terminal_psi::CrashCause::Trap,
            alternatives: vec![terminal_psi::CrashRouteGuard::Truth],
        }];
        // {2,3,5,6}: 2 -> 3 -> 5 -> 6 -> 2 is the cycle's shape, with 2
        // also exiting to return block 4. Member 6's call always crashes,
        // so its strict edge back to 2 can never commit: a completing walk
        // enters the chain only to die inside member 6, and can never
        // reach 2's exit through it — while a crash-terminal walk still
        // crosses the chain once before member 6's visit ends it.
        let mut semantic = machine(
            1,
            1,
            vec![
                block(1, Vec::new(), jump(1, 2)),
                block(
                    2,
                    vec![boolean_constant(20, 9_000, true)],
                    conditional(2, 3, 3, 4),
                ),
                block(3, Vec::new(), jump(5, 5)),
                block(5, Vec::new(), jump(6, 6)),
                block(
                    6,
                    vec![integer_constant(30, 300, 0), crash_call],
                    jump(7, 2),
                ),
                block(4, Vec::new(), return_unit(8)),
            ],
            Some(TerminalRankedScc::Natural(vec![TerminalNaturalCycle {
                rank_type: IntegerType::new(IntegerSign::Unsigned, 8).expect("u8"),
                ranks: [2, 3, 5, 6]
                    .into_iter()
                    .map(|block| TerminalBlockNaturalRank {
                        block: id(block),
                        value: id::<ValueId>(7_000),
                    })
                    .collect(),
                edges: vec![
                    rank_edge(2, 2, 3, TerminalNaturalRankComparison::Preserving),
                    rank_edge(5, 3, 5, TerminalNaturalRankComparison::Preserving),
                    rank_edge(6, 5, 6, TerminalNaturalRankComparison::Preserving),
                    rank_edge(7, 6, 2, TerminalNaturalRankComparison::Strict),
                ],
            }])),
        );
        semantic.contract.crash_routes = vec![terminal_psi::CrashRouteBucket {
            cause: terminal_psi::CrashCause::Trap,
            alternatives: vec![terminal_psi::CrashRouteGuard::Truth],
        }];
        let module = module(1, vec![semantic, callee]);

        let bounds = machine_outcomes(&module, 1);
        assert_eq!(
            bounds.returned,
            Some(1 + 2 + 1),
            "entry edge plus member 2 crossed once — chain members 3 and \
             5 can only reach member 6's dead traversal, never the exit — \
             plus the exit block"
        );
        assert_eq!(
            bounds.crashed,
            Some(1 + (2 + 1 + 1) + 3),
            "the crash lane still crosses chain members 2, 3, and 5 once \
             each before member 6's crashing visit — two operations plus \
             callee 9's crash edge — ends the walk"
        );
        assert_eq!(
            derive_maximum_entry_bound(&module, id(1)),
            Ok(1 + (2 + 1 + 1) + 3)
        );
    }

    /// When a component's only exit rides a member whose visit can never
    /// complete, no admitted walk returns: the machine's `returned` outcome
    /// is `None`, the crash-inclusive ceiling still certifies every
    /// (crash-terminal) execution, and a caller sees the callee honestly —
    /// its own terminator is unreachable, so it derives no safe-point row
    /// and reports no returned walk of its own.
    #[test]
    fn natural_cycle_without_returning_exit_reports_no_normal_walk() {
        let mut callee = machine(5, 5, vec![], None);
        callee.blocks = vec![block(
            5,
            Vec::new(),
            Terminator::Crash {
                edge: id(50),
                cause: terminal_psi::CrashCause::Trap,
                site_guard: Vec::new(),
                frontier_lower_bound: Vec::new(),
            },
        )];
        callee.contract.crash_routes = vec![terminal_psi::CrashRouteBucket {
            cause: terminal_psi::CrashCause::Trap,
            alternatives: vec![terminal_psi::CrashRouteGuard::Truth],
        }];
        // caller 6 calls the never-returning machine then returns; the call
        // passes the callee's rank parameter, republishes the Trap route it
        // propagates, and the caller contract covers it — the same coverage
        // rule the cyclic machine's call obeys.
        let mut caller_call = call_unit(60, 1);
        let OperationKind::CallUnit {
            crash_continuations,
            arguments,
            ..
        } = &mut caller_call.kind
        else {
            unreachable!("call_unit builds a CallUnit operation")
        };
        *arguments = vec![id(9_100)];
        *crash_continuations = vec![terminal_psi::CrashRouteBucket {
            cause: terminal_psi::CrashCause::Trap,
            alternatives: vec![terminal_psi::CrashRouteGuard::Truth],
        }];
        let mut caller = machine(
            6,
            6,
            vec![block(
                6,
                vec![integer_constant(61, 9_100, 1), caller_call],
                return_unit(8),
            )],
            None,
        );
        caller.contract.crash_routes = vec![terminal_psi::CrashRouteBucket {
            cause: terminal_psi::CrashCause::Trap,
            alternatives: vec![terminal_psi::CrashRouteGuard::Truth],
        }];
        let module = module(1, vec![never_returning_cyclic_machine(), callee, caller]);

        let bounds = machine_outcomes(&module, 1);
        assert_eq!(
            bounds.returned, None,
            "every traversal reaches member 3's crash-only call"
        );
        assert_eq!(
            bounds.crashed,
            Some(1 + 2 + 4),
            "the crash read still certifies every walk: header 2's one \
             completing visit — member 3's dead traversal removes every \
             cycle that could re-enter it — plus member 3's one crashing \
             visit: three operations and callee 5's crash edge"
        );
        assert_eq!(
            derive_maximum_entry_bound(&module, id(1)),
            Ok(1 + 2 + 4),
            "the entry certificate follows the crash lane when no walk \
             returns"
        );

        let caller_bounds = machine_outcomes(&module, 6);
        assert_eq!(
            caller_bounds.returned, None,
            "the caller inherits the callee's honest no-return verdict"
        );
        assert_eq!(caller_bounds.crashed, Some(2 + (1 + 2 + 4)));

        // The unreachable return edge names the call that ends every walk
        // rather than fabricating an interior bound through member 3's exit.
        let subject = PreparedFuelModule::new(&module);
        let prepared = PreparedSegments::new(&subject, id(1)).expect("machine prepares");
        assert_eq!(
            prepared.segment_certificate(id(1), id(5), &mut BTreeMap::new()),
            Err(FixedFuelError::SegmentEndUnreachableAfterCall {
                block: id(3),
                callee: id(5),
            }),
            "the exit edge rides a member no traversal can complete"
        );

        let caller_prepared =
            PreparedSegments::new(&subject, id(6)).expect("caller machine prepares");
        assert!(
            caller_prepared
                .derive_catalog()
                .expect("catalog derives")
                .is_empty(),
            "a block whose call can never return has no reachable terminator \
             and publishes no safe-point segment"
        );
    }

    /// When no block, call, or cleanup can crash there is no crash-terminal
    /// walk to bound: the `crashed` lane is `None`, not a phantom
    /// whole-graph ceiling, so a caller composing it inherits nothing the
    /// callee cannot commit — while the returned lane keeps the
    /// rank-amplified bound unchanged.
    #[test]
    fn natural_cycle_without_crash_path_reports_no_crash_outcome() {
        let callee = machine(5, 5, vec![block(5, Vec::new(), return_unit(6))], None);
        let module = module(1, vec![cyclic_machine(8, vec![call_unit(10, 5)]), callee]);

        let bounds = machine_outcomes(&module, 1);
        assert_eq!(
            bounds.crashed, None,
            "every member completes and every edge commits, so no walk crashes"
        );
        // member_units = visit2(1) + visit3(1 call + callee 1 + 1 jump) = 4;
        // component = 4*256 = 1024; bound = 1 + 1024 + 1 = 1026.
        assert_eq!(bounds.returned, Some(1 + 4 * 256 + 1));
        assert_eq!(
            derive_maximum_entry_bound(&module, id(1)),
            Ok(1 + 4 * 256 + 1)
        );
    }

    /// A crash past the component boundary rides the same exit discipline
    /// the returned lane uses: the walk completes the interior at the rank
    /// ceiling, leaves through a member whose visit can complete, and
    /// crashes inside the successor — here the exit block's own `Crash`
    /// terminator — so no walk returns at all.
    #[test]
    fn natural_cycle_exit_into_crashing_block_composes_the_tail() {
        let mut walk = cyclic_machine(8, vec![integer_constant(10, 11, 0)]);
        walk.blocks[3].terminator = Terminator::Crash {
            edge: id(50),
            cause: terminal_psi::CrashCause::Trap,
            site_guard: Vec::new(),
            frontier_lower_bound: Vec::new(),
        };
        walk.contract.crash_routes = vec![terminal_psi::CrashRouteBucket {
            cause: terminal_psi::CrashCause::Trap,
            alternatives: vec![terminal_psi::CrashRouteGuard::Truth],
        }];
        let module = module(1, vec![walk]);

        let bounds = machine_outcomes(&module, 1);
        assert_eq!(
            bounds.returned, None,
            "the only exit ends on a Crash terminator, so no walk returns"
        );
        // interior = (visit2 1 + visit3 2) * 256 = 768; exit 3 -> 4 leaves
        // through completing member 2, then block 4's crash edge charges 1;
        // bound = 1 entry + 768 + 1.
        assert_eq!(bounds.crashed, Some(1 + 3 * 256 + 1));
        assert_eq!(
            derive_maximum_entry_bound(&module, id(1)),
            Ok(1 + 3 * 256 + 1)
        );
    }
}
