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
    /// Every receiving-world occurrence is tied to one real unfurled input.
    pub input_selections: arena::HandleSpan<super::ProductInputSelection>,
    pub scope: crate::trait_definition::RetainedTraitScope,
    pub supplying_application: crate::static_applications::StaticMachineApplicationHandle,
    pub implementation: Option<crate::typed_trees::ClosedConformanceApplication>,
}

impl TypedTrees {
    /// Canonical complete application, projected for a serialization boundary.
    /// Provider identity includes its own entire telescope, even phantom inputs
    /// absent from the satisfied trait. Compact conformance commitments are not
    /// portable application identities.
    pub fn product_operand_application_identity(
        &self,
        handle: Handle<ProductOperandApplication>,
    ) -> Result<Option<crate::type_identity::NormalizedTypeIdentity>, &'static str> {
        let application = self.product_operand_applications.get(handle);
        let (parameters_empty, lifetimes_empty) = match application.role {
            Some(ProductOperandRole::BoundarySubject) => {
                let declaration = self
                    .trait_definition_by_symbol(application.declaration)
                    .ok_or("Product boundary application lost its declaration")?;
                (
                    self.trait_type_parameters(declaration).is_empty(),
                    declaration.lifetime_parameters.is_empty(),
                )
            }
            Some(ProductOperandRole::Provider) => {
                let declaration = self
                    .conformance_by_symbol(application.declaration)
                    .ok_or("Product implementation application lost its declaration")?;
                (
                    self.conformance_type_parameters(declaration).is_empty(),
                    declaration.lifetime_parameters.is_empty(),
                )
            }
            _ => return Err("Product application has no nominal declaration role"),
        };
        // Empty telescopes are the same declaration whether an empty <> was
        // authored or omitted. Nonempty applied telescopes retain every input.
        if parameters_empty && lifetimes_empty {
            return Ok(None);
        }
        let identity = match application.role {
            Some(ProductOperandRole::BoundarySubject) => application
                .scope
                .scope(self)
                .and_then(|scope| scope.portable_application_identity(self)),
            Some(ProductOperandRole::Provider) => self.portable_conformance_application_identity(
                application
                    .implementation
                    .as_ref()
                    .ok_or("Product implementation lost its exact closure")?,
                application.supplying_application,
            ),
            _ => unreachable!("nominal role checked above"),
        }
        .ok_or("Product application has no complete portable static identity")?;
        Ok(Some(identity))
    }

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

    /// The package input owner admitted the original executed choice and its
    /// immutable source payload. Rooting it demands ordinary receiving-world
    /// checking and capture; it supplies neither provider installation nor code
    /// from the producing Build activation.
    pub fn retain_dependency_product_application(
        &mut self,
        handle: Handle<ProductOperandApplication>,
    ) {
        if !self
            .retained_product_application_handles
            .span_or_empty(self.roots.retained_product_applications)
            .contains(&handle)
        {
            self.tables
                .retained_product_application_handles
                .append_to_span(&mut self.roots.retained_product_applications, handle);
        }
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
