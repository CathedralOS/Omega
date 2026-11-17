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
    headers: Vec<SourceCallingHeader>,
}

struct SourceCallingHeader {
    requirement: SymbolHandle,
    declaring: TraitApplicationEnvironment,
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
    /// Calling metadata admits the same complete source header as a call does,
    /// without supplying a body or creating a boundary invocation. Read the
    /// prepared application, not its unapplied declaration's clause handles.
    fn read_calling_requirement_header(
        &mut self,
        context: MachineId,
        scope: &AppliedTraitScope<'_>,
        requirement: SymbolHandle,
    ) -> Checked<crate::body::Signature> {
        let typed = self.typed;
        let (owner, declared) = typed
            .trait_machine_signature(requirement)
            .filter(|(owner, _)| owner.is_boundary && owner.symbol == scope.declaration.symbol)
            .ok_or_else(|| {
                services::unavailable(
                    context,
                    "Calling requirement lost its exact declaring boundary header",
                )
            })?;
        let source = typed
            .applied_callable_signature(scope, requirement)
            .or_else(|| scope.keeps_declared_signatures(typed).then_some(declared))
            .ok_or_else(|| {
                services::unavailable(
                    context,
                    "Calling requirement lost its prepared source contract application",
                )
            })?;
        let (signature, _) = self
            .boundary_signature(context, source)
            .map_err(|refusal| self.read_failure_at(refusal, requirement, "the Calling header"))?;
        self.admitted_declarations.include(owner.symbol);
        Ok(signature)
    }
    /// Find the services and entry contracts this header names while their
    /// calling policies can still add machine bodies. Read its types and
    /// clauses later during storage preparation.
    fn prepare_calling_header_dependencies(
        &mut self,
        context: MachineId,
        source: &typed_trees::signature::StateSignature,
    ) -> Checked<()> {
        let reach_author = self
            .typed
            .closed_requirement_application(source.symbol)
            .map_or(source.symbol, |closed| closed.template);
        self.reach_ceiling(context, reach_author, false)?;
        let parameters = super::span(
            |span| self.typed.state_parameters.span(span),
            source.parameters,
            context,
            "Calling header parameter span",
        )?;
        self.entry_ceiling(
            context,
            self.typed.state_signature_invokes(source),
            parameters,
            true,
        )?;
        Ok(())
    }

    fn prepare_calling_requirement_header(
        &mut self,
        context: MachineId,
        declaring: &AppliedTraitScope<'_>,
        requirement: SymbolHandle,
        prepared: &mut SourceCallingPolicies,
    ) -> Checked<()> {
        let typed = self.typed;
        let (_, declared) = typed
            .trait_machine_signature(requirement)
            .filter(|(owner, _)| owner.is_boundary && owner.symbol == declaring.declaration.symbol)
            .ok_or_else(|| {
                services::unavailable(
                    context,
                    "Calling requirement lost its exact declaring boundary header",
                )
            })?;
        let applied = typed
            .applied_callable_signature(declaring, requirement)
            .or_else(|| {
                declaring
                    .keeps_declared_signatures(typed)
                    .then_some(declared)
            })
            .ok_or_else(|| {
                services::unavailable(
                    context,
                    "Calling requirement lost its prepared source contract application",
                )
            })?;
        self.prepare_calling_header_dependencies(context, applied)
            .map_err(|refusal| self.read_failure_at(refusal, requirement, "the Calling header"))?;
        let header = SourceCallingHeader {
            requirement,
            declaring: declaring.environment(),
        };
        if !prepared.headers.iter().any(|known| {
            known.requirement == header.requirement && known.declaring == header.declaring
        }) {
            prepared.headers.push(header);
        }
        Ok(())
    }

