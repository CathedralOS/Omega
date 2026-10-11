//! Closed static operands share one transitive closure judgment.
//! Materialized nominal origins and embedded expressions remain part of the
//! application, including phantom parameters. Region-erasing ordinary identity
//! never establishes that an application is closed or exact.

use crate::{TypedTrees, types};
use symbols::SymbolHandle;

#[derive(Default)]
struct ClosedOperands {
    symbols: Vec<SymbolHandle>,
    types: Vec<types::TypeReferenceHandle>,
    expressions: Vec<crate::expression::ExpressionHandle>,
}

impl Extend<SymbolHandle> for ClosedOperands {
    fn extend<Symbols: IntoIterator<Item = SymbolHandle>>(&mut self, symbols: Symbols) {
        self.symbols.extend(symbols);
    }
}

impl crate::references::TypedReferences for ClosedOperands {
    fn type_reference(&mut self, reference: types::TypeReferenceHandle) -> bool {
        if self.types.contains(&reference) {
            return false;
        }
        self.types.push(reference);
        true
    }
    fn expression(&mut self, expression: crate::expression::ExpressionHandle) -> bool {
        if self.expressions.contains(&expression) {
            return false;
        }
        self.expressions.push(expression);
        true
    }
}

impl TypedTrees {
    /// Representation closure, not predicate admission or provider authority.
    pub fn require_closed_application_operands(
        &self,
        arguments: &[types::TypeReferenceHandle],
    ) -> Result<(), &'static str> {
        self.require_application_operand_bindings(arguments, SymbolHandle::invalid())
    }

    /// Formal declaration well-formedness is not concrete closure. Only type
    /// binders from the supplying lexical chain may remain symbolic; runtime
    /// values and foreign declaration binders still cannot become operands.
    pub fn require_declared_application_operands(
        &self,
        arguments: &[types::TypeReferenceHandle],
        lexical_owner: SymbolHandle,
    ) -> Result<(), &'static str> {
        self.require_application_operand_bindings(arguments, lexical_owner)
    }

    fn require_application_operand_bindings(
        &self,
        arguments: &[types::TypeReferenceHandle],
        lexical_owner: SymbolHandle,
    ) -> Result<(), &'static str> {
        let mut operands = ClosedOperands::default();
        for argument in arguments.iter() {
            crate::references::visit_type_reference(self, &mut operands, *argument);
        }
        // A materialized nominal owns an origin application even when its
        // visible node is now a bare name. Closure must follow that origin,
        // just as region-sensitive identity does, including phantom operands.
        let mut next = 0;
        while let Some(reference) = operands.types.get(next).copied() {
            next += 1;
            let symbol = match self.type_reference_table.type_reference(reference) {
                types::TypeReferenceNode::Named { symbol, .. } => *symbol,
                types::TypeReferenceNode::Generic { base_symbol, .. } => *base_symbol,
                _ => continue,
            };
            if let Some(origin) = self
                .data_definition_by_symbol(symbol)
                .and_then(|definition| definition.generic_instance)
            {
                crate::references::visit_type_reference(self, &mut operands, origin);
            }
        }
        if operands
            .symbols
            .iter()
            .filter(|symbol| symbol.is_valid())
            .any(|symbol| {
                if self
                    .data_type_parameter_by_symbol(*symbol)
                    .is_some_and(|parameter| {
                        matches!(
                            parameter.kind,
                            crate::data::TypeParameterKind::Type
                                | crate::data::TypeParameterKind::Const { .. }
                        )
                    })
                {
                    let parent = self.symbols.get(*symbol).parent;
                    let mut owner = lexical_owner;
                    while owner.is_valid() {
                        if parent == owner {
                            return false;
                        }
                        owner = self.symbols.get(owner).parent;
                    }
                }
                matches!(
                    self.symbols.get(*symbol).kind,
                    symbols::SymbolKind::TypeParameter
                        | symbols::SymbolKind::MachineParameter
                        | symbols::SymbolKind::ConformanceParameter
                        | symbols::SymbolKind::PropositionParameter
                        | symbols::SymbolKind::PropositionMachineParameter
                        | symbols::SymbolKind::Local
                        | symbols::SymbolKind::Parameter
                )
            })
        {
            return Err("open operands do not close a requirement instance");
        }
        for reference in &operands.types {
            if let types::TypeReferenceNode::Named { symbol, .. } =
                self.type_reference_table.type_reference(*reference)
                && let Some(definition) = self.data_definition_by_symbol(*symbol)
                && (!definition.type_parameters.is_empty()
                    || !definition.lifetime_parameters.is_empty())
            {
                return Err("closed requirement operand omits a nominal telescope");
            }
        }
        // The present exact-application representation deliberately refuses
        // region-bearing operands; an erased carrier identity is not custody.
        for argument in arguments.iter().copied() {
            if self
                .region_free_scoped_type_identity(
                    crate::type_identity::TypeIdentityRequest::ordinary(argument),
                    &[],
                )
                .is_none()
            {
                return Err("closed requirement needs a retained exact region application");
            }
        }
        Ok(())
    }
}
