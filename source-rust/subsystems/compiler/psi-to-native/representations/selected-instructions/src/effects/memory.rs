//! The memory part of a selected instruction's encoded effects has one wire
//! definition for catalog identities and saved pre-allocation plans.
use super::catalog::MachineIdentityBytes;
use super::program::encoding::{Cursor, PreAllocationMachineEffectDecodeError as DecodeError};
use crate::AtomicForm;
use register_model::RegisterViewId;

trait Field: Sized {
    fn encode(self, bytes: &mut (impl MachineIdentityBytes + ?Sized));
    fn decode(cursor: &mut Cursor<'_>) -> Result<Self, DecodeError>;
}

impl Field for u16 {
    fn encode(self, bytes: &mut (impl MachineIdentityBytes + ?Sized)) {
        bytes.identity_bytes(&self.to_le_bytes());
    }
    fn decode(cursor: &mut Cursor<'_>) -> Result<Self, DecodeError> {
        cursor.u16()
    }
}

impl Field for u32 {
    fn encode(self, bytes: &mut (impl MachineIdentityBytes + ?Sized)) {
        bytes.identity_bytes(&self.to_le_bytes());
    }
    fn decode(cursor: &mut Cursor<'_>) -> Result<Self, DecodeError> {
        cursor.u32()
    }
}

impl Field for RegisterViewId {
    fn encode(self, bytes: &mut (impl MachineIdentityBytes + ?Sized)) {
        self.0.encode(bytes);
    }
    fn decode(cursor: &mut Cursor<'_>) -> Result<Self, DecodeError> {
        cursor.u16().map(Self)
    }
}

impl Field for AtomicForm {
    fn encode(self, bytes: &mut (impl MachineIdentityBytes + ?Sized)) {
        bytes.identity_bytes(&[self.ordinal()]);
    }
    fn decode(cursor: &mut Cursor<'_>) -> Result<Self, DecodeError> {
        Self::from_ordinal(cursor.byte()?).ok_or(DecodeError::InvalidField)
    }
}

// Explicit Rust discriminants make a duplicate wire tag a compilation error.
// A row's fields appear in exactly the order in which both codecs visit them.
macro_rules! memory_effects {
    ($($tag:literal => $effect:ident $( { $($field:ident: $ty:ty),+ $(,)? } )?),+ $(,)?) => {
        #[repr(u8)]
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum MachineEncodedMemoryEffect {
            $($effect $( { $($field: $ty),+ } )? = $tag,)+
        }

        impl MachineEncodedMemoryEffect {
            pub(super) fn encode(self, bytes: &mut (impl MachineIdentityBytes + ?Sized)) {
                match self {
                    $(Self::$effect $( { $($field),+ } )? => {
                        bytes.identity_bytes(&[$tag]);
                        $($( $field.encode(bytes); )+)?
                    },)+
                }
            }

            pub(super) fn decode(cursor: &mut Cursor<'_>) -> Result<Self, DecodeError> {
                Ok(match cursor.byte()? {
                    $($tag => Self::$effect $( { $($field: <$ty as Field>::decode(cursor)?),+ } )?,)+
                    _ => return Err(DecodeError::InvalidField),
                })
            }
        }
    };
}

memory_effects! {
    0 => NoneV1,
    1 => ReadActivationStackV1 { stack_pointer: RegisterViewId, byte_count: u16 },
    2 => WriteReturnAddressBelowStackPointerV1 { stack_pointer: RegisterViewId, byte_count: u16 },
    3 => ReadPointerV1 { pointer_operand: u16, byte_count: u16 },
    4 => WriteFrameStorageV1 { stack_pointer: RegisterViewId, byte_count: u16 },
    5 => ReadIndexedPointerV1 { pointer_operand: u16, index_operand: u16, byte_count: u16 },
    6 => HostedWriteByteV1 { stack_pointer: RegisterViewId },
    7 => WritePointerV1 { pointer_operand: u16 },
    8 => HostedReadByteV1 { stack_pointer: RegisterViewId },
    9 => CopyBytesV1 {
        source_pointer_operand: u16,
        destination_pointer_operand: u16,
        count_operand: u16,
    },
    10 => ReadFrameStorageV1 { stack_pointer: RegisterViewId, byte_count: u16 },
    11 => AtomicPointerV1 { pointer_operand: u16, form: AtomicForm },
    12 => TemporaryStackStorageV1 { stack_pointer: RegisterViewId, byte_count: u16 },
    13 => ScanByteSequenceV1 { base_pointer_operand: u16, count_operand: u16 },
    14 => CompareByteSequencesV1 {
        left_base_operand: u16,
        left_count_operand: u16,
        right_base_operand: u16,
        right_count_operand: u16,
    },
    15 => ZeroBytesV1 { destination_pointer_operand: u16, count_operand: u16 },
    16 => ReadEncodedScalarV1 { pointer_operand: u16, codec_operand: u16 },
    17 => WriteEncodedScalarV1 { pointer_operand: u16, codec_operand: u16 },
    18 => AtomicFenceV1,
    19 => ReadTableFieldV1 { pointer_operand: u16, byte_offset: u32 },
    20 => PrivateCallbackMaterializationV1,
    21 => WriteFixedPointerV1 { pointer_operand: u16, byte_count: u16 },
    22 => TaskRegisterActivationV1 {
        table_pointer_operand: u16,
        table_byte_count_operand: u16,
        task_pointer_operand: u16,
        selector_operand: u16,
        scratch_pointer_operand: u16,
        stack_pointer: RegisterViewId,
    },
    23 => SegmentDescriptorReloadV1 {
        table_pointer_operand: u16,
        table_byte_count_operand: u16,
        selector_operand: u16,
        scratch_pointer_operand: u16,
        stack_pointer: RegisterViewId,
    },
    25 => InstalledInterruptCpuCheckV1 {
        table_pointer_operand: u16,
        task_pointer_operand: u16,
        scratch_pointer_operand: u16,
        stack_pointer: RegisterViewId,
    },
    26 => InstalledInterruptTaskCommitV1 {
        table_pointer_operand: u16,
        task_pointer_operand: u16,
        scratch_pointer_operand: u16,
        stack_pointer: RegisterViewId,
    },
    27 => InstalledInterruptTablePublicationV1 {
        table_pointer_operand: u16,
        scratch_pointer_operand: u16,
    },
    28 => CatalogFenceV1,
    29 => ReadScaledIndexV1 { pointer_operand: u16, index_operand: u16, element_bytes: u16 },
}
