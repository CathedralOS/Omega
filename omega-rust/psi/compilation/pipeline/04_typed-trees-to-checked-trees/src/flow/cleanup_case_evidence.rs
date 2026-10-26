//! Case membership at one checked cleanup occurrence.
//!
//! Partial transfers can retire live-value contexts while the custody schedule
//! retains authenticated exclusions for that exact dying owner. Effects and
//! executable residual geometry must consume the same evidence. The schedule
//! intersects observations at joins and invalidates them on overlapping writes;
//! these rows are not inferred from absent claims or constructor spelling.
//! `RemainingCustody::exclude_absent_case` receives these paths from checked
//! permission-schedule case tests. That producer authenticates the complete
//! owned, typed prefix; this consumer checks occurrence/root correspondence.
//! Independently supplied or persisted rows need that same authentication.

use checked_trees::{CheckFacts, ContextualCleanupDisposition};
use facts::{FactContextHandle, PlaceSegment};
use std::cell::OnceCell;
use validation::{CanonicalPlace, CleanupCaseReachability};

type OccurrenceContexts = Vec<(usize, Vec<FactContextHandle>)>;

/// Invocation-local query state; it neither publishes nor strengthens facts.
pub(crate) struct CleanupCaseEvidence<'a> {
    program: &'a dyn validation::CheckingInput,
    facts: &'a CheckFacts,
    disposition: &'a ContextualCleanupDisposition,
    inactive: Vec<&'a [PlaceSegment]>,
    definite: Vec<&'a [PlaceSegment]>,
    contexts: OnceCell<Option<OccurrenceContexts>>,
}

impl<'a> CleanupCaseEvidence<'a> {
    pub(crate) fn new(
        program: &'a dyn validation::CheckingInput,
        facts: &'a CheckFacts,
        disposition: &'a ContextualCleanupDisposition,
    ) -> Option<Self> {
        let cleanup = &facts.contextual_cleanup;
        let inactive = cleanup
            .exclusions
            .span(disposition.inactive_cases)?
            .iter()
            .map(|place| {
                let path = cleanup.segments.span(place.segments)?;
                (place.root == disposition.root
                    && matches!(path.last(), Some(PlaceSegment::Case { .. })))
                .then_some(path)
            })
            .collect::<Option<Vec<_>>>()?;
        let excluded = cleanup.exclusions.span(disposition.exclusions)?;
        let definite = cleanup
            .exclusions
            .span(disposition.definite_exclusions)?
            .iter()
            .map(|place| {
                let path = cleanup.segments.span(place.segments)?;
                (place.root == disposition.root
                    && excluded.iter().any(|excluded| {
                        excluded.root == place.root
                            && cleanup.segments.span(excluded.segments) == Some(path)
                    }))
                .then_some(path)
            })
            .collect::<Option<Vec<_>>>()?;
        Some(Self {
            program,
            facts,
            disposition,
            inactive,
            definite,
            contexts: OnceCell::new(),
        })
    }

    /// Producer custody proves a real transfer on every edge where this
    /// case exists; an unknown exit tag alone supplies no such evidence.
    pub(crate) fn definitely_transferred(&self, path: &[PlaceSegment]) -> bool {
        self.definite.contains(&path)
    }

    pub(crate) fn contexts(&self) -> &Option<OccurrenceContexts> {
        self.contexts.get_or_init(|| {
            self.facts
                .flow
                .control
                .state(self.disposition.machine, self.disposition.state)
                .and_then(|state| {
                    crate::facts::contextual_cleanup::occurrence_contexts(
                        self.program,
                        self.facts,
                        state,
                        self.disposition,
                    )
                })
        })
    }

    fn excludes(&self, path: &[PlaceSegment]) -> bool {
        self.inactive
            .iter()
            .any(|inactive| path.starts_with(inactive))
    }

    fn proves_case(
        &self,
        subject: &CanonicalPlace,
        variant: symbols::SymbolHandle,
        required: bool,
    ) -> bool {
        self.contexts()
            .as_ref()
            .filter(|points| !points.is_empty())
            .is_some_and(|points| {
                points.iter().all(|(statement, contexts)| {
                    super::place_case_has_value(
                        self.program,
                        &self.facts.semantic,
                        contexts,
                        self.disposition.machine,
                        self.disposition.state,
                        *statement,
                        subject,
                        variant,
                        required,
                    )
                })
            })
    }

    pub(crate) fn reachability(&self, path: &[PlaceSegment]) -> Option<CleanupCaseReachability> {
        if !path
            .iter()
            .any(|segment| matches!(segment, PlaceSegment::Case { .. }))
        {
            return Some(CleanupCaseReachability::Selected);
        }
        if self.excludes(path) {
            return Some(CleanupCaseReachability::Inactive);
        }
        let subject = CanonicalPlace {
            root: self.disposition.root,
            segments: path.to_vec(),
        };
        if self
            .contexts()
            .as_ref()
            .filter(|points| !points.is_empty())
            .is_some_and(|points| {
                points.iter().all(|(statement, contexts)| {
                    super::place_case_path_is_inactive(
                        self.program,
                        &self.facts.semantic,
                        contexts,
                        self.disposition.machine,
                        self.disposition.state,
                        *statement,
                        &subject,
                    )
                })
            })
        {
            return Some(CleanupCaseReachability::Inactive);
        }
        let statement = match self.disposition.source {
            language_semantics::PermissionEventSource::Call {
                statement_index, ..
            }
            | language_semantics::PermissionEventSource::Statement { statement_index } => {
                statement_index
            }
            _ => self.disposition.exit.statement_index,
        };
        for (position, segment) in path.iter().enumerate() {
            let PlaceSegment::Case { variant } = segment else {
                continue;
            };
            let subject = CanonicalPlace {
                root: self.disposition.root,
                segments: path[..position].to_vec(),
            };
            if self.proves_case(&subject, *variant, true) {
                continue;
            }
            let reference = validation::canonical_place_type_reference(
                self.program,
                self.disposition.state,
                statement,
                &subject,
            )?;
            let owner =
                self.program
                    .closed_data_definition(validation::unwrapped_type_reference(
                        self.program,
                        reference,
                    )?)?;
            let mut declared = false;
            let mut has_alternative = false;
            // An established finite sum selects this case only if every other
            // declared case is explicitly excluded at this same typed prefix.
            // A missing context, moved claim or equal-looking sibling is never
            // negative membership evidence.
            for member in self.program.data_members(owner) {
                let typed_trees::data::DataMember::Variant(alternative) = member else {
                    continue;
                };
                if alternative.symbol == *variant {
                    declared = true;
                    continue;
                }
                has_alternative = true;
                let mut excluded = subject.segments.clone();
                excluded.push(PlaceSegment::Case {
                    variant: alternative.symbol,
                });
                if !self.excludes(&excluded)
                    && !self.proves_case(&subject, alternative.symbol, false)
                {
                    return Some(CleanupCaseReachability::Runtime);
                }
            }
            if !declared {
                return None;
            }
            if !has_alternative {
                return Some(CleanupCaseReachability::Runtime);
            }
        }
        Some(CleanupCaseReachability::Selected)
    }
}
