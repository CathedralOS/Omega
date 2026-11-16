//! Bind anonymous callable contracts to the declaration that supplies their scope.
//!
//! Lowering can encounter a callable before the surrounding declaration has a
//! symbol. These invocation-owned rows retain the exact association until symbol
//! allocation completes. Nested signatures own their scopes, so a later body or
//! alias use cannot rebind an earlier contract.

use crate::resolution::lowerer::PendingSignatureLocation;
use diagnostics::Diagnostic;
use symbol_resolved_trees::SymbolResolvedTrees;
use symbol_resolved_trees::callable_values::RuntimeCallableContractHandle;
use symbols::SymbolHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CallableScopeHandle(usize);

#[derive(Debug, Clone, Copy)]
pub(crate) enum CallableScopeOwner {
    Root,
    Data(usize),
    Alias(usize),
    Domain(usize),
    Machine(usize),
    Trait(usize),
    Conformance(usize),
    Operator(usize),
    Proposition(usize),
    Mathematical(usize),
    Measure(usize),
    Signature(PendingSignatureLocation),
}

#[derive(Debug)]
struct Scope {
    parent: Option<CallableScopeHandle>,
    owner: Option<CallableScopeOwner>,
}

#[derive(Debug)]
pub(crate) struct RuntimeCallableScopes {
    scopes: Vec<Scope>,
    current: CallableScopeHandle,
    pending: Vec<(RuntimeCallableContractHandle, CallableScopeHandle)>,
}

impl Default for RuntimeCallableScopes {
    fn default() -> Self {
        Self {
            scopes: vec![Scope {
                parent: None,
                owner: Some(CallableScopeOwner::Root),
            }],
            current: CallableScopeHandle(0),
            pending: Vec::new(),
        }
    }
}

impl RuntimeCallableScopes {
    pub(crate) fn begin(&mut self, owner: Option<CallableScopeOwner>) -> CallableScopeHandle {
        let handle = CallableScopeHandle(self.scopes.len());
        self.scopes.push(Scope {
            parent: Some(self.current),
            owner,
        });
        self.current = handle;
        handle
    }

    pub(crate) fn end(&mut self, handle: CallableScopeHandle) {
        assert_eq!(
            self.current, handle,
            "callable scopes close in lexical order"
        );
        self.current = self.scopes[handle.0]
            .parent
            .expect("a lowering scope has an enclosing declaration");
    }

    pub(crate) fn bind(&mut self, handle: CallableScopeHandle, owner: CallableScopeOwner) {
        let scope = &mut self.scopes[handle.0];
        assert!(
            scope.owner.is_none(),
            "a callable scope has one declaration owner"
        );
        scope.owner = Some(owner);
    }

    pub(crate) fn retain(&mut self, contract: RuntimeCallableContractHandle) {
        self.pending.push((contract, self.current));
    }

    pub(crate) fn bind_program(
        &mut self,
        program: &mut SymbolResolvedTrees,
    ) -> Result<(), Diagnostic> {
        for &(contract, scope) in &self.pending {
            let owner = self.scopes[scope.0].owner.ok_or_else(|| {
                Diagnostic::error("runtime callable contract lost its declaration scope")
            })?;
            let owner = owner.symbol(program)?;
            let contract = program
                .tables
                .declarations
                .runtime_callable_contracts
                .get_mut(contract);
            if contract.lexical_owner.is_valid() {
                return Err(Diagnostic::error(
                    "runtime callable contract already has a lexical declaration owner",
                ));
            }
            contract.lexical_owner = owner;
        }
        self.pending.clear();
        Ok(())
    }
}

impl CallableScopeOwner {
    fn symbol(self, program: &SymbolResolvedTrees) -> Result<SymbolHandle, Diagnostic> {
        let declarations = &program.tables.declarations;
        let symbol = match self {
            Self::Root => program.symbols.root(),
            Self::Data(index) => program.data_definitions[index].symbol,
            Self::Alias(index) => program.type_aliases[index].symbol,
            Self::Domain(index) => program.domain_definitions[index].symbol,
            Self::Machine(index) => program.machines[index].symbol,
            Self::Trait(index) => program.traits[index].symbol,
            Self::Conformance(index) => program.conformances[index].symbol,
            Self::Operator(index) => program.operators[index].symbol,
            Self::Proposition(index) => program.propositions[index].symbol,
            Self::Mathematical(index) => program.mathematical_definitions[index].symbol,
            Self::Measure(index) => program.measures[index].symbol,
            Self::Signature(PendingSignatureLocation::Trait(handle)) => {
                declarations.trait_machine_signatures.get(handle).symbol
            }
            Self::Signature(PendingSignatureLocation::MachineParameter(handle)) => {
                declarations.data_type_parameters.get(handle).symbol
            }
            Self::Signature(PendingSignatureLocation::RuntimeCallable(handle)) => {
                declarations
                    .runtime_callable_contracts
                    .get(handle)
                    .signature
                    .symbol
            }
        };
        if !symbol.is_valid() {
            return Err(Diagnostic::error(
                "runtime callable contract's supplying declaration has no symbol",
            ));
        }
        Ok(symbol)
    }
}

impl crate::resolution::lowerer::Lowerer {
    pub(crate) fn with_runtime_callable_scope<T>(
        &mut self,
        owner: CallableScopeOwner,
        operation: impl FnOnce(&mut Self) -> Result<T, Diagnostic>,
    ) -> Result<T, Diagnostic> {
        let scope = self.runtime_callable_scopes.begin(Some(owner));
        let result = operation(self);
        self.runtime_callable_scopes.end(scope);
        result
    }
}
