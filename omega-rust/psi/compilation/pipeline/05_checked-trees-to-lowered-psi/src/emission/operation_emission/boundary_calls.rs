//! Emit one nested boundary application inside an operand evaluation.
//!
//! A statement-level boundary call is emitted by the attached Unit operation
//! frame. A boundary application nested under another operand (`a * b + c`)
//! has no statement operation, so the computation expansion hands its completed
//! operands here. The emitted `BoundaryCall` records the application's exact
//! source-call occurrence beside the operation, exactly as the statement route
//! does, so stage 07 rejoins it to one checked operator application and
//! provider selection stays one-to-one with Terminal operations.

pub(crate) mod result_policy;

use super::buffer::{OperationBuffer, SourceCallCoordinate};
use super::calls::{CallEmissionContext, PreparedStructuralArgument};
use super::expressions::{LoweredDirectExpression, emit_direct_expression};
use crate::lowering_error::{LoweringError, unsupported};
use crate::terminal_identities::value_id;
use result_policy::BoundaryResultPolicy;
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
    pub(crate) result_policy: BoundaryResultPolicy,
    pub(crate) arguments: Vec<LoweredDirectExpression>,
    pub(crate) structural_arguments: Vec<PreparedStructuralArgument>,
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
    // Mixed computation operands use the shared non-linear call custody lane.
    // Linear claims require their checked completion-receipt namespace and are
    // not manufactured from a boundary's formal declaration at emission.
    if call.structural_arguments.len() != target.structural.len()
        || target
            .structural
            .iter()
            .any(|parameter| parameter.multiplicity == terminal_psi::StructuralMultiplicity::Linear)
    {
        return unsupported("computed boundary arguments disagree with non-linear call custody");
    }
    let boundary = target.id;
    let requirement_count = target.requirement_count;
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
    let structural_arguments = call
        .structural_arguments
        .iter()
        .map(|argument| argument.emit(parameters, call_emission))
        .collect::<Result<Vec<_>, _>>()?;
    // Match the statement call's declared requirement rows. Proof finalization
    // discharges these obligations at this exact operation and completed actuals.
    let requirement_obligations = (0..requirement_count)
        .map(|_| call_emission.allocate_requirement())
        .collect::<Result<Vec<_>, _>>()?;
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
    let raw = ValueDeclaration {
        id: result,
        scalar_type: call.result_type.scalar_type,
        qualifications: Default::default(),
    };
    let policy = call.result_policy.bind(raw, &arguments)?;
    operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: operation,
        result: OperationResult::Scalar(raw),
        kind: OperationKind::BoundaryCall {
            boundary,
            arguments,
            structural_arguments,
            completion_receipts: Vec::new(),
            requirement_obligations,
        },
    });
    Ok(result_policy::emit(policy, raw, next_value_identity, operations)?.id)
}
