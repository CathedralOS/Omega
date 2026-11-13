//! A Build backing retains the program the lowered checker accepted. Selecting
//! its public implementation never lowers another program or drops companions.

use std::sync::Arc;

use diagnostics::Diagnostic;
use semantic_vocabulary::{BoundaryMachineId, IdTable, MachineId, PackageKeyIdentity};
use symbols::SymbolHandle;
use terminal_codec::{CanonicalTerminalArtifact, EvidenceSeals};
use typed_trees::trait_definition::AppliedTraitScope;
use typed_trees::{FrozenTypedTrees, TypedTrees};

use crate::PortableTerminalProgram;

pub struct ProducedBuildProvider {
    program: Arc<PortableTerminalProgram>,
    // These are the actual source and proof owners of `program`, retained even
    // when an empty requirement has no executable slots.
    _typed: Arc<FrozenTypedTrees>,
    _evidence: Arc<EvidenceSeals>,
    has_code: bool,
    path: String,
    package: PackageKeyIdentity,
    declaration: String,
    requirement: String,
    application: String,
}

impl ProducedBuildProvider {
    pub fn artifact(&self) -> Option<&Arc<CanonicalTerminalArtifact>> {
        self.has_code.then(|| self.program.artifact())
    }
    pub fn program(&self) -> &PortableTerminalProgram {
        &self.program
    }
    pub fn path(&self) -> &str {
        &self.path
    }
    pub fn package(&self) -> PackageKeyIdentity {
        self.package
    }
    pub fn declaration(&self) -> &str {
        &self.declaration
    }
    pub fn requirement(&self) -> &str {
        &self.requirement
    }
    pub fn application(&self) -> &str {
        &self.application
    }
}

/// Target-neutral source coordinates accompany the accepted portable program.
/// Omega decomposes its publication packet at this boundary; Psi does not read
/// provider settlement or reconstruct a historical checked-tree owner.
pub struct BuildProviderProductionRequest<'source> {
    pub typed: &'source Arc<FrozenTypedTrees>,
    pub program: Arc<PortableTerminalProgram>,
    pub evidence: &'source Arc<EvidenceSeals>,
    pub machines: &'source IdTable<MachineId, SymbolHandle>,
    pub boundaries: &'source IdTable<BoundaryMachineId, SymbolHandle>,
    pub exact_toolchain_sources: &'source [(source::SourceId, [u8; 32])],
    pub package: PackageKeyIdentity,
    pub declaration_path: &'source str,
}