    pub(super) fn read_calling_requirement_headers(
        &mut self,
        context: MachineId,
        prepared: &SourceCallingPolicies,
    ) -> Checked<()> {
        if self.type_purpose != super::storage_metadata::TypePurpose::StorageDescription {
            return Err(services::unavailable(
                context,
                "Calling headers before the complete body closure",
            ));
        }
        for header in &prepared.headers {
            let scope = header.declaring.scope(self.typed).ok_or_else(|| {
                services::unavailable(
                    context,
                    "Calling header lost its exact declaring application",
                )
            })?;
            self.read_calling_requirement_header(context, &scope, header.requirement)?;
        }
        Ok(())
    }

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
                let sources = source::source_identity::toolchain_source_identities(
                    typed.symbols.source_files(),
                )
                .map_err(|reason| services::unavailable(context, reason))?;
                let catalog = Arc::new(PreparedServiceCatalog::prepare(context_source, &sources));
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
                self.prepare_calling_requirement_header(
                    context,
                    &declaring,
                    source.symbol,
                    prepared,
                )?;
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

#[cfg(test)]
mod header_reading_tests {
    use super::*;
    use language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionExposure as Exposure, AuthoredDeclarationSelectionKind as Kind,
        AuthoredDeclarationSelectionLateBinding as Binding,
    };
    use symbols::{
        BuiltinTypeAtom, SymbolKind, SymbolNameRef, SymbolTableBuilder, builtin_type_symbols,
    };
    use typed_trees::domain::ProofFact;
    use typed_trees::expression::{
        BinaryOperator, ExpressionNode, TableBinaryExpression, TableNamePath, TableUnaryExpression,
        UnaryOperator,
    };
    use typed_trees::signature::{
        SignatureContract, SignatureContractKind, StateParameter, StateSignature,
    };
    use typed_trees::trait_definition::{TraitApplicationArguments, TraitDefinition};
    use typed_trees::types::TypeReferenceNode;

    fn fixture(valid_predicate: bool) -> (TypedTrees, SymbolHandle, SymbolHandle, SymbolHandle) {
        header_fixture(valid_predicate, false)
    }

