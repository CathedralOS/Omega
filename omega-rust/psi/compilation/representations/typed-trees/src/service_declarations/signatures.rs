//! Prepare source-owned boundary telescopes before any target requests an ABI.
//!
//! Names, static-machine ordinals, nominal callback requirements and native
//! positions are declaration facts, not decisions for target materialization.
//! Unsupported projections remain deferred refusals; census construction never
//! rejects an unused declaration. Source coordinates stay beside, not inside,
//! the neutral telescope and belong only to this catalog's unchanged owner.

use crate::TypedTrees;
use crate::provider_declarations::ProviderPlanningContext;
use crate::types::TypeReferenceHandle;
use arena::Arena;
use language_semantics::callback_declarations::{
    BOUNDARY_NATIVE_PARAMETER_CAPACITY, BoundaryCallbackBinderDeclaration,
    BoundaryDirectCallbackDeclaration, BoundaryRuntimeFormal, BoundaryTelescope,
};
use semantic_vocabulary::{
    PackageKeyIdentity, callback_requirement_id, nominal_callback_binder_id,
    nominal_callback_native_parameter_id,
};
use symbols::SymbolHandle;

pub struct BoundaryCallbackSource {
    pub parameter_symbol: SymbolHandle,
    pub requirement_trait: SymbolHandle,
    pub requirement_machine: SymbolHandle,
}

pub struct PreparedBoundarySignature {
    pub telescope: BoundaryTelescope,
    pub callback_sources: Vec<BoundaryCallbackSource>,
    pub parameter_types: Vec<TypeReferenceHandle>,
    pub return_type: TypeReferenceHandle,
}

pub struct BoundarySignatureDeclaration {
    /// Default compatibility selection is Product-only. Explicit authored
    /// policies retain the whole checked input census, including Build copies.
    pub default_eligible: bool,
    pub symbol: SymbolHandle,
    pub package: Option<PackageKeyIdentity>,
    pub path: String,
    pub method: String,
    pub requirement_identity: String,
    pub projection: Result<PreparedBoundarySignature, String>,
}

impl Default for BoundarySignatureDeclaration {
    fn default() -> Self {
        Self {
            default_eligible: false,
            symbol: SymbolHandle::invalid(),
            package: None,
            path: String::new(),
            method: String::new(),
            requirement_identity: String::new(),
            projection: Err("boundary signature census has an absent declaration".into()),
        }
    }
}

pub struct PreparedBoundarySignatures {
    declarations: Arena<BoundarySignatureDeclaration>,
}

impl PreparedBoundarySignatures {
    pub(super) fn prepare(context: ProviderPlanningContext<'_>) -> Self {
        let typed = context.typed;
        let product = ProviderPlanningContext::new(typed, source::DependencyScope::Product);
        let mut declarations = Arena::new();
        // Authored policy evaluation walks the complete checked input. Keep
        // that population and mark the Product-only default subset separately.
        // Otherwise preparation silently loses Build-purpose policy premises.
        for definition in typed.traits() {
            for signature in typed.trait_machine_signatures(definition) {
                let requirement_identity =
                    typed.trait_requirement_callable_identity_text(definition, signature);
                let projection = prepare_trait_signature(typed, signature, &requirement_identity);
                declarations.insert(BoundarySignatureDeclaration {
                    default_eligible: product.contains(definition.symbol),
                    symbol: signature.symbol,
                    package: typed.symbols.symbol_package_identity(definition.symbol),
                    path: typed.trait_declaration_path(definition),
                    method: signature.name.as_str().to_owned(),
                    requirement_identity,
                    projection,
                });
            }
        }
        // Trait rows precede top-level rows, including deferred malformed
        // top-level projections. This order is part of overload diagnostics.
        for machine in typed.machines().iter().filter(|machine| {
            machine.supply_mode == language_semantics::MachineSupplyMode::TopLevelRequirement
        }) {
            let [entry] = typed.machine_states(machine) else {
                continue;
            };
            let requirement_identity = typed
                .machine_callable_identity_text(machine)
                .unwrap_or_default();
            let projection =
                prepare_top_level_signature(typed, machine, entry, &requirement_identity);
            declarations.insert(BoundarySignatureDeclaration {
                default_eligible: product.contains(machine.symbol),
                symbol: machine.symbol,
                package: typed.symbols.symbol_package_identity(machine.symbol),
                path: typed.symbols.display_path(machine.symbol, "::"),
                method: entry.name.as_str().to_owned(),
                requirement_identity,
                projection,
            });
        }
        Self { declarations }
    }

