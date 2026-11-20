#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DeclarationKind {
    Const,
    Data,
    Field,
    Case,
    CaseField,
    Domain,
    Trait,
    Conformance,
    Machine,
    AsmMachine,
    MathLet,
}

pub type DeclarationId = omega_program_contracts::DeclarationId<DeclarationKind>;
pub type DeclarationName = omega_program_contracts::DeclarationName<DeclarationKind>;
