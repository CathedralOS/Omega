//! Evaluate scalar call operands in authored order, then emit the exact callee.

use super::buffer::{OperationBuffer, SourceCallCoordinate};
use super::expressions::{LoweredDirectExpression, emit_direct_expression};
use crate::direct_calls::{DirectCall, DirectCallResult};
use crate::lowering_error::LoweringError;
use crate::lowering_error::unsupported;
use crate::proofs::crash_routes::{lower_checked_crash_route_buckets, lowered_direct_scalar_term};
use crate::terminal_identities::obligation_id;
use crate::terminal_identities::value_id;
use semantic_vocabulary::{MachineId, ObligationId, QualifiedScalarType, ValueId};
use terminal_psi::{Operation, StructuralArgument, ValueDeclaration};

/// The source place is known during preparation; runtime path values and their
/// bounds obligations belong to the emitting call's actual scalar frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PreparedStructuralArgument {
    pub(crate) place: semantic_vocabulary::PlaceId,
    pub(crate) access: terminal_psi::StructuralAccess,
    pub(crate) path: Vec<
        crate::expression_preparation::bindings::structural_paths::PreparedStructuralPathSegment,
    >,
    /// Field-selector snapshots in path order, carried in the call input frame.
    pub(crate) selector_positions: Vec<usize>,
}

