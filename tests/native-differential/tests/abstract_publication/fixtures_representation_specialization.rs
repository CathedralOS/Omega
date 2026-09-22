//! Representation-specialization projection fixture: a source-produced machine
//! whose `EstablishScalarCase` producer proves two membership observations.

use super::{VerifiedPsiOptimizationUnit, verified};
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use checked_trees_to_lowered_psi::lower_machine;

/// Two memberships observe one established place: `c in Choice::Some` folds
/// to `true` and `c in Choice::Empty` folds to `false` inside a single
/// candidate covering the place, so the pass commits exactly once.
const ESTABLISHED_MEMBERSHIPS_SOURCE: &str = r#"
    data Choice { case Empty; case Some(value: u32); }
    machine probe() -> bool {
        let c: Choice = Choice::Some { value: 37 };
        (c in Choice::Some) == (c in Choice::Empty)
    }
"#;

/// Two scalar field reads observe one established place: `p.x` folds to `37`
/// and `p.flag` folds to `true` inside a single candidate covering the place
/// under the `EstablishRecord` producer basis, so the field-value rule
/// commits exactly once.
const ESTABLISHED_FIELDS_SOURCE: &str = r#"
    data Point { x: u32; flag: bool; }
    machine probe() -> bool {
        let p: Point = Point { x: 37, flag: true };
        (p.x == 37) == p.flag
    }
"#;

/// A real source-produced membership machine: the full Psi front half lowers
/// `probe` to Terminal Psi, then the ordinary artifact admission builds the
/// verified optimization unit — no hand-constructed structural places.
pub(super) fn representation_specialization_membership_verified() -> VerifiedPsiOptimizationUnit {
    let checked = crate::front_end::checked_program(ESTABLISHED_MEMBERSHIPS_SOURCE);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("probe"))
        .expect("lower established memberships");
    verified(lowered.semantic_module, lowered.proof_bundle)
}

/// A real source-produced field-read machine: the full Psi front half lowers
/// `probe` to Terminal Psi, then the ordinary artifact admission builds the
/// verified optimization unit — no hand-constructed structural places.
pub(super) fn representation_specialization_field_value_verified() -> VerifiedPsiOptimizationUnit {
    let checked = crate::front_end::checked_program(ESTABLISHED_FIELDS_SOURCE);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("probe"))
        .expect("lower established field values");
    verified(lowered.semantic_module, lowered.proof_bundle)
}
