//! Invocation-owned operation identities and exact source-occurrence companions.

use crate::lowering_error::LoweringError;
use crate::lowering_error::unsupported;
use crate::terminal_identities::operation_id;
use lowered_psi::{LoweredSelectedIeeeFloatFmaOccurrence, LoweredSourceCallOccurrence};
use semantic_vocabulary::{OperationId, PlaceId, ValueId};
use terminal_psi::{Operation, ValueDeclaration};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SourceCallCoordinate {
    pub(crate) state: symbols::SymbolHandle,
    pub(crate) statement_index: usize,
    pub(crate) call_ordinal: usize,
}

/// Module-wide operation identities for one machine namespace. Machine zero
/// uses the historical one-based range; additional machines receive disjoint
/// ranges when source call-closure production composes them.
pub(crate) struct OperationBuffer {
    /// Authored structural bindings may complete at a control join rather than
    /// a call operation. Keep those places in the same state-local namespace.
    pub(crate) structural_values: Vec<(u32, terminal_psi::StructuralOperationResult)>,
    pub(crate) selected_ieee_float_comparisons:
        Vec<lowered_psi::LoweredSelectedIeeeFloatComparisonOccurrence>,
    pub(crate) selected_integer_comparisons:
        Vec<lowered_psi::LoweredSelectedIntegerComparisonOccurrence>,
    pub(crate) next_identity: u64,
    pub(crate) operations: Vec<Operation>,
    /// Temporary observations available on the current emission path only.
    pub(crate) byte_lengths: Vec<(PlaceId, ValueId)>,
    pub(crate) source_calls: Vec<LoweredSourceCallOccurrence>,
    pub(crate) selected_ieee_float_fmas: Vec<LoweredSelectedIeeeFloatFmaOccurrence>,
}

impl OperationBuffer {
    pub(crate) fn new(identity_base: u64) -> Self {
        Self {
            structural_values: Vec::new(),
            next_identity: identity_base
                .checked_add(1)
                .expect("operation identity base admits one-based identities"),
            operations: Vec::new(),
            byte_lengths: Vec::new(),
            source_calls: Vec::new(),
            selected_ieee_float_fmas: Vec::new(),
            selected_ieee_float_comparisons: Vec::new(),
            selected_integer_comparisons: Vec::new(),
        }
    }

    pub(crate) fn allocate(&mut self) -> OperationId {
        let id = operation_id(self.next_identity);
        self.next_identity = self
            .next_identity
            .checked_add(1)
            .expect("terminal operation identities advance");
        id
    }

    pub(crate) fn record_source_call(
        &mut self,
        coordinate: SourceCallCoordinate,
        source_site: Option<checked_trees::NominalMachineUseSite>,
        operation: OperationId,
        target: symbols::SymbolHandle,
    ) -> Result<(), LoweringError> {
        self.record_source_call_with_values(coordinate, source_site, operation, target, &[])
    }

    pub(crate) fn record_source_call_with_values(
        &mut self,
        coordinate: SourceCallCoordinate,
        source_site: Option<checked_trees::NominalMachineUseSite>,
        operation: OperationId,
        target: symbols::SymbolHandle,
        source_values_before_call: &[ValueDeclaration],
    ) -> Result<(), LoweringError> {
        if self.source_calls.iter().any(|existing| {
            existing.source_state == coordinate.state
                && existing.statement_index == coordinate.statement_index
                && existing.call_ordinal == coordinate.call_ordinal
        }) {
            return Err(LoweringError::DuplicateContentPartitionProducerCoordinate);
        }
        self.source_calls.push(LoweredSourceCallOccurrence {
            source_site,
            source_state: coordinate.state,
            statement_index: coordinate.statement_index,
            call_ordinal: coordinate.call_ordinal,
            terminal_operation: operation,
            source_target: target,
            source_values_before_call: source_values_before_call.to_vec(),
        });
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn record_selected_ieee_float_fma(
        &mut self,
        coordinate: SourceCallCoordinate,
        operation: OperationId,
        requirement_operator: symbols::SymbolHandle,
        provider_plan_report_fingerprint: u64,
        provider_plan_commitment: checked_trees::CheckedProviderPlanCommitment,
        format: semantic_vocabulary::IeeeFloatFormat,
    ) -> Result<(), LoweringError> {
        if provider_plan_report_fingerprint == 0 || provider_plan_commitment.is_empty() {
            return unsupported(
                "selected IEEE FMA occurrence lacks complete ProviderPlan evidence",
            );
        }
        if self.selected_ieee_float_fmas.iter().any(|existing| {
            existing.source_state == coordinate.state
                && existing.statement_index == coordinate.statement_index
                && existing.call_ordinal == coordinate.call_ordinal
        }) {
            return unsupported("selected IEEE FMA occurrence coordinate is duplicated");
        }
        self.selected_ieee_float_fmas
            .push(LoweredSelectedIeeeFloatFmaOccurrence {
                source_state: coordinate.state,
                statement_index: coordinate.statement_index,
                call_ordinal: coordinate.call_ordinal,
                terminal_operation: operation,
                requirement_operator,
                provider_plan_report_fingerprint,
                provider_plan_commitment,
                format,
            });
        Ok(())
    }
}

impl std::ops::Deref for OperationBuffer {
    type Target = Vec<Operation>;

    fn deref(&self) -> &Self::Target {
        &self.operations
    }
}

impl std::ops::DerefMut for OperationBuffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.operations
    }
}

#[cfg(test)]
mod tests {
    use super::{LoweringError, OperationBuffer, SourceCallCoordinate, ValueDeclaration};
    use crate::terminal_identities::value_id;
    use semantic_vocabulary::ScalarType;

    #[test]
    fn operation_identities_start_after_the_selected_machine_base() {
        for base in [0, 1_u64 << 32] {
            let mut buffer = OperationBuffer::new(base);
            assert_eq!(buffer.allocate().get(), base + 1);
            assert_eq!(buffer.allocate().get(), base + 2);
        }
    }

    #[test]
    fn source_call_custody_owns_the_pre_call_values_and_rejects_duplicate_coordinates() {
        let mut buffer = OperationBuffer::new(0);
        let coordinate = SourceCallCoordinate {
            state: Default::default(),
            statement_index: 7,
            call_ordinal: 2,
        };
        let mut values = vec![ValueDeclaration {
            id: value_id(4),
            scalar_type: ScalarType::Boolean,
            qualifications: Default::default(),
        }];
        let operation = buffer.allocate();
        buffer
            .record_source_call_with_values(
                coordinate,
                None,
                operation,
                Default::default(),
                &values,
            )
            .unwrap();
        values.clear();
        assert_eq!(buffer.source_calls[0].source_values_before_call.len(), 1);
        assert_eq!(buffer.source_calls[0].terminal_operation, operation);

        let other_operation = buffer.allocate();
        assert!(matches!(
            buffer.record_source_call(coordinate, None, other_operation, Default::default()),
            Err(LoweringError::DuplicateContentPartitionProducerCoordinate)
        ));
        assert_eq!(buffer.source_calls.len(), 1);
        buffer
            .record_source_call(
                SourceCallCoordinate {
                    call_ordinal: 3,
                    ..coordinate
                },
                None,
                other_operation,
                Default::default(),
            )
            .unwrap();
        assert_eq!(buffer.source_calls.len(), 2);
    }
}
