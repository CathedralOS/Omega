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
        boundary_call_candidates, dynamic_call_targets, maximum_machine_outcomes, used_premises,
    };
    use super::super::segment_partition::{PreparedFuelModule, PreparedSegments};
    use super::super::{
        derive_fixed_entry_fuel, derive_fixed_segment_fuel, derive_maximum_entry_bound,
        validate_fixed_entry_fuel, validate_fixed_segment_fuel,
    };
    use super::{
        BlockId, EdgeId, FixedFuelError, OperationKind, Proposition, TerminalFuelSchedule,
        TerminalMachine, TerminalModule, TerminalNaturalCycle, TerminalRankedScc, Terminator,
    };
    use semantic_vocabulary::{
        ContractId, IntegerMathTerm, IntegerSign, IntegerType, IntegerValue, ObligationId,
        ScalarTerm, ScalarType, ValueId,
    };
    use std::collections::{BTreeMap, BTreeSet};
    use terminal_psi::{
        Block, MachineContract, Operation, OperationResult, ProviderCandidateConformance,
        ProviderRefinement, ProviderSignature, ScalarBlockInvariant, ScalarBlockInvariantArrival,
        TerminalBlockNaturalRank, TerminalIndirectDynamicDispatch, TerminalMachineResult,
        TerminalNaturalRankComparison, TerminalNaturalRankEdge, TerminalStoredDynamicDispatch,
        ValueDeclaration,
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
        conditional_arguments(
            9_000,
            edge_true,
            target_true,
            Vec::new(),
            edge_false,
            target_false,
            Vec::new(),
        )
    }

    /// A conditional whose successors carry explicit scalar arguments — the
    /// shape a path-fact test needs to bind a component member's rank
    /// parameter through a selected edge.
    #[allow(clippy::too_many_arguments)]
    fn conditional_arguments(
        condition: u64,
        edge_true: u64,
        target_true: u64,
        arguments_true: Vec<ValueId>,
        edge_false: u64,
        target_false: u64,
        arguments_false: Vec<ValueId>,
    ) -> Terminator {
        let successor = |edge, target, arguments| terminal_psi::SuccessorEdge {
            edge: id::<EdgeId>(edge),
            target: id::<BlockId>(target),
            arguments,
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
        };
        Terminator::Conditional {
            condition: id(condition),
            when_true: successor(edge_true, target_true, arguments_true),
            when_false: successor(edge_false, target_false, arguments_false),
        }
    }

    fn integer_less_than(operation_id: u64, result: u64, left: u64, right: u64) -> Operation {
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: id(operation_id),
            result: OperationResult::Scalar(ValueDeclaration {
                id: id(result),
                scalar_type: ScalarType::Boolean,
                qualifications: Default::default(),
            }),
            kind: OperationKind::IntegerLessThan {
                left: id(left),
                right: id(right),
            },
        }
    }

    /// One `scalar_block_invariants` roster row: `predicate` holds on every
    /// arrival to `(machine, header)` — including the entries a cyclic
    /// component's rank bound must cover.
    fn scalar_invariant(
        machine: u64,
        header: u64,
        predicate: Proposition,
        arrival_edges: &[u64],
    ) -> ScalarBlockInvariant {
        ScalarBlockInvariant {
            machine: id(machine),
            header: id(header),
            predicate,
            arrivals: arrival_edges
                .iter()
                .map(|edge| ScalarBlockInvariantArrival {
                    edge: id(*edge),
                    obligation: id::<ObligationId>(9_000 + *edge),
                })
                .collect(),
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
                requirement_obligations: Vec::new(),
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

    /// A machine that retains a `Natural` ranking but leaves a cyclic
    /// component uncovered reports the verifier-derived component identity
    /// with the `Unranked` cause when the condensed walk revisits it — the
    /// same report an entirely unranked machine produces, not the block a
    /// traversal happened to revisit.
    #[test]
    fn partially_ranked_cycle_reports_unranked_component() {
        let mut walk = cyclic_machine(32, Vec::new());
        walk.ranked_scc = Some(TerminalRankedScc::Natural(Vec::new()));
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

    /// A u64 rank bound cannot publish a whole-entry certificate — the
    /// unbounded-rank report names the component — but cannot overflow a
    /// per-traversal row: the catalog still derives and every ceiling stays
    /// at the single-block traversal charge.
    #[test]
    fn natural_cycle_u64_catalog_stays_per_traversal_when_entry_overflows() {
        let walk = ranked_countdown_machine(64);
        let component = terminal_verifier::cyclic_component_identity(&walk, &[id(2), id(3)]);
        let module = module(1, vec![walk]);
        assert_eq!(
            derive_maximum_entry_bound(&module, id(1)),
            Err(FixedFuelError::UnboundedCycleComponent {
                component,
                cause: crate::UnboundedCycleCause::UnboundedRank,
            })
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

    /// A segment whose bound must consult a component's rank-multiplied
    /// interior reports the unbounded-rank cause when that interior cannot
    /// be represented in the certificate's `u64` ceiling — the same
    /// component-and-cause shape the whole-entry report carries, not a
    /// flat overflow. A `u128` carrier reaches it through the checked visit
    /// arithmetic itself; a `u64` carrier through the ceiling gate.
    #[test]
    fn natural_cycle_segment_reports_unbounded_rank_component() {
        for bits in [64_u16, 128_u16] {
            let walk = ranked_countdown_machine(bits);
            let component = terminal_verifier::cyclic_component_identity(&walk, &[id(2), id(3)]);
            let module = module(1, vec![walk]);
            let subject = PreparedFuelModule::new(&module);
            let prepared = PreparedSegments::new(&subject, id(1)).expect("machine prepares");
            assert_eq!(
                prepared
                    .segment_certificate(id(1), id(5), &mut BTreeMap::new())
                    .map(|certificate| certificate.ceiling_units),
                Err(FixedFuelError::UnboundedCycleComponent {
                    component,
                    cause: crate::UnboundedCycleCause::UnboundedRank,
                }),
                "segment crossing a wide-rank component names the component, \
                 not a flat overflow (u{bits} carrier)"
            );
        }
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

    /// Only an unconditional literal ceiling binds: a disjunction with an
    /// unbounded arm gives none, an implication whose premise the ambient
    /// rows cannot discharge stays conditional, a literal on the left of
    /// `<=` is a lower bound, a wrong-typed or signed literal cannot cap
    /// the unsigned carrier, a clause naming another value binds nothing,
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
            // `p <= 5` holds only when `q <= 4` does, and no ambient row
            // shows that — a conditional ceiling binds nothing alone.
            Proposition::Implication {
                premise: Box::new(Proposition::LessOrEqual(
                    parameter_term(101, rank_type),
                    integer_literal(rank_type, 4),
                )),
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
                used_premises(&module, module.machines.first().expect("one machine")).is_empty(),
                "a clause the bound never rests on is not a premise"
            );
        }
    }

    /// An arm derives its ceiling against the ambient rows too: under
    /// `p <= q` the row `q <= 4` still holds, so that arm caps `p` at 4
    /// and the disjunction caps it at `max(4, 7)`. The certificate binds
    /// the disjunction row and the ambient row the first arm consulted.
    #[test]
    fn contract_requires_disjunction_arm_uses_ambient_rows() {
        let rank_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let relation = Proposition::LessOrEqual(
            parameter_term(101, rank_type),
            integer_literal(rank_type, 4),
        );
        let disjunction = Proposition::Disjunction(vec![
            Proposition::LessOrEqual(
                parameter_term(100, rank_type),
                parameter_term(101, rank_type),
            ),
            Proposition::LessOrEqual(
                parameter_term(100, rank_type),
                integer_literal(rank_type, 7),
            ),
        ]);
        let mut walk = ranked_countdown_machine(8);
        walk.parameters.push(ValueDeclaration {
            qualifications: Default::default(),
            id: id(101),
            scalar_type: ScalarType::Integer(rank_type),
        });
        walk.contract.requires = vec![relation.clone(), disjunction.clone()];
        let module = module(1, vec![walk]);
        assert_eq!(
            derive_maximum_entry_bound(&module, id(1)),
            Ok(1 + 4 * 8 + 1)
        );
        assert_eq!(
            used_premises(&module, module.machines.first().expect("one machine")),
            vec![relation, disjunction],
            "the bound rests on the disjunction row plus the ambient row \
             the first arm's chain consulted"
        );
    }

    /// A relational leaf every arm states is unconditional: `p <= q` in
    /// one arm and `p < q` in the other still entail `p <= q`, which the
    /// ambient `q <= 5` then caps. An arm that leaves the relation out
    /// shares nothing, so the same contract without it binds no chain.
    #[test]
    fn contract_requires_disjunction_shares_common_edges() {
        let rank_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let ceiling = Proposition::LessOrEqual(
            parameter_term(101, rank_type),
            integer_literal(rank_type, 5),
        );
        let disjunction = Proposition::Disjunction(vec![
            Proposition::Conjunction(vec![
                Proposition::LessOrEqual(
                    parameter_term(100, rank_type),
                    parameter_term(101, rank_type),
                ),
                Proposition::LessOrEqual(
                    parameter_term(100, rank_type),
                    integer_literal(rank_type, 9),
                ),
            ]),
            Proposition::Conjunction(vec![
                Proposition::LessThan(
                    parameter_term(100, rank_type),
                    parameter_term(101, rank_type),
                ),
                Proposition::LessOrEqual(
                    parameter_term(100, rank_type),
                    integer_literal(rank_type, 9),
                ),
            ]),
        ]);
        let mut walk = ranked_countdown_machine(8);
        walk.parameters.push(ValueDeclaration {
            qualifications: Default::default(),
            id: id(101),
            scalar_type: ScalarType::Integer(rank_type),
        });
        walk.contract.requires = vec![ceiling.clone(), disjunction.clone()];
        let built = module(1, vec![walk]);
        // Each arm caps p at min(9, 5) — the arm-local `p <= q` edge
        // transfers q's ambient 5 inside the arm — and the disjunction
        // keeps the maximum across arms: 5.
        assert_eq!(derive_maximum_entry_bound(&built, id(1)), Ok(1 + 4 * 6 + 1));
        assert_eq!(
            used_premises(&built, built.machines.first().expect("one machine")),
            vec![ceiling, disjunction]
        );

        // The shared edge itself is load-bearing when the target's bound
        // arrives only through a later conditional row: `(p <= q) or
        // (p < q)` alone caps nothing since q is then unbounded, but the
        // unconditional `p <= q` it entails transfers the second
        // disjunction's `q <= 4` at the outer level.
        let first = Proposition::Disjunction(vec![
            Proposition::LessOrEqual(
                parameter_term(100, rank_type),
                parameter_term(101, rank_type),
            ),
            Proposition::LessThan(
                parameter_term(100, rank_type),
                parameter_term(101, rank_type),
            ),
        ]);
        let second = Proposition::Disjunction(vec![
            Proposition::LessOrEqual(
                parameter_term(101, rank_type),
                integer_literal(rank_type, 3),
            ),
            Proposition::LessOrEqual(
                parameter_term(101, rank_type),
                integer_literal(rank_type, 4),
            ),
        ]);
        let mut walk = ranked_countdown_machine(8);
        walk.parameters.push(ValueDeclaration {
            qualifications: Default::default(),
            id: id(101),
            scalar_type: ScalarType::Integer(rank_type),
        });
        walk.contract.requires = vec![first.clone(), second.clone()];
        let rebuilt = module(1, vec![walk]);
        assert_eq!(
            derive_maximum_entry_bound(&rebuilt, id(1)),
            Ok(1 + 4 * 5 + 1)
        );
        assert_eq!(
            used_premises(&rebuilt, rebuilt.machines.first().expect("one machine")),
            vec![first, second],
            "the bound rests on the row stating the shared edge and the \
             row bounding its target"
        );
    }

    /// An arm that cannot hold under any valuation drops out of the
    /// maximum instead of sinking the row: `p <= 5` or an impossible
    /// `p < 0` is simply `p <= 5`. A disjunction with no live arm at all
    /// contributes nothing.
    #[test]
    fn contract_requires_disjunction_skips_unsatisfiable_arms() {
        let rank_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let disjunction = Proposition::Disjunction(vec![
            Proposition::LessOrEqual(
                parameter_term(100, rank_type),
                integer_literal(rank_type, 5),
            ),
            Proposition::LessThan(
                parameter_term(100, rank_type),
                integer_literal(rank_type, 0),
            ),
        ]);
        let mut walk = ranked_countdown_machine(8);
        walk.contract.requires = vec![disjunction];
        let built = module(1, vec![walk]);
        assert_eq!(derive_maximum_entry_bound(&built, id(1)), Ok(1 + 4 * 6 + 1));

        let mut walk = ranked_countdown_machine(8);
        walk.contract.requires = vec![Proposition::Disjunction(vec![
            Proposition::Falsehood,
            Proposition::LessThan(
                parameter_term(100, rank_type),
                integer_literal(rank_type, 0),
            ),
        ])];
        let rebuilt = module(1, vec![walk]);
        assert_eq!(
            derive_maximum_entry_bound(&rebuilt, id(1)),
            Ok(1 + 4 * 256 + 1),
            "a vacuous disjunction contributes no ceiling"
        );
    }

    /// An implication row whose premise the ambient rows discharge makes
    /// its conclusion unconditional: `True -> p <= 5`, `q <= 4 -> p <= 5`
    /// under `q <= 3`, `p <= q -> p <= 5` under a chain that derives
    /// `p <= q`, and a conjunctive premise needing every child. Each binds
    /// the implication row plus the rows that discharged its premise.
    #[test]
    fn contract_requires_implication_discharged_premise_tightens() {
        let rank_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let conclusion = || {
            Proposition::LessOrEqual(
                parameter_term(100, rank_type),
                integer_literal(rank_type, 5),
            )
        };
        // `True -> p <= 5` is simply `p <= 5`.
        let mut walk = ranked_countdown_machine(8);
        walk.contract.requires = vec![Proposition::Implication {
            premise: Box::new(Proposition::Truth),
            conclusion: Box::new(conclusion()),
        }];
        let built = module(1, vec![walk]);
        assert_eq!(derive_maximum_entry_bound(&built, id(1)), Ok(1 + 4 * 6 + 1));

        // `q <= 4 -> p <= 5` under `q <= 3`: the settled ceiling on q
        // already fits the premise's bound.
        let ambient = Proposition::LessOrEqual(
            parameter_term(101, rank_type),
            integer_literal(rank_type, 3),
        );
        let implication = Proposition::Implication {
            premise: Box::new(Proposition::LessOrEqual(
                parameter_term(101, rank_type),
                integer_literal(rank_type, 4),
            )),
            conclusion: Box::new(conclusion()),
        };
        let mut walk = ranked_countdown_machine(8);
        walk.parameters.push(ValueDeclaration {
            qualifications: Default::default(),
            id: id(101),
            scalar_type: ScalarType::Integer(rank_type),
        });
        walk.contract.requires = vec![ambient.clone(), implication.clone()];
        let rebuilt = module(1, vec![walk]);
        assert_eq!(
            derive_maximum_entry_bound(&rebuilt, id(1)),
            Ok(1 + 4 * 6 + 1)
        );
        assert_eq!(
            used_premises(&rebuilt, rebuilt.machines.first().expect("one machine")),
            vec![ambient, implication],
            "the certificate binds the implication row and the row that \
             discharged its premise"
        );

        // `p <= q -> p <= 5` under `p <= r` and `r <= q`: the chain
        // derives the premise without a verbatim row.
        let hops = [
            Proposition::LessOrEqual(
                parameter_term(100, rank_type),
                parameter_term(102, rank_type),
            ),
            Proposition::LessOrEqual(
                parameter_term(102, rank_type),
                parameter_term(101, rank_type),
            ),
        ];
        let implication = Proposition::Implication {
            premise: Box::new(Proposition::LessOrEqual(
                parameter_term(100, rank_type),
                parameter_term(101, rank_type),
            )),
            conclusion: Box::new(conclusion()),
        };
        let mut walk = ranked_countdown_machine(8);
        for raw in [101, 102] {
            walk.parameters.push(ValueDeclaration {
                qualifications: Default::default(),
                id: id(raw),
                scalar_type: ScalarType::Integer(rank_type),
            });
        }
        walk.contract.requires = vec![hops[0].clone(), hops[1].clone(), implication.clone()];
        let rebuilt = module(1, vec![walk]);
        assert_eq!(
            derive_maximum_entry_bound(&rebuilt, id(1)),
            Ok(1 + 4 * 6 + 1)
        );
        assert_eq!(
            used_premises(&rebuilt, rebuilt.machines.first().expect("one machine")),
            vec![hops[0].clone(), hops[1].clone(), implication],
            "the discharging chain's rows join the premise set"
        );

        // A conjunctive premise needs every child discharged.
        let implication = Proposition::Implication {
            premise: Box::new(Proposition::Conjunction(vec![
                Proposition::LessOrEqual(
                    parameter_term(101, rank_type),
                    integer_literal(rank_type, 3),
                ),
                Proposition::Truth,
            ])),
            conclusion: Box::new(conclusion()),
        };
        let ambient = Proposition::LessOrEqual(
            parameter_term(101, rank_type),
            integer_literal(rank_type, 3),
        );
        let mut walk = ranked_countdown_machine(8);
        walk.parameters.push(ValueDeclaration {
            qualifications: Default::default(),
            id: id(101),
            scalar_type: ScalarType::Integer(rank_type),
        });
        walk.contract.requires = vec![ambient, implication];
        let rebuilt = module(1, vec![walk]);
        assert_eq!(
            derive_maximum_entry_bound(&rebuilt, id(1)),
            Ok(1 + 4 * 6 + 1)
        );
    }

    /// A `Natural` countdown entered through a conditional edge: entry 1
    /// computes `condition` by comparing two machine-visible values, then
    /// the `enter` arm passes machine parameter `initial` (id 100) into
    /// header 2's rank parameter `rank` (id 200) while the other arm
    /// leaves to return block 5. `enter_on_true` selects which arm enters
    /// the component, and `condition` is the comparison value the
    /// conditional reads. The component and its {2,3} ranking match
    /// `ranked_countdown_machine`.
    fn guarded_countdown_machine(
        enter_on_true: bool,
        condition: TerminatorCondition,
    ) -> TerminalMachine {
        let rank_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let scalar = ScalarType::Integer(rank_type);
        let value = |raw: u64| ValueDeclaration {
            qualifications: Default::default(),
            id: id(raw),
            scalar_type: scalar,
        };
        // The entering arm carries `initial` into the header's rank
        // parameter; the leaving arm takes no arguments.
        let (enter_edge, leave_edge) = if enter_on_true { (1, 6) } else { (6, 1) };
        let successor =
            |edge: u64, target: u64, arguments: Vec<ValueId>| terminal_psi::SuccessorEdge {
                edge: id(edge),
                target: id(target),
                arguments,
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                structural_arguments: Vec::new(),
                trivial_affine_discards: Vec::new(),
            };
        let (when_true, when_false) = if enter_on_true {
            (
                successor(enter_edge, 2, vec![id(100)]),
                successor(leave_edge, 5, Vec::new()),
            )
        } else {
            (
                successor(leave_edge, 5, Vec::new()),
                successor(enter_edge, 2, vec![id(100)]),
            )
        };
        let mut semantic = machine(
            1,
            1,
            vec![
                block(
                    1,
                    condition.operations,
                    Terminator::Conditional {
                        condition: condition.value,
                        when_true,
                        when_false,
                    },
                ),
                Block {
                    erased_scalar_formals: Vec::new(),
                    erased_proof_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: id(2),
                    parameters: vec![value(200)],
                    operations: vec![boolean_constant(20, 9_001, true)],
                    terminator: conditional_arguments(9_001, 2, 3, Vec::new(), 3, 4, Vec::new()),
                },
                block(
                    3,
                    vec![integer_constant(30, 300, 0)],
                    jump_arguments(4, 2, vec![id(300)]),
                ),
                block(4, Vec::new(), return_unit(5)),
                block(5, Vec::new(), return_unit(7)),
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
                    rank_edge(2, 2, 3, TerminalNaturalRankComparison::Preserving),
                    rank_edge(4, 3, 2, TerminalNaturalRankComparison::Strict),
                ],
            }])),
        );
        semantic.parameters = vec![value(100)];
        semantic
    }

    /// The condition value a `guarded_countdown_machine` entry block
    /// computes: `operations` run before the branch reads `value`.
    struct TerminatorCondition {
        operations: Vec<Operation>,
        value: ValueId,
    }

    /// `left < right` where `literal` is the constant operation's value —
    /// `less_than_condition(100, 101, 6)` computes `initial < 6` and
    /// `less_than_condition(101, 100, 6)` computes `6 < initial`, so both
    /// operand orders exercise both arm polarities.
    fn less_than_condition(left: u64, right: u64, literal: u64) -> TerminatorCondition {
        TerminatorCondition {
            operations: vec![
                integer_constant(10, 101, literal),
                integer_less_than(11, 9_000, left, right),
            ],
            value: id(9_000),
        }
    }

    /// `not inner` — a `BooleanNot` over another condition's value.
    fn negated_condition(inner: TerminatorCondition) -> TerminatorCondition {
        TerminatorCondition {
            operations: inner
                .operations
                .into_iter()
                .chain([Operation {
                    static_reach_binding: None,
                    suspension_crossing: None,
                    id: id(12),
                    result: OperationResult::Scalar(ValueDeclaration {
                        id: id(9_002),
                        scalar_type: ScalarType::Boolean,
                        qualifications: Default::default(),
                    }),
                    kind: OperationKind::BooleanNot {
                        operand: inner.value,
                    },
                }])
                .collect(),
            value: id(9_002),
        }
    }

    /// A conditional whose condition is a constant — the unselected arm is
    /// infeasible, so an edge it would carry needs no rank bound.
    fn constant_condition(value: bool) -> TerminatorCondition {
        TerminatorCondition {
            operations: vec![boolean_constant(10, 9_000, value)],
            value: id(9_000),
        }
    }

    /// The false arm reads the negated condition: entering on
    /// `!(initial < 6)` — reconstructed as `6 <= initial` — places no
    /// ceiling on the argument and leaves the carrier maximum standing,
    /// while `!(6 < initial)` — `initial <= 6` — caps it at 6 through the
    /// same `Guard` channel.
    #[test]
    fn edge_guard_false_arm_uses_the_negated_polarity() {
        let rank_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        // False arm of `100 < 6` is `6 <= 100`: a lower bound on the
        // arrival, so the carrier's 255 stands.
        let built = module(
            1,
            vec![guarded_countdown_machine(
                false,
                less_than_condition(100, 101, 6),
            )],
        );
        assert_eq!(
            derive_maximum_entry_bound(&built, id(1)),
            Ok(3 + 4 * 256 + 1),
            "a lower-bound guard tightens no entry rank"
        );
        assert!(
            used_premises(&built, built.machines.first().expect("one machine")).is_empty(),
            "a guard that bounds nothing is no premise"
        );

        // False arm of `6 < 100` is `100 <= 6`: a true ceiling, so seven
        // rank observations arrive.
        let rebuilt = module(
            1,
            vec![guarded_countdown_machine(
                false,
                less_than_condition(101, 100, 6),
            )],
        );
        assert_eq!(
            derive_maximum_entry_bound(&rebuilt, id(1)),
            Ok(3 + 4 * 7 + 1)
        );
        assert_eq!(
            used_premises(&rebuilt, rebuilt.machines.first().expect("one machine")),
            vec![Proposition::LessOrEqual(
                parameter_term(100, rank_type),
                integer_literal(rank_type, 6),
            )],
            "the false arm's premise is the negated comparison"
        );
    }

    /// `BooleanNot` inverts the arm's polarity: entering on the false arm
    /// of `not (6 < initial)` selects `not (initial <= 6)` —
    /// `6 < initial` — which bounds nothing, while the true arm selects
    /// `initial <= 6` and caps the entry.
    #[test]
    fn edge_guard_boolean_not_threads_polarity() {
        let rank_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let built = module(
            1,
            vec![guarded_countdown_machine(
                true,
                negated_condition(less_than_condition(101, 100, 6)),
            )],
        );
        assert_eq!(
            derive_maximum_entry_bound(&built, id(1)),
            Ok(4 + 4 * 7 + 1),
            "not (6 < 100) is 100 <= 6 on the true arm"
        );
        assert_eq!(
            used_premises(&built, built.machines.first().expect("one machine")),
            vec![Proposition::LessOrEqual(
                parameter_term(100, rank_type),
                integer_literal(rank_type, 6),
            )]
        );

        let rebuilt = module(
            1,
            vec![guarded_countdown_machine(
                false,
                negated_condition(less_than_condition(101, 100, 6)),
            )],
        );
        assert_eq!(
            derive_maximum_entry_bound(&rebuilt, id(1)),
            Ok(4 + 4 * 256 + 1),
            "not (100 <= 6) is 6 < 100: no ceiling on the arrival"
        );
    }

    /// An edge no execution can take carries no arrival: a constant-false
    /// condition marks the entering edge infeasible, so the component's
    /// first-entry coverage needs no bound for it — the component admits
    /// rank 0 on its single visit slot rather than inheriting the carrier
    /// maximum from an arrival that cannot exist.
    #[test]
    fn edge_guard_constant_condition_marks_the_arm_infeasible() {
        let built = module(
            1,
            vec![guarded_countdown_machine(true, constant_condition(false))],
        );
        // The true arm can never select the component: bound = entry
        // (bconst + conditional = 2) + one visit slot (4) + exit (1). The
        // same entry with a feasible but unbounded arm keeps the carrier
        // maximum instead.
        assert_eq!(derive_maximum_entry_bound(&built, id(1)), Ok(2 + 4 + 1));

        let rebuilt = module(
            1,
            vec![guarded_countdown_machine(true, constant_condition(true))],
        );
        assert_eq!(
            derive_maximum_entry_bound(&rebuilt, id(1)),
            Ok(2 + 4 * 256 + 1),
            "a feasible edge's unbounded argument keeps the carrier maximum"
        );
    }

    /// A verified header invariant covers every first entry at once:
    /// `rank <= 5` on `(machine 1, header 2)` caps the member's observed
    /// rank regardless of what each incoming edge passes, and the
    /// certificate binds the invariant predicate through its own channel —
    /// not as a contract row.
    #[test]
    fn header_invariant_tightens_without_a_contract_row() {
        let rank_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let predicate = Proposition::LessOrEqual(
            parameter_term(200, rank_type),
            integer_literal(rank_type, 5),
        );
        let mut module = module(1, vec![ranked_countdown_machine(8)]);
        module.scalar_block_invariants = vec![scalar_invariant(1, 2, predicate.clone(), &[1])];
        assert!(
            module
                .machines
                .first()
                .expect("one machine")
                .contract
                .requires
                .is_empty(),
            "no contract row exists to be miscredited"
        );
        assert_eq!(
            derive_maximum_entry_bound(&module, id(1)),
            Ok(1 + 4 * 6 + 1)
        );
        assert_eq!(
            used_premises(&module, module.machines.first().expect("one machine")),
            vec![predicate],
            "the premise is the verified invariant predicate itself"
        );
    }

    /// A `MathValue` names a parameter's unbounded mathematical value:
    /// `IntegerMathLessOrEqual(MathValue p, k)` caps `p` at `k`, and the
    /// linear solver folds the surrounding arithmetic — `2*p + 3 <= 9`
    /// still caps `p` at 3, `p < 6` at 5, `p == 7` at 7.
    #[test]
    fn contract_requires_integer_math_affine_ceilings() {
        let rank_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let math_value = |parameter: u64| IntegerMathTerm::MathValue {
            source_type: rank_type,
            value: id(parameter),
        };
        let math_literal = |value: u128| IntegerMathTerm::literal(IntegerValue::Unsigned(value));
        let cases = [
            // `p <= 5`.
            (
                Proposition::IntegerMathLessOrEqual(math_value(100), math_literal(5)),
                6,
            ),
            // `2*p + 3 <= 9` — `p <= 3`.
            (
                Proposition::IntegerMathLessOrEqual(
                    IntegerMathTerm::Add(
                        Box::new(IntegerMathTerm::Multiply(
                            Box::new(math_literal(2)),
                            Box::new(math_value(100)),
                        )),
                        Box::new(math_literal(3)),
                    ),
                    math_literal(9),
                ),
                4,
            ),
            // `p < 6` — `p <= 5`.
            (
                Proposition::IntegerMathLessThan(math_value(100), math_literal(6)),
                6,
            ),
            // `p == 7` — an exact quotient is still a ceiling.
            (
                Proposition::IntegerMathEqual(math_value(100), math_literal(7)),
                8,
            ),
            // `p - 2 <= 5` — `p <= 7`.
            (
                Proposition::IntegerMathLessOrEqual(
                    IntegerMathTerm::Subtract(Box::new(math_value(100)), Box::new(math_literal(2))),
                    math_literal(5),
                ),
                8,
            ),
        ];
        for (clause, visits) in cases {
            let mut walk = ranked_countdown_machine(8);
            walk.contract.requires = vec![clause.clone()];
            let built = module(1, vec![walk]);
            assert_eq!(
                derive_maximum_entry_bound(&built, id(1)),
                Ok(1 + 4 * visits + 1),
                "{clause:?} did not tighten the entry rank"
            );
            assert_eq!(
                used_premises(&built, built.machines.first().expect("one machine")),
                vec![clause]
            );
        }
    }

    /// Mathematical orderings between values transfer ceilings the same
    /// way scalar comparisons do: `MathValue p <= MathValue q` hands `p`
    /// `q`'s bound, and `p + 2 <= q` deducts the constant. `p == q` keeps
    /// the bidirectional zero-cost pair.
    #[test]
    fn contract_requires_integer_math_relational_ceilings() {
        let rank_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let math_value = |parameter: u64| IntegerMathTerm::MathValue {
            source_type: rank_type,
            value: id(parameter),
        };
        let math_literal = |value: u128| IntegerMathTerm::literal(IntegerValue::Unsigned(value));
        let q_ceiling = Proposition::IntegerMathLessOrEqual(math_value(101), math_literal(4));
        let walk = |clause: Proposition| {
            let mut walk = ranked_countdown_machine(8);
            walk.parameters.push(ValueDeclaration {
                qualifications: Default::default(),
                id: id(101),
                scalar_type: ScalarType::Integer(rank_type),
            });
            walk.contract.requires = vec![q_ceiling.clone(), clause];
            module(1, vec![walk])
        };
        // `p <= q` under `q <= 4` — `p <= 4`.
        let built = walk(Proposition::IntegerMathLessOrEqual(
            math_value(100),
            math_value(101),
        ));
        assert_eq!(derive_maximum_entry_bound(&built, id(1)), Ok(1 + 4 * 5 + 1));
        // `p + 2 <= q` under `q <= 4` — `p <= 2`.
        let built = walk(Proposition::IntegerMathLessOrEqual(
            IntegerMathTerm::Add(Box::new(math_value(100)), Box::new(math_literal(2))),
            math_value(101),
        ));
        assert_eq!(derive_maximum_entry_bound(&built, id(1)), Ok(1 + 4 * 3 + 1));
        // `p == q` under `q <= 4` — `p <= 4`.
        let built = walk(Proposition::IntegerMathEqual(
            math_value(100),
            math_value(101),
        ));
        assert_eq!(derive_maximum_entry_bound(&built, id(1)), Ok(1 + 4 * 5 + 1));
        // `p <= q + 2` is a slack relation the transfer edges cannot
        // express — the bound it contributes is `q`'s whole carrier
        // maximum plus two, which tightens nothing.
        let built = walk(Proposition::IntegerMathLessOrEqual(
            math_value(100),
            IntegerMathTerm::Add(Box::new(math_value(101)), Box::new(math_literal(2))),
        ));
        assert_eq!(
            derive_maximum_entry_bound(&built, id(1)),
            Ok(1 + 4 * 256 + 1)
        );
        // `2*p <= q` names a non-unit coefficient: `q`'s own carrier
        // maximum still caps it at `p <= 255 / 2` — a real bound, not a
        // transfer edge.
        let built = walk(Proposition::IntegerMathLessOrEqual(
            IntegerMathTerm::Multiply(Box::new(math_literal(2)), Box::new(math_value(100))),
            math_value(101),
        ));
        assert_eq!(
            derive_maximum_entry_bound(&built, id(1)),
            Ok(1 + 4 * 128 + 1)
        );
        // `p * q <= 100` is nonlinear — the linear form does not solve
        // and the row contributes nothing.
        let built = walk(Proposition::IntegerMathLessOrEqual(
            IntegerMathTerm::Multiply(Box::new(math_value(100)), Box::new(math_value(101))),
            math_literal(100),
        ));
        assert_eq!(
            derive_maximum_entry_bound(&built, id(1)),
            Ok(1 + 4 * 256 + 1)
        );
    }

    /// Mathematical contradictions prune disjunction arms like scalar
    /// ones: `p <= 5` or impossible `p < 0` is `p <= 5`, and an equality
    /// with no solution — `2*p == 5` over integers — is unsatisfiable on
    /// its own. A mathematical premise also discharges against a settled
    /// scalar ceiling.
    #[test]
    fn contract_requires_integer_math_conditional_forms() {
        let rank_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let math_value = |parameter: u64| IntegerMathTerm::MathValue {
            source_type: rank_type,
            value: id(parameter),
        };
        let math_literal = |value: u128| IntegerMathTerm::literal(IntegerValue::Unsigned(value));
        let math_signed_literal =
            |value: i128| IntegerMathTerm::literal(IntegerValue::Signed(value));
        // `(p <= 5) or (p < 0)`.
        let disjunction = Proposition::Disjunction(vec![
            Proposition::IntegerMathLessOrEqual(math_value(100), math_literal(5)),
            Proposition::IntegerMathLessThan(math_value(100), math_signed_literal(0)),
        ]);
        let mut walk = ranked_countdown_machine(8);
        walk.contract.requires = vec![disjunction.clone()];
        let built = module(1, vec![walk]);
        assert_eq!(derive_maximum_entry_bound(&built, id(1)), Ok(1 + 4 * 6 + 1));
        assert_eq!(
            used_premises(&built, built.machines.first().expect("one machine")),
            vec![disjunction]
        );

        // `(2*p == 5) or (p <= 3)` — the impossible arm drops out.
        let disjunction = Proposition::Disjunction(vec![
            Proposition::IntegerMathEqual(
                IntegerMathTerm::Multiply(Box::new(math_literal(2)), Box::new(math_value(100))),
                math_literal(5),
            ),
            Proposition::IntegerMathLessOrEqual(math_value(100), math_literal(3)),
        ]);
        let mut walk = ranked_countdown_machine(8);
        walk.contract.requires = vec![disjunction.clone()];
        let built = module(1, vec![walk]);
        assert_eq!(derive_maximum_entry_bound(&built, id(1)), Ok(1 + 4 * 4 + 1));
        assert_eq!(
            used_premises(&built, built.machines.first().expect("one machine")),
            vec![disjunction]
        );

        // `q <= 3` discharges the math premise `q <= 4`, whose conclusion
        // then caps `p` at 5 — the ceiling belongs to the implication row,
        // not the ambient one.
        let ambient = Proposition::LessOrEqual(
            parameter_term(101, rank_type),
            integer_literal(rank_type, 3),
        );
        let implication = Proposition::Implication {
            premise: Box::new(Proposition::IntegerMathLessOrEqual(
                math_value(101),
                math_literal(4),
            )),
            conclusion: Box::new(Proposition::IntegerMathLessOrEqual(
                math_value(100),
                math_literal(5),
            )),
        };
        let mut walk = ranked_countdown_machine(8);
        walk.parameters.push(ValueDeclaration {
            qualifications: Default::default(),
            id: id(101),
            scalar_type: ScalarType::Integer(rank_type),
        });
        walk.contract.requires = vec![ambient.clone(), implication.clone()];
        let built = module(1, vec![walk]);
        assert_eq!(derive_maximum_entry_bound(&built, id(1)), Ok(1 + 4 * 6 + 1));
        assert_eq!(
            used_premises(&built, built.machines.first().expect("one machine")),
            vec![ambient, implication]
        );

        // An affine premise discharges through a chain: `p + 1 <= q`
        // follows from `p < q` alone — the strict hop's cost is the 1 the
        // premise needs, and the `q <= 9` ceiling row is not consulted.
        let hops = [
            Proposition::LessOrEqual(
                parameter_term(101, rank_type),
                integer_literal(rank_type, 9),
            ),
            Proposition::LessThan(
                parameter_term(100, rank_type),
                parameter_term(101, rank_type),
            ),
        ];
        let implication = Proposition::Implication {
            premise: Box::new(Proposition::IntegerMathLessOrEqual(
                IntegerMathTerm::Add(Box::new(math_value(100)), Box::new(math_literal(1))),
                math_value(101),
            )),
            conclusion: Box::new(Proposition::IntegerMathLessOrEqual(
                math_value(100),
                math_literal(5),
            )),
        };
        let mut walk = ranked_countdown_machine(8);
        walk.parameters.push(ValueDeclaration {
            qualifications: Default::default(),
            id: id(101),
            scalar_type: ScalarType::Integer(rank_type),
        });
        walk.contract.requires = vec![hops[0].clone(), hops[1].clone(), implication.clone()];
        let built = module(1, vec![walk]);
        assert_eq!(derive_maximum_entry_bound(&built, id(1)), Ok(1 + 4 * 6 + 1));
        assert_eq!(
            used_premises(&built, built.machines.first().expect("one machine")),
            vec![hops[1].clone(), implication],
            "the premise rests on the strict hop; the unconsulted ceiling \
             row stays out"
        );
    }

    /// A clause naming several values still bounds each
    /// positive-coefficient variable once the rest collapse at their
    /// carrier extremes: `p + q <= 9` caps `p` at 9 since `q` bottoms at
    /// zero, `2*p + 3*q <= 19` caps `p` at 9 through the floor division,
    /// and `p + s <= 10` with `s` a signed i8 caps `p` at 138 because `s`
    /// can absorb −128 of the constant. The other variable needs no
    /// clause of its own — its carrier range prices the collapse.
    #[test]
    fn contract_requires_integer_math_linear_row_ceilings() {
        let rank_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let math_value = |parameter: u64| IntegerMathTerm::MathValue {
            source_type: rank_type,
            value: id(parameter),
        };
        let math_literal = |value: u128| IntegerMathTerm::literal(IntegerValue::Unsigned(value));
        let walk = |clause: Proposition, extra: Option<IntegerSign>| {
            let mut walk = ranked_countdown_machine(8);
            if let Some(sign) = extra {
                walk.parameters.push(ValueDeclaration {
                    qualifications: Default::default(),
                    id: id(101),
                    scalar_type: ScalarType::Integer(
                        IntegerType::new(sign, 8).expect("parameter type"),
                    ),
                });
            }
            walk.contract.requires = vec![clause];
            module(1, vec![walk])
        };
        // `p + q <= 9` — `p <= 9`.
        let built = walk(
            Proposition::IntegerMathLessOrEqual(
                IntegerMathTerm::Add(Box::new(math_value(100)), Box::new(math_value(101))),
                math_literal(9),
            ),
            Some(IntegerSign::Unsigned),
        );
        assert_eq!(
            derive_maximum_entry_bound(&built, id(1)),
            Ok(1 + 4 * 10 + 1)
        );
        // `2*p + 3*q <= 19` — `p <= 9`.
        let built = walk(
            Proposition::IntegerMathLessOrEqual(
                IntegerMathTerm::Add(
                    Box::new(IntegerMathTerm::Multiply(
                        Box::new(math_literal(2)),
                        Box::new(math_value(100)),
                    )),
                    Box::new(IntegerMathTerm::Multiply(
                        Box::new(math_literal(3)),
                        Box::new(math_value(101)),
                    )),
                ),
                math_literal(19),
            ),
            Some(IntegerSign::Unsigned),
        );
        assert_eq!(
            derive_maximum_entry_bound(&built, id(1)),
            Ok(1 + 4 * 10 + 1)
        );
        // `p + s <= 10` with `s` signed — `p <= 10 + 128`.
        let signed_value = IntegerMathTerm::MathValue {
            source_type: signed,
            value: id(101),
        };
        let built = walk(
            Proposition::IntegerMathLessOrEqual(
                IntegerMathTerm::Add(Box::new(math_value(100)), Box::new(signed_value)),
                math_literal(10),
            ),
            Some(IntegerSign::Signed),
        );
        assert_eq!(
            derive_maximum_entry_bound(&built, id(1)),
            Ok(1 + 4 * 139 + 1)
        );
        // `p <= q + r` collapses both right-side carriers at their
        // maxima — on a `u16` rank `p <= 510` tightens.
        let narrow_value = |parameter: u64| IntegerMathTerm::MathValue {
            source_type: rank_type,
            value: id(parameter),
        };
        let mut wide = ranked_countdown_machine(16);
        for (parameter, scalar_type) in [(101, rank_type), (102, rank_type)] {
            wide.parameters.push(ValueDeclaration {
                qualifications: Default::default(),
                id: id(parameter),
                scalar_type: ScalarType::Integer(scalar_type),
            });
        }
        wide.contract.requires = vec![Proposition::IntegerMathLessOrEqual(
            IntegerMathTerm::MathValue {
                source_type: IntegerType::new(IntegerSign::Unsigned, 16).expect("u16"),
                value: id(100),
            },
            IntegerMathTerm::Add(Box::new(narrow_value(101)), Box::new(narrow_value(102))),
        )];
        let built = module(1, vec![wide]);
        assert_eq!(
            derive_maximum_entry_bound(&built, id(1)),
            Ok(1 + 4 * 511 + 1),
            "`p <= q + r` caps `p` at 255 + 255 on a u16 rank carrier"
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
        assert!(
            used_premises(&computed, computed.machines.first().expect("one machine")).is_empty()
        );

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
            used_premises(&unbounded, unbounded.machines.first().expect("one machine")).is_empty()
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
        assert!(used_premises(&missing, missing.machines.first().expect("one machine")).is_empty());
    }

    /// A relational `requires` row derives a conditional ceiling no literal
    /// names: `p <= q` transfers `q`'s own contract ceiling to `p`, `p < q`
    /// transfers it less one, and `p == q` transfers it both ways. The
    /// certificate binds every contract row the achieving chain traverses —
    /// each is an independently discharged premise, so the tightened bound
    /// rests on checked clauses rather than a guessed ceiling.
    #[test]
    fn contract_requires_relational_chain_derives_a_conditional_ceiling() {
        let rank_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        for (relation, expected) in [
            (
                Proposition::LessOrEqual(
                    parameter_term(100, rank_type),
                    parameter_term(101, rank_type),
                ),
                1 + 4 * 6 + 1,
            ),
            (
                Proposition::LessThan(
                    parameter_term(100, rank_type),
                    parameter_term(101, rank_type),
                ),
                1 + 4 * 5 + 1,
            ),
            (
                Proposition::Equal(
                    parameter_term(100, rank_type),
                    parameter_term(101, rank_type),
                ),
                1 + 4 * 6 + 1,
            ),
            (
                Proposition::Equal(
                    parameter_term(101, rank_type),
                    parameter_term(100, rank_type),
                ),
                1 + 4 * 6 + 1,
            ),
        ] {
            let ceiling = Proposition::LessOrEqual(
                parameter_term(101, rank_type),
                integer_literal(rank_type, 5),
            );
            let mut walk = ranked_countdown_machine(8);
            walk.parameters.push(ValueDeclaration {
                qualifications: Default::default(),
                id: id(101),
                scalar_type: ScalarType::Integer(rank_type),
            });
            walk.contract.requires = vec![relation.clone(), ceiling.clone()];
            let module = module(1, vec![walk]);
            assert_eq!(
                derive_maximum_entry_bound(&module, id(1)),
                Ok(expected),
                "{relation:?}"
            );
            assert_eq!(
                used_premises(&module, module.machines.first().expect("one machine")),
                vec![relation, ceiling],
                "the bound binds every row its chain rests on"
            );
        }
    }

    /// A segment crossing a surviving interior cycle binds every chain row
    /// its tightened rank bound rested on, while rows outside the
    /// achieving derivation stay unbound.
    #[test]
    fn segment_certificate_binds_the_chain_its_interior_consulted() {
        let rank_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let relation = Proposition::LessOrEqual(
            parameter_term(100, rank_type),
            parameter_term(101, rank_type),
        );
        let ceiling = Proposition::LessOrEqual(
            parameter_term(101, rank_type),
            integer_literal(rank_type, 5),
        );
        let mut walk = ranked_countdown_machine(8);
        walk.parameters.push(ValueDeclaration {
            qualifications: Default::default(),
            id: id(101),
            scalar_type: ScalarType::Integer(rank_type),
        });
        walk.contract.requires = vec![relation.clone(), ceiling.clone()];
        let module = module(1, vec![walk]);
        let subject = PreparedFuelModule::new(&module);
        let prepared = PreparedSegments::new(&subject, id(1)).expect("machine prepares");

        let per_traversal = prepared
            .segment_certificate(id(1), id(1), &mut BTreeMap::new())
            .expect("entry-edge row derives");
        assert!(per_traversal.relevant_preconditions.is_empty());

        let cyclic = prepared
            .segment_certificate(id(1), id(3), &mut BTreeMap::new())
            .expect("entry-to-exit row derives");
        assert_eq!(cyclic.ceiling_units, 1 + 4 * 6);
        assert_eq!(
            cyclic.relevant_preconditions,
            vec![relation.clone(), ceiling.clone()],
            "the cyclic interior binds the whole chain it rests on"
        );
        let mid_component = prepared
            .segment_certificate(id(3), id(3), &mut BTreeMap::new())
            .expect("mid-component cyclic row derives");
        assert_eq!(
            mid_component.relevant_preconditions,
            vec![relation, ceiling]
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
            used_premises(&module, module.machines.first().expect("one machine")).is_empty(),
            "an unreachable component's clause is not a bound premise"
        );
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
            &module.scalar_block_invariants,
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