impl BuildProviderProductionRequest<'_> {
    pub fn produce(self) -> Result<ProducedBuildProvider, Vec<Diagnostic>> {
        let refuse = |reason: &str| vec![Diagnostic::error(reason)];
        let typed = self.typed;
        let module = self.program.artifact().semantic_module();
        if self.machines.len() != module.machines.len()
            || self.boundaries.len() != module.boundary_machines.len()
        {
            return Err(refuse(
                "Build provider source census differs from its accepted program",
            ));
        }
        let machine_ids = module
            .machines
            .iter()
            .map(|machine| machine.id)
            .collect::<Vec<_>>();
        self.evidence
            .project_for_machines(module, &machine_ids)
            .map_err(|reason| refuse(&format!("Build provider evidence: {reason}")))?;
        for machine in &module.machines {
            let source = self
                .machines
                .get(machine.id)
                .copied()
                .ok_or_else(|| refuse("Build provider machine lost its source declaration"))?;
            let declaration = machine
                .declaration
                .as_ref()
                .ok_or_else(|| refuse("Build provider machine has no checked declaration"))?;
            let original = typed
                .machine_by_symbol(source)
                .ok_or_else(|| refuse("Build provider machine source is not a typed machine"))?;
            let semantic = typed
                .normalized_hermetic_symbol_identity(source)
                .map_err(|reason| refuse(&reason))?;
            if module.identities.text(declaration.semantic_declaration) != Some(semantic.as_str())
                || typed.machine_callable_identity_text(original).as_deref()
                    != module.identities.text(declaration.callable)
            {
                return Err(refuse(
                    "Build provider machine changes its accepted source identity",
                ));
            }
        }
        for boundary in &module.boundary_machines {
            let source = self
                .boundaries
                .get(boundary.id)
                .copied()
                .ok_or_else(|| refuse("Build provider boundary lost its source declaration"))?;
            let declaration = boundary
                .declaration
                .as_ref()
                .ok_or_else(|| refuse("Build provider boundary has no checked declaration"))?;
            let semantic = typed
                .normalized_hermetic_symbol_identity(source)
                .map_err(|reason| refuse(&reason))?;
            if module
                .identities
                .text(declaration.source.semantic_declaration)
                != Some(semantic.as_str())
            {
                return Err(refuse(
                    "Build provider boundary changes its accepted source identity",
                ));
            }
        }
        let mut candidates = typed.conformances().iter().filter(|conformance| {
            conformance.is_public
                && typed
                    .symbols
                    .symbol_product_package_identity(conformance.symbol)
                    == Some(self.package)
                && typed.symbols.display_path(conformance.symbol, "::") == self.declaration_path
        });
        let selected = candidates.next().ok_or_else(|| {
            refuse("Build backing names no public implementation in its exact dependency package")
        })?;
        if candidates.next().is_some() {
            return Err(refuse(
                "Build backing declaration is ambiguous in its dependency package",
            ));
        }
        let scope = AppliedTraitScope::from_conformance(typed, selected).ok_or_else(|| {
            refuse("Build backing has no complete checked requirement application")
        })?;
        if !scope.declaration.is_boundary
            || !selected.type_parameters.is_empty()
            || !selected.lifetime_parameters.is_empty()
        {
            return Err(refuse(
                "Build backing must name a closed implementation of a boundary requirement",
            ));
        }
        typed
            .require_receiving_trait_operands(&scope)
            .map_err(refuse)?;
        let nominal = |symbol| {
            typed
                .exact_owner_nominal_identity(symbol, self.exact_toolchain_sources)
                .and_then(|identity| typed.type_identity_text(identity))
                .filter(|identity| !identity.is_empty())
                .ok_or_else(|| refuse("Build provider declaration lost its exact portable owner"))
        };
        let declaration = nominal(selected.symbol)?;
        let requirement = nominal(scope.declaration.symbol)?;
        let application = scope
            .portable_application_identity(typed)
            .and_then(|identity| typed.type_identity_text(identity))
            .filter(|identity| !identity.is_empty())
            .ok_or_else(|| {
                refuse("Build backing requirement application is not closed and portable")
            })?;
        for row in &selected.rows {
            let mut admitted = module.provider_candidates.iter().filter(|candidate| {
                module
                    .identities
                    .text(candidate.provider_declaration.identity())
                    == Some(declaration.as_str())
                    && self.machines.get(candidate.candidate) == Some(&row.realization_machine)
                    && self
                        .boundaries
                        .get(candidate.boundary)
                        .is_some_and(|source| {
                            *source == row.requirement
                                || typed
                                    .closed_requirement_application(*source)
                                    .is_some_and(|closed| closed.template == row.requirement)
                        })
                    && module.identities.text(candidate.receiving_application)
                        == Some(application.as_str())
            });
            if admitted.next().is_none() || admitted.next().is_some() {
                return Err(refuse(
                    "Build backing has no unique admitted candidate for its exact applied requirement",
                ));
            }
        }
        // Every companion must still name this artifact. Retain the real
        // batches rather than manufacturing empty records at this boundary.
        let view: terminal_codec::TerminalExecutionView<'_> = self.program.artifact().view().into();
        if self.program.build_bindings().calls_for(view).is_none()
            || self
                .program
                .build_bindings()
                .declarations_for(view)
                .is_none()
            || self.program.build_bindings().queries_for(view).is_none()
            || self
                .program
                .build_bindings()
                .native_formations_for(view)
                .is_none()
            || self.program.build_requests().calls_for(view).is_none()
            || self.program.build_filesystem().calls_for(view).is_none()
        {
            return Err(refuse(
                "Build provider companions belong to another accepted program",
            ));
        }
        Ok(ProducedBuildProvider {
            program: self.program,
            _typed: Arc::clone(typed),
            _evidence: Arc::clone(self.evidence),
            has_code: !selected.rows.is_empty(),
            path: self.declaration_path.to_owned(),
            package: self.package,
            declaration,
            requirement,
            application,
        })
    }
}

/// A closed conformance is an authored requirement application even when no
/// body invokes that requirement. Materialize it through the same application
/// owner used by calls and Product operands before typed trees are frozen.
pub fn prepare_build_provider_requirements(
    program: &mut TypedTrees,
) -> Result<(), Vec<Diagnostic>> {
    let refuse = |reason: &str| vec![Diagnostic::error(reason)];
    let mut applications = Vec::new();
    for conformance in program.conformances().iter().filter(|conformance| {
        conformance.type_parameters.is_empty()
            && conformance.lifetime_parameters.is_empty()
            && conformance.trait_lifetime_arguments.is_empty()
    }) {
        let Some(scope) = AppliedTraitScope::from_conformance(program, conformance) else {
            continue;
        };
        if !scope.declaration.is_boundary {
            continue;
        }
        for row in &conformance.rows {
            let declaring = scope
                .declaring_scope(program, row.declaring_trait)
                .ok_or_else(|| {
                    refuse("Build provider requirement lost its exact declaring application")
                })?;
            if declaring.keeps_declared_signatures(program) {
                continue;
            }
            let (_, signature) = program
                .trait_machine_signature(row.requirement)
                .ok_or_else(|| refuse("Build provider row names no requirement signature"))?;
            // A local family is not a closed backing. Its own formation owner
            // reports it; this operation never invents actuals for its binders.
            if !signature.type_parameters.is_empty() || !signature.lifetime_parameters.is_empty() {
                continue;
            }
            applications.push((
                conformance.symbol,
                row.requirement,
                signature.name.clone(),
                declaring.environment(),
            ));
        }
    }
    for (owner, requirement, name, environment) in applications {
        let arguments = program
            .materialize_trait_application_arguments(&environment)
            .map_err(refuse)?;
        let application =
            program
                .expression_table
                .retain_static_machine_application(requirement, name, &[]);
        let source = program
            .expression_table
            .static_machine_application_mut(application);
        source.lexical_owner = owner;
        source.binding_declaration = requirement;
        program
            .instantiate_requirement_application(&environment, requirement, &arguments, application)
            .map_err(refuse)?;
    }
    Ok(())
}
