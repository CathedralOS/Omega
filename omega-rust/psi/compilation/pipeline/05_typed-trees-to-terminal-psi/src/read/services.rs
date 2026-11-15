//! A boundary invocation keeps the actual applied service and its parents.
//! Build this graph from the shared trait-application owner; a missing parent
//! is unavailable, and a provider choice cannot replace its declaration.

use semantic_vocabulary::{MachineId, ServiceId};
use terminal_check_tables::{Checked, Construct};
use terminal_psi::{ServiceDeclaration, TerminalModule};
use typed_trees::trait_definition::{AppliedTraitScope, TraitApplicationArguments};

use super::Program;
use crate::{Unserved, refuse};

impl<'t> Program<'t> {
    pub(super) fn service(
        &mut self,
        caller: MachineId,
        scope: AppliedTraitScope<'t>,
    ) -> Checked<ServiceId> {
        let unavailable =
            |reason| refuse(caller, Construct::Machine, Unserved::Declaration(reason));
        if let Some((service, _)) = self
            .services
            .iter()
            .find(|(_, (known, _))| known.same_application(self.typed, &scope))
        {
            return Ok(service);
        }
        if self.resolving_services.contains(&scope.declaration.symbol) {
            return Err(unavailable("cyclic applied service parent declarations"));
        }
        if scope.portable_application_identity(self.typed).is_none() {
            return Err(unavailable(
                "service has no complete exact application identity",
            ));
        }
        self.resolving_services.push(scope.declaration.symbol);
        let parents = (|| {
            scope
                .parent_scopes(self.typed)
                .map_err(unavailable)?
                .into_iter()
                .map(|parent| self.service(caller, parent))
                .collect::<Checked<Vec<_>>>()
        })();
        self.resolving_services.pop();
        let mut parents = parents?;
        parents.sort_unstable();
        parents.dedup();
        Ok(self.services.push((scope, parents)))
    }

    pub(super) fn declared_service(
        &mut self,
        caller: MachineId,
        owner: &'t typed_trees::trait_definition::TraitDefinition,
    ) -> Checked<ServiceId> {
        let scope = AppliedTraitScope::new(
            self.typed,
            owner,
            TraitApplicationArguments {
                lexical_owner: owner.symbol,
                enclosing_data_applications: &[],
                arguments: &[],
                lifetimes: &[],
            },
        )
        .ok_or_else(|| {
            refuse(
                caller,
                Construct::Machine,
                Unserved::Declaration("boundary service needs its complete applied declaration"),
            )
        })?;
        self.service(caller, scope)
    }

    /// What `owner`'s header says about service reach: its `reaches` clause
    /// as the services it names and their boundary parents, or `Inferred`
    /// when it wrote none. One reader for a checked body, a requirement and
    /// a boundary requirement.
    ///
    /// effects.md, "Declarations": "`reaches A + B` bounds service reach by
    /// the named boundary services and their boundary parents"; "Boundary
    /// traits supply identities that may be named in `reaches`"; "Ordinary
    /// traits are not service identities." The typed trees keep one authored
    /// row per written clause, with no target for `reaches none`, so a
    /// clause is told from an omission by the row and never by an empty
    /// set. `reaches <= Bound` contributes its written upper bound while
    /// checking source; the selected installation still owes its exact row.
    pub(super) fn reach_ceiling(
        &mut self,
        caller: MachineId,
        owner: symbols::SymbolHandle,
        requirement: bool,
    ) -> Checked<crate::Ceiling> {
        let typed = self.typed;
        let mut written = false;
        let mut named = Vec::new();
        for row in typed.authored_service_reach_rows_for(owner) {
            if row.installation_bound && !requirement {
                continue;
            }
            written = true;
            named.extend(row.targets.iter().map(|target| target.service));
        }
        if !written {
            return Ok(crate::Ceiling::Inferred);
        }
        Ok(crate::Ceiling::Authored(self.named_row(
            caller,
            named,
            crate::body::Axis::Reach,
        )?))
    }

