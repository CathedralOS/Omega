use omega_control_flow::StateKey;
use omega_state_calls::{StateCallLowering, StateCallRole};
use omega_state_storage::{StateMutationKind, StateMutationLowering};
use psi_arena::{Arena, HandleSpan, PagedArena};
use psi_checked_trees::expression::{ExpressionHandle, ExpressionTable};
use psi_checked_trees::name::Identifier;
use psi_checked_trees::types::{TypeReferenceHandle, TypeReferenceTable};
use psi_symbols::SymbolHandle;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeDispatchBodyPlan {
    pub bodies: Arena<RuntimeDispatchBody>,
    pub expressions: ExpressionTable,
    pub invariant_names: Arena<Identifier>,
    pub operations: PagedArena<RuntimeDispatchBodyOperation>,
    pub type_references: TypeReferenceTable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeDispatchBody {
    pub key: StateKey,
    pub dispatch_index: u32,
    pub operations: HandleSpan<RuntimeDispatchBodyOperation>,
}

impl Default for RuntimeDispatchBody {
    fn default() -> Self {
        Self {
            key: StateKey::default(),
            dispatch_index: 0,
            operations: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeDispatchBodyOperation {
    pub source_key: StateKey,
    pub statement_index: usize,
    pub kind: RuntimeDispatchBodyOperationKind,
}

impl Default for RuntimeDispatchBodyOperation {
    fn default() -> Self {
        Self {
            source_key: StateKey::default(),
            statement_index: 0,
            kind: RuntimeDispatchBodyOperationKind::Other,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum RuntimeDispatchBodyOperationKind {
    HostCall {
        call_ordinal: usize,
    },
    /// An `asm { hlt }` statement: a raw MachineHalt instruction, not a state
    /// transition or host call. See the privileged_effects_and_binary_trust
    /// brief and the parallel `HostCall` handling.
    MachineHalt,
    /// An x86 memory-ordering fence. The shared kind survives until encoding
    /// so target gating and opcode selection use the catalog distinction.
    MemoryFence(psi_language_core::inline_assembly::AsmFenceKind),
    /// x86 CLI/STI interrupt-flag control. The kind retains the catalog's
    /// disable versus delayed-enable distinction through selection.
    InterruptControl(psi_language_core::inline_assembly::AsmInterruptControlKind),
    /// Compiler-balanced RFLAGS snapshot into a u64 destination place.
    FlagsSnapshot,
    /// Compiler-balanced RFLAGS restore from a u64 source place.
    FlagsRestore,
    /// Structured x86 RDMSR into a u64 destination place.
    MsrRead,
    /// Structured x86 WRMSR from a u32 index and u64 value.
    MsrWrite,
    ControlRegisterRead(psi_language_core::inline_assembly::AsmControlRegister),
    ControlRegisterWrite(psi_language_core::inline_assembly::AsmControlRegister),
    /// An `asm { out <port>, <value> }` statement (a Call to `asm#port_out`):
    /// a raw port write, operands resolved at selection.
    PortWrite,
    /// An `asm { in <dest>, <port> }` statement (an Assignment whose value is a
    /// Call to `asm#port_in`): a raw port read into a destination place.
    PortRead,
    InlineLeafStateCall {
        role: StateCallRole,
        call_ordinal: usize,
        target_key: StateKey,
        argument_count: usize,
    },
    InlineStateCall {
        role: StateCallRole,
        call_ordinal: usize,
        target_key: StateKey,
        argument_count: usize,
        lowering: StateCallLowering,
    },
    StateCall {
        role: StateCallRole,
        call_ordinal: usize,
        target_key: StateKey,
        argument_count: usize,
        lowering: StateCallLowering,
    },
    LocalStorage {
        symbol: SymbolHandle,
        name: Identifier,
        type_symbol: SymbolHandle,
        type_reference: TypeReferenceHandle,
        invariant_names: HandleSpan<Identifier>,
    },
    Mutation {
        mutation_kind: StateMutationKind,
        lowering: StateMutationLowering,
    },
    StateCallResult {
        role: StateCallRole,
        call_ordinal: usize,
        target_key: StateKey,
        value: ExpressionHandle,
    },
    #[default]
    Other,
}
