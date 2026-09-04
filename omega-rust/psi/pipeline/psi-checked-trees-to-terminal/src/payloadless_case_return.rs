//! Exact zero-input payloadless structural-case construction and return.

use super::*;

pub(super) fn lower_payloadless_case_return_machine(
    checked: &CheckedTrees,
    plan: &psi_checked_trees::CheckedPayloadlessCaseReturnMachinePlan,
) -> Result<LoweredTerminalPsi, LoweringError> {
    if !plan.machine.is_valid() || !plan.state.is_valid() {
        return unsupported("payloadless case return plan has an invalid source identity");
    }
    if plan.attachment_type_identity.is_empty()
        || plan.result.type_identity.is_empty()
        || plan.returned_case_identity.is_empty()
        || plan.result.multiplicity != psi_language_semantics::Multiplicity::Unrestricted
        || !plan.result.qualifications.is_empty()
    {
        return unsupported("payloadless case return plan is outside the exact checked shape");
    }

    let plans = &checked.facts.flow.terminal_structural_returns;
    let (structural_types, type_ids) = lower_structural_type_plans(&plans.structural_types)?;
    let result_type = lookup_type_id(&type_ids, &plan.result.type_identity)?;
    let attachment = lookup_type_id(&type_ids, &plan.attachment_type_identity)?;
    let result_declaration = structural_types
        .iter()
        .find(|declaration| declaration.id == result_type)
        .ok_or(LoweringError::Unsupported(
            "payloadless case return result has no structural type",
        ))?;
    let StructuralTypeShape::Sum { cases } = &result_declaration.shape else {
        return unsupported("payloadless case return result is not a structural sum");
    };
    if cases.len() < 2 || cases.iter().any(|case| !case.fields.is_empty()) {
        return unsupported("payloadless case return result is not a closed payloadless sum");
    }
    let result_case = cases
        .iter()
        .find_map(|case| (case.identity == plan.returned_case_identity).then_some(case.id))
        .ok_or(LoweringError::Unsupported(
            "payloadless case return references an unknown nominal case",
        ))?;

    let terminal_machine = machine_id(1);
    let operation = operation_id(1);
    let operation_result_place = place_id(1);
    let machine_result_place = place_id(RESULT_STRUCTURAL_PLACE_ID);
    let machine = TerminalMachine {
        id: terminal_machine,
        attachment: Some(attachment),
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Structural(StructuralResultDeclaration {
            place: machine_result_place,
            structural_type: result_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        }),
        structural_places: vec![
            StructuralPlaceDeclaration {
                id: operation_result_place,
                kind: StructuralPlaceKind::OperationResult {
                    producer: operation,
                    structural_type: result_type,
                },
            },
            StructuralPlaceDeclaration {
                id: machine_result_place,
                kind: StructuralPlaceKind::Result,
            },
        ],
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block_id(1),
        blocks: vec![Block {
            id: block_id(1),
            parameters: Vec::new(),
            operations: vec![Operation {
                id: operation,
                result: psi_terminal::OperationResult::Structural(
                    psi_terminal::StructuralOperationResult {
                        place: operation_result_place,
                        structural_type: result_type,
                        multiplicity: StructuralMultiplicity::Unrestricted,
                        qualifications: Vec::new(),
                        projected_qualifications: Vec::new(),
                        claims: Vec::new(),
                    },
                ),
                kind: OperationKind::EstablishPayloadlessCase { result_case },
            }],
            terminator: Terminator::ReturnStructural {
                edge: edge_id(1),
                source: operation_result_place,
                returned_claims: Vec::new(),
                trivial_affine_discards: Vec::new(),
            },
        }],
        contract: MachineContract {
            id: contract_id(1),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    };
    Ok(LoweredTerminalPsi {
        semantic_module: TerminalModule {
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry: terminal_machine,
            structural_types,
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
            machines: vec![machine],
        },
        proof_bundle: ProofBundle::default(),
        debug_map: None,
        source_call_occurrences: Vec::new(),
        selected_ieee_float_fma_occurrences: Vec::new(),
    })
}
