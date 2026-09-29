//! Emit one nested boundary application inside an operand evaluation.
//!
//! A statement-level boundary call is emitted by the attached Unit operation
//! frame. A boundary application nested under another operand (`a * b + c`)
//! has no statement operation, so the computation expansion hands its completed
//! operands here. The emitted `BoundaryCall` records the application's exact
//! source-call occurrence beside the operation, exactly as the statement route
//! does, so stage 07 rejoins it to one checked operator application and
//! provider selection stays one-to-one with Terminal operations.

use super::buffer::{OperationBuffer, SourceCallCoordinate};
use super::calls::CallEmissionContext;
use super::expressions::{LoweredDirectExpression, emit_direct_expression};
use crate::lowering_error::{LoweringError, unsupported};
use crate::terminal_identities::value_id;
use semantic_vocabulary::{QualifiedScalarType, ValueId};
use terminal_psi::{Operation, OperationKind, OperationResult, ValueDeclaration};

/// A completed nested boundary application: its operands are parameters of
/// the emitting block in authored order, and its result is one bare scalar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LoweredBoundaryCallBinding {
    pub(crate) source_coordinate: SourceCallCoordinate,
    /// The authored call expression this application executes.
    pub(crate) source_expression: typed_trees::expression::ExpressionHandle,
    pub(crate) target_machine: symbols::SymbolHandle,
    pub(crate) result_type: QualifiedScalarType,
    pub(crate) arguments: Vec<LoweredDirectExpression>,
}

pub(super) fn emit_boundary_call_binding(
    call: &LoweredBoundaryCallBinding,
    parameters: &[ValueDeclaration],
    next_value_identity: &mut u64,
    operations: &mut OperationBuffer,
    call_emission: &mut CallEmissionContext<'_>,
) -> Result<ValueId, LoweringError> {
    let mut targets = call_emission
        .boundaries
        .iter()
        .filter(|boundary| boundary.source == call.target_machine);
    let (Some(target), None) = (targets.next(), targets.next()) else {
        return unsupported("nested boundary application has no unique lowered declaration");
    };
    // Admission only builds scalar-only, receiver-free nodes. A declaration
    // that owes structural operands or scalar `requires` obligations needs the
    // statement route's transfer and obligation custody instead.
    if !target.structural.is_empty() || target.requirement_count != 0 {
        return unsupported(
            "nested boundary application target needs structural or obligation custody",
        );
    }
    if !call.result_type.qualifications.is_empty() {
        return unsupported("nested boundary application result carries qualifications");
    }
    if call.arguments.len() != target.scalar.len() {
        return unsupported("nested boundary application operand count disagrees");
    }
    let parameter_types = parameters
        .iter()
        .map(|parameter| QualifiedScalarType {
            scalar_type: parameter.scalar_type,
            qualifications: parameter.qualifications,
        })
        .collect::<Vec<_>>();
    let mut arguments = Vec::with_capacity(call.arguments.len());
    for (argument, expected) in call.arguments.iter().zip(&target.scalar) {
        let value_type = argument.value_type(&parameter_types)?;
        if value_type.scalar_type != *expected || !value_type.qualifications.is_empty() {
            return unsupported("nested boundary application operand disagrees with its formal");
        }
        arguments.push(emit_direct_expression(
            argument,
            parameters,
            next_value_identity,
            operations,
        ));
    }
    let result = value_id(*next_value_identity);
    *next_value_identity = next_value_identity
        .checked_add(1)
        .ok_or(LoweringError::Unsupported(
            "nested boundary application value identity overflow",
        ))?;
    let operation = operations.allocate();
    operations.record_source_call(
        call.source_coordinate,
        Some(checked_trees::NominalMachineUseSite::Expression(
            call.source_expression,
        )),
        operation,
        call.target_machine,
    )?;
    operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: operation,
        result: OperationResult::Scalar(ValueDeclaration {
            id: result,
            scalar_type: call.result_type.scalar_type,
            qualifications: Default::default(),
        }),
        kind: OperationKind::BoundaryCall {
            boundary: target.id,
            arguments,
            structural_arguments: Vec::new(),
            completion_receipts: Vec::new(),
            requirement_obligations: Vec::new(),
        },
    });
    Ok(result)
}
