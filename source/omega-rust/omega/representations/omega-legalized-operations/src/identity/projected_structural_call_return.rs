//! Canonical identity for the atomic projected-roster legalization family.

use super::calling::{encode_call_plan, encode_placement, encode_shape};
use super::shared::*;
use super::structural::{encode_effect, encode_ownership_roster};
use super::structural_types::{
    encode_multiplicity, encode_structural_parameter, encode_structural_path,
    encode_structural_type, encode_target_structural_argument, encode_target_structural_parameter,
};
use psi_terminal::{
    StructuralOperationResult, StructuralPathQualification, StructuralResultDeclaration,
};

pub(super) fn encode_projected_structural_call_return(
    bytes: &mut Vec<u8>,
    closure: &LegalizedProjectedStructuralCallReturn,
) {
    bytes.push(match closure.recipe {
        ProjectedStructuralCallReturnLegalizationRecipe::OwnedLinearDirectV1 => 1,
    });
    encode_caller(bytes, &closure.caller);
    encode_callee(bytes, &closure.callee);
    bytes.extend_from_slice(&closure.caller_entry_block.get().to_le_bytes());
    bytes.extend_from_slice(&closure.callee_entry_block.get().to_le_bytes());
    encode_nodes(bytes, &closure.caller_nodes);
    encode_nodes(bytes, &closure.callee_nodes);
}

fn encode_caller(bytes: &mut Vec<u8>, function: &omega_target_operations::TargetFunction) {
    encode_function_header(bytes, function);
    let omega_target_operations::TargetOperation::ReturnStructuralCall {
        psi_edge,
        psi_operation,
        operation_result,
        result,
        callee,
        structural_types,
        call_plan,
        callee_call_plan,
        structural_parameters,
        arguments,
        claim_transfers,
        returned_claim_transfers,
        returned_claims,
        requirement_obligations,
        crash_continuations,
    } = &function.operation
    else {
        unreachable!("validated projected caller shape")
    };
    bytes.extend_from_slice(&psi_edge.get().to_le_bytes());
    bytes.extend_from_slice(&psi_operation.get().to_le_bytes());
    encode_operation_result(bytes, operation_result);
    encode_result(bytes, result);
    bytes.extend_from_slice(&callee.get().to_le_bytes());
    encode_structural_types(bytes, structural_types);
    encode_call_plan(bytes, call_plan);
    encode_call_plan(bytes, callee_call_plan);
    encode_target_parameters(bytes, structural_parameters);
    encode_len(bytes, arguments.len());
    for argument in arguments {
        encode_target_structural_argument(bytes, argument);
    }
    encode_len(bytes, claim_transfers.len());
    for transfer in claim_transfers {
        bytes.extend_from_slice(&transfer.claim.get().to_le_bytes());
        bytes.extend_from_slice(&transfer.argument_index.to_le_bytes());
    }
    encode_len(bytes, returned_claim_transfers.len());
    for transfer in returned_claim_transfers {
        bytes.extend_from_slice(&transfer.callee_claim.get().to_le_bytes());
        bytes.extend_from_slice(&transfer.caller_claim.get().to_le_bytes());
    }
    encode_ids(bytes, returned_claims.iter().map(|claim| claim.get()));
    encode_ids(
        bytes,
        requirement_obligations
            .iter()
            .map(|obligation| obligation.get()),
    );
    let crash = psi_terminal_codec::encode_crash_route_buckets(crash_continuations)
        .expect("validated crash routes remain canonical");
    encode_len(bytes, crash.len());
    bytes.extend_from_slice(&crash);
}

