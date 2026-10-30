//! Prepare intrinsic declaration facts while source is available.
//!
//! Target settlement consumes these immutable facts without reopening source.
//! Catalog eligibility and accepted public-plan authority remain independent;
//! deferred shape failures are demanded only after the original ownership gate.

use checked_trees::typed_trees::TypedTrees;
use checked_trees::typed_trees::types::TypeReferenceNode;
use diagnostics::Diagnostic;
use effects::CompilerIntrinsicExecutionIdentity;
use effects::provider_plan::{ProviderBinding, ProviderPlan, ProviderPlanRow};
use language_semantics::MachineSupplyMode;
use semantic_vocabulary::PackageKeyIdentity;
use symbols::{BuiltinTypeAtom, SymbolHandle};
use target::HostedIntrinsicBundle;

/// An offered consumer binding, optionally pinned to the declaration it
/// resolved and an independently accepted public plan. Neither is a projection
/// of the direct callable's plan.
#[derive(Clone, Copy)]
pub struct AcceptedIntrinsicBinding<'a> {
    pub accepted: &'a package_compilation::AcceptedSemanticBinding,
    pub declaration_identity: Option<&'a str>,
    pub public_plan: Option<&'a ProviderPlan>,
}

/// Source-derived declaration facts, never host execution permission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntrinsicDeclaration {
    target: String,
    kind: IntrinsicDeclarationKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum IntrinsicDeclarationKind {
    Unsupported,
    Placed(Result<CompilerIntrinsicExecutionIdentity, Diagnostic>),
    Fixed(Result<Option<CompilerIntrinsicExecutionIdentity>, Diagnostic>),
    Hosted {
        subject: HostedSubject,
        declaration_identity: Result<String, Diagnostic>,
        rows: Vec<HostedDeclarationRow>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HostedDeclarationRow {
    service: HostedService,
    bundled: bool,
    shape: Result<bool, Diagnostic>,
    execution: CompilerIntrinsicExecutionIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HostedSubject {
    declaration_path: String,
    service_owner: Option<PackageKeyIdentity>,
    requirement_owner: Option<PackageKeyIdentity>,
    realization_owner: Option<PackageKeyIdentity>,
}

impl HostedSubject {
    fn prepare(
        typed: &TypedTrees,
        service: SymbolHandle,
        requirement: SymbolHandle,
        realization: SymbolHandle,
    ) -> Self {
        Self {
            declaration_path: typed.symbols.display_path(service, "::"),
            service_owner: typed.symbols.symbol_package_identity(service),
            requirement_owner: typed.symbols.symbol_package_identity(requirement),
            realization_owner: typed.symbols.symbol_package_identity(realization),
        }
    }

    fn matches(
        &self,
        service: HostedService,
        plan: &ProviderPlan,
        binding: &package_compilation::AcceptedSemanticBinding,
    ) -> bool {
        use package_compilation::AcceptedSemanticBindingRole;
        let owns_service = match service {
            HostedService::Console => {
                binding.role() == AcceptedSemanticBindingRole::ConsoleExitProcessI32
                    && self.service_owner == Some(binding.package())
                    && self.requirement_owner == Some(binding.package())
                    && plan.schema.trait_package_identity == Some(binding.package())
            }
            HostedService::ProcessExit => {
                binding.role() == AcceptedSemanticBindingRole::ProcessExitExitProcessI32
                    && self.service_owner.is_none()
                    && self.requirement_owner.is_none()
                    && plan.schema.trait_package_identity.is_none()
            }
        };
        owns_service
            && self.realization_owner == Some(binding.package())
            && plan.provider_type_package_identity == Some(binding.package())
            && plan.origin_package_identity == Some(binding.package())
            && self.declaration_path == binding.declaration_path()
            && plan.schema.identity_digest() == binding.normalized_schema_digest()
            && binding.selected_provider_plan_digest() == Some(plan.identity_digest())
    }
}

impl IntrinsicDeclaration {
    /// Preserve the original row's declaration facts. Unsupported candidates
    /// and failures remain data; unselected declarations acquire no new obligation.
    pub fn prepare(
        typed: &TypedTrees,
        plan: &ProviderPlan,
        schema: crate::ProviderSchemaDeclaration,
        row: &ProviderPlanRow,
        requirement: SymbolHandle,
        realization: SymbolHandle,
        declaration_identity: Result<String, Diagnostic>,
    ) -> Self {
        let kind = Self::prepare_kind(
            typed,
            plan,
            schema,
            row,
            requirement,
            realization,
            declaration_identity,
        );
        Self {
            target: plan.target.clone(),
            kind,
        }
    }

    fn prepare_kind(
        typed: &TypedTrees,
        plan: &ProviderPlan,
        schema: crate::ProviderSchemaDeclaration,
        row: &ProviderPlanRow,
        requirement: SymbolHandle,
        realization: SymbolHandle,
        declaration_identity: Result<String, Diagnostic>,
    ) -> IntrinsicDeclarationKind {
        if let crate::ProviderSchemaDeclaration::SealedPlacedAccessor(symbol) = schema {
            let result = (|| {
                let context =
                    crate::ProviderPlanningContext::new(typed, source::DependencyScope::Product);
                let accessor = crate::SealedPlacedAccessor::rejoin_plan(context, plan)?
                    .ok_or_else(|| {
                        Diagnostic::error("sealed placed row has no exact catalog plan")
                    })?;
                if symbol != requirement
                    || symbol != realization
                    || symbol != accessor.machine()
                    || plan.rows.as_slice() != std::slice::from_ref(row)
                {
                    return Err(Diagnostic::error(
                        "sealed placed execution substituted its declaration role or exact row",
                    ));
                }
                let operation = match accessor.operation() {
                    crate::SealedPlacedScalarOperation::Read => {
                        effects::CompilerPlacedScalarOperation::Read
                    }
                    crate::SealedPlacedScalarOperation::Take => {
                        effects::CompilerPlacedScalarOperation::Take
                    }
                    crate::SealedPlacedScalarOperation::Write => {
                        effects::CompilerPlacedScalarOperation::Write
                    }
                };
                Ok(CompilerIntrinsicExecutionIdentity::PlacedScalarAccess {
                    contract: accessor.execution_contract(context)?,
                    placement_commitment: accessor.placement_commitment(),
                    access_slot: accessor.access_slot(),
                    operation,
                })
            })();
            return IntrinsicDeclarationKind::Placed(result);
        }
        let crate::ProviderSchemaDeclaration::BoundaryTrait(service) = schema else {
            return IntrinsicDeclarationKind::Unsupported;
        };
        if plan.target == "uefi_x86_64" {
            for (file, bytes, name, method, implementation, shape, execution) in [
                (
                    "extent.omg",
                    BUNDLED_EXTENT,
                    "ProgramStorageEntry",
                    "enter",
                    "ProgramStorageEntryNativeProvider::enter",
                    BoundaryIntrinsicShape::ProgramStorageToUnit,
                    CompilerIntrinsicExecutionIdentity::ProgramStorageEntry,
                ),
                (
                    "program_image.omg",
                    BUNDLED_PROGRAM_IMAGE,
                    "ProgramImage",
                    "retained_suffix_start",
                    "ProgramImageNativeProvider::retained_suffix_start",
                    BoundaryIntrinsicShape::UnitToAddress,
                    CompilerIntrinsicExecutionIdentity::ProgramImageRetainedSuffixStart,
                ),
            ] {
                if [service, requirement, realization]
                    .into_iter()
                    .all(|symbol| {
                        typed.symbols.symbol_source_origin(symbol)
                            == Some(source::SourceOrigin::Toolchain)
                            && exact_bundled_standalone_source(typed, symbol, file, bytes)
                    })
                {
                    return IntrinsicDeclarationKind::Fixed(
                        boundary_row_shape(
                            typed,
                            plan,
                            row,
                            service,
                            requirement,
                            realization,
                            name,
                            true,
                            method,
                            implementation,
                            shape,
                        )
                        .map(|matches| matches.then_some(execution)),
                    );
                }
            }
        }
        let Some(bundle) = HostedIntrinsicBundle::from_target_name(&plan.target) else {
            return IntrinsicDeclarationKind::Unsupported;
        };
        let rows = HOSTED_INTRINSIC_ROWS
            .iter()
            .map(|hosted| HostedDeclarationRow {
                service: hosted.service,
                bundled: hosted.service.bundled_binding(
                    typed,
                    service,
                    requirement,
                    realization,
                    bundle,
                ),
                // Keep the error until the same ownership gate actually demands this shape.
                shape: boundary_row_shape(
                    typed,
                    plan,
                    row,
                    service,
                    requirement,
                    realization,
                    hosted.service.trait_name(),
                    hosted.shape == BoundaryIntrinsicShape::I32ToUnit,
                    hosted.requirement,
                    hosted.realization,
                    hosted.shape,
                ),
                execution: hosted.identity,
            })
            .collect();
        IntrinsicDeclarationKind::Hosted {
            subject: HostedSubject::prepare(typed, service, requirement, realization),
            declaration_identity,
            rows,
        }
    }

    /// Join target and accepted public-plan custody; no source symbols, trees,
    /// schema reconstruction or source-location recognition enter this operation.
    pub fn settle(
        &self,
        plan: &ProviderPlan,
        target: Option<&str>,
        console: Option<AcceptedIntrinsicBinding<'_>>,
        process_exit: Option<AcceptedIntrinsicBinding<'_>>,
    ) -> Result<Option<CompilerIntrinsicExecutionIdentity>, Diagnostic> {
        if let IntrinsicDeclarationKind::Placed(result) = &self.kind {
            let execution = *result.as_ref().map_err(Clone::clone)?;
            if target != Some(self.target.as_str()) {
                return Err(Diagnostic::error(
                    "sealed placed execution substituted its target",
                ));
            }
            return Ok(Some(execution));
        }
        if target != Some(self.target.as_str()) {
            return Ok(None);
        }
        match &self.kind {
            IntrinsicDeclarationKind::Unsupported => Ok(None),
            IntrinsicDeclarationKind::Placed(_) => Ok(None),
            IntrinsicDeclarationKind::Fixed(result) => result.clone(),
            IntrinsicDeclarationKind::Hosted {
                subject,
                declaration_identity,
                rows,
            } => {
                for row in rows {
                    let accepted = match row.service {
                        HostedService::Console => console,
                        HostedService::ProcessExit => process_exit,
                    };
                    let consumed = if let Some(binding) = accepted {
                        let public = binding.public_plan.unwrap_or(plan);
                        let matches = subject.matches(row.service, public, binding.accepted)
                            && binding.public_plan.is_none_or(|public| {
                                public.schema == plan.schema && public.target == plan.target
                            });
                        if matches {
                            match binding.declaration_identity {
                                Some(expected) => {
                                    declaration_identity.as_ref().map_err(Clone::clone)? == expected
                                }
                                None => true,
                            }
                        } else {
                            false
                        }
                    } else {
                        false
                    };
                    if (row.bundled || consumed) && *row.shape.as_ref().map_err(Clone::clone)? {
                        return Ok(Some(row.execution));
                    }
                }
                Ok(None)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostedService {
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
        typed: &checked_trees::typed_trees::TypedTrees,
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
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BoundaryIntrinsicShape {
    I32ToUnit,
    UnitToByteRead,
    UnitToAddress,
    ProgramStorageToUnit,
}

/// One hosted row the compiler realizes itself: the service requirement and
/// realization it binds, their exact signature shape, and the execution
/// identity it closes as.
struct HostedIntrinsicRow {
    service: HostedService,
    requirement: &'static str,
    realization: &'static str,
    shape: BoundaryIntrinsicShape,
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
        shape: BoundaryIntrinsicShape::I32ToUnit,
        identity: CompilerIntrinsicExecutionIdentity::HostedExitProcessI32,
    },
    HostedIntrinsicRow {
        service: HostedService::ProcessExit,
        requirement: "exit_process",
        realization: "ProcessExitNativeProvider::exit_process",
        shape: BoundaryIntrinsicShape::I32ToUnit,
        identity: CompilerIntrinsicExecutionIdentity::HostedExitProcessI32,
    },
    HostedIntrinsicRow {
        service: HostedService::Console,
        requirement: "write_byte",
        realization: "ConsoleNativeProvider::write_byte",
        shape: BoundaryIntrinsicShape::I32ToUnit,
        identity: CompilerIntrinsicExecutionIdentity::HostedWriteByteI32,
    },
    HostedIntrinsicRow {
        service: HostedService::Console,
        requirement: "read_byte",
        realization: "ConsoleNativeProvider::read_byte",
        shape: BoundaryIntrinsicShape::UnitToByteRead,
        identity: CompilerIntrinsicExecutionIdentity::HostedReadByte,
    },
];

const BUNDLED_PROGRAM_IMAGE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../source/library/core/program_image.omg"
));

const BUNDLED_EXTENT: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../source/library/core/extent.omg"
));

const BUNDLED_CONSOLE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../source/library/std/console.omg"
));

const BUNDLED_PROCESS_EXIT: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../source/library/core/process_exit.omg"
));

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
    typed: &checked_trees::typed_trees::TypedTrees,
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

