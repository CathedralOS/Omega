/*
We prepare intrinsic declaration facts while source is available so target
settlement can consume immutable declaration records without reopening source.
Catalog eligibility and accepted public-plan authority remain independent, and
deferred shape failures are demanded only after the original ownership gate
matches.

We classify sealed scalar accessors (`IntrinsicDeclarationKind::Access`),
UEFI fixed-image intrinsics (`ProgramStorageEntry` and `ProgramImage`), and
hosted console/process-exit rows (`HOSTED_INTRINSIC_ROWS`). Whether a
declaration is the bundled one comes only from the sealed intrinsic catalog
(`language_core::sealed_intrinsics`); this file keeps the signature shapes and
execution identities each catalog operation must satisfy. Hosted byte-input
(`read_byte`) requires `blocks;` plus a single unconditional `crashes Trap`
contract on both requirement and realization, whereas byte-output and
process-exit retain nonblocking, crash-free envelopes.
*/

use crate::provider_planning::is_unit_type;
use crate::provider_planning::type_reference_node;
use diagnostics::Diagnostic;
use effects::CompilerIntrinsicExecutionIdentity;
use effects::provider_plan::{ProviderBinding, ProviderPlan, ProviderPlanRow};
use language_core::sealed_intrinsics::{FixedImageOperation, HostedOperation, SealedIntrinsic};
use language_semantics::MachineSupplyMode;
use semantic_vocabulary::PackageKeyIdentity;
use symbols::{BuiltinTypeAtom, SymbolHandle};
use target::HostedIntrinsicBundle;
use typed_trees::TypedTrees;
use typed_trees::types::TypeReferenceNode;

