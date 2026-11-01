//! Declaration inputs have one binding route in whole expressions and integer
//! operands. Prefer a completed capture: its backing may already have changed.
//! Otherwise observe the exact admitted destination under the source formal's
//! carrier; arithmetic policy comes from that same formal type, never a hint.

use super::Builder;
use checked_trees::CheckedScalarComputationHandle;
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::types::TypeReferenceHandle;

impl Builder<'_, '_> {
    pub(super) fn declaration_input(
        &mut self,
        expression: ExpressionHandle,
    ) -> Result<Option<(CheckedScalarComputationHandle, TypeReferenceHandle)>, &'static str> {
        let ExpressionNode::Name(name) = self.program.expression_table.expression(expression)
        else {
            return Ok(None);
        };
        let symbol = name.symbol;
        let mut captures = self
            .declaration_values
            .iter()
            .filter(|(formal, _)| *formal == symbol);
        if let Some((_, value)) = captures.next() {
            if captures.next().is_some() {
                return Err("a declaration input has multiple completed captures");
            }
            let formal = self
                .declaration_parameters
                .iter()
                .find(|formal| formal.symbol == symbol)
                .ok_or("a captured declaration input has no source formal")?;
            if self.program.primitive_type_reference(formal.type_reference)
                != Some(self.plans.nodes.get(*value).primitive_type)
            {
                return Err("a completed declaration capture has the wrong formal carrier");
            }
            return Ok(Some((*value, formal.type_reference)));
        }
        if !self
            .program
            .expression_table
            .name_path_is_single_member(name)
        {
            return Ok(None);
        }
        let mut bindings = self.declaration_bindings.iter().filter(|(formal, _)| {
            formal.root == facts::PlaceRoot::Symbol(symbol) && formal.segments.is_empty()
        });
        let Some((_, coordinate)) = bindings.next() else {
            return Ok(None);
        };
        if bindings.next().is_some() {
            return Err("a declaration input has multiple destination coordinates");
        }
        let coordinate = coordinate.clone();
        let reference = self
            .declaration_parameters
            .iter()
            .find(|formal| formal.symbol == symbol)
            .ok_or("a declaration coordinate has no source formal")?
            .type_reference;
        let primitive = self
            .program
            .primitive_type_reference(reference)
            .ok_or("a declaration coordinate has no fixed scalar carrier")?;
        // Geometry alone supplies no dynamic-index receipts. An indexed or
        // otherwise unavailable destination still refuses through this owner.
        let value = self
            .scalar_coordinate(expression, &coordinate, primitive, &[])
            .ok_or("a declaration coordinate lacks a supported entry observation")?;
        Ok(Some((value, reference)))
    }

    /// Pure lowering owns a scalar telescope, so a subtree that refers to
    /// an explicitly rebound declaration input must first pass through the
    /// computation graph. The shared typed-reference walk owns all expression
    /// forms; independent literals and ordinary formals retain their fast path.
    pub(super) fn has_declaration_binding(&self, expression: ExpressionHandle) -> bool {
        if self.declaration_bindings.is_empty() {
            return false;
        }
        struct Bindings<'bindings> {
            bindings: &'bindings [(validation::CanonicalPlace, validation::CanonicalPlace)],
            found: bool,
            visited: Vec<ExpressionHandle>,
        }
        impl Extend<SymbolHandle> for Bindings<'_> {
            fn extend<T: IntoIterator<Item = SymbolHandle>>(&mut self, symbols: T) {
                self.found |= symbols.into_iter().any(|symbol| {
                    self.bindings
                        .iter()
                        .any(|(formal, _)| formal.root == facts::PlaceRoot::Symbol(symbol))
                });
            }
        }
        impl typed_trees::references::TypedReferences for Bindings<'_> {
            fn expression(&mut self, expression: ExpressionHandle) -> bool {
                if self.visited.contains(&expression) {
                    return false;
                }
                self.visited.push(expression);
                true
            }
            fn type_reference(&mut self, _: TypeReferenceHandle) -> bool {
                false
            }
        }
        let mut bindings = Bindings {
            bindings: self.declaration_bindings,
            found: false,
            visited: Vec::new(),
        };
        typed_trees::references::visit_expression(self.program, &mut bindings, expression);
        bindings.found
    }
}
