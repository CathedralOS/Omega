use omega_calling_conventions::{StateFootprintEvidence, ValuePlacement, ValueShape};

/// Target-closed inbound storage row retained by provider preparation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivedBoundaryEntryStorage<Write> {
    pub writes: Vec<Write>,
    pub parameters: Vec<DerivedBoundaryEntryParameterStorage>,
    pub footprint: StateFootprintEvidence,
}

/// Relationship between one semantic parameter and its validated ABI home.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivedBoundaryEntryParameterStorage {
    pub parameter_index: usize,
    pub destination_byte_offset: usize,
    pub shape: ValueShape,
    pub placement: ValuePlacement,
    pub write_range: std::ops::Range<usize>,
}

impl<Write> DerivedBoundaryEntryStorage<Write> {
    pub fn parameter(
        &self,
        parameter_index: usize,
    ) -> Option<&DerivedBoundaryEntryParameterStorage> {
        self.parameters
            .iter()
            .find(|parameter| parameter.parameter_index == parameter_index)
    }
}
