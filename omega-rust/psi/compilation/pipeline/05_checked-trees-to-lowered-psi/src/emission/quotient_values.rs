//! Quotient adaptations lower to ordinary copies and selected direct calls.
//! Both construction schedules supply completed operands to these emitters.
use super::{StructuralPlaceDeclaration, StructuralPlaceKind};
use crate::direct_calls::{CallRecord, DirectCall, DirectCallResult, emit_direct_call};
use crate::emission::operation_emission::buffer::{
    OperationBuffer, SourceCallCoordinate, scalar_result_declaration, structural_result_declaration,
};
use crate::emission::operation_emission::calls::CallEmissionContext;
use crate::lowering_error::{LoweringError, unsupported};
use checked_trees::{CheckedStructuralValueHandle, CheckedStructuralValueKind, CheckedTrees};
use semantic_vocabulary::{PlaceId, StructuralTypeId};
use terminal_psi::{StructuralArgument, StructuralMultiplicity};

pub(crate) fn emit_call(
    checked: &CheckedTrees,
    value: CheckedStructuralValueHandle,
    actuals: &[(StructuralArgument, StructuralTypeId)],
    place: PlaceId,
    structural_type: StructuralTypeId,
    next_value: &mut u64,
    operations: &mut OperationBuffer,
    calls: &mut CallEmissionContext<'_>,
) -> Result<StructuralPlaceDeclaration, LoweringError> {
    let node = checked.facts.values.structural_values.nodes.get(value);
    let CheckedStructuralValueKind::QuotientCall {
        request,
        representative_state,
        arguments,
    } = &node.kind
    else {
        return unsupported("quotient invocation has no checked call node");
    };
    let request = *request;
    let representative_state = *representative_state;
    let mut approvals = checked.facts.quotient_operations.iter().filter(|row| {
        row.request == request
            && row.public_machine == node.machine
            && row.public_state == node.state
            && row.representative_state == representative_state
    });
    if approvals.next().is_none() || approvals.next().is_some() {
        return unsupported("quotient invocation lost its unique checked correspondence");
    }
    let (representative, state) =
        validation::find_machine_by_entry_state(&checked.typed, representative_state).ok_or(
            LoweringError::Unsupported("quotient representative body is absent"),
        )?;
    let signature =
        crate::unit::attached_unit::signatures::find(calls.signatures, representative.symbol)?
            .clone();
    // Private inferred causes are as real as published causes. This
    // bounded lane emits no continuations only when the effective checked
    // ceiling is empty, including the representative's helper closure.
    let crash_routes = crate::unit::effective_crash_routes(checked, representative.symbol)?;
    if !signature.scalar_parameters.is_empty()
        || !signature.erased_scalar_parameters.is_empty()
        || !signature.erased_proof_parameters.is_empty()
        || !signature.requires.is_empty()
        || !crash_routes.is_empty()
        || signature.parameters.len() != arguments.len()
    {
        return unsupported(
            "quotient invocation requires an unimplemented operand or contract lane",
        );
    }
    if checked.typed.type_multiplicity(state.return_type)
        != language_semantics::Multiplicity::Unrestricted
        || !validation::view_result_carrier_qualifications(&checked.typed, state.return_type)
            .map_err(LoweringError::Unsupported)?
            .is_empty()
        || !crate::unit::attached_unit::parameters::authored_result_qualification_domains(
            checked,
            representative.symbol,
        )
        .is_empty()
        || !validation::structural_result_projected_qualifications(
            &checked.typed,
            state.return_type,
        )
        .map_err(LoweringError::Unsupported)?
        .is_empty()
    {
        return unsupported("quotient invocation result carries custody or qualifications");
    }
    let mut arguments = Vec::new();
    for ((actual, ty), formal) in actuals.iter().zip(&signature.parameters) {
        if *ty != formal.structural_type
            || !actual.path.is_empty()
            || formal.multiplicity != StructuralMultiplicity::Unrestricted
            || !formal.qualifications.is_empty()
            || !formal.projected_qualifications.is_empty()
        {
            return unsupported("quotient invocation input is not an exact custody-free carrier");
        }
        arguments.push(StructuralArgument {
            access: formal.access,
            ..actual.clone()
        });
    }
    let callee =
        crate::terminal_identities::lookup_machine_id(calls.machine_ids, representative.symbol)?;
    let (result, mode, scalar) = if let Some(scalar) = signature.scalar_result {
        if !scalar.qualifications.is_empty() {
            return unsupported("quotient scalar result has unimplemented qualifications");
        }
        let id = crate::terminal_identities::value_id(crate::terminal_identities::allocate_dense(
            next_value,
        )?);
        (
            scalar_result_declaration(id, scalar.scalar_type),
            DirectCallResult::Scalar,
            Some(id),
        )
    } else {
        (
            structural_result_declaration(
                place,
                structural_type,
                StructuralMultiplicity::Unrestricted,
            ),
            DirectCallResult::Structural {
                multiplicity: StructuralMultiplicity::Unrestricted,
            },
            None,
        )
    };
    let call = emit_direct_call(
        operations,
        SourceCallCoordinate {
            state: node.state,
            statement_index: node.statement_index as usize,
            call_ordinal: 0,
        },
        None,
        representative_state,
        CallRecord::Quotient(value),
        result,
        DirectCall::new(
            callee,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            arguments,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            mode,
        ),
    )?;
    if let Some(scalar) = scalar {
        crate::unit::attached_unit::primitive_locals::establish_at(
            place,
            operations,
            scalar,
            structural_type,
            calls.structural_types,
            None,
        )
    } else {
        Ok(StructuralPlaceDeclaration {
            id: place,
            kind: StructuralPlaceKind::OperationResult {
                producer: call,
                structural_type,
            },
        })
    }
}

pub(crate) fn emit_adaptation(
    value: CheckedStructuralValueHandle,
    source: PlaceId,
    source_type: StructuralTypeId,
    place: PlaceId,
    structural_type: StructuralTypeId,
    operations: &mut OperationBuffer,
) -> StructuralPlaceDeclaration {
    let copy = operations.allocate();
    operations.push(terminal_psi::Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: copy,
        result: structural_result_declaration(
            place,
            structural_type,
            StructuralMultiplicity::Unrestricted,
        ),
        kind: terminal_psi::OperationKind::StructuralLeafCopy {
            source,
            path: Vec::new(),
        },
    });
    operations.quotient_adaptations.push(
        crate::emission::operation_emission::buffer::SourceQuotientAdaptation {
            value,
            copy,
            source,
            result: place,
            source_type,
            result_type: structural_type,
        },
    );
    StructuralPlaceDeclaration {
        id: place,
        kind: StructuralPlaceKind::OperationResult {
            producer: copy,
            structural_type,
        },
    }
}
