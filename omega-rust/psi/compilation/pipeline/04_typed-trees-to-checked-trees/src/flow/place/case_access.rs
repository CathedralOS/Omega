//! Case payload access consumes live tag evidence for the exact saved subject.
//! Conditional field contracts may move with a whole sum; extracting a payload
//! must discharge every case crossed by the source place. Prepare observations
//! once for a batch, so each conditional membership does not rescan all facts
//! and resolve the same guard operations. The batch owns scratch observations
//! from one immutable scope; it never survives a write, join or context change.

use facts::{FactContextHandle, FactPlan, PlaceRoot, PlaceSegment};
use symbols::SymbolHandle;
use validation::CanonicalPlace;

use super::canonical_place_from_semantic_place;
mod observations;
use observations::CaseObservation;

pub(crate) struct CaseEvidence<'program> {
    program: &'program dyn validation::CheckingInput,
    semantic: &'program FactPlan,
    contexts: Vec<FactContextHandle>,
    machine: &'program typed_trees::machine::Machine,
    state: &'program typed_trees::state::State,
    statement_index: usize,
    observations: Vec<CaseObservation>,
    indexed_observations: Vec<usize>,
}

/// Coordinates prepared for repeated variant queries in one immutable evidence
/// batch. This is local query scratch, never retained across a context change.
struct PreparedCaseSubject {
    place: CanonicalPlace,
    exact_start: usize,
    indexed_start: usize,
}

impl<'program> CaseEvidence<'program> {
    pub(crate) fn prepare(
        program: &'program dyn validation::CheckingInput,
        semantic: &'program FactPlan,
        contexts: &[FactContextHandle],
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        statement_index: usize,
    ) -> Option<Self> {
        let machine = program.machine_by_symbol(machine_symbol)?;
        let state = program.state_in_machine(machine, state_symbol)?;
        let observations =
            observations::collect(program, semantic, contexts, machine, state, statement_index);
        // Only unresolved index expressions need semantic coordinate matching.
        // Retain their positions in the already sorted observation batch.
        let indexed_observations = observations
            .iter()
            .enumerate()
            .filter_map(|(position, row)| observations::has_index(&row.place).then_some(position))
            .collect();
        Some(Self {
            program,
            semantic,
            contexts: contexts.to_vec(),
            machine,
            state,
            statement_index,
            observations,
            indexed_observations,
        })
    }

    pub(crate) fn cases_are_selected(&self, subject: &CanonicalPlace) -> bool {
        self.cases_are_selected_after(subject, 0)
    }

    fn cases_are_selected_after(&self, subject: &CanonicalPlace, prefix_length: usize) -> bool {
        if prefix_length > subject.segments.len() {
            return false;
        }
        subject
            .segments
            .iter()
            .enumerate()
            .skip(prefix_length)
            .all(|(position, segment)| {
                let PlaceSegment::Case { variant } = segment else {
                    return true;
                };
                self.has_value(
                    &CanonicalPlace {
                        root: subject.root,
                        segments: subject.segments[..position].to_vec(),
                    },
                    *variant,
                    true,
                )
            })
    }

    pub(crate) fn path_is_inactive(&self, subject: &CanonicalPlace) -> bool {
        subject
            .segments
            .iter()
            .enumerate()
            .any(|(position, segment)| {
                let PlaceSegment::Case { variant } = segment else {
                    return false;
                };
                self.has_value(
                    &CanonicalPlace {
                        root: subject.root,
                        segments: subject.segments[..position].to_vec(),
                    },
                    *variant,
                    false,
                )
            })
    }

