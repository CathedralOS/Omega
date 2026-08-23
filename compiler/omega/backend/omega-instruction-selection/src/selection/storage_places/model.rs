use omega_abstract_operations::RuntimeStorageRegion;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::selection) struct RuntimeStoragePlace {
    pub(in crate::selection) region: RuntimeStorageRegion,
    pub(in crate::selection) byte_offset: usize,
    pub(in crate::selection) byte_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::selection) struct RuntimeBitFieldPlace {
    pub(in crate::selection) region: RuntimeStorageRegion,
    pub(in crate::selection) base_byte_offset: usize,
    pub(in crate::selection) value_byte_count: usize,
    pub(in crate::selection) fragments: Vec<omega_layout::BitFieldFragment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::selection) enum RuntimeStoredIntegerSource {
    Direct {
        region: RuntimeStorageRegion,
        byte_offset: usize,
    },
    Pointee {
        pointer_byte_offset: usize,
        field_byte_offset: usize,
    },
    FrameIndexed {
        descriptor_offset: usize,
        index_region: RuntimeStorageRegion,
        index_offset: usize,
        index_byte_size: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
    },
    FrameBaseIndexed {
        base_byte_offset: usize,
        index_offset: usize,
        index_byte_size: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
    },
    MachineIndexed {
        base_byte_offset: usize,
        index_region: RuntimeStorageRegion,
        index_offset: usize,
        index_byte_size: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
    },
}

/// One `IntegerAt` projection after layout resolution. The source width and
/// interpretation describe the physical integer; the carrier fields describe
/// the portable semantic value produced by extension.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::selection) struct RuntimeStoredIntegerProjection {
    pub(in crate::selection) source: RuntimeStoredIntegerSource,
    pub(in crate::selection) stored_byte_count: usize,
    pub(in crate::selection) carrier_byte_count: usize,
    pub(in crate::selection) interpretation: psi_layout_plans::IntegerInterpretation,
    pub(in crate::selection) carrier_signed: bool,
    pub(in crate::selection) write_is_total: bool,
}

impl RuntimeStoredIntegerSource {
    pub(in crate::selection) fn as_place(&self) -> Option<omega_abstract_operations::Place> {
        use omega_abstract_operations::{Place, PlaceStep};
        match *self {
            Self::Direct {
                region,
                byte_offset,
            } => Some(Place::at(region, byte_offset)),
            Self::Pointee {
                pointer_byte_offset,
                field_byte_offset,
            } => Place::at(RuntimeStorageRegion::RuntimeFrame, pointer_byte_offset)
                .with_step(PlaceStep::Deref)?
                .with_step(PlaceStep::ConstOffset(field_byte_offset)),
            Self::FrameIndexed {
                descriptor_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            } => Place::at(RuntimeStorageRegion::RuntimeFrame, descriptor_offset)
                .with_step(PlaceStep::Deref)?
                .with_step(PlaceStep::ScaledIndex {
                    index_region,
                    index_offset,
                    index_byte_size,
                    element_byte_size,
                })?
                .with_step(PlaceStep::ConstOffset(field_byte_offset)),
            Self::FrameBaseIndexed {
                base_byte_offset,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            } => Place::at(RuntimeStorageRegion::RuntimeFrame, base_byte_offset)
                .with_step(PlaceStep::ScaledIndex {
                    index_region: RuntimeStorageRegion::RuntimeFrame,
                    index_offset,
                    index_byte_size,
                    element_byte_size,
                })?
                .with_step(PlaceStep::ConstOffset(field_byte_offset)),
            Self::MachineIndexed {
                base_byte_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            } => Place::at(RuntimeStorageRegion::Machine, base_byte_offset)
                .with_step(PlaceStep::ScaledIndex {
                    index_region,
                    index_offset,
                    index_byte_size,
                    element_byte_size,
                })?
                .with_step(PlaceStep::ConstOffset(field_byte_offset)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::selection) struct RuntimeFrameIndexedTarget {
    pub(in crate::selection) descriptor_offset: usize,
    pub(in crate::selection) index_region: RuntimeStorageRegion,
    pub(in crate::selection) index_offset: usize,
    pub(in crate::selection) index_byte_size: usize,
    pub(in crate::selection) element_byte_size: usize,
    pub(in crate::selection) field_byte_offset: usize,
    pub(in crate::selection) byte_count: usize,
    pub(in crate::selection) is_bounded_byte_buffer: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::selection) struct RuntimeFrameBaseIndexedTarget {
    pub(in crate::selection) base_byte_offset: usize,
    pub(in crate::selection) index_region: RuntimeStorageRegion,
    pub(in crate::selection) index_offset: usize,
    pub(in crate::selection) index_byte_size: usize,
    pub(in crate::selection) element_byte_size: usize,
    pub(in crate::selection) field_byte_offset: usize,
    pub(in crate::selection) byte_count: usize,
    pub(in crate::selection) is_bounded_byte_buffer: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::selection) struct RuntimeFrameFixedIndexedTarget {
    pub(in crate::selection) descriptor_offset: usize,
    pub(in crate::selection) element_index: usize,
    pub(in crate::selection) element_byte_size: usize,
    pub(in crate::selection) field_byte_offset: usize,
    pub(in crate::selection) byte_count: usize,
}

impl RuntimeFrameFixedIndexedTarget {
    pub(in crate::selection) fn pointee_field_byte_offset(&self) -> Option<usize> {
        self.element_index
            .checked_mul(self.element_byte_size)?
            .checked_add(self.field_byte_offset)
    }
}
