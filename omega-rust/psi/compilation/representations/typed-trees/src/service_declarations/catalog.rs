//! Freeze the source-owned service census before any target chooses supply.
//!
//! The final immutable owner supplies live application roots, abstract boundary
//! declarations and retained captures. Arena presence alone is not liveness.
//! Unsupported projections and catalog-only correspondence failures remain data;
//! preparing the census does not reject a dormant declaration or grant execution.

use super::projection::{self, ServiceRequirementSource};
use super::{ProjectedServiceSurface, ProviderSchemaDeclaration};
use crate::PlacedScalarAccessorDeclaration;
use crate::provider_declarations::ProviderPlanningContext;
use crate::trait_definition::AppliedTraitScope;
use crate::type_identity::TypeIdentityId;
use crate::types::TypeReferenceHandle;
use arena::{Arena, Handle, HandleSpan};
use diagnostics::Diagnostic;
use language_semantics::service_surface::ServiceSurface;
use semantic_vocabulary::PackageKeyIdentity;
use std::sync::Arc;
use symbols::SymbolHandle;

mod calling_owners;

/// Exact source correspondence and its deferred calling-owner obligation.
/// Ordinary inherited traits may occur in a service surface without becoming
/// boundary calling owners. Merely preparing that surface must not reject it.
pub struct PreparedServiceRequirement {
    pub source: ServiceRequirementSource,
    calling_eligibility: Result<(), Diagnostic>,
}

impl Default for PreparedServiceRequirement {
    fn default() -> Self {
        Self {
            source: ServiceRequirementSource::default(),
            calling_eligibility: Ok(()),
        }
    }
}

impl PreparedServiceRequirement {
    pub fn require_calling_owner(&self) -> Result<(), Diagnostic> {
        self.calling_eligibility.clone()
    }
}

/// Lookup coordinates are issued by source preparation, never reconstructed
/// from a rendered signature or a runtime-erased carrier.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ServiceOrigin {
    Declaration(ProviderSchemaDeclaration),
    Application {
        declaration: SymbolHandle,
        argument_owner: SymbolHandle,
        application: TypeIdentityId,
    },
    Named {
        declaration: SymbolHandle,
        capture: Handle<crate::typed_trees::CapturedMachineApplication>,
        receiver: SymbolHandle,
    },
}

struct ServiceRecord {
    origin: ServiceOrigin,
    schema: ProviderSchemaDeclaration,
    argument_owner: SymbolHandle,
    package: Option<PackageKeyIdentity>,
    path: String,
    declaration: Option<Arc<ServiceSurface>>,
    requirements: HandleSpan<PreparedServiceRequirement>,
    arguments: HandleSpan<TypeReferenceHandle>,
    correspondence: Result<(), Diagnostic>,
}

impl Default for ServiceRecord {
    fn default() -> Self {
        // Arena zero is an absent record, not a source declaration.
        let schema = ProviderSchemaDeclaration::BoundaryTrait(SymbolHandle::invalid());
        Self {
            origin: ServiceOrigin::Declaration(schema),
            schema,
            argument_owner: SymbolHandle::invalid(),
            package: None,
            path: String::new(),
            declaration: None,
            requirements: HandleSpan::default(),
            arguments: HandleSpan::default(),
            correspondence: Ok(()),
        }
    }
}

struct SealedDeclaration {
    symbol: SymbolHandle,
    declaration: Result<Option<PlacedScalarAccessorDeclaration>, Diagnostic>,
}

impl Default for SealedDeclaration {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            declaration: Ok(None),
        }
    }
}

/// Invocation-local facts tied to one immutable source snapshot and purpose.
/// Product's existing inventory retains its checked Arc; early Build retains
/// its provisional projection until its consumers finish. No lookup runs source
/// projection, and equal-looking Build/Product copies cannot share coordinates.
pub struct PreparedServiceCatalog {
    owner_identity: u64,
    owner_address: usize,
    scope: source::DependencyScope,
    records: Arena<ServiceRecord>,
    requirements: Arena<PreparedServiceRequirement>,
    arguments: Arena<TypeReferenceHandle>,
    sealed: Arena<SealedDeclaration>,
    signatures: super::signatures::PreparedBoundarySignatures,
}

