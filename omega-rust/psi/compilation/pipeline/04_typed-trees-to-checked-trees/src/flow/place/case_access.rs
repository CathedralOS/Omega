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
    observations: Vec<CaseObservation>,
}

impl<'program> CaseEvidence<'program> {
    pub(crate) fn prepare(
        program: &'program dyn validation::CheckingInput,
        semantic: &FactPlan,
        contexts: &[FactContextHandle],
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        statement_index: usize,
    ) -> Option<Self> {
        let machine = program.machine_by_symbol(machine_symbol)?;
        let state = program.state_in_machine(machine, state_symbol)?;
        Some(Self {
            program,
            observations: observations::collect(
                program,
                semantic,
                contexts,
                machine,
                state,
                statement_index,
            ),
        })
    }

    pub(crate) fn cases_are_selected(&self, subject: &CanonicalPlace) -> bool {
        subject
            .segments
            .iter()
            .enumerate()
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

    fn has_value(&self, subject: &CanonicalPlace, case: SymbolHandle, required: bool) -> bool {
        if !stable_subject(subject)
            || !case.is_valid()
            || self.program.symbols.get(case).kind != symbols::SymbolKind::Variant
        {
            return false;
        }
        let start = self
            .observations
            .partition_point(|row| row.place.compare_identity(subject).is_lt());
        let has_case_evidence = |case: SymbolHandle, required: bool| {
            self.observations[start..]
                .iter()
                .take_while(|row| row.place == *subject)
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
    if !subject
        .segments
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
    .is_some_and(|evidence| evidence.cases_are_selected(subject))
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

pub(crate) fn stable_subject(subject: &CanonicalPlace) -> bool {
    // An evaluated temporary is an identity too. Matching its occurrence does
    // not replay its producer or transfer a tag to an equal-looking sibling.
    (matches!(subject.root, PlaceRoot::Symbol(symbol) if symbol.is_valid())
        || matches!(subject.root, PlaceRoot::Expression(expression) if expression.is_valid()))
        && subject.segments.iter().all(|segment| {
            matches!(segment, PlaceSegment::Field { symbol } if symbol.is_valid())
                || matches!(segment, PlaceSegment::Case { variant } if variant.is_valid())
                || matches!(segment, PlaceSegment::FixedIndex { .. })
        })
}
