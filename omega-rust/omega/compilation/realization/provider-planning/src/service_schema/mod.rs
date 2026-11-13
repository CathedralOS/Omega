// Target calling evidence attachment and replay correspondence for Psi-owned
// boundary declarations. Psi supplies the neutral surface and exact ordered
// requirement correspondence; replay joins compare both the canonical
// requirement/trait path and `trait_package_identity` so same-spelled boundary
// operators or traits from different packages cannot substitute for one another.

use crate::ProviderPlanningContext;
use crate::calling_policy_plans::BoundaryCallingPlanRealization;
use crate::provider_planning::trait_name_text;
use checked_trees::typed_trees::service_declarations::projection;
use checked_trees::typed_trees::service_declarations::{
    PreparedServiceSurface, ProviderSchemaDeclaration,
};
use effects::provider_plan::{ServiceCallingApplication, ServiceCallingPlan, ServiceSchema};

pub use checked_trees::typed_trees::service_declarations::projection::is_product_declaration;

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
        None,
        calling_plans,
    )
}

/*
    Attach calling plans to the surface of a native Build plan bound at one
    closed application of a generic boundary. That surface was found by the
    application's portable identity, not through an implementation, so it has
    no argument handles or owning declaration to compare; the plans evaluated
    for that application are the ones whose owner application is that same
    identity.
*/
pub(crate) fn attach_native_application_calling(
    prepared: &PreparedServiceSurface<'_>,
    application: &str,
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
        Some(application),
        calling_plans,
    )
}

fn attach_requirement_calling(
    declaration: std::sync::Arc<language_semantics::service_surface::ServiceSurface>,
    source: ProviderSchemaDeclaration,
    argument_owner: symbols::SymbolHandle,
    requirements: impl IntoIterator<Item = symbols::SymbolHandle>,
    boundary_arguments: &[checked_trees::typed_trees::types::TypeReferenceHandle],
    native_application: Option<&str>,
    calling_plans: &[BoundaryCallingPlanRealization],
) -> ServiceSchema {
    let mut schema = ServiceSchema::from_declaration(declaration);
    if matches!(source, crate::ProviderSchemaDeclaration::BoundaryTrait(_)) {
        for (method, requirement) in schema.declaration.methods.iter().zip(requirements) {
            for application in calling_plans.iter().filter(|application| {
                application.boundary_trait == source.symbol()
                    && application.requirement_machine == requirement
                    && match native_application {
                        Some(bound) => application.calling_owner_application() == bound,
                        None => {
                            application.boundary_arguments == boundary_arguments
                                && application.argument_owner() == argument_owner
                        }
                    }
            }) {
                let attachment = ServiceCallingApplication {
                    requirement: method.requirement_identity,
                    abi: application.materialized_signature().abi.clone(),
                    plan: ServiceCallingPlan {
                        report_fingerprint: application.report_fingerprint,
                        commitment: application.commitment,
                        promise: effects::provider_plan::BoundaryCallingPlanPromise::from_digest(
                            application
                                .validated_boundary_entry_plan
                                .contract_commitment_digest(),
                        ),
                    },
                };
                if !schema.calling_applications.contains(&attachment) {
                    schema.calling_applications.push(attachment);
                }
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
        ) == trait_name_text(schema)
        && program.symbols.symbol_package_identity(operator.symbol)
            == schema.declaration.trait_package_identity
}

pub fn schema_binds_exact_boundary_trait(
    context: ProviderPlanningContext<'_>,
    schema: &ServiceSchema,
    trait_definition: &checked_trees::typed_trees::trait_definition::TraitDefinition,
) -> bool {
    let program = context.typed;
    context.contains(trait_definition.symbol)
        && trait_definition.is_boundary
        && program.trait_declaration_path(trait_definition) == trait_name_text(schema)
        && program
            .symbols
            .symbol_package_identity(trait_definition.symbol)
            == schema.declaration.trait_package_identity
}

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
    let schema_trait_name = trait_name_text(schema);
    if requirement.spelling.is_some() {
        let target_id = program.callable_identity_by_text(schema_trait_name);
        if !target_id.is_valid() || program.machine_callable_identity(requirement) != target_id {
            return false;
        }
    } else if program.symbols.display_path(requirement.symbol, "::") != schema_trait_name {
        return false;
    }
    from_typed_boundary_requirement(context, requirement).is_some_and(|candidate| {
        candidate.identity_text(candidate.declaration.trait_name) == schema_trait_name
    })
}

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
