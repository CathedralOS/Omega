#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SymbolKind {
    #[default]
    Unknown,
    Root,
    Module,
    BuiltinType,
    Invariant,
    Data,
    Field,
    Variant,
    Machine,
    State,
    Parameter,
    Local,
    Platform,
    HostCapability,
    Object,
    Function,
    Section,
    Import,
}
