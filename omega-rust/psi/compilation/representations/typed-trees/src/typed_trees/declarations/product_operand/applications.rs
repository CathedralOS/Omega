//! Prepared designated applications retain the source recipe independently of
//! their closed substitution. Formation and reached demand remain distinct;
//! neither cached arena presence nor a formed recipe grants selection authority.
use crate::{TypedTrees, expression::StaticMachineArgument};
use arena::Handle;
use language_semantics::declaration_selection::{
    ProductDeclarationOperand, ProductOperandRole, product_operands::ProductStaticEnvironment,
};
use std::sync::Arc;
use symbols::SymbolHandle;

/// Source preparation can judge an inert recipe without demanding a realizer.
/// A completed Build choice separately demands its selected implementation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
pub enum ProductApplicationDemand {
    #[default]
    FormationOnly,
    Selected,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct ProductOperandApplication {
    pub operand: ProductDeclarationOperand,
    pub environment: Option<Arc<ProductStaticEnvironment>>,
    pub owner: SymbolHandle,
    pub role: Option<ProductOperandRole>,
    pub declaration: SymbolHandle,
    /// An executed Build row demands code; source-only formations do not.
    pub demand: ProductApplicationDemand,
    /// Original carrier and application recipe, before executable substitution.
    pub argument: StaticMachineArgument,
    pub scope: crate::trait_definition::RetainedTraitScope,
    pub supplying_application: crate::static_applications::StaticMachineApplicationHandle,
    pub implementation: Option<crate::typed_trees::ClosedConformanceApplication>,
}

impl TypedTrees {
    pub fn prepared_product_operand(
        &self,
        operand: &ProductDeclarationOperand,
        environment: Option<&ProductStaticEnvironment>,
        owner: SymbolHandle,
        role: ProductOperandRole,
    ) -> Option<Handle<ProductOperandApplication>> {
        self.product_operand_applications
            .iter()
            .find_map(|(handle, application)| {
                (application.operand == *operand
                    && application.environment.as_deref() == environment
                    && application.owner == owner
                    && application.role == Some(role))
                .then_some(handle)
            })
    }

    /// Preparation has already independently formed every supplied actual.
    /// Retaining the selected application adds a real source demand; it grants
    /// no provider installation, execution authority, or checking judgment.
    pub fn retain_product_operand_application(
        &mut self,
        application: ProductOperandApplication,
    ) -> Handle<ProductOperandApplication> {
        if let Some(existing) = self.prepared_product_operand(
            &application.operand,
            application.environment.as_deref(),
            application.owner,
            application
                .role
                .expect("prepared Product operand has a declaration role"),
        ) {
            return existing;
        }
        self.product_operand_applications.insert(application)
    }
}
