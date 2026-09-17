//! Selected compiler-intrinsic execution identities: deriving the identity
//! of one settled row (console read, write and exit, process exit) and
//! matching an accepted binding against it.

use crate::{
    SelectedCompilerIntrinsicExecutionIdentity,
    derive_selected_compiler_intrinsic_execution_identity,
};
use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;
use effects::provider_plan::{ProviderBinding, ProviderPlan, ProviderPlanRow};
use language_semantics::MachineSupplyMode;
use provider_planning::{CompilerIntrinsicExecutionIdentity, ProviderSchemaDeclaration};
use symbols::{BuiltinTypeAtom, SymbolHandle};
use typed_trees::types::TypeReferenceNode;

/// Rederive one selected compiler-intrinsic row from exact checked declaration
/// symbols and the independently selected canonical target.
///
/// Boundary-operator rows preserve the established float catalog. Console
/// entries bind the exact requirement and realization on each supported target.
/// Byte input publishes its honest hosted envelope (`blocks;` plus an
/// unconditional `crashes Trap` route) on the requirement and realization
/// alike; byte output and exit keep their nonblocking, crash-free ceiling.
/// The source leaf is bodyless boundary supply without an authored
/// payload-free `via`; toolchain custody or one settled ordinary-package
/// consumer binding must additionally own that row.
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
    accepted_binding: Option<&package_compilation::AcceptedSemanticBinding>,
    accepted_process_exit_binding: Option<&package_compilation::AcceptedSemanticBinding>,
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
        accepted_process_exit_binding,
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
    accepted_process_exit_binding: Option<&crate::ResolvedAcceptedSemanticBinding>,
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
        accepted_process_exit_binding.map(crate::ResolvedAcceptedSemanticBinding::accepted),
        accepted_process_exit_binding
            .map(crate::ResolvedAcceptedSemanticBinding::declaration_symbol),
    )
}

#[allow(clippy::too_many_arguments)]
fn derive_selected_compiler_intrinsic_execution_identity_for_row_with_binding_and_symbol(
    checked: &CheckedTrees,
    plan: &ProviderPlan,
    schema: ProviderSchemaDeclaration,
    row: &ProviderPlanRow,
    requirement_symbol: SymbolHandle,
    realization_symbol: SymbolHandle,
    selected_target: Option<&str>,
    accepted_binding: Option<&package_compilation::AcceptedSemanticBinding>,
    accepted_declaration_symbol: Option<SymbolHandle>,
    accepted_process_exit_binding: Option<&package_compilation::AcceptedSemanticBinding>,
    accepted_process_exit_declaration_symbol: Option<SymbolHandle>,
) -> Result<Option<SelectedCompilerIntrinsicExecutionIdentity>, Diagnostic> {
    if !matches!(row.binding, ProviderBinding::CompilerIntrinsic { .. }) {
        return Ok(None);
    }
    // The float intrinsic catalog is keyed on the requirement view: a named
    // boundary operator or a top-level `boundary requirement` machine.
    if matches!(schema, ProviderSchemaDeclaration::BoundaryOperator(_))
        || (matches!(schema, ProviderSchemaDeclaration::BoundaryRequirement(_))
            && provider_planning::IntrinsicRequirement::by_symbol(
                &checked.typed,
                requirement_symbol,
            )
            .is_some_and(|requirement| {
                requirement.kind == provider_planning::IntrinsicRequirementKind::TopLevelRequirement
            }))
    {
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
    if hosted_console_exit_row(
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
            CompilerIntrinsicExecutionIdentity::HostedExitProcessI32,
        )));
    }
    if hosted_process_exit_row(
        checked,
        plan,
        row,
        trait_symbol,
        requirement_symbol,
        realization_symbol,
        selected_target,
        accepted_process_exit_binding,
        accepted_process_exit_declaration_symbol,
    )? {
        return Ok(Some(SelectedCompilerIntrinsicExecutionIdentity::Closed(
            CompilerIntrinsicExecutionIdentity::HostedExitProcessI32,
        )));
    }
    if hosted_console_write_byte_row(
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
            CompilerIntrinsicExecutionIdentity::HostedWriteByteI32,
        )));
    }
    if hosted_console_read_byte_row(
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
            CompilerIntrinsicExecutionIdentity::HostedReadByte,
        )));
    }
    Ok(Some(
        SelectedCompilerIntrinsicExecutionIdentity::Unsupported,
    ))
}

