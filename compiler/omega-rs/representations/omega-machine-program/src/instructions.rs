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
    RuntimePointeeIntegerWrite,
    RuntimeStorageBinaryWrite,
    RuntimeStorageConvert,
    AtomicLoad,
    AtomicStore,
    AtomicFetchAdd,
    AtomicSwap,
    AtomicCompareExchange,
    RuntimePointeeBinaryWrite,
    RuntimeFrameIndexedIntegerWrite,
    RuntimeFrameBaseIndexedIntegerWrite,
    RuntimeMachineIndexedIntegerWrite,
    WireLiteralByteAppend,
    WireScalarVarintAppend,
    WireTextBytesAppend,
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
    HostCallSequence,
    /// The x86 `hlt` privileged instruction (`asm { hlt }`). Zero operands,
    /// no relocation. See the privileged_effects_and_binary_trust brief.
    MachineHalt,
    /// An x86 load/store/full memory-ordering fence.
    MemoryFence(omega_core::inline_assembly::AsmFenceKind),
    /// x86 CLI/STI interrupt-flag control.
    InterruptControl(omega_core::inline_assembly::AsmInterruptControlKind),
    /// Compiler-balanced RFLAGS snapshot.
    FlagsSnapshot,
    /// Compiler-balanced RFLAGS restore.
    FlagsRestore,
    /// Structured x86 RDMSR.
    MsrRead,
    /// Structured x86 WRMSR.
    MsrWrite,
    ControlRegisterRead(omega_core::inline_assembly::AsmControlRegister),
    ControlRegisterWrite(omega_core::inline_assembly::AsmControlRegister),
    /// The x86 `out dx, al` port write (`asm { out .. }`). Storage operands
    /// relocate like any runtime-value read.
    PortWrite,
    /// The x86 `in al, dx` port read (`asm { in .. }`), storing the byte to a
    /// destination place.
    PortRead,
    Return,
}
