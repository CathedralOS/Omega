//! Physical adapter operations and their declared register/state effects.

use calling_conventions::MachineRegister;
use target::Architecture;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdapterInstructions {
    pub architecture: Architecture,
    pub operations: Vec<AdapterOperation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdapterOperation {
    Frame {
        bytes: u32,
        interval: u64,
        touches: u32,
        enter: bool,
    },
    FrameMemory {
        register: MachineRegister,
        offset: u32,
        size: u8,
        load: bool,
        scratch: MachineRegister,
    },
    PointerMemory {
        register: MachineRegister,
        base: MachineRegister,
        offset: u32,
        size: u8,
        load: bool,
    },
    FrameAddress {
        register: MachineRegister,
        offset: u32,
    },
    AlignedFrameAddress {
        register: MachineRegister,
        offset: u32,
        alignment: u16,
        member: u32,
    },
    Constant {
        register: MachineRegister,
        value: u64,
    },
    SignExtend {
        register: MachineRegister,
        bits: u16,
    },
    Call,
    Return,
    InterruptReturn,
    ClearDirection,
    X86PushPop {
        register: u8,
        pop: bool,
    },
    X86AdjustStack {
        amount: u8,
        subtract: bool,
    },
    X86RequireEqual {
        register: MachineRegister,
        value: u8,
    },
    X86IncrementChecked {
        register: MachineRegister,
    },
    X86DescriptorStore {
        offset: u32,
        operation: u8,
    },
    X86CompareEax {
        value: u32,
    },
    X86Require {
        condition: u8,
    },
    X86Stop,
    X86Fixed(X86AdapterTemplate),
    X86Scalar32(Scalar32Operation),
}

/// Physical operands are settled before encoding, including the 32-bit width.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scalar32Operation {
    Move {
        destination: MachineRegister,
        source: MachineRegister,
    },
    Or {
        destination: MachineRegister,
        source: MachineRegister,
    },
    And {
        register: MachineRegister,
        value: u32,
    },
    ShiftLeft {
        register: MachineRegister,
        bits: u8,
    },
    Compare {
        register: MachineRegister,
        value: u32,
    },
    Test {
        register: MachineRegister,
        value: u32,
    },
    ByteSwap {
        register: MachineRegister,
    },
}

// Each fixed template is a declared, operand-free instruction sequence. Variable
// register numbers, offsets and constants use the typed operations above.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86AdapterTemplate {
    AlignStack,
    MaskAndTestIst,
    ShiftHighGateAddress,
    MergeMiddleGateAddress,
    MergeLowGateAddress,
    ReadTaskSelector,
    TestSelectorAlignment,
    TestSelectorNonzero,
    SelectorLimit,
    CompareDescriptorLimit,
    AddDescriptorBase,
    HighDescriptorBase,
    MergeDescriptorBase,
    FinishDescriptorBase,
    LoadIstTop,
    LoadIstRoot,
    CompareActiveRoot,
    CompareProvider,
}

impl AdapterInstructions {
    pub fn new(architecture: Architecture) -> Self {
        Self {
            architecture,
            operations: Vec::new(),
        }
    }

    pub fn push(&mut self, operation: AdapterOperation) {
        self.operations.push(operation);
    }

    pub fn frame(&mut self, bytes: u32, interval: u64, touches: u32, enter: bool) {
        self.push(AdapterOperation::Frame {
            bytes,
            interval,
            touches,
            enter,
        });
    }
}
#[derive(Default)]
struct Effects {
    reads: Vec<MachineRegister>,
    writes: Vec<MachineRegister>,
    used: Vec<calling_conventions::MachineState>,
    clobbered: Vec<calling_conventions::MachineState>,
}

impl Effects {
    fn read(&mut self, register: MachineRegister) {
        self.reads.push(register);
        if register == MachineRegister::X86Rsp {
            self.used
                .push(calling_conventions::MachineState::StackPointer);
        }
    }
    fn write(&mut self, register: MachineRegister) {
        self.writes.push(register);
        if register == MachineRegister::X86Rsp {
            self.clobbered
                .push(calling_conventions::MachineState::StackPointer);
        }
    }
    fn use_state(&mut self, state: calling_conventions::MachineState) {
        self.used.push(state);
    }
    fn clobber(&mut self, state: calling_conventions::MachineState) {
        self.clobbered.push(state);
    }
    fn stack(&mut self, architecture: Architecture, write: bool) {
        self.use_state(calling_conventions::MachineState::StackPointer);
        if architecture == Architecture::X86_64 {
            self.read(MachineRegister::X86Rsp);
        }
        if write {
            self.clobber(calling_conventions::MachineState::StackPointer);
            if architecture == Architecture::X86_64 {
                self.write(MachineRegister::X86Rsp);
            }
        }
    }
    fn finish(self) -> calling_conventions::StateEffectsEvidence {
        calling_conventions::StateEffectsEvidence::new(
            calling_conventions::RegisterSet::new(self.reads),
            calling_conventions::RegisterSet::new(self.writes),
            calling_conventions::MachineStateSet::new(self.used),
            calling_conventions::MachineStateSet::new(self.clobbered),
        )
    }
}

