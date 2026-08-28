use crate::{
    ContainmentEvidence, ContainmentGuarantee, ExecutableEntryOrigin, ExecutableIdentity,
    ExecutableTcbEntry, ExecutableTcbProfileAcceptance, ExecutionScope, ImplementationEvidence,
    ProviderIdentity, ScopeCompleteness, SelectedProviderRequirement,
};

/// Owner of one selected-provider manifest contributing to a live process
/// scope. Process-static executables remain a baseline instead of becoming
/// private to every replaceable era that uses them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutableManifestSource {
    ProcessStaticBaseline,
    ComponentEra(u64),
}

/// One profile-accepted component-era manifest retained under its exact live
/// era identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedExecutableEra {
    identity: u64,
    acceptance: ExecutableTcbProfileAcceptance,
}

impl AdmittedExecutableEra {
    pub const fn identity(&self) -> u64 {
        self.identity
    }

    pub const fn acceptance(&self) -> &ExecutableTcbProfileAcceptance {
        &self.acceptance
    }
}

/// Exact entry contribution from one live manifest. More than one contribution
/// can name the same executable subject when eras retain different containment
/// evidence for it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ExecutableEntryContribution {
    source: ExecutableManifestSource,
    entry: ExecutableTcbEntry,
}

/// One executable subject in the coexistence union. A selected provider
/// requirement remains part of that subject even when two rows share physical
/// code. Containment is deliberately not folded into the subject: the live
/// guarantee is the intersection of independently retained contributions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoexistingExecutableTcbEntry {
    pub provider_identity: ProviderIdentity,
    pub provider_plan_identity: u64,
    pub selected_requirement: Option<SelectedProviderRequirement>,
    pub executable_identity: ExecutableIdentity,
    pub implementation_evidence: ImplementationEvidence,
    pub origin: ExecutableEntryOrigin,
    pub execution_scope: ExecutionScope,
    contributions: Vec<ExecutableEntryContribution>,
}

impl CoexistingExecutableTcbEntry {
    pub fn sources(&self) -> Vec<ExecutableManifestSource> {
        let mut sources = Vec::new();
        for contribution in &self.contributions {
            if !sources.contains(&contribution.source) {
                sources.push(contribution.source);
            }
        }
        sources
    }

    /// A containment axis applies to the live union only when every manifest
    /// row contributing this executable subject carries independent evidence
    /// for that axis.
    pub fn has_universal_containment(&self, guarantee: ContainmentGuarantee) -> bool {
        !self.contributions.is_empty()
            && self.contributions.iter().all(|contribution| {
                contribution
                    .entry
                    .containment
                    .iter()
                    .any(|evidence| evidence.guarantee == guarantee)
            })
    }