/// Rejoin one service's exit-process semantic role to one exact package-owned
/// selected row. For Console this recognizes Process authority; the
/// toolchain-owned ProcessExit trait carries no package identity and the
/// bound package owns the provider nominal. Neither claims that the selected
/// target has a closed compiler lowering, so inferred supply is not required.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExitProcessDeclaration {
    subject: HostedSubject,
    kind: ExitProcessDeclarationKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ExitProcessDeclarationKind {
    CheckedAdapter {
        requirement_exits: bool,
    },
    Boundary {
        console: Result<bool, Diagnostic>,
        process_exit: Result<bool, Diagnostic>,
    },
}

impl ExitProcessDeclaration {
    pub fn prepare(
        typed: &TypedTrees,
        plan: &ProviderPlan,
        row: &ProviderPlanRow,
        service: SymbolHandle,
        requirement: SymbolHandle,
        realization: SymbolHandle,
    ) -> Self {
        let kind = if matches!(row.binding, ProviderBinding::CheckedAdapter { .. }) {
            ExitProcessDeclarationKind::CheckedAdapter {
                requirement_exits: matches!(
                    checked_trees::typed_trees::boundary_completions::boundary_requirement_completion(
                        typed, requirement,
                    ),
                    semantic_vocabulary::BoundaryCompletion::External(
                        semantic_vocabulary::ExternalCompletionKind::ProcessExit
                    )
                ),
            }
        } else {
            let shape = |host: HostedService| {
                let hosted = host.exit_process_row();
                boundary_row_shape(
                    typed,
                    plan,
                    row,
                    service,
                    requirement,
                    realization,
                    host.trait_name(),
                    false,
                    hosted.requirement,
                    hosted.realization,
                    hosted.shape,
                )
            };
            ExitProcessDeclarationKind::Boundary {
                console: shape(HostedService::Console),
                process_exit: shape(HostedService::ProcessExit),
            }
        };
        Self {
            subject: HostedSubject::prepare(typed, service, requirement, realization),
            kind,
        }
    }

