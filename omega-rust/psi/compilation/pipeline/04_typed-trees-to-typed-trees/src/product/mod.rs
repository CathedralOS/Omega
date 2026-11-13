//! Product operand preparation has no producer since the old typed
//! preparation algorithms were retired. These entrances keep the request
//! shapes and refuse every operand by name; they publish nothing.

use arena::Handle;
use diagnostics::Diagnostic;
use language_semantics::declaration_selection::product_operands::ProductStaticEnvironment;
use language_semantics::declaration_selection::{ProductDeclarationOperand, ProductOperandRole};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::product_operand::{ProductApplicationDemand, ProductOperandApplication};

const NO_PRODUCT_ROUTE: &str = "no product route yet: Product operand preparation requires a structural producer after retiring the old typed preparation algorithms";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreparedProductRequirement {
    pub application: Option<Handle<ProductOperandApplication>>,
    pub requirement: SymbolHandle,
    pub closed_requirement: Option<SymbolHandle>,
    pub trait_scope: Option<typed_trees::trait_definition::RetainedTraitScope>,
}

pub fn prepare_product_operand_application(
    _program: &mut TypedTrees,
    operand: &ProductDeclarationOperand,
    _role: ProductOperandRole,
    _author: SymbolHandle,
    _static_environment: Option<&ProductStaticEnvironment>,
    _demand: ProductApplicationDemand,
) -> Result<Option<Handle<ProductOperandApplication>>, Vec<Diagnostic>> {
    Err(product_error(operand))
}

pub fn prepare_product_requirement_operand(
    _program: &mut TypedTrees,
    operand: &ProductDeclarationOperand,
    _author: SymbolHandle,
    _static_environment: Option<&ProductStaticEnvironment>,
) -> Result<PreparedProductRequirement, Vec<Diagnostic>> {
    Err(product_error(operand))
}

fn product_error(operand: &ProductDeclarationOperand) -> Vec<Diagnostic> {
    vec![Diagnostic::error(NO_PRODUCT_ROUTE).with_source_span(operand.source_span)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_product_operand_is_refused_and_never_becomes_an_empty_result() {
        let mut program = TypedTrees::default();
        let operand = ProductDeclarationOperand::default();
        assert!(
            prepare_product_operand_application(
                &mut program,
                &operand,
                ProductOperandRole::Requirement,
                SymbolHandle::invalid(),
                None,
                ProductApplicationDemand::FormationOnly,
            )
            .is_err()
        );
        assert!(
            prepare_product_requirement_operand(
                &mut program,
                &operand,
                SymbolHandle::invalid(),
                None
            )
            .is_err()
        );
    }
}
