//! Moving execution custody copies an address, not the stack behind it.
//! The native program supplies these homes only for its checked activation
//! declarations; other opaque values have no representation here.

use semantic_vocabulary::StructuralTypeId;
use target::NativeTarget;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExecutionOwnerHome {
    ty: StructuralTypeId,
    bytes: u32,
    alignment: u16,
}

impl ExecutionOwnerHome {
    pub fn for_target(ty: StructuralTypeId, target: NativeTarget) -> Option<Self> {
        let bytes = u32::try_from(target.pointer_size).ok()?;
        let alignment = u16::try_from(target.pointer_alignment).ok()?;
        (bytes != 0 && alignment.is_power_of_two() && bytes.is_multiple_of(u32::from(alignment)))
            .then_some(Self { ty, bytes, alignment })
    }

    pub const fn ty(self) -> StructuralTypeId { self.ty }
    pub const fn bytes(self) -> u32 { self.bytes }
    pub const fn alignment(self) -> u16 { self.alignment }
}

/// Both slices belong to the same target-bound structural declarations.
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeValueStorage<'a> {
    pub sequences: &'a [(StructuralTypeId, u64)],
    pub execution_owners: &'a [ExecutionOwnerHome],
}
