use arena::Handle;
use diagnostics::Diagnostic;
use language_core::inline_assembly::AsmAuthorityAdmission;
use language_semantics::declaration_selection::product_operands::ProductStaticEnvironment;
use language_semantics::declaration_selection::{ProductDeclarationOperand, ProductOperandRole};
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;
use typed_trees::opaque_representations::OpaqueRepresentationSelection;
use typed_trees::product_operand::{ProductApplicationDemand, ProductOperandApplication};
use typed_trees::{FrozenTypedTrees, TypedTrees};

/// Requested input policy, not evidence that preparation or checking occurred.
#[derive(Clone, Copy)]
pub struct PreparationRequest<'request> {
    admits_pending_inputs: bool,
    deferred_layout_applications: &'request [typed_trees::types::TypeReferenceHandle],
    asm_authority: Option<AsmAuthorityAdmission>,
    opaque_representation_selections: &'request [OpaqueRepresentationSelection],
    wire_compatibility_demands: &'request [language_semantics::wire::WireCompatibilityDemand],
}

impl<'request> PreparationRequest<'request> {
    pub const fn settled() -> Self {
        Self {
            admits_pending_inputs: false,
            deferred_layout_applications: &[],
            asm_authority: None,
            opaque_representation_selections: &[],
            wire_compatibility_demands: &[],
        }
    }

    pub const fn preliminary() -> Self {
        Self {
            admits_pending_inputs: true,
            ..Self::settled()
        }
    }

    pub const fn with_asm_authority(mut self, authority: AsmAuthorityAdmission) -> Self {
        self.asm_authority = Some(authority);
        self
    }

    pub const fn with_opaque_representation_selections(
        mut self,
        selections: &'request [OpaqueRepresentationSelection],
    ) -> Self {
        self.opaque_representation_selections = selections;
        self
    }

    pub const fn with_wire_compatibility_demands(
        mut self,
        demands: &'request [language_semantics::wire::WireCompatibilityDemand],
    ) -> Self {
        self.wire_compatibility_demands = demands;
        self
    }

    pub fn admission(&self) -> PreparationAdmission {
        PreparationAdmission {
            admits_pending_inputs: self.admits_pending_inputs,
            deferred_layout_applications: self.deferred_layout_applications.to_vec(),
            asm_authority: self.asm_authority,
            opaque_representation_selections: self.opaque_representation_selections.to_vec(),
            wire_compatibility_demands: self.wire_compatibility_demands.to_vec(),
        }
    }

    fn has_work(&self) -> bool {
        !self.deferred_layout_applications.is_empty()
            || !self.opaque_representation_selections.is_empty()
            || !self.wire_compatibility_demands.is_empty()
    }
}

/// An owned copy of requested policy only; this carries no checked receipts.
#[derive(Debug, Clone, PartialEq, compiler_storage::Record)]
pub struct PreparationAdmission {
    admits_pending_inputs: bool,
    deferred_layout_applications: Vec<typed_trees::types::TypeReferenceHandle>,
    asm_authority: Option<AsmAuthorityAdmission>,
    opaque_representation_selections: Vec<OpaqueRepresentationSelection>,
    wire_compatibility_demands: Vec<language_semantics::wire::WireCompatibilityDemand>,
}

impl PreparationAdmission {
    pub fn deferred_layout_applications(&self) -> &[typed_trees::types::TypeReferenceHandle] {
        &self.deferred_layout_applications
    }
}

/// Compatibility name for input metadata, with no source-judgment output API.
pub type TypedPreparation = PreparationAdmission;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreparedProductRequirement {
    pub application: Option<Handle<ProductOperandApplication>>,
    pub requirement: SymbolHandle,
    pub closed_requirement: Option<SymbolHandle>,
    pub trait_scope: Option<typed_trees::trait_definition::RetainedTraitScope>,
}

const NO_SOURCE_ROUTE: &str = "no product route yet: nonempty source preparation requires a structural producer after retiring the old typed preparation algorithms";
const NO_PRODUCT_ROUTE: &str = "no product route yet: Product operand preparation requires a structural producer after retiring the old typed preparation algorithms";

fn no_source_route() -> Vec<Diagnostic> {
    vec![Diagnostic::error(NO_SOURCE_ROUTE)]
}

