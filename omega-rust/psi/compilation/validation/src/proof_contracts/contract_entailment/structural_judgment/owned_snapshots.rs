//! Complete inputs captured by the normal-value owner. A snapshot is
//! mathematical identity, not permission to copy, move, or read its storage.
//! Borrowed inputs retain their exact referent and canonical storage version;
//! a partial write need not leave a reconstructible aggregate term. Scalar
//! observations remain separate and cannot manufacture that initialized home.

use super::storage_effects::{CapturedStorageCoordinate, StorageVersion};
use super::{StructuralJudge, StructuralTerm};
use crate::proof_contracts::contract_entailment::structural_prefix::{
    PrefixObservation, StructuralPrefixSite, StructuralPrefixVisitor, observe_runtime_state_prefix,
};
use crate::{CheckingInput, RankSubjectCoordinate};
use facts::PlaceRoot;
use symbols::SymbolHandle;
use typed_trees::{machine::Machine, state::State, types::TypeReferenceHandle};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CapturedOwnedSnapshot {
    actual_type: TypeReferenceHandle,
    // Abstract entry terms can use declaration-local variables. Their source
    // scope is part of identity even when two parameters share a spelling.
    source_scope: SymbolHandle,
    value: SnapshotValue,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum SnapshotValue {
    Completed(StructuralTerm),
    Borrowed {
        home: CapturedStorageCoordinate,
        version: StorageVersion,
        projection: Vec<facts::PlaceSegment>,
    },
}

impl CapturedOwnedSnapshot {
    pub(crate) fn actual_type(&self) -> TypeReferenceHandle {
        self.actual_type
    }

    pub(in crate::proof_contracts::contract_entailment) fn project(
        &self,
        program: &dyn CheckingInput,
        path: &[facts::PlaceSegment],
    ) -> Option<Self> {
        let value = match &self.value {
            SnapshotValue::Completed(value) => SnapshotValue::Completed(
                super::super::observed_projections::project_observed_value(
                    program,
                    self.actual_type,
                    value.clone(),
                    path,
                )?,
            ),
            SnapshotValue::Borrowed {
                home,
                version,
                projection,
            } => {
                // This identifies inline storage only. A stored reference or
                // runtime selector requires its own captured backing/selector
                // receipt; its declaration spelling cannot supply one here.
                let mut reference = self.actual_type;
                for segment in path {
                    if crate::reference_shell(program, reference).is_some()
                        || !matches!(
                            segment,
                            facts::PlaceSegment::Field { .. }
                                | facts::PlaceSegment::FixedIndex { .. }
                        )
                    {
                        return None;
                    }
                    reference = crate::project_type_reference_from_segments(
                        program,
                        reference,
                        std::slice::from_ref(segment),
                    )?;
                }
                if crate::reference_shell(program, reference).is_some() {
                    return None;
                }
                SnapshotValue::Borrowed {
                    home: home.clone(),
                    version: *version,
                    projection: projection.iter().chain(path).cloned().collect(),
                }
            }
        };
        Some(Self {
            actual_type: crate::project_type_reference_from_segments(
                program,
                self.actual_type,
                path,
            )?,
            source_scope: self.source_scope,
            value,
        })
    }

    fn project_call_input(
        &self,
        judge: &StructuralJudge<'_>,
        path: &[facts::PlaceSegment],
        site: crate::CallSite<'_>,
        caller: SymbolHandle,
        actuals: &[StructuralTerm],
    ) -> Option<Self> {
        let program = judge.program;
        let SnapshotValue::Completed(value) = &self.value else {
            return self.project(program, path);
        };
        let parameters = site.parameters(program, caller)?;
        if parameters.len() != actuals.len() {
            return None;
        }
        let target = site.target_symbol(program);
        let bindings = parameters
            .iter()
            .zip(actuals)
            .map(|(formal, value)| (formal.symbol, value.clone()))
            .collect::<Vec<_>>();
        Some(Self {
            actual_type: crate::project_type_reference_from_segments(
                program,
                self.actual_type,
                path,
            )?,
            source_scope: self.source_scope,
            value: SnapshotValue::Completed(
                super::super::observed_projections::project_observed_value_with_selectors(
                    program,
                    judge,
                    self.actual_type,
                    value.clone(),
                    path,
                    |selector| {
                        judge.bound_contract_term(
                            selector,
                            target,
                            parameters,
                            target,
                            &bindings,
                            &[],
                            None,
                            0,
                        )
                    },
                )?,
            ),
        })
    }

    pub(in crate::proof_contracts::contract_entailment) fn completed(
        judge: &StructuralJudge<'_>,
        source_scope: SymbolHandle,
        actual_type: TypeReferenceHandle,
        value: &StructuralTerm,
    ) -> Option<Self> {
        let actual_type = if crate::reference_shell(judge.program, actual_type).is_some() {
            let observed =
                crate::value_custody::expression_types::readable_observation_type_reference(
                    judge.program,
                    actual_type,
                )?;
            // Only the ledger's authenticated inline observation can supply
            // a referent snapshot. A descriptor, or a list of scalar reads,
            // does not establish this whole value or grant its ownership.
            let retained =
                super::super::structural_terms::owned_observation_type(judge.program, value)?;
            crate::declared_carriers_match(judge.program, observed, retained).then_some(())?;
            observed
        } else {
            actual_type
        };
        Some(Self {
            actual_type,
            source_scope,
            value: SnapshotValue::Completed(judge.owned_argument_observation(actual_type, value)?),
        })
    }

    fn borrowed_at_call(
        judge: &StructuralJudge<'_>,
        state: &State,
        subject: &crate::machine_calls::calls::InvocationEntrySubject,
    ) -> Option<Self> {
        crate::reference_shell(judge.program, subject.parameter_type)?
            .access
            .is_readable()
            .then_some(())?;
        judge
            .program
            .closed_data_definition(subject.type_reference)?;
        let [source] = subject.sources.as_slice() else {
            return None;
        };
        if source.invocation_output.is_some()
            || source.is_image_storage
            || !source.recasts.is_empty()
            || source.representation.is_some()
        {
            return None;
        }
        // The frame owns this exact current referent, independently of whether
        // its old contents survived. A contents-invalidation test here would
        // reject precisely the partial writes whose new input we must capture.
        // The initialized carrier is independent of the measure's scalar reads.
        // Equality of snapshots includes the one storage graph's version, so
        // a later alias write cannot revive the earlier complete input.
        Some(Self {
            actual_type: subject.type_reference,
            source_scope: state.symbol,
            value: SnapshotValue::Borrowed {
                home: judge.captured_reference_subject(subject, state)?,
                version: judge.storage_version?,
                projection: Vec::new(),
            },
        })
    }
}

/// Query the invocation's real entry ledger. The supplying prefix judge owns
/// initialization and inherited arrival values; this does not replay a body.
pub fn captured_entry_owned_snapshot(
    program: &dyn CheckingInput,
    machine: &Machine,
    state: &State,
    coordinate: &RankSubjectCoordinate,
) -> Option<CapturedOwnedSnapshot> {
    let parameter = program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.symbol == coordinate.parameter)?;
    let actual_type = crate::project_type_reference_from_segments(
        program,
        parameter.type_reference,
        &coordinate.path,
    )?;
    let mut capture = EntrySnapshot {
        state: state.symbol,
        coordinate,
        actual_type,
        value: None,
        failed: false,
    };
    observe_runtime_state_prefix(program, machine, state, &mut capture)?;
    (!capture.failed).then_some(capture.value).flatten()
}

