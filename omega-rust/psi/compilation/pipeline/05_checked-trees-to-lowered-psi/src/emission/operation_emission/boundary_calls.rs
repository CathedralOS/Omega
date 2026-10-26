//! Emit one nested boundary application inside an operand evaluation.
//!
//! A statement-level boundary call is emitted by the attached Unit operation
//! frame. A boundary application nested under another operand (`a * b + c`)
//! has no statement operation, so the computation expansion hands its completed
//! operands here. The emitted `BoundaryCall` records the application's exact
//! source-call occurrence beside the operation, exactly as the statement route
//! does, so stage 07 rejoins it to one checked operator application and
//! provider selection stays one-to-one with Terminal operations.

mod overflow_eligibility;
pub(crate) mod result_policy;

use super::buffer::{OperationBuffer, SourceCallCoordinate};
use super::calls::{CallEmissionContext, PreparedStructuralArgument};
use super::expressions::{LoweredDirectExpression, emit_direct_expression};
use crate::lowering_error::{LoweringError, unsupported};
use crate::terminal_identities::value_id;
use result_policy::BoundaryResultPolicy;
use semantic_vocabulary::{QualifiedScalarType, ValueId};
use terminal_psi::{Operation, OperationKind, OperationResult, ValueDeclaration};

/// Completed operands are shared by statement calls and expression calls of
/// every result shape. A boundary transfer is a completion receipt, not an
/// ordinary body's claim transfer; each declared requirement retains its slot.
pub(crate) struct BoundaryOperands {
    pub(crate) boundary: semantic_vocabulary::BoundaryMachineId,
    pub(crate) arguments: Vec<ValueId>,
    pub(crate) structural_arguments: Vec<terminal_psi::StructuralArgument>,
    pub(crate) completion_receipts: Vec<terminal_psi::CompletionReceipt>,
    pub(crate) requirement_obligations: Vec<semantic_vocabulary::ObligationId>,
}

pub(crate) fn complete_operands(
    target: &crate::unit::attached_unit::BoundaryParameters,
    arguments: &[ValueDeclaration],
    structural_arguments: Vec<terminal_psi::StructuralArgument>,
    receipts: &[crate::emission::claim_bindings::PreparedClaimTransfer],
    calls: &mut CallEmissionContext<'_>,
    current_claims: &mut crate::emission::current_claims::CurrentClaimNames,
) -> Result<BoundaryOperands, LoweringError> {
    if structural_arguments.len() != target.structural.len() {
        return unsupported("boundary arguments disagree with their declared structural roster");
    }
    let arguments = crate::unit::attached_unit::argument_evaluation::validated_values(
        Some(arguments),
        &target.scalar,
    )?
    .iter()
    .map(|value| value.id)
    .collect();
    let completion_receipts = crate::emission::claim_bindings::emit_completion_receipts(
        receipts,
        calls.claims.bindings(),
        &structural_arguments,
        current_claims,
    )?;
    Ok(BoundaryOperands {
        boundary: target.id,
        arguments,
        structural_arguments,
        completion_receipts,
        requirement_obligations: (0..target.requirement_count)
            .map(|_| calls.allocate_requirement())
            .collect::<Result<Vec<_>, _>>()?,
    })
}

impl BoundaryOperands {
    pub(crate) fn into_kind(self) -> OperationKind {
        OperationKind::BoundaryCall {
            boundary: self.boundary,
            arguments: self.arguments,
            structural_arguments: self.structural_arguments,
            completion_receipts: self.completion_receipts,
            requirement_obligations: self.requirement_obligations,
        }
    }
}

/// A completed nested boundary application: its operands are parameters of
/// the emitting block in authored order, and its result is one bare scalar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LoweredBoundaryCallBinding {
    pub(crate) source_coordinate: SourceCallCoordinate,
    pub(crate) loan_completions: Vec<crate::emission::loan_completions::PreparedLoanCompletion>,
    /// The authored call expression this application executes.
    pub(crate) source_expression: typed_trees::expression::ExpressionHandle,
    pub(crate) target_machine: symbols::SymbolHandle,
    pub(crate) result_type: QualifiedScalarType,
    pub(crate) result_policy: BoundaryResultPolicy,
    pub(crate) arguments: Vec<LoweredDirectExpression>,
    pub(crate) structural_arguments: Vec<PreparedStructuralArgument>,
    pub(crate) completion_receipts: Vec<crate::emission::claim_bindings::PreparedClaimTransfer>,
}

pub(super) fn emit_boundary_call_binding(
    call: &LoweredBoundaryCallBinding,
    parameters: &[ValueDeclaration],
    next_value_identity: &mut u64,
    operations: &mut OperationBuffer,
    call_emission: &mut CallEmissionContext<'_>,
    current_claims: &mut crate::emission::current_claims::CurrentClaimNames,
) -> Result<ValueId, LoweringError> {
    let mut targets = call_emission
        .boundaries
        .iter()
        .filter(|boundary| boundary.source == call.target_machine);
    let (Some(target), None) = (targets.next(), targets.next()) else {
        return unsupported("nested boundary application has no unique lowered declaration");
    };
    if call.structural_arguments.len() != target.structural.len() {
        return unsupported("computed boundary arguments disagree with their call shape");
    }
    if target.result != terminal_psi::BoundaryMachineResult::Scalar(call.result_type) {
        return unsupported("nested boundary application result disagrees with its signature");
    }
    if call.arguments.len() != target.scalar.len() {
        return unsupported("nested boundary application operand count disagrees");
    }
    let parameter_types = parameters
        .iter()
        .map(|parameter| parameter.value_type())
        .collect::<Vec<_>>();
    let mut arguments = Vec::with_capacity(call.arguments.len());
    for (argument, expected) in call.arguments.iter().zip(&target.scalar) {
        let value_type = argument.value_type(&parameter_types)?;
        if value_type != *expected {
            return unsupported("nested boundary application operand disagrees with its formal");
        }
        arguments.push(ValueDeclaration {
            id: emit_direct_expression(argument, parameters, next_value_identity, operations),
            scalar_type: value_type.scalar_type,
            qualifications: value_type.qualifications,
        });
    }
    let structural_arguments = call
        .structural_arguments
        .iter()
        .map(|argument| argument.emit(parameters, call_emission, next_value_identity, operations))
        .collect::<Result<Vec<_>, _>>()?;
    let operands = complete_operands(
        target,
        &arguments,
        structural_arguments,
        &call.completion_receipts,
        call_emission,
        current_claims,
    )?;
    let result = value_id(*next_value_identity);
    *next_value_identity = next_value_identity
        .checked_add(1)
        .ok_or(LoweringError::Unsupported(
            "nested boundary application value identity overflow",
        ))?;
    let raw = ValueDeclaration {
        id: result,
        scalar_type: call.result_type.scalar_type,
        qualifications: call.result_type.qualifications,
    };
    let policy = call.result_policy.bind(
        raw,
        &operands.arguments,
        &target
            .scalar
            .iter()
            .map(|parameter| parameter.scalar_type)
            .collect::<Vec<_>>(),
        next_value_identity,
        operations,
    )?;
    let operation = operations.allocate();
    operations.record_source_call(
        call.source_coordinate,
        Some(checked_trees::NominalMachineUseSite::Expression(
            call.source_expression,
        )),
        operation,
        call.target_machine,
        &lowered_psi::LoweredSourceFrontierBindings::default(),
    )?;
    operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: operation,
        result: OperationResult::Scalar(raw),
        kind: operands.into_kind(),
    });
    Ok(result_policy::emit(policy, raw, next_value_identity, operations)?.id)
}
