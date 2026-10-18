//! Admit receiving-root selections against an exact Terminal artifact.
//! Catalog availability is not selection; assumed relationships remain explicit
//! conditional evidence. Runtime host adapters and dispatch state live in the
//! execution engine, not in this immutable selection result.

use semantic_vocabulary::{BoundaryMachineId, MachineId};
use std::collections::{BTreeMap, BTreeSet};

/// Omega-owned policy input naming one exact verified terminal provider row.
/// Selection is intentionally separate from terminal-Psi semantic bytes.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProviderInstallationSelection {
    pub boundary: BoundaryMachineId,
    pub provider_identity: String,
    /// Canonical selecting declaration, distinct even when two maps share
    /// a display path and the same physical callable body.
    pub provider_declaration: String,
    pub candidate: MachineId,
}

/// Omega-owned policy input authorizing one root-level assumed binding
/// contract relationship
/// (wiki/spec/build/assumed_binding_contracts.md).
///
/// This is the runtime-policy side of `Build::assume_binding_contract`: only
/// the consuming runtime root may authorize the relationship, so it arrives
/// as sponsor-supplied installation input, never as artifact bytes — a
/// dependency cannot grant the assumption to itself. Admission replays the
/// link against the verified catalog: the supplied side must reach one
/// selected implementation, competing checked or assumed supplies for the
/// required requirement reject, and cyclic binding aliases reject. The
/// relationship applies at this crossing only; it mints no
/// `provider_candidates` conformance and no `installed` redirect, so a
/// direct call of the supplied requirement keeps its own checked evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AssumedBindingSelection {
    /// The caller-facing requirement (`Required::operation`) whose link the
    /// root assumes.
    pub required: BoundaryMachineId,
    /// The requirement (`Supplied::operation`) whose selected implementation
    /// the assumed link invokes.
    pub supplied_requirement: BoundaryMachineId,
}

/// One admitted assumed binding contract relationship, bound to the same
/// terminal-Psi identity as the checked provider installation it extends.
///
/// `implementation` is the actual selected code the assumed link invokes —
/// binding identity cannot hide it. `supply_path` retains every assumed link
/// followed to reach it, outermost first, so a chained assumption keeps its
/// complete transitive trust record. Both requirements retain their own
/// identities: the row is conditional evidence under the recorded
/// assumption, not a checked conformance between them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedAssumedBinding {
    /// The caller-facing requirement (`Required::operation`).
    pub required: BoundaryMachineId,
    /// The requirement (`Supplied::operation`) whose selection resolves the
    /// actual implementation.
    pub supplied_requirement: BoundaryMachineId,
    /// The concrete selected implementation reached by following the supply
    /// side's selection and any nested assumed links — never the abstract
    /// requirement itself.
    pub implementation: MachineId,
    pub selected_requirement: BoundaryMachineId,
    /// Requirements traversed through nested assumed links before reaching
    /// `implementation`, in traversal order. Empty when
    /// `supplied_requirement` had an ordinary checked selection.
    pub supply_path: Vec<BoundaryMachineId>,
}

/// Immutable checked-provider selections for one exact Terminal artifact.
/// Private fields can only be constructed through catalog admission. This
/// grants no host adapter, external-completion authority, or runtime state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedProviderSelections {
    terminal_psi: terminal_psi::TerminalPsiIdentity,
    providers: BTreeMap<BoundaryMachineId, AdmittedProviderSelection>,
    assumed_bindings: Vec<AdmittedAssumedBinding>,
}

// Build agreement compares selecting declarations, not just dispatch bodies:
// independent named maps may deliberately share the same implementation.
// The canonical handle is scoped by the enclosing Terminal artifact identity.
#[derive(Debug, Clone, PartialEq, Eq)]
struct AdmittedProviderSelection {
    candidate: MachineId,
    provider_declaration: terminal_psi::NominalDeclarationIdentity,
    receiving_application: terminal_psi::TerminalIdentityIndex,
    receiving_region_arguments: Vec<terminal_psi::TerminalIdentityIndex>,
}

impl AdmittedProviderSelections {
    /// Observe whether this exact artifact-scoped candidate is the installed
    /// row, including the choosing declaration and complete receiving tuple.
    pub fn contains_candidate(
        &self,
        candidate: &terminal_psi::ProviderCandidateConformance,
    ) -> bool {
        self.providers
            .get(&candidate.boundary)
            .is_some_and(|installed| {
                installed.candidate == candidate.candidate
                    && installed.provider_declaration == candidate.provider_declaration
                    && installed.receiving_application == candidate.receiving_application
                    && installed.receiving_region_arguments == candidate.receiving_region_arguments
            })
    }