struct EntrySnapshot<'a> {
    state: SymbolHandle,
    coordinate: &'a RankSubjectCoordinate,
    actual_type: TypeReferenceHandle,
    value: Option<CapturedOwnedSnapshot>,
    failed: bool,
}

/// Capture an actual at the ordered call-entry cut, after all operands and
/// before this invocation's guarantees. Distinct reached values stay unknown;
/// choosing a convenient predecessor would splice an invalid rank witness.
pub fn captured_call_owned_snapshot(
    program: &dyn CheckingInput,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    site: crate::CallSite<'_>,
    position: usize,
    path: &[facts::PlaceSegment],
) -> Option<CapturedOwnedSnapshot> {
    let mut capture = CallSnapshot {
        state: state.symbol,
        statement_index,
        site,
        position,
        path,
        value: None,
        failed: false,
    };
    observe_runtime_state_prefix(program, machine, state, &mut capture)?;
    (!capture.failed).then_some(capture.value).flatten()
}

struct CallSnapshot<'a> {
    state: SymbolHandle,
    statement_index: usize,
    site: crate::CallSite<'a>,
    position: usize,
    path: &'a [facts::PlaceSegment],
    value: Option<CapturedOwnedSnapshot>,
    failed: bool,
}

impl StructuralPrefixVisitor for CallSnapshot<'_> {
    fn site(&self) -> StructuralPrefixSite {
        StructuralPrefixSite {
            state: self.state,
            statement_index: self.statement_index,
            in_transition_target: true,
        }
    }
    fn unresolved(&mut self, _: &Machine) {
        self.failed = true;
    }
    fn is_complete(&self) -> bool {
        self.failed
    }
    fn observe<'program>(&mut self, observation: PrefixObservation<'_, 'program>) {
        use super::normal_returns::OperationOperandDemand;
        let (program, machine, state, judge, environment) = observation;
        let capture = (|| {
            let arguments = crate::call_site_argument_expressions(program, &self.site);
            let argument = *arguments.get(self.position)?;
            // Expression result typing can observe a referent implicitly. The
            // declared reference still owns the source-home join at this cut.
            let actual_type =
                crate::declared_place_type_raw(program, machine, Some(state), argument).or_else(
                    || crate::expression_result_type_reference(program, machine, state, argument),
                )?;
            let parameters = self.site.parameters(program, machine.symbol)?;
            let subjects = if crate::reference_shell(program, actual_type).is_some() {
                let frames = judge.frame_resolver()?;
                Some(match self.site {
                    crate::CallSite::Expression { expression, .. } => frames
                        .expression_entry_reference_subjects(machine, expression, parameters)?,
                    crate::CallSite::TransitionNamed { .. } => {
                        let target = crate::collect_call_sites_in_statement(
                            program,
                            machine,
                            state,
                            self.statement_index,
                        )
                        .iter()
                        .enumerate()
                        .filter_map(|(ordinal, _)| {
                            crate::transition_call_target(
                                program,
                                machine,
                                state,
                                self.statement_index,
                                ordinal,
                            )
                        })
                        .find(|target| self.site.completes_transition(program, *target))?;
                        frames.transition_entry_reference_subjects(machine, target, parameters)?
                    }
                    crate::CallSite::Statement(call) => {
                        frames.statement_entry_reference_subjects(machine, call, parameters)?
                    }
                })
            } else {
                None
            };
            let demand = if subjects.is_some() {
                OperationOperandDemand::Selected(&[])
            } else {
                OperationOperandDemand::All
            };
            let entries = match self.site {
                crate::CallSite::Expression { expression, .. } => judge.normal_operation_entries(
                    self.statement_index,
                    expression,
                    demand,
                    environment,
                    (machine, state, state.return_type),
                ),
                crate::CallSite::TransitionNamed { .. } => judge.normal_argument_entries(
                    arguments,
                    self.site.parameters(program, machine.symbol)?,
                    demand,
                    environment,
                    (machine, state, state.return_type),
                    Some(self.site),
                ),
                crate::CallSite::Statement(_) => return None,
            }?;
            if entries.is_empty() {
                return None;
            }
            for entry in entries {
                let snapshot = if let Some(subjects) = &subjects {
                    let formal = parameters.get(self.position)?;
                    let subject = subjects
                        .iter()
                        .find(|subject| subject.parameter == formal.symbol)?;
                    CapturedOwnedSnapshot::borrowed_at_call(&entry.judge, state, subject)?
                        .project(program, self.path)?
                } else {
                    let actuals = entry.observed_arguments()?;
                    let value = actuals.get(self.position)?;
                    CapturedOwnedSnapshot::completed(
                        &entry.judge,
                        state.symbol,
                        actual_type,
                        value,
                    )?
                    .project_call_input(
                        &entry.judge,
                        self.path,
                        self.site,
                        machine.symbol,
                        &actuals,
                    )?
                };
                if self
                    .value
                    .as_ref()
                    .is_some_and(|existing| existing != &snapshot)
                {
                    return None;
                }
                self.value = Some(snapshot);
            }
            Some(())
        })();
        if capture.is_none() {
            self.failed = true;
        }
    }
}

impl StructuralPrefixVisitor for EntrySnapshot<'_> {
    fn site(&self) -> StructuralPrefixSite {
        StructuralPrefixSite {
            state: self.state,
            statement_index: 0,
            in_transition_target: false,
        }
    }

    fn unresolved(&mut self, _: &Machine) {
        self.failed = true;
    }

    fn is_complete(&self) -> bool {
        self.failed
    }

    fn observe<'program>(&mut self, observation: PrefixObservation<'_, 'program>) {
        let (_, _, state, judge, _) = observation;
        let captured = (|| {
            let value = judge.captured_entry_place_projection(crate::CanonicalPlace {
                root: PlaceRoot::Symbol(self.coordinate.parameter),
                segments: self.coordinate.path.clone(),
            })?;
            CapturedOwnedSnapshot::completed(&judge, state.symbol, self.actual_type, &value)
        })();
        let Some(captured) = captured else {
            self.failed = true;
            return;
        };
        if self
            .value
            .as_ref()
            .is_some_and(|existing| existing != &captured)
        {
            self.failed = true;
        } else {
            self.value = Some(captured);
        }
    }
}