fn hosted_console_write_byte_row(
    checked: &CheckedTrees,
    plan: &ProviderPlan,
    row: &ProviderPlanRow,
    trait_symbol: SymbolHandle,
    requirement_symbol: SymbolHandle,
    realization_symbol: SymbolHandle,
    selected_target: Option<&str>,
    accepted_binding: Option<&package_compilation::AcceptedSemanticBinding>,
    accepted_declaration_symbol: Option<SymbolHandle>,
) -> Result<bool, Diagnostic> {
    console_row_for_targets(
        checked,
        plan,
        row,
        trait_symbol,
        requirement_symbol,
        realization_symbol,
        selected_target,
        accepted_binding,
        accepted_declaration_symbol,
        &["linux_x86_64", "linux_arm64", "macos_arm64"],
        "write_byte",
        "ConsoleNativeProvider::write_byte",
        ConsoleIntrinsicShape::I32ToUnit,
    )
}

fn hosted_console_read_byte_row(
    checked: &CheckedTrees,
    plan: &ProviderPlan,
    row: &ProviderPlanRow,
    trait_symbol: SymbolHandle,
    requirement_symbol: SymbolHandle,
    realization_symbol: SymbolHandle,
    selected_target: Option<&str>,
    accepted_binding: Option<&package_compilation::AcceptedSemanticBinding>,
    accepted_declaration_symbol: Option<SymbolHandle>,
) -> Result<bool, Diagnostic> {
    console_row_for_targets(
        checked,
        plan,
        row,
        trait_symbol,
        requirement_symbol,
        realization_symbol,
        selected_target,
        accepted_binding,
        accepted_declaration_symbol,
        &["linux_x86_64", "linux_arm64", "macos_arm64"],
        "read_byte",
        "ConsoleNativeProvider::read_byte",
        ConsoleIntrinsicShape::UnitToByteRead,
    )
}

fn hosted_console_exit_row(
    checked: &CheckedTrees,
    plan: &ProviderPlan,
    row: &ProviderPlanRow,
    trait_symbol: SymbolHandle,
    requirement_symbol: SymbolHandle,
    realization_symbol: SymbolHandle,
    selected_target: Option<&str>,
    accepted_binding: Option<&package_compilation::AcceptedSemanticBinding>,
    accepted_declaration_symbol: Option<SymbolHandle>,
) -> Result<bool, Diagnostic> {
    console_row_for_targets(
        checked,
        plan,
        row,
        trait_symbol,
        requirement_symbol,
        realization_symbol,
        selected_target,
        accepted_binding,
        accepted_declaration_symbol,
        &["linux_x86_64", "linux_arm64", "macos_arm64"],
        "exit_process",
        "ConsoleNativeProvider::exit_process",
        ConsoleIntrinsicShape::I32ToUnit,
    )
}

#[derive(Clone, Copy)]
enum ConsoleIntrinsicShape {
    I32ToUnit,
    UnitToByteRead,
}

#[allow(clippy::too_many_arguments)]
fn console_row_for_targets(
    checked: &CheckedTrees,
    plan: &ProviderPlan,
    row: &ProviderPlanRow,
    trait_symbol: SymbolHandle,
    requirement_symbol: SymbolHandle,
    realization_symbol: SymbolHandle,
    selected_target: Option<&str>,
    accepted_binding: Option<&package_compilation::AcceptedSemanticBinding>,
    accepted_declaration_symbol: Option<SymbolHandle>,
    supported_targets: &[&str],
    requirement_name: &str,
    realization_name: &str,
    shape: ConsoleIntrinsicShape,
) -> Result<bool, Diagnostic> {
    let Some(selected_target) = selected_target.filter(|target| supported_targets.contains(target))
    else {
        return Ok(false);
    };
    if plan.target != selected_target {
        return Ok(false);
    }
    let typed = &checked.typed;
    let legacy_bundled_binding = exact_bundled_console_binding(
        typed,
        trait_symbol,
        requirement_symbol,
        realization_symbol,
        selected_target,
    );
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
    if !legacy_bundled_binding && !accepted_package_binding {
        return Ok(false);
    }

    boundary_row_shape(
        checked,
        plan,
        row,
        trait_symbol,
        requirement_symbol,
        realization_symbol,
        "Console",
        matches!(shape, ConsoleIntrinsicShape::I32ToUnit),
        requirement_name,
        realization_name,
        shape,
    )
}