impl PreparedStructuralArgument {
    pub(crate) fn from_static(argument: StructuralArgument) -> Result<Self, LoweringError> {
        let path = argument.path.into_iter().map(|segment| {
            if matches!(segment, terminal_psi::StructuralPathSegment::RuntimeIndex { .. }) {
                return unsupported("prepared call cannot capture another operation's runtime path");
            }
            Ok(crate::expression_preparation::bindings::structural_paths::PreparedStructuralPathSegment::Static(segment))
        }).collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            place: argument.place,
            access: argument.access,
            path,
            selector_positions: Vec::new(),
        })
    }

    pub(super) fn emit(
        &self,
        values: &[ValueDeclaration],
        calls: &mut CallEmissionContext<'_>,
        next_value: &mut u64,
        operations: &mut OperationBuffer,
    ) -> Result<StructuralArgument, LoweringError> {
        let selectors = self
            .selector_positions
            .iter()
            .map(|position| {
                let value = values.get(*position).ok_or(LoweringError::Unsupported(
                    "captured selector has no value in the call input frame",
                ))?;
                if !matches!(
                    value.scalar_type,
                    semantic_vocabulary::ScalarType::Integer(integer) if !integer.is_address()
                ) {
                    return unsupported("captured selector is not an integer");
                }
                Ok(value.id)
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        let mut selectors = selectors.into_iter();
        let path =
            crate::expression_preparation::bindings::structural_paths::emit_captured_argument_path(
                &self.path,
                self.place,
                Some(values),
                &mut selectors,
                Some(calls),
                Some((next_value, operations)),
            )?;
        if selectors.next().is_some() {
            return unsupported("captured selectors exceed the structural argument path");
        }
        Ok(StructuralArgument {
            place: self.place,
            access: self.access,
            path,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LoweredDirectCallBinding {
    pub(crate) source_coordinate: SourceCallCoordinate,
    pub(crate) target_machine: symbols::SymbolHandle,
    pub(crate) result_type: QualifiedScalarType,
    pub(crate) arguments: Vec<LoweredDirectExpression>,
    /// Proof-only actuals in the callee's erased-formal order, lowered under
    /// the caller's retained/erased scalar namespaces.
    pub(crate) erased_arguments: Vec<LoweredDirectExpression>,
    /// Erased proof-only actuals in the callee's erased-proof roster order.
    /// `Formal` positions index the caller's erased-proof roster; scalar
    /// leaves resolve against this operation's caller namespace at emission.
    pub(crate) erased_proof_arguments: Vec<crate::scalar_lowering::contracts::LoweredProofTerm>,
    pub(crate) structural_arguments: Vec<PreparedStructuralArgument>,
    /// The caller claims each owned operand hands over, resolved through the
    /// emitting body's claim table when the call is written.
    pub(crate) claim_transfers: Vec<crate::emission::claim_bindings::PreparedClaimTransfer>,
    pub(crate) crash_continuations: Vec<checked_trees::CrashRouteBucket>,
    pub(crate) parameter_relative_crash_routes: Vec<checked_trees::CrashRouteBucket>,
}

pub(crate) struct CallEmissionContext<'a> {
    pub(crate) domain_ids: &'a [crate::terminal_identities::StructuralDomainBinding],
    pub(crate) claims: crate::emission::claim_bindings::ClaimBindings<'a>,
    pub(crate) cleanup_receivers: &'a [(symbols::SymbolHandle, semantic_vocabulary::PlaceId)],
    pub(crate) machine_ids: &'a [(symbols::SymbolHandle, MachineId)],
    pub(crate) requirement_counts: &'a [(symbols::SymbolHandle, usize)],
    /// Lowered boundary declarations a nested boundary application may name.
    /// Routes without an attached Unit closure carry none, so such an
    /// application fails closed there instead of guessing an identity.
    pub(crate) boundaries: &'a [crate::unit::attached_unit::BoundaryParameters],
    pub(crate) next_obligation_identity: u64,
    pub(crate) obligation_limit: u64,
}

impl CallEmissionContext<'_> {
    pub(crate) fn allocate_requirement(&mut self) -> Result<ObligationId, LoweringError> {
        if self.next_obligation_identity >= self.obligation_limit {
            return unsupported("terminal call obligations exceed their machine identity range");
        }
        let obligation = obligation_id(self.next_obligation_identity);
        self.next_obligation_identity = self
            .next_obligation_identity
            .checked_add(1)
            .expect("terminal call obligation identities advance");
        Ok(obligation)
    }
}

pub(super) fn emit_scalar_call_binding(
    call: &LoweredDirectCallBinding,
    parameters: &[ValueDeclaration],
    caller_erased_formals: &[ValueDeclaration],
    next_value_identity: &mut u64,
    operations: &mut OperationBuffer,
    call_emission: &mut CallEmissionContext<'_>,
) -> Result<ValueId, LoweringError> {
    let parameter_types = parameters
        .iter()
        .map(|parameter| parameter.value_type())
        .collect::<Vec<_>>();
    let arguments = call
        .arguments
        .iter()
        .map(|argument| {
            let value_type = argument.value_type(&parameter_types)?;
            Ok(ValueDeclaration {
                qualifications: value_type.qualifications,
                id: emit_direct_expression(argument, parameters, next_value_identity, operations),
                scalar_type: value_type.scalar_type,
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    emit_direct_call_operation(
        call,
        // Terminal calls retain the callee's published routes substituted onto
        // evaluated argument values. Source-refined surviving buckets can fold
        // a literal guard to Truth or remove it, which is not that exact value
        // substitution and disagrees with the independently verified callee.
        &call.parameter_relative_crash_routes,
        &arguments,
        parameters,
        caller_erased_formals,
        &arguments,
        next_value_identity,
        operations,
        call_emission,
    )
}

fn emit_direct_call_operation(
    call: &LoweredDirectCallBinding,
    crash_routes: &[checked_trees::CrashRouteBucket],
    crash_values: &[ValueDeclaration],
    source_values_before_call: &[ValueDeclaration],
    caller_erased_formals: &[ValueDeclaration],
    arguments: &[ValueDeclaration],
    next_value_identity: &mut u64,
    operations: &mut OperationBuffer,
    call_emission: &mut CallEmissionContext<'_>,
) -> Result<ValueId, LoweringError> {
    let callee = crate::terminal_identities::lookup_machine_id_with(
        call_emission.machine_ids,
        call.target_machine,
        "direct scalar call target is absent from the terminal closure",
    )?;
    let crash_continuations = lower_checked_crash_route_buckets(crash_routes, crash_values)?;
    let requirement_count = call_emission
        .requirement_counts
        .iter()
        .find_map(|(source, count)| (*source == call.target_machine).then_some(*count))
        .ok_or(LoweringError::Unsupported(
            "direct scalar call target has no prepared contract",
        ))?;
    let requirement_obligations = (0..requirement_count)
        .map(|_| call_emission.allocate_requirement())
        .collect::<Result<Vec<_>, _>>()?;
    let result = value_id(*next_value_identity);
    *next_value_identity = next_value_identity
        .checked_add(1)
        .expect("generated value identity advances after a direct call");
    let operation = operations.allocate();
    operations.record_source_call_with_values(
        call.source_coordinate,
        None,
        operation,
        call.target_machine,
        source_values_before_call,
    )?;
    let erased_arguments = call
        .erased_arguments
        .iter()
        .map(|argument| {
            lowered_direct_scalar_term(argument, source_values_before_call, caller_erased_formals)
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let erased_proof_arguments = call
        .erased_proof_arguments
        .iter()
        .map(|term| {
            crate::scalar_lowering::contracts::lowered_proof_term(
                term,
                source_values_before_call,
                caller_erased_formals,
            )
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let structural_arguments = call
        .structural_arguments
        .iter()
        .map(|argument| {
            argument.emit(
                source_values_before_call,
                call_emission,
                next_value_identity,
                operations,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let claim_transfers = crate::emission::claim_bindings::emit_prepared_claim_transfers(
        &call.claim_transfers,
        call_emission.claims.bindings(),
    )?;
    operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: operation,
        result: terminal_psi::OperationResult::Scalar(ValueDeclaration {
            qualifications: call.result_type.qualifications,
            id: result,
            scalar_type: call.result_type.scalar_type,
        }),
        kind: DirectCall {
            callee,
            arguments: arguments.iter().map(|argument| argument.id).collect(),
            erased_arguments,
            erased_proof_arguments,
            structural_arguments,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
            result: DirectCallResult::Scalar,
        }
        .into_operation_kind(),
    });
    Ok(result)
}
