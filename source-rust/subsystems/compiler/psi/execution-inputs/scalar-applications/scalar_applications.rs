//! Check observations and prerequisite, entry, ABI and product scopes for published Psi scalar applications.
#![forbid(unsafe_code)]
mod abi_storage;
pub use abi_storage::CallingAbiStorageDemand;

mod observations;
pub use observations::ScalarLayoutObservationError;

mod conditions;
pub use conditions::ScalarConditionSelection;
mod entry_checks;
pub use entry_checks::{AdmittedScalarEntry, ScalarEntryChecks, ScalarEntryPrerequisites};
mod completion;
pub use completion::{AdmittedScalarApplications, ScalarApplicationClosure, ScalarScopeInputs};
#[cfg(test)]
mod completion_tests;
use arena::Arena;
use psi_provider_selections::AdmittedProviderSelections;
use psi_target_layouts::{AdmittedTargetLayouts, TerminalTargetContext};
use semantic_vocabulary::{ScalarApplicationId, ValueId};
use terminal_semantics::entry_values::TerminalScalarValue;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) enum ScalarConditionScope {
    #[default]
    Source,
    SourceRoots(Vec<terminal_psi::ProofDeclarationId>),
    Invocation(Vec<terminal_psi::ProofDeclarationId>),
    AbiStorage(AbiStorageScope),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AbiStorageScope {
    declaration: semantic_vocabulary::CallingAbiIdentity,
    applications: Vec<ScalarApplicationId>,
}

impl From<&CallingAbiStorageDemand> for AbiStorageScope {
    fn from(demand: &CallingAbiStorageDemand) -> Self {
        Self {
            declaration: demand.declaration().clone(),
            applications: demand.applications().to_vec(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ApplicationObservations {
    value: Option<TerminalScalarValue>,
    results: Arena<EntryResult>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) enum EntryResult {
    #[default]
    Missing,
    Observed {
        result: ValueId,
        value: TerminalScalarValue,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct CompletedScalarConditions {
    ranges: Vec<usize>,
    predicates: Vec<usize>,
    completions: Vec<usize>,
}

impl CompletedScalarConditions {
    fn new(conditions: &ScalarConditionSelection<'_>) -> Self {
        fn sorted(positions: &[usize]) -> Vec<usize> {
            let mut positions = positions.to_vec();
            positions.sort_unstable();
            positions
        }
        Self {
            ranges: sorted(conditions.ranges()),
            predicates: sorted(conditions.predicates()),
            completions: sorted(conditions.completions()),
        }
    }
}

pub struct ScalarApplicationObservations<'artifact> {
    artifact: terminal_codec::TerminalExecutionView<'artifact>,
    targets: Option<&'artifact TerminalTargetContext>,
    providers: &'artifact AdmittedProviderSelections,
    layouts: Option<AdmittedTargetLayouts>,
    layout_target: Option<psi_target_layouts::LayoutTargetFacts>,
    carrier_layout: Option<semantic_vocabulary::TargetCarrierLayout>,
    applications: std::sync::Arc<Arena<ApplicationObservations>>,
    condition_scope: ScalarConditionScope,
    inherited: Option<completion::CompletedScalarScope>,
    reexecuted: Vec<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScalarApplicationValues(std::sync::Arc<Arena<ApplicationObservations>>);
