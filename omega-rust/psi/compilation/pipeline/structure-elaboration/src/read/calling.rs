//! Prepare only explicitly selected calling-policy applications from their frozen owner.
//! Policy bodies follow the ordinary machine and conformance readers. ABI storage
//! is prepared by the common metadata-storage owner, independently of value use.

use semantic_vocabulary::{CallingSignatureId, MachineId};
use symbols::SymbolHandle;
use terminal_check_tables::Checked;
use typed_trees::service_declarations::signatures::PreparedBoundaryAbiSignature;
use typed_trees::trait_definition::{
    AppliedTraitScope, TraitApplicationArguments, TraitApplicationEnvironment,
};
use typed_trees::{FrozenTypedTrees, TypedTrees};

use super::{Program, services};

/// This is source selection context, not executable or package authority.
#[derive(Debug, Clone, Copy)]
pub struct CallingPolicySelections<'source> {
    pub source: &'source FrozenTypedTrees,
    pub applications: &'source [TraitApplicationEnvironment],
    pub entries: &'source [ProgramEntryContractSelection<'source>],
}

#[derive(Debug, Clone, Copy)]
pub struct ProgramEntryContractSelection<'source> {
    pub target: &'source str,
    pub application: &'source TraitApplicationEnvironment,
    pub physical_source_digest: [u8; 32],
    pub implementation: SymbolHandle,
}

#[derive(Default)]
pub(crate) struct SourceCallingPolicies<'source> {
    pub(crate) requests: Vec<SourceCallingRequest>,
    pub(crate) signatures: Vec<&'source PreparedBoundaryAbiSignature>,
    pub(crate) entries: Vec<SourceProgramEntryContract>,
}

pub(crate) struct SourceProgramEntryContract {
    pub(crate) target: String,
    pub(crate) declaration: String,
    pub(crate) application: String,
    pub(crate) physical_source_digest: [u8; 32],
    pub(crate) entry: MachineId,
}

pub(crate) struct SourceCallingRequest {
    pub(crate) declaration: String,
    pub(crate) display_name: String,
    pub(crate) instances: Vec<SourceCallingInstance>,
}

pub(crate) struct SourceCallingInstance {
    pub(crate) declaring_application: String,
    pub(crate) policy: MachineId,
    pub(crate) signatures: Vec<CallingSignatureId>,
    pub(crate) schema: std::sync::Arc<language_semantics::service_surface::ServiceSurface>,
}

fn application_identity(typed: &TypedTrees, scope: &AppliedTraitScope<'_>) -> Option<String> {
    scope
        .portable_application_identity(typed)
        .and_then(|identity| typed.type_identity_text(identity))
        .filter(|identity| !identity.is_empty())
}

