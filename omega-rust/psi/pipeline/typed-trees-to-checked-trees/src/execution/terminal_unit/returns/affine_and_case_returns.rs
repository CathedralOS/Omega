//! Claim-free affine and payloadless case return machines.

use crate::execution::terminal_unit::calls::{
    entry_claims, free_structural_scalar_signature, structural_scalar_signature,
    structural_signature,
};
use crate::execution::terminal_unit::cleanup::{
    machine_has_content_evidence, service_reach_is_empty, service_reach_plan_is_empty,
};
use crate::execution::terminal_unit::types::{
    ShapeCollector, has_plain_owned_contents, machine_binders, parameter_qualifications,
    projected_parameter_qualifications, state_flow, type_graph_requires_nominal_drop,
};
use crate::execution::terminal_unit::{
    CheckFacts, CheckedClaimFreeAffineStructuralReturnMachinePlan,
    CheckedPayloadlessCaseReturnMachinePlan, CheckedStructuralAccess, CheckedStructuralResultPlan,
    CheckedUnitStructuralTypeShape, DataMember, ExpressionNode, Multiplicity, PrimitiveType,
    SignatureContractKind, StatementNode, TypeReferenceNode, TypedTrees,
};

pub(crate) fn build_claim_free_affine_structural_return_machine(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
) -> Option<CheckedClaimFreeAffineStructuralReturnMachinePlan> {
    let [state] = program.machine_states(machine) else {
        return None;
    };
    let [StatementNode::Expression(return_expression)] =
        program.statement_table.statements(state.statement_nodes)
    else {
        return None;
    };
    if !program.machine_contracts(machine).is_empty()
        || !program.state_contracts(state).is_empty()
        || machine_has_content_evidence(facts, machine.symbol, state.symbol)
    {
        return None;
    }
    let binders = machine_binders(program, machine);
    if !has_plain_owned_contents(program, state.return_type) {
        return None;
    }
    let (attachment_type_identity, structural_parameters, scalar_parameters) =
        if machine.attached_data.is_some() {
            let (attachment, structural, scalar) =
                structural_scalar_signature(program, shapes, machine, state, &binders, false)?;
            (Some(attachment), structural, scalar)
        } else {
            let (structural, scalar) =
                free_structural_scalar_signature(program, shapes, state, &binders)?;
            (None, structural, scalar)
        };
    let [structural_parameter] = structural_parameters.as_slice() else {
        return None;
    };
    if structural_parameter.is_self
        || structural_parameter.multiplicity != Multiplicity::Affine
        || structural_parameter.access != CheckedStructuralAccess::Owned
        || !structural_parameter.qualifications.is_empty()
        || structural_parameter.fused_service_erasure.is_some()
        || crate::execution::terminal_unit::types::abi_parameter_count(
            program.state_parameters(state),
        ) != structural_parameters.len() + scalar_parameters.len()
        || scalar_parameters.iter().any(|parameter| {
            !matches!(
                parameter.primitive_type,
                PrimitiveType::I8
                    | PrimitiveType::I16
                    | PrimitiveType::I32
                    | PrimitiveType::I64
                    | PrimitiveType::U8
                    | PrimitiveType::U16
                    | PrimitiveType::U32
                    | PrimitiveType::U64
            )
        })
    {
        return None;
    }
    let source_parameters = program.state_parameters(state);
    let source_parameter = source_parameters.get(structural_parameter.position as usize)?;
    let ExpressionNode::Name(path) = program.expression_table.expression(*return_expression) else {
        return None;
    };
    if path.symbol != source_parameter.symbol
        || program
            .expression_table
            .name_path_members(path.members)
            .len()
            != 1
        || crate::checks::type_multiplicity(program, state.return_type) != Multiplicity::Affine
    {
        return None;
    }
    let result_type_identity = shapes.add_type(state.return_type, &binders, &[])?;
    if result_type_identity != structural_parameter.type_identity
        || type_graph_requires_nominal_drop(program, state.return_type)
        || !parameter_qualifications(program, shapes, state.return_type, &binders)?.is_empty()
    {
        return None;
    }
    let flow = state_flow(facts, machine.symbol, state.symbol)?;
    let checked_entry_claims = entry_claims(
        program,
        facts,
        machine.symbol,
        state.symbol,
        &structural_parameters,
        source_parameters,
    )?;
    if !facts
        .flow
        .control
        .calls
        .span_or_empty(flow.calls)
        .is_empty()
        || !service_reach_is_empty(facts, flow.service_reach)
        || !checked_entry_claims.is_empty()
    {
        return None;
    }
    Some(CheckedClaimFreeAffineStructuralReturnMachinePlan {
        machine: machine.symbol,
        state: state.symbol,
        attachment_type_identity,
        structural_parameter: structural_parameter.clone(),
        scalar_parameters,
        result: CheckedStructuralResultPlan {
            type_identity: result_type_identity,
            multiplicity: Multiplicity::Affine,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        },
        return_statement_ordinal: 0,
    })
}