fn exact_bundled_console_binding(
    typed: &typed_trees::TypedTrees,
    trait_symbol: SymbolHandle,
    requirement_symbol: SymbolHandle,
    realization_symbol: SymbolHandle,
    selected_target: &str,
) -> bool {
    const CONSOLE: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../source/library/std/console.omg"
    ));
    const LINUX_X64: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../source/library/std/targets/linux_x86_64/console_impl.omg"
    ));
    const LINUX_ARM64: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../source/library/std/targets/linux_arm64/console_impl.omg"
    ));
    const MACOS_ARM64: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../source/library/std/targets/macos_arm64/console_impl.omg"
    ));
    let (realization_path, realization_source) = match selected_target {
        "linux_x86_64" => ("targets/linux_x86_64/console_impl.omg", LINUX_X64),
        "linux_arm64" => ("targets/linux_arm64/console_impl.omg", LINUX_ARM64),
        "macos_arm64" => ("targets/macos_arm64/console_impl.omg", MACOS_ARM64),
        _ => return false,
    };
    exact_bundled_standalone_source(typed, trait_symbol, "console.omg", CONSOLE)
        && exact_bundled_standalone_source(typed, requirement_symbol, "console.omg", CONSOLE)
        && exact_bundled_standalone_source(
            typed,
            realization_symbol,
            realization_path,
            realization_source,
        )
}

fn exact_bundled_standalone_source(
    typed: &typed_trees::TypedTrees,
    symbol: SymbolHandle,
    expected_relative_path: &str,
    expected_source: &[u8],
) -> bool {
    typed
        .symbols
        .symbol_source_span(symbol)
        .and_then(|span| typed.symbols.source_file(span))
        .is_some_and(|source| {
            source.package_identity.is_none()
                && source.path.strip_prefix(&source.package_root).ok()
                    == Some(std::path::Path::new(expected_relative_path))
                && source.source.as_bytes() == expected_source
        })
}

fn hosted_process_exit_row(
    checked: &CheckedTrees,
    plan: &ProviderPlan,
    row: &ProviderPlanRow,
    trait_symbol: SymbolHandle,
    requirement_symbol: SymbolHandle,
    realization_symbol: SymbolHandle,
    selected_target: Option<&str>,
    accepted_binding: Option<&package_compilation::AcceptedSemanticBinding>,
    accepted_declaration_symbol: Option<SymbolHandle>,
) -> Result<bool, Diagnostic> {
    process_exit_row_for_targets(
        checked,
        plan,
        row,
        trait_symbol,
        requirement_symbol,
        realization_symbol,
        selected_target,
        accepted_binding,
        accepted_declaration_symbol,
        &["linux_x86_64", "linux_arm64", "macos_arm64"],
    )
}

/// Canonical core `ProcessExit::exit_process` rows. The exact toolchain-owned
/// requirement identity is checked against the bundled core source (or, for
/// package-aware consumers, against the accepted semantic binding); only the
/// exact `ProcessExitNativeProvider::exit_process` realization closes as the
/// hosted process-exit builtin.
#[allow(clippy::too_many_arguments)]
fn process_exit_row_for_targets(
    checked: &CheckedTrees,
    plan: &ProviderPlan,
    row: &ProviderPlanRow,
    trait_symbol: SymbolHandle,
    requirement_symbol: SymbolHandle,
    realization_symbol: SymbolHandle,
    selected_target: Option<&str>,
    accepted_binding: Option<&package_compilation::AcceptedSemanticBinding>,
    accepted_declaration_symbol: Option<SymbolHandle>,
    supported_targets: &[&str],
) -> Result<bool, Diagnostic> {
    let Some(selected_target) = selected_target.filter(|target| supported_targets.contains(target))
    else {
        return Ok(false);
    };
    if plan.target != selected_target {
        return Ok(false);
    }
    let typed = &checked.typed;
    let legacy_bundled_binding = exact_bundled_process_exit_binding(
        typed,
        trait_symbol,
        requirement_symbol,
        realization_symbol,
        selected_target,
    );
    let accepted_package_binding = accepted_binding.is_some_and(|binding| {
        accepted_binding_matches_process_exit_row_identity(
            checked,
            plan,
            trait_symbol,
            requirement_symbol,
            realization_symbol,
            binding,
        ) && accepted_declaration_symbol.is_none_or(|symbol| symbol == trait_symbol)
    });
    if !legacy_bundled_binding && !accepted_package_binding {
        return Ok(false);
    }

    boundary_row_shape(
        checked,
        plan,
        row,
        trait_symbol,
        requirement_symbol,
        realization_symbol,
        "ProcessExit",
        true,
        "exit_process",
        "ProcessExitNativeProvider::exit_process",
        ConsoleIntrinsicShape::I32ToUnit,
    )
}

