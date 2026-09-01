use crate::{
    SelectedCompilerIntrinsicExecutionIdentity,
    derive_selected_compiler_intrinsic_execution_identity,
};
use omega_effects::provider_plan::{ProviderBinding, ProviderPlan, ProviderPlanRow};
use omega_provider_planning::plans::{
    CompilerIntrinsicExecutionIdentity, ProviderSchemaDeclaration,
};
use psi_checked_trees::CheckedTrees;
use psi_diagnostics::Diagnostic;
use psi_language_semantics::{
    ExternalBindingIdentity, ExternalBindingMechanism, MachineSupplyMode,
};
use psi_symbols::{BuiltinTypeAtom, SymbolHandle};
use psi_typed_trees::types::TypeReferenceNode;

/// Rederive one selected compiler-intrinsic row from exact checked declaration
/// symbols and the independently selected canonical target.
///
/// Boundary-operator rows preserve the established float catalog. The first
/// boundary-trait catalog entry is deliberately singular: the exact Linux
/// `Console::exit_process(i32) -> Unit` requirement and
/// `ConsoleNativeProvider::exit_process(i32) -> Unit` realization. Legacy
/// toolchain custody or one settled ordinary-package consumer binding must
/// additionally own that row.
pub fn derive_selected_compiler_intrinsic_execution_identity_for_row(
    checked: &CheckedTrees,
    plan: &ProviderPlan,
    schema: ProviderSchemaDeclaration,
    row: &ProviderPlanRow,
    requirement_symbol: SymbolHandle,
    realization_symbol: SymbolHandle,
    selected_target: Option<&str>,
) -> Result<Option<SelectedCompilerIntrinsicExecutionIdentity>, Diagnostic> {
    derive_selected_compiler_intrinsic_execution_identity_for_row_with_binding(
        checked,
        plan,
        schema,
        row,
        requirement_symbol,
        realization_symbol,
        selected_target,
        None,
    )
}

pub fn derive_selected_compiler_intrinsic_execution_identity_for_row_with_binding(
    checked: &CheckedTrees,
    plan: &ProviderPlan,
    schema: ProviderSchemaDeclaration,
    row: &ProviderPlanRow,
    requirement_symbol: SymbolHandle,
    realization_symbol: SymbolHandle,
    selected_target: Option<&str>,
    accepted_binding: Option<&omega_package_compilation::AcceptedSemanticBinding>,
) -> Result<Option<SelectedCompilerIntrinsicExecutionIdentity>, Diagnostic> {
    derive_selected_compiler_intrinsic_execution_identity_for_row_with_binding_and_symbol(
        checked,
        plan,
        schema,
        row,
        requirement_symbol,
        realization_symbol,
        selected_target,
        accepted_binding,
        None,
    )
}

pub fn derive_selected_compiler_intrinsic_execution_identity_for_row_with_resolved_binding(
    checked: &CheckedTrees,
    plan: &ProviderPlan,
    schema: ProviderSchemaDeclaration,
    row: &ProviderPlanRow,
    requirement_symbol: SymbolHandle,
    realization_symbol: SymbolHandle,
    selected_target: Option<&str>,
    accepted_binding: Option<&crate::ResolvedAcceptedSemanticBinding>,
) -> Result<Option<SelectedCompilerIntrinsicExecutionIdentity>, Diagnostic> {
    derive_selected_compiler_intrinsic_execution_identity_for_row_with_binding_and_symbol(
        checked,
        plan,
        schema,
        row,
        requirement_symbol,
        realization_symbol,
        selected_target,
        accepted_binding.map(crate::ResolvedAcceptedSemanticBinding::accepted),
        accepted_binding.map(crate::ResolvedAcceptedSemanticBinding::declaration_symbol),
    )
}

fn derive_selected_compiler_intrinsic_execution_identity_for_row_with_binding_and_symbol(
    checked: &CheckedTrees,
    plan: &ProviderPlan,
    schema: ProviderSchemaDeclaration,
    row: &ProviderPlanRow,
    requirement_symbol: SymbolHandle,
    realization_symbol: SymbolHandle,
    selected_target: Option<&str>,
    accepted_binding: Option<&omega_package_compilation::AcceptedSemanticBinding>,
    accepted_declaration_symbol: Option<SymbolHandle>,
) -> Result<Option<SelectedCompilerIntrinsicExecutionIdentity>, Diagnostic> {
    if !matches!(row.binding, ProviderBinding::CompilerIntrinsic { .. }) {
        return Ok(None);
    }
    if matches!(schema, ProviderSchemaDeclaration::BoundaryOperator(_)) {
        return derive_selected_compiler_intrinsic_execution_identity(
            checked,
            plan,
            requirement_symbol,
        );
    }
    let ProviderSchemaDeclaration::BoundaryTrait(trait_symbol) = schema else {
        return Ok(Some(
            SelectedCompilerIntrinsicExecutionIdentity::Unsupported,
        ));
    };
    if linux_console_exit_row(
        checked,
        plan,
        row,
        trait_symbol,
        requirement_symbol,
        realization_symbol,
        selected_target,
        accepted_binding,
        accepted_declaration_symbol,
    )? {
        return Ok(Some(SelectedCompilerIntrinsicExecutionIdentity::Closed(
            CompilerIntrinsicExecutionIdentity::LinuxExitGroupI32,
        )));
    }
    Ok(Some(
        SelectedCompilerIntrinsicExecutionIdentity::Unsupported,
    ))
}

