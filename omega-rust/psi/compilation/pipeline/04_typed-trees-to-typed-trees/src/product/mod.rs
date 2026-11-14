//! Identify the exact Product requirement a Build binding names.
//! This prepares declaration identity, not a source verdict or native promise.
//! Applied operands still need their structural application producer.

use arena::Handle;
use diagnostics::Diagnostic;
use language_semantics::declaration_selection::product_operands::ProductStaticEnvironment;
use language_semantics::declaration_selection::{ProductDeclarationOperand, ProductOperandRole};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::product_operand::{
    ProductApplicationDemand, ProductOperandApplication, resolve_product_path,
};

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
    program: &mut TypedTrees,
    operand: &ProductDeclarationOperand,
    author: SymbolHandle,
    _static_environment: Option<&ProductStaticEnvironment>,
) -> Result<PreparedProductRequirement, Vec<Diagnostic>> {
    if operand.application.is_some() {
        return Err(product_error(operand));
    }
    let program = program.product_declarations_mut();
    let refuse = |message| vec![Diagnostic::error(message).with_source_span(operand.source_span)];
    if !program.symbols.symbols().nodes().is_valid(author)
        || program.machine_by_symbol(author).is_none()
        || !program
            .symbols
            .symbol_is_in_dependency_scope(author, source::DependencyScope::Build)
    {
        return Err(refuse("native requirement has no original Build author"));
    }
    let occurrence = program
        .symbols
        .rejoin_product_occurrence(operand.source_span);
    if occurrence.span.start >= occurrence.span.end
        || program
            .symbols
            .symbol_provenance_source_span(author)
            .is_none_or(|source| source.source_id != occurrence.source_id)
    {
        return Err(refuse(
            "native requirement occurrence does not belong to its Build author",
        ));
    }
    let requirement = resolve_product_path(
        program,
        &operand.path,
        occurrence,
        ProductOperandRole::Requirement,
    )
    .ok_or_else(|| {
        refuse(
            "native binding must name one exact boundary requirement visible to its Build author",
        )
    })?;
    let owner = program.symbols.get(requirement).parent;
    let declaration = program
        .trait_definition_by_symbol(owner)
        .filter(|declaration| declaration.is_boundary)
        .ok_or_else(|| refuse("native requirement lost its declaring boundary trait"))?;
    let signature = program
        .trait_machine_signature_by_symbol(declaration, requirement)
        .ok_or_else(|| refuse("native requirement lost its exact declared signature"))?;
    if !declaration.type_parameters.is_empty() || !declaration.lifetime_parameters.is_empty() {
        return Err(refuse(
            "no product route yet: native requirement needs its complete compile-time application",
        ));
    }
    // The trait application is closed, but the method may bind its own
    // arguments. Native bindings retain that whole schema; concrete calls
    // supply those arguments, not Build's activation (foreign_bindings.md,
    // "Exact requirement lifetime application"). No substituted copy is owed
    // here, and native admission must still check the complete signature.
    let closed_requirement = signature.type_parameters.is_empty().then_some(requirement);
    Ok(PreparedProductRequirement {
        application: None,
        requirement,
        closed_requirement,
        trait_scope: None,
    })
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