    pub const fn terminal_psi(&self) -> terminal_psi::TerminalPsiIdentity {
        self.terminal_psi
    }

    pub fn assumed_bindings(&self) -> &[AdmittedAssumedBinding] {
        &self.assumed_bindings
    }

    /// Observe the installed row's exact receiving application, scoped by
    /// this installation's Terminal identity. Zero means no association,
    /// never permission to substitute another application.
    pub fn receiving_application(
        &self,
        boundary: BoundaryMachineId,
    ) -> terminal_psi::TerminalIdentityIndex {
        self.providers.get(&boundary).map_or(
            terminal_psi::TerminalIdentityIndex::default(),
            |selection| selection.receiving_application,
        )
    }

    /// Observe public provider slots in their original order. An empty tuple
    /// grants no region correspondence by itself; consumers also join the
    /// exact installed boundary, application and enclosing artifact identity.
    pub fn receiving_region_arguments(
        &self,
        boundary: BoundaryMachineId,
    ) -> &[terminal_psi::TerminalIdentityIndex] {
        self.providers.get(&boundary).map_or(&[], |selection| {
            selection.receiving_region_arguments.as_slice()
        })
    }

    /// Borrow admitted rows while constructing a consumer's execution state.
    /// Observation provides no way to forge or modify this sealed selection.
    pub fn providers(&self) -> impl Iterator<Item = (BoundaryMachineId, MachineId)> + '_ {
        self.providers
            .iter()
            .map(|(&boundary, selection)| (boundary, selection.candidate))
    }
}

#[derive(Debug)]
pub enum ProviderInstallationError {
    SemanticDecode(terminal_codec::CodecError),
    UnknownOrDuplicateSelection {
        boundary: BoundaryMachineId,
        candidate: MachineId,
    },
    /// One root-authorized assumed binding contract failed admission replay
    /// against the verified artifact.
    AssumedBindingRejected {
        required: BoundaryMachineId,
        supplied_requirement: BoundaryMachineId,
        reason: AssumedBindingRejection,
    },
}

/// Why one assumed binding contract relationship was rejected at
/// installation admission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssumedBindingRejection {
    /// The required side names no boundary machine in the verified artifact.
    UnknownRequirement,
    /// The supplied side names no boundary machine in the verified artifact.
    UnknownSuppliedRequirement,
    /// An ordinary checked provider selection already supplies the required
    /// requirement: competing explicit checked and assumed supplies for the
    /// same operation reject rather than silently preferring the assumption.
    CompetingCheckedSupply,
    /// Another assumed binding already claims the required requirement —
    /// every required member still needs exactly one supply.
    CompetingAssumption,
    /// Following the supplied side's links reaches no selected
    /// implementation: the supplied side must have one selected complete
    /// provider and executable realization.
    UnselectedSupply,
    /// The assumed link resolves through itself or a cycle of assumed
    /// links — cyclic binding aliases reject.
    CyclicSupply,
    IncompatibleRuntimeShape,
    IncompatibleOperationalContract,
    UnsupportedAssumedAuthority,
}

/// Decode a canonical artifact, then admit only selections that exactly name
/// rows in its canonical provider catalog.
pub fn admit_provider_installation_from_artifact(
    semantic_bytes: &[u8],
    selections: &[ProviderInstallationSelection],
) -> Result<AdmittedProviderSelections, ProviderInstallationError> {
    admit_provider_installation_with_assumed_bindings_from_artifact(semantic_bytes, selections, &[])
}

/// Admit receiving-host selections against a retained canonical artifact.
/// The immutable view preserves the module/identity relationship; selection
/// and assumption checks are identical to external byte admission. No module
/// decoding or identity reconstruction is needed within the same invocation.
pub fn admit_provider_installation(
    artifact: terminal_codec::TerminalArtifactView<'_>,
    selections: &[ProviderInstallationSelection],
    assumptions: &[AssumedBindingSelection],
) -> Result<AdmittedProviderSelections, ProviderInstallationError> {
    admit_provider_selections(
        artifact.module(),
        artifact.identity(),
        selections,
        assumptions,
    )
}

