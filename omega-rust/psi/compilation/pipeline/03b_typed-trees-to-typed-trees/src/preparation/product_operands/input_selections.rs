//! Rejoin original static-input coordinates at their real receiving operands.
//! Paths describe an argument's position, never a declaration chosen by text.
use super::*;
use typed_trees::product_operand::{
    ProductArgumentStep, ProductInputSelection, ProductInputSelectionTarget,
    ProductTypeSelectionSlot,
};
impl Lower<'_> {
    pub(super) fn argument(
        &mut self,
        ordinal: usize,
        operand: Handle<ProductStaticOperand>,
    ) -> Result<expr::StaticMachineArgument, Vec<Diagnostic>> {
        let ordinal = u32::try_from(ordinal)
            .map_err(|_| self.error("static input argument ordinal exceeds its representation"))?;
        self.input_path.push(ProductArgumentStep::Argument(ordinal));
        let result = self.operand(operand);
        self.input_path.pop();
        result
    }

    fn input_occurrence(
        &self,
        coordinate: Handle<ProductOperandSelection>,
    ) -> Result<
        language_semantics::declaration_selection::AuthoredDeclarationSelectionOccurrenceId,
        Vec<Diagnostic>,
    > {
        if !self.graph.selections.is_valid(coordinate) {
            return Err(self.error("static input has a foreign source-selection coordinate"));
        }
        let selection = self.graph.selections.get(coordinate);
        let kind = selection
            .kind
            .ok_or_else(|| self.error("static input lost its source-selection kind"))?;
        let span = self
            .program
            .symbols
            .rejoin_product_occurrence(selection.source_span);
        self.program
            .authored_declaration_selections()
            .source_coordinate_occurrence(span, selection.exposure, kind)
            .map_err(|reason| vec![Diagnostic::error(reason.to_string()).with_source_span(span)])
    }

    fn input_ordinal(&self) -> Result<u32, Vec<Diagnostic>> {
        match self.input_path.first() {
            Some(ProductArgumentStep::Argument(ordinal)) => Ok(*ordinal),
            _ => Err(self.error("static input has no enclosing actual argument")),
        }
    }

    pub(super) fn retain_operand_input(
        &mut self,
        coordinate: Handle<ProductOperandSelection>,
    ) -> Result<(), Vec<Diagnostic>> {
        if !coordinate.is_valid() {
            return Ok(());
        }
        let occurrence = self.input_occurrence(coordinate)?;
        let argument_ordinal = self.input_ordinal()?;
        let path = self
            .program
            .product_input_argument_steps
            .insert_many(self.input_path.iter().copied());
        self.input_selections.push(ProductInputSelection {
            occurrence: Some(occurrence),
            argument_ordinal,
            target: ProductInputSelectionTarget::Argument(path),
        });
        Ok(())
    }

    pub(super) fn retain_type_input(
        &mut self,
        source: Handle<ProductOperandType>,
        reference: TypeReferenceHandle,
    ) -> Result<(), Vec<Diagnostic>> {
        let (name, slot) = match self.graph.types.get(source) {
            ProductOperandType::Named(name)
            | ProductOperandType::Generic { name, .. }
            | ProductOperandType::Dynamic { name, .. } => {
                (*name, ProductTypeSelectionSlot::Declaration)
            }
            ProductOperandType::Array {
                length: ProductArrayLength::Parameter(name) | ProductArrayLength::Call(name),
                ..
            } => (*name, ProductTypeSelectionSlot::ArrayExtent),
            _ => return Ok(()),
        };
        let coordinate = self.graph.names.get(name).authored_selection;
        if !coordinate.is_valid() {
            return Ok(());
        }
        let occurrence = self.input_occurrence(coordinate)?;
        let argument_ordinal = self.input_ordinal()?;
        let selection = ProductInputSelection {
            occurrence: Some(occurrence),
            argument_ordinal,
            target: ProductInputSelectionTarget::Type { reference, slot },
        };
        if !self.input_selections.contains(&selection) {
            self.input_selections.push(selection);
        }
        Ok(())
    }
}