    /// Prepare each selected callable and its binder ceiling in the same
    /// catalog as the executable body. An unused binding needs its own
    /// checked declaration but contributes no call or service reach.
    pub(crate) fn application_binders(
        &mut self,
        owner: MachineId,
    ) -> Checked<crate::declare::BinderReach> {
        use typed_trees::static_applications::StaticMachineBindingValue;

        let typed = self.typed;
        let unserved = |what| refuse(owner, Construct::Machine, Unserved::Declaration(what));
        let symbol = *self
            .symbols
            .get(owner)
            .ok_or_else(|| unserved("a callback application lost its owner"))?;
        let Some(instantiation) = typed.machine_instantiation(symbol).map_err(unserved)? else {
            return Ok(Vec::new());
        };
        let bindings = typed
            .expression_table
            .static_machine_application_bindings(instantiation.binding_application());
        let mut bounds = Vec::new();
        for binding in bindings {
            let StaticMachineBindingValue::Machine(argument) = &binding.value else {
                continue;
            };
            let contract = typed
                .retained_static_machine_contract(binding.parameter)
                .ok_or_else(|| unserved("a static callback binding has no source requirement"))?;
            let bound = match self.reach_ceiling(owner, contract.signature().symbol, true)? {
                crate::Ceiling::Inferred => None,
                crate::Ceiling::Authored(bound) => Some(bound),
            };
            let selected = typed
                .machine_by_symbol(argument.symbol)
                .or_else(|| {
                    typed
                        .unique_machine_by_entry_state_symbol(argument.symbol)
                        .map(|(machine, _)| machine)
                })
                .ok_or_else(|| {
                    unserved("a static callback binding has no exact selected machine")
                })?;
            if typed.machine_callable_parameters(selected).is_empty() {
                self.machine(selected.symbol)?;
            }
            bounds.push((binding.parameter, bound));
        }
        Ok(bounds)
    }

    /// What a header says about synchronous entry: its `invokes` rows as
    /// the requirement identities they name. `bodyless` is a requirement,
    /// boundary or not.
    ///
    /// effects.md, "Direct synchronous invocation": on a checked body an
    /// omitted clause means "Infer and publish its entries" and a written
    /// one bounds them; on a bodyless requirement omission is "No
    /// synchronous entry", an empty written row. "A row names a parameter
    /// path or a requirement identity." A parameter path stands for the
    /// binding a call supplies, so charging a caller for it needs the
    /// actual substituted at each call; no slice reads that yet, and it is
    /// refused by name, never dropped.
    pub(super) fn entry_ceiling(
        &mut self,
        caller: MachineId,
        rows: &[typed_trees::signature::AuthoredInvocation],
        bodyless: bool,
    ) -> Checked<crate::Ceiling> {
        use typed_trees::signature::AuthoredInvocationTarget as Named;
        if rows.is_empty() && !bodyless {
            return Ok(crate::Ceiling::Inferred);
        }
        let mut named = Vec::with_capacity(rows.len());
        for row in rows {
            match row.target {
                Named::Service(requirement) => named.push(requirement),
                Named::Parameter { .. } => {
                    return Err(refuse(
                        caller,
                        Construct::Machine,
                        Unserved::Machine("invokes naming a parameter path"),
                    ));
                }
                // "A row names a parameter path or a requirement identity",
                // and typing found that nothing the row's source can see
                // has its name, so it names neither in every program. A
                // row whose name something visible has may name one where
                // typing does not follow it yet.
                Named::Undeclared => {
                    return Err(crate::reject(
                        caller,
                        crate::SourceOrigin::of(row.source_span),
                        format!(
                            "`invokes {}` names no parameter and no requirement: a name no declaration visible \
                             here has is an error wherever it appears",
                            row.name.as_str()
                        ),
                    ));
                }
                Named::Unresolved => {
                    return Err(refuse(
                        caller,
                        Construct::Machine,
                        Unserved::Typing(
                            "invokes naming a declaration typing does not select as a requirement",
                        ),
                    ));
                }
            }
        }
        Ok(crate::Ceiling::Authored(self.named_row(
            caller,
            named,
            crate::body::Axis::Entries,
        )?))
    }

