//! The exhaustive match makes a new producer obligation visible to the report.
use omega_typed::ObligationKind;
use std::collections::BTreeMap;

pub fn counts(prepared: &omega_prepared::PreparedTrees<'_>) -> BTreeMap<&'static str, usize> {
    let mut counts = BTreeMap::new();
    for obligation in &prepared.typed().obligations {
        *counts.entry(name(&obligation.kind)).or_insert(0) += 1;
    }
    counts
}

macro_rules! obligation_names {
    ($($kind:ident $fields:tt),+ $(,)?) => {
        pub const KINDS: &[&str] = &[$(stringify!($kind)),+];
        pub fn name(value: &ObligationKind) -> &'static str {
            match value { $(ObligationKind::$kind $fields => stringify!($kind)),+ }
        }
    };
}

// The one unit variant also accepts the empty struct pattern.
obligation_names! {
    RefineRequirement {..}, EstablishDomain (..), RefineMathematicalCitation {..},
    RefineCallableValue {..}, RefineCallable {..}, PositiveWidth (..), FormRecursiveApplication {..},
    Termination (..), TotalCrashGuard {}, AdmitMathematicalAssumption (..), VerifyLibraryImplementation (..),
    ProveGuarantee {..}, FormSquash {..}, MathematicalFact {..}, UniverseMembership {..},
    SupplyHypothesis {..}, ApplyCallbackGuarantee {..}, ApplyHypothesis {..}, CleanupContract (..),
    EvaluateAccessPlan {..}, RefineEvidence {..}, CrashCoverage {..}, OperationContract {..},
    ExactCast {..}, EstablishData (..), ZeroValue (..), CopyRead (..), RestoreBorrowed {..},
    Move {..}, MathematicalApplication {..}, Recast {..}, Borrow {..}, ReturnBorrow {..},
    BindLifetime {..}, CallPrecondition (..), ZeroField {..},
}
