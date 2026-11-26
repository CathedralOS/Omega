#[derive(Debug, Clone, Copy)]
pub enum TerminalMachineSelection<'a> {
    Declarations,
    Name(&'a str),
    Symbol(symbols::SymbolHandle),
}

#[derive(Clone, Copy, Default)]
pub struct TerminalProgramRoots<'a> {
    pub machines: &'a [symbols::SymbolHandle],
    pub boundaries: &'a [symbols::SymbolHandle],
}