    /// The row a clause on `axis` writes by naming `named`: the one reading
    /// of a written ceiling. Each name must be a boundary trait, whose
    /// applied declaration is its service identity in the module's catalog.
    /// A reach row is closed under boundary parents; an entry row is not
    /// (`invokes_entry_ceiling.md`, "No parent closure").
    ///
    /// For reach a name that is not a boundary trait is a mistake of the
    /// program ("Ordinary traits are not service identities"). For entries
    /// it may be a valid requirement identity ("Route and direction decide
    /// what a row may name, not declaration kind"), which the catalog has
    /// no identity for yet, so it is not served.
    fn named_row(
        &mut self,
        caller: MachineId,
        named: Vec<symbols::SymbolHandle>,
        axis: crate::body::Axis,
    ) -> Checked<Vec<ServiceId>> {
        let typed = self.typed;
        let mut row = Vec::with_capacity(named.len());
        for service in named {
            let declaration = typed
                .trait_definition_by_symbol(service)
                .filter(|declaration| declaration.is_boundary)
                .ok_or_else(|| {
                    refuse(
                        caller,
                        Construct::Machine,
                        match axis {
                            crate::body::Axis::Reach => Unserved::Typing(
                                "reaches names something that is not a boundary trait",
                            ),
                            crate::body::Axis::Entries => Unserved::Machine(
                                "invokes naming a requirement that is not a boundary trait",
                            ),
                        },
                    )
                })?;
            row.push(self.declared_service(caller, declaration)?);
        }
        Ok(match axis {
            crate::body::Axis::Reach => self.with_parents(row),
            crate::body::Axis::Entries => {
                row.sort_unstable();
                row.dedup();
                row
            }
        })
    }

    /// `services` and every boundary parent of one, strictly increasing.
    /// effects.md: "parents(R) subset-of normalize(R)".
    fn with_parents(&self, mut services: Vec<ServiceId>) -> Vec<ServiceId> {
        let mut next = 0;
        while let Some(service) = services.get(next).copied() {
            next += 1;
            if let Some((_, parents)) = self.services.get(service) {
                for parent in parents {
                    if !services.contains(parent) {
                        services.push(*parent);
                    }
                }
            }
        }
        services.sort_unstable();
        services.dedup();
        services
    }

