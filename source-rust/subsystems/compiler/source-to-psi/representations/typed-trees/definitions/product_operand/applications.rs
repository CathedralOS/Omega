use crate::declaration_selection::ProductDeclarationOperand;
use crate::declaration_selection::ProductOperandRole;
use crate::declaration_selection::product_operands::ProductStaticEnvironment;
use crate::source_state::{TypedTrees, expression::StaticMachineArgument};
use arena::Handle;
use std::sync::Arc;
use symbols::SymbolHandle;

fn requirement_role(role: Option<ProductOperandRole>) -> bool {
    matches!(
        role,
        Some(ProductOperandRole::Requirement | ProductOperandRole::TestRequirement)
    )
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, record_storage::Record)]
pub enum ProductApplicationDemand {
    #[default]
    FormationOnly,
    Selected,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
pub struct ProductOperandApplication {
    pub operand: ProductDeclarationOperand,
    pub environment: Option<Arc<ProductStaticEnvironment>>,
    pub owner: SymbolHandle,
    pub role: Option<ProductOperandRole>,
    pub declaration: SymbolHandle,
    pub declaration_parameters: arena::HandleSpan<crate::source_state::data::TypeParameter>,
    pub declaration_lifetime_count: u32,
    pub demand: ProductApplicationDemand,
    pub argument: StaticMachineArgument,
    pub input_selections: arena::HandleSpan<super::ProductInputSelection>,
    pub scope: crate::source_state::trait_definition::RetainedTraitScope,
    pub supplying_application:
        crate::source_state::static_applications::StaticMachineApplicationHandle,
    pub implementation: Option<crate::source_state::ClosedConformanceApplication>,
}

impl TypedTrees {
    pub fn product_operand_application_identity(
        &self,
        handle: Handle<ProductOperandApplication>,
    ) -> Result<Option<String>, &'static str> {
        if !self.product_operand_applications.is_valid(handle) {
            return Err("Product application has no live prepared row");
        }
        let application = self.product_operand_applications.get(handle);
        let role = self.product_operand_application_role(application)?;
        if !matches!(
            role,
            Some(
                ProductOperandRole::BoundarySubject
                    | ProductOperandRole::Provider
                    | ProductOperandRole::Requirement
                    | ProductOperandRole::TestRequirement
            )
        ) {
            return Err("Product application has no nominal declaration role");
        }
        let parameters = self
            .data_type_parameters
            .span(application.declaration_parameters)
            .ok_or("Product application has a stale declared formal table")?;
        if parameters.is_empty()
            && application.declaration_lifetime_count == 0
            && !requirement_role(role)
        {
            return Ok(None);
        }
        if requirement_role(role) {
            let closed = self.product_requirement_application(handle)?;
            let (owner, requirement) = self
                .trait_machine_signature(closed.signature.symbol)
                .ok_or("Product requirement application lost its closed signature")?;
            return Ok(Some(
                self.trait_requirement_callable_identity_text(owner, requirement),
            ));
        }
        let identity = match role {
            Some(ProductOperandRole::BoundarySubject) => application
                .scope
                .scope(self)
                .and_then(|scope| scope.portable_application_identity(self)),
            // A provider is identified by its declaration and the
            // complete argument tuple it is applied to, phantom
            // arguments included (conformances.md: an application
            // keeps "the exact package-owned trait application,
            // including phantom arguments"). A closed conformance
            // record states the same tuple when one was formed.
            Some(ProductOperandRole::Provider) => match &application.implementation {
                Some(implementation) => self.portable_conformance_application_identity(
                    implementation,
                    application.supplying_application,
                ),
                None => {
                    let supplying = application.supplying_application;
                    if !self
                        .expression_table
                        .static_machine_application_is_valid(supplying)
                    {
                        return Err(
                            "Product implementation lost its supplying application",
                        );
                    }
                    crate::source_state::type_identity::portable_applied_index_identity(
                        self,
                        crate::source_state::type_identity::IndexIdentityArgument::StaticArgument(
                            &application.argument,
                        ),
                        self.static_machine_application(supplying).lexical_owner,
                        &[],
                    )
                }
            },
            _ => unreachable!("nominal role checked above"),
        }
        .ok_or("Product application has no complete portable static identity")?;
        self.type_identity_text(identity)
            .map(Some)
            .ok_or("Product application lost its canonical identity")
    }

    pub fn product_operand_application_role(
        &self,
        application: &ProductOperandApplication,
    ) -> Result<Option<ProductOperandRole>, &'static str> {
        self.product_operand_declaration_role(application.role, application.declaration)
    }

    pub fn product_operand_declaration_role(
        &self,
        role: Option<ProductOperandRole>,
        declaration: SymbolHandle,
    ) -> Result<Option<ProductOperandRole>, &'static str> {
        if role != Some(ProductOperandRole::RootGrantSubject) {
            return Ok(role);
        }
        if self.conformance_by_symbol(declaration).is_some() {
            Ok(Some(ProductOperandRole::Provider))
        } else if self.trait_machine_signature(declaration).is_some() {
            Ok(Some(ProductOperandRole::Requirement))
        } else {
            Err(
                "root grant application names neither a provider nor a boundary requirement",
            )
        }
    }

    /// A native binding selects the canonical closed header only while
    /// its original supplying arguments and declaring scope still match.
    pub fn product_requirement_application(
        &self,
        handle: Handle<ProductOperandApplication>,
    ) -> Result<crate::source_state::ClosedRequirementApplication<'_>, &'static str>
    {
        if !self.product_operand_applications.is_valid(handle) {
            return Err("Product requirement application has no live prepared row");
        }
        let application = self.product_operand_applications.get(handle);
        if !requirement_role(self.product_operand_application_role(application)?) {
            return Err("Product application is not a requirement");
        }
        let closed = self
            .closed_requirement_application(application.declaration)
            .ok_or("Product requirement application has no canonical closed header")?;
        if closed.template != application.argument.symbol {
            return Err("Product requirement application belongs to another declaration");
        }
        self.require_closed_requirement_call_application(
            application.declaration,
            application.supplying_application,
        )?;
        let scope = application
            .scope
            .scope(self)
            .ok_or("Product requirement application lost its declaring trait scope")?;
        if !self
            .requirement_application_matches_contract_scope(application.declaration, &scope)
        {
            return Err(
                "Product requirement application changed its declaring trait scope",
            );
        }
        Ok(closed)
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

    pub fn retain_dependency_product_application(
        &mut self,
        handle: Handle<ProductOperandApplication>,
    ) {
        if !self
            .retained_product_application_handles
            .span_or_empty(self.roots.retained_product_applications)
            .iter()
            .any(|retained| *retained == handle)
        {
            self.tables
                .retained_product_application_handles
                .append_to_span(&mut self.roots.retained_product_applications, handle);
        }
    }

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
