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
use target::HostedIntrinsicBundle;
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
    accepted_console_binding: Option<&crate::ResolvedAcceptedSemanticBinding>,
    accepted_process_exit_binding: Option<&crate::ResolvedAcceptedSemanticBinding>,
) -> Result<Option<SelectedCompilerIntrinsicExecutionIdentity>, Diagnostic> {
    row_execution_identity(
        checked,
        plan,
        schema,
        row,
        requirement_symbol,
        realization_symbol,
        selected_target,
        accepted_console_binding.map(AcceptedRowBinding::resolved),
        accepted_process_exit_binding.map(AcceptedRowBinding::resolved),
    )
}

/// A settled package-consumer binding offered to one row, with the service
/// declaration it resolved to once review consumed it.
#[derive(Clone, Copy)]
pub(crate) struct AcceptedRowBinding<'binding> {
    pub(crate) accepted: &'binding package_compilation::AcceptedSemanticBinding,
    pub(crate) declaration_symbol: Option<SymbolHandle>,
}

impl<'binding> AcceptedRowBinding<'binding> {
    fn resolved(binding: &'binding crate::ResolvedAcceptedSemanticBinding) -> Self {
        Self {
            accepted: binding.accepted(),
            declaration_symbol: Some(binding.declaration_symbol()),
        }
    }
}

