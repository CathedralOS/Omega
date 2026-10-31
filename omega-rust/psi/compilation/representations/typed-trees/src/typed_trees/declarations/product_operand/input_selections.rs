//! Receiving-world custody of real static inputs. Source coordinates rejoin
//! occurrence IDs during preparation; immutable checking reads the actual
//! operand/type again rather than accepting a preselected symbol as evidence.
use crate::{
    TypedTrees,
    expression::StaticMachineArgument,
    types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode},
};
use language_semantics::declaration_selection::AuthoredDeclarationSelectionOccurrenceId;
use symbols::SymbolHandle;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
pub enum ProductArgumentStep {
    #[default]
    Invalid,
    Argument(u32),
    Member,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub enum ProductTypeSelectionSlot {
    Declaration,
    ArrayExtent,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub enum ProductInputSelectionTarget {
    #[default]
    Invalid,
    /// Relative to the retained original whole nominal operand. The first
    /// step must be an actual ordinal, never the inert root declaration.
    Argument(arena::HandleSpan<ProductArgumentStep>),
    Type {
        reference: TypeReferenceHandle,
        slot: ProductTypeSelectionSlot,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct ProductInputSelection {
    /// None belongs only to the arena absence row.
    pub occurrence: Option<AuthoredDeclarationSelectionOccurrenceId>,
    /// Original Product actual ordinal; it remains useful when the selected
    /// type node is deeply structural or the actual is otherwise phantom.
    pub argument_ordinal: u32,
    pub target: ProductInputSelectionTarget,
}

impl ProductInputSelection {
    pub fn selected_symbol(
        &self,
        program: &TypedTrees,
        arguments: &[StaticMachineArgument],
    ) -> Result<SymbolHandle, &'static str> {
        let symbol = match &self.target {
            ProductInputSelectionTarget::Invalid => {
                return Err("static input has no retained operand target");
            }
            ProductInputSelectionTarget::Argument(path) => {
                if !matches!(program.product_input_argument_steps.span_or_empty(*path).first(), Some(ProductArgumentStep::Argument(index)) if *index == self.argument_ordinal)
                {
                    return Err("Product input selection lost its actual argument ordinal");
                }
                let steps = program.product_input_argument_steps.span_or_empty(*path);
                let mut argument = arguments
                    .get(self.argument_ordinal as usize)
                    .ok_or("static input has a foreign original argument ordinal")?;
                for step in steps.iter().skip(1) {
                    let application = argument
                        .application
                        .as_ref()
                        .ok_or("Product input selection has no retained source application")?;
                    argument = match step {
                        ProductArgumentStep::Invalid => {
                            return Err("static input has an invalid argument path step");
                        }
                        ProductArgumentStep::Argument(index) => application
                            .arguments
                            .get(*index as usize)
                            .ok_or("Product input selection has a foreign argument ordinal")?,
                        ProductArgumentStep::Member => program
                            .expression_table
                            .static_member(application.member)
                            .ok_or("Product input selection has a foreign member continuation")?,
                    };
                }
                argument.symbol
            }
            ProductInputSelectionTarget::Type { reference, slot } => {
                let actual = arguments
                    .get(self.argument_ordinal as usize)
                    .ok_or("Product type input selection lost its actual argument ordinal")?;
                #[derive(Default)]
                struct Inputs {
                    types:
                        std::collections::HashSet<TypeReferenceHandle, symbols::BuildSymbolHasher>,
                    expressions: std::collections::HashSet<
                        crate::expression::ExpressionHandle,
                        symbols::BuildSymbolHasher,
                    >,
                }
                impl Extend<SymbolHandle> for Inputs {
                    fn extend<T: IntoIterator<Item = SymbolHandle>>(&mut self, _: T) {}
                }
                impl crate::references::TypedReferences for Inputs {
                    fn type_reference(&mut self, reference: TypeReferenceHandle) -> bool {
                        self.types.insert(reference)
                    }
                    fn expression(
                        &mut self,
                        expression: crate::expression::ExpressionHandle,
                    ) -> bool {
                        self.expressions.insert(expression)
                    }
                    fn proof_fact(&mut self, _: arena::Handle<crate::domain::ProofFact>) -> bool {
                        false
                    }
                    fn mathematical_type(
                        &mut self,
                        _: crate::mathematical::MathematicalTypeHandle,
                    ) -> bool {
                        false
                    }
                }
                let mut inputs = Inputs::default();
                crate::references::visit_static_argument(program, &mut inputs, actual);
                if !inputs.types.contains(reference) {
                    return Err(
                        "Product type selection is not a retained input of its actual argument",
                    );
                }
                if !program.contains_type_reference(*reference) {
                    return Err("Product input selection has a foreign type reference");
                }
                match (
                    slot,
                    program.type_reference_table.type_reference(*reference),
                ) {
                    (
                        ProductTypeSelectionSlot::Declaration,
                        TypeReferenceNode::Named { symbol, .. },
                    ) => *symbol,
                    (
                        ProductTypeSelectionSlot::Declaration,
                        TypeReferenceNode::Generic { base_symbol, .. },
                    ) => *base_symbol,
                    (
                        ProductTypeSelectionSlot::Declaration,
                        TypeReferenceNode::DynamicTrait { symbol, .. },
                    ) => *symbol,
                    (
                        ProductTypeSelectionSlot::ArrayExtent,
                        TypeReferenceNode::FixedArray {
                            length: FixedArrayLength::ConstParameter { symbol, .. },
                            ..
                        },
                    ) => *symbol,
                    (
                        ProductTypeSelectionSlot::ArrayExtent,
                        TypeReferenceNode::FixedArray {
                            length: FixedArrayLength::ConstCall { machine, .. },
                            ..
                        },
                    ) => *machine,
                    _ => {
                        return Err(
                            "Product input selection no longer names its retained type declaration slot",
                        );
                    }
                }
            }
        };
        if !symbol.is_valid() {
            return Err("Product input selection has no exact checked declaration");
        }
        Ok(program.symbols.authored_origin(symbol))
    }
}