fn linux_console_exit_row(
    checked: &CheckedTrees,
    plan: &ProviderPlan,
    row: &ProviderPlanRow,
    trait_symbol: SymbolHandle,
    requirement_symbol: SymbolHandle,
    realization_symbol: SymbolHandle,
    selected_target: Option<&str>,
    accepted_binding: Option<&omega_package_compilation::AcceptedSemanticBinding>,
    accepted_declaration_symbol: Option<SymbolHandle>,
) -> Result<bool, Diagnostic> {
    let Some(selected_target @ ("linux_x86_64" | "linux_arm64")) = selected_target else {
        return Ok(false);
    };
    if plan.target != selected_target {
        return Ok(false);
    }
    let typed = &checked.typed;
    let legacy_toolchain_binding = [trait_symbol, requirement_symbol, realization_symbol]
        .into_iter()
        .all(|symbol| {
            typed.symbols.symbol_source_origin(symbol) == Some(psi_source::SourceOrigin::Toolchain)
        });
    let accepted_package_binding = accepted_binding.is_some_and(|binding| {
        accepted_binding_matches_selected_row_identity(
            checked,
            plan,
            trait_symbol,
            requirement_symbol,
            realization_symbol,
            binding,
        ) && accepted_declaration_symbol.is_none_or(|symbol| symbol == trait_symbol)
    });
    if !legacy_toolchain_binding && !accepted_package_binding {
        return Ok(false);
    }

    console_exit_process_i32_row_shape(
        checked,
        plan,
        row,
        trait_symbol,
        requirement_symbol,
        realization_symbol,
    )
}

/// Rejoin the target-independent Console semantic role to one exact package-
/// owned selected row. This recognizes Process authority; it does not claim
/// that the selected target has a closed compiler lowering.
pub(crate) fn accepted_binding_matches_console_exit_process_i32_row(
    checked: &CheckedTrees,
    plan: &ProviderPlan,
    row: &ProviderPlanRow,
    trait_symbol: SymbolHandle,
    requirement_symbol: SymbolHandle,
    realization_symbol: SymbolHandle,
    binding: &omega_package_compilation::AcceptedSemanticBinding,
) -> Result<bool, Diagnostic> {
    if !accepted_binding_matches_selected_row_identity(
        checked,
        plan,
        trait_symbol,
        requirement_symbol,
        realization_symbol,
        binding,
    ) {
        return Ok(false);
    }
    console_exit_process_i32_row_shape(
        checked,
        plan,
        row,
        trait_symbol,
        requirement_symbol,
        realization_symbol,
    )
}

