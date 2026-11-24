//! Stage 05 supplies one lowered program to stage 06. The root closure, generic
//! forms, and conformance witnesses are checked together before publication.
#![forbid(unsafe_code)]

pub mod compilation;
pub mod mathematics;
pub mod names;
pub mod program;
mod tables;

use compilation::{
    CallableArrivalWitness, CallableFormationDefect, CallableFormationFailure, GenericForms,
};
use program::Elaboration;
use symbols::SymbolHandle;
pub use tables::*;

#[derive(Debug, Clone)]
pub enum TerminalCompilation {
    FormationFailures(Vec<CallableFormationFailure>),
    FormationDefect(CallableFormationDefect),
    Prepared(PreparedCompilation),
}

#[derive(Debug, Clone)]
pub struct PreparedCompilation {
    pub closure: Checked<Elaboration>,
    /// Every refusal reading met after the one `closure` holds, in the order
    /// it met them: declarations first, then bodies by `MachineId`. Each is
    /// a verdict of its own, so one compilation names every machine the
    /// checker cannot read yet. Empty whenever `closure` is `Ok`.
    pub further: Vec<crate::Refusal>,
    /// Proof-only calls from each installed source arrival to its continuation.
    /// These never become executable or publishable closures.
    pub arrivals: Vec<Checked<CallableArrivalWitness>>,
    pub forms: Vec<GenericForms>,
    pub witnesses: Vec<Checked<Elaboration>>,
    /// Exact generated machine to template correspondence, fixed by preparation.
    pub specializations: arena::IdTable<MachineId, Option<SymbolHandle>>,
    /// Root selection position to its independently judged conformance module.
    pub selection_witnesses: Vec<(usize, usize)>,
    /// Every mathematical declaration of the subject, as one kernel signature.
    pub mathematics: mathematics::MathematicalSignature,
}
