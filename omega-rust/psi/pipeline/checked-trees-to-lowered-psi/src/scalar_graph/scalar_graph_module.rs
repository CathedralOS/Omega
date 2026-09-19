//! Scalar-graph terminal module assembly: `build_scalar_graph_module_in_namespace`
//! allocates the parameter and identity namespaces, emits every state through
//! `GraphEmission` (`state_emission`, `short_circuit_staging`), finalizes the
//! reserved groups (`pending_blocks`), then assembles the machine, contract
//! and proof bundle.
use super::{
    BTreeMap, Block, ContentPartitionComposition, ContractClause, EvidenceRoute, KnownDirectScalar,
    LoweredContentIdentityReshuffles, LoweredContentPartitionCompositions, LoweredPsi,
    LoweringError, MachineContract, MachineId, ObligationEvidence, OperationKind, OperationResult,
    PrimitiveJudgment, ProofBundle, Proposition, QualifiedScalarType, ScalarFloatRange, ScalarTerm,
    ScalarType, StructuralArgument, StructuralParameterDeclaration, StructuralPlaceDeclaration,
    StructuralPlaceKind, TERMINAL_MACHINE_IDENTITY_STRIDE, TerminalMachine, TerminalMachineResult,
    TerminalModule, Terminator, ValueDeclaration, VocabularyMarker, block_id, contract_id, edge_id,
    lower_checked_crash_route_buckets, merge_content_place_declaration, obligation_id,
    scalar_source_block, terminal_scalar_type, unsupported, value_id,
};
use crate::emission::boolean_control::PendingNestedBlockGroup;
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::emission::operation_emission::calls::CallEmissionContext;
use crate::scalar_graph::scalar_graph_lowering::prepared_graph::{
    LoweredScalarBranchState, LoweredScalarBranchTerminator, LoweredScalarEffect,
    PreparedScalarContract,
};

mod owned_parameters;
mod pending_blocks;
mod qualifications;
mod ranking;
mod short_circuit_staging;
mod state_emission;
use crate::scalar_graph::scalar_contracts;
use semantic_vocabulary::BlockId;

/// One scalar-graph machine's emission in flight: the prepared states and
/// their parameter namespaces, the identity counters, the shared operation
/// buffer and call context, and the blocks accumulated so far. `emit_state`
/// walks one state; `resolve_pending_blocks` finalizes reserved groups.
pub(super) struct GraphEmission<'a> {
    pub(super) states: &'a [LoweredScalarBranchState],
    pub(super) state_parameters: Vec<Vec<ValueDeclaration>>,
    /// Per-state proof-only erased formal rosters in authored order.
    pub(super) state_erased_formals: Vec<Vec<ValueDeclaration>>,
    pub(super) loop_plan:
        Option<&'a crate::scalar_graph::scalar_graph_lowering::cycles::ScalarLoopPlan>,
    pub(super) terminal_machine: MachineId,
    pub(super) identity_base: u64,
    pub(super) parameters: &'a [ValueDeclaration],
    pub(super) scalar_qualifications: terminal_psi::ScalarQualificationCatalog,
    pub(super) all_operations: OperationBuffer,
    pub(super) call_emission: CallEmissionContext<'a>,
    pub(super) next_edge_identity: u64,
    pub(super) next_block_identity: u64,
    pub(super) next_value_identity: u64,
    pub(super) pending_blocks: Vec<PendingNestedBlockGroup>,
    pub(super) inlined_blocks: Vec<Block>,
    pub(super) blocks: Vec<Block>,
}

/// One graph state as its emitter sees it before the body is emitted.
pub(super) struct StateFrame<'s> {
    pub(super) state: &'s LoweredScalarBranchState,
    pub(super) source_block: BlockId,
    pub(super) source_block_parameters: Vec<ValueDeclaration>,
    pub(super) current_parameters: &'s Vec<ValueDeclaration>,
    /// The emitting state's proof-only erased formal roster.
    pub(super) erased_formals: &'s [ValueDeclaration],
}

