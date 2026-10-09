//! Provenance replay of selected provider plans against the typed program,
//! for package review. Review compiles a candidate package and then replays
//! the retained provenance of every authored selected plan before it projects
//! inert policy; the ordinary compile does not run this replay on the plans it
//! just derived (pipeline.md, "Verification placement"). Retained symbols
//! establish scope before any portable schema or binding comparison.

use crate::ProviderPlanningContext;
use crate::provider_planning::provenance_replay::requirement_identities::{
    external_provider_binding, inferred_compiler_leaf_binding, provider_boundary_arguments,
};
use crate::provider_planning::provenance_replay::{
    DerivedProviderPlan, ProviderPlanProvenance, ProviderSchemaDeclaration,
    SelectedTargetMachineOrigin, exact_checked_adapter,
};
use crate::provider_planning::{ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceSchema};

