//! Checked build result: normalized settings and independently checked selections.

use crate::{RootBinding, WireCompatibilityDemand};
use effects::AuthoredBehaviorExclusion;
use package_compilation::build_execution::build_value::EvaluatedBuildSettings;
use provider_planning::ProviderSelection;
use representation_planning::OpaqueRepresentationSelection;
use trust_model::AuthoredComponentAssumptionAcceptance;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BuildConfig {
    pub settings: EvaluatedBuildSettings,
    /// CH10 ROOT GRANTS (GR3): the symbol paths the final build accepted
    /// via `b.accept_boundary<pkg::symbol>();` -- harvested STATICALLY
    /// from the build machine's marker calls (grants are declarations,
    /// not runtime effects; the evaluator serves the marker as a no-op).
    pub grants: Vec<trust_model::AuthoredRootGrant>,
    /// PRV4c: explicit provider-type choices for boundary slots. These are
    /// declarations harvested from the authoritative build machine; they are
    /// validated against derived candidates before selection grants anything.
    pub provider_selections: Vec<ProviderSelection>,
    /// Exact named conformances activated as physical carriers for
    /// boundary-opaque values. The compiler derives shape from each carrier.
    pub opaque_representation_selections: Vec<OpaqueRepresentationSelection>,
    /// Channel/store compatibility demands. Each marker names the
    /// edge, format lineage, local and peer schemas, and the directional facts
    /// the final build requires.
    pub wire_compatibility_demands: Vec<WireCompatibilityDemand>,
    /// Target-owned inbound root slots bound by the authoritative build
    /// machine. The binding names an exact source machine; no entry discovery
    /// or naming convention participates once a binding is present.
    pub root_bindings: Vec<RootBinding>,
    /// Behavior exclusions (wiki/spec/build/behavior_exclusions.md): the
    /// authored `builder.exclude_crash(...)` product-admission requirements
    /// harvested statically from the root build machine's checked call scope,
    /// each retaining its exact toolchain case identity and authored span.
    /// These rows declare what the selected executable composition must not
    /// reach; the admission join itself consumes them later.
    pub behavior_exclusions: Vec<AuthoredBehaviorExclusion>,
    /// Accepted component-assumption digests
    /// (wiki/spec/build/component_publication.md): the authored
    /// `builder.accept_component_assumption(...)` declarations harvested
    /// statically from the root build machine's checked call scope. An
    /// attached independent-component description that binds an inseparable
    /// assumption verifies only when its exact digest appears here; the
    /// roster carries no accepted digest until the build authors one.
    pub accepted_component_assumptions: Vec<AuthoredComponentAssumptionAcceptance>,
}
