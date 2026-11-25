//! The placement index of resident custody is the qualified type of its value.

use super::{
    Checked, Construct, MachineId, Program, Qualification, SourceOrigin, TypeReferenceHandle,
    Unserved, refuse,
};

impl Program<'_> {
    /// Read the canonical qualified type P and the resident value type T under
    /// the source frames that supplied them to initialization, extraction or a view.
    pub(super) fn resident_index(
        &mut self,
        id: MachineId,
        source: SourceOrigin,
        placement: TypeReferenceHandle,
        schema: TypeReferenceHandle,
    ) -> Checked<Qualification> {
        let typed = self.typed;
        let unavailable = |reason| refuse(id, Construct::Machine, Unserved::Call(reason));
        let placed = self.local_type(id, placement)?;
        let value = self.local_type(id, schema)?;
        if matches!(placed, crate::body::ValueType::Rigid(_))
            || matches!(value, crate::body::ValueType::Rigid(_))
        {
            return Err(unavailable(
                "resident placement needs a closed type index and value",
            ));
        }
        let domains = placed.qualification().domains;
        if domains.is_empty() {
            return Err(crate::reject(
                id,
                source,
                format!(
                    "resident placement index `{}` must name a domain-qualified type",
                    typed.display_type_reference_with_constraints(placement)
                ),
            ));
        }
        // Record coordinates can differ between an imported type and the
        // same type inside a domain alias. Compare their source indices under
        // the frames that supplied them. Peel only outer qualifications:
        // a domain inside a generic argument still belongs to the carrier.
        let carrier = |mut ty: crate::source_state::SourceTypeApplication| loop {
            let base = typed
                .type_reference_table
                .unconstrained_type_reference(ty.reference);
            if base == ty.reference {
                return ty.scope.index_identity(typed, base);
            }
            ty = ty.scope.supplied_type_reference(typed, base).ok()?;
        };
        let supplied = self
            .source_scope
            .supplied_type_reference(typed, schema)
            .map_err(unavailable)?;
        let index = self
            .source_scope
            .supplied_type_reference(typed, placement)
            .map_err(unavailable)?;
        let supplied_reference = supplied.reference;
        let (Some(placed_carrier), Some(value_carrier)) = (carrier(index), carrier(supplied))
        else {
            return Err(unavailable(
                "resident placement needs the closed source carriers of its index and value",
            ));
        };
        let semantic = value.qualification();
        if placed_carrier != value_carrier
            || placed.policy() != semantic.policy
            || !semantic.domains.is_subset_of(&domains)
        {
            return Err(crate::reject(
                id,
                source,
                format!(
                    "resident placement index `{}` does not qualify the supplied value type `{}`",
                    typed.display_type_reference_with_constraints(placement),
                    typed.display_type_reference_with_constraints(supplied_reference)
                ),
            ));
        }
        Ok(placed.qualification())
    }
}