    pub(crate) fn publish_boundaries(
        &self,
        caller: MachineId,
        signatures: crate::body::Signatures<'_>,
        outcomes: &semantic_vocabulary::IdTable<MachineId, crate::Outcome>,
        records: &crate::lower::places::Records<'_>,
        qualifications: &mut crate::qualifications::Qualifications,
        module: &mut TerminalModule,
    ) -> Checked<()> {
        let mut boundaries = Vec::with_capacity(self.boundary_symbols.len());
        for (boundary, symbol) in self.boundary_symbols.iter() {
            let signature = signatures.get(
                semantic_vocabulary::TargetMachineCallable::Boundary(boundary),
                caller,
            )?;
            let unavailable =
                |reason| refuse(caller, Construct::Machine, Unserved::Declaration(reason));
            let mut rows = [Vec::new(), Vec::new()];
            for axis in crate::body::Axis::ALL {
                let crate::Ceiling::Authored(ceiling) = signature.ceiling(axis) else {
                    return Err(unavailable(match axis {
                        crate::body::Axis::Reach => {
                            "boundary declaration has no declared reach ceiling"
                        }
                        crate::body::Axis::Entries => {
                            "boundary declaration has no declared entry ceiling"
                        }
                    }));
                };
                for service in ceiling {
                    rows[axis as usize].push(
                        self.services
                            .get(*service)
                            .and_then(|(scope, _)| scope.portable_application_identity(self.typed))
                            .and_then(|identity| self.typed.type_identity_text(identity))
                            .filter(|identity| !identity.is_empty())
                            .ok_or_else(|| {
                                unavailable("retained service lost its exact application identity")
                            })?,
                    );
                }
            }
            let [reach, entries] = rows
                .each_ref()
                .map(|row: &Vec<String>| row.iter().map(String::as_str).collect::<Vec<&str>>());
            let execution = if let Some(
                language_core::sealed_intrinsics::SealedIntrinsic::HostedPrimitive(operation),
            ) = self.typed.symbols.sealed_intrinsic(*symbol)
            {
                let machine = self
                    .typed
                    .machine_by_symbol(*symbol)
                    .ok_or_else(|| unavailable("hosted primitive lost its source machine"))?;
                let mut declarations = self
                    .typed
                    .machine_trait_conformances(machine)
                    .iter()
                    .filter_map(|edge| {
                        typed_trees::machine::resolve_satisfied_declaration(
                            self.typed, machine, edge,
                        )
                    });
                let requirement = declarations
                    .next()
                    .ok_or_else(|| unavailable("hosted primitive lost its requirement"))?
                    .symbol();
                if declarations.next().is_some() {
                    return Err(unavailable("hosted primitive has multiple requirements"));
                }
                let requirement = self
                    .boundary_symbols
                    .iter()
                    .find_map(|(id, symbol)| (*symbol == requirement).then_some(id))
                    .ok_or_else(|| unavailable("hosted primitive requirement was not published"))?;
                terminal_psi::CheckedBoundaryExecution::HostedPrimitive {
                    operation,
                    requirement,
                }
            } else if self.typed.symbols.sealed_intrinsic(*symbol)
                == Some(
                    language_core::sealed_intrinsics::SealedIntrinsic::FloatOperationRequirement,
                )
            {
                terminal_psi::CheckedBoundaryExecution::NumericPrimitive
            } else {
                terminal_psi::CheckedBoundaryExecution::Provider
            };
            let metadata = crate::declare::boundary(
                self.typed,
                *symbol,
                execution,
                signature,
                crate::declare::Published {
                    operational: signature.operational.published([false; 2]),
                    reach: &reach,
                    entries: &entries,
                },
                &mut qualifications.identities,
            )
            .map_err(|diagnostic| terminal_check_tables::Refusal {
                reason: terminal_check_tables::RefusalReason::Unsupported,
                machine: caller,
                concerns: Construct::Machine,
                source_span: diagnostic.source_span,
                detail: format!("declaration {}", diagnostic.message),
            })?;
            let service = *terminal_check_tables::row(
                &self.boundary_services,
                boundary,
                caller,
                Construct::Machine,
            )?;
            let kind = if execution == terminal_psi::CheckedBoundaryExecution::NumericPrimitive {
                let requirement = typed_trees::declarations::intrinsic_requirements::IntrinsicRequirement::by_symbol(self.typed, *symbol)
                    .ok_or_else(|| unavailable("numeric primitive lost its declaration kind"))?;
                match requirement.kind {
                    typed_trees::declarations::intrinsic_requirements::IntrinsicRequirementKind::Operator => semantic_vocabulary::BoundaryRequirementKind::Operator,
                    typed_trees::declarations::intrinsic_requirements::IntrinsicRequirementKind::TopLevelRequirement => semantic_vocabulary::BoundaryRequirementKind::Machine,
                }
            } else if execution.compiler_owned() {
                semantic_vocabulary::BoundaryRequirementKind::Machine
            } else {
                semantic_vocabulary::BoundaryRequirementKind::Trait
            };
            let mut declaration = crate::lower::boundary(
                caller,
                boundary,
                signatures,
                outcomes,
                records,
                metadata,
                service,
                if execution == terminal_psi::CheckedBoundaryExecution::NumericPrimitive {
                    self.typed.numeric_intrinsic_execution(*symbol)
                } else {
                    None
                },
                kind,
                qualifications,
            )?;
            declaration.service_reach_is_installation_bound =
                self.typed.trait_machine_signature(*symbol).is_some_and(
                    |(owner, signature)| {
                        owner.is_boundary && signature.service_reach_is_installation_bound
                    },
                );
            if declaration.service_reach_is_installation_bound {
                declaration.fixed_service_reach.clear();
            }
            declaration.result_lifetimes = self
                .declared_result_lifetimes(caller, signature, &module.structural_types)
                .map_err(unavailable)?;
            if declaration.result_lifetimes.is_none() {
                return Err(unavailable(
                    "boundary result has an unresolved declared lifetime frontier",
                ));
            }
            declaration.reference_outputs = self
                .declared_reference_outputs(
                    caller,
                    signature,
                    &module.structural_types,
                    &declaration.structural_parameters,
                )
                .map_err(unavailable)?;
            boundaries.push(declaration);
        }
        module.boundary_machines = boundaries;
        Ok(())
    }

