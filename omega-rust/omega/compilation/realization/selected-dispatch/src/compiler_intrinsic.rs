// Join prepared declaration facts to exact target and accepted provider custody.
// Source recognizers belong to provider-planning's intrinsic declaration producer,
// and numeric meaning remains in Psi's external declarations; neither preparation
// nor a closed catalog identity supplies host execution permission.
// Independent review projects the candidate source and uses the same source-free
// target join as direct-call completion, while deferred shape failures retain
// their original target and ownership demand gate.

use diagnostics::Diagnostic;
use effects::provider_plan::{ProviderBinding, ProviderPlan, ProviderPlanRow};
use provider_planning::intrinsic_declarations::IntrinsicDeclaration;
use provider_planning::service_declarations::ServiceDeclarations;
use provider_planning::{CompilerIntrinsicExecutionIdentity, ProviderSchemaDeclaration};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;

pub(crate) use provider_planning::intrinsic_declarations::{
    AcceptedIntrinsicBinding as AcceptedRowBinding, HostedService,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedCompilerIntrinsicExecutionIdentity {
    Closed(CompilerIntrinsicExecutionIdentity),
    Unsupported,
}

pub fn derive_selected_compiler_intrinsic_execution_identity_for_row(
    typed: &TypedTrees,
    numeric: crate::NumericProviderSelection<'_>,
    services: &ServiceDeclarations,
    plan: &ProviderPlan,
    schema: ProviderSchemaDeclaration,
    row: &ProviderPlanRow,
    requirement_symbol: SymbolHandle,
    realization_symbol: SymbolHandle,
    selected_target: Option<&str>,
    accepted_console_binding: Option<&crate::ResolvedAcceptedSemanticBinding>,
    accepted_process_exit_binding: Option<&crate::ResolvedAcceptedSemanticBinding>,
) -> Result<Option<SelectedCompilerIntrinsicExecutionIdentity>, Diagnostic> {
    fn resolved<'a>(
        services: &ServiceDeclarations,
        binding: &'a crate::ResolvedAcceptedSemanticBinding,
    ) -> Result<AcceptedRowBinding<'a>, Diagnostic> {
        services.rejoin(binding.declaration())?;
        Ok(AcceptedRowBinding {
            accepted: binding.accepted(),
            declaration_identity: Some(binding.declaration().nominal_identity.as_str()),
            public_plan: None,
        })
    }
    row_execution_identity(
        typed,
        numeric,
        services,
        plan,
        schema,
        row,
        requirement_symbol,
        realization_symbol,
        selected_target,
        accepted_console_binding
            .map(|binding| resolved(services, binding))
            .transpose()?,
        accepted_process_exit_binding
            .map(|binding| resolved(services, binding))
            .transpose()?,
    )
}

pub(crate) fn row_execution_identity(
    typed: &TypedTrees,
    numeric: crate::NumericProviderSelection<'_>,
    services: &ServiceDeclarations,
    plan: &ProviderPlan,
    schema: ProviderSchemaDeclaration,
    row: &ProviderPlanRow,
    requirement: SymbolHandle,
    realization: SymbolHandle,
    target: Option<&str>,
    console: Option<AcceptedRowBinding<'_>>,
    process_exit: Option<AcceptedRowBinding<'_>>,
) -> Result<Option<SelectedCompilerIntrinsicExecutionIdentity>, Diagnostic> {
    if !matches!(row.binding, ProviderBinding::CompilerIntrinsic { .. }) {
        return Ok(None);
    }
    if let Some(execution) =
        numeric_execution(numeric, plan, schema, row, requirement, realization)?
    {
        return Ok(Some(SelectedCompilerIntrinsicExecutionIdentity::Closed(
            execution,
        )));
    }
    let identity = services
        .for_symbol(schema.symbol())
        .map(|service| service.declaration.nominal_identity.clone());
    let declaration =
        IntrinsicDeclaration::prepare(typed, plan, schema, row, requirement, realization, identity);
    settle_declaration(&declaration, plan, target, console, process_exit).map(Some)
}

pub(crate) fn numeric_execution(
    numeric: crate::NumericProviderSelection<'_>,
    plan: &ProviderPlan,
    schema: ProviderSchemaDeclaration,
    row: &ProviderPlanRow,
    requirement: SymbolHandle,
    realization: SymbolHandle,
) -> Result<Option<CompilerIntrinsicExecutionIdentity>, Diagnostic> {
    if matches!(schema, ProviderSchemaDeclaration::BoundaryOperator(_))
        || (matches!(schema, ProviderSchemaDeclaration::BoundaryRequirement(_))
            && numeric.has_requirement_schema(requirement))
    {
        return numeric
            .execution(plan, row, requirement, realization)
            .map(Some);
    }
    Ok(None)
}

pub(crate) fn settle_declaration(
    declaration: &IntrinsicDeclaration,
    plan: &ProviderPlan,
    target: Option<&str>,
    console: Option<AcceptedRowBinding<'_>>,
    process_exit: Option<AcceptedRowBinding<'_>>,
) -> Result<SelectedCompilerIntrinsicExecutionIdentity, Diagnostic> {
    Ok(
        match declaration.settle(plan, target, console, process_exit)? {
            Some(identity) => SelectedCompilerIntrinsicExecutionIdentity::Closed(identity),
            None => SelectedCompilerIntrinsicExecutionIdentity::Unsupported,
        },
    )
}
