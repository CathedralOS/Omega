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
    Local,
    HostCapability,
    Object,
    Function,
    Section,
    Import,
}