/// The row's execution identity. Review calls this before any binding has
/// resolved its declaration; compilation calls it through the public entry.
#[allow(clippy::too_many_arguments)]
pub(crate) fn row_execution_identity(
    checked: &CheckedTrees,
    plan: &ProviderPlan,
    schema: ProviderSchemaDeclaration,
    row: &ProviderPlanRow,
    requirement_symbol: SymbolHandle,
    realization_symbol: SymbolHandle,
    selected_target: Option<&str>,
    accepted_console_binding: Option<AcceptedRowBinding<'_>>,
    accepted_process_exit_binding: Option<AcceptedRowBinding<'_>>,
) -> Result<Option<SelectedCompilerIntrinsicExecutionIdentity>, Diagnostic> {
    if !matches!(row.binding, ProviderBinding::CompilerIntrinsic { .. }) {
        return Ok(None);
    }
    // The float intrinsic catalog is keyed on the requirement view: a named
    // boundary operator or a top-level `boundary requirement` machine.
    if matches!(schema, ProviderSchemaDeclaration::BoundaryOperator(_))
        || (matches!(schema, ProviderSchemaDeclaration::BoundaryRequirement(_))
            && provider_planning::IntrinsicRequirement::by_symbol(checked, requirement_symbol)
                .is_some_and(|requirement| {
                    requirement.kind
                        == provider_planning::IntrinsicRequirementKind::TopLevelRequirement
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
    let Some((selected_target, bundle)) = selected_hosted_bundle(selected_target) else {
        return Ok(Some(
            SelectedCompilerIntrinsicExecutionIdentity::Unsupported,
        ));
    };
    if plan.target == selected_target {
        for hosted in &HOSTED_INTRINSIC_ROWS {
            let accepted = match hosted.service {
                HostedService::Console => accepted_console_binding,
                HostedService::ProcessExit => accepted_process_exit_binding,
            };
            if hosted.closes(
                checked,
                plan,
                row,
                trait_symbol,
                requirement_symbol,
                realization_symbol,
                bundle,
                accepted,
            )? {
                return Ok(Some(SelectedCompilerIntrinsicExecutionIdentity::Closed(
                    hosted.identity,
                )));
            }
        }
    }
    Ok(Some(
        SelectedCompilerIntrinsicExecutionIdentity::Unsupported,
    ))
}

/// A standard-library service whose hosted rows the compiler realizes itself.
#[derive(Clone, Copy)]
pub(crate) enum HostedService {
    Console,
    ProcessExit,
}

impl HostedService {
    /// This service's exit-process row in [`HOSTED_INTRINSIC_ROWS`].
    fn exit_process_row(self) -> &'static HostedIntrinsicRow {
        match self {
            Self::Console => &HOSTED_INTRINSIC_ROWS[0],
            Self::ProcessExit => &HOSTED_INTRINSIC_ROWS[1],
        }
    }

    fn trait_name(self) -> &'static str {
        match self {
            Self::Console => "Console",
            Self::ProcessExit => "ProcessExit",
        }
    }

    /// Whether the bundled toolchain sources own the trait, requirement and
    /// the selected bundle's realization.
    fn bundled_binding(
        self,
        typed: &typed_trees::TypedTrees,
        trait_symbol: SymbolHandle,
        requirement_symbol: SymbolHandle,
        realization_symbol: SymbolHandle,
        bundle: HostedIntrinsicBundle,
    ) -> bool {
        let (service_file, service_source, realization_file, realization_source) = match self {
            Self::Console => (
                "console.omg",
                BUNDLED_CONSOLE,
                bundle.console_source(),
                bundled_console_source(bundle),
            ),
            Self::ProcessExit => (
                "process_exit.omg",
                BUNDLED_PROCESS_EXIT,
                bundle.process_exit_source(),
                bundled_process_exit_source(bundle),
            ),
        };
        exact_bundled_standalone_source(typed, trait_symbol, service_file, service_source)
            && exact_bundled_standalone_source(
                typed,
                requirement_symbol,
                service_file,
                service_source,
            )
            && exact_bundled_standalone_source(
                typed,
                realization_symbol,
                realization_file,
                realization_source,
            )
    }

    /// Whether one settled package-consumer binding owns the selected row.
    fn accepted_binding_matches(
        self,
        checked: &CheckedTrees,
        plan: &ProviderPlan,
        trait_symbol: SymbolHandle,
        requirement_symbol: SymbolHandle,
        realization_symbol: SymbolHandle,
        binding: &package_compilation::AcceptedSemanticBinding,
    ) -> bool {
        match self {
            Self::Console => accepted_binding_matches_selected_row_identity(
                checked,
                plan,
                trait_symbol,
                requirement_symbol,
                realization_symbol,
                binding,
            ),
            Self::ProcessExit => accepted_binding_matches_process_exit_row_identity(
                checked,
                plan,
                trait_symbol,
                requirement_symbol,
                realization_symbol,
                binding,
            ),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ConsoleIntrinsicShape {
    I32ToUnit,
    UnitToByteRead,
}

/// One hosted row the compiler realizes itself: the service requirement and
/// realization it binds, their exact signature shape, and the execution
/// identity it closes as.
struct HostedIntrinsicRow {
    service: HostedService,
    requirement: &'static str,
    realization: &'static str,
    shape: ConsoleIntrinsicShape,
    identity: CompilerIntrinsicExecutionIdentity,
}

/// Hosted rows in recognition order; the first row a selected row matches
/// decides its identity. Only a selected target that names an admitted
/// [`HostedIntrinsicBundle`] earns any of them.
const HOSTED_INTRINSIC_ROWS: [HostedIntrinsicRow; 4] = [
    HostedIntrinsicRow {
        service: HostedService::Console,
        requirement: "exit_process",
        realization: "ConsoleNativeProvider::exit_process",
        shape: ConsoleIntrinsicShape::I32ToUnit,
        identity: CompilerIntrinsicExecutionIdentity::HostedExitProcessI32,
    },
    HostedIntrinsicRow {
        service: HostedService::ProcessExit,
        requirement: "exit_process",
        realization: "ProcessExitNativeProvider::exit_process",
        shape: ConsoleIntrinsicShape::I32ToUnit,
        identity: CompilerIntrinsicExecutionIdentity::HostedExitProcessI32,
    },
    HostedIntrinsicRow {
        service: HostedService::Console,
        requirement: "write_byte",
        realization: "ConsoleNativeProvider::write_byte",
        shape: ConsoleIntrinsicShape::I32ToUnit,
        identity: CompilerIntrinsicExecutionIdentity::HostedWriteByteI32,
    },
    HostedIntrinsicRow {
        service: HostedService::Console,
        requirement: "read_byte",
        realization: "ConsoleNativeProvider::read_byte",
        shape: ConsoleIntrinsicShape::UnitToByteRead,
        identity: CompilerIntrinsicExecutionIdentity::HostedReadByte,
    },
];

impl HostedIntrinsicRow {
    /// The selected row closes as this hosted row when the bundled toolchain
    /// sources or the offered consumer binding own it and its declarations
    /// have this row's exact shape. A resultless hosted row also requires
    /// inferred supply; byte input keeps its authored crash envelope.
    #[allow(clippy::too_many_arguments)]
    fn closes(
        &self,
        checked: &CheckedTrees,
        plan: &ProviderPlan,
        row: &ProviderPlanRow,
        trait_symbol: SymbolHandle,
        requirement_symbol: SymbolHandle,
        realization_symbol: SymbolHandle,
        bundle: HostedIntrinsicBundle,
        accepted: Option<AcceptedRowBinding<'_>>,
    ) -> Result<bool, Diagnostic> {
        let bundled = self.service.bundled_binding(
            checked,
            trait_symbol,
            requirement_symbol,
            realization_symbol,
            bundle,
        );
        let consumed = accepted.is_some_and(|binding| {
            self.service.accepted_binding_matches(
                checked,
                plan,
                trait_symbol,
                requirement_symbol,
                realization_symbol,
                binding.accepted,
            ) && binding
                .declaration_symbol
                .is_none_or(|symbol| symbol == trait_symbol)
        });
        if !bundled && !consumed {
            return Ok(false);
        }
        boundary_row_shape(
            checked,
            plan,
            row,
            trait_symbol,
            requirement_symbol,
            realization_symbol,
            self.service.trait_name(),
            self.shape == ConsoleIntrinsicShape::I32ToUnit,
            self.requirement,
            self.realization,
            self.shape,
        )
    }
}

const BUNDLED_CONSOLE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../source/library/std/console.omg"
));

const BUNDLED_PROCESS_EXIT: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../source/library/core/process_exit.omg"
));