    pub(crate) fn publish_services(
        &self,
        caller: MachineId,
        module: &mut TerminalModule,
    ) -> Checked<()> {
        let mut identities = std::mem::take(&mut module.identities).into_builder();
        let mut services = Vec::with_capacity(self.services.len());
        for (id, (scope, parents)) in self.services.iter() {
            let identity = scope
                .portable_application_identity(self.typed)
                .and_then(|identity| self.typed.type_identity_text(identity))
                .filter(|identity| !identity.is_empty())
                .ok_or_else(|| {
                    refuse(
                        caller,
                        Construct::Machine,
                        Unserved::Declaration(
                            "retained service lost its exact application identity",
                        ),
                    )
                })?;
            services.push(ServiceDeclaration {
                id,
                identity: identities.intern(&identity),
                parents: parents.clone(),
            });
        }
        module.identities = identities.build();
        module.services = services;
        Ok(())
    }

    /// Notes that a Build selection awaits a provider for `service`.
    pub(super) fn awaits(&mut self, service: ServiceId) {
        if !self.readiness.contains(&service) {
            self.readiness.push(service);
        }
    }

    /// The trait that declares each awaited requirement, by its exact
    /// owner's identity, interned in `module`. The provider the Build host
    /// later installs is matched on this identity and on the application
    /// identity of the service row, so both are the ones a provider's own
    /// module states for the same declarations.
    pub(crate) fn publish_readiness(
        &self,
        caller: MachineId,
        module: &mut TerminalModule,
    ) -> Checked<Vec<(ServiceId, terminal_psi::NominalDeclarationIdentity)>> {
        if self.readiness.is_empty() {
            return Ok(Vec::new());
        }
        let unavailable =
            |reason| refuse(caller, Construct::Machine, Unserved::Declaration(reason));
        let sources =
            source::source_identity::toolchain_source_identities(self.typed.symbols.source_files())
                .map_err(|_| {
                    unavailable(
                        "an awaited requirement has a toolchain source with no exact identity",
                    )
                })?;
        let mut identities = std::mem::take(&mut module.identities).into_builder();
        let rows = self
            .readiness
            .iter()
            .map(|service| {
                let (scope, _) = self
                    .services
                    .get(*service)
                    .ok_or("an awaited requirement lost its service row")?;
                crate::declare::nominal(
                    self.typed,
                    &sources,
                    scope.declaration.symbol,
                    &mut identities,
                )
                .map(|identity| {
                    (
                        *service,
                        terminal_psi::NominalDeclarationIdentity::new(identity),
                    )
                })
            })
            .collect::<Result<Vec<_>, &'static str>>();
        module.identities = identities.build();
        rows.map_err(unavailable)
    }
}