/// Authenticated query entrance for one unchanged source owner and purpose.
/// Construction checks the pairing once; lookup never revisits source syntax.
#[derive(Clone, Copy)]
pub struct ServiceCatalog<'catalog> {
    context: ProviderPlanningContext<'catalog>,
    catalog: &'catalog PreparedServiceCatalog,
}

/// The complete declaration census includes unsupported dormant declarations.
/// Dropping these rows would change ambiguity and deferred-error behavior.
pub struct PreparedServiceDeclaration<'catalog> {
    pub schema: ProviderSchemaDeclaration,
    pub package: Option<PackageKeyIdentity>,
    pub path: &'catalog str,
    pub surface: Option<PreparedServiceSurface<'catalog>>,
}

/// A borrowed catalog row is a declaration, not selection or admission evidence.
pub struct PreparedServiceSurface<'catalog> {
    pub schema: ProviderSchemaDeclaration,
    pub argument_owner: SymbolHandle,
    pub package: Option<PackageKeyIdentity>,
    pub path: &'catalog str,
    pub declaration: &'catalog Arc<ServiceSurface>,
    pub requirements: &'catalog [PreparedServiceRequirement],
    pub boundary_arguments: &'catalog [TypeReferenceHandle],
    correspondence: &'catalog Result<(), Diagnostic>,
}

impl PreparedServiceSurface<'_> {
    pub fn exact_requirements(&self) -> Result<Vec<SymbolHandle>, Diagnostic> {
        self.correspondence.as_ref().map_err(Clone::clone)?;
        Ok(self
            .requirements
            .iter()
            .map(|requirement| requirement.source.symbol)
            .collect())
    }
}

impl PreparedServiceCatalog {
    /// Complete source formation once, preserving source order and deferred
    /// absence/refusals. The caller must finish mutable preparation first.
    pub fn prepare(context: ProviderPlanningContext<'_>) -> Self {
        let program = context.typed;
        let mut catalog = Self {
            owner_identity: program.identity.get(),
            owner_address: std::ptr::from_ref(program) as usize,
            scope: context.scope,
            records: Arena::new(),
            requirements: Arena::new(),
            arguments: Arena::new(),
            sealed: Arena::new(),
            signatures: super::signatures::PreparedBoundarySignatures::prepare(context),
        };

        // Open generic declarations are needed even without executable uses.
        for declaration in program
            .traits()
            .iter()
            .filter(|declaration| declaration.is_boundary && context.contains(declaration.symbol))
        {
            let schema = ProviderSchemaDeclaration::BoundaryTrait(declaration.symbol);
            catalog.retain(
                context,
                ServiceOrigin::Declaration(schema),
                schema,
                projection::from_typed(context, declaration),
                &[],
            );
        }

        // This is the same live census used by logical and executable source
        // requirement preparation, including occurrence-owned enclosing frames.
        for root in program.applied_trait_roots() {
            root.visit(program, &mut |scope| {
                if !scope.declaration.is_boundary || !context.contains(scope.declaration.symbol) {
                    return;
                }
                let schema = ProviderSchemaDeclaration::BoundaryTrait(scope.declaration.symbol);
                let origin = ServiceOrigin::Application {
                    declaration: scope.declaration.symbol,
                    argument_owner: scope.argument_owner,
                    application: scope.application_identity(program).unwrap_or_default(),
                };
                if catalog.has_origin(origin) {
                    return;
                }
                catalog.retain(
                    context,
                    origin,
                    schema,
                    projection::from_typed_application(context, scope.clone()),
                    &[],
                );
            });
        }

        for declaration in program
            .conformances()
            .iter()
            .filter(|declaration| context.contains(declaration.symbol))
        {
            catalog.retain_named(
                context,
                declaration.symbol,
                Handle::default(),
                AppliedTraitScope::from_conformance(program, declaration),
                program.type_reference_handles(declaration.arguments),
            );
        }
        for (capture, application) in program.captured_machine_applications.iter() {
            let Some(declaration) = program
                .conformance_by_symbol(application.conformance_application.declaration)
                .filter(|declaration| context.contains(declaration.symbol))
            else {
                continue;
            };
            catalog.retain_named(
                context,
                declaration.symbol,
                capture,
                application
                    .conformance_application
                    .trait_scope
                    .scope(program),
                program.type_reference_handles(declaration.arguments),
            );
        }

        for operator in program
            .operators()
            .iter()
            .filter(|operator| operator.is_boundary && context.contains(operator.symbol))
        {
            let schema = ProviderSchemaDeclaration::BoundaryOperator(operator.symbol);
            catalog.retain(
                context,
                ServiceOrigin::Declaration(schema),
                schema,
                projection::from_typed_operator(context, operator),
                &[],
            );
        }
        for machine in program.machines().iter().filter(|machine| {
            context.contains(machine.symbol)
                && machine.supply_mode == language_semantics::MachineSupplyMode::TopLevelRequirement
                && !machine.body_is_present
        }) {
            let schema = ProviderSchemaDeclaration::BoundaryRequirement(machine.symbol);
            catalog.retain(
                context,
                ServiceOrigin::Declaration(schema),
                schema,
                projection::from_typed_boundary_requirement(context, machine),
                &[],
            );
        }
        catalog.retain_sealed(context);
        catalog
    }