    /// The exact public plan gates deferred source-shape refusals. A checked
    /// adapter additionally needs its selected completion evidence; recognizing
    /// the requirement alone does not grant a completion or intrinsic identity.
    pub fn matches(
        &self,
        service: HostedService,
        plan: &ProviderPlan,
        checked_external_completion: bool,
        binding: &package_compilation::AcceptedSemanticBinding,
    ) -> Result<bool, Diagnostic> {
        if !self.subject.matches(service, plan, binding) {
            return Ok(false);
        }
        match &self.kind {
            ExitProcessDeclarationKind::CheckedAdapter { requirement_exits } => {
                Ok(checked_external_completion && *requirement_exits)
            }
            ExitProcessDeclarationKind::Boundary {
                console,
                process_exit,
            } => match service {
                HostedService::Console => console.clone(),
                HostedService::ProcessExit => process_exit.clone(),
            },
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn boundary_row_shape(
    checked: &TypedTrees,
    plan: &ProviderPlan,
    row: &ProviderPlanRow,
    trait_symbol: SymbolHandle,
    requirement_symbol: SymbolHandle,
    realization_symbol: SymbolHandle,
    trait_name: &str,
    require_inferred_supply: bool,
    requirement_name: &str,
    realization_name: &str,
    shape: BoundaryIntrinsicShape,
) -> Result<bool, Diagnostic> {
    let typed = checked;

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
    if !crate::service_schema::schema_binds_exact_boundary_trait(
        crate::ProviderPlanningContext::new(typed, source::DependencyScope::Product),
        &plan.schema,
        definition,
    ) || method.name != requirement_name
        || method.requirement_owner != typed.trait_declaration_path(definition)
        || plan.schema.identity_text(method.requirement_identity) != requirement_identity
        || plan.identities.text(row.requirement_identity) != requirement_identity
        || !exact_boundary_signature(typed, requirement, requirement_name, trait_symbol, shape)
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
    let Some(realization_identity) = typed.machine_callable_identity_text(realization) else {
        return Ok(false);
    };
    // A sealed catalogue leaf has its own exact compiler-intrinsic identity.
    // Build-owned native backing is a separately admitted row, never a legacy
    // external realization or an authored forwarding signature.
    let ProviderBinding::CompilerIntrinsic {
        machine: intrinsic_machine,
    } = &row.binding
    else {
        return Ok(false);
    };
    // The realization's published envelope must spell the same honest shape
    // as its requirement: the hosted byte-input leaf may occupy the worker
    // while it waits and traps on a failed read; byte output and process
    // exit keep their nonblocking, crash-free ceiling on this leg.
    let expect_blocking_trap = matches!(shape, BoundaryIntrinsicShape::UnitToByteRead);
    if realization.suspends
        || realization.blocks != expect_blocking_trap
        || !exact_realization_contracts(typed, realization, expect_blocking_trap)
    {
        return Ok(false);
    }
    if realization.name.as_str() != realization_name
        || intrinsic_machine != &realization_identity
        || !realization.lifetime_parameters.is_empty()
        || !typed.machine_type_parameters(realization).is_empty()
        || realization.body_is_present
    {
        return Ok(false);
    }
    let _ = require_inferred_supply;
    if realization.supply_mode != MachineSupplyMode::Boundary {
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
                && checked_trees::typed_trees::machine::resolve_satisfied_declaration(
                    typed,
                    realization,
                    conformance,
                )
                .is_some_and(|declaration| declaration.symbol() == requirement_symbol)
        })
        .count();
    Ok(conformances == 1)
}

fn exact_boundary_signature(
    typed: &checked_trees::typed_trees::TypedTrees,
    signature: &checked_trees::typed_trees::signature::StateSignature,
    requirement_name: &str,
    trait_symbol: SymbolHandle,
    shape: BoundaryIntrinsicShape,
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
        BoundaryIntrinsicShape::ProgramStorageToUnit => {
            !signature.blocks
                && canonical_storage_parameters(
                    typed,
                    typed.state_signature_parameters(signature),
                    typed.state_signature_contracts(signature),
                )
                && matches!(
                    typed
                        .type_reference_table
                        .type_reference(signature.return_type),
                    TypeReferenceNode::Unit
                )
        }
        BoundaryIntrinsicShape::UnitToAddress => {
            !signature.blocks
                && typed.state_signature_contracts(signature).is_empty()
                && typed.state_signature_parameters(signature).is_empty()
                && exact_address_type(typed, signature.return_type)
        }
        BoundaryIntrinsicShape::I32ToUnit => {
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
        BoundaryIntrinsicShape::UnitToByteRead => {
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
    typed: &checked_trees::typed_trees::TypedTrees,
    realization: &checked_trees::typed_trees::machine::Machine,
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
    typed: &checked_trees::typed_trees::TypedTrees,
    signature: &checked_trees::typed_trees::signature::StateSignature,
) -> bool {
    sole_unconditional_trap(typed.state_signature_contracts(signature))
}

fn exact_boundary_state(
    typed: &checked_trees::typed_trees::TypedTrees,
    state: &checked_trees::typed_trees::state::State,
    trait_symbol: SymbolHandle,
    shape: BoundaryIntrinsicShape,
) -> bool {
    match shape {
        BoundaryIntrinsicShape::ProgramStorageToUnit => {
            canonical_storage_parameters(
                typed,
                typed.state_parameters(state),
                typed.state_contracts(state),
            ) && matches!(
                typed.type_reference_table.type_reference(state.return_type),
                TypeReferenceNode::Unit
            )
        }
        BoundaryIntrinsicShape::UnitToAddress => {
            typed.state_parameters(state).is_empty() && exact_address_type(typed, state.return_type)
        }
        BoundaryIntrinsicShape::I32ToUnit => {
            exact_i32_parameter(typed, typed.state_parameters(state))
                && matches!(
                    typed.type_reference_table.type_reference(state.return_type),
                    TypeReferenceNode::Unit
                )
        }
        BoundaryIntrinsicShape::UnitToByteRead => {
            typed.state_parameters(state).is_empty()
                && exact_byte_read_type(typed, state.return_type, trait_symbol)
        }
    }
}

/// Only used after the exact bundled owner and declaration roles have joined.
/// Their source bytes fix both `Extent in Granted` types; ordinary checked
/// conformance fixes the realization's same requirement application.
/// Each qualification lowers to a parameter-rooted membership precondition,
/// on the trait signature or realization entry state respectively. Match that
/// exact roster rather than treating qualified parameters as contract-free.
fn canonical_storage_parameters(
    typed: &checked_trees::typed_trees::TypedTrees,
    parameters: &[checked_trees::typed_trees::signature::StateParameter],
    contracts: &[checked_trees::typed_trees::signature::SignatureContract],
) -> bool {
    if parameters.len() != 2 || contracts.len() != parameters.len() {
        return false;
    }
    parameters
        .iter()
        .zip(contracts)
        .all(|(parameter, contract)| {
            if parameter.is_const
                || parameter.is_mutable
                || !matches!(
                    contract.kind,
                    checked_trees::typed_trees::signature::SignatureContractKind::Requires
                )
                || contract.binding.is_some()
            {
                return false;
            }
            let TypeReferenceNode::Constrained { constraints, .. } = typed
                .type_reference_table
                .type_reference(parameter.type_reference)
            else {
                return false;
            };
            let [checked_trees::typed_trees::types::TypeConstraintNode::Domain(domain)] =
                typed.type_reference_table.constraints(*constraints)
            else {
                return false;
            };
            let [checked_trees::typed_trees::domain::ProofFact::Membership(membership)] =
                typed.proof_facts.span_or_empty(contract.facts)
            else {
                return false;
            };
            membership.domain_symbol == domain.symbol
                && membership.semantic_domain == domain.semantic_id
                && domain.arguments.is_empty()
                && membership.domain_arguments.is_empty()
                && typed
                    .expression_table
                    .name_path_node(membership.value)
                    .is_some_and(|path| {
                        path.head_symbol == parameter.symbol
                            && path.symbol == parameter.symbol
                            && typed
                                .expression_table
                                .name_path_member_symbols(path.member_symbols)
                                == [parameter.symbol]
                    })
        })
}

fn exact_address_type(
    typed: &checked_trees::typed_trees::TypedTrees,
    reference: checked_trees::typed_trees::types::TypeReferenceHandle,
) -> bool {
    matches!(typed.type_reference_table.type_reference(reference),
        TypeReferenceNode::Named { symbol, .. }
        if typed.symbols.builtin_type_atom(*symbol) == Some(BuiltinTypeAtom::Address))
}

fn exact_byte_read_type(
    typed: &checked_trees::typed_trees::TypedTrees,
    type_reference: checked_trees::typed_trees::types::TypeReferenceHandle,
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
    typed: &checked_trees::typed_trees::TypedTrees,
    parameters: &[checked_trees::typed_trees::signature::StateParameter],
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
fn sole_unconditional_trap(
    contracts: &[checked_trees::typed_trees::signature::SignatureContract],
) -> bool {
    let [contract] = contracts else {
        return false;
    };
    matches!(
        contract.kind,
        checked_trees::typed_trees::signature::SignatureContractKind::Crashes {
            cause: checked_trees::typed_trees::signature::CrashCause::Trap,
        }
    ) && contract.facts.is_empty()
        && contract.binding.is_none()
}
