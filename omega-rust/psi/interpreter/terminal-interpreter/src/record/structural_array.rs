//! Structural array establishment uses the same child storage transaction as
//! records and cases; each element is a selected owned structural value.
//! Actual child memberships use the same relocation as records, with real
//! FixedIndex paths. Declared result metadata never supplies missing child proof.
use super::storage::{ConstructorStorage, ConstructorValue};
use super::*;
use terminal_psi::OperationKind;

impl TerminalExecution<'_> {
    pub(crate) fn execute_establish_structural_array(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<crate::execution::OperationFlow, TerminalInterpretError> {
        let OperationKind::EstablishStructuralArray { elements } = &operation.kind else {
            return Err(malformed());
        };
        let result = operation.result.structural().ok_or_else(malformed)?;
        let Some(StructuralTypeShape::FixedArray { element, length }) = self
            .program
            .structural_types()
            .get(&result.structural_type)
            .map(|row| &row.shape)
        else {
            return Err(malformed());
        };
        let element = *element;
        if u64::try_from(elements.len()).ok() != Some(*length)
            || !result.qualifications.is_empty()
            || self.live_affine_discard_at(result.place).is_some()
            || self
                .live_claims
                .values()
                .any(|claim| claim.place == Some(result.place))
        {
            return Err(malformed());
        }
        let mut cursor = self.local_structural_identities.cursor();
        let mut value = TerminalStructuralValue {
            opaque_identity: self
                .local_structural_identities
                .allocate_staged(&mut cursor)?,
            structural_type: result.structural_type,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            path: Vec::new(),
        };
        let mut staged = ConstructorStorage::default();
        let mut frontier = self.live_affine_frontier.clone();
        let mut consumed = Vec::new();
        let mut claim_operands = Vec::new();
        for (index, argument) in elements.iter().enumerate() {
            let child = ConstructorValue::resolve(self, argument)?;
            if argument.access != StructuralAccess::Owned
                || argument.place == result.place
                || child.structural_type() != element
            {
                return Err(malformed());
            }
            let transfers_custody =
                self.stage_constructor_operand(argument, &mut frontier, &mut consumed)?;
            let destination = StructuralRuntimePlace {
                opaque_identity: value.opaque_identity,
                path: vec![StructuralPathSegment::FixedIndex(
                    u64::try_from(index).map_err(|_| malformed())?,
                )],
            };
            child.append_memberships(&destination.path, &mut value.projected_qualifications);
            claim_operands.push((argument, destination.path.clone()));
            staged.stage(self, argument, &child, &destination, transfers_custody)?;
        }
        value.projected_qualifications.sort();
        value.projected_qualifications.dedup();
        let claims =
            self.stage_constructor_claims(result, &claim_operands, &mut frontier, &|current| {
                staged.case_at_projection(self, &value, &current.path)
            })?;
        self.commit_constructor_operands(frontier, &consumed, claims)?;
        self.local_structural_identities.commit_cursor(cursor);
        staged.publish(self)?;
        self.structural_values.insert(result.place, value);
        Ok(crate::execution::OperationFlow::Advance)
    }
}