    fn header_fixture(
        valid_predicate: bool,
        linear: bool,
    ) -> (TypedTrees, SymbolHandle, SymbolHandle, SymbolHandle) {
        let mut builder = SymbolTableBuilder::new();
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let children = builder.insert_children(
            root,
            builtin_type_symbols().into_iter().chain([
                (SymbolKind::Trait, SymbolNameRef::Static("Raw")),
                (SymbolKind::Trait, SymbolNameRef::Static("Other")),
                (SymbolKind::Data, SymbolNameRef::Static("Token")),
                (SymbolKind::Machine, SymbolNameRef::Static("clause_helper")),
            ]),
        );
        let owner = children.handles().nth(symbols::BUILTIN_TYPE_COUNT).unwrap();
        let other = children
            .handles()
            .nth(symbols::BUILTIN_TYPE_COUNT + 1)
            .unwrap();
        let requirement = builder
            .insert_children(owner, [(SymbolKind::State, SymbolNameRef::Static("read"))])
            .start();
        let parameters = builder.insert_children(
            requirement,
            [
                (SymbolKind::Parameter, SymbolNameRef::Static("left")),
                (SymbolKind::Parameter, SymbolNameRef::Static("right")),
            ],
        );
        let token = children
            .handles()
            .nth(symbols::BUILTIN_TYPE_COUNT + 2)
            .unwrap();
        let helper = children
            .handles()
            .nth(symbols::BUILTIN_TYPE_COUNT + 3)
            .unwrap();
        let token_members = builder.insert_children(
            token,
            [
                (SymbolKind::Field, SymbolNameRef::Static("allowed")),
                (SymbolKind::Domain, SymbolNameRef::Static("Allowed")),
            ],
        );
        let field = token_members.handle_at(0);
        let domain = token_members.handle_at(1);
        let helper_entry = builder
            .insert_children(
                helper,
                [(SymbolKind::State, SymbolNameRef::Static("entry"))],
            )
            .start();
        let helper_parameter = builder
            .insert_children(
                helper_entry,
                [(SymbolKind::Parameter, SymbolNameRef::Static("flag"))],
            )
            .start();
        let mut typed = TypedTrees {
            symbols: builder.finish(),
            ..Default::default()
        };
        let boolean_symbol = typed
            .symbols
            .builtin_atom_symbol(BuiltinTypeAtom::Bool)
            .unwrap();
        let boolean = typed.type_reference_table.insert(TypeReferenceNode::Named {
            symbol: boolean_symbol,
            name: "bool".into(),
        });
        let unit = typed.type_reference_table.insert(TypeReferenceNode::Unit);
        let token_type = typed.type_reference_table.insert(TypeReferenceNode::Named {
            symbol: token,
            name: "Token".into(),
        });
        let members = typed
            .data_members
            .insert_many([typed_trees::data::DataMember::Field(
                typed_trees::data::DataField {
                    symbol: field,
                    name: "allowed".into(),
                    type_reference: boolean,
                    ..Default::default()
                },
            )]);
        typed.push_data_definition(typed_trees::data::DataDefinition {
            symbol: token,
            name: "Token".into(),
            members,
            properties: typed_trees::data::DataProperties {
                multiplicity: language_semantics::Multiplicity::Linear,
                carry: None,
            },
            ..Default::default()
        });
        let semantic_id = typed.semantic_domains.intern("Token::Allowed");
        typed.push_domain_definition(typed_trees::domain::DomainDefinition {
            symbol: domain,
            name: "Allowed".into(),
            target_type: token_type,
            semantic_id,
            ..Default::default()
        });
        let semantic_id = typed
            .retain_domain_application(domain, token_type, &[])
            .unwrap();
        let constraints = typed.type_reference_table.insert_constraints([
            typed_trees::types::TypeConstraintNode::Domain(typed_trees::types::DomainConstraint {
                name: "Allowed".into(),
                symbol: domain,
                semantic_id,
                ..Default::default()
            }),
        ]);
        let qualified = typed
            .type_reference_table
            .insert(TypeReferenceNode::Constrained {
                base_type: token_type,
                constraints,
            });
        let helper_parameters = typed.state_parameters.insert_many([StateParameter {
            symbol: helper_parameter,
            name: "flag".into(),
            type_reference: boolean,
            ..Default::default()
        }]);
        let mut helper_members = Default::default();
        typed
            .expression_table
            .push_name_path_member(&mut helper_members, "flag".into());
        let helper_body = typed
            .expression_table
            .insert(ExpressionNode::Name(TableNamePath {
                members: helper_members,
                head_symbol: helper_parameter,
                symbol: helper_parameter,
                ..Default::default()
            }));
        let statements = typed.statement_table.insert_statements([
            typed_trees::statement::StatementNode::Expression(helper_body),
        ]);
        let states = typed
            .machine_states
            .insert_many([typed_trees::state::State {
                symbol: helper_entry,
                name: "entry".into(),
                parameters: helper_parameters,
                return_type: boolean,
                statement_nodes: statements,
                ..Default::default()
            }]);
        typed.push_machine(typed_trees::machine::Machine {
            symbol: helper,
            name: "clause_helper".into(),
            states,
            body_is_present: true,
            body_status: typed_trees::machine::MachineBodyStatus::Complete,
            ..Default::default()
        });
        let formals = typed
            .state_parameters
            .insert_many(
                parameters
                    .handles()
                    .enumerate()
                    .map(|(i, symbol)| StateParameter {
                        symbol,
                        name: if i == 0 {
                            "left".into()
                        } else {
                            "right".into()
                        },
                        type_reference: if linear && i == 0 { qualified } else { boolean },
                        ..Default::default()
                    }),
            );
        let mut left_members = Default::default();
        typed
            .expression_table
            .push_name_path_member(&mut left_members, "left".into());
        let mut right_members = Default::default();
        typed
            .expression_table
            .push_name_path_member(&mut right_members, "right".into());
        let left = typed
            .expression_table
            .insert(ExpressionNode::Name(TableNamePath {
                members: left_members,
                head_symbol: parameters.handle_at(0),
                symbol: parameters.handle_at(0),
                ..Default::default()
            }));
        let left = if linear {
            let field = typed.expression_table.insert(ExpressionNode::Member(
                typed_trees::expression::TableMemberExpression {
                    receiver: left,
                    selection: typed_trees::expression::MemberSelection::Field(field),
                    member: "allowed".into(),
                    case_variant: None,
                },
            ));
            let arguments = typed.expression_table.insert_expression_handles([field]);
            typed.expression_table.insert(ExpressionNode::Call(
                typed_trees::expression::TableCallExpression {
                    target_symbol: helper,
                    target: "clause_helper".into(),
                    arguments,
                    ..Default::default()
                },
            ))
        } else {
            left
        };
        let right = typed
            .expression_table
            .insert(ExpressionNode::Name(TableNamePath {
                members: right_members,
                head_symbol: parameters.handle_at(1),
                symbol: parameters.handle_at(1),
                ..Default::default()
            }));
        let negation = typed
            .expression_table
            .insert(ExpressionNode::Unary(TableUnaryExpression {
                operator: UnaryOperator::LogicalNot,
                operand: right,
            }));
        let predicate =
            typed
                .expression_table
                .insert(ExpressionNode::Binary(TableBinaryExpression {
                    left,
                    operator: BinaryOperator::And,
                    right: negation,
                    policy: numerics::arithmetic::ArithmeticDomain::Exact,
                }));
        for expression in [predicate, negation] {
            let occurrence = typed
                .record_late_bound_authored_declaration_selection(
                    Default::default(),
                    Exposure::PublicInterface,
                    Kind::Operator,
                    Binding::CheckedOperator,
                )
                .unwrap();
            typed
                .expression_table
                .attach_authored_selection_occurrences(expression, [occurrence]);
        }
        let predicate = if valid_predicate {
            predicate
        } else {
            typed
                .expression_table
                .insert(ExpressionNode::TypeExpression(unit))
        };
        let facts = typed
            .proof_facts
            .insert_many([ProofFact::Expression(predicate)]);
        let contracts = typed.signature_contracts.insert_many([SignatureContract {
            kind: SignatureContractKind::Requires,
            facts,
            binding: None,
            ..Default::default()
        }]);
        let no_services = typed.service_reach_rows.intern(Vec::new());
        let machines = typed.trait_machine_signatures.insert_many([StateSignature {
            symbol: requirement,
            name: "read".into(),
            parameters: formals,
            return_type: unit,
            contracts,
            service_reach_row: no_services,
            declares_no_crash_route: true,
            ..Default::default()
        }]);
        for (symbol, name, machines) in [
            (owner, "Raw", machines),
            (other, "Other", Default::default()),
        ] {
            typed.push_trait_definition(TraitDefinition {
                symbol,
                name: name.into(),
                is_boundary: true,
                machines,
                ..Default::default()
            });
        }
        (typed, owner, other, requirement)
    }