impl AdapterInstructions {
    pub fn effects(&self) -> calling_conventions::StateEffectsEvidence {
        self.operations.iter().fold(
            calling_conventions::StateEffectsEvidence::empty(),
            |effects, operation| effects.compose(&operation.effects(self.architecture)),
        )
    }
}

// AArch64 can encode a naturally aligned, twelve-bit scaled displacement directly.
// Other byte offsets require an address register before the load or store.
fn needs_address_register(architecture: Architecture, offset: u32, size: u8) -> bool {
    architecture == Architecture::Aarch64
        && (!offset.is_multiple_of(u32::from(size)) || offset > 4095 * u32::from(size))
}

impl AdapterOperation {
    pub fn effects(&self, architecture: Architecture) -> calling_conventions::StateEffectsEvidence {
        use AdapterOperation as Op;
        use MachineRegister as R;
        use calling_conventions::MachineState as State;
        let mut e = Effects::default();
        match *self {
            Op::Frame { bytes, .. } if bytes != 0 => {
                e.stack(architecture, true);
                if architecture == Architecture::X86_64 {
                    e.clobber(State::Flags);
                }
            }
            Op::Frame { .. } => {}
            Op::FrameMemory {
                register,
                offset,
                size,
                load,
                scratch,
            } => {
                e.stack(architecture, false);
                if needs_address_register(architecture, offset, size) {
                    e.write(scratch);
                    e.read(scratch);
                }
                if load {
                    e.write(register);
                } else {
                    e.read(register);
                }
            }
            Op::PointerMemory {
                register,
                base,
                offset,
                size,
                load,
            } => {
                e.read(base);
                if needs_address_register(architecture, offset, size) {
                    e.write(base);
                }
                if load {
                    e.write(register);
                } else {
                    e.read(register);
                }
            }
            Op::FrameAddress { register, offset } => {
                e.stack(architecture, false);
                e.write(register);
                if architecture == Architecture::Aarch64
                    && ((offset >> 12) > 4095 || (offset >= 4096 && offset & 4095 != 0))
                {
                    e.read(register);
                }
            }
            Op::AlignedFrameAddress { register, .. } => {
                e.stack(architecture, false);
                e.write(register);
                e.read(register);
                if architecture == Architecture::X86_64 {
                    e.clobber(State::Flags);
                }
            }
            Op::Constant { register, value } => {
                e.write(register);
                if architecture == Architecture::Aarch64 {
                    // This form emits the low MOVZ followed by one MOVK for
                    // each nonzero upper halfword; each MOVK reads its destination.
                    if value >> 16 != 0 {
                        e.read(register);
                    }
                }
            }
            Op::SignExtend { register, .. } => {
                e.read(register);
                e.write(register);
            }
            Op::Call => {
                if architecture == Architecture::X86_64 {
                    e.stack(architecture, true);
                } else {
                    e.write(R::Aarch64X(30));
                }
                e.clobber(State::InstructionPointer);
            }
            Op::Return => {
                if architecture == Architecture::X86_64 {
                    e.stack(architecture, true);
                } else {
                    e.read(R::Aarch64X(30));
                }
                e.clobber(State::InstructionPointer);
            }
            Op::InterruptReturn => {
                for state in [
                    State::Flags,
                    State::InstructionPointer,
                    State::StackPointer,
                    State::SegmentState,
                ] {
                    e.clobber(state);
                }
            }
            Op::ClearDirection => e.clobber(State::Flags),
            Op::X86PushPop { register, pop } => {
                e.stack(architecture, true);
                let register = [
                    R::X86Rax,
                    R::X86Rcx,
                    R::X86Rdx,
                    R::X86Rbx,
                    R::X86Rsp,
                    R::X86Rbp,
                    R::X86Rsi,
                    R::X86Rdi,
                    R::X86R8,
                    R::X86R9,
                    R::X86R10,
                    R::X86R11,
                    R::X86R12,
                    R::X86R13,
                    R::X86R14,
                    R::X86R15,
                ][usize::from(register)];
                if pop {
                    e.write(register);
                } else {
                    e.read(register);
                }
            }
            Op::X86AdjustStack { .. } => {
                e.stack(architecture, true);
                e.clobber(State::Flags);
            }
            Op::X86RequireEqual { register, .. } => {
                e.read(register);
                e.clobber(State::Flags);
                e.clobber(State::InstructionPointer);
            }
            Op::X86IncrementChecked { register } => {
                e.read(register);
                e.write(register);
                e.clobber(State::Flags);
                e.clobber(State::InstructionPointer);
            }
            Op::X86DescriptorStore { operation, .. } => {
                e.stack(architecture, false);
                e.use_state(State::ControlState);
                if operation >= 2 {
                    e.clobber(State::ControlState);
                }
            }
            Op::X86CompareEax { .. } => {
                e.read(R::X86Rax);
                e.clobber(State::Flags);
            }
            Op::X86Require { .. } | Op::X86Stop => {
                e.clobber(State::Flags);
                e.clobber(State::InstructionPointer);
            }
            Op::X86Fixed(template) => template.effects(&mut e),
            Op::X86Scalar32(operation) => {
                use Scalar32Operation as S;
                match operation {
                    S::Move {
                        destination,
                        source,
                    } => {
                        e.read(source);
                        e.write(destination);
                    }
                    S::Or {
                        destination,
                        source,
                    } => {
                        e.read(source);
                        e.read(destination);
                        e.write(destination);
                        e.clobber(State::Flags);
                    }
                    S::And { register, .. } | S::ShiftLeft { register, .. } => {
                        e.read(register);
                        e.write(register);
                        e.clobber(State::Flags);
                    }
                    S::Compare { register, .. } | S::Test { register, .. } => {
                        e.read(register);
                        e.clobber(State::Flags);
                    }
                    S::ByteSwap { register } => {
                        e.read(register);
                        e.write(register);
                    }
                }
            }
        }
        e.finish()
    }
}

