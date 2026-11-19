use omega_resolver::DeclarationId;

/// A predicate published by a compiler or library interface.
/// Operands are positional, so renaming a caller's bindings cannot change it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContractTerm {
    Argument(usize),
    Result,
    Subject,
    Natural(u64),
    Field(&'static ContractTerm, &'static str),
    Length(&'static ContractTerm),
    Subtract(&'static ContractTerm, &'static ContractTerm),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContractPredicate {
    Less(ContractTerm, ContractTerm),
    LessEqual(ContractTerm, ContractTerm),
    Equal(ContractTerm, ContractTerm),
}

/// Every predicate in a clause must hold. A library guarantee is part of the
/// interface that its implementation must prove before it can be linked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InterfaceContract {
    pub declaration: DeclarationId,
    pub requires: &'static [ContractPredicate],
    pub ensures: &'static [ContractPredicate],
    pub copy_parameters: &'static [usize],
}