impl Program<'_> {
    pub(crate) fn prepare_calling_policies<'source>(
        &mut self,
        context: MachineId,
        selected: CallingPolicySelections<'source>,
    ) -> Checked<SourceCallingPolicies<'source>> {
        let typed = self.typed;
        if !std::ptr::eq(typed, &**selected.source) {
            return Err(services::unavailable(
                context,
                "calling policy selections belong to another frozen source owner",
            ));
        }
        let catalog = selected.source.boundary_signatures();
        let calling = catalog.calling();
        let mut prepared = SourceCallingPolicies::default();
        let mut seen = std::collections::HashSet::<
            (SymbolHandle, SymbolHandle, String),
            symbols::BuildSymbolHasher,
        >::default();
        for environment in selected
            .applications
            .iter()
            .chain(selected.entries.iter().map(|entry| entry.application))
        {
            let scope = environment.scope(typed).ok_or_else(|| {
                services::unavailable(
                    context,
                    "selected calling schema has no current exact application",
                )
            })?;
            let owner = application_identity(typed, &scope).ok_or_else(|| {
                services::unavailable(
                    context,
                    "selected calling schema has no complete application identity",
                )
            })?;
            if !seen.insert((
                scope.declaration.symbol,
                scope.argument_owner,
                owner.clone(),
            )) {
                continue;
            }
            if !scope.declaration.is_boundary {
                return Err(services::unavailable(
                    context,
                    "selected calling schema is not a boundary declaration",
                ));
            }
            self.served(context, scope.declaration.symbol)?;
            let declaration = typed
                .normalized_hermetic_symbol_identity(scope.declaration.symbol)
                .map_err(|reason| services::unavailable(context, reason))?;
            let schema = typed_trees::service_declarations::projection::from_typed_application(
                typed_trees::provider_declarations::ProviderPlanningContext::new(
                    typed,
                    source::DependencyScope::Product,
                ),
                scope.clone(),
            )
            .ok_or_else(|| {
                services::unavailable(
                    context,
                    "selected calling schema has no complete normalized source projection",
                )
            })?;
            schema
                .validate_correspondence()
                .map_err(|diagnostic| services::unavailable(context, diagnostic.message))?;
            if schema.declaration.root_application_identity() != Some(owner.as_str()) {
                return Err(services::unavailable(
                    context,
                    "selected schema projection changed its complete root application",
                ));
            }
            let mut requests = calling
                .requests()
                .filter(|request| request.boundary == scope.declaration.symbol);
            let request = match (requests.next(), requests.next()) {
                (Some(request), None) => request,
                _ => {
                    return Err(services::unavailable(
                        context,
                        "selected schema has no unique prepared Calling relationship",
                    ));
                }
            };
            let instances = calling.instances(request).map_err(|diagnostics| {
                services::unavailable(
                    context,
                    diagnostics
                        .into_iter()
                        .map(|diagnostic| diagnostic.message)
                        .collect::<Vec<_>>()
                        .join("; "),
                )
            })?;
            let mut matched = None;
            for instance in instances {
                let candidate = AppliedTraitScope::new(
                    typed,
                    scope.declaration,
                    TraitApplicationArguments {
                        lexical_owner: instance.argument_owner,
                        enclosing_data_applications: &[],
                        arguments: calling.arguments(instance),
                        lifetimes: &[],
                    },
                )
                .ok_or_else(|| {
                    services::unavailable(
                        context,
                        "prepared Calling instance lost its complete source application",
                    )
                })?;
                let candidate = application_identity(typed, &candidate).ok_or_else(|| {
                    services::unavailable(
                        context,
                        "prepared Calling instance has no portable application identity",
                    )
                })?;
                if candidate == owner
                    && instance.argument_owner == scope.argument_owner
                    && matched.replace(instance).is_some()
                {
                    return Err(services::unavailable(
                        context,
                        "selected Calling application has competing prepared instances",
                    ));
                }
            }
            let instance = matched.ok_or_else(|| {
                services::unavailable(
                    context,
                    "selected schema application has no exactly corresponding Calling instance",
                )
            })?;
            let policy = *instance
                .policy
                .as_ref()
                .map_err(|reason| services::unavailable(context, reason.clone()))?;
            let implementation = selected_policy_implementation(typed, &scope)
                .map_err(|reason| services::unavailable(context, reason))?;
            // Formation/refinement is the ordinary exact selected conformance,
            // never an inferred contract fabricated from the machine body.
            if !self.conformance_declaration(context, implementation)? {
                return Err(services::unavailable(
                    context,
                    "Calling policy has no exact source implementation declaration",
                ));
            }
            let policy = self.machine(policy)?;
            if !self.fixed_inputs.contains(&policy) {
                self.fixed_inputs.push(policy);
            }
            self.admitted_declarations.include(scope.declaration.symbol);
            let mut signatures = Vec::new();
            for signature in calling.signatures(instance) {
                if let Some(reason) = &signature.abi_refusal {
                    return Err(services::unavailable(context, reason.clone()));
                }
                let source = catalog.applied_declaration(signature).ok_or_else(|| {
                    services::unavailable(
                        context,
                        "Calling instance lost an exact requirement declaration",
                    )
                })?;
                let declaring = signature
                    .declaring_scope
                    .as_ref()
                    .and_then(|scope| scope.scope(typed))
                    .ok_or_else(|| {
                        services::unavailable(
                            context,
                            "Calling signature lost its exact declaring application",
                        )
                    })?;
                self.admitted_declarations
                    .include(declaring.declaration.symbol);
                self.admitted_declarations.include(source.symbol);
                for &id in &signature.abi_signatures {
                    let mut rows = catalog.abi_signatures().iter().filter(|row| row.id == id);
                    let row = match (rows.next(), rows.next()) {
                        (Some(row), None) => row,
                        _ => {
                            return Err(services::unavailable(
                                context,
                                "Calling signature has no unique complete ABI key",
                            ));
                        }
                    };
                    if !row.source_templates.contains(&source.symbol) {
                        return Err(services::unavailable(
                            context,
                            "Calling ABI key does not retain its exact requirement source",
                        ));
                    }
                    if !signatures.contains(&id) {
                        signatures.push(id);
                    }
                    if !prepared.signatures.iter().any(|known| known.id == id) {
                        prepared.signatures.push(row);
                    }
                }
                if signature.abi_signatures.is_empty() {
                    return Err(services::unavailable(
                        context,
                        "Calling requirement has no prepared ABI application",
                    ));
                }
            }
            let new_instance = SourceCallingInstance {
                declaring_application: owner,
                policy,
                signatures,
                schema: schema.declaration,
            };
            match prepared
                .requests
                .iter_mut()
                .find(|known| known.declaration == declaration)
            {
                Some(known) => {
                    if let Some(existing) = known.instances.iter().find(|known| {
                        known.declaring_application == new_instance.declaring_application
                    }) {
                        if existing.policy != new_instance.policy
                            || existing.signatures != new_instance.signatures
                            || existing.schema != new_instance.schema
                        {
                            return Err(services::unavailable(
                                context,
                                "equal Calling applications select different policies or signatures",
                            ));
                        }
                    } else {
                        known.instances.push(new_instance);
                    }
                }
                None => prepared.requests.push(SourceCallingRequest {
                    declaration,
                    display_name: request.name.clone(),
                    instances: vec![new_instance],
                }),
            }
        }
        for entry in selected.entries {
            if entry.target.is_empty() {
                return Err(services::unavailable(
                    context,
                    "selected entry contract has no exact target coordinate",
                ));
            }
            let scope = entry.application.scope(typed).ok_or_else(|| {
                services::unavailable(
                    context,
                    "selected entry contract lost its complete source application",
                )
            })?;
            let application = application_identity(typed, &scope).ok_or_else(|| {
                services::unavailable(
                    context,
                    "selected entry contract has no canonical full application identity",
                )
            })?;
            let declaration = typed
                .normalized_hermetic_symbol_identity(scope.declaration.symbol)
                .map_err(|reason| services::unavailable(context, reason))?;
            if !prepared.requests.iter().any(|request| {
                request.declaration == declaration
                    && request
                        .instances
                        .iter()
                        .any(|instance| instance.declaring_application == application)
            }) {
                return Err(services::unavailable(
                    context,
                    "selected entry contract has no admitted normalized schema application",
                ));
            }
            if prepared
                .entries
                .iter()
                .any(|known| known.target == entry.target)
            {
                return Err(services::unavailable(
                    context,
                    "selected entry target has competing source contract nominations",
                ));
            }
            let implementation = self.machine(entry.implementation)?;
            if !self.fixed_inputs.contains(&implementation) {
                self.fixed_inputs.push(implementation);
            }
            prepared.entries.push(SourceProgramEntryContract {
                target: entry.target.to_owned(),
                declaration,
                application,
                physical_source_digest: entry.physical_source_digest,
                entry: implementation,
            });
        }
        prepared.signatures.sort_by_key(|signature| signature.id);
        Ok(prepared)
    }
}

fn selected_policy_implementation(
    typed: &TypedTrees,
    boundary: &AppliedTraitScope<'_>,
) -> Result<SymbolHandle, &'static str> {
    let vocabularies = typed_trees::calling_vocabulary::standard_calling_vocabularies(typed);
    let mut selected = None;
    for (calling, _) in vocabularies {
        let Some(scope) = boundary.declaring_scope(typed, calling.symbol) else {
            continue;
        };
        for (_, candidate) in typed.trait_implementation_applications.iter() {
            let Some(source) = candidate.source_scope.scope(typed) else {
                continue;
            };
            if source.argument_owner == scope.argument_owner
                && source.same_application(typed, &scope)
            {
                if selected
                    .replace(candidate.application.declaration)
                    .is_some()
                {
                    return Err("Calling policy has competing exact implementation applications");
                }
            }
        }
    }
    selected.ok_or("Calling policy lost its exact implementation application")
}