    /// Exact owned payload namespaces excluded in this immutable scope. The
    /// same batch supplies linear availability and affine residual geometry;
    /// a saved reference descriptor never supplies ownership of its referent.
    pub(crate) fn inactive_owned_cases(&self) -> Vec<CanonicalPlace> {
        let mut inactive = Vec::new();
        let mut previous = None;
        let mut previous_root = None;
        let mut root_is_plain_without_cleanup = false;
        let mut subject_cleanup_types = Vec::new();
        for observation in &self.observations {
            let subject = &observation.place;
            if previous_root != Some(&subject.root) {
                // A nested plain field can still serve an ancestor's hook or
                // loan cleanup. Authenticate the original whole root first.
                let root = CanonicalPlace {
                    root: subject.root,
                    segments: Vec::new(),
                };
                root_is_plain_without_cleanup = validation::canonical_owned_place_type_reference(
                    self.program,
                    self.state.symbol,
                    self.statement_index,
                    &root,
                )
                .is_some_and(|reference| {
                    validation::has_plain_contents_without_nominal_cleanup(self.program, reference)
                });
                previous_root = Some(&subject.root);
            }
            if previous == Some(subject) {
                continue;
            }
            previous = Some(subject);
            if !stable_subject(
                self.program,
                self.machine,
                self.state,
                self.statement_index,
                subject,
            ) {
                continue;
            }
            let Some(reference) = validation::canonical_owned_place_type_reference(
                self.program,
                self.state.symbol,
                self.statement_index,
                subject,
            ) else {
                continue;
            };
            if root_is_plain_without_cleanup {
                // A whole affine claim stays outside its case namespaces.
                // Only complete ordinary copy payloads make those namespaces
                // irrelevant. Reuse exact original type answers at this one
                // immutable cut, never across writes or joins.
                let no_case_cleanup = if let Some((_, answer)) = subject_cleanup_types
                    .iter()
                    .find(|(retained, _)| *retained == reference)
                {
                    *answer
                } else {
                    let answer =
                        validation::case_payloads_have_no_owned_cleanup(self.program, reference);
                    subject_cleanup_types.push((reference, answer));
                    answer
                };
                if no_case_cleanup {
                    continue;
                }
            }
            let reference = validation::peel_declared_constraints(self.program, reference);
            let Some(data) = self.program.closed_data_definition(reference) else {
                continue;
            };
            let prepared = self.prepare_stable_subject(subject);
            for member in self.program.data_members(data) {
                let typed_trees::data::DataMember::Variant(variant) = member else {
                    continue;
                };
                if self.has_value_for_subject(&prepared, variant.symbol, false) {
                    let mut path = subject.clone();
                    path.segments.push(PlaceSegment::Case {
                        variant: variant.symbol,
                    });
                    inactive.push(path);
                }
            }
        }
        inactive
    }

    fn has_value(&self, subject: &CanonicalPlace, case: SymbolHandle, required: bool) -> bool {
        if !stable_subject(
            self.program,
            self.machine,
            self.state,
            self.statement_index,
            subject,
        ) || !case.is_valid()
            || self.program.symbols.get(case).kind != symbols::SymbolKind::Variant
        {
            return false;
        }
        let prepared = self.prepare_stable_subject(subject);
        self.has_value_for_subject(&prepared, case, required)
    }

    /// The caller authenticated stability at this evidence batch's exact cut.
    /// Only the variant changes inside `inactive_owned_cases`, so normalize the
    /// subject and locate its observation ranges once before querying variants.
    fn prepare_stable_subject(&self, subject: &CanonicalPlace) -> PreparedCaseSubject {
        let mut subject = subject.clone();
        observations::normalize_place(
            self.program,
            self.semantic,
            &self.contexts,
            self.state.symbol,
            self.statement_index,
            &mut subject,
        );
        let exact_start = self
            .observations
            .partition_point(|row| row.place.compare_identity(&subject).is_lt());
        // Fixed coordinates use the full-place binary range. An unresolved
        // immutable index can additionally match a different expression handle,
        // but never a fixed index after shared constant/pinned normalization.
        // Search only the prepared unresolved-index roster for its exact root.
        let indexed_start = if observations::has_index(&subject) {
            let root = CanonicalPlace {
                root: subject.root,
                segments: Vec::new(),
            };
            self.indexed_observations.partition_point(|position| {
                CanonicalPlace {
                    root: self.observations[*position].place.root,
                    segments: Vec::new(),
                }
                .compare_identity(&root)
                .is_lt()
            })
        } else {
            self.indexed_observations.len()
        };
        PreparedCaseSubject {
            place: subject,
            exact_start,
            indexed_start,
        }
    }