/// The selected canonical target together with the hosted bundle it admits.
fn selected_hosted_bundle(selected_target: Option<&str>) -> Option<(&str, HostedIntrinsicBundle)> {
    let selected_target = selected_target?;
    let bundle = HostedIntrinsicBundle::from_target_name(selected_target)?;
    Some((selected_target, bundle))
}

macro_rules! bundled_intrinsic_source {
    ($name:ident, $($variant:ident => $file:literal),+ $(,)?) => {
        fn $name(bundle: HostedIntrinsicBundle) -> &'static [u8] {
            match bundle {
                $(
                    HostedIntrinsicBundle::$variant => {
                        const BYTES: &[u8] = include_bytes!(concat!(
                            env!("CARGO_MANIFEST_DIR"),
                            "/../../../../../source/library/std/targets/",
                            $file
                        ));
                        BYTES
                    }
                )+
            }
        }
    };
}

// Compile-time copy of one admitted bundle's console provider closure. The
// path each constant reads is the bundle's `console_source()`; the test
// module holds the two in agreement with the checked-in file.
bundled_intrinsic_source!(
    bundled_console_source,
    LinuxArm64 => "linux_arm64/console_impl.omg",
    LinuxX64 => "linux_x86_64/console_impl.omg",
    MacosArm64 => "macos_arm64/console_impl.omg",
    MacosX64 => "macos_x86_64/console_impl.omg",
);

