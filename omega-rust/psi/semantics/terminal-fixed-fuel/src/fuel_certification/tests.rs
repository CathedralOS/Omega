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
    use super::super::{
        derive_fixed_segment_fuel, derive_maximum_entry_bound,
        derive_validated_fixed_safe_point_segments, retain_validated_fixed_safe_point_segments,
        validate_retained_fixed_safe_point_segments,
    };
    use super::{
        BlockId, EdgeId, FixedFuelError, FuelScheduleIdentity, OperationKind, Proposition,
        TerminalMachine, TerminalModule, TerminalNaturalCycle, TerminalRankedScc, Terminator,
        identity,
    };
    use semantic_vocabulary::{
        ContractId, IntegerSign, IntegerType, IntegerValue, ScalarType, ValueId,
    };
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
            id: id(operation_id),
            result: OperationResult::Unit,
            kind: OperationKind::CallUnit {
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

    #[test]
    fn unranked_cycle_still_reports_control_cycle() {
        let mut walk = cyclic_machine(32, Vec::new());
        walk.ranked_scc = None;
        let module = module(1, vec![walk]);
        assert_eq!(
            derive_maximum_entry_bound(&module, id(1)),
            Err(FixedFuelError::ControlCycle(id(2)))
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
    /// cover it and derivation rejects rather than under-approximates.
    #[test]
    fn dynamic_parameter_call_rejects_as_invocation_bound() {
        let walk = cyclic_machine(32, vec![parameter_unit_call(10)]);
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
}
