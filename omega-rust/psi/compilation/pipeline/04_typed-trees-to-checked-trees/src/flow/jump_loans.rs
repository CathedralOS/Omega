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
    is_data_binding: bool,
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
    let data_parameter = crate::lookup::machine_by_symbol(program, machine_symbol)
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
                ParameterLoanSources {
                    parameter,
                    is_data_binding: data_parameter == Some(parameter),
                    sources,
                }
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::front_end::typed_program;
    use facts::PlaceSegment;

    fn places() -> (TypedTrees, CanonicalPlace, CanonicalPlace, CanonicalPlace) {
        let program = typed_program(
            "data Inner { tag: u64; }
             data Main { inner: Inner; }
             machine Main::run(main: &mut Main) {}",
        );
        let machine = &program.machines()[0];
        let state = &program.machine_states(machine)[0];
        let root = PlaceRoot::Symbol(program.state_parameters(state)[0].symbol);
        let field = |name| {
            program
                .data_definitions()
                .iter()
                .flat_map(|data| program.data_members(data))
                .find_map(|member| match member {
                    typed_trees::data::DataMember::Field(field) if field.name.as_str() == name => {
                        Some(PlaceSegment::Field {
                            symbol: field.symbol,
                        })
                    }
                    _ => None,
                })
                .expect("fixture field")
        };
        let inner = field("inner");
        let tag = field("tag");
        let parent = CanonicalPlace {
            root,
            segments: vec![],
        };
        let child = CanonicalPlace {
            root,
            segments: vec![inner],
        };
        let descendant = CanonicalPlace {
            root,
            segments: vec![inner, tag],
        };
        (program, parent, child, descendant)
    }

    fn forwards(
        program: &TypedTrees,
        loan: &CanonicalPlace,
        sources: Option<Vec<CanonicalPlace>>,
        is_data_binding: bool,
    ) -> bool {
        let PlaceRoot::Symbol(parameter) = loan.root else {
            panic!("fixture parameter")
        };
        TargetRun::Jump(vec![ParameterLoanSources {
            parameter,
            is_data_binding,
            sources,
        }])
        .forwards(program, parameter, loan)
    }

    #[test]
    fn data_parent_transport_releases_only_the_strict_child_loan() {
        let (program, parent, child, descendant) = places();
        assert!(!forwards(
            &program,
            &child,
            Some(vec![parent.clone()]),
            true
        ));
        // The exact loan can itself become the successor's data binding.
        assert!(forwards(&program, &child, Some(vec![child.clone()]), true));
        // A descendant reborrow still belongs to the incoming loan's cohort.
        assert!(forwards(&program, &child, Some(vec![descendant]), true));
        // An ordinary reference-bearing carrier keeps the conservative overlap law.
        assert!(forwards(&program, &child, Some(vec![parent]), false));
    }

    #[test]
    fn unresolved_data_origins_cannot_establish_child_release() {
        let (program, _, child, _) = places();
        assert!(forwards(&program, &child, None, true));
        let unknown = PlaceSegment::Field {
            symbol: SymbolHandle::invalid(),
        };
        let source = CanonicalPlace {
            root: child.root,
            segments: vec![unknown],
        };
        let loan = CanonicalPlace {
            root: child.root,
            segments: vec![unknown, child.segments[0]],
        };
        // Equal unresolved tokens are not proof that source is the parent.
        assert!(forwards(&program, &loan, Some(vec![source]), true));
    }
}