// Compile-time copy of one admitted bundle's process-exit provider closure,
// paired with `process_exit_source()` the same way as the console copy.
bundled_intrinsic_source!(
    bundled_process_exit_source,
    LinuxArm64 => "linux_arm64/process_exit_impl.omg",
    LinuxX64 => "linux_x86_64/process_exit_impl.omg",
    MacosArm64 => "macos_arm64/process_exit_impl.omg",
    MacosX64 => "macos_x86_64/process_exit_impl.omg",
);

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
            source.origin == source::SourceOrigin::Toolchain
                && source.package_identity.is_none()
                && source.path.strip_prefix(&source.package_root).ok()
                    == Some(std::path::Path::new(expected_relative_path))
                && source.source.as_bytes() == expected_source
        })
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
    let typed = checked;
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

/// Rejoin one service's exit-process semantic role to one exact package-owned
/// selected row. For Console this recognizes Process authority; the
/// toolchain-owned ProcessExit trait carries no package identity and the
/// bound package owns the provider nominal. Neither claims that the selected
/// target has a closed compiler lowering, so inferred supply is not required.
pub(crate) fn accepted_binding_matches_exit_process_row(
    service: HostedService,
    checked: &CheckedTrees,
    plan: &ProviderPlan,
    row: &ProviderPlanRow,
    trait_symbol: SymbolHandle,
    requirement_symbol: SymbolHandle,
    realization_symbol: SymbolHandle,
    binding: &package_compilation::AcceptedSemanticBinding,
) -> Result<bool, Diagnostic> {
    if !service.accepted_binding_matches(
        checked,
        plan,
        trait_symbol,
        requirement_symbol,
        realization_symbol,
        binding,
    ) {
        return Ok(false);
    }
    let hosted = service.exit_process_row();
    boundary_row_shape(
        checked,
        plan,
        row,
        trait_symbol,
        requirement_symbol,
        realization_symbol,
        service.trait_name(),
        false,
        hosted.requirement,
        hosted.realization,
        hosted.shape,
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
    let typed = checked;
    let Some(definition) = typed.trait_definition_by_symbol(trait_symbol) else {
        return Ok(false);
    };
    let Some(requirement) = typed.trait_machine_signature_by_symbol(definition, requirement_symbol)
    else {
        return Ok(false);
    };
    let requirement_identity =
        typed.trait_requirement_callable_identity_text(definition, requirement);
    let methods = plan
        .schema
        .methods
        .iter()
        .filter(|method| plan.schema.row_binds_method(row, method, &plan.identities))
        .collect::<Vec<_>>();
    let [method] = methods.as_slice() else {
        return Err(Diagnostic::error(format!(
            "selected ProviderPlan `{}` compiler-intrinsic row rejoins {} schema methods",
            plan.name,
            methods.len(),
        )));
    };
    if !provider_planning::service_schema::schema_binds_exact_boundary_trait(
        provider_planning::ProviderPlanningContext::new(typed, source::DependencyScope::Product),
        &plan.schema,
        definition,
    ) || method.name != requirement_name
        || method.requirement_owner != typed.trait_declaration_path(definition)
        || plan.schema.identity_text(method.requirement_identity) != requirement_identity
        || plan.identities.text(row.requirement_identity) != requirement_identity
        || !exact_console_signature(typed, requirement, requirement_name, trait_symbol, shape)
    {
        return Ok(false);
    }

    let intrinsic_machine = match &row.binding {
        ProviderBinding::CompilerIntrinsic { machine } => Some(machine),
        ProviderBinding::Import { .. } => None,
        _ => return Ok(false),
    };
    boundary_declaration_shape(
        checked,
        trait_symbol,
        requirement_symbol,
        realization_symbol,
        trait_name,
        require_inferred_supply,
        requirement_name,
        realization_name,
        shape,
        intrinsic_machine.map(String::as_str),
    )
}

/// The original declaration relation is shared by selected rows and direct
/// compiler supply. A plan binds this relation; it never creates its custody.
#[allow(clippy::too_many_arguments)]
fn boundary_declaration_shape(
    checked: &CheckedTrees,
    trait_symbol: SymbolHandle,
    requirement_symbol: SymbolHandle,
    realization_symbol: SymbolHandle,
    trait_name: &str,
    require_inferred_supply: bool,
    requirement_name: &str,
    realization_name: &str,
    shape: ConsoleIntrinsicShape,
    intrinsic_machine: Option<&str>,
) -> Result<bool, Diagnostic> {
    let typed = checked;

    let traits = typed
        .traits()
        .iter()
        .filter(|definition| definition.symbol == trait_symbol)
        .collect::<Vec<_>>();
    let [definition] = traits.as_slice() else {
        return Err(Diagnostic::error(format!(
            "hosted intrinsic resolves its exact service symbol to {} trait declarations",
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
            "hosted intrinsic resolves its exact requirement symbol to {} trait signatures",
            requirements.len(),
        )));
    };
    if !exact_console_signature(typed, requirement, requirement_name, trait_symbol, shape) {
        return Ok(false);
    }
    let realizations = typed
        .machines()
        .iter()
        .filter(|machine| machine.symbol == realization_symbol)
        .collect::<Vec<_>>();
    let [realization] = realizations.as_slice() else {
        return Err(Diagnostic::error(format!(
            "hosted intrinsic resolves its exact realization symbol to {} machines",
            realizations.len(),
        )));
    };
    let Some(realization_identity) = typed.machine_callable_identity_text(realization) else {
        return Ok(false);
    };
    // A hosted console leaf is either compiler-known (the intrinsic names the
    // realization's own identity) or, on a target whose kernel entry is DLL
    // linkage, an evaluated import the realization reaches through `via`.
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
        || intrinsic_machine.is_some_and(|machine| machine != realization_identity)
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
    let evaluated_import = intrinsic_machine.is_none()
        && matches!(
            realization.supply_mode,
            MachineSupplyMode::ExternalRealization { .. }
        );
    if require_inferred_supply && !inferred_supply {
        return Ok(false);
    }
    if !inferred_supply && legacy_binding.is_none() && !evaluated_import {
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
                        && conformance.external_binding_source_span.is_some())
                    || (evaluated_import
                        && conformance.external_binding.is_none()
                        && conformance.via_expression.is_valid()))
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
    let typed = checked;
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
    sole_unconditional_trap(contracts)
}