    #[test]
    fn queued_calling_header_describes_linear_qualified_inputs_and_checks_its_clauses() {
        let (typed, owner, _, requirement) = header_fixture(true, true);
        let scope = AppliedTraitScope::new(
            &typed,
            typed.trait_definition_by_symbol(owner).unwrap(),
            TraitApplicationArguments {
                lexical_owner: owner,
                enclosing_data_applications: &[],
                arguments: &[],
                lifetimes: &[],
            },
        )
        .unwrap();
        let context = MachineId::new(1).unwrap();
        let mut program = Program::new(&typed, false);
        let mut policies = SourceCallingPolicies::default();
        program
            .prepare_calling_requirement_header(context, &scope, requirement, &mut policies)
            .unwrap();
        assert_eq!(policies.headers.len(), 1);
        assert!(program.admitted_signature_views.is_empty());
        let (_, source) = typed.trait_machine_signature(requirement).unwrap();
        let reference = typed.state_parameters.span(source.parameters).unwrap()[0].type_reference;
        let crate::body::ValueType::Record(record, _, _) =
            program.value_type(context, reference).unwrap()
        else {
            panic!("the qualified input must retain its linear record type");
        };
        assert!(program.symbols.is_empty());
        program.begin_declaration_storage(context, 0).unwrap();
        program.prepare_calling_storage(context, &policies).unwrap();
        assert_eq!(program.admitted_signature_views.len(), 1);
        // Storage description preserves the same declared multiplicity;
        // reading a calling header neither creates a value nor retires a claim.
        assert_eq!(
            program.records.get(record).unwrap().multiplicity,
            terminal_psi::StructuralMultiplicity::Linear,
        );
        let signature = program
            .read_calling_requirement_header(context, &scope, requirement)
            .unwrap();
        assert!(signature.parameters[0].qualification() != crate::body::Qualification::NONE);
        assert!(!signature.requires.is_empty());
        program.completed_operators.finish();
        crate::selection_questions::capture_with_bodies(
            &typed,
            &program.admitted_declarations,
            &program.admitted_bodies,
            &program.completed_operators,
            &program.admitted_signature_views,
        )
        .expect("the field/helper predicate retains exact And and Not completions");
        assert!(program.symbols.is_empty());
        assert!(program.signatures.is_empty());
        assert!(program.boundaries.rows.is_empty());
        assert!(program.fixed_inputs.is_empty());
        let helper = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "clause_helper")
            .unwrap();
        assert!(program.admitted_bodies.contains(helper.symbol));
        assert!(
            program
                .machine(helper.symbol)
                .unwrap_err()
                .detail
                .contains("machine admission after storage metadata")
        );
    }

    #[test]
    fn queued_linear_calling_header_refuses_an_unformed_clause_at_its_source() {
        let (typed, owner, _, requirement) = header_fixture(false, true);
        let scope = AppliedTraitScope::new(
            &typed,
            typed.trait_definition_by_symbol(owner).unwrap(),
            TraitApplicationArguments {
                lexical_owner: owner,
                enclosing_data_applications: &[],
                arguments: &[],
                lifetimes: &[],
            },
        )
        .unwrap();
        let context = MachineId::new(1).unwrap();
        let mut program = Program::new(&typed, false);
        let mut policies = SourceCallingPolicies::default();
        program
            .prepare_calling_requirement_header(context, &scope, requirement, &mut policies)
            .unwrap();
        program.begin_declaration_storage(context, 0).unwrap();
        let Err(refused) = program.prepare_calling_storage(context, &policies) else {
            panic!("a queued linear header must not skip its unformed clause");
        };
        assert_eq!(
            refused.reason,
            terminal_check_tables::RefusalReason::Unsupported
        );
        assert!(
            refused
                .detail
                .contains("clause that is not a comparison or conjunction")
        );
        assert!(refused.detail.contains("the Calling header"));
        assert!(refused.detail.contains("Raw::read"));
        assert!(program.admitted_signature_views.is_empty());
        assert!(program.symbols.is_empty());
    }

    #[test]
    fn calling_header_admission_reads_the_complete_bodyless_precondition() {
        let (typed, owner, _, requirement) = fixture(true);
        let declaration = typed.trait_definition_by_symbol(owner).unwrap();
        let scope = AppliedTraitScope::new(
            &typed,
            declaration,
            TraitApplicationArguments {
                lexical_owner: owner,
                enclosing_data_applications: &[],
                arguments: &[],
                lifetimes: &[],
            },
        )
        .unwrap();
        let context = MachineId::new(1).unwrap();
        let mut program = Program::new(&typed, false);
        program.admitted_declarations.include(owner);
        program.completed_operators.finish();
        let unresolved = crate::selection_questions::capture(
            &typed,
            &program.admitted_declarations,
            &program.completed_operators,
            &program.admitted_signature_views,
        )
        .unwrap_err();
        assert!(unresolved.to_string().contains("CheckedOperator"));
        let signature = program
            .read_calling_requirement_header(context, &scope, requirement)
            .unwrap();
        assert!(!signature.requires.is_empty());
        assert_eq!(program.admitted_signature_views.len(), 1);
        assert_eq!(program.admitted_signature_views[0].symbol, requirement);
        program.completed_operators.finish();
        crate::selection_questions::capture(
            &typed,
            &program.admitted_declarations,
            &program.completed_operators,
            &program.admitted_signature_views,
        )
        .expect("the exact And and negation have actual checked reader completions");
        assert!(program.signatures.is_empty());
        assert!(program.boundaries.rows.is_empty());
        assert!(program.fixed_inputs.is_empty());
    }

    #[test]
    fn calling_header_admission_refuses_another_declaring_scope() {
        let (typed, _, other, requirement) = fixture(true);
        let declaration = typed.trait_definition_by_symbol(other).unwrap();
        let scope = AppliedTraitScope::new(
            &typed,
            declaration,
            TraitApplicationArguments {
                lexical_owner: other,
                enclosing_data_applications: &[],
                arguments: &[],
                lifetimes: &[],
            },
        )
        .unwrap();
        let mut program = Program::new(&typed, false);
        let refusal = program
            .read_calling_requirement_header(MachineId::new(1).unwrap(), &scope, requirement)
            .unwrap_err();
        assert!(refusal.detail.contains("exact declaring boundary header"));
        assert!(program.admitted_signature_views.is_empty());
    }

    #[test]
    fn calling_header_admission_does_not_skip_an_unformed_contract() {
        let (typed, owner, _, requirement) = fixture(false);
        let declaration = typed.trait_definition_by_symbol(owner).unwrap();
        let scope = AppliedTraitScope::new(
            &typed,
            declaration,
            TraitApplicationArguments {
                lexical_owner: owner,
                enclosing_data_applications: &[],
                arguments: &[],
                lifetimes: &[],
            },
        )
        .unwrap();
        let mut program = Program::new(&typed, false);
        let refusal = program
            .read_calling_requirement_header(MachineId::new(1).unwrap(), &scope, requirement)
            .expect_err(
                "ABI metadata cannot admit a contract that the callable reader cannot form",
            );
        assert_eq!(
            refusal.reason,
            terminal_check_tables::RefusalReason::Unsupported
        );
        assert!(program.admitted_signature_views.is_empty());
        assert!(program.boundaries.rows.is_empty());
    }
}
