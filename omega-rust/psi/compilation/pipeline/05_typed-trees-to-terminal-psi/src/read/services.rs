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
    /// set. A ceiling bound by an installation (`reaches <= Bound`) is an
    /// abstract row, which no slice reads yet.
    pub(super) fn reach_ceiling(
        &mut self,
        caller: MachineId,
        owner: symbols::SymbolHandle,
    ) -> Checked<crate::Ceiling> {
        let typed = self.typed;
        let unserved = |what| refuse(caller, Construct::Machine, Unserved::Machine(what));
        let mut written = false;
        let mut named = Vec::new();
        for row in typed.authored_service_reach_rows_for(owner) {
            if row.installation_bound {
                return Err(unserved("service reach bound by an installation"));
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
                // so a bare name nothing in the program has names neither,
                // in every program. A path, or a name something has, may
                // name one where typing does not follow it yet.
                Named::Unresolved
                    if !row.name.as_str().contains(['.', ':'])
                        && !self
                            .typed
                            .names_nothing_has(vec![row.name.as_str()])
                            .is_empty() =>
                {
                    return Err(crate::reject(
                        caller,
                        crate::SourceOrigin::of(row.source_span),
                        format!(
                            "`invokes {}` names no parameter and no requirement: a name nothing \
                             in the program declares is an error wherever it appears",
                            row.name.as_str()
                        ),
                    ));
                }
                Named::Unresolved => {
                    return Err(refuse(
                        caller,
                        Construct::Machine,
                        Unserved::Typing("invokes names nothing this program declares"),
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
        module: &mut TerminalModule,
    ) -> Checked<()> {
        let mut identities = std::mem::take(&mut module.identities).into_builder();
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
            let metadata = crate::declare::boundary(
                self.typed,
                *symbol,
                signature,
                crate::declare::Published {
                    operational: signature.operational.published([false; 2]),
                    reach: &reach,
                    entries: &entries,
                },
                &mut identities,
            )
            .map_err(unavailable)?;
            let service = *terminal_check_tables::row(
                &self.boundary_services,
                boundary,
                caller,
                Construct::Machine,
            )?;
            let declaration = crate::lower::boundary(
                caller, boundary, signatures, outcomes, records, metadata, service,
            )?;
            boundaries.push(declaration);
        }
        module.identities = identities.build();
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
}
