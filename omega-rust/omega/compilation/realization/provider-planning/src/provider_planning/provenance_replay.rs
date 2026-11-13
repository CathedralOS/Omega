// Row provenance is retained data. Satisfaction derivation and old checked
// adapter replay no longer create provider plans.
use effects::provider_plan::ProviderPlan;
pub use typed_trees::service_declarations::ProviderSchemaDeclaration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderRowRealization {
    Authored(symbols::SymbolHandle),
    BuildNative(std::sync::Arc<crate::native_bindings::NativeBuildRowOrigin>),
    Toolchain,
}

impl ProviderRowRealization {
    pub const fn authored(&self) -> Option<symbols::SymbolHandle> {
        match self {
            Self::Authored(symbol) => Some(*symbol),
            _ => None,
        }
    }
    pub fn native(&self) -> Option<&crate::native_bindings::NativeBuildRowOrigin> {
        match self {
            Self::BuildNative(origin) => Some(origin),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderPlanProvenance {
    pub schema: ProviderSchemaDeclaration,
    pub provider_type: Option<symbols::SymbolHandle>,
    pub named_application: Option<typed_trees::provider_declarations::NamedProviderApplication>, // Original named application, before selected rows lose their source scope.
    pub row_requirements: Vec<symbols::SymbolHandle>,
    pub row_realizations: Vec<ProviderRowRealization>,
    pub row_target_machine_origins: Vec<Option<SelectedTargetMachineOrigin>>, // Pre-resolution target-scoped declaration custody for catalog-inferred rows; None for ordinary checked, legacy external, and evaluated-payload rows.
    pub row_accessors: Vec<Option<crate::SealedAccessor>>,
}

pub use language_semantics::provider_declarations::SelectedTargetMachineOrigin;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivedProviderPlan {
    pub plan: ProviderPlan,
    pub provenance: ProviderPlanProvenance,
}

mod requirement_identities {
    use crate::intrinsic_declarations::{BoundaryIntrinsicShape, exact_byte_read_type};
    use crate::provider_planning::provenance_replay::SelectedTargetMachineOrigin;
    use crate::provider_planning::{ProviderBinding, TypedTrees, is_unit_type};
    use target::HostedIntrinsicBundle;
    use typed_trees::{signature::StateSignature, trait_definition::TraitDefinition};

    pub(crate) fn exact_satisfied_requirement_identity(
        typed: &TypedTrees,
        trait_symbol: symbols::SymbolHandle,
        requirement_symbol: symbols::SymbolHandle,
    ) -> String {
        if trait_symbol == requirement_symbol
            && let Some(requirement) = typed.machines().iter().find(|requirement| {
                requirement.symbol == requirement_symbol
                    && requirement.supply_mode
                        == language_semantics::MachineSupplyMode::TopLevelRequirement
            })
        {
            return typed
                .machine_callable_identity_text(requirement)
                .unwrap_or_default();
        }
        let Some(definition) = typed.trait_definition_by_symbol(trait_symbol) else {
            return String::new();
        };
        typed
            .trait_machine_signature_by_symbol(definition, requirement_symbol)
            .map(|signature| typed.trait_requirement_callable_identity_text(definition, signature))
            .unwrap_or_default()
    }

    fn exact_external_binding_identity<'typed>(
        typed: &'typed TypedTrees,
        machine: &typed_trees::machine::Machine,
        trait_name: &str,
        requirement_name: &str,
    ) -> Option<&'typed language_semantics::ExternalBindingIdentity> {
        let mut matching = typed
            .machine_trait_conformances(machine)
            .iter()
            .filter(|conformance| same_semantic_name(conformance.name.as_str(), trait_name))
            .filter(|conformance| {
                conformance.requirement.as_ref().map(|name| name.as_str()) == Some(requirement_name)
            })
            .filter_map(|conformance| conformance.external_binding);
        let binding = matching.next()?;
        if matching.next().is_some() {
            return None;
        }
        typed.external_bindings.identity(binding)
    }

    pub(crate) fn exact_installed_external_binding_identity<'typed>(
        typed: &'typed TypedTrees,
        machine: &typed_trees::machine::Machine,
        conformance_binding: language_semantics::ExternalBindingId,
        trait_name: &str,
        requirement_name: &str,
    ) -> Option<&'typed language_semantics::ExternalBindingIdentity> {
        let language_semantics::MachineSupplyMode::ExternalRealization {
            binding: Some(supply_binding),
            mechanism: Some(supply_mechanism),
        } = machine.supply_mode
        else {
            return None;
        };
        if supply_binding != conformance_binding {
            return None;
        }
        let binding =
            exact_external_binding_identity(typed, machine, trait_name, requirement_name)?;
        (binding.mechanism() == supply_mechanism).then_some(binding)
    }

    fn inferred_hosted_catalog_leaf(
        machine_name: &str,
    ) -> Option<(
        &'static str,
        &'static str,
        &'static str,
        BoundaryIntrinsicShape,
    )> {
        match machine_name {
            "ConsoleNativeProvider::exit_process" => Some((
                "ConsoleNativeProvider",
                "Console",
                "exit_process",
                BoundaryIntrinsicShape::I32ToUnit,
            )),
            "ConsoleNativeProvider::write_byte" => Some((
                "ConsoleNativeProvider",
                "Console",
                "write_byte",
                BoundaryIntrinsicShape::I32ToUnit,
            )),
            "ConsoleNativeProvider::read_byte" => Some((
                "ConsoleNativeProvider",
                "Console",
                "read_byte",
                BoundaryIntrinsicShape::UnitToByteRead,
            )),
            "ProcessExitNativeProvider::exit_process" => Some((
                "ProcessExitNativeProvider",
                "ProcessExit",
                "exit_process",
                BoundaryIntrinsicShape::I32ToUnit,
            )),
            _ => None,
        }
    }

    fn satisfied_boundary_requirement<'typed>(
        typed: &'typed TypedTrees,
        machine: &'typed typed_trees::machine::Machine,
        conformance: &'typed typed_trees::machine::TraitConformance,
    ) -> Option<(&'typed TraitDefinition, &'typed StateSignature)> {
        let typed_trees::machine::SatisfiedDeclaration::Trait {
            definition,
            requirement,
        } = typed_trees::machine::resolve_satisfied_declaration(typed, machine, conformance)?
        else {
            return None;
        };
        (definition.is_boundary
            && definition.symbol == conformance.symbol
            && requirement.symbol == conformance.requirement_symbol)
            .then_some((definition, requirement))
    }

    pub(crate) fn inferred_hosted_console_compiler_intrinsic(
        typed: &TypedTrees,
        machine: &typed_trees::machine::Machine,
        conformance: &typed_trees::machine::TraitConformance,
        selected_target: Option<&str>,
        target_machine_origins: &[SelectedTargetMachineOrigin],
    ) -> Option<(ProviderBinding, SelectedTargetMachineOrigin)> {
        let (provider_name, trait_name, requirement_name, shape) =
            inferred_hosted_catalog_leaf(machine.name.as_str())?;
        let supports_target =
            |target: &str| HostedIntrinsicBundle::from_target_name(target).is_some();
        if selected_target.is_some_and(|target| !supports_target(target))
            || machine.supply_mode != language_semantics::MachineSupplyMode::Boundary
            || machine.body_is_present
            || machine.attached_data.as_ref().map(|name| name.as_str()) != Some(provider_name)
            || !machine.lifetime_parameters.is_empty()
            || !typed.machine_type_parameters(machine).is_empty()
            || conformance.name.as_str() != trait_name
            || conformance.requirement.as_ref().map(|name| name.as_str()) != Some(requirement_name)
            || conformance.external_binding.is_some()
            || conformance.via_expression.is_valid()
            || conformance.external_binding_source_span.is_some()
        {
            return None;
        }
        let origin = sole_machine_origin(target_machine_origins, machine.symbol, |origin| {
            match selected_target {
                Some(target) => origin.target == target,
                None => supports_target(&origin.target),
            }
        })?;
        let (definition, requirement) =
            satisfied_boundary_requirement(typed, machine, conformance)?;
        if definition.name.as_str() != trait_name
            || !definition.lifetime_parameters.is_empty()
            || !typed.trait_type_parameters(definition).is_empty()
            || requirement.name.as_str() != requirement_name
            || machine
                .name
                .as_str()
                .strip_prefix(provider_name)
                .and_then(|suffix| suffix.strip_prefix("::"))
                != Some(requirement.name.as_str())
            || !exact_catalog_signature(
                typed,
                typed.state_signature_parameters(requirement),
                requirement.return_type,
                definition.symbol,
                shape,
            )
        {
            return None;
        }
        let [entry] = typed.machine_states(machine) else {
            return None;
        };
        if !exact_catalog_signature(
            typed,
            typed.state_parameters(entry),
            entry.return_type,
            definition.symbol,
            shape,
        ) {
            return None;
        }
        let machine = typed.machine_callable_identity_text(machine)?;
        (!machine.is_empty()).then_some((
            ProviderBinding::CompilerIntrinsic { machine },
            (*origin).clone(),
        ))
    }

    pub(crate) fn inferred_selected_target_compiler_leaf(
        typed: &TypedTrees,
        machine: &typed_trees::machine::Machine,
        conformance: &typed_trees::machine::TraitConformance,
        selected_target: Option<&str>,
        target_machine_origins: &[SelectedTargetMachineOrigin],
    ) -> Option<(ProviderBinding, SelectedTargetMachineOrigin)> {
        let hosted = selected_target
            .is_none_or(|target| HostedIntrinsicBundle::from_target_name(target).is_some());
        if hosted
            || inferred_hosted_catalog_leaf(machine.name.as_str()).is_some()
            || machine.supply_mode != language_semantics::MachineSupplyMode::Boundary
            || machine.body_is_present
            || machine.attached_data.is_none()
            || !machine.lifetime_parameters.is_empty()
            || !typed.machine_type_parameters(machine).is_empty()
            || conformance.external_binding.is_some()
            || conformance.via_expression.is_valid()
            || conformance.external_binding_source_span.is_some()
        {
            return None;
        }
        let origin = sole_machine_origin(target_machine_origins, machine.symbol, |origin| {
            selected_target == Some(origin.target.as_str())
        })?;
        satisfied_boundary_requirement(typed, machine, conformance)?;
        let [_entry] = typed.machine_states(machine) else {
            return None;
        };
        let machine = typed.machine_callable_identity_text(machine)?;
        (!machine.is_empty()).then_some((
            ProviderBinding::CompilerIntrinsic { machine },
            (*origin).clone(),
        ))
    }

    pub(crate) fn inferred_compiler_leaf_binding(
        typed: &TypedTrees,
        machine: &typed_trees::machine::Machine,
        conformance: &typed_trees::machine::TraitConformance,
        selected_target: Option<&str>,
        target_machine_origins: &[SelectedTargetMachineOrigin],
    ) -> Option<(ProviderBinding, SelectedTargetMachineOrigin)> {
        inferred_hosted_console_compiler_intrinsic(
            typed,
            machine,
            conformance,
            selected_target,
            target_machine_origins,
        )
        .or_else(|| {
            inferred_selected_target_compiler_leaf(
                typed,
                machine,
                conformance,
                selected_target,
                target_machine_origins,
            )
        })
    }

    fn exact_catalog_signature(
        typed: &TypedTrees,
        parameters: &[typed_trees::signature::StateParameter],
        return_type: typed_trees::types::TypeReferenceHandle,
        trait_symbol: symbols::SymbolHandle,
        shape: BoundaryIntrinsicShape,
    ) -> bool {
        match shape {
            BoundaryIntrinsicShape::I32ToUnit => {
                exact_catalog_i32_to_unit_signature(typed, parameters, return_type)
            }
            BoundaryIntrinsicShape::UnitToByteRead => {
                parameters.is_empty() && exact_byte_read_type(typed, return_type, trait_symbol)
            }
            BoundaryIntrinsicShape::UnitToAddress
            | BoundaryIntrinsicShape::ProgramStorageToUnit => false,
        }
    }

    fn exact_catalog_i32_to_unit_signature(
        typed: &TypedTrees,
        parameters: &[typed_trees::signature::StateParameter],
        return_type: typed_trees::types::TypeReferenceHandle,
    ) -> bool {
        let [parameter] = parameters else {
            return false;
        };
        !parameter.is_const
            && !parameter.is_mutable
            && typed.primitive_type_reference(parameter.type_reference)
                == Some(typed_trees::types::PrimitiveType::I32)
            && is_unit_type(typed, return_type)
    }

    pub(crate) fn external_provider_binding(
        binding: &language_semantics::ExternalBindingIdentity,
        dispatch_table: &str,
        intrinsic_machine_identity: &str,
        table_field_storage: Result<effects::provider_plan::TableFieldStorage, String>,
    ) -> ProviderBinding {
        use language_semantics::ExternalBindingIdentity;

        match binding {
            ExternalBindingIdentity::Syscall { number } => {
                ProviderBinding::Syscall { number: *number }
            }
            ExternalBindingIdentity::CompilerIntrinsic => ProviderBinding::CompilerIntrinsic {
                machine: intrinsic_machine_identity.to_owned(),
            },
            ExternalBindingIdentity::VtableSlot { index } => {
                ProviderBinding::VtableSlot { index: *index }
            }
            ExternalBindingIdentity::VtableField { field } => ProviderBinding::VtableField {
                table: dispatch_table.to_owned(),
                field: field.clone(),
                storage: table_field_storage,
            },
            ExternalBindingIdentity::TableFunction { field } => ProviderBinding::TableFunction {
                table: dispatch_table.to_owned(),
                field: field.clone(),
                storage: table_field_storage,
            },
        }
    }

    pub(crate) fn realization_machine_identity(
        typed: &TypedTrees,
        machine: &typed_trees::machine::Machine,
    ) -> String {
        typed
            .machine_callable_identity_text(machine)
            .unwrap_or_default()
    }

    pub use symbols::same_semantic_name;

    pub fn satisfies_plan_name(target: &str, trait_name: &str, provider_type: &str) -> String {
        match (target.is_empty(), provider_type.is_empty()) {
            (true, true) => format!("satisfies::{trait_name}"),
            (false, true) => format!("{target}::satisfies::{trait_name}"),
            (true, false) => format!("{provider_type}::satisfies::{trait_name}"),
            (false, false) => format!("{target}::{provider_type}::satisfies::{trait_name}"),
        }
    }

    fn sole_machine_origin(
        target_machine_origins: &[SelectedTargetMachineOrigin],
        machine: symbols::SymbolHandle,
        admits: impl Fn(&SelectedTargetMachineOrigin) -> bool,
    ) -> Option<&SelectedTargetMachineOrigin> {
        let origins = target_machine_origins
            .iter()
            .filter(|origin| origin.machine == machine && admits(origin))
            .collect::<Vec<_>>();
        let [origin] = origins.as_slice() else {
            return None;
        };
        Some(origin)
    }
}