#[derive(Clone, Copy)]
pub struct AcceptedIntrinsicBinding<'a> {
    pub accepted: &'a package_compilation::AcceptedSemanticBinding,
    pub declaration_identity: Option<&'a str>,
    pub public_plan: Option<&'a ProviderPlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntrinsicDeclaration {
    target: String,
    kind: IntrinsicDeclarationKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum IntrinsicDeclarationKind {
    Unsupported,
    Access(Result<CompilerIntrinsicExecutionIdentity, Diagnostic>),
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
                    && plan.schema.declaration.trait_package_identity == Some(binding.package())
            }
            HostedService::ProcessExit => {
                binding.role() == AcceptedSemanticBindingRole::ProcessExitExitProcessI32
                    && self.service_owner.is_none()
                    && self.requirement_owner.is_none()
                    && plan.schema.declaration.trait_package_identity.is_none()
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
        if let crate::ProviderSchemaDeclaration::SealedAccessor(symbol) = schema {
            let result = (|| {
                let context =
                    crate::ProviderPlanningContext::new(typed, source::DependencyScope::Product);
                let accessor =
                    crate::SealedAccessor::rejoin_plan(context, plan)?.ok_or_else(|| {
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
                    crate::SealedAccessorScalarOperation::Read => {
                        effects::CompilerAccessorScalarOperation::Read
                    }
                    crate::SealedAccessorScalarOperation::Take => {
                        effects::CompilerAccessorScalarOperation::Take
                    }
                    crate::SealedAccessorScalarOperation::Write => {
                        effects::CompilerAccessorScalarOperation::Write
                    }
                };
                Ok(CompilerIntrinsicExecutionIdentity::AccessorScalarAccess {
                    contract: accessor.execution_contract(context)?,
                    placement_commitment: accessor.placement_commitment(),
                    access_slot: accessor.access_slot(),
                    operation,
                })
            })();
            return IntrinsicDeclarationKind::Access(result);
        }
        let crate::ProviderSchemaDeclaration::BoundaryTrait(service) = schema else {
            return IntrinsicDeclarationKind::Unsupported;
        };
        if plan.target == "uefi_x86_64" {
            for (operation, name, method, implementation, shape, execution) in [
                (
                    FixedImageOperation::ProgramStorageEntry,
                    "ProgramStorageEntry",
                    "enter",
                    "ProgramStorageEntryNativeProvider::enter",
                    BoundaryIntrinsicShape::ProgramStorageToUnit,
                    CompilerIntrinsicExecutionIdentity::ProgramStorageEntry,
                ),
                (
                    FixedImageOperation::ProgramImageRetainedSuffixStart,
                    "ProgramImage",
                    "retained_suffix_start",
                    "ProgramImageNativeProvider::retained_suffix_start",
                    BoundaryIntrinsicShape::UnitToAddress,
                    CompilerIntrinsicExecutionIdentity::ProgramImageRetainedSuffixStart,
                ),
            ] {
                if typed.symbols.sealed_member_intrinsic(service, method)
                    == Some(SealedIntrinsic::FixedImageRequirement(operation))
                    && sealed_realization(typed, realization)
                        == Some(SealedIntrinsic::FixedImageRealization(operation))
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
                bundled: hosted.bundled_binding(typed, service, realization, bundle),
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

    pub fn settle(
        &self,
        plan: &ProviderPlan,
        target: Option<&str>,
        console: Option<AcceptedIntrinsicBinding<'_>>,
        process_exit: Option<AcceptedIntrinsicBinding<'_>>,
    ) -> Result<Option<CompilerIntrinsicExecutionIdentity>, Diagnostic> {
        if let IntrinsicDeclarationKind::Access(result) = &self.kind {
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
            IntrinsicDeclarationKind::Access(_) => Ok(None),
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
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum BoundaryIntrinsicShape {
    I32ToUnit,
    UnitToByteRead,
    UnitToAddress,
    ProgramStorageToUnit,
}

struct HostedIntrinsicRow {
    service: HostedService,
    operation: HostedOperation,
    requirement: &'static str,
    realization: &'static str,
    shape: BoundaryIntrinsicShape,
    identity: CompilerIntrinsicExecutionIdentity,
}

impl HostedIntrinsicRow {
    /*
        Whether this row's requirement and the selected target's realization
        are the bundled declarations the sealed intrinsic catalog names. The
        realization row carries its target, so another target's bundled file
        never supplies custody here.
    */
    fn bundled_binding(
        &self,
        typed: &TypedTrees,
        service: SymbolHandle,
        realization: SymbolHandle,
        bundle: HostedIntrinsicBundle,
    ) -> bool {
        typed
            .symbols
            .sealed_member_intrinsic(service, self.requirement)
            == Some(SealedIntrinsic::HostedRequirement(self.operation))
            && sealed_realization(typed, realization)
                == Some(SealedIntrinsic::HostedRealization {
                    operation: self.operation,
                    target: bundle.target_name(),
                })
    }
}

/*
    The catalog row of a realization machine, read under the machine's
    declared name so target-selected realizations keep their source spelling.
*/
fn sealed_realization(typed: &TypedTrees, realization: SymbolHandle) -> Option<SealedIntrinsic> {
    let machine = typed.machine(realization)?;
    typed
        .symbols
        .sealed_declaration(realization, machine.name.as_str())
}

const HOSTED_INTRINSIC_ROWS: [HostedIntrinsicRow; 4] = [
    HostedIntrinsicRow {
        service: HostedService::Console,
        operation: HostedOperation::ConsoleExitProcess,
        requirement: "exit_process",
        realization: "ConsoleNativeProvider::exit_process",
        shape: BoundaryIntrinsicShape::I32ToUnit,
        identity: CompilerIntrinsicExecutionIdentity::HostedExitProcessI32,
    },
    HostedIntrinsicRow {
        service: HostedService::ProcessExit,
        operation: HostedOperation::ProcessExit,
        requirement: "exit_process",
        realization: "ProcessExitNativeProvider::exit_process",
        shape: BoundaryIntrinsicShape::I32ToUnit,
        identity: CompilerIntrinsicExecutionIdentity::HostedExitProcessI32,
    },
    HostedIntrinsicRow {
        service: HostedService::Console,
        operation: HostedOperation::ConsoleWriteByte,
        requirement: "write_byte",
        realization: "ConsoleNativeProvider::write_byte",
        shape: BoundaryIntrinsicShape::I32ToUnit,
        identity: CompilerIntrinsicExecutionIdentity::HostedWriteByteI32,
    },
    HostedIntrinsicRow {
        service: HostedService::Console,
        operation: HostedOperation::ConsoleReadByte,
        requirement: "read_byte",
        realization: "ConsoleNativeProvider::read_byte",
        shape: BoundaryIntrinsicShape::UnitToByteRead,
        identity: CompilerIntrinsicExecutionIdentity::HostedReadByte,
    },
];

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
                    typed_trees::boundary_completions::boundary_requirement_completion(
                        typed,
                        requirement,
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
        .declaration
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
    let intrinsic_machine = match &row.binding {
        ProviderBinding::CompilerIntrinsic { machine } => Some(machine),
        ProviderBinding::Import { .. } => None,
        _ => return Ok(false),
    };
    let expect_blocking_trap = matches!(shape, BoundaryIntrinsicShape::UnitToByteRead);
    if realization.suspends
        || realization.blocks != expect_blocking_trap
        || !exact_realization_contracts(typed, realization, expect_blocking_trap)
    {
        return Ok(false);
    }
    if realization.name.as_str() != realization_name
        || intrinsic_machine.is_some_and(|machine| machine != &realization_identity)
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
    if !exact_boundary_state(typed, entry, trait_symbol, shape) {
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

fn exact_boundary_signature(
    typed: &typed_trees::TypedTrees,
    signature: &typed_trees::signature::StateSignature,
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
                && is_unit_type(typed, signature.return_type)
        }
        BoundaryIntrinsicShape::UnitToAddress => {
            !signature.blocks
                && typed.state_signature_contracts(signature).is_empty()
                && typed.state_signature_parameters(signature).is_empty()
                && exact_address_type(typed, signature.return_type)
        }
        BoundaryIntrinsicShape::I32ToUnit => {
            !signature.blocks
                && exact_i32_parameter(typed, typed.state_signature_parameters(signature))
                && is_unit_type(typed, signature.return_type)
        }
        BoundaryIntrinsicShape::UnitToByteRead => {
            signature.blocks
                && exact_unconditional_trap_crash_contract(typed, signature)
                && typed.state_signature_parameters(signature).is_empty()
                && exact_byte_read_type(typed, signature.return_type, trait_symbol)
        }
    }
}

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

fn exact_unconditional_trap_crash_contract(
    typed: &typed_trees::TypedTrees,
    signature: &typed_trees::signature::StateSignature,
) -> bool {
    sole_unconditional_trap(typed.state_signature_contracts(signature))
}

fn exact_boundary_state(
    typed: &typed_trees::TypedTrees,
    state: &typed_trees::state::State,
    trait_symbol: SymbolHandle,
    shape: BoundaryIntrinsicShape,
) -> bool {
    match shape {
        BoundaryIntrinsicShape::ProgramStorageToUnit => {
            canonical_storage_parameters(
                typed,
                typed.state_parameters(state),
                typed.state_contracts(state),
            ) && is_unit_type(typed, state.return_type)
        }
        BoundaryIntrinsicShape::UnitToAddress => {
            typed.state_parameters(state).is_empty() && exact_address_type(typed, state.return_type)
        }
        BoundaryIntrinsicShape::I32ToUnit => {
            exact_i32_parameter(typed, typed.state_parameters(state))
                && is_unit_type(typed, state.return_type)
        }
        BoundaryIntrinsicShape::UnitToByteRead => {
            typed.state_parameters(state).is_empty()
                && exact_byte_read_type(typed, state.return_type, trait_symbol)
        }
    }
}

fn canonical_storage_parameters(
    typed: &typed_trees::TypedTrees,
    parameters: &[typed_trees::signature::StateParameter],
    contracts: &[typed_trees::signature::SignatureContract],
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
                    typed_trees::signature::SignatureContractKind::Requires
                )
                || contract.binding.is_some()
            {
                return false;
            }
            let TypeReferenceNode::Constrained { constraints, .. } =
                type_reference_node(typed, parameter.type_reference)
            else {
                return false;
            };
            let [typed_trees::types::TypeConstraintNode::Domain(domain)] =
                typed.type_reference_table.constraints(*constraints)
            else {
                return false;
            };
            let [typed_trees::domain::ProofFact::Membership(membership)] =
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
    typed: &typed_trees::TypedTrees,
    reference: typed_trees::types::TypeReferenceHandle,
) -> bool {
    matches!(type_reference_node(typed, reference),
        TypeReferenceNode::Named { symbol, .. }
        if typed.symbols.builtin_type_atom(*symbol) == Some(BuiltinTypeAtom::Address))
}

pub(crate) fn exact_byte_read_type(
    typed: &typed_trees::TypedTrees,
    type_reference: typed_trees::types::TypeReferenceHandle,
    trait_symbol: SymbolHandle,
) -> bool {
    let TypeReferenceNode::Named { symbol, name } = type_reference_node(typed, type_reference)
    else {
        return false;
    };
    name.as_str() == "ByteRead"
        && match typed.symbols.symbol_package_identity(trait_symbol) {
            Some(package) => typed.symbols.symbol_package_identity(*symbol) == Some(package),
            None => {
                typed.symbols.sealed_intrinsic(*symbol) == Some(SealedIntrinsic::HostedByteRead)
            }
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
    let TypeReferenceNode::Named { symbol, name } =
        type_reference_node(typed, parameter.type_reference)
    else {
        return false;
    };
    typed.symbols.builtin_type_atom(*symbol) == Some(BuiltinTypeAtom::I32) && name.as_str() == "i32"
}

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