    /// Pairing checks occur at source-consuming entrances, not as a grant.
    /// The identity closes address reuse; the address distinguishes clones that
    /// retain the source memo tag. Neither coordinate is portable or serialized.
    pub fn bind<'catalog>(
        &'catalog self,
        context: ProviderPlanningContext<'catalog>,
    ) -> Result<ServiceCatalog<'catalog>, Diagnostic> {
        if self.owner_identity != context.typed.identity.get()
            || self.owner_address != std::ptr::from_ref(context.typed) as usize
            || self.scope != context.scope
        {
            return Err(Diagnostic::error(
                "service catalog belongs to another source snapshot or dependency purpose",
            ));
        }
        Ok(ServiceCatalog {
            context,
            catalog: self,
        })
    }

    fn lookup(&self, origin: ServiceOrigin) -> Option<PreparedServiceSurface<'_>> {
        let mut matching = self
            .records
            .iter()
            .filter(|(_, record)| record.origin == origin);
        let (_, record) = matching.next()?;
        if matching.next().is_some() {
            return None;
        }
        self.view(record)
    }
}

impl<'catalog> ServiceCatalog<'catalog> {
    pub fn signatures(self) -> &'catalog super::signatures::PreparedBoundarySignatures {
        &self.catalog.signatures
    }

    pub fn context(self) -> ProviderPlanningContext<'catalog> {
        self.context
    }

    pub fn declaration(
        self,
        schema: ProviderSchemaDeclaration,
    ) -> Option<PreparedServiceSurface<'catalog>> {
        self.catalog.lookup(ServiceOrigin::Declaration(schema))
    }

    pub fn named_application(
        self,
        declaration: SymbolHandle,
        capture: Handle<crate::typed_trees::CapturedMachineApplication>,
        receiver: SymbolHandle,
    ) -> Option<PreparedServiceSurface<'catalog>> {
        self.catalog.lookup(ServiceOrigin::Named {
            declaration,
            capture,
            receiver,
        })
    }

    pub fn declared_traits(self) -> impl Iterator<Item = PreparedServiceDeclaration<'catalog>> {
        self.catalog
            .records
            .iter()
            .filter(|(_, record)| {
                matches!(
                    record.origin,
                    ServiceOrigin::Declaration(ProviderSchemaDeclaration::BoundaryTrait(_))
                )
            })
            .map(move |(_, record)| PreparedServiceDeclaration {
                schema: record.schema,
                package: record.package,
                path: &record.path,
                surface: self.catalog.view(record),
            })
    }

    pub fn sealed_accessor(
        self,
        symbol: SymbolHandle,
    ) -> Result<Option<&'catalog PlacedScalarAccessorDeclaration>, Diagnostic> {
        let Some((_, record)) = self
            .catalog
            .sealed
            .iter()
            .find(|(_, record)| record.symbol == symbol)
        else {
            return Ok(None);
        };
        record
            .declaration
            .as_ref()
            .map(Option::as_ref)
            .map_err(Clone::clone)
    }
}

