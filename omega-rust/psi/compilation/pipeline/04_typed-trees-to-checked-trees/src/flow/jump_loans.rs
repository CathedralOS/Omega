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

use validation::CanonicalPlace;
use validation::{CallSite, find_call_site, find_state_in_machine};

use crate::flow::canonical_place_segments_may_overlap;
use crate::flow::mutation::{WritePlaceNamespace, instantiate_call_relative_places};

use checked_trees::BorrowCallFact;
use facts::PlaceRoot;
use symbols::SymbolHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct HeldAccesses<Access> {
    pub(super) parameter: SymbolHandle,
    pub(super) accesses: Vec<Access>,
}

pub(super) fn held_accesses<Access: Clone>(
    program: &dyn ::validation::CheckingInput,
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
    program: &'program dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
) -> impl Iterator<Item = SymbolHandle> + 'program {
    program
        .state_parameters(state)
        .iter()
        .filter_map(|parameter| {
            (!validation::has_contents(
                program,
                parameter.type_reference,
                &[],
                validation::ContentsRequirement::NumericOwned,
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
    is_data_binding: bool,
    /// None is an unresolved source, not absence of a loan.
    sources: Option<Vec<CanonicalPlace>>,
}

impl TargetRun {
    pub(super) fn accesses<'access, Access>(
        &'access self,
        program: &'access dyn ::validation::CheckingInput,
        whole: &'access [Access],
        held: &'access [HeldAccesses<Access>],
        loan: Option<&'access CanonicalPlace>,
    ) -> impl Iterator<Item = &'access Access> {
        self.new_accesses(program, whole, 0, held, &[], loan)
    }

    pub(super) fn new_accesses<'access, Access>(
        &'access self,
        program: &'access dyn ::validation::CheckingInput,
        whole: &'access [Access],
        seen_whole: usize,
        held: &'access [HeldAccesses<Access>],
        seen_held: &'access [usize],
        loan: Option<&'access CanonicalPlace>,
    ) -> impl Iterator<Item = &'access Access> {
        let whole_run = matches!(self, Self::Whole) || loan.is_none();
        let whole = if whole_run {
            &whole[seen_whole.min(whole.len())..]
        } else {
            &[]
        };
        whole.iter().chain(
            held.iter()
                .enumerate()
                .filter(move |(_, entry)| {
                    !whole_run
                        && loan.is_some_and(|loan| self.forwards(program, entry.parameter, loan))
                })
                .flat_map(move |(idx, entry)| {
                    let start = seen_held.get(idx).copied().unwrap_or(0).min(entry.accesses.len());
                    &entry.accesses[start..]
                }),
        )
    }

    fn forwards(
        &self,
        program: &dyn ::validation::CheckingInput,
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
                source.root == loan.root
                    // Transporting a suspended parent does not also forward
                    // every descendant loan. The exact loan or a reborrow
                    // inside it still retains its held footprint, including
                    // when that carrier becomes the successor's data binding.
                    // Only a proved strict ancestor establishes this release;
                    // uncertain selector relationships remain conservative.
                    && !(binding.is_data_binding
                        && source.segments.len() < loan.segments.len()
                        && loan.segments.iter().all(|segment| {
                            !crate::flow::place_segment_has_unresolved_identity(*segment)
                        })
                        && source.segments.iter().zip(&loan.segments).all(
                            |(source_segment, loan_segment)| {
                                crate::flow::canonical_place_segments_equal(
                                    program,
                                    *source_segment,
                                    *loan_segment,
                                )
                            },
                        ))
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
    program: &dyn ::validation::CheckingInput,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    call: &BorrowCallFact,
) -> TargetRun {
    let Some(target) = find_state_in_machine(program, machine_symbol, call.target_symbol) else {
        return TargetRun::Whole;
    };
    let Some(CallSite::TransitionNamed { path, .. }) = find_call_site(
        program,
        machine_symbol,
        state_symbol,
        call.statement_index,
        call.call_ordinal,
    ) else {
        return TargetRun::Whole;
    };
    // The callee state's identity is a summary coordinate. A machine-named
    // invocation still holds incoming loans through completion, even when its
    // entry belongs to this same machine and native recursion uses a backedge.
    // Only an authored local-state transfer closes the source state scope.
    if program.machine_by_symbol(path.symbol).is_some() {
        return TargetRun::Whole;
    }
    let caller = find_state_in_machine(program, machine_symbol, state_symbol);
    let data_parameter = program
        .machine_by_symbol(machine_symbol)
        .and_then(|machine| {
            program.data_binding_parameter(machine, program.state_parameters(target))
        })
        .map(|parameter| parameter.symbol);
    TargetRun::Jump(
        loan_parameters(program, target)
            .map(|parameter| {
                let sources = instantiate_call_relative_places(
                    program,
                    machine_symbol,
                    state_symbol,
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
                        let root = source.root.as_symbol()?;
                        // Local aliases and reference-bearing constructions need their
                        // complete origin relation. Until available, retain rather than
                        // mistaking a fresh local name for an independent loan.
                        if !root.is_valid()
                            || (root != machine_symbol
                                && !caller.is_some_and(|state| {
                                    program.state_parameter_by_symbol(state, root).is_some()
                                }))
                        {
                            return None;
                        }
                    }
                    Some(sources)
                });
                ParameterLoanSources {
                    parameter,
                    is_data_binding: data_parameter == Some(parameter),
                    sources,
                }
            })
            .collect(),
    )
}