fn require_no_work(program: &TypedTrees) -> Result<(), Vec<Diagnostic>> {
    // Compare the entire semantic input, not just the root lists: retained
    // declarations, expressions, proof requests or plans can outlive a root.
    // TypedTrees equality excludes program identity and derived interning or
    // index state.
    // This is an empty-input test, never an acceptance judgment.
    if program == &TypedTrees::default() {
        Ok(())
    } else {
        Err(no_source_route())
    }
}

pub fn prepare_typed_trees(
    program: TypedTrees,
    request: &PreparationRequest<'_>,
) -> Result<(FrozenTypedTrees, TypedPreparation), Vec<Diagnostic>> {
    require_no_work(&program)?;
    if request.has_work() {
        return Err(no_source_route());
    }
    // Preserve the original owned input. No rewritten tree, checking facts or
    // substitute receipts are manufactured for this genuine no-work case.
    Ok((program.into(), request.admission()))
}

pub fn prepare_execution_applications(program: &mut TypedTrees) -> Result<(), Vec<Diagnostic>> {
    require_no_work(program)
}

pub fn prepare_constant_execution(program: TypedTrees) -> Result<TypedTrees, Vec<Diagnostic>> {
    require_no_work(&program)?;
    Ok(program)
}

pub fn bind_pre_specialization_authored_selections(
    program: &mut TypedTrees,
) -> Result<(), Diagnostic> {
    require_no_work(program).map_err(|_| Diagnostic::error(NO_SOURCE_ROUTE))
}

pub fn prepare_requirement_expression_views(
    program: &mut TypedTrees,
    roots: &[(SymbolHandle, ExpressionHandle)],
) -> Result<(), Vec<Diagnostic>> {
    require_no_work(program)?;
    if roots.is_empty() {
        Ok(())
    } else {
        Err(no_source_route())
    }
}

pub fn restore_copied_call_sources(
    _program: &mut TypedTrees,
    expressions: &[ExpressionHandle],
) -> Result<(), &'static str> {
    if expressions.is_empty() {
        Ok(())
    } else {
        Err("no product route yet: copied call source restoration has no structural producer")
    }
}

pub fn prepare_retained_product_application(
    _program: &mut TypedTrees,
    operand: &ProductDeclarationOperand,
    _role: ProductOperandRole,
    _declaration: SymbolHandle,
) -> Result<Handle<ProductOperandApplication>, Vec<Diagnostic>> {
    Err(product_error(operand))
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

pub fn prepare_retained_product_requirement_operand(
    _program: &mut TypedTrees,
    operand: &ProductDeclarationOperand,
    _requirement: SymbolHandle,
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
    fn empty_input_retains_original_identity_and_requested_policy() {
        let program = TypedTrees::default();
        let identity = program.identity;
        let request =
            PreparationRequest::preliminary().with_asm_authority(AsmAuthorityAdmission::HOSTED);
        let expected = request.admission();
        let (frozen, metadata) = prepare_typed_trees(program, &request).unwrap();
        assert_eq!(frozen.identity.get(), identity.get());
        assert_eq!(metadata, expected);
    }

    #[test]
    fn orphaned_source_work_is_refused_without_mutating_input() {
        let mut program = TypedTrees::default();
        program
            .tables
            .mathematical_definitions
            .insert(typed_trees::mathematical::MathematicalDefinition::default());
        assert!(program.roots.mathematical_definitions.is_empty());
        let original = program.clone();
        assert!(prepare_execution_applications(&mut program).is_err());
        assert_eq!(program, original);
        assert!(prepare_typed_trees(program, &PreparationRequest::settled()).is_err());
    }

    #[test]
    fn requested_work_and_product_operands_do_not_become_empty_results() {
        let mut program = TypedTrees::default();
        let deferred = [typed_trees::types::TypeReferenceHandle::invalid()];
        let request = PreparationRequest {
            deferred_layout_applications: &deferred,
            ..PreparationRequest::settled()
        };
        assert!(prepare_typed_trees(program.clone(), &request).is_err());
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
        assert!(restore_copied_call_sources(&mut program, &[ExpressionHandle::invalid()]).is_err());
        assert!(restore_copied_call_sources(&mut program, &[]).is_ok());
    }
}