/// Decode a canonical artifact, then admit its provider selections and the
/// consuming root's assumed binding contract relationships as one
/// installation bound to that exact artifact identity.
///
/// `assumptions` are the runtime-policy side of
/// `Build::assume_binding_contract<Required::operation, Supplied::operation>(builder)`
/// (wiki/spec/build/assumed_binding_contracts.md): only the consuming runtime
/// root may authorize them, so they arrive as sponsor input beside the
/// checked selections, never from artifact bytes — a dependency cannot grant
/// the assumption to itself. Admission replays each link against the
/// verified catalog. An unknown requirement on either side, a competing
/// checked selection or second assumption for the required requirement, a
/// supply chain which reaches no selected implementation, and a cyclic
/// binding alias each reject the whole installation.
///
/// An admitted assumption is retained on the installation as conditional
/// evidence — the artifact's guarantees hold under the recorded assumption,
/// whose falsity invalidates them without producing any unconditional claim.
/// It mints no `provider_candidates` row and no checked `installed`
/// redirect: the relationship applies at the named crossing only, so
/// ordinary boundary dispatch and direct calls of either requirement keep
/// their own evidence custody.
pub fn admit_provider_installation_with_assumed_bindings_from_artifact(
    semantic_bytes: &[u8],
    selections: &[ProviderInstallationSelection],
    assumptions: &[AssumedBindingSelection],
) -> Result<AdmittedProviderSelections, ProviderInstallationError> {
    let module = terminal_codec::decode_module(semantic_bytes)
        .map_err(ProviderInstallationError::SemanticDecode)?;
    let identity = terminal_codec::terminal_psi_identity(&module)
        .map_err(ProviderInstallationError::SemanticDecode)?;
    admit_provider_selections(&module, identity, selections, assumptions)
}

fn admit_provider_selections(
    module: &terminal_psi::TerminalModule,
    identity: terminal_psi::TerminalPsiIdentity,
    selections: &[ProviderInstallationSelection],
    assumptions: &[AssumedBindingSelection],
) -> Result<AdmittedProviderSelections, ProviderInstallationError> {
    let mut installed = BTreeMap::new();
    for selection in selections {
        let mut candidates = module.provider_candidates.iter().filter(|candidate| {
            candidate.boundary == selection.boundary
                && module.identities.text(candidate.provider_identity)
                    == Some(selection.provider_identity.as_str())
                && module
                    .identities
                    .text(candidate.provider_declaration.identity())
                    == Some(selection.provider_declaration.as_str())
                && candidate.candidate == selection.candidate
        });
        let candidate = candidates.next();
        if selection.provider_identity.is_empty()
            || selection.provider_declaration.is_empty()
            || module
                .boundary_machine(selection.boundary)
                .is_none_or(|boundary| {
                    boundary.role != terminal_psi::BoundaryDeclarationRole::Callable
                })
            || installed.contains_key(&selection.boundary)
            || candidates.next().is_some()
        {
            return Err(ProviderInstallationError::UnknownOrDuplicateSelection {
                boundary: selection.boundary,
                candidate: selection.candidate,
            });
        }
        let Some(candidate) = candidate else {
            return Err(ProviderInstallationError::UnknownOrDuplicateSelection {
                boundary: selection.boundary,
                candidate: selection.candidate,
            });
        };
        installed.insert(
            selection.boundary,
            AdmittedProviderSelection {
                candidate: candidate.candidate,
                provider_declaration: candidate.provider_declaration.clone(),
                receiving_application: candidate.receiving_application,
                receiving_region_arguments: candidate.receiving_region_arguments.clone(),
            },
        );
    }
    let assumed_bindings = admit_assumed_bindings(module, &installed, assumptions)?;
    Ok(AdmittedProviderSelections {
        terminal_psi: identity,
        providers: installed,
        assumed_bindings,
    })
}

