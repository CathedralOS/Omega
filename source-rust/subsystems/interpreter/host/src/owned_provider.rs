//! Bind a supplier's exported record to the admitted Psi image that will run it.
use component_description::{
    AcceptedComponentAssumptions, COMPONENT_DESCRIPTION_SCHEMA_V3, ComponentVerificationRequest,
    verify_component,
};
use psi_authorization::AuthorizedTerminalArtifact;
use semantic_vocabulary::BoundaryMachineId;
use terminal_interpreter::{
    TerminalInstance, TerminalInstantiation, TerminalProgram, TerminalRuntimeSupplies,
    TerminalRuntimeSupplyRequest,
};
use terminal_psi::{TargetMachineCallable, TerminalIdentityIndex};

/// An installed provider with no outstanding import, resource or host-effect
/// obligations. Loading a new one does not change an existing installation.
pub struct OwnedPsiProvider {
    receiver: terminal_psi::TerminalPsiIdentity,
    program: TerminalProgram<'static>,
    members: Vec<InstalledMember>,
}

struct InstalledMember {
    receiving: BoundaryMachineId,
    supplies: TerminalRuntimeSupplies,
}

/// The member and any invocation started from it borrow the installed image.
///
/// Even a member returned by a helper prevents an earlier replacement:
/// ```compile_fail,E0506
/// use psi_authorization::AuthorizedTerminalArtifact;
/// use semantic_vocabulary::BoundaryMachineId;
/// use terminal_runtime::embedding::{OwnedPsiProvider, PsiProviderMember};
///
/// fn project<'a>(p: &'a OwnedPsiProvider, app: &AuthorizedTerminalArtifact, id: BoundaryMachineId)
///     -> PsiProviderMember<'a> { p.member(app, id).unwrap() }
///
/// fn swap_too_early(p: &mut OwnedPsiProvider, next: OwnedPsiProvider,
///                   app: &AuthorizedTerminalArtifact, id: BoundaryMachineId) {
///     let call = project(p, app, id);
///     *p = next;
///     let _ = call.start(Default::default());
/// }
/// ```
pub struct PsiProviderMember<'owner> {
    owner: &'owner OwnedPsiProvider,
    member: &'owner InstalledMember,
}

impl OwnedPsiProvider {
    pub fn load(
        receiver: &AuthorizedTerminalArtifact,
        declaration: TerminalIdentityIndex,
        application: TerminalIdentityIndex,
        image: AuthorizedTerminalArtifact,
        description: &[u8],
        environment: TerminalRuntimeSupplyRequest<'_>,
    ) -> Result<Self, String> {
        if environment.entry.is_some() {
            return Err("the exported operation, not the caller, supplies the entry".into());
        }
        let checked = verify_component(
            description,
            &ComponentVerificationRequest {
                expected_subject: image.artifact().view().identity(),
                accepted_schemas: [COMPONENT_DESCRIPTION_SCHEMA_V3].into(),
                accepted_assumptions: AcceptedComponentAssumptions::Exact(Default::default()),
            },
        )
        .and_then(|component| component.with_executable_artifact(image.shared_artifact()))
        .map_err(|reason| reason.to_string())?;
        let left = receiver.artifact().semantic_module();
        let expected = left.service_schema(declaration, application)?;
        let right = checked.module();
        let identities_match = |a, b| {
            left.identities
                .text(a)
                .zip(right.identities.text(b))
                .is_some_and(|(x, y)| !x.is_empty() && x == y)
        };
        let mut exports = right.interfaces.provider_exports.iter().filter(|e| {
            identities_match(declaration, e.schema_declaration)
                && identities_match(application, e.schema_application)
        });
        let (Some(export), None) = (exports.next(), exports.next()) else {
            return Err("image does not export the requested closed provider record".into());
        };
        let actual = right.service_schema(export.schema_declaration, export.schema_application)?;
        if actual.surface != expected.surface {
            return Err("exported and requested operation record contracts differ".into());
        }
        // Admit demands only for an actual export of the requested record.
        // This route does not install suppliers with unmet dependencies.
        if !(checked.imports().is_empty()
            && checked.obligations().is_empty()
            && checked.custody().is_empty()
            && checked.outgoing().is_empty())
        {
            return Err("provider needs import, authority, resource or custody admission".into());
        }
        let program = TerminalProgram::retain_authorized(&image);
        let mut members = Vec::with_capacity(export.members.len());
        for member in &export.members {
            let boundary = right
                .boundary_machine(member.boundary)
                .ok_or("exported member has no boundary")?;
            let mut receiving = left.interfaces.boundary_machines.iter().filter(|b| {
                identities_match(b.identity, boundary.identity)
                    && b.owner.same_identity_as(
                        &left.identities,
                        &boundary.owner,
                        &right.identities,
                    )
            });
            let (Some(receiving), None) = (receiving.next(), receiving.next()) else {
                return Err("receiver has no unique published requirement for the export".into());
            };
            // Check the dependent callable contract between the two artifacts,
            // including loans and evidence, exactly as for a static component.
            let admitted = psi_provider_selections::admit_component_provider(
                receiver.artifact().view(),
                receiving.id,
                image.artifact().view(),
                &psi_provider_selections::ProviderInstallationSelection {
                    boundary: member.boundary,
                    provider_identity: right
                        .identities
                        .text(export.provider_identity)
                        .ok_or("missing provider identity")?
                        .into(),
                    provider_declaration: right
                        .identities
                        .text(export.provider_declaration)
                        .ok_or("missing provider declaration")?
                        .into(),
                    candidate: member.machine,
                },
            )?;
            let supplies = Self::install_entry(&program, &admitted, environment)?;
            members.push(InstalledMember {
                receiving: admitted.receiving_boundary(),
                supplies,
            });
        }
        members.sort_unstable_by_key(|m| m.receiving);
        Ok(Self {
            receiver: receiver.artifact().view().identity(),
            program,
            members,
        })
    }

