//! Executable contracts for instructions over independent scalar working registers.
//! Every write samples its inputs from the pre-instruction state. Flags are
//! Boolean registers. An ISA needing aliased views, memory or environment effects
//! needs those semantics before supplying a contract in this vocabulary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InstructionInput {
    Operand(usize),
    Register(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InstructionValue {
    Copy(InstructionInput),
    IsZero(InstructionInput),
    Boolean(bool),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstructionWrite {
    pub destination: InstructionInput,
    pub value: InstructionValue,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InstructionExit {
    Continue,
    Return,
    Jump {
        target: usize,
    },
    /// The condition reads the incoming state, before this instruction commits any writes.
    Branch {
        condition: InstructionInput,
        target: usize,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstructionSemantics {
    pub writes: Vec<InstructionWrite>,
    pub exit: InstructionExit,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InstructionParameter<T> {
    /// An instruction that admits either register storage or an evaluated scalar.
    Scalar(T),
    Register(T),
    Expression(T),
    LocalBlock,
}
