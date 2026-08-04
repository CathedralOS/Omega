#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SymbolKind {
    #[default]
    Unknown,
    Root,
    Module,
    BuiltinType,
    BuiltinFunction,
    Invariant,
    Data,
    Domain,
    Field,
    Variant,
    Machine,
    Operator,
    Proposition,
    State,
    Trait,
    Conformance,
    WireSchema,
    Parameter,
    TypeParameter,
    /// A compile-time machine-symbol parameter. Unlike a type parameter this
    /// symbol is callable inside its generic machine through its authored
    /// signature contract.
    MachineParameter,
    /// An opaque proof-static machine identity used as a proposition-family
    /// index. It is deliberately not callable and owns no signature children.
    PropositionMachineParameter,
    Local,
    HostCapability,
    Object,
    Function,
    Section,
    Import,
}
