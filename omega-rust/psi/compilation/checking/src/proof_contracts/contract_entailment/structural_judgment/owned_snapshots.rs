//! Complete owned values captured by the normal-value owner. A snapshot is
//! mathematical identity, not permission to copy, move, or read its storage.
//! Scalar observations remain separate; they cannot manufacture this value.

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
    value: StructuralTerm,
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
        Some(Self {
            actual_type: crate::project_type_reference_from_segments(
                program,
                self.actual_type,
                path,
            )?,
            source_scope: self.source_scope,
            value: super::super::observed_projections::project_observed_value(
                program,
                self.actual_type,
                self.value.clone(),
                path,
            )?,
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
            value: super::super::observed_projections::project_observed_value_with_selectors(
                program,
                judge,
                self.actual_type,
                self.value.clone(),
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
        })
    }

    pub(in crate::proof_contracts::contract_entailment) fn completed(
        judge: &StructuralJudge<'_>,
        source_scope: SymbolHandle,
        actual_type: TypeReferenceHandle,
        value: &StructuralTerm,
    ) -> Option<Self> {
        Some(Self {
            actual_type,
            source_scope,
            value: judge.owned_argument_observation(actual_type, value)?,
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
            let actual_type =
                crate::expression_result_type_reference(program, machine, state, argument)?;
            let entries = match self.site {
                crate::CallSite::Expression { expression, .. } => judge.normal_operation_entries(
                    self.statement_index,
                    expression,
                    OperationOperandDemand::All,
                    environment,
                    (machine, state, state.return_type),
                ),
                crate::CallSite::TransitionNamed { .. } => judge.normal_argument_entries(
                    arguments,
                    self.site.parameters(program, machine.symbol)?,
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
                let actuals = entry.observed_arguments()?;
                let value = actuals.get(self.position)?;
                let snapshot = CapturedOwnedSnapshot::completed(
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
                )?;
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
