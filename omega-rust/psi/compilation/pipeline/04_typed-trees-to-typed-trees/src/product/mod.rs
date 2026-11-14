//! Product operand preparation: a Build names Product declarations as the
//! operands of its requests, and each is prepared here before its admission
//! reads it.
//!
//! An operand that is a plain declaration path needs no preparation of its
//! own: it has no application to form, and a requirement is the one visible
//! declaration its path selects. An operand applied to static arguments
//! needs the application formed and typed, which has had no producer since
//! the old typed preparation algorithms were retired; it is refused by name
//! and publishes nothing.

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
    if operand.application.is_none() {
        return Ok(None);
    }
    Err(product_error(operand))
}

pub fn prepare_product_requirement_operand(
    program: &mut TypedTrees,
    operand: &ProductDeclarationOperand,
    _author: SymbolHandle,
    _static_environment: Option<&ProductStaticEnvironment>,
) -> Result<PreparedProductRequirement, Vec<Diagnostic>> {
    if operand.application.is_some() {
        return Err(product_error(operand));
    }
    typed_trees::product_operand::resolve_product_path(
        program,
        &operand.path,
        operand.source_span,
        ProductOperandRole::Requirement,
    )
    .map(|requirement| PreparedProductRequirement {
        application: None,
        requirement,
        closed_requirement: None,
        trait_scope: None,
    })
    .ok_or_else(|| {
        vec![
            Diagnostic::error(
                "native binding must name one exact visible package-owned requirement",
            )
            .with_source_span(operand.source_span),
        ]
    })
}

fn product_error(operand: &ProductDeclarationOperand) -> Vec<Diagnostic> {
    vec![Diagnostic::error(NO_PRODUCT_ROUTE).with_source_span(operand.source_span)]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An operand applied to static arguments has no producer: it is
    /// refused in both entrances and never becomes an empty result.
    #[test]
    fn an_applied_product_operand_is_refused_and_never_becomes_an_empty_result() {
        let mut program = TypedTrees::default();
        let operand = ProductDeclarationOperand {
            application: Some(Default::default()),
            ..Default::default()
        };
        assert!(
            prepare_product_operand_application(
                &mut program,
                &operand,
                ProductOperandRole::Provider,
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

    /// A plain declaration path has no application to prepare. As a
    /// requirement it must still select a declaration: in a program that
    /// declares none it is refused, not passed on unresolved.
    #[test]
    fn a_plain_operand_has_no_application_and_a_requirement_must_resolve() {
        let mut program = TypedTrees::default();
        let operand = ProductDeclarationOperand::default();
        assert_eq!(
            prepare_product_operand_application(
                &mut program,
                &operand,
                ProductOperandRole::Provider,
                SymbolHandle::invalid(),
                None,
                ProductApplicationDemand::FormationOnly,
            ),
            Ok(None)
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
