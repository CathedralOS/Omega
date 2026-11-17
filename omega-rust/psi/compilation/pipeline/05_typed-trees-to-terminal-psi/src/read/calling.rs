//! Prepare only explicitly selected calling-policy applications from their source owner.
//! Policy bodies follow the ordinary machine and conformance readers. ABI storage
//! is prepared by the common metadata-storage owner, independently of value use.

use std::sync::Arc;

use semantic_vocabulary::{CallingSignatureId, MachineId};
use symbols::SymbolHandle;
use terminal_check_tables::Checked;
use typed_trees::TypedTrees;
use typed_trees::service_declarations::signatures::PreparedBoundaryAbiSignature;
use typed_trees::service_declarations::{
    PreparedServiceCatalog, provider_declarations::ProviderPlanningContext,
};
use typed_trees::trait_definition::{
    AppliedTraitScope, TraitApplicationArguments, TraitApplicationEnvironment,
};

use super::{Program, services};

/// This is source selection context, not executable or package authority.
#[derive(Debug, Clone, Copy)]
pub struct CallingPolicySelections<'source> {
    pub source: &'source TypedTrees,
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
pub(crate) struct SourceCallingPolicies {
    pub(crate) requests: Vec<SourceCallingRequest>,
    catalog: Option<Arc<PreparedServiceCatalog>>,
    pub(crate) signatures: Vec<SourceCallingSignature>,
    pub(crate) entries: Vec<SourceProgramEntryContract>,
    completed_applications: Vec<(SymbolHandle, SymbolHandle, String)>,
}

pub(crate) struct SourceCallingSignature {
    pub(crate) id: CallingSignatureId,
    row: usize,
}