/// Replay each root-authorized assumed binding against the verified catalog
/// and the admitted checked selections.
///
/// Every returned row carries the actual implementation its assumed link
/// invokes: the supply side resolves through its checked selection or
/// through nested assumed links — retaining each traversed requirement in
/// `supply_path` — and never through artifact bytes alone.
fn admit_assumed_bindings(
    module: &terminal_psi::TerminalModule,
    installed: &BTreeMap<BoundaryMachineId, AdmittedProviderSelection>,
    assumptions: &[AssumedBindingSelection],
) -> Result<Vec<AdmittedAssumedBinding>, ProviderInstallationError> {
    let boundary_ids: BTreeSet<BoundaryMachineId> = module
        .boundary_machines
        .iter()
        .filter(|boundary| boundary.role == terminal_psi::BoundaryDeclarationRole::Callable)
        .map(|boundary| boundary.id)
        .collect();
    let mut admitted = Vec::new();
    for assumption in assumptions {
        let reject = |reason| ProviderInstallationError::AssumedBindingRejected {
            required: assumption.required,
            supplied_requirement: assumption.supplied_requirement,
            reason,
        };
        if !boundary_ids.contains(&assumption.required) {
            return Err(reject(AssumedBindingRejection::UnknownRequirement));
        }
        // Competing explicit checked and assumed supplies for the same
        // operation reject rather than silently preferring the assumption.
        if installed.contains_key(&assumption.required) {
            return Err(reject(AssumedBindingRejection::CompetingCheckedSupply));
        }
        // Every required member still needs exactly one supply.
        if admitted
            .iter()
            .any(|binding: &AdmittedAssumedBinding| binding.required == assumption.required)
        {
            return Err(reject(AssumedBindingRejection::CompetingAssumption));
        }
        if !boundary_ids.contains(&assumption.supplied_requirement) {
            return Err(reject(AssumedBindingRejection::UnknownSuppliedRequirement));
        }
        // Semantic entry/exit assumptions cannot recast carriers or grant
        // custody, calling-mode or operational compatibility. Both declarations
        // come from the independently verified catalog and retain their identities.
        let required =
            terminal_psi::find_boundary_machine(&module.boundary_machines, assumption.required)
                .unwrap();
        let supplied = terminal_psi::find_boundary_machine(
            &module.boundary_machines,
            assumption.supplied_requirement,
        )
        .unwrap();
        let qualified_result = |boundary: &terminal_psi::BoundaryMachineDeclaration| {
            matches!(&boundary.result, terminal_psi::BoundaryMachineResult::Structural(result)
                if !result.qualifications.is_empty() || !result.projected_qualifications.is_empty())
        };
        if !required.program_local_root_introductions.is_empty()
            || !supplied.program_local_root_introductions.is_empty()
            || !required.content_guarantees.is_empty()
            || !supplied.content_guarantees.is_empty()
            || required.requires != supplied.requires
            || qualified_result(required)
            || qualified_result(supplied)
        {
            return Err(reject(AssumedBindingRejection::UnsupportedAssumedAuthority));
        }
        if required.parameter_order != supplied.parameter_order
            || required.scalar_parameters != supplied.scalar_parameters
            || required.structural_parameters != supplied.structural_parameters
            || required.result != supplied.result
            || required.reference_outputs != supplied.reference_outputs
            || required.attachment != supplied.attachment
        {
            return Err(reject(AssumedBindingRejection::IncompatibleRuntimeShape));
        }
        if required.kind != supplied.kind
            || required.completion != supplied.completion
            || (required.unconditional_termination && !supplied.unconditional_termination)
            || required.crash_routes != supplied.crash_routes
            || supplied.published_service_ceiling.iter().any(|service| {
                Some(*service) != supplied.owning_service
                    && !required.published_service_ceiling.contains(service)
            })
        {
            return Err(reject(
                AssumedBindingRejection::IncompatibleOperationalContract,
            ));
        }
        // Follow the supply side's links to the actual selected
        // implementation, retaining every traversed assumption. Cyclic
        // binding aliases reject rather than resolving through themselves.
        let mut visiting = BTreeSet::from([assumption.required]);
        let mut supply_path = Vec::new();
        let mut requirement = assumption.supplied_requirement;
        let implementation = loop {
            if let Some(candidate) = installed.get(&requirement) {
                break candidate.candidate;
            }
            if !visiting.insert(requirement) {
                return Err(reject(AssumedBindingRejection::CyclicSupply));
            }
            let Some(next) = assumptions.iter().find(|link| link.required == requirement) else {
                return Err(reject(AssumedBindingRejection::UnselectedSupply));
            };
            supply_path.push(requirement);
            requirement = next.supplied_requirement;
        };
        admitted.push(AdmittedAssumedBinding {
            required: assumption.required,
            supplied_requirement: assumption.supplied_requirement,
            implementation,
            selected_requirement: requirement,
            supply_path,
        });
    }
    Ok(admitted)
}
