#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MachineInstruction {
    pub selected_instruction_index: u32,
    pub kind: MachineInstructionKind,
}

impl Default for MachineInstruction {
    fn default() -> Self {
        Self {
            selected_instruction_index: 0,
            kind: MachineInstructionKind::NoOp,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachineInstructionKind {
    NoOp,
    DispatchLoopEnter,
    DispatchCaseEnter,
    DispatchGuardCompareStatic,
    RuntimeTextLiteralCompare,
    RuntimeTextStorageCompare,
    RuntimeStorageCompare,
    RuntimeStorageValueCompare,
    RuntimeTextLiteralWrite,
    RuntimeTextLiteralSegmentWrite,
    RuntimeTextStoredSuffixAppend,
    RuntimeTextBufferMaterialize,
    RuntimeTextBufferMaterializeToRuntimePointee,
    RuntimeTextBufferMaterializeToRuntimeFrameIndexed,
    RuntimeTextStoredPlaceAppend,
    RuntimeTextStoredPlaceAppendToRuntimePointee,
    RuntimeTextStoredPlaceAppendToRuntimeFrameIndexed,
    RuntimeTextLiteralAppend,
    RuntimeTextLiteralAppendToRuntimePointee,
    RuntimeTextLiteralAppendToRuntimeFrameIndexed,
    RuntimeMachineIntegerWrite,
    RuntimeStorageBitFieldWrite,
    RuntimePointeeIntegerWrite,
    RuntimeStorageBinaryWrite,
    RuntimeStorageConvert,
    AtomicLoad,
    AtomicStore,
    AtomicFetchAdd,
    AtomicFetchSub,
    AtomicFetchXor,
    AtomicFetchOr,
    AtomicFetchAnd,
    AtomicSwap,
    AtomicCompareExchange,
    RuntimePointeeBinaryWrite,
    RuntimeFrameIndexedIntegerWrite,
    RuntimeFrameBaseIndexedIntegerWrite,
    RuntimeMachineIndexedIntegerWrite,
    WireLiteralByteAppend,
    WireScalarVarintAppend,
    WireTextBytesAppend,
    WireScalarSliceAppend,
    WireExpectedByteRead,
    WireScalarVarintRead,
    WireByteSliceRead,
    WireNestedOpenRead,
    WireNestedCloseRead,
    WireRepeatedScalarVarintAppend,
    WireRepeatedScalarVarintRead,
    RuntimeFrameIndexedBinaryWrite,
    RuntimeFrameBaseIndexedBinaryWrite,
    RuntimeMachineIndexedBinaryWrite,
    RuntimeMachineStringWrite,
    RuntimeMachineBoundedBufferWrite,
    RuntimeMachineBoundedBufferSourceAppend,
    RuntimeMachineBoundedBufferLiteralAppend,
    RuntimePointeeBoundedBufferWrite,
    RuntimeFrameStringWrite,
    RuntimePointeeStringWrite,
    RuntimeFrameIndexedStringWrite,
    RuntimeMachineIndexedStringWrite,
    RuntimeStorageAddressToRuntimeFrameWrite,
    DataAddressToRuntimeFrameWrite,
    RuntimePointeeAddressToRuntimeFrameWrite,
    RuntimeFrameIndexedAddressToRuntimeFrameWrite,
    RuntimeFrameFixedIndexedAddressToRuntimeFrameWrite,
    RuntimeFrameBaseIndexedAddressToRuntimeFrameWrite,
    RuntimeMachineIndexedAddressToRuntimeFrameWrite,
    RuntimeTextLineRead,
    RuntimeByteRead,
    RuntimeByteWrite,
    RuntimeStorageCopy,
    RuntimeStorageCopyToRuntimeFrameIndexed,
    RuntimeStorageCopyFromRuntimeFrameIndexed,
    RuntimeStorageCopyFromRuntimeFrameFixedIndexed,
    RuntimeStorageCopyFromRuntimeFrameFixedIndexedToRuntimePointee,
    RuntimeStorageCopyFromRuntimeFrameIndexedToRuntimePointee,
    RuntimeStorageCopyFromRuntimeMachineIndexed,
    RuntimeStorageCopyFromRuntimeMachineDoubleIndexed,
    RuntimeStorageCopyToRuntimeMachineDoubleIndexed,
    RuntimeMachineDoubleIndexedIntegerWrite,
    RuntimeMachineDoubleIndexedBinaryWrite,
    RuntimeStorageCopyFromRuntimeFrameBaseDoubleIndexed,
    RuntimeStorageCopyFromRuntimeFrameBaseIndexed,
    RuntimeStorageCopyToRuntimeMachineIndexed,
    RuntimeStorageCopyMachineIndexedToMachineIndexed,
    RuntimeStorageCopyToRuntimePointee,
    RuntimeStorageCopyFromRuntimePointeeToRuntimeFrame,
    DispatchStateWrite,
    ReturnRegisterIntegerWrite,
    RuntimeStorageCopyToReturnRegister,
    /// Entry prologue: store an incoming argument register into the entry
    /// parameter's frame slot (the calling plan's inbound direction).
    EntryArgumentRegisterWrite,
    /// Entry prologue: copy an incoming stack-argument fragment into the
    /// parameter's frame slot (the calling plan's inbound direction).
    EntryStackArgumentWrite,
    /// Entry prologue: copy an indirectly passed aggregate into its frame slot.
    EntryIndirectArgumentWrite,
    /// Entry prologue: bind `args: &[u8]` as a slice descriptor over the
    /// entry-argument spill.
    EntryArgumentsSliceDescriptorWrite,
    DispatchTerminate,
    DispatchCaseLeave,
    /// Direct call whose target is retained by the selected operation and
    /// resolved through compiler-private function identity at relocation.
    InternalFunctionCall,
    /// Compiler-private `lea reg, [rsp+disp32]` caller-frame address recipe.
    OutgoingStackAddressLoad,
    OutgoingStackFrameReserve,
    OutgoingStackU64Write,
    EntryIndirectU64ToOutgoingStackCopy,
    OutgoingStackFrameRelease,
    HostCallSequence,
    DynamicTableCallSequence,
    /// The x86 `hlt` privileged instruction (`asm { hlt }`). Zero operands,
    /// no relocation. See the privileged_effects_and_binary_trust brief.
    MachineHalt,
    /// An x86 load/store/full memory-ordering fence.
    MemoryFence(psi_language_core::inline_assembly::AsmFenceKind),
    /// x86 CLI/STI interrupt-flag control.
    InterruptControl(psi_language_core::inline_assembly::AsmInterruptControlKind),
    /// Compiler-balanced RFLAGS snapshot.
    FlagsSnapshot,
    /// Compiler-balanced RFLAGS restore.
    FlagsRestore,
    /// Structured x86 RDMSR.
    MsrRead,
    /// Structured x86 WRMSR.
    MsrWrite,
    ControlRegisterRead(psi_language_core::inline_assembly::AsmControlRegister),
    ControlRegisterWrite(psi_language_core::inline_assembly::AsmControlRegister),
    /// The x86 `out dx, al` port write (`asm { out .. }`). Storage operands
    /// relocate like any runtime-value read.
    PortWrite,
    /// The x86 `in al, dx` port read (`asm { in .. }`), storing the byte to a
    /// destination place.
    PortRead,
    Return,
}

impl MachineInstructionKind {
    /// Whether this instruction comes from the user-checked assembly catalog
    /// and therefore must retain independent final-image validation evidence.
    pub const fn requires_checked_assembly_validation(self) -> bool {
        matches!(
            self,
            Self::MachineHalt
                | Self::MemoryFence(_)
                | Self::InterruptControl(_)
                | Self::FlagsSnapshot
                | Self::FlagsRestore
                | Self::MsrRead
                | Self::MsrWrite
                | Self::ControlRegisterRead(_)
                | Self::ControlRegisterWrite(_)
                | Self::PortWrite
                | Self::PortRead
        )
    }
}

#[cfg(test)]
mod tests {
    use super::MachineInstructionKind;
    use psi_language_core::inline_assembly::{
        AsmControlRegister, AsmFenceKind, AsmInterruptControlKind,
    };

    #[test]
    fn checked_catalog_instruction_classes_fail_closed() {
        for kind in [
            MachineInstructionKind::MachineHalt,
            MachineInstructionKind::MemoryFence(AsmFenceKind::Full),
            MachineInstructionKind::InterruptControl(AsmInterruptControlKind::Disable),
            MachineInstructionKind::FlagsSnapshot,
            MachineInstructionKind::FlagsRestore,
            MachineInstructionKind::MsrRead,
            MachineInstructionKind::MsrWrite,
            MachineInstructionKind::ControlRegisterRead(AsmControlRegister::Cr3),
            MachineInstructionKind::ControlRegisterWrite(AsmControlRegister::Cr3),
            MachineInstructionKind::PortWrite,
            MachineInstructionKind::PortRead,
        ] {
            assert!(kind.requires_checked_assembly_validation(), "{kind:?}");
        }

        assert!(!MachineInstructionKind::NoOp.requires_checked_assembly_validation());
    }
}
