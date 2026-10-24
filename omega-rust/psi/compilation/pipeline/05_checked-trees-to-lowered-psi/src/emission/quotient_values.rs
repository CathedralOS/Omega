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
    if !signature.erased_scalar_parameters.is_empty()
        || !signature.erased_proof_parameters.is_empty()
        || !crash_routes.is_empty()
        || signature.parameters.len() + signature.scalar_parameters.len() != arguments.len()
        || checked.typed.state_parameters(state).len() != actuals.len()
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
    let mut scalar_arguments = Vec::new();
    let mut structural_formals = signature.parameters.iter();
    let mut scalar_formals = signature.scalar_parameters.iter();
    for ((actual, ty), source_formal) in actuals.iter().zip(checked.typed.state_parameters(state)) {
        if !actual.path.is_empty() {
            return unsupported("quotient invocation input is not a whole checked value");
        }
        if let Some(primitive) = checked
            .typed
            .primitive_type_reference(source_formal.type_reference)
        {
            let scalar_type = crate::emission::scalar_types::terminal_scalar_type(primitive)?;
            let formal = scalar_formals.next().ok_or(LoweringError::Unsupported(
                "quotient scalar formal is absent",
            ))?;
            if formal.scalar_type != scalar_type
                || !formal.qualifications.is_empty()
                || !calls.structural_types.iter().any(|declaration| {
                    declaration.id == *ty
                        && declaration.shape
                            == terminal_psi::StructuralTypeShape::PrimitiveScalar(scalar_type)
                })
            {
                return unsupported("quotient scalar input has a different checked carrier");
            }
            scalar_arguments.push(
                crate::emission::operation_emission::expressions::emit_scalar_leaf(
                    terminal_psi::OperationKind::PrimitiveScalarRead {
                        source: actual.place,
                        path: Vec::new(),
                    },
                    scalar_type,
                    next_value,
                    operations,
                ),
            );
        } else {
            let formal = structural_formals.next().ok_or(LoweringError::Unsupported(
                "quotient structural formal is absent",
            ))?;
            if *ty != formal.structural_type
                || formal.multiplicity != StructuralMultiplicity::Unrestricted
                || !formal.qualifications.is_empty()
                || !formal.projected_qualifications.is_empty()
            {
                return unsupported(
                    "quotient invocation input is not an exact custody-free carrier",
                );
            }
            arguments.push(StructuralArgument {
                access: formal.access,
                ..actual.clone()
            });
        }
    }
    if structural_formals.next().is_some() || scalar_formals.next().is_some() {
        return unsupported("quotient invocation argument lanes are incomplete");
    }
    // The source correspondence established every authored precondition at this
    // exact call. Use the complete ordinary requirement roster: only authored
    // source-checked rows receive that discharge; generated carrier predicates
    // remain ordinary verifier obligations. The runtime-requirement projection
    // needs special substitution only for crash continuations, excluded above.
    // Quotient theorem schemas are not runtime witnesses or a second verifier.
    let requirement_obligations =
        crate::unit::attached_unit::ordinary_calls::prepare_requirement_obligations(
            &signature.call_target(),
            calls,
            operations,
        )?;
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
            scalar_arguments,
            Vec::new(),
            Vec::new(),
            arguments,
            Vec::new(),
            requirement_obligations,
            Vec::new(),
            mode,
        ),
    )?;
    if let Some(scalar) = scalar {
        let scalar = crate::unit::attached_unit::primitive_locals::completed_scalar(
            scalar,
            &[],
            operations,
        )?;
        crate::unit::attached_unit::primitive_locals::establish_at(
            place,
            operations,
            scalar,
            structural_type,
            crate::unit::attached_unit::primitive_locals::PrimitiveStorageCatalog {
                types: calls.structural_types,
                domains: calls.domain_declarations,
                scalars: calls.scalar_qualifications,
            },
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
