//! Atomic loads share structural observation geometry, while their explicit
//! mode retains the event and ordering at this computation's evaluation point.
//! Select/Dispatch own whether this node executes; no parser hoist can preserve
//! that distinction. The location remains an actual dedicated atomic field.
use super::{
    Builder, CheckedScalarComputationHandle, CheckedScalarComputationKind, ExpressionHandle,
    PrimitiveType,
};

impl Builder<'_, '_> {
    pub(super) fn atomic_load(
        &mut self,
        expression: ExpressionHandle,
        expected: PrimitiveType,
    ) -> Option<CheckedScalarComputationHandle> {
        let carrier = validation::atomic_load_carrier(self.program, expression)?;
        let validation::AtomicAccessOperation::Load(ordering) = carrier.operation else {
            return None;
        };
        let machine = self.program.machine_by_symbol(self.machine)?;
        let state = self.program.state_by_symbol(self.state)?;
        if !validation::place_is_atomic_storage(self.program, machine, Some(state), carrier.place)
            || !validation::place_has_builtin_coordinates(
                self.program,
                machine,
                Some(state),
                carrier.place,
            )
        {
            return None;
        }
        // A completed descriptor supplies the receiver's identity and type;
        // the atomic leaf remains live storage reached through that descriptor.
        // Use the same exact extraction boundary as ordinary saved projections.
        // Missing checked publication is not the absence of a copied operand.
        // A real Flow copy use must keep its saved root or refuse this plan.
        let captured = self.captured_dispatch_value(carrier.place);
        if self.uses_captured_dispatch_value(carrier.place) && captured.is_none() {
            return None;
        }
        let evaluated_receiver = if let Some(copy) = captured {
            let receiver = validation::DispatchExtraction::payload_binding_receiver(
                self.program,
                carrier.place,
                copy.expression,
            )?;
            Some((
                receiver,
                validation::StructuralSource {
                    root: facts::PlaceRoot::Expression(copy.expression),
                    receiver_value: ExpressionHandle::invalid(),
                    statement_ordinal: u32::try_from(self.statement_index).ok()?,
                    type_reference: copy.type_reference,
                },
            ))
        } else {
            None
        };
        let field = validation::structural_atomic_field(
            self.program,
            self.machine,
            self.state,
            carrier.place,
            &[],
            evaluated_receiver,
            |receiver| {
                // A selected completed receiver must be found on this spine.
                // Failure cannot reopen the old, potentially mutated storage.
                if evaluated_receiver.is_some() {
                    return None;
                }
                validation::structural_source(
                    self.program,
                    self.machine,
                    self.state,
                    u32::try_from(self.statement_index).ok()?,
                    carrier.place,
                    receiver,
                )
            },
        )?;
        if field.primitive_type != expected
            || !field.field.is_valid()
            || field.path.iter().any(|segment| {
                !matches!(
                    segment,
                    checked_trees::CheckedUnitStructuralPathSegment::Field(_)
                        | checked_trees::CheckedUnitStructuralPathSegment::Case(_)
                        | checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(_)
                        | checked_trees::CheckedUnitStructuralPathSegment::Referent
                )
            })
        {
            return None;
        }
        let computation = self.structural_field(carrier.place, field)?;
        let CheckedScalarComputationKind::StructuralField {
            source_expression,
            atomic_ordering,
            ..
        } = &mut self.plans.nodes.get_mut(computation).kind
        else {
            return None;
        };
        *source_expression = expression;
        *atomic_ordering = Some(ordering);
        Some(computation)
    }
}