impl PreparedServiceCatalog {
    fn view<'catalog>(
        &'catalog self,
        record: &'catalog ServiceRecord,
    ) -> Option<PreparedServiceSurface<'catalog>> {
        Some(PreparedServiceSurface {
            schema: record.schema,
            argument_owner: record.argument_owner,
            package: record.package,
            path: &record.path,
            declaration: record.declaration.as_ref()?,
            requirements: self.requirements.span_or_empty(record.requirements),
            boundary_arguments: self.arguments.span_or_empty(record.arguments),
            correspondence: &record.correspondence,
        })
    }

    fn has_origin(&self, origin: ServiceOrigin) -> bool {
        if matches!(origin, ServiceOrigin::Application { application, .. } if !application.is_valid())
        {
            return false;
        }
        self.records
            .iter()
            .any(|(_, record)| record.origin == origin)
    }

    fn retain(
        &mut self,
        context: ProviderPlanningContext<'_>,
        origin: ServiceOrigin,
        schema: ProviderSchemaDeclaration,
        projected: Option<ProjectedServiceSurface<'_>>,
        arguments: &[TypeReferenceHandle],
    ) {
        let mut record = ServiceRecord {
            origin,
            schema,
            package: context
                .typed
                .symbols
                .symbol_package_identity(schema.symbol()),
            path: context.typed.symbols.display_path(schema.symbol(), "::"),
            arguments: self.arguments.insert_many(arguments.iter().copied()),
            ..ServiceRecord::default()
        };
        if let Some(projected) = projected {
            record.correspondence = projected.validate_correspondence();
            record.argument_owner = projected.argument_owner;
            record.requirements =
                self.requirements
                    .insert_many(projected.requirements.into_iter().enumerate().map(
                        |(ordinal, source)| {
                            let calling_eligibility = if matches!(
                                projected.schema,
                                ProviderSchemaDeclaration::BoundaryTrait(_)
                            ) {
                                projected
                                    .declaration
                                    .methods
                                    .get(ordinal)
                                    .ok_or_else(|| {
                                        Diagnostic::error(
                                            "service calling row lost its ordered correspondence",
                                        )
                                    })
                                    .and_then(|method| {
                                        calling_owners::prepare(
                                            context,
                                            &projected.declaration,
                                            method,
                                            source,
                                        )
                                    })
                            } else {
                                Ok(())
                            };
                            PreparedServiceRequirement {
                                source,
                                calling_eligibility,
                            }
                        },
                    ));
            record.declaration = Some(projected.declaration);
        }
        self.records.insert(record);
    }

    fn retain_named<'program>(
        &mut self,
        context: ProviderPlanningContext<'program>,
        declaration: SymbolHandle,
        capture: Handle<crate::typed_trees::CapturedMachineApplication>,
        root: Option<AppliedTraitScope<'program>>,
        arguments: &[TypeReferenceHandle],
    ) {
        let Some(root) = root else {
            return;
        };
        let mut receivers = Vec::new();
        root.visit(context.typed, &mut |scope| {
            if scope.declaration.is_boundary
                && context.contains(scope.declaration.symbol)
                && !receivers.contains(&scope.declaration.symbol)
            {
                receivers.push(scope.declaration.symbol);
            }
        });
        for receiver in receivers {
            let schema = ProviderSchemaDeclaration::BoundaryTrait(receiver);
            let projected = root
                .declaring_scope(context.typed, receiver)
                .and_then(|scope| projection::from_typed_application(context, scope));
            self.retain(
                context,
                ServiceOrigin::Named {
                    declaration,
                    capture,
                    receiver,
                },
                schema,
                projected,
                arguments,
            );
        }
    }

    fn retain_sealed(&mut self, context: ProviderPlanningContext<'_>) {
        let program = context.typed;
        let mut symbols = Vec::new();
        for view in &program.placed_view_plans {
            for field in &view.fields {
                for target in &field.accessor_targets {
                    for symbol in [target.machine_symbol, target.state_symbol] {
                        if !symbols.contains(&symbol) {
                            symbols.push(symbol);
                        }
                    }
                }
            }
        }
        for symbol in symbols {
            let declaration =
                PlacedScalarAccessorDeclaration::authenticate(program, symbol).map(|declaration| {
                    declaration.filter(|declaration| context.contains(declaration.machine()))
                });
            if let Ok(Some(declaration)) = &declaration {
                let schema = ProviderSchemaDeclaration::SealedPlacedAccessor(declaration.machine());
                if !self.has_origin(ServiceOrigin::Declaration(schema)) {
                    let projected = program.machine(declaration.machine()).and_then(|machine| {
                        projection::project_boundary_machine_schema(context, machine, schema)
                    });
                    self.retain(
                        context,
                        ServiceOrigin::Declaration(schema),
                        schema,
                        projected,
                        &[],
                    );
                }
            }
            self.sealed.insert(SealedDeclaration {
                symbol,
                declaration,
            });
        }
    }
}