    fn has_value_for_subject(
        &self,
        prepared: &PreparedCaseSubject,
        case: SymbolHandle,
        required: bool,
    ) -> bool {
        if !case.is_valid() || self.program.symbols.get(case).kind != symbols::SymbolKind::Variant {
            return false;
        }
        let subject = &prepared.place;
        let has_case_evidence = |case: SymbolHandle, required: bool| {
            self.observations[prepared.exact_start..]
                .iter()
                .take_while(|row| row.place == *subject)
                .chain(
                    self.indexed_observations[prepared.indexed_start..]
                        .iter()
                        .map(|position| &self.observations[*position])
                        .take_while(|row| row.place.root == subject.root)
                        .filter(|row| {
                            row.place != *subject
                                && super::canonical_places_agree(self.program, &row.place, subject)
                        }),
                )
                .any(|row| {
                    self.program.symbols.get(row.variant).parent
                        == self.program.symbols.get(case).parent
                        && if row.selected {
                            (row.variant == case) == required
                        } else {
                            row.variant == case && !required
                        }
                })
        };
        if has_case_evidence(case, required) {
            return true;
        }
        if !required || self.program.symbols.get(case).kind != symbols::SymbolKind::Variant {
            return false;
        }
        let Some(definition) = self
            .program
            .data_definition_by_symbol(self.program.symbols.get(case).parent)
        else {
            return false;
        };
        // Absence supplies no exclusion. Every other declared alternative must
        // be explicitly excluded at this exact live place before closing a sum.
        let mut declared = false;
        let mut excluded_alternative = false;
        for member in self.program.data_members(definition) {
            let typed_trees::data::DataMember::Variant(alternative) = member else {
                continue;
            };
            if alternative.symbol == case {
                declared = true;
            } else {
                if !has_case_evidence(alternative.symbol, false) {
                    return false;
                }
                excluded_alternative = true;
            }
        }
        declared && excluded_alternative
    }
}

pub(crate) fn place_cases_are_selected(
    program: &dyn validation::CheckingInput,
    semantic: &FactPlan,
    contexts: &[FactContextHandle],
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    subject: &CanonicalPlace,
) -> bool {
    place_cases_are_selected_after(
        program,
        semantic,
        contexts,
        machine_symbol,
        state_symbol,
        statement_index,
        subject,
        0,
    )
}

/// A saved descriptor authenticates only its own namespace prefix. Case
/// segments reached below that descriptor still require live tag evidence.
#[allow(clippy::too_many_arguments)]
pub(crate) fn place_cases_are_selected_after(
    program: &dyn validation::CheckingInput,
    semantic: &FactPlan,
    contexts: &[FactContextHandle],
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    subject: &CanonicalPlace,
    prefix_length: usize,
) -> bool {
    if prefix_length > subject.segments.len() {
        return false;
    }
    if !subject.segments[prefix_length..]
        .iter()
        .any(|segment| matches!(segment, PlaceSegment::Case { .. }))
    {
        return true;
    }
    CaseEvidence::prepare(
        program,
        semantic,
        contexts,
        machine_symbol,
        state_symbol,
        statement_index,
    )
    .is_some_and(|evidence| evidence.cases_are_selected_after(subject, prefix_length))
}

/// A whole value owes a payload invariant only when its enclosing cases are
/// active. Contrary tag evidence discharges the conditional obligation, but
/// never authorizes reading that payload.
pub(crate) fn place_case_path_is_inactive(
    program: &dyn validation::CheckingInput,
    semantic: &FactPlan,
    contexts: &[FactContextHandle],
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    subject: &CanonicalPlace,
) -> bool {
    if !subject
        .segments
        .iter()
        .any(|segment| matches!(segment, PlaceSegment::Case { .. }))
    {
        return false;
    }
    CaseEvidence::prepare(
        program,
        semantic,
        contexts,
        machine_symbol,
        state_symbol,
        statement_index,
    )
    .is_some_and(|evidence| evidence.path_is_inactive(subject))
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn place_case_has_value(
    program: &dyn validation::CheckingInput,
    semantic: &FactPlan,
    contexts: &[FactContextHandle],
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    subject: &CanonicalPlace,
    case: SymbolHandle,
    required: bool,
) -> bool {
    CaseEvidence::prepare(
        program,
        semantic,
        contexts,
        machine_symbol,
        state_symbol,
        statement_index,
    )
    .is_some_and(|evidence| evidence.has_value(subject, case, required))
}

pub(crate) fn stable_subject(
    program: &dyn validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    subject: &CanonicalPlace,
) -> bool {
    // An evaluated temporary is an identity too. Live facts name its exact
    // occurrence; matching them neither replays its producer nor transfers a
    // selected tag to another equal-looking call or constructor.
    (matches!(subject.root, PlaceRoot::Symbol(symbol) if symbol.is_valid())
        || matches!(subject.root, PlaceRoot::Expression(expression) if expression.is_valid()))
        && subject.segments.iter().all(|segment| {
            matches!(segment, PlaceSegment::Field { symbol } if symbol.is_valid())
                || matches!(segment, PlaceSegment::Case { variant } if variant.is_valid())
                || matches!(segment, PlaceSegment::FixedIndex { .. })
                || matches!(segment, PlaceSegment::Index { expression }
                if validation::place_has_immutable_denotation(
                    program, machine, state, statement_index, *expression,
                ))
        })
}