/// Continuations are built backward, but fresh record producer identities must
/// follow production order: independent affine cleanup uses reverse producer
/// order. Allocate operations in CFG reverse-postorder, retaining block indices
/// and their parameter/target namespace. Keep unrelated existing graph bytes
/// stable; they do not yet introduce statement-owned record lifetimes.
fn emission_order(states: &[LoweredScalarBranchState]) -> Vec<usize> {
    if !states.iter().any(|state| {
        state
            .structural_effects
            .iter()
            .any(|effect| matches!(effect, LoweredScalarEffect::EstablishRecord(_)))
    }) {
        return (0..states.len()).collect();
    }
    let mut visited = vec![false; states.len()];
    let mut pending = vec![(0, false)];
    let mut order = Vec::with_capacity(states.len());
    while let Some((position, completing)) = pending.pop() {
        if completing {
            order.push(position);
            continue;
        }
        if visited[position] {
            continue;
        }
        visited[position] = true;
        pending.push((position, true));
        match &states[position].terminator {
            LoweredScalarBranchTerminator::Jump { target, .. }
            | LoweredScalarBranchTerminator::Qualify { target, .. } => {
                pending.push((*target, false))
            }
            LoweredScalarBranchTerminator::Conditional {
                when_true_target,
                when_false_target,
                ..
            } => {
                pending.push((*when_false_target, false));
                pending.push((*when_true_target, false));
            }
            LoweredScalarBranchTerminator::Return { .. }
            | LoweredScalarBranchTerminator::Crash(_) => {}
        }
    }
    order.reverse();
    order
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_scalar_graph_module(
    states: &[LoweredScalarBranchState],
    result_type: QualifiedScalarType,
    scalar_qualifications: &terminal_psi::ScalarQualificationCatalog,
    contract: PreparedScalarContract,
    crash_routes: Vec<checked_trees::CrashRouteBucket>,
    identity_reshuffles: LoweredContentIdentityReshuffles,
    partition_compositions: LoweredContentPartitionCompositions,
    terminal_machine: MachineId,
    identity_base: u64,
    machine_ids: &[(symbols::SymbolHandle, MachineId)],
    requirement_counts: &[(symbols::SymbolHandle, usize)],
    loop_plan: Option<&crate::scalar_graph::scalar_graph_lowering::cycles::ScalarLoopPlan>,
) -> Result<LoweredPsi, LoweringError> {
    build_scalar_graph_module_in_namespace(
        states,
        result_type,
        scalar_qualifications,
        contract,
        crash_routes,
        identity_reshuffles,
        partition_compositions,
        terminal_machine,
        identity_base,
        machine_ids,
        requirement_counts,
        &[],
        loop_plan,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_scalar_graph_module_in_namespace(
    states: &[LoweredScalarBranchState],
    result_type: QualifiedScalarType,
    scalar_qualifications: &terminal_psi::ScalarQualificationCatalog,
    contract: PreparedScalarContract,
    crash_routes: Vec<checked_trees::CrashRouteBucket>,
    identity_reshuffles: LoweredContentIdentityReshuffles,
    partition_compositions: LoweredContentPartitionCompositions,
    terminal_machine: MachineId,
    identity_base: u64,
    machine_ids: &[(symbols::SymbolHandle, MachineId)],
    requirement_counts: &[(symbols::SymbolHandle, usize)],
    structural_parameters: &[StructuralParameterDeclaration],
    loop_plan: Option<&crate::scalar_graph::scalar_graph_lowering::cycles::ScalarLoopPlan>,
) -> Result<LoweredPsi, LoweringError> {
    let scalar_qualifications = scalar_qualifications.clone();
    let parameters = states[0]
        .parameter_types
        .iter()
        .enumerate()
        .map(|(index, scalar_type)| ValueDeclaration {
            id: value_id(
                identity_base
                    .checked_add(
                        u64::try_from(index).expect("parameter index fits a semantic identity"),
                    )
                    .expect("parameter identity base admits the parameter index")
                    .checked_add(1)
                    .expect("parameter identity is nonzero"),
            ),
            scalar_type: scalar_type.scalar_type,
            qualifications: scalar_type.qualifications,
        })
        .collect::<Vec<_>>();
    let erased_scalar_formals = states[0]
        .erased_formal_types
        .iter()
        .enumerate()
        .map(|(index, scalar_type)| ValueDeclaration {
            id: value_id(
                identity_base
                    .checked_add(
                        u64::try_from(parameters.len() + index)
                            .expect("erased formal index fits a semantic identity"),
                    )
                    .expect("erased formal identity base admits the roster index")
                    .checked_add(1)
                    .expect("erased formal identity is nonzero"),
            ),
            scalar_type: scalar_type.scalar_type,
            qualifications: scalar_type.qualifications,
        })
        .collect::<Vec<_>>();
    let crash_routes = lower_checked_crash_route_buckets(&crash_routes, &parameters)?;
    let mut next_value_identity = identity_base
        .checked_add(
            u64::try_from(parameters.len() + erased_scalar_formals.len())
                .expect("parameter count fits a semantic identity"),
        )
        .expect("parameter count fits the machine identity namespace")
        .checked_add(1)
        .expect("generated identities follow parameter identities");
    let mut state_parameters = Vec::with_capacity(states.len());
    let mut state_erased_formals = Vec::with_capacity(states.len());
    for (position, state) in states.iter().enumerate() {
        if position == 0 && loop_plan.is_none() {
            state_parameters.push(parameters.clone());
            state_erased_formals.push(erased_scalar_formals.clone());
            continue;
        }
        state_parameters.push(
            state
                .parameter_types
                .iter()
                .map(|scalar_type| {
                    let parameter = ValueDeclaration {
                        id: value_id(next_value_identity),
                        scalar_type: scalar_type.scalar_type,
                        qualifications: scalar_type.qualifications,
                    };
                    next_value_identity = next_value_identity
                        .checked_add(1)
                        .expect("scalar graph block parameter identities advance");
                    parameter
                })
                .collect(),
        );
        state_erased_formals.push(
            state
                .erased_formal_types
                .iter()
                .map(|scalar_type| {
                    let parameter = ValueDeclaration {
                        id: value_id(next_value_identity),
                        scalar_type: scalar_type.scalar_type,
                        qualifications: scalar_type.qualifications,
                    };
                    next_value_identity = next_value_identity
                        .checked_add(1)
                        .expect("scalar graph erased formal identities advance");
                    parameter
                })
                .collect(),
        );
    }

    let all_operations = OperationBuffer::new(identity_base);
    let call_obligation_base = identity_base
        .checked_add(TERMINAL_MACHINE_IDENTITY_STRIDE / 2)
        .expect("call obligation range fits the machine identity namespace");
    let call_emission = CallEmissionContext {
        machine_ids,
        requirement_counts,
        next_obligation_identity: call_obligation_base,
        obligation_limit: identity_base
            .checked_add(TERMINAL_MACHINE_IDENTITY_STRIDE)
            .expect("machine identity namespace has a finite upper bound"),
    };
    let next_edge_identity = identity_base
        .checked_add(1)
        .expect("edge identity base admits one-based identities");
    let next_block_identity = identity_base
        .checked_add(u64::try_from(states.len()).expect("state count fits a semantic identity"))
        .expect("state count fits the machine identity namespace")
        .checked_add(1)
        .expect("conditional binding blocks follow source blocks");
    let pending_blocks = Vec::new();
    let inlined_blocks = Vec::new();
    let blocks = Vec::with_capacity(states.len());
    let mut emission = GraphEmission {
        states,
        state_parameters,
        state_erased_formals,
        loop_plan,
        terminal_machine,
        identity_base,
        parameters: &parameters,
        scalar_qualifications,
        all_operations,
        call_emission,
        next_edge_identity,
        next_block_identity,
        next_value_identity,
        pending_blocks,
        inlined_blocks,
        blocks,
    };
    for index in emission_order(states) {
        emission.emit_state(index)?;
    }
    emission.resolve_pending_blocks()?;
    let GraphEmission {
        mut scalar_qualifications,
        all_operations,
        next_edge_identity,
        next_block_identity,
        next_value_identity,
        mut blocks,
        ..
    } = emission;
    blocks.sort_by_key(|block| block.id);
    // Structural bindings belong to the original graph block even when scalar
    // call expansion inserts additional continuations beneath that block.
    for (position, state) in states.iter().enumerate() {
        if !state.structural_parameters.is_empty() {
            let block = blocks
                .iter_mut()
                .find(|block| block.id == scalar_source_block(identity_base, position))
                .ok_or(LoweringError::Unsupported(
                    "structural binding lost its graph block",
                ))?;
            if !block.structural_parameters.is_empty() {
                return unsupported("structural binding collided with an existing block namespace");
            }
            block.structural_parameters = state.structural_parameters.clone();
        }
    }
    // parameter_storage -> owned::validate must establish source no-code
    // eligibility before assembly; this pass only completes runtime custody.
    let graph_entry = scalar_source_block(identity_base, 0);
    if let Some(plan) = loop_plan {
        blocks
            .iter_mut()
            .find(|block| block.id == graph_entry)
            .ok_or(LoweringError::Unsupported("scalar loop header is absent"))?
            .structural_parameters = plan.parameters.clone();
    }
    owned_parameters::complete(
        loop_plan.map_or(structural_parameters, |plan| plan.parameters.as_slice()),
        scalar_source_block(identity_base, 0),
        &mut blocks,
    )?;
    let result = ValueDeclaration {
        id: value_id(next_value_identity),
        scalar_type: result_type.scalar_type,
        qualifications: result_type.qualifications,
    };
    let mut resolved_partition_compositions = partition_compositions
        .compositions
        .into_iter()
        .map(|composition| {
            let Some(occurrence) = all_operations.source_calls.iter().find(|occurrence| {
                occurrence.source_state == composition.producer_coordinate.state
                    && occurrence.statement_index == composition.producer_coordinate.statement_index
                    && occurrence.call_ordinal == composition.producer_coordinate.call_ordinal
            }) else {
                return Err(LoweringError::ContentPartitionProducerOperationMissing);
            };
            if occurrence.source_target != composition.source_callable {
                return Err(LoweringError::ContentPartitionProducerTargetMismatch);
            }
            Ok(ContentPartitionComposition {
                producer_operation: occurrence.terminal_operation,
                source_report_fingerprint: composition.source_report_fingerprint,
                source_structural_places: composition.source_structural_places,
                source: composition.source,
                input_claims: composition.input_claims,
                substitutions: composition.substitutions,
                derived: composition.derived,
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    resolved_partition_compositions.sort();
    let (requires, ensures, evidence) = match (result_type.scalar_type, contract) {
        (
            ScalarType::Boolean,
            PreparedScalarContract::ClosedLiteral(KnownDirectScalar::Boolean(value)),
        ) => {
            let literal = ScalarTerm::boolean(value);
            let goal = Proposition::Equal(literal.clone(), literal);
            let obligation = obligation_id(
                identity_base
                    .checked_add(1)
                    .expect("contract obligation identity is one-based"),
            );
            (
                vec![goal.clone()],
                vec![ContractClause {
                    obligation,
                    proposition: goal,
                }],
                vec![ObligationEvidence {
                    obligation,
                    route: EvidenceRoute::KernelDerived(PrimitiveJudgment::ReflexiveEquality),
                }],
            )
        }
        (
            ScalarType::Integer(integer_type),
            PreparedScalarContract::ClosedLiteral(KnownDirectScalar::Integer(value)),
        ) => {
            let literal = ScalarTerm::integer(integer_type, value)
                .expect("validated source contract fits the result type");
            let goal = Proposition::Equal(literal.clone(), literal);
            let obligation = obligation_id(
                identity_base
                    .checked_add(1)
                    .expect("contract obligation identity is one-based"),
            );
            (
                vec![goal.clone()],
                vec![ContractClause {
                    obligation,
                    proposition: goal,
                }],
                vec![ObligationEvidence {
                    obligation,
                    route: EvidenceRoute::KernelDerived(PrimitiveJudgment::ClosedIntegerRelation),
                }],
            )
        }
        (_, PreparedScalarContract::Empty) => (Vec::new(), Vec::new(), Vec::new()),
        (_, PreparedScalarContract::Predicates(plan)) => {
            let requires = scalar_contracts::clauses(
                &scalar_contracts::covered_requires(&plan)?,
                &parameters,
                &erased_scalar_formals,
            )?
            .into_iter()
            .collect();
            // Retained authored floating ranges are not propositions: publish
            // the exact IEEE endpoints against the dense entry parameter
            // identities so independently replayed call deliveries are
            // checked at every call edge. The exclusive maximum stays
            // authored; no predecessor arithmetic rewrites it.
            for range in plan.float_entry_ranges().unwrap_or_default() {
                let parameter =
                    parameters
                        .get(range.position)
                        .ok_or(LoweringError::Unsupported(
                            "scalar float entry range lost its dense entry parameter",
                        ))?;
                let declared = terminal_scalar_type(range.primitive_type)?;
                let row = ScalarFloatRange {
                    machine: terminal_machine,
                    parameter: parameter.id,
                    minimum: range.minimum,
                    maximum: range.maximum,
                    maximum_inclusive: range.maximum_inclusive,
                };
                if parameter.scalar_type != declared
                    || declared != ScalarType::IeeeFloat(row.format())
                    || !row.ordered()
                {
                    return unsupported(
                        "scalar float entry range disagrees with its declared carrier",
                    );
                }
                scalar_qualifications.float_entry_ranges.push(row);
            }
            let mut namespace = parameters.clone();
            namespace.push(result);
            let ensures =
                scalar_contracts::clauses(plan.ensures(), &namespace, &erased_scalar_formals)?
                    .into_iter()
                    .map(|proposition| ContractClause {
                        obligation: obligation_id(
                            identity_base
                                .checked_add(1)
                                .expect("contract obligation is one-based"),
                        ),
                        proposition,
                    })
                    .collect();
            // These are real parameter/result relations. Finalization proves
            // their requirements at calls and guarantees from emitted exits.
            (requires, ensures, Vec::new())
        }
        _ => unreachable!("validated scalar contract matches the machine result type"),
    };
    let mut structural_places = identity_reshuffles
        .structural_places
        .into_iter()
        .map(|place| (place.id, place.kind))
        .collect::<BTreeMap<_, _>>();
    for place in partition_compositions.structural_places {
        merge_content_place_declaration(&mut structural_places, place)
            .expect("checked lowering rejects conflicting structural places");
    }
    for place in crate::scalar_graph::scalar_computations::arrays::declarations(&all_operations)
        .chain(crate::scalar_graph::scalar_computations::cases::declarations(&all_operations))
        .chain(
            crate::scalar_graph::scalar_graph_lowering::structural_values::declarations(
                &all_operations,
            ),
        )
    {
        merge_content_place_declaration(&mut structural_places, place)?;
    }
    for parameter in structural_parameters {
        merge_content_place_declaration(
            &mut structural_places,
            StructuralPlaceDeclaration {
                id: parameter.place,
                kind: StructuralPlaceKind::Parameter {
                    position: parameter.position,
                    is_self: parameter.is_self,
                },
            },
        )?;
    }
    for block in &blocks {
        for parameter in &block.structural_parameters {
            merge_content_place_declaration(
                &mut structural_places,
                StructuralPlaceDeclaration {
                    id: parameter.place,
                    kind: StructuralPlaceKind::BlockParameter {
                        block: block.id,
                        position: parameter.position,
                    },
                },
            )?;
        }
    }
    let entry = if let Some(plan) = loop_plan {
        for parameter in &plan.parameters {
            merge_content_place_declaration(
                &mut structural_places,
                StructuralPlaceDeclaration {
                    id: parameter.place,
                    kind: StructuralPlaceKind::BlockParameter {
                        block: graph_entry,
                        position: parameter.position,
                    },
                },
            )?;
        }
        let entry = block_id(next_block_identity);
        blocks.push(Block {
            id: entry,
            parameters: Vec::new(),
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::Jump {
                edge: edge_id(next_edge_identity),
                target: graph_entry,
                arguments: parameters.iter().map(|parameter| parameter.id).collect(),
                erased_arguments: Vec::new(),
                structural_arguments: structural_parameters
                    .iter()
                    .map(|parameter| StructuralArgument {
                        place: parameter.place,
                        path: Vec::new(),
                        access: parameter.access,
                    })
                    .collect(),
                trivial_affine_discards: Vec::new(),
                residual_affine_discards: Vec::new(),
            },
        });
        entry
    } else {
        graph_entry
    };
    for operation in blocks.iter().flat_map(|block| &block.operations) {
        if matches!(
            operation.kind,
            OperationKind::EstablishPrimitiveLocal { .. }
        ) {
            let OperationResult::Structural(result) = &operation.result else {
                return unsupported("primitive local establishment lost its structural result");
            };
            merge_content_place_declaration(
                &mut structural_places,
                StructuralPlaceDeclaration {
                    id: result.place,
                    kind: StructuralPlaceKind::OperationResult {
                        producer: operation.id,
                        structural_type: result.structural_type,
                    },
                },
            )?;
        }
    }
    scalar_qualifications
        .coercions
        .sort_by_key(|coercion| (coercion.machine, coercion.edge, coercion.argument_ordinal));
    scalar_qualifications
        .float_entry_ranges
        .sort_by_key(|range| (range.machine, range.parameter));
    let mut lowered = LoweredPsi {
        semantic_module: TerminalModule {
            scalar_qualifications,
            scalar_block_invariants: Vec::new(),
            operation_crash_contracts: Vec::new(),
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry: terminal_machine,
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
            machines: vec![TerminalMachine {
                closed_reach_application: None,
                declared_service_reach: Vec::new(),
                id: terminal_machine,
                attachment: None,
                structural_parameters: structural_parameters.to_vec(),
                ranked_scc: None,
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                parameters,
                result: TerminalMachineResult::Scalar(result),
                structural_places: structural_places
                    .into_iter()
                    .map(|(id, kind)| StructuralPlaceDeclaration { id, kind })
                    .collect(),
                content_entry_claims: identity_reshuffles.entry_claims,
                content_identity_reshuffles: identity_reshuffles.reshuffles,
                content_partition_compositions: resolved_partition_compositions,
                entry,
                blocks,
                contract: MachineContract {
                    id: contract_id(terminal_machine.get()),
                    crash_routes,
                    erased_scalar_formals,
                    requires,
                    ensures,
                    outcome_specific_ensures: Vec::new(),
                },
            }],
        },
        proof_bundle: ProofBundle {
            recursive_components: Vec::new(),
            control_cycles: Vec::new(),
            evidence_producers: Vec::new(),
            evidence,
        },
        debug_map: None,
        source_call_occurrences: all_operations.source_calls,
        selected_ieee_float_fma_occurrences: all_operations.selected_ieee_float_fmas,
        selected_ieee_float_comparison_occurrences: all_operations.selected_ieee_float_comparisons,
        selected_integer_comparison_occurrences: all_operations.selected_integer_comparisons,
    };
    if let Some(plan) = loop_plan {
        ranking::retain(&mut lowered.semantic_module.machines[0], graph_entry, plan)?;
    }
    Ok(lowered)
}
