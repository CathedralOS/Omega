//! A representation view keeps the storage it actually borrowed before the
//! new interpretation. The whole source is a safe enclosure for owned fields;
//! its logical field symbols must never be appended to the source layout.
//! This relation is independent of lifetime roots and exact value referents.
//! Resident views borrow content, so their Extent descriptor is excluded.
use super::Execution;
use arena::Handle;
use checked_trees::{FlowOperandFact, FlowOperandRetention};
use typed_trees::expression::{ExpressionHandle, TableCallExpression};

impl Execution<'_, '_, '_> {
    pub(super) fn view_enclosure(
        &self,
        profile: Handle<FlowOperandRetention>,
    ) -> Handle<FlowOperandFact> {
        if self.context.control.operand_retentions.is_valid(profile) {
            self.context
                .control
                .operand_retentions
                .get(profile)
                .enclosing_storage
        } else {
            Handle::invalid()
        }
    }

    pub(super) fn reinterpreted_view_retention(
        &mut self,
        source: &FlowOperandFact,
        lifetime: Handle<FlowOperandRetention>,
    ) -> Handle<FlowOperandRetention> {
        if !self.context.control.operand_retentions.is_valid(lifetime) {
            return lifetime;
        }
        let profile = *self.context.control.operand_retentions.get(lifetime);
        let enclosing_storage = self.context.control.operands.insert(*source);
        self.context
            .control
            .operand_retentions
            .insert(FlowOperandRetention {
                enclosing_storage,
                ..profile
            })
    }

    pub(super) fn capture_formation_enclosure(
        &mut self,
        expression: ExpressionHandle,
        call: &TableCallExpression,
        inputs: &[(ExpressionHandle, FlowOperandFact)],
        retention: &mut Option<Handle<FlowOperandRetention>>,
    ) {
        let Some(formation) = validation::reference_formation_source(self.program, expression)
        else {
            return;
        };
        if formation.interpretation != validation::ReferenceFormationInterpretation::Placed {
            return;
        }
        // The formation owner authenticated this single ordinary-source factory.
        // The namespace receiver was evaluated separately; it has no source slot.
        let Some([(_, source)]) = inputs.get(usize::from(call.receiver.is_valid())..) else {
            return;
        };
        if let Some(lifetime) = *retention {
            *retention = Some(self.reinterpreted_view_retention(source, lifetime));
        }
    }
}