impl SourceCallingPolicies {
    pub(crate) fn signatures<'source>(
        &'source self,
        typed: &'source TypedTrees,
        context: MachineId,
    ) -> Checked<impl ExactSizeIterator<Item = Checked<&'source PreparedBoundaryAbiSignature>>>
    {
        let rows = match &self.catalog {
            Some(catalog) => catalog
                .bind(ProviderPlanningContext::new(
                    typed,
                    source::DependencyScope::Product,
                ))
                .map_err(|diagnostic| services::unavailable(context, diagnostic.message))?
                .signatures()
                .abi_signatures(),
            None => &[],
        };
        Ok(self.signatures.iter().map(move |signature| {
            rows.get(signature.row)
                .filter(|row| row.id == signature.id)
                .ok_or_else(|| {
                    services::unavailable(
                        context,
                        "selected calling ABI lost its exact prepared catalog row",
                    )
                })
        }))
    }
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
    pub(crate) fn require_complete_boundary_parent_contracts(
        &self,
        context: MachineId,
        prepared: &SourceCallingPolicies,
    ) -> Checked<()> {
        for environment in &self.boundary_parent_contracts {
            let scope = environment.scope(self.typed).ok_or_else(|| {
                services::unavailable(
                    context,
                    "boundary parent contract lost its exact application",
                )
            })?;
            let identity = application_identity(self.typed, &scope).ok_or_else(|| {
                services::unavailable(
                    context,
                    "boundary parent contract has no complete application identity",
                )
            })?;
            if !prepared.completed_applications.contains(&(
                scope.declaration.symbol,
                scope.argument_owner,
                identity,
            )) {
                return Err(services::unavailable(
                    context,
                    "boundary parent contract has no admitted contract owner",
                ));
            }
        }
        Ok(())
    }

    pub(crate) fn prepare_calling_policies<'source>(
        &mut self,
        context: MachineId,
        selected: Option<CallingPolicySelections<'source>>,
        prepared: &mut SourceCallingPolicies,
    ) -> Checked<bool> {
        let typed = self.typed;
        let (applications, entries) = match selected {
            Some(selected) => {
                if !std::ptr::eq(typed, selected.source) {
                    return Err(services::unavailable(
                        context,
                        "calling policy selections belong to another source owner",
                    ));
                }
                (selected.applications, selected.entries)
            }
            None => (&[][..], &[][..]),
        };
        let context_source = ProviderPlanningContext::new(typed, source::DependencyScope::Product);
        let environments = applications
            .iter()
            .cloned()
            .chain(entries.iter().map(|entry| entry.application.clone()))
            .chain(self.boundary_parent_contracts.iter().cloned())
            .collect::<Vec<_>>();
        if environments.is_empty() && entries.is_empty() {
            return Ok(false);
        }
        let before = prepared.completed_applications.len();
        let prepared_catalog = match &prepared.catalog {
            Some(catalog) => Arc::clone(catalog),
            None => {
                let catalog = self
                    .shared
                    .calling_catalog(typed, source::DependencyScope::Product)
                    .as_ref()
                    .map(Arc::clone)
                    .map_err(|reason| services::unavailable(context, reason.clone()))?;
                prepared.catalog = Some(Arc::clone(&catalog));
                catalog
            }
        };
        let bound_catalog = prepared_catalog
            .bind(context_source)
            .map_err(|diagnostic| services::unavailable(context, diagnostic.message))?;
        let catalog = bound_catalog.signatures();
        let calling = catalog.calling();
        for environment in &environments {
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
            let coordinate = (
                scope.declaration.symbol,
                scope.argument_owner,
                owner.clone(),
            );
            if prepared.completed_applications.contains(&coordinate) {
                continue;
            }
            if !scope.declaration.is_boundary {
                return Err(services::unavailable(
                    context,
                    "selected calling schema is not a boundary declaration",
                ));
            }
            self.served(context, scope.declaration.symbol)?;
            // Schema keys are exact nominal owners. A member's semantic
            // declaration is a different identity and cannot name its trait.
            let declaration = typed_trees::service_declarations::exact_identity(
                typed,
                scope.declaration.symbol,
                bound_catalog.exact_toolchain_sources(),
            )
            .map_err(|diagnostic| services::unavailable(context, diagnostic.message))?;
            let schema = typed_trees::service_declarations::projection::from_typed_application(
                typed_trees::provider_declarations::ProviderPlanningContext::new(
                    typed,
                    source::DependencyScope::Product,
                ),
                scope.clone(),
            )
            .map_err(|diagnostic| services::unavailable(context, diagnostic.message))?;
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
            // The Calling owner consumes only the relationship it actually
            // prepared. Every other ordinary parent remains an obligation.
            scope
                .try_visit(typed, &mut |parent| {
                    if !parent.declaration.is_boundary
                        && (parent.declaration.symbol != request.relationship
                            || !typed
                                .trait_machine_signatures(parent.declaration)
                                .is_empty())
                    {
                        return Err(
                            "boundary parent contract is not covered by its admitted relationship",
                        );
                    }
                    Ok(())
                })
                .map_err(|reason| services::unavailable(context, reason))?;
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
            let implementation = policy
                .realization(typed, policy.slot)
                .map_err(|reason| services::unavailable(context, reason))?
                .symbol;
            // Formation/refinement is the ordinary exact selected conformance,
            // never an inferred contract fabricated from the machine body.
            if !self.conformance_declaration(context, policy.provider)? {
                return Err(services::unavailable(
                    context,
                    "Calling policy has no exact source implementation declaration",
                ));
            }
            let policy = self.machine(implementation)?;
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
                    let mut rows = catalog
                        .abi_signatures()
                        .iter()
                        .enumerate()
                        .filter(|(_, row)| row.id == id);
                    let (row_index, row) = match (rows.next(), rows.next()) {
                        (Some(row), None) => row,
                        _ => {
                            return Err(services::unavailable(
                                context,
                                "Calling signature has no unique complete ABI key",
                            ));
                        }
                    };
                    if !row
                        .projections_in_context(typed, self.purpose)
                        .any(|projection| projection.source == source.symbol)
                    {
                        return Err(services::unavailable(
                            context,
                            "Calling ABI key does not retain its exact requirement source",
                        ));
                    }
                    if !signatures.contains(&id) {
                        signatures.push(id);
                    }
                    if !prepared.signatures.iter().any(|known| known.id == id) {
                        prepared
                            .signatures
                            .push(SourceCallingSignature { id, row: row_index });
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
            prepared.completed_applications.push(coordinate);
        }
        let mut entry_targets = Vec::with_capacity(entries.len());
        for entry in entries {
            if entry_targets.contains(&entry.target) {
                return Err(services::unavailable(
                    context,
                    "selected entry target has competing source contract nominations",
                ));
            }
            entry_targets.push(entry.target);
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
            // Schema keys are exact nominal owners. A member's semantic
            // declaration is a different identity and cannot name its trait.
            let declaration = typed_trees::service_declarations::exact_identity(
                typed,
                scope.declaration.symbol,
                bound_catalog.exact_toolchain_sources(),
            )
            .map_err(|diagnostic| services::unavailable(context, diagnostic.message))?;
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
            let implementation = self.machine(entry.implementation)?;
            if let Some(known) = prepared
                .entries
                .iter()
                .find(|known| known.target == entry.target)
            {
                if known.declaration != declaration
                    || known.application != application
                    || known.physical_source_digest != entry.physical_source_digest
                    || known.entry != implementation
                {
                    return Err(services::unavailable(
                        context,
                        "selected entry target has competing source contract nominations",
                    ));
                }
                continue;
            }
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
        Ok(prepared.completed_applications.len() != before)
    }
}

#[cfg(test)]
mod inherited_contract_owner_tests {
    use super::*;
    use source::{SourceMap, SourceOrigin, SourceSpan, Span};
    use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
    use typed_trees::name::Identifier;
    use typed_trees::signature::StateSignature;
    use typed_trees::trait_definition::{TraitDefinition, TraitRequirement};

    #[test]
    fn inherited_callable_parents_require_completion_of_the_exact_application() {
        let mut sources = SourceMap::default();
        let source = sources
            .add_with_metadata(
                "parents/main.omg".into(),
                "Child Parent Other".into(),
                "parents".into(),
                semantic_vocabulary::PackageKeyIdentity::from_digest([9; 32]),
                SourceOrigin::User,
            )
            .source_id;
        let mut symbols = SymbolTableBuilder::with_sources(Some(Arc::new(sources)));
        let root = symbols.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let declarations = symbols.insert_children(
            root,
            [(0, 5), (6, 12), (13, 18)].map(|(start, end)| {
                (
                    SymbolKind::Trait,
                    SymbolNameRef::Source(SourceSpan::new(source, Span::new(start, end))),
                )
            }),
        );
        let child = declarations.handle_at(0);
        let parent = declarations.handle_at(1);
        let other = declarations.handle_at(2);
        let requirement = symbols
            .insert_children(
                parent,
                [(SymbolKind::State, SymbolNameRef::Static("enter"))],
            )
            .handle_at(0);
        let mut typed = TypedTrees {
            symbols: symbols.finish(),
            ..Default::default()
        };
        let unit = typed
            .type_reference_table
            .insert(typed_trees::types::TypeReferenceNode::Unit);
        let machines = typed.trait_machine_signatures.insert_many([StateSignature {
            symbol: requirement,
            name: Identifier::from("enter"),
            return_type: unit,
            ..Default::default()
        }]);
        typed.push_trait_definition(TraitDefinition {
            symbol: parent,
            name: Identifier::from("Parent"),
            is_boundary: true,
            machines,
            ..Default::default()
        });
        let parents = typed.trait_requirements.insert_many([TraitRequirement {
            symbol: parent,
            name: Identifier::from("Parent"),
            ..Default::default()
        }]);
        typed.push_trait_definition(TraitDefinition {
            symbol: child,
            name: Identifier::from("Child"),
            is_boundary: true,
            requires: parents,
            ..Default::default()
        });
        typed.push_trait_definition(TraitDefinition {
            symbol: other,
            name: Identifier::from("Other"),
            is_boundary: true,
            requires: parents,
            ..Default::default()
        });
        let scope =
            AppliedTraitScope::abstract_scope(typed.trait_definition_by_symbol(child).unwrap());
        let identity = application_identity(&typed, &scope).unwrap();
        let other_scope =
            AppliedTraitScope::abstract_scope(typed.trait_definition_by_symbol(other).unwrap());
        let other_identity = application_identity(&typed, &other_scope).unwrap();
        let context = MachineId::new(1).unwrap();
        let mut program = Program::new(&typed, false);
        program
            .require_boundary_parent_contract(context, &scope)
            .unwrap();
        assert_eq!(program.boundary_parent_contracts.len(), 1);
        let mut policies = SourceCallingPolicies::default();
        for wrong in [
            None,
            Some((other, other_scope.argument_owner, other_identity.clone())),
            Some((child, other, identity.clone())),
            Some((child, scope.argument_owner, other_identity)),
        ] {
            policies.completed_applications = wrong.into_iter().collect();
            let refused = program
                .require_complete_boundary_parent_contracts(context, &policies)
                .expect_err("another completed application cannot supply the inherited contract");
            assert_eq!(
                refused.detail,
                "boundary parent contract has no admitted contract owner"
            );
        }
    }
}