fn exact_bundled_process_exit_binding(
    typed: &typed_trees::TypedTrees,
    trait_symbol: SymbolHandle,
    requirement_symbol: SymbolHandle,
    realization_symbol: SymbolHandle,
    selected_target: &str,
) -> bool {
    const CORE: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../source/library/core/process_exit.omg"
    ));
    const LINUX_X64: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../source/library/std/targets/linux_x86_64/process_exit_impl.omg"
    ));
    const LINUX_ARM64: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../source/library/std/targets/linux_arm64/process_exit_impl.omg"
    ));
    const MACOS_ARM64: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../source/library/std/targets/macos_arm64/process_exit_impl.omg"
    ));
    let (realization_path, realization_source) = match selected_target {
        "linux_x86_64" => ("targets/linux_x86_64/process_exit_impl.omg", LINUX_X64),
        "linux_arm64" => ("targets/linux_arm64/process_exit_impl.omg", LINUX_ARM64),
        "macos_arm64" => ("targets/macos_arm64/process_exit_impl.omg", MACOS_ARM64),
        _ => return false,
    };
    exact_bundled_standalone_source(typed, trait_symbol, "process_exit.omg", CORE)
        && exact_bundled_standalone_source(typed, requirement_symbol, "process_exit.omg", CORE)
        && exact_bundled_standalone_source(
            typed,
            realization_symbol,
            realization_path,
            realization_source,
        )
}

/// Exact package-consumer identity for a canonical `ProcessExit` row. The
/// toolchain-owned trait and requirement carry no package identity; the
/// bound package owns only the provider nominal and its plan.
fn accepted_binding_matches_process_exit_row_identity(
    checked: &CheckedTrees,
    plan: &ProviderPlan,
    trait_symbol: SymbolHandle,
    requirement_symbol: SymbolHandle,
    realization_symbol: SymbolHandle,
    binding: &package_compilation::AcceptedSemanticBinding,
) -> bool {
    let typed = &checked.typed;
    binding.role() == package_compilation::AcceptedSemanticBindingRole::ProcessExitExitProcessI32
        && typed
            .symbols
            .symbol_package_identity(trait_symbol)
            .is_none()
        && typed
            .symbols
            .symbol_package_identity(requirement_symbol)
            .is_none()
        && typed.symbols.symbol_package_identity(realization_symbol) == Some(binding.package())
        && plan.schema.trait_package_identity.is_none()
        && plan.provider_type_package_identity == Some(binding.package())
        && plan.origin_package_identity == Some(binding.package())
        && typed.symbols.display_path(trait_symbol, "::") == binding.declaration_path()
        && plan.schema.identity_digest() == binding.normalized_schema_digest()
        && binding.selected_provider_plan_digest() == Some(plan.identity_digest())
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
    binding: &package_compilation::AcceptedSemanticBinding,
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
    boundary_row_shape(
        checked,
        plan,
        row,
        trait_symbol,
        requirement_symbol,
        realization_symbol,
        "Console",
        false,
        "exit_process",
        "ConsoleNativeProvider::exit_process",
        ConsoleIntrinsicShape::I32ToUnit,
    )
}

/// Rejoin the canonical core `ProcessExit` requirement to one exact
/// package-owned selected row. The toolchain-owned trait carries no package
/// identity; the bound package owns the provider nominal.
pub(crate) fn accepted_binding_matches_process_exit_i32_row(
    checked: &CheckedTrees,
    plan: &ProviderPlan,
    row: &ProviderPlanRow,
    trait_symbol: SymbolHandle,
    requirement_symbol: SymbolHandle,
    realization_symbol: SymbolHandle,
    binding: &package_compilation::AcceptedSemanticBinding,
) -> Result<bool, Diagnostic> {
    if !accepted_binding_matches_process_exit_row_identity(
        checked,
        plan,
        trait_symbol,
        requirement_symbol,
        realization_symbol,
        binding,
    ) {
        return Ok(false);
    }
    boundary_row_shape(
        checked,
        plan,
        row,
        trait_symbol,
        requirement_symbol,
        realization_symbol,
        "ProcessExit",
        false,
        "exit_process",
        "ProcessExitNativeProvider::exit_process",
        ConsoleIntrinsicShape::I32ToUnit,
    )
}