    /// Return every exact evidence identity for a universally established
    /// axis. `None` means at least one contributing row lacks the guarantee.
    pub fn universal_containment_evidence(
        &self,
        guarantee: ContainmentGuarantee,
    ) -> Option<Vec<AttributedContainmentEvidence>> {
        if !self.has_universal_containment(guarantee) {
            return None;
        }
        Some(
            self.contributions
                .iter()
                .filter_map(|contribution| {
                    contribution
                        .entry
                        .containment
                        .iter()
                        .find(|evidence| evidence.guarantee == guarantee)
                        .cloned()
                        .map(|evidence| AttributedContainmentEvidence {
                            source: contribution.source,
                            evidence,
                        })
                })
                .collect(),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttributedContainmentEvidence {
    pub source: ExecutableManifestSource,
    pub evidence: ContainmentEvidence,
}

/// Exact source result retained in the aggregate completeness decision. This
/// avoids inventing a synthetic selected-provider closure identity for several
/// independently selected eras.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttributedManifestCompleteness {
    pub source: ExecutableManifestSource,
    pub profile: String,
    pub completeness: ScopeCompleteness,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoexistingScopeCompleteness {
    Complete {
        scope: ExecutionScope,
        sources: Vec<AttributedManifestCompleteness>,
    },
    Incomplete {
        scope: ExecutionScope,
        sources: Vec<AttributedManifestCompleteness>,
    },
}

impl CoexistingScopeCompleteness {
    pub const fn scope(&self) -> ExecutionScope {
        match self {
            Self::Complete { scope, .. } | Self::Incomplete { scope, .. } => *scope,
        }
    }

    pub fn sources(&self) -> &[AttributedManifestCompleteness] {
        match self {
            Self::Complete { sources, .. } | Self::Incomplete { sources, .. } => sources,
        }
    }

    pub const fn is_complete(&self) -> bool {
        matches!(self, Self::Complete { .. })
    }
}

/// Scope-relative live report for one process-static baseline and every
/// coexisting component era admitted so far.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoexistingExecutableTcbReport {
    scope: ExecutionScope,
    known_entries: Vec<CoexistingExecutableTcbEntry>,
    completeness: CoexistingScopeCompleteness,
}

impl CoexistingExecutableTcbReport {
    pub const fn scope(&self) -> ExecutionScope {
        self.scope
    }

    pub fn known_entries(&self) -> &[CoexistingExecutableTcbEntry] {
        &self.known_entries
    }

    pub const fn completeness(&self) -> &CoexistingScopeCompleteness {
        &self.completeness
    }
}

/// Accepted manifest set for process-static services plus independently named
/// live component eras. Removal is intentionally absent until an era ledger
/// supplies exact closing and quiescence evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoexistingExecutableTcbSet {
    scope: ExecutionScope,
    process_static_baseline: ExecutableTcbProfileAcceptance,
    eras: Vec<AdmittedExecutableEra>,
}

impl CoexistingExecutableTcbSet {
    pub fn new(process_static_baseline: ExecutableTcbProfileAcceptance) -> Result<Self, String> {
        let scope = process_static_baseline.scope();
        validate_accepted_manifest(&process_static_baseline, scope)?;
        Ok(Self {
            scope,
            process_static_baseline,
            eras: Vec::new(),
        })
    }

    pub const fn scope(&self) -> ExecutionScope {
        self.scope
    }

    pub const fn process_static_baseline(&self) -> &ExecutableTcbProfileAcceptance {
        &self.process_static_baseline
    }

    pub fn eras(&self) -> &[AdmittedExecutableEra] {
        &self.eras
    }

    pub fn admit_era(
        &mut self,
        identity: u64,
        acceptance: ExecutableTcbProfileAcceptance,
    ) -> Result<(), String> {
        if identity == 0 {
            return Err("component executable era has the reserved zero identity".into());
        }
        if self.eras.iter().any(|era| era.identity == identity) {
            return Err(format!(
                "component executable era {identity:#018x} is admitted more than once"
            ));
        }
        validate_accepted_manifest(&acceptance, self.scope)?;
        self.eras.push(AdmittedExecutableEra {
            identity,
            acceptance,
        });
        self.eras.sort_by_key(|era| era.identity);
        Ok(())
    }

    pub(crate) fn retire_era_after_quiescence(
        &mut self,
        identity: u64,
    ) -> Result<AdmittedExecutableEra, String> {
        let Some(index) = self.eras.iter().position(|era| era.identity == identity) else {
            return Err(format!(
                "component executable era {identity:#018x} is not live"
            ));
        };
        Ok(self.eras.remove(index))
    }

