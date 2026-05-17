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
    Field,
    Variant,
    Machine,
    State,
    Parameter,
    TypeParameter,
    Local,
    Platform,
    HostCapability,
    Object,
    Function,
    Section,
    Import,
}