    fn install_entry(
        program: &TerminalProgram<'_>,
        admitted: &psi_provider_selections::AdmittedComponentProvider,
        environment: TerminalRuntimeSupplyRequest<'_>,
    ) -> Result<TerminalRuntimeSupplies, String> {
        if program.terminal_psi() != admitted.provider() {
            return Err("exported entry is not backed by this installed program".into());
        }
        let machine = admitted.candidate();
        let supplies = program
            .admit_runtime_supplies(TerminalRuntimeSupplyRequest {
                entry: Some(TargetMachineCallable::Machine(machine)),
                ..environment
            })
            .map_err(|e| format!("exported provider entry: {e:?}"))?;
        if program
            .entry_installed(&supplies)
            .map_err(|e| format!("installed provider entry: {e:?}"))?
            .machine
            != machine
        {
            return Err("provider installation selects a different machine".into());
        }
        let needs = program
            .execution_requirements(Some(&supplies))
            .map_err(|e| format!("exported provider dependencies: {e:?}"))?;
        if !(needs.imports.is_empty()
            && needs.missing_providers.is_empty()
            && needs.external_completions.is_empty()
            && needs.scalar_applications.is_empty()
            && needs.scalar_entry_conditions.is_empty())
        {
            return Err("exported entry has unresolved execution requirements".into());
        }
        Ok(supplies)
    }

    pub fn member(
        &self,
        receiver: &AuthorizedTerminalArtifact,
        requirement: BoundaryMachineId,
    ) -> Option<PsiProviderMember<'_>> {
        if receiver.artifact().view().identity() != self.receiver {
            return None;
        }
        let i = self
            .members
            .binary_search_by_key(&requirement, |m| m.receiving)
            .ok()?;
        Some(PsiProviderMember {
            owner: self,
            member: &self.members[i],
        })
    }
}

impl<'owner> PsiProviderMember<'owner> {
    pub fn start(
        &self,
        request: TerminalInstantiation<'_>,
    ) -> Result<TerminalInstance<'owner, 'owner>, String> {
        if request.installation.is_some() {
            return Err("a projected member retains its provider's installation".into());
        }
        // Reuse the installed code and its catalogs for every call; an
        // invocation still borrows this particular owner's installation.
        let program: &TerminalProgram<'owner> = &self.owner.program;
        program
            .instantiate(TerminalInstantiation {
                installation: Some(&self.member.supplies),
                ..request
            })
            .map_err(|e| format!("start exported provider member: {e:?}"))
    }
}
