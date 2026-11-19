//! Hosted entry operations and their symbolic destinations.
use calling_conventions::{MachineRegister as R, StateEffectsEvidence};
use machine_code::hosted_entry::{EntrySymbol, HostedEntryMode};
use target::NativeTarget;
pub type HostedEntryProgram<Origin> = crate::PhysicalRecipe<Origin, HostedEntryInstructions>;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86Reference {
    StoreRsp,
    StoreRax,
    LoadRsp,
    LoadRcx,
    Address(R),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureCondition {
    Zero,
    NotEqual,
    AboveOrEqual,
}

// Every variant denotes a fixed instruction sequence. Operands that vary with
// the program are separate operation fields, never partially encoded buffers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86Template {
    AlignStack,
    ObserverFrame,
    EnvironmentArgument,
    OpenFileArguments,
    FifthPointer,
    ObserverStack,
    TestEax,
    CheckEnvironmentExtent,
    CheckFileHandle,
    WriteArguments,
    ReturnZero,
    Return,
    LinuxExitZero,
    LinuxUnitExitZero,
    LinuxWriteStart,
    LinuxWriteCheck,
    ObserverSuccess,
    ObserverFailure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum A64Template {
    SaveStackLink,
    SwitchStack,
    LoadFd,
    DarwinWrite,
    LinuxWrite,
    CheckWrite,
    DarwinFailure,
    LinuxFailure,
    DarwinComplete,
    LinuxComplete,
    SaveLoaderFrame,
    RestoreLoaderFrame,
    ReturnZero,
    LinuxExitZero,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operation {
    X86Fixed(X86Template),
    X86Reference {
        operation: X86Reference,
        destination: EntrySymbol,
        addend: i64,
    },
    X86Initialize {
        destination: EntrySymbol,
        contents: Vec<u8>,
    },
    X86Stack {
        bytes: u8,
        release: bool,
    },
    X86StackConstant {
        offset: u8,
        value: u32,
    },
    X86WriteLength {
        bytes: u32,
        windows: bool,
    },
    X86CompareLength {
        bytes: u32,
        windows: bool,
    },
    X86Failure(FailureCondition),
    FailureLabel,
    A64Fixed(A64Template),
    A64Address {
        register: u8,
        destination: EntrySymbol,
    },
    A64Initialize {
        destination: EntrySymbol,
        contents: Vec<u8>,
    },
    A64Count {
        register: u8,
        value: u32,
    },
    Call(EntrySymbol),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostedEntryInstructions {
    pub target: NativeTarget,
    pub mode: HostedEntryMode,
    pub operations: Vec<Operation>,
    pub imports: Vec<target::NormalizedForeignLocator>,
    pub effects: StateEffectsEvidence,
    pub call_overhead: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostedEntryContract {
    pub target: NativeTarget,
    pub mode: HostedEntryMode,
    pub imports: Vec<target::NormalizedForeignLocator>,
    pub effects: StateEffectsEvidence,
    pub call_overhead: u64,
}
impl<Origin> HostedEntryProgram<Origin> {
    pub fn into_parts(self) -> (HostedEntryContract, Vec<Operation>) {
        (
            HostedEntryContract {
                target: self.instructions.target,
                mode: self.instructions.mode,
                imports: self.instructions.imports,
                effects: self.instructions.effects,
                call_overhead: self.instructions.call_overhead,
            },
            self.instructions.operations,
        )
    }
}
impl<Origin> HostedEntryProgram<Origin> {
    pub fn imports(&self) -> &[target::NormalizedForeignLocator] {
        &self.instructions.imports
    }
    pub fn effects(&self) -> &StateEffectsEvidence {
        &self.instructions.effects
    }
    pub fn call_overhead(&self) -> u64 {
        self.instructions.call_overhead
    }
}
