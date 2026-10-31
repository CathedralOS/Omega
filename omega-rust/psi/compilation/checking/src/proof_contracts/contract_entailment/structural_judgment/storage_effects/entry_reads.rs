//! Declaration-entry observations have an explicit cut before statement zero.
//! Their selectors come from that entry's formal-value namespace, rather than
//! an executable expression schedule. They never enter the completed-read
//! roster. Current reads continue to require their actual executed selectors.

use super::super::StructuralTerm;
use super::{CapturedStorageCoordinate, StructuralJudge};
use typed_trees::{expression::ExpressionHandle, machine::Machine, state::State};

impl StructuralJudge<'_> {
    pub(super) fn capture_entry_storage_place(
        &self,
        place: crate::CanonicalPlace,
    ) -> Option<CapturedStorageCoordinate> {
        if self.storage_write_sequence != 0 {
            return None;
        }
        self.capture_storage_place_with_selectors(place, |expression| {
            self.declaration_term(expression)
        })
    }

    /// The caller has independently admitted the source observation's geometry,
    /// selectors and formation. Authenticate its exact readable backing at entry
    /// before freezing its scalar value; no declaration grants a later reread.
    pub(in crate::proof_contracts::contract_entailment) fn admitted_entry_scalar_value(
        &mut self,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
    ) -> Result<StructuralTerm, &'static str> {
        if self.declaration_state != state.symbol || self.storage_write_sequence != 0 {
            return Err("the rank entry observation is outside its declaration entry epoch");
        }
        if !self.ordinary_storage_path(Some((machine, state)), expression) {
            return Err("the rank entry input is not a selected builtin storage path");
        }
        super::super::super::observed_projections::scalar_storage_read(
            self.program,
            machine,
            state,
            expression,
        )
        .ok_or("the rank entry input has no declared primitive storage-read geometry")?;
        let canonical = crate::canonical_place_from_expression(self.program, expression)
            .ok_or("the rank entry input has no canonical storage coordinate")?;
        let root = canonical
            .root
            .as_symbol()
            .ok_or("the rank entry input has no declared storage root")?;
        self.program
            .state_parameter_by_symbol(state, root)
            .ok_or("the rank entry input is not rooted in its state formals")?;
        let backing = self
            .frame_resolver()
            .ok_or("the rank entry input has no call-frame resolver")?
            .storage_read_backing_before_statement(machine, state, 0, root, &canonical.segments)
            .ok_or("the rank entry input has no authenticated before-statement backing")?;
        let coordinate = self
            .capture_entry_storage_place(canonical)
            .ok_or("the rank entry input has no exact formal selector snapshot")?;
        let value = self
            .declaration_term(expression)
            .ok_or("the rank entry input has no admitted entry value description")?;
        let backing = self.freeze_entry_scalar_read_backing(state, backing);
        self.retain_storage_value(coordinate.clone(), value, true);
        let stored = std::sync::Arc::make_mut(&mut self.stored_values)
            .iter_mut()
            .find(|stored| stored.coordinate == coordinate)
            .ok_or("the rank entry input lost its retained storage observation")?;
        stored.backing = Some(backing);
        stored.completed_read = None;
        stored
            .entry
            .clone()
            .ok_or("the rank entry input lost its entry scalar snapshot")
    }
}
