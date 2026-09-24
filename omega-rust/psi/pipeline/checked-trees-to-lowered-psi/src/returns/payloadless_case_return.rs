//! Exact zero-input payloadless structural-case construction and return.

use super::{
    Block, CheckedTrees, LoweredPsi, LoweringError, MachineContract, Operation, OperationKind,
    ProofBundle, RESULT_STRUCTURAL_PLACE_ID, StructuralMultiplicity, StructuralPlaceDeclaration,
    StructuralPlaceKind, StructuralResultDeclaration, StructuralTypeShape, TerminalMachine,
    TerminalMachineResult, TerminalModule, Terminator, block_id, contract_id, edge_id,
    lookup_type_id, lower_structural_type_plans, machine_id, operation_id, place_id, unsupported,
};
pub(crate) fn lower_payloadless_case_return_machine(
    checked: &CheckedTrees,
    plan: &checked_trees::CheckedPayloadlessCaseReturnMachinePlan,
) -> Result<LoweredPsi, LoweringError> {
    if !plan.machine.is_valid() || !plan.state.is_valid() {
        return unsupported("payloadless case return plan has an invalid source identity");
    }
    if plan.attachment_type_identity.is_empty()
        || plan.result.type_identity.is_empty()
        || plan.returned_case_identity.is_empty()
        || plan.result.multiplicity != language_semantics::Multiplicity::Unrestricted
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
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: terminal_machine,
        attachment: Some(attachment),
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Structural(StructuralResultDeclaration {
            reference_sources: Vec::new(),
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
            structural_parameters: Vec::new(),
            id: block_id(1),
            parameters: Vec::new(),
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            operations: vec![Operation {
                static_reach_binding: None,
                suspension_crossing: None,
                id: operation,
                result: terminal_psi::OperationResult::Structural(
                    terminal_psi::StructuralOperationResult {
                        qualification_establishments: Vec::new(),
                        place: operation_result_place,
                        structural_type: result_type,
                        multiplicity: StructuralMultiplicity::Unrestricted,
                        qualifications: Vec::new(),
                        projected_qualifications: Vec::new(),
                        claims: Vec::new(),
                    },
                ),
                kind: OperationKind::EstablishScalarCase {
                    result_case,
                    fields: Vec::new(),
                },
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
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    };
    Ok(LoweredPsi {
        semantic_module: TerminalModule {
            structural_types,
            machines: vec![machine],
            ..TerminalModule::for_entry(terminal_machine)
        },
        proof_bundle: ProofBundle::default(),
        debug_map: None,
        source_call_occurrences: Vec::new(),
        selected_ieee_float_fma_occurrences: Vec::new(),
        selected_ieee_float_comparison_occurrences: Vec::new(),
        selected_integer_comparison_occurrences: Vec::new(),
    })
}
