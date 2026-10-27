//! Reuse the typed reference owner's edges to discover constant uses erased
//! into types. This walk never infers invocation edges or re-resolves names.

use super::{Collector, Context};
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::{
    TypedTrees,
    expression::ExpressionHandle,
    machine::Machine,
    references::{self, TypedReferences},
    types::{TypeConstantOrigin, TypeReferenceHandle},
};

pub(super) enum Root<'program> {
    Type(TypeReferenceHandle),
    Expression(ExpressionHandle),
    Machine(&'program Machine),
    Contracts(arena::HandleSpan<typed_trees::signature::SignatureContract>),
}

impl<'program> Collector<'program> {
    pub(super) fn type_dependencies(
        &mut self,
        root: Root<'program>,
        context: Context<'program>,
    ) -> Result<(), String> {
        let program = self.program;
        let mut inventory = TypeDependencies {
            program,
            root_expression: ExpressionHandle::invalid(),
            lexical_owner: context.lexical_owner,
            contract_owner: None,
            expressions: Vec::new(),
            types: Vec::new(),
            pending_data: Vec::new(),
            visited_data: Vec::new(),
            current_data: SymbolHandle::invalid(),
            origins: Vec::new(),
            error: None,
        };
        match root {
            Root::Type(reference) => {
                references::visit_type_reference(program, &mut inventory, reference)
            }
            Root::Expression(expression) => {
                inventory.root_expression = expression;
                references::visit_expression(program, &mut inventory, expression);
            }
            Root::Machine(machine) => references::visit_machine(program, &mut inventory, machine),
            Root::Contracts(contracts) => inventory.contracts(contracts, context.lexical_owner),
        }
        while let Some(symbol) = inventory.pending_data.pop() {
            if inventory.visited_data.contains(&symbol) {
                continue;
            }
            inventory.visited_data.push(symbol);
            if program
                .data_definitions()
                .iter()
                .filter(|data| data.symbol == symbol)
                .count()
                != 1
            {
                return Err("constant type dependency lost its exact data declaration".into());
            }
            inventory.current_data = symbol;
            references::visit_declarations(program, &mut inventory);
        }
        if let Some(error) = inventory.error {
            return Err(error.into());
        }
        for origin in inventory.origins {
            self.constant(
                origin.declaration,
                origin.reference,
                ExpressionHandle::invalid(),
                context,
            )?;
        }
        self.expressions
            .extend(
                inventory
                    .expressions
                    .into_iter()
                    .map(|(expression, lexical_owner)| {
                        (
                            expression,
                            Context {
                                lexical_owner,
                                ..context
                            },
                        )
                    }),
            );
        Ok(())
    }
}

struct TypeDependencies<'program> {
    program: &'program TypedTrees,
    root_expression: ExpressionHandle,
    lexical_owner: SymbolHandle,
    contract_owner: Option<SymbolHandle>,
    expressions: Vec<(ExpressionHandle, SymbolHandle)>,
    types: Vec<TypeReferenceHandle>,
    pending_data: Vec<SymbolHandle>,
    visited_data: Vec<SymbolHandle>,
    current_data: SymbolHandle,
    origins: Vec<TypeConstantOrigin>,
    error: Option<&'static str>,
}

impl TypeDependencies<'_> {
    fn contracts(
        &mut self,
        contracts: arena::HandleSpan<typed_trees::signature::SignatureContract>,
        owner: SymbolHandle,
    ) {
        let program = self.program;
        let clauses = program.tables.signature_contracts.span_or_empty(contracts);
        if clauses.len() != contracts.len() {
            self.error = Some("constant custody has stale signature clauses");
            return;
        }
        for clause in clauses {
            self.contract_facts(clause.facts, owner);
        }
    }

    fn contract_facts(
        &mut self,
        facts: arena::HandleSpan<typed_trees::domain::ProofFact>,
        owner: SymbolHandle,
    ) {
        let program = self.program;
        let clauses = program.proof_facts.span_or_empty(facts);
        if clauses.len() != facts.len() {
            self.error = Some("constant custody has stale contract facts");
            return;
        }
        let previous = self.contract_owner.replace(owner);
        for fact in clauses {
            references::visit_proof_fact(program, self, fact);
        }
        self.contract_owner = previous;
    }
}

impl Extend<SymbolHandle> for TypeDependencies<'_> {
    fn extend<T: IntoIterator<Item = SymbolHandle>>(&mut self, symbols: T) {
        for symbol in symbols {
            let symbol = if self.program.symbols.get(symbol).kind == SymbolKind::Variant {
                self.program.symbols.get(symbol).parent
            } else {
                symbol
            };
            if self.program.symbols.get(symbol).kind == SymbolKind::Data {
                self.pending_data.push(symbol);
            }
        }
    }
}

impl TypedReferences for TypeDependencies<'_> {
    fn callable_signature(&mut self, signature: &typed_trees::signature::StateSignature) {
        self.contract_facts(signature.where_facts, signature.symbol);
        self.contracts(signature.contracts, signature.symbol);
    }

    fn declaration(&mut self, symbol: SymbolHandle) -> bool {
        symbol == self.current_data
    }

    fn type_reference(&mut self, reference: TypeReferenceHandle) -> bool {
        if self.types.contains(&reference) {
            return false;
        }
        self.types.push(reference);
        match self
            .program
            .type_reference_table
            .constant_origins(reference)
        {
            Ok(origins) => {
                for origin in origins {
                    if !self.origins.contains(origin) {
                        self.origins.push(*origin);
                    }
                }
                true
            }
            Err(error) => {
                self.error = Some(error);
                false
            }
        }
    }

    fn expression(&mut self, expression: ExpressionHandle) -> bool {
        if let Some(owner) = self.contract_owner {
            self.expressions.push((expression, owner));
            return false;
        }
        if expression == self.root_expression {
            self.root_expression = ExpressionHandle::invalid();
            return true;
        }
        false
    }

    fn type_expression(&mut self, expression: ExpressionHandle) -> bool {
        // The custody collector owns expression context, cycle control and
        // invocation selection. Return only type-owned endpoints to that same
        // traversal. Machine inventory must not promote proof/ranking roots
        // or walk runtime children a second time under the entry state's scope.
        if expression.is_valid() {
            self.expressions.push((
                expression,
                self.contract_owner.unwrap_or(self.lexical_owner),
            ));
        }
        false
    }

    fn proof_fact(&mut self, _: arena::Handle<typed_trees::domain::ProofFact>) -> bool {
        false
    }
}