    pub fn live_report(&self) -> CoexistingExecutableTcbReport {
        let mut entries = Vec::new();
        let mut completeness = Vec::new();
        accumulate_manifest(
            &mut entries,
            &mut completeness,
            ExecutableManifestSource::ProcessStaticBaseline,
            &self.process_static_baseline,
        );
        for era in &self.eras {
            accumulate_manifest(
                &mut entries,
                &mut completeness,
                ExecutableManifestSource::ComponentEra(era.identity),
                &era.acceptance,
            );
        }
        let weakest_is_complete = completeness.iter().all(|attributed| {
            matches!(attributed.completeness, ScopeCompleteness::Complete { .. })
        });
        let completeness = if weakest_is_complete {
            CoexistingScopeCompleteness::Complete {
                scope: self.scope,
                sources: completeness,
            }
        } else {
            CoexistingScopeCompleteness::Incomplete {
                scope: self.scope,
                sources: completeness,
            }
        };
        CoexistingExecutableTcbReport {
            scope: self.scope,
            known_entries: entries,
            completeness,
        }
    }
}

fn validate_accepted_manifest(
    acceptance: &ExecutableTcbProfileAcceptance,
    required_scope: ExecutionScope,
) -> Result<(), String> {
    if acceptance.scope() != required_scope {
        return Err(format!(
            "accepted executable manifest scope {:?} does not match coexistence scope {:?}",
            acceptance.scope(),
            required_scope
        ));
    }
    let manifest = acceptance.manifest();
    let manifest_scope = match manifest.completeness {
        ScopeCompleteness::Complete { scope, .. } | ScopeCompleteness::Incomplete { scope, .. } => {
            scope
        }
    };
    if manifest_scope != required_scope {
        return Err(format!(
            "accepted executable manifest reports scope {:?}, not coexistence scope {:?}",
            manifest_scope, required_scope
        ));
    }
    if let Some(entry) = manifest
        .known_entries
        .iter()
        .find(|entry| entry.execution_scope != required_scope)
    {
        return Err(format!(
            "accepted executable entry {:?} belongs to scope {:?}, not coexistence scope {:?}",
            entry.executable_identity, entry.execution_scope, required_scope
        ));
    }
    Ok(())
}

fn accumulate_manifest(
    entries: &mut Vec<CoexistingExecutableTcbEntry>,
    completeness: &mut Vec<AttributedManifestCompleteness>,
    source: ExecutableManifestSource,
    acceptance: &ExecutableTcbProfileAcceptance,
) {
    let manifest = acceptance.manifest();
    for entry in &manifest.known_entries {
        if let Some(unioned) = entries
            .iter_mut()
            .find(|unioned| same_executable_subject(unioned, entry))
        {
            let contribution = ExecutableEntryContribution {
                source,
                entry: entry.clone(),
            };
            if !unioned.contributions.contains(&contribution) {
                unioned.contributions.push(contribution);
            }
        } else {
            entries.push(CoexistingExecutableTcbEntry {
                provider_identity: entry.provider_identity.clone(),
                provider_plan_identity: entry.provider_plan_identity,
                selected_requirement: entry.selected_requirement.clone(),
                executable_identity: entry.executable_identity.clone(),
                implementation_evidence: entry.implementation_evidence.clone(),
                origin: entry.origin,
                execution_scope: entry.execution_scope,
                contributions: vec![ExecutableEntryContribution {
                    source,
                    entry: entry.clone(),
                }],
            });
        }
    }
    completeness.push(AttributedManifestCompleteness {
        source,
        profile: acceptance.profile().name.clone(),
        completeness: manifest.completeness.clone(),
    });
}

fn same_executable_subject(
    left: &CoexistingExecutableTcbEntry,
    right: &ExecutableTcbEntry,
) -> bool {
    left.provider_identity == right.provider_identity
        && left.provider_plan_identity == right.provider_plan_identity
        && left.selected_requirement == right.selected_requirement
        && left.executable_identity == right.executable_identity
        && left.implementation_evidence == right.implementation_evidence
        && left.origin == right.origin
        && left.execution_scope == right.execution_scope
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ExecutableTcbManifest, ExecutableTcbProfile, IncompleteCause, IncompleteScopePolicy,
        OpaqueInProcessBinding, evaluate_executable_tcb_profile,
    };

    fn entry(name: &str, containment: Vec<ContainmentEvidence>) -> ExecutableTcbEntry {
        ExecutableTcbEntry {
            provider_identity: ProviderIdentity::NominalType("RuntimeServices".into()),
            provider_plan_identity: 31,
            selected_requirement: None,
            executable_identity: ExecutableIdentity::CurrentArtifactMachine(name.into()),
            implementation_evidence: ImplementationEvidence::CheckedBody {
                machine: name.into(),
            },
            origin: ExecutableEntryOrigin::StaticSelection,
            execution_scope: ExecutionScope::CallerAddressSpace,
            containment,
        }
    }

