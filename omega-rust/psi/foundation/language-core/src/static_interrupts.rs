//! Keep installation, context access and acknowledgement completion distinct.
//! These operation names grant no installed root or borrowed backing by themselves.

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub enum StaticInterruptOperation {
    Install,
    Context,
    BeginCompletion,
    FinishCompletion,
}

/// The compiler implements these declarations' contracts. Matching their names
/// alone never changes an authored boundary into a compiler operation.
pub const CORE_DECLARATIONS: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../source/library/core/static_interrupt.omg"
));

impl StaticInterruptOperation {
    pub const ALL: [Self; 4] = [
        Self::Install,
        Self::Context,
        Self::BeginCompletion,
        Self::FinishCompletion,
    ];

    /// Installing handlers changes CPU state and requires MachineControl.
    /// Psi checks this service requirement before native lowering.
    pub const fn required_service_identity(self) -> Option<&'static str> {
        match self {
            Self::Install => Some("toolchain::MachineControl"),
            Self::Context | Self::BeginCompletion | Self::FinishCompletion => None,
        }
    }

    pub const fn code(self) -> u8 {
        match self {
            Self::Install => 0,
            Self::Context => 1,
            Self::BeginCompletion => 2,
            Self::FinishCompletion => 3,
        }
    }

    pub fn from_core_declaration(name: &str, source: &[u8]) -> Option<Self> {
        if source != CORE_DECLARATIONS {
            return None;
        }
        match name {
            "X86StaticInterruptScope::run" => Some(Self::Install),
            "view_static_interrupt_context" => Some(Self::Context),
            "begin_static_interrupt_completion" => Some(Self::BeginCompletion),
            "finish_static_interrupt_completion" => Some(Self::FinishCompletion),
            _ => None,
        }
    }
}