fn console_exit_process_i32_row_shape(
    checked: &CheckedTrees,
    plan: &ProviderPlan,
    row: &ProviderPlanRow,
    trait_symbol: SymbolHandle,
    requirement_symbol: SymbolHandle,
    realization_symbol: SymbolHandle,
) -> Result<bool, Diagnostic> {
    let typed = &checked.typed;

    let traits = typed
        .traits()
        .iter()
        .filter(|definition| definition.symbol == trait_symbol)
        .collect::<Vec<_>>();
    let [definition] = traits.as_slice() else {
        return Err(Diagnostic::error(format!(
            "selected ProviderPlan `{}` resolves its exact service symbol to {} trait declarations",
            plan.name,
            traits.len(),
        )));
    };
    if !definition.is_boundary
        || definition.name.as_str() != "Console"
        || !definition.lifetime_parameters.is_empty()
        || !typed.trait_type_parameters(definition).is_empty()
    {
        return Ok(false);
    }
    let requirements = typed
        .trait_machine_signatures(definition)
        .iter()
        .filter(|signature| signature.symbol == requirement_symbol)
        .collect::<Vec<_>>();
    let [requirement] = requirements.as_slice() else {
        return Err(Diagnostic::error(format!(
            "selected ProviderPlan `{}` resolves its exact requirement symbol to {} trait signatures",
            plan.name,
            requirements.len(),
        )));
    };
    let requirement_identity = typed
        .normalized_trait_requirement_overload_identity(definition, requirement)
        .identity();
    let methods = plan
        .schema
        .methods
        .iter()
        .filter(|method| plan.schema.row_binds_method(row, method))
        .collect::<Vec<_>>();
    let [method] = methods.as_slice() else {
        return Err(Diagnostic::error(format!(
            "selected ProviderPlan `{}` compiler-intrinsic row rejoins {} schema methods",
            plan.name,
            methods.len(),
        )));
    };
    if plan.schema.trait_name != definition.name.as_str()
        || method.name != "exit_process"
        || method.requirement_owner != definition.name.as_str()
        || method.requirement_identity != requirement_identity
        || row.requirement_identity != requirement_identity
        || !exact_i32_to_unit_signature(typed, requirement)
    {
        return Ok(false);
    }

    let realizations = typed
        .machines()
        .iter()
        .filter(|machine| machine.symbol == realization_symbol)
        .collect::<Vec<_>>();
    let [realization] = realizations.as_slice() else {
        return Err(Diagnostic::error(format!(
            "selected ProviderPlan `{}` resolves its exact realization symbol to {} machines",
            plan.name,
            realizations.len(),
        )));
    };
    let Some(realization_identity) = typed.normalized_machine_overload_identity(realization) else {
        return Ok(false);
    };
    let realization_identity = realization_identity.identity();
    let ProviderBinding::CompilerIntrinsic { machine } = &row.binding else {
        unreachable!("caller already admitted only compiler-intrinsic rows");
    };
    if realization.name.as_str() != "ConsoleNativeProvider::exit_process"
        || machine != &realization_identity
        || !realization.lifetime_parameters.is_empty()
        || !typed.machine_type_parameters(realization).is_empty()
        || realization.body_is_present
    {
        return Ok(false);
    }
    let MachineSupplyMode::ExternalRealization { binding, mechanism } = realization.supply_mode
    else {
        return Ok(false);
    };
    let (Some(binding), Some(mechanism)) = (binding, mechanism) else {
        return Ok(false);
    };
    if mechanism != ExternalBindingMechanism::CompilerIntrinsic
        || typed.external_bindings.identity(binding)
            != Some(&ExternalBindingIdentity::CompilerIntrinsic)
    {
        return Ok(false);
    }
    let [entry] = typed.machine_states(realization) else {
        return Ok(false);
    };
    if !exact_i32_to_unit_state(typed, entry) {
        return Ok(false);
    }
    let conformances = typed
        .machine_trait_conformances(realization)
        .iter()
        .filter(|conformance| {
            conformance.symbol == trait_symbol
                && conformance.requirement_symbol == requirement_symbol
                && conformance.requirement.as_ref().map(|name| name.as_str())
                    == Some("exit_process")
                && conformance.external_binding == Some(binding)
                && psi_typed_trees::machine::resolve_satisfied_declaration(
                    typed,
                    realization,
                    conformance,
                )
                .is_some_and(|declaration| declaration.symbol() == requirement_symbol)
        })
        .count();
    Ok(conformances == 1)
}

pub(crate) fn accepted_binding_matches_selected_row_identity(
    checked: &CheckedTrees,
    plan: &ProviderPlan,
    trait_symbol: SymbolHandle,
    requirement_symbol: SymbolHandle,
    realization_symbol: SymbolHandle,
    binding: &omega_package_compilation::AcceptedSemanticBinding,
) -> bool {
    let typed = &checked.typed;
    binding.role() == omega_package_compilation::AcceptedSemanticBindingRole::ConsoleExitProcessI32
        && [trait_symbol, requirement_symbol, realization_symbol]
            .into_iter()
            .all(|symbol| typed.symbols.symbol_package_identity(symbol) == Some(binding.package()))
        && plan.schema.trait_package_identity == Some(binding.package())
        && plan.provider_type_package_identity == Some(binding.package())
        && plan.origin_package_identity == Some(binding.package())
        && typed.symbols.display_path(trait_symbol, "::") == binding.declaration_path()
        && plan.schema.identity_digest() == binding.normalized_schema_digest()
        && plan.identity_digest() == binding.selected_provider_plan_digest()
}

fn exact_i32_to_unit_signature(
    typed: &psi_typed_trees::TypedTrees,
    signature: &psi_typed_trees::signature::StateSignature,
) -> bool {
    signature.name.as_str() == "exit_process"
        && signature.lifetime_parameters.is_empty()
        && typed.state_signature_type_parameters(signature).is_empty()
        && signature.native_callback_parameters.is_empty()
        && !signature.suspends
        && !signature.blocks
        && exact_i32_parameter(typed, typed.state_signature_parameters(signature))
        && matches!(
            typed
                .type_reference_table
                .type_reference(signature.return_type),
            TypeReferenceNode::Unit
        )
}

fn exact_i32_to_unit_state(
    typed: &psi_typed_trees::TypedTrees,
    state: &psi_typed_trees::state::State,
) -> bool {
    exact_i32_parameter(typed, typed.state_parameters(state))
        && matches!(
            typed.type_reference_table.type_reference(state.return_type),
            TypeReferenceNode::Unit
        )
}

fn exact_i32_parameter(
    typed: &psi_typed_trees::TypedTrees,
    parameters: &[psi_typed_trees::signature::StateParameter],
) -> bool {
    let [parameter] = parameters else {
        return false;
    };
    if parameter.is_self || parameter.is_const || parameter.is_mutable {
        return false;
    }
    let TypeReferenceNode::Named { symbol, name } = typed
        .type_reference_table
        .type_reference(parameter.type_reference)
    else {
        return false;
    };
    typed.symbols.builtin_type_atom(*symbol) == Some(BuiltinTypeAtom::I32) && name.as_str() == "i32"
}