    fn evidence(guarantee: ContainmentGuarantee, identity: &str) -> ContainmentEvidence {
        ContainmentEvidence {
            guarantee,
            evidence_identity: identity.into(),
        }
    }

    #[test]
    fn coexistence_subject_keeps_selected_overload_identity() {
        let mut first = entry("ConvertProvider::convert", Vec::new());
        first.selected_requirement = Some(SelectedProviderRequirement {
            method: "convert".into(),
            requirement_identity: "named-callable:path=Convert::convert;result=Ordinary".into(),
        });
        let mut second = first.clone();
        second
            .selected_requirement
            .as_mut()
            .expect("selected requirement")
            .requirement_identity = "named-callable:path=Convert::convert;result=Saturating".into();
        let unioned = CoexistingExecutableTcbEntry {
            provider_identity: first.provider_identity.clone(),
            provider_plan_identity: first.provider_plan_identity,
            selected_requirement: first.selected_requirement.clone(),
            executable_identity: first.executable_identity.clone(),
            implementation_evidence: first.implementation_evidence.clone(),
            origin: first.origin,
            execution_scope: first.execution_scope,
            contributions: vec![ExecutableEntryContribution {
                source: ExecutableManifestSource::ProcessStaticBaseline,
                entry: first,
            }],
        };

        assert!(
            !same_executable_subject(&unioned, &second),
            "same executable and plan must not collapse distinct selected overload rows"
        );
    }

    fn accepted(
        profile_name: &str,
        entries: Vec<ExecutableTcbEntry>,
        completeness: ScopeCompleteness,
    ) -> ExecutableTcbProfileAcceptance {
        let manifest = ExecutableTcbManifest {
            known_entries: entries,
            completeness,
        };
        evaluate_executable_tcb_profile(
            &manifest,
            &ExecutableTcbProfile {
                name: profile_name.into(),
                scope: ExecutionScope::CallerAddressSpace,
                allow_static_current_artifact_checked_bodies: true,
                exact_allowances: Vec::new(),
                incomplete_scope: IncompleteScopePolicy::PermitAndMark,
            },
        )
        .expect("test manifest accepted by exact profile")
    }

    fn complete(identity: u64) -> ScopeCompleteness {
        ScopeCompleteness::Complete {
            scope: ExecutionScope::CallerAddressSpace,
            selected_provider_closure_identity: identity,
            opaque_closure_evidence: Vec::new(),
            runtime_closure_evidence: Vec::new(),
        }
    }

    #[test]
    fn live_report_unions_baseline_and_coexisting_eras_without_fabricating_a_closure() {
        let baseline_entry = entry("ProcessClock", Vec::new());
        let baseline = accepted("platform", vec![baseline_entry.clone()], complete(1));
        let mut set = CoexistingExecutableTcbSet::new(baseline).expect("baseline");
        set.admit_era(
            20,
            accepted("era-v2", vec![entry("CodecV2", Vec::new())], complete(20)),
        )
        .expect("second era");
        set.admit_era(
            10,
            accepted(
                "era-v1",
                vec![baseline_entry, entry("CodecV1", Vec::new())],
                complete(10),
            ),
        )
        .expect("first era");

        let report = set.live_report();
        assert_eq!(report.known_entries().len(), 3);
        let clock = report
            .known_entries()
            .iter()
            .find(|entry| {
                matches!(
                    entry.executable_identity,
                    ExecutableIdentity::CurrentArtifactMachine(ref machine)
                        if machine == "ProcessClock"
                )
            })
            .expect("one process-static subject");
        assert_eq!(
            clock.sources(),
            vec![
                ExecutableManifestSource::ProcessStaticBaseline,
                ExecutableManifestSource::ComponentEra(10),
            ]
        );
        assert!(report.completeness().is_complete());
        assert_eq!(report.completeness().sources().len(), 3);
    }

