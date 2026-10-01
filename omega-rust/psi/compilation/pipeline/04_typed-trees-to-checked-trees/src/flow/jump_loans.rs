//! Receiver footprints under individually forwarded argument loans.
//!
//! Returning calls retain every incoming loan through the target's whole run.
//! A sibling-state jump instead closes the source scope: only the loans its
//! actuals carry survive. Forwarding an independent reference must not prolong
//! a dropped loan. Each summary therefore has one held footprint per incoming
//! reference-bearing parameter, joined along formal/actual correspondence.
//! This is a monotone relation over parameters, not a powerset of combinations.
//!
//! Parameter identity and structural places come from the existing call-place
//! projection. Plain owned arguments carry no loans. An unresolved origin
//! conservatively retains every possible incoming loan; it never proves release.
//! Whole-run mutation summaries and independent loan-closure checks are unchanged.
//! Multiple reference leaves within one parameter currently share its footprint;
//! dropping just one such leaf can therefore still be conservatively rejected.

use crate::flow::mutation::{WritePlaceNamespace, instantiate_call_relative_places};
use crate::flow::normalize_attached_place_root;
use crate::flow::{CanonicalPlace, canonical_place_segments_may_overlap};
use crate::semantic::calls::{CallSite, find_call_site, find_state_in_machine};
use checked_trees::{BorrowCallFact, BorrowFacts};
use facts::PlaceRoot;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct HeldAccesses<Access> {
    pub(super) parameter: SymbolHandle,
    pub(super) accesses: Vec<Access>,
}

pub(super) fn held_accesses<Access: Clone>(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    direct: &[Access],
) -> Vec<HeldAccesses<Access>> {
    loan_parameters(program, state)
        .map(|parameter| HeldAccesses {
            parameter,
            accesses: direct.to_vec(),
        })
        .collect()
}

fn loan_parameters<'program>(
    program: &'program TypedTrees,
    state: &typed_trees::state::State,
) -> impl Iterator<Item = SymbolHandle> + 'program {
    program
        .state_parameters(state)
        .iter()
        .filter_map(|parameter| {
            (!parameter.is_self
                && !validation::has_plain_owned_contents_with_numeric_constraints(
                    program,
                    parameter.type_reference,
                ))
            .then_some(parameter.symbol)
        })
}

pub(super) enum TargetRun {
    Whole,
    Jump(Vec<ParameterLoanSources>),
}

pub(super) struct ParameterLoanSources {
    parameter: SymbolHandle,
    /// None is an unresolved source, not absence of a loan.
    sources: Option<Vec<CanonicalPlace>>,
}

impl TargetRun {
    pub(super) fn accesses<'access, Access>(
        &'access self,
        program: &'access TypedTrees,
        whole: &'access [Access],
        held: &'access [HeldAccesses<Access>],
        loan: Option<&'access CanonicalPlace>,
    ) -> impl Iterator<Item = &'access Access> {
        let whole_run = matches!(self, Self::Whole) || loan.is_none();
        let whole = if whole_run { whole } else { &[] };
        whole.iter().chain(
            held.iter()
                .filter(move |entry| {
                    !whole_run
                        && loan.is_some_and(|loan| self.forwards(program, entry.parameter, loan))
                })
                .flat_map(|entry| &entry.accesses),
        )
    }

    fn forwards(
        &self,
        program: &TypedTrees,
        parameter: SymbolHandle,
        loan: &CanonicalPlace,
    ) -> bool {
        let Self::Jump(parameters) = self else {
            return true;
        };
        let Some(binding) = parameters
            .iter()
            .find(|binding| binding.parameter == parameter)
        else {
            return true;
        };
        binding.sources.as_ref().is_none_or(|sources| {
            sources.iter().any(|source| {
                if !matches!(loan.root, PlaceRoot::Symbol(symbol) if symbol.is_valid()) {
                    return true;
                }
                source.root == crate::flow::normalized_event_place_root(program, loan.root)
                    && canonical_place_segments_may_overlap(
                        program,
                        &source.segments,
                        &loan.segments,
                    )
            })
        })
    }
}

pub(super) fn target_run(
    program: &TypedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    borrow: &BorrowFacts,
    call: &BorrowCallFact,
) -> TargetRun {
    let Some(target) = find_state_in_machine(program, machine_symbol, call.target_symbol) else {
        return TargetRun::Whole;
    };
    if !matches!(
        find_call_site(
            program,
            machine_symbol,
            state_symbol,
            call.statement_index,
            call.call_ordinal
        ),
        Some(CallSite::TransitionNamed { .. })
    ) {
        return TargetRun::Whole;
    }
    let caller = find_state_in_machine(program, machine_symbol, state_symbol);
    TargetRun::Jump(
        loan_parameters(program, target)
            .map(|parameter| {
                let sources = instantiate_call_relative_places(
                    program,
                    machine_symbol,
                    state_symbol,
                    borrow,
                    call,
                    &CanonicalPlace {
                        root: PlaceRoot::Symbol(parameter),
                        segments: Vec::new(),
                    },
                    WritePlaceNamespace::AccessRoute,
                    None,
                )
                .and_then(|mut sources| {
                    for source in &mut sources {
                        if !matches!(source.root, PlaceRoot::Symbol(root) if root.is_valid()) {
                            return None;
                        }
                        normalize_attached_place_root(
                            program,
                            machine_symbol,
                            state_symbol,
                            source,
                        );
                        source.root =
                            crate::flow::normalized_event_place_root(program, source.root);
                        let PlaceRoot::Symbol(root) = source.root else {
                            return None;
                        };
                        // Local aliases and reference-bearing constructions need their
                        // complete origin relation. Until available, retain rather than
                        // mistaking a fresh local name for an independent loan.
                        if !root.is_valid()
                            || (root != machine_symbol
                                && !caller.is_some_and(|state| {
                                    program
                                        .state_parameters(state)
                                        .iter()
                                        .any(|parameter| parameter.symbol == root)
                                }))
                        {
                            return None;
                        }
                    }
                    Some(sources)
                });
                ParameterLoanSources { parameter, sources }
            })
            .collect(),
    )
}