fn encode_callee(bytes: &mut Vec<u8>, function: &omega_target_operations::TargetFunction) {
    encode_function_header(bytes, function);
    let omega_target_operations::TargetOperation::ReturnStructuralParameter {
        call_plan,
        scalar_parameters: _,
        parameters,
        source,
        result,
        shape,
        source_placement,
        result_placement,
        psi_edge,
        returned_claims,
        trivial_affine_locals,
        trivial_affine_discards,
    } = &function.operation
    else {
        unreachable!("validated projected callee shape")
    };
    encode_call_plan(bytes, call_plan);
    encode_len(bytes, parameters.len());
    for parameter in parameters {
        encode_semantic_parameter(bytes, parameter);
    }
    encode_semantic_parameter(bytes, source);
    encode_result(bytes, result);
    encode_shape(bytes, *shape);
    encode_placement(bytes, source_placement);
    encode_placement(bytes, result_placement);
    bytes.extend_from_slice(&psi_edge.get().to_le_bytes());
    encode_ids(bytes, returned_claims.iter().map(|claim| claim.get()));
    encode_len(bytes, trivial_affine_locals.len());
    for (operation, place, declaration) in trivial_affine_locals {
        bytes.extend_from_slice(&operation.get().to_le_bytes());
        super::structural_types::encode_structural_place(bytes, *place);
        encode_structural_type(bytes, declaration);
    }
    encode_ids(
        bytes,
        trivial_affine_discards.iter().map(|place| place.get()),
    );
}

fn encode_function_header(bytes: &mut Vec<u8>, function: &omega_target_operations::TargetFunction) {
    bytes.extend_from_slice(&function.machine.get().to_le_bytes());
    encode_option_id(bytes, function.attachment.map(|value| value.get()));
    bytes.push(u8::from(function.fixed_integer_scalar_abi.is_some()));
    encode_ids(
        bytes,
        function
            .provenance
            .operations
            .iter()
            .map(|value| value.get()),
    );
    encode_ids(
        bytes,
        function.provenance.edges.iter().map(|value| value.get()),
    );
}

fn encode_structural_types(bytes: &mut Vec<u8>, declarations: &[StructuralTypeDeclaration]) {
    encode_len(bytes, declarations.len());
    for declaration in declarations {
        encode_structural_type(bytes, declaration);
    }
}

fn encode_target_parameters(
    bytes: &mut Vec<u8>,
    parameters: &[omega_target_operations::TargetStructuralParameter],
) {
    encode_len(bytes, parameters.len());
    for parameter in parameters {
        encode_target_structural_parameter(bytes, parameter);
        encode_projected_qualifications(bytes, &parameter.projected_qualifications);
    }
}

fn encode_semantic_parameter(bytes: &mut Vec<u8>, parameter: &StructuralParameterDeclaration) {
    encode_structural_parameter(bytes, parameter);
    encode_projected_qualifications(bytes, &parameter.projected_qualifications);
}

fn encode_operation_result(bytes: &mut Vec<u8>, result: &StructuralOperationResult) {
    bytes.extend_from_slice(&result.place.get().to_le_bytes());
    bytes.extend_from_slice(&result.structural_type.get().to_le_bytes());
    encode_multiplicity(bytes, result.multiplicity);
    encode_ids(
        bytes,
        result.qualifications.iter().map(|domain| domain.get()),
    );
    encode_projected_qualifications(bytes, &result.projected_qualifications);
    encode_len(bytes, result.claims.len());
    for claim in &result.claims {
        bytes.extend_from_slice(&claim.claim.get().to_le_bytes());
        encode_structural_path(bytes, &claim.path);
    }
}

fn encode_result(bytes: &mut Vec<u8>, result: &StructuralResultDeclaration) {
    bytes.extend_from_slice(&result.place.get().to_le_bytes());
    bytes.extend_from_slice(&result.structural_type.get().to_le_bytes());
    encode_multiplicity(bytes, result.multiplicity);
    encode_ids(
        bytes,
        result.qualifications.iter().map(|domain| domain.get()),
    );
    encode_projected_qualifications(bytes, &result.projected_qualifications);
}

fn encode_projected_qualifications(bytes: &mut Vec<u8>, rows: &[StructuralPathQualification]) {
    encode_len(bytes, rows.len());
    for row in rows {
        encode_structural_path(bytes, &row.path);
        bytes.extend_from_slice(&row.domain.get().to_le_bytes());
    }
}

fn encode_nodes(bytes: &mut Vec<u8>, nodes: &[LegalizedStructuralNodeCustody]) {
    encode_len(bytes, nodes.len());
    for node in nodes {
        encode_fuel(bytes, &node.fuel);
        encode_effect(bytes, node.effect);
        encode_ownership_roster(bytes, &node.ownership);
    }
}