    #[test]
    fn one_incomplete_era_makes_the_live_scope_incomplete_with_attribution() {
        let baseline = accepted("platform", Vec::new(), complete(1));
        let mut set = CoexistingExecutableTcbSet::new(baseline).expect("baseline");
        let cause = IncompleteCause::SelectedOpaqueProvider {
            provider_identity: ProviderIdentity::NominalType("OpaqueCodec".into()),
            provider_plan_identity: 92,
            method: "decode".into(),
            requirement_identity: "Codec::decode".into(),
            binding: OpaqueInProcessBinding::StringBackedImportBootstrap {
                library: "codec".into(),
                symbol: "decode".into(),
            },
        };
        set.admit_era(
            10,
            accepted(
                "era-open",
                Vec::new(),
                ScopeCompleteness::Incomplete {
                    scope: ExecutionScope::CallerAddressSpace,
                    causes: vec![cause.clone()],
                    opaque_closure_evidence: Vec::new(),
                    runtime_closure_evidence: Vec::new(),
                },
            ),
        )
        .expect("marked incomplete era");

        let report = set.live_report();
        assert!(matches!(
            report.completeness(),
            CoexistingScopeCompleteness::Incomplete { sources, .. }
                if sources.len() == 2
                    && matches!(
                        &sources[1],
                        AttributedManifestCompleteness {
                            source: ExecutableManifestSource::ComponentEra(10),
                            completeness: ScopeCompleteness::Incomplete { causes, .. },
                            ..
                        } if causes == &vec![cause]
                    )
        ));
    }

    #[test]
    fn live_containment_is_the_intersection_of_all_contributing_rows() {
        let baseline = accepted(
            "platform",
            vec![entry(
                "SharedGateway",
                vec![
                    evidence(ContainmentGuarantee::MemoryIsolation, "memory:baseline"),
                    evidence(ContainmentGuarantee::FaultContainment, "fault:baseline"),
                ],
            )],
            complete(1),
        );
        let mut set = CoexistingExecutableTcbSet::new(baseline).expect("baseline");
        set.admit_era(
            10,
            accepted(
                "era",
                vec![entry(
                    "SharedGateway",
                    vec![evidence(
                        ContainmentGuarantee::MemoryIsolation,
                        "memory:era",
                    )],
                )],
                complete(10),
            ),
        )
        .expect("era");

        let report = set.live_report();
        let shared = &report.known_entries()[0];
        assert!(shared.has_universal_containment(ContainmentGuarantee::MemoryIsolation));
        assert_eq!(
            shared
                .universal_containment_evidence(ContainmentGuarantee::MemoryIsolation)
                .expect("both sources prove memory")
                .len(),
            2
        );
        assert!(!shared.has_universal_containment(ContainmentGuarantee::FaultContainment));
        assert_eq!(
            shared.universal_containment_evidence(ContainmentGuarantee::FaultContainment),
            None
        );
    }

    #[test]
    fn era_admission_rejects_zero_duplicate_and_scope_drift() {
        let baseline = accepted("platform", Vec::new(), complete(1));
        let mut set = CoexistingExecutableTcbSet::new(baseline).expect("baseline");
        let era = accepted("era", Vec::new(), complete(10));
        assert!(set.admit_era(0, era.clone()).is_err());
        set.admit_era(10, era.clone()).expect("first era identity");
        assert!(set.admit_era(10, era).is_err());

        let isolated_manifest = ExecutableTcbManifest {
            known_entries: Vec::new(),
            completeness: ScopeCompleteness::Complete {
                scope: ExecutionScope::IsolatedProvider(77),
                selected_provider_closure_identity: 77,
                opaque_closure_evidence: Vec::new(),
                runtime_closure_evidence: Vec::new(),
            },
        };
        let isolated_profile = ExecutableTcbProfile {
            name: "isolated".into(),
            scope: ExecutionScope::IsolatedProvider(77),
            allow_static_current_artifact_checked_bodies: true,
            exact_allowances: Vec::new(),
            incomplete_scope: IncompleteScopePolicy::Reject,
        };
        let isolated = evaluate_executable_tcb_profile(&isolated_manifest, &isolated_profile)
            .expect("isolated acceptance");
        assert!(
            set.admit_era(20, isolated)
                .expect_err("scope drift")
                .contains("does not match coexistence scope")
        );
    }
}