    pub fn declarations(&self) -> impl Iterator<Item = &BoundarySignatureDeclaration> {
        self.declarations.iter().map(|(_, declaration)| declaration)
    }

    pub fn for_requirement(
        &self,
        symbol: SymbolHandle,
        requirement: &str,
    ) -> Option<&BoundarySignatureDeclaration> {
        self.declarations().find(|declaration| {
            declaration.symbol == symbol && declaration.requirement_identity == requirement
        })
    }
}

fn prepare_top_level_signature(
    typed: &TypedTrees,
    machine: &crate::machine::Machine,
    entry: &crate::state::State,
    owner: &str,
) -> Result<PreparedBoundarySignature, String> {
    if machine.body_is_present
        || !machine.lifetime_parameters.is_empty()
        || !typed.machine_type_parameters(machine).is_empty()
    {
        return Err("compatibility calling plans require one nongeneric bodyless top-level boundary requirement".into());
    }
    let parameters = typed.state_parameters(entry);
    if parameters.len() > BOUNDARY_NATIVE_PARAMETER_CAPACITY {
        return Err(format!(
            "top-level boundary requirement has {} native parameters; calling policies currently support at most {BOUNDARY_NATIVE_PARAMETER_CAPACITY}",
            parameters.len()
        ));
    }
    let parameters = parameters
        .iter()
        .enumerate()
        .map(|(ordinal, parameter)| {
            Ok(BoundaryRuntimeFormal {
                identity: nominal_callback_native_parameter_id(owner, parameter.name.as_str()),
                native_ordinal: u32::try_from(ordinal).map_err(
                    |_| "top-level boundary requirement has too many runtime parameters",
                )?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(PreparedBoundarySignature {
        telescope: BoundaryTelescope {
            parameters,
            ..Default::default()
        },
        callback_sources: Vec::new(),
        parameter_types: typed
            .state_parameters(entry)
            .iter()
            .map(|parameter| parameter.type_reference)
            .collect(),
        return_type: entry.return_type,
    })
}

fn prepare_trait_signature(
    typed: &TypedTrees,
    signature: &crate::signature::StateSignature,
    owner: &str,
) -> Result<PreparedBoundarySignature, String> {
    let mut binders = Vec::new();
    let mut callback_sources = Vec::new();
    let mut static_machine_ordinal = 0u32;
    let invocations = crate::invocations::declared_signature_invocations(typed, signature);
    for parameter in typed.state_signature_type_parameters(signature) {
        let crate::data::TypeParameterKind::Machine { contract } = &parameter.kind else {
            continue;
        };
        let ordinal = static_machine_ordinal;
        static_machine_ordinal = static_machine_ordinal
            .checked_add(1)
            .ok_or_else(|| "boundary signature has too many static machine binders".to_owned())?;
        let crate::data::MachineParameterContract::Nominal {
            trait_definition,
            requirement,
            ..
        } = contract
        else {
            continue;
        };
        let owner_trait = typed
            .trait_definition_by_symbol(*trait_definition)
            .ok_or_else(|| "nominal callback binder lost its declaring trait".to_owned())?;
        let requirement_row = typed
            .trait_machine_signature_by_symbol(owner_trait, *requirement)
            .ok_or_else(|| "nominal callback binder lost its exact requirement".to_owned())?;
        let requirement_identity =
            typed.trait_requirement_callable_identity_text(owner_trait, requirement_row);
        binders.push(BoundaryCallbackBinderDeclaration {
            binder: nominal_callback_binder_id(owner, ordinal, parameter.name.as_str()),
            requirement: callback_requirement_id(&requirement_identity),
            static_machine_ordinal: ordinal,
            requirement_identity,
            invokes_synchronously: invocations
                .contains(&flow_effects::InvocationTarget::Service(*trait_definition)),
        });
        callback_sources.push(BoundaryCallbackSource {
            parameter_symbol: parameter.symbol,
            requirement_trait: *trait_definition,
            requirement_machine: *requirement,
        });
    }
    let runtime_parameters = typed.state_signature_parameters(signature);
    let total = runtime_parameters
        .len()
        .checked_add(signature.native_callback_parameters.len())
        .ok_or_else(|| "boundary native parameter telescope length overflow".to_owned())?;
    if total > BOUNDARY_NATIVE_PARAMETER_CAPACITY {
        return Err(format!(
            "boundary signature has {total} native parameters; calling policies currently support at most {BOUNDARY_NATIVE_PARAMETER_CAPACITY}"
        ));
    }
    let mut direct_callbacks = Vec::new();
    for direct in &signature.native_callback_parameters {
        let ordinal = usize::try_from(direct.native_ordinal)
            .map_err(|_| "native callback parameter ordinal exceeds usize".to_owned())?;
        if ordinal >= total {
            return Err(format!(
                "native callback parameter `{}` has telescope position {}, outside {} declared native parameters",
                direct.name, direct.native_ordinal, total
            ));
        }
        if direct_callbacks
            .iter()
            .any(|prior: &BoundaryDirectCallbackDeclaration| {
                prior.native_ordinal == direct.native_ordinal || prior.name == direct.name.as_str()
            })
            || runtime_parameters
                .iter()
                .any(|runtime| runtime.name.as_str() == direct.name.as_str())
        {
            return Err(format!(
                "native callback parameter `{}` repeats a native telescope position or declared parameter name",
                direct.name
            ));
        }
        let parameter = typed
            .state_signature_type_parameters(signature)
            .iter()
            .find(|parameter| parameter.name.as_str() == direct.binder.as_str())
            .ok_or_else(|| {
                format!(
                    "native callback parameter `{}` names unknown machine binder `{}`",
                    direct.name, direct.binder
                )
            })?;
        let binder = callback_sources.iter().position(|source| source.parameter_symbol == parameter.symbol)
            .and_then(|position| binders.get(position))
            .ok_or_else(|| format!("native callback parameter `{}` binder `{}` does not have one exact nominal callback requirement", direct.name, direct.binder))?;
        if direct_callbacks
            .iter()
            .any(|prior| prior.binder == binder.binder)
        {
            return Err(format!(
                "native callback binder `{}` is assigned to more than one declared native callback parameter",
                direct.binder
            ));
        }
        direct_callbacks.push(BoundaryDirectCallbackDeclaration {
            name: direct.name.as_str().to_owned(),
            identity: nominal_callback_native_parameter_id(owner, direct.name.as_str()),
            native_ordinal: direct.native_ordinal,
            binder: binder.binder,
            requirement: binder.requirement,
        });
    }
    direct_callbacks.sort_unstable_by_key(|parameter| parameter.native_ordinal);
    let mut native_ordinal = 0u32;
    let mut parameters = Vec::with_capacity(runtime_parameters.len());
    for parameter in runtime_parameters {
        while direct_callbacks
            .iter()
            .any(|direct| direct.native_ordinal == native_ordinal)
        {
            native_ordinal = native_ordinal
                .checked_add(1)
                .ok_or_else(|| "boundary native parameter ordinal overflow".to_owned())?;
        }
        parameters.push(BoundaryRuntimeFormal {
            identity: nominal_callback_native_parameter_id(owner, parameter.name.as_str()),
            native_ordinal,
        });
        native_ordinal = native_ordinal
            .checked_add(1)
            .ok_or_else(|| "boundary native parameter ordinal overflow".to_owned())?;
    }
    Ok(PreparedBoundarySignature {
        telescope: BoundaryTelescope {
            parameters,
            binders,
            direct_callbacks,
        },
        callback_sources,
        parameter_types: runtime_parameters
            .iter()
            .map(|parameter| parameter.type_reference)
            .collect(),
        return_type: signature.return_type,
    })
}
