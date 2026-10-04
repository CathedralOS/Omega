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
        self.expressions.extend(
            inventory
                .expressions
                .into_iter()
                .map(|expression| (expression, context)),
        );
        Ok(())
    }
}

struct TypeDependencies<'program> {
    program: &'program TypedTrees,
    root_expression: ExpressionHandle,
    expressions: Vec<ExpressionHandle>,
    types: Vec<TypeReferenceHandle>,
    pending_data: Vec<SymbolHandle>,
    visited_data: Vec<SymbolHandle>,
    current_data: SymbolHandle,
    origins: Vec<TypeConstantOrigin>,
    error: Option<&'static str>,
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
            self.expressions.push(expression);
        }
        false
    }

    fn proof_fact(&mut self, _: arena::Handle<typed_trees::domain::ProofFact>) -> bool {
        false
    }
}
