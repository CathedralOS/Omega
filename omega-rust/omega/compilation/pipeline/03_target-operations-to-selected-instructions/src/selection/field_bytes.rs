//! Where a field byte observation finds its bytes. A bounded owner keeps
//! them inline after its live-length word; a shared view field keeps a
//! two-word descriptor whose first word points at backing outside the root.
//! Indexed reads, measured scans and copies share this geometry. A field's
//! storage layout does not change with its consumer: a descriptor must be
//! loaded, never addressed as though its words were inline payload.

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::selection) enum FieldBytes {
    /// The payload's offset from the root.
    Inline(u32),
    /// The descriptor's offset from the root; its pointer word is loaded.
    Descriptor(u32),
}

impl FieldBytes {
    /// Read from the exact incoming, state-owned, or completed home. The
    /// operation's accepted bound still supplies the readable live extent;
    /// neither a storage layout nor ownership manufactures that proof.
    pub(in crate::selection) fn resolve(
        function: &legalized_operations::LegalizedScalarFunction,
        source: &terminal_psi::StructuralArgument,
        field: semantic_vocabulary::StructuralFieldId,
        indices: &[legalized_operations::LegalizedRuntimeIndexOperand],
    ) -> Option<Self> {
        let structural_type = root_type(function, source)?;
        let signature = function.structural.as_ref()?;
        if source.access == terminal_psi::StructuralAccess::WriteOnlyBorrow {
            return None;
        }
        let geometry = crate::structural_inputs::structural_reference_input::projected_byte_field(
            structural_type,
            &source.path,
            field,
            &signature.structural_types,
        )?;
        if !crate::structural_inputs::structural_reference_input::runtime_indices_match(
            indices,
            &geometry.elements,
        ) {
            return None;
        }
        if let Some(offset) = geometry.descriptor_offset {
            Some(Self::Descriptor(offset))
        } else {
            geometry.capacity?;
            Some(Self::Inline(geometry.length_offset.checked_add(8)?))
        }
    }

    pub(in crate::selection) const fn offset(self) -> u32 {
        match self {
            Self::Inline(offset) | Self::Descriptor(offset) => offset,
        }
    }
}

/// Current place custody is independent of field geometry and source syntax.
pub(in crate::selection) fn root_type(
    function: &legalized_operations::LegalizedScalarFunction,
    source: &terminal_psi::StructuralArgument,
) -> Option<semantic_vocabulary::StructuralTypeId> {
    use terminal_psi::{StructuralAccess, StructuralMultiplicity};
    if let Some(parameter) = super::primitive_local_input::read_parameter(function, source.place) {
        if !access_covers(parameter.access, source.access)
            || parameter.multiplicity == StructuralMultiplicity::Linear
            || !parameter.qualifications.is_empty()
            || !parameter.projected_qualifications.is_empty()
        {
            return None;
        }
        return Some(parameter.structural_type);
    }
    if let Some(structural_type) =
        super::established_view_input::element_view_type(function, source.place)
    {
        return access_covers(
            super::established_view_input::view_access(function, source.place)?,
            source.access,
        )
        .then_some(structural_type);
    }
    let (_, result) = super::aggregate_result_input::home(function, source.place)?;
    (access_covers(StructuralAccess::Owned, source.access)
        && result.multiplicity != StructuralMultiplicity::Linear
        && result.qualifications.is_empty()
        && result.projected_qualifications.is_empty()
        && result.claims.is_empty())
    .then_some(result.structural_type)
}

/// Reading may narrow owned or mutable custody to a shared observation. It
/// cannot turn a descriptor's ownership into exclusive backing permission.
fn access_covers(
    available: terminal_psi::StructuralAccess,
    requested: terminal_psi::StructuralAccess,
) -> bool {
    use terminal_psi::StructuralAccess;
    available == requested
        || requested == StructuralAccess::SharedBorrow
            && matches!(
                available,
                StructuralAccess::Owned | StructuralAccess::MutableBorrow
            )
}