#[allow(clippy::too_many_arguments)]
fn boundary_row_shape(
    checked: &CheckedTrees,
    plan: &ProviderPlan,
    row: &ProviderPlanRow,
    trait_symbol: SymbolHandle,
    requirement_symbol: SymbolHandle,
    realization_symbol: SymbolHandle,
    trait_name: &str,
    require_inferred_supply: bool,
    requirement_name: &str,
    realization_name: &str,
    shape: ConsoleIntrinsicShape,
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
        || definition.name.as_str() != trait_name
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
        || method.name != requirement_name
        || method.requirement_owner != definition.name.as_str()
        || method.requirement_identity != requirement_identity
        || row.requirement_identity != requirement_identity
        || !exact_console_signature(typed, requirement, requirement_name, trait_symbol, shape)
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
    // The realization's published envelope must spell the same honest shape
    // as its requirement: the hosted byte-input leaf may occupy the worker
    // while it waits and traps on a failed read; byte output and process
    // exit keep their nonblocking, crash-free ceiling on this leg.
    let expect_blocking_trap = matches!(shape, ConsoleIntrinsicShape::UnitToByteRead);
    if realization.suspends
        || realization.blocks != expect_blocking_trap
        || !exact_realization_contracts(typed, realization, expect_blocking_trap)
    {
        return Ok(false);
    }
    if realization.name.as_str() != realization_name
        || machine != &realization_identity
        || !realization.lifetime_parameters.is_empty()
        || !typed.machine_type_parameters(realization).is_empty()
        || realization.body_is_present
    {
        return Ok(false);
    }
    let inferred_supply = realization.supply_mode == MachineSupplyMode::Boundary;
    let legacy_binding = match realization.supply_mode {
        MachineSupplyMode::ExternalRealization {
            binding: Some(binding),
            mechanism: Some(language_semantics::ExternalBindingMechanism::CompilerIntrinsic),
        } if typed.external_bindings.identity(binding)
            == Some(&language_semantics::ExternalBindingIdentity::CompilerIntrinsic) =>
        {
            Some(binding)
        }
        _ => None,
    };
    if !inferred_supply && (legacy_binding.is_none() || require_inferred_supply) {
        return Ok(false);
    }
    let [entry] = typed.machine_states(realization) else {
        return Ok(false);
    };
    if !exact_console_state(typed, entry, trait_symbol, shape) {
        return Ok(false);
    }
    let conformances = typed
        .machine_trait_conformances(realization)
        .iter()
        .filter(|conformance| {
            conformance.symbol == trait_symbol
                && conformance.requirement_symbol == requirement_symbol
                && conformance.requirement.as_ref().map(|name| name.as_str())
                    == Some(requirement_name)
                && ((inferred_supply
                    && conformance.external_binding.is_none()
                    && !conformance.via_expression.is_valid()
                    && conformance.external_binding_source_span.is_none())
                    || (!require_inferred_supply
                        && conformance.external_binding == legacy_binding
                        && !conformance.via_expression.is_valid()
                        && conformance.external_binding_source_span.is_some()))
                && typed_trees::machine::resolve_satisfied_declaration(
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
    binding: &package_compilation::AcceptedSemanticBinding,
) -> bool {
    let typed = &checked.typed;
    binding.role() == package_compilation::AcceptedSemanticBindingRole::ConsoleExitProcessI32
        && [trait_symbol, requirement_symbol, realization_symbol]
            .into_iter()
            .all(|symbol| typed.symbols.symbol_package_identity(symbol) == Some(binding.package()))
        && plan.schema.trait_package_identity == Some(binding.package())
        && plan.provider_type_package_identity == Some(binding.package())
        && plan.origin_package_identity == Some(binding.package())
        && typed.symbols.display_path(trait_symbol, "::") == binding.declaration_path()
        && plan.schema.identity_digest() == binding.normalized_schema_digest()
        && binding.selected_provider_plan_digest() == Some(plan.identity_digest())
}

fn exact_console_signature(
    typed: &typed_trees::TypedTrees,
    signature: &typed_trees::signature::StateSignature,
    requirement_name: &str,
    trait_symbol: SymbolHandle,
    shape: ConsoleIntrinsicShape,
) -> bool {
    if signature.name.as_str() != requirement_name
        || !signature.lifetime_parameters.is_empty()
        || !typed.state_signature_type_parameters(signature).is_empty()
        || !signature.native_callback_parameters.is_empty()
        || signature.suspends
    {
        return false;
    }
    match shape {
        ConsoleIntrinsicShape::I32ToUnit => {
            // Hosted byte output and process exit still publish a nonblocking,
            // non-crashing ceiling; this leg does not widen their envelope.
            !signature.blocks
                && exact_i32_parameter(typed, typed.state_signature_parameters(signature))
                && matches!(
                    typed
                        .type_reference_table
                        .type_reference(signature.return_type),
                    TypeReferenceNode::Unit
                )
        }
        ConsoleIntrinsicShape::UnitToByteRead => {
            // Hosted input may occupy the worker while it waits for a byte and
            // traps on a failed host read: the exact `read_byte` identity is the
            // requirement that publishes `blocks;` plus one unconditional
            // `crashes Trap` route. Omitting either still closes nothing here;
            // callers keep acknowledging `block` and covering `Trap` honestly.
            signature.blocks
                && exact_unconditional_trap_crash_contract(typed, signature)
                && typed.state_signature_parameters(signature).is_empty()
                && exact_byte_read_type(typed, signature.return_type, trait_symbol)
        }
    }
}

/// The realization publishes the same envelope as its requirement: exactly
/// one unconditional `crashes Trap` contract on the hosted byte-input leaf and
/// no contracts at all on the nonblocking output/exit leaves.
fn exact_realization_contracts(
    typed: &typed_trees::TypedTrees,
    realization: &typed_trees::machine::Machine,
    expect_unconditional_trap: bool,
) -> bool {
    let contracts = typed.machine_contracts(realization);
    if !expect_unconditional_trap {
        return contracts.is_empty();
    }
    let [contract] = contracts else {
        return false;
    };
    matches!(
        contract.kind,
        typed_trees::signature::SignatureContractKind::Crashes {
            cause: typed_trees::signature::CrashCause::Trap,
        }
    ) && contract.facts.is_empty()
        && contract.binding.is_none()
}

/// The hosted byte-input failure contract is exactly one `crashes Trap` clause
/// whose route list is empty (the published `true` route), and no other
/// requires/ensures/crash contract on the signature.
fn exact_unconditional_trap_crash_contract(
    typed: &typed_trees::TypedTrees,
    signature: &typed_trees::signature::StateSignature,
) -> bool {
    let contracts = typed.state_signature_contracts(signature);
    let [contract] = contracts else {
        return false;
    };
    matches!(
        contract.kind,
        typed_trees::signature::SignatureContractKind::Crashes {
            cause: typed_trees::signature::CrashCause::Trap,
        }
    ) && contract.facts.is_empty()
        && contract.binding.is_none()
}

fn exact_console_state(
    typed: &typed_trees::TypedTrees,
    state: &typed_trees::state::State,
    trait_symbol: SymbolHandle,
    shape: ConsoleIntrinsicShape,
) -> bool {
    match shape {
        ConsoleIntrinsicShape::I32ToUnit => {
            exact_i32_parameter(typed, typed.state_parameters(state))
                && matches!(
                    typed.type_reference_table.type_reference(state.return_type),
                    TypeReferenceNode::Unit
                )
        }
        ConsoleIntrinsicShape::UnitToByteRead => {
            typed.state_parameters(state).is_empty()
                && exact_byte_read_type(typed, state.return_type, trait_symbol)
        }
    }
}

fn exact_byte_read_type(
    typed: &typed_trees::TypedTrees,
    type_reference: typed_trees::types::TypeReferenceHandle,
    trait_symbol: SymbolHandle,
) -> bool {
    let TypeReferenceNode::Named { symbol, name } =
        typed.type_reference_table.type_reference(type_reference)
    else {
        return false;
    };
    name.as_str() == "ByteRead"
        && match typed.symbols.symbol_package_identity(trait_symbol) {
            Some(package) => typed.symbols.symbol_package_identity(*symbol) == Some(package),
            None => exact_bundled_standalone_source(
                typed,
                *symbol,
                "console.omg",
                include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../../../source/library/std/console.omg"
                )),
            ),
        }
}

fn exact_i32_parameter(
    typed: &typed_trees::TypedTrees,
    parameters: &[typed_trees::signature::StateParameter],
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
