//! Where an indexed field byte read finds its bytes. A bounded owner keeps
//! them inline after its live-length word; a shared view field keeps a
//! two-word descriptor whose first word points at backing outside the root.
//! Construction and replay share this geometry so neither re-derives it.

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
    ) -> Option<Self> {
        use terminal_psi::{StructuralAccess, StructuralMultiplicity};
        let signature = function.structural.as_ref()?;
        if source.access == StructuralAccess::WriteOnlyBorrow {
            return None;
        }
        let structural_type = if let Some(parameter) =
            super::primitive_local_input::read_parameter(function, source.place)
        {
            if parameter.access != source.access
                || parameter.multiplicity == StructuralMultiplicity::Linear
                || !parameter.qualifications.is_empty()
                || !parameter.projected_qualifications.is_empty()
            {
                return None;
            }
            parameter.structural_type
        } else {
            let (_, result) = super::aggregate_result_input::home(function, source.place)?;
            if source.access != StructuralAccess::Owned
                || result.multiplicity == StructuralMultiplicity::Linear
                || !result.qualifications.is_empty()
                || !result.projected_qualifications.is_empty()
                || !result.claims.is_empty()
            {
                return None;
            }
            result.structural_type
        };
        if let Some((offset, _)) =
            crate::structural_inputs::structural_reference_input::observed_byte_field_storage(
                structural_type,
                &source.path,
                field,
                &signature.structural_types,
            )
        {
            return Some(Self::Inline(offset.checked_add(8)?));
        }
        crate::structural_inputs::structural_reference_input::observed_byte_view_field_descriptor(
            structural_type,
            &source.path,
            field,
            &signature.structural_types,
        )
        .map(Self::Descriptor)
    }

    pub(in crate::selection) const fn offset(self) -> u32 {
        match self {
            Self::Inline(offset) | Self::Descriptor(offset) => offset,
        }
    }
}
