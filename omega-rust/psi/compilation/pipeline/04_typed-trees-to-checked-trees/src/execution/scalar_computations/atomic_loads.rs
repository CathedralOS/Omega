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
        let field = validation::structural_atomic_field(
            self.program,
            self.machine,
            self.state,
            carrier.place,
            &[],
            |receiver| {
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
            || field.receiver_value.is_valid()
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
