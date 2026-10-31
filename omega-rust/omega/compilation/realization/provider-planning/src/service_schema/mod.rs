//! Attach target calling evidence to Psi-owned boundary declarations.
//!
//! No neutral signature, claim, reach or inherited receiver construction lives
//! here. Psi supplies the surface and exact ordered requirement correspondence.

use crate::ProviderPlanningContext;
use crate::calling_policy_plans::BoundaryCallingPlanRealization;
use checked_trees::typed_trees::service_declarations::projection;
use checked_trees::typed_trees::service_declarations::{
    PreparedServiceSurface, ProviderSchemaDeclaration,
};
use effects::provider_plan::{ServiceCallingApplication, ServiceCallingPlan, ServiceSchema};

pub use checked_trees::typed_trees::service_declarations::projection::is_product_declaration;

/// Source formation already fixed both the neutral surface and attachment
/// coordinates. Targets attach evidence without projecting that source again.
pub(crate) fn attach_calling(
    prepared: &PreparedServiceSurface<'_>,
    calling_plans: &[BoundaryCallingPlanRealization],
) -> ServiceSchema {
    attach_requirement_calling(
        prepared.declaration.clone(),
        prepared.schema,
        prepared.argument_owner,
        prepared
            .requirements
            .iter()
            .map(|requirement| requirement.source.symbol),
        prepared.boundary_arguments,
        calling_plans,
    )
}

fn attach_requirement_calling(
    declaration: std::sync::Arc<language_semantics::service_surface::ServiceSurface>,
    source: ProviderSchemaDeclaration,
    argument_owner: symbols::SymbolHandle,
    requirements: impl IntoIterator<Item = symbols::SymbolHandle>,
    boundary_arguments: &[checked_trees::typed_trees::types::TypeReferenceHandle],
    calling_plans: &[BoundaryCallingPlanRealization],
) -> ServiceSchema {
    let mut schema = ServiceSchema::from_declaration(declaration);
    if matches!(source, crate::ProviderSchemaDeclaration::BoundaryTrait(_)) {
        for (method, requirement) in schema.declaration.methods.iter().zip(requirements) {
            if let Some(application) = calling_plans.iter().find(|application| {
                application.boundary_trait == source.symbol()
                    && application.boundary_arguments == boundary_arguments
                    && application.argument_owner() == argument_owner
                    && application.requirement_machine == requirement
            }) {
                schema.calling_applications.push(ServiceCallingApplication {
                    requirement: method.requirement_identity,
                    plan: ServiceCallingPlan {
                        report_fingerprint: application.report_fingerprint,
                        commitment: application.commitment,
                    },
                });
            }
        }
    }
    schema
}

pub fn from_typed_operator(
    context: ProviderPlanningContext<'_>,
    operator: &checked_trees::typed_trees::operator::OperatorDefinition,
) -> Option<ServiceSchema> {
    let projected = projection::from_typed_operator(context, operator)?;
    Some(ServiceSchema::from_declaration(projected.declaration))
}

pub fn from_typed_boundary_requirement(
    context: ProviderPlanningContext<'_>,
    requirement: &checked_trees::typed_trees::machine::Machine,
) -> Option<ServiceSchema> {
    let projected = projection::from_typed_boundary_requirement(context, requirement)?;
    Some(ServiceSchema::from_declaration(projected.declaration))
}

/// Whether a service schema binds exactly this boundary-operator coordinate.
///
/// [`checked_trees::typed_trees::operator::boundary_operator_requirement_identity`] is
/// deliberately package-blind: two packages may declare same-spelled,
/// same-signature boundary operators whose canonical identities are equal.
/// Every replay join must therefore compare the retained package identity
/// too, or a foreign declaration could substitute for the selected
/// coordinate.
pub fn schema_binds_exact_boundary_operator(
    context: ProviderPlanningContext<'_>,
    schema: &ServiceSchema,
    operator: &checked_trees::typed_trees::operator::OperatorDefinition,
) -> bool {
    let program = context.typed;
    context.contains(operator.symbol)
        && operator.is_boundary
        && checked_trees::typed_trees::operator::boundary_operator_requirement_identity(
            program, operator,
        ) == schema.identity_text(schema.declaration.trait_name)
        && program.symbols.symbol_package_identity(operator.symbol)
            == schema.declaration.trait_package_identity
}

/// Whether a service schema binds exactly this boundary trait declaration.
/// Trait names share the same package-blind replay hazard as operator
/// requirement identities.
pub fn schema_binds_exact_boundary_trait(
    context: ProviderPlanningContext<'_>,
    schema: &ServiceSchema,
    trait_definition: &checked_trees::typed_trees::trait_definition::TraitDefinition,
) -> bool {
    let program = context.typed;
    context.contains(trait_definition.symbol)
        && trait_definition.is_boundary
        && program.trait_declaration_path(trait_definition)
            == schema.identity_text(schema.declaration.trait_name)
        && program
            .symbols
            .symbol_package_identity(trait_definition.symbol)
            == schema.declaration.trait_package_identity
}

/// Whether a service schema binds exactly this top-level boundary
/// requirement machine.
pub fn schema_binds_exact_boundary_requirement(
    context: ProviderPlanningContext<'_>,
    schema: &ServiceSchema,
    requirement: &checked_trees::typed_trees::machine::Machine,
) -> bool {
    let program = context.typed;
    if !context.contains(requirement.symbol)
        || !requirement.symbol.is_valid()
        || requirement.supply_mode != language_semantics::MachineSupplyMode::TopLevelRequirement
        || requirement.body_is_present
        || program.symbols.symbol_package_identity(requirement.symbol)
            != schema.declaration.trait_package_identity
    {
        return false;
    }
    let schema_trait_name = schema.identity_text(schema.declaration.trait_name);
    if requirement.spelling.is_some() {
        let target_id = program.callable_identity_by_text(schema_trait_name);
        if !target_id.is_valid() || program.machine_callable_identity(requirement) != target_id {
            return false;
        }
    } else if program.symbols.display_path(requirement.symbol, "::") != schema_trait_name {
        return false;
    }
    from_typed_boundary_requirement(context, requirement).is_some_and(|candidate| {
        // Ids only carry meaning inside the table that minted them;
        // the candidate interns in its own table, so compare the
        // resolved text, never the raw ids.
        candidate.identity_text(candidate.declaration.trait_name) == schema_trait_name
    })
}

/// A generated scalar accessor has its own compiler-catalog declaration role.
/// It does not make ordinary bodyless boundary machines intrinsic candidates.
pub fn from_sealed_accessor(
    context: ProviderPlanningContext<'_>,
    accessor: &crate::SealedAccessor,
) -> Result<ServiceSchema, diagnostics::Diagnostic> {
    if crate::SealedAccessor::authenticate(context, accessor.machine())?.as_ref() != Some(accessor)
    {
        return Err(diagnostics::Diagnostic::error(
            "sealed accessor schema substituted its authenticated declaration",
        ));
    }
    let machine = context
        .typed
        .machine(accessor.machine())
        .ok_or_else(|| diagnostics::Diagnostic::error("sealed accessor schema lost its machine"))?;
    projection::project_boundary_machine_schema(
        context,
        machine,
        crate::ProviderSchemaDeclaration::SealedAccessor(machine.symbol),
    )
    .map(|projected| ServiceSchema::from_declaration(projected.declaration))
    .ok_or_else(|| {
        diagnostics::Diagnostic::error("sealed accessor has no complete published callable schema")
    })
}
