//! Authored declaration selections carried from resolved source through typing and lowering.
#![forbid(unsafe_code)]

pub mod build_stage;
pub mod declaration_selection;
mod domain_contracts;
mod machine_contracts;
pub use domain_contracts::{DomainEstablishmentRoute, DomainSemanticRole, DomainSemanticRoles};
pub use machine_contracts::{
    MachineSupplyMode, MachineTerminationPlan, ProgressPremise, ProgressSubject, RankRange,
    RankingViewSelection, RankingWitness, TerminationGuarantee, TerminationInterface,
};
pub mod source_selections;