pub(crate) fn build_payloadless_case_return_machine(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
) -> Option<CheckedPayloadlessCaseReturnMachinePlan> {
    let [state] = program.machine_states(machine) else {
        return None;
    };
    let [StatementNode::Expression(return_expression)] =
        program.statement_table.statements(state.statement_nodes)
    else {
        return None;
    };
    if !machine.lifetime_parameters.is_empty()
        || !program.machine_type_parameters(machine).is_empty()
        || !program.machine_owned_data(machine).is_empty()
        || !program.machine_trait_conformances(machine).is_empty()
        || !machine.conformance_bounds.is_empty()
        || !program.machine_invokes(machine).is_empty()
        || machine.suspends
        || machine.blocks
        || !program.machine_contracts(machine).iter().all(|contract| {
            matches!(
                contract.kind,
                SignatureContractKind::EnsuresForResultCase { .. }
            )
        })
        || !program.state_contracts(state).is_empty()
        || !program.state_parameters(state).is_empty()
    {
        return None;
    }
    let state_flow = state_flow(facts, machine.symbol, state.symbol)?;
    if !facts
        .flow
        .control
        .calls
        .span_or_empty(state_flow.calls)
        .is_empty()
        || !service_reach_is_empty(facts, state_flow.service_reach)
        || !service_reach_plan_is_empty(
            facts,
            facts.service_reaches.plan_for_machine(machine.symbol)?,
        )
    {
        return None;
    }

    let binders = machine_binders(program, machine);
    let (attachment_type_identity, structural_parameters) =
        structural_signature(program, shapes, machine, state, &binders, false)?;
    if !structural_parameters.is_empty() {
        return None;
    }
    let TypeReferenceNode::Named {
        symbol: result_data_symbol,
        ..
    } = program
        .type_reference_table
        .type_reference(state.return_type)
    else {
        return None;
    };
    if crate::checks::type_multiplicity(program, state.return_type) != Multiplicity::Unrestricted {
        return None;
    }
    let result_qualifications =
        parameter_qualifications(program, shapes, state.return_type, &binders)?;
    if !result_qualifications.is_empty() {
        return None;
    }
    let returned_case_symbol = match program.expression_table.expression(*return_expression) {
        // Retained case references still require their exact nominal path.
        // Newly resolved bare values share the case-literal arm below.
        ExpressionNode::Name(path)
            if path.head_symbol == *result_data_symbol
                && path.symbol.is_valid()
                && program
                    .expression_table
                    .name_path_members(path.members)
                    .len()
                    == 2 =>
        {
            path.symbol
        }
        // Bare payload-free values and explicit braces share this resolved
        // construction while retaining the actual nominal case symbol.
        ExpressionNode::StructLiteral(literal)
            if literal.type_symbol == *result_data_symbol
                && literal.case_name.is_some()
                && program
                    .expression_table
                    .struct_fields(literal.fields)
                    .is_empty() =>
        {
            literal.case_symbol?
        }
        _ => return None,
    };
    let result_data = program
        .data_definitions()
        .iter()
        .find(|data| data.symbol == *result_data_symbol)?;
    let result_members = program.data_members(result_data);
    if result_members.len() < 2
        || result_members.iter().any(|member| {
            !matches!(
                member,
                DataMember::Variant(variant)
                    if program.data_payload_fields(variant).is_empty()
            )
        })
    {
        return None;
    }
    let returned_case_identity = result_members.iter().find_map(|member| {
        let DataMember::Variant(variant) = member else {
            return None;
        };
        (variant.symbol == returned_case_symbol).then(|| {
            variant
                .identity
                .map(|identity| format!("#{identity}"))
                .unwrap_or_else(|| variant.name.as_str().to_owned())
        })
    })?;
    let result_type_identity = shapes.add_type(state.return_type, &binders, &[])?;
    let CheckedUnitStructuralTypeShape::Sum { cases } =
        &shapes.types.get(&result_type_identity)?.shape
    else {
        return None;
    };
    if cases.len() != result_members.len()
        || !cases.iter().all(|case| case.fields.is_empty())
        || !cases
            .iter()
            .any(|case| case.identity == returned_case_identity)
    {
        return None;
    }

    Some(CheckedPayloadlessCaseReturnMachinePlan {
        machine: machine.symbol,
        state: state.symbol,
        attachment_type_identity,
        result: CheckedStructuralResultPlan {
            type_identity: result_type_identity,
            multiplicity: Multiplicity::Unrestricted,
            qualifications: result_qualifications,
            projected_qualifications: projected_parameter_qualifications(
                program,
                shapes,
                state.return_type,
                &binders,
            )?,
        },
        returned_case_identity,
    })
}