impl X86AdapterTemplate {
    fn effects(self, e: &mut Effects) {
        use MachineRegister as R;
        use calling_conventions::MachineState as State;
        match self {
            Self::AlignStack => {
                e.stack(Architecture::X86_64, true);
                e.clobber(State::Flags);
            }
            Self::MaskAndTestIst => {
                e.read(R::X86Rax);
                e.write(R::X86Rax);
                e.clobber(State::Flags);
            }
            Self::ShiftHighGateAddress | Self::HighDescriptorBase | Self::SelectorLimit => {
                e.read(R::X86Rax);
                e.write(R::X86Rax);
                e.write(R::X86Rcx);
                e.clobber(State::Flags);
            }
            Self::MergeMiddleGateAddress => {
                e.read(R::X86Rax);
                e.read(R::X86Rcx);
                e.write(R::X86Rax);
                e.write(R::X86Rcx);
                e.clobber(State::Flags);
            }
            Self::MergeLowGateAddress => {
                e.read(R::X86Rax);
                e.read(R::X86Rcx);
                e.write(R::X86Rax);
                e.clobber(State::Flags);
            }
            Self::ReadTaskSelector => {
                e.use_state(State::ControlState);
                e.write(R::X86Rax);
                e.read(R::X86Rax);
            }
            Self::TestSelectorAlignment | Self::TestSelectorNonzero | Self::CompareActiveRoot => {
                e.read(R::X86Rax);
                e.clobber(State::Flags);
            }
            Self::CompareDescriptorLimit => {
                e.read(R::X86Rax);
                e.read(R::X86Rdx);
                e.clobber(State::Flags);
            }
            Self::AddDescriptorBase => {
                e.read(R::X86Rcx);
                e.read(R::X86Rdx);
                e.write(R::X86Rdx);
                e.clobber(State::Flags);
            }
            Self::MergeDescriptorBase => {
                e.read(R::X86Rax);
                e.read(R::X86Rcx);
                e.write(R::X86Rcx);
                e.clobber(State::Flags);
            }
            Self::FinishDescriptorBase => {
                e.read(R::X86Rax);
                e.read(R::X86Rcx);
                e.write(R::X86Rax);
                e.write(R::X86Rdx);
                e.clobber(State::Flags);
            }
            Self::LoadIstTop => {
                e.read(R::X86Rdx);
                e.read(R::X86Rcx);
                e.write(R::X86Rdx);
            }
            Self::LoadIstRoot => {
                e.read(R::X86Rdx);
                e.write(R::X86Rdx);
            }
            Self::CompareProvider => {
                e.read(R::X86Rax);
                e.read(R::X86Rcx);
                e.clobber(State::Flags);
            }
        }
    }
}
