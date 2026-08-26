#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SymbolKind {
    #[default]
    Unknown,
    Root,
    Module,
    BuiltinType,
    BuiltinFunction,
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
    /// A compile-time value declaration retained only as authored-selection
    /// provenance after its value has been substituted.
    Const,
    /// A proof-static machine-telescope binder whose concrete argument is one
    /// exact package-scoped conformance.
    ConformanceParameter,
    WireSchema,
    Parameter,
    TypeParameter,
    /// A compile-time machine-symbol parameter. Unlike a type parameter this
    /// symbol is callable inside its generic machine through its authored
    /// signature contract.
    MachineParameter,
    /// A generic proof-formula family. It is applicable only in proof-fact
    /// position and owns the value-parameter symbols of its authored
    /// proposition signature.
    PropositionParameter,
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