/// The hosted byte-input failure contract is exactly one `crashes Trap` clause
/// whose route list is empty (the published `true` route), and no other
/// requires/ensures/crash contract on the signature.
fn exact_unconditional_trap_crash_contract(
    typed: &typed_trees::TypedTrees,
    signature: &typed_trees::signature::StateSignature,
) -> bool {
    sole_unconditional_trap(typed.state_signature_contracts(signature))
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
                    "/../../../../../source/library/std/console.omg"
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
    if parameter.is_const || parameter.is_mutable {
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

/// Whether `contracts` is exactly one unconditional `crashes Trap` clause:
/// a single contract, Trap cause, no facts, no binding.
fn sole_unconditional_trap(contracts: &[typed_trees::signature::SignatureContract]) -> bool {
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

/// Issue direct native supply from the same private hosted catalog used by
/// selected intrinsic rows. Population grants no provider selection: each
/// retained callable is its own original declaration, not its abstract slot.
pub(crate) fn retain_direct_compiler_intrinsics(
    checked: &CheckedTrees,
    target: Option<target::TargetProfile>,
    selected: &[provider_planning::SelectedProviderReviewProvenance],
    calling: &[provider_planning::calling_policy_plans::BoundaryCallingPlanRealization],
    opaque: &[representation_planning::OpaqueRepresentationSelection],
) -> Result<Vec<std::sync::Arc<effects::DirectForeignRealization>>, Vec<Diagnostic>> {
    let Some(target) = target else {
        return Ok(Vec::new());
    };
    let Some(bundle) = HostedIntrinsicBundle::from_target_name(target.target_name()) else {
        return Ok(Vec::new());
    };
    let typed = &checked.typed;
    let context =
        provider_planning::ProviderPlanningContext::new(typed, source::DependencyScope::Product);
    let mut retained = Vec::new();
    // Selection already owns these original rows. Publish their native calling
    // custody from that aligned roster, not by rediscovering a Product copy of
    // the same declaration during standalone source enumeration.
    for original in selected {
        for ((requirement_symbol, realization_symbol), execution) in original
            .provider
            .row_requirements
            .iter()
            .zip(&original.provider.row_realizations)
            .zip(&original.row_compiler_intrinsic_executions)
        {
            let Some(execution) = execution else {
                continue;
            };
            if !HOSTED_INTRINSIC_ROWS
                .iter()
                .any(|hosted| hosted.identity == *execution)
            {
                continue;
            }
            let definition = typed
                .trait_definition_by_symbol(original.provider.schema.symbol())
                .ok_or_else(|| vec![Diagnostic::error(
                    "selected hosted intrinsic lost its original service declaration",
                )])?;
            let requirement = typed
                .trait_machine_signature_by_symbol(definition, *requirement_symbol)
                .ok_or_else(|| vec![Diagnostic::error(
                    "selected hosted intrinsic lost its original requirement declaration",
                )])?;
            let mut realizations = typed
                .machines()
                .iter()
                .filter(|machine| machine.symbol == *realization_symbol);
            let (Some(realization), None) = (realizations.next(), realizations.next()) else {
                return Err(vec![Diagnostic::error(
                    "selected hosted intrinsic has no unique original realization declaration",
                )]);
            };
            retain_hosted_intrinsic_calling_origin(
                typed,
                target,
                definition,
                requirement,
                realization,
                original.plan.schema.clone(),
                *execution,
                effects::DirectCompilerIntrinsicOrigin::SelectedProvider(original.plan.clone()),
                calling,
                opaque,
                &mut retained,
            )?;
        }
    }
    for realization in typed.machines() {
        if !context.contains(realization.symbol)
            || realization
                .authored_target
                .as_ref()
                .map(|target| target.as_str())
                != Some(target.target_name())
        {
            continue;
        }
        for clause in typed.machine_trait_conformances(realization) {
            let Some(typed_trees::machine::SatisfiedDeclaration::Trait {
                definition,
                requirement,
            }) = typed_trees::machine::resolve_satisfied_declaration(typed, realization, clause)
            else {
                continue;
            };
            if !context.contains(definition.symbol) || !context.contains(requirement.symbol) {
                continue;
            }
            for hosted in &HOSTED_INTRINSIC_ROWS {
                if definition.name.as_str() != hosted.service.trait_name()
                    || requirement.name.as_str() != hosted.requirement
                    || realization.name.as_str() != hosted.realization
                {
                    continue;
                }
                let Some(overload) = typed.machine_callable_identity_text(realization) else {
                    continue;
                };
                // An original selected row has already issued this exact
                // realization. Do not mint a second standalone origin for it.
                if selected.iter().any(|original| {
                    original.provider.schema.symbol() == definition.symbol
                        && original.provider.row_requirements.iter()
                            .zip(&original.provider.row_realizations)
                            .zip(&original.row_compiler_intrinsic_executions)
                            .any(|((original_requirement, original_realization), execution)| {
                                *original_requirement == requirement.symbol
                                    && *original_realization == realization.symbol
                                    && *execution == Some(hosted.identity)
                            })
                }) {
                    continue;
                }
                if !hosted.service.bundled_binding(
                    typed,
                    definition.symbol,
                    requirement.symbol,
                    realization.symbol,
                    bundle,
                ) {
                    continue;
                }
                if !boundary_declaration_shape(
                    checked,
                    definition.symbol,
                    requirement.symbol,
                    realization.symbol,
                    hosted.service.trait_name(),
                    hosted.shape == ConsoleIntrinsicShape::I32ToUnit,
                    hosted.requirement,
                    hosted.realization,
                    hosted.shape,
                    Some(&overload),
                )
                .map_err(|error| vec![error])?
                {
                    continue;
                }
                let source_digest = |symbol| {
                    let unit = typed
                        .symbols
                        .symbol_source_span(symbol)
                        .and_then(|span| typed.symbols.source_file(span))?;
                    package_compilation::toolchain_source_identity_digest(unit).ok()
                };
                let (Some(trait_source), Some(requirement_source), Some(realization_source)) = (
                    source_digest(definition.symbol),
                    source_digest(requirement.symbol),
                    source_digest(realization.symbol),
                ) else {
                    continue;
                };
                let Some(schema) =
                    provider_planning::service_schema::from_typed(context, calling, definition)
                else {
                    continue;
                };
                retain_hosted_intrinsic_calling_origin(
                    typed,
                    target,
                    definition,
                    requirement,
                    realization,
                    schema,
                    hosted.identity,
                    effects::DirectCompilerIntrinsicOrigin::BundledDeclarations([
                        trait_source,
                        requirement_source,
                        realization_source,
                    ]),
                    calling,
                    opaque,
                    &mut retained,
                )?;
            }
        }
    }
    Ok(retained)
}

/// Both original issuers retain the same complete declaration/calling relation.
/// No missing entry becomes an omitted candidate: once catalog custody exists,
/// a missing application is a diagnostic at its producing boundary.
#[allow(clippy::too_many_arguments)]
fn retain_hosted_intrinsic_calling_origin(
    typed: &typed_trees::TypedTrees,
    target: target::TargetProfile,
    definition: &typed_trees::trait_definition::TraitDefinition,
    requirement: &typed_trees::signature::StateSignature,
    realization: &typed_trees::machine::Machine,
    schema: effects::provider_plan::ServiceSchema,
    execution: CompilerIntrinsicExecutionIdentity,
    origin: effects::DirectCompilerIntrinsicOrigin,
    calling: &[provider_planning::calling_policy_plans::BoundaryCallingPlanRealization],
    opaque: &[representation_planning::OpaqueRepresentationSelection],
    retained: &mut Vec<std::sync::Arc<effects::DirectForeignRealization>>,
) -> Result<(), Vec<Diagnostic>> {
    let requirement_identity =
        typed.trait_requirement_callable_identity_text(definition, requirement);
    let mut methods = schema.methods.iter().filter(|method| {
        schema.identity_text(method.requirement_identity) == requirement_identity
    });
    let (Some(method), None) = (methods.next(), methods.next()) else {
        return Err(vec![Diagnostic::error(
            "hosted intrinsic lost its unique original schema method",
        )]);
    };
    let declaration = typed.machine_declaration_callable_identity(realization.symbol);
    let Some(overload) = typed.machine_callable_identity_text(realization) else {
        return Err(vec![Diagnostic::error(
            "hosted intrinsic lost its original callable overload",
        )]);
    };
    if !declaration.is_valid() {
        return Err(vec![Diagnostic::error(
            "hosted intrinsic lost its unique physical declaration",
        )]);
    }
    let binding = effects::DirectForeignBinding::CompilerIntrinsic {
        execution,
        origin: origin.clone(),
    };
    let entry = provider_planning::direct_boundary_entry_plan(
        typed,
        target,
        &schema,
        method,
        requirement.symbol,
        &binding,
        calling,
        opaque,
    ).ok_or_else(|| vec![Diagnostic::error(format!(
        "hosted intrinsic `{requirement_identity}` cannot retain its original calling application",
    ))])?;
    let direct = effects::DirectForeignRealization::from_compiler_catalog(
        typed.callable_identity_text(declaration),
        overload,
        requirement_identity,
        target,
        schema,
        execution,
        origin,
        entry,
    ).map_err(|error| vec![Diagnostic::error(error)])?;
    if retained.iter().any(|row| row.callable_identity() == direct.callable_identity()) {
        return Err(vec![Diagnostic::error(
            "direct compiler intrinsic has ambiguous original catalog custody",
        )]);
    }
    retained.push(std::sync::Arc::new(direct));
    Ok(())
}
