//! Executable contracts for selected target instructions over named scalar
//! locations. The catalog declares each location's role and lifetime
//! independently. Every write samples its inputs from the pre-instruction state.
//! Flags are Boolean locations.
//!
//! This vocabulary cannot state overlapping register views, memory accesses,
//! ISA barrier ordering, cache/TLB/DMA effects or hardware service reach. Each
//! declared location is independent of every other. A catalog whose instructions
//! need any of those effects needs them added first.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InstructionInput {
    Operand(usize),
    /// A location declared by the selected catalog. Its role and lifetime come from
    /// that declaration, not from this reference.
    Location(String),
}

/// Whether a catalog location is a register (`wiki/spec/language/assembly.md`,
/// "Contracts and clobbers"). A write to a register, including an implicit flag
/// effect, is one of the instruction's changed registers. Source code can name a
/// register as an operand, in `registers` or in `clobbers`. Other target state,
/// such as abstract device state, is not a register and has no source name.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum LocationRole {
    Register,
    State,
}

/// How long a catalog location's value lasts (`wiki/spec/language/assembly.md`,
/// "Assembly state and explicit communication"). A region location, such as a
/// working register or scratch flag, starts each assembly region unsupplied and
/// ends with it. A persistent location, such as a system register, an
/// architectural control flag or device state, belongs to the selected execution
/// environment and keeps its value across regions, labels and bodies.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum LocationLifetime {
    Region,
    Persistent,
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
