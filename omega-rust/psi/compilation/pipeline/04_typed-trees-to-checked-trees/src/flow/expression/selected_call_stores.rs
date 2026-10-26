//! Resource footprints of an exact completed local selector. Dispatch and
//! guarantees remain public. Unknown values, differing alternatives and absent
//! checked bodies supply no relation; constructor inventory is never closure.

use super::Execution;
use arena::{Handle, HandleSpan};
use checked_trees::{
    BorrowCallFact, DynamicConformanceFacts, FlowControlFacts, FlowOperandFact, FlowStatementFact,
};
use facts::PlaceSegment;
use symbols::SymbolHandle;
use typed_trees::{machine::Machine, state::State, types::TypeReferenceHandle};

impl<'a, 'b, 'plans> Execution<'a, 'b, 'plans> {
    pub(super) fn selected_store_target<'inputs>(
        &mut self,
        call: &BorrowCallFact,
        actuals: &'inputs [FlowOperandFact],
    ) -> Option<(&'plans Machine, &'plans State, &'inputs [FlowOperandFact])> {
        if let Some(state) = validation::call_target_state(self.program, call.target_symbol) {
            return Some((
                self.program.machine_holding_state(state.symbol)?,
                state,
                actuals,
            ));
        }
        let slot = self.program.dynamic_slot_by_symbol(call.target_symbol)?;
        let prefix = slot
            .parameters
            .len()
            .checked_sub(slot.data_parameters().len())?;
        if prefix != 1 || actuals.len() != slot.parameters.len() {
            return None;
        }
        let selector = self.context.control.operands.insert(*actuals.first()?);
        let statements = super::super::appended_span_since(
            &self.context.control.statements,
            self.context.state_statements_start,
        );
        let implementation = completed_selection(
            self.program,
            &self.context.control,
            self.context.dynamic_conformances.as_ref()?,
            selector,
            slot.parameters.first()?.type_reference,
            Some((self.state.symbol, statements)),
        )?;
        let conformance = self.program.conformance_by_symbol(implementation)?;
        if !conformance.type_parameters.is_empty() {
            return None;
        }
        let mut rows = self
            .context
            .dynamic_conformances
            .as_ref()?
            .selections
            .iter()
            .filter(|selection| {
                selection.implementation == implementation
                    && selection.interface_identity == slot.interface_identity
            })
            .flat_map(|selection| &selection.rows)
            .filter(|row| {
                row.declaring_trait == slot.declaring_trait && row.requirement == slot.requirement
            });
        let row = rows.next()?;
        // Several constructor occurrences can authenticate the same immutable
        // row. A different row or finite family needs its own exact tuple proof.
        if !row.family_tuple.is_empty() || rows.any(|other| other != row) {
            return None;
        }
        let machine = self.program.machine_by_symbol(row.realization_machine)?;
        let state = self.program.state_by_symbol(row.realization_state)?;
        if !machine.body_is_present
            || !machine.supply_mode.is_checked_body()
            || machine.target_family_interface
            || self.program.machine_states(machine).first()?.symbol != state.symbol
            || !self.program.machine_type_parameters(machine).is_empty()
        {
            return None;
        }
        let parameters = self.program.state_parameters(state);
        let public = slot.data_parameters();
        if parameters.len() != public.len()
            || !parameters.iter().zip(public).all(|(private, public)| {
                private.is_const == public.is_const
                    && private.is_mutable == public.is_mutable
                    && validation::declared_carriers_match(
                        self.program,
                        private.type_reference,
                        public.type_reference,
                    )
            })
            || !validation::declared_carriers_match(
                self.program,
                state.return_type,
                slot.return_type,
            )
        {
            return None;
        }
        Some((machine, state, actuals.get(prefix..)?))
    }
}

fn completed_selection(
    program: &dyn validation::CheckingInput,
    control: &FlowControlFacts,
    receipts: &DynamicConformanceFacts,
    value: Handle<FlowOperandFact>,
    expected: TypeReferenceHandle,
    active_statements: Option<(SymbolHandle, HandleSpan<FlowStatementFact>)>,
) -> Option<SymbolHandle> {
    enum Work {
        Visit(Handle<FlowOperandFact>, Vec<PlaceSegment>),
        Leave,
    }
    let expected = program.normalized_type_identity(expected);
    let mut pending = vec![Work::Visit(value, Vec::new())];
    let mut active = Vec::new();
    let mut selected = None;
    while let Some(work) = pending.pop() {
        let Work::Visit(value, projection) = work else {
            active.pop()?;
            continue;
        };
        if !control.operands.is_valid(value) || active.contains(&(value, projection.clone())) {
            return None;
        }
        let capture = control.operands.get(value);
        let reference = validation::project_type_reference_from_segments(
            program,
            capture.type_reference,
            &projection,
        )?;
        if program.normalized_type_identity(reference) != expected {
            return None;
        }
        active.push((value, projection.clone()));
        pending.push(Work::Leave);
        // Joins quantify over every completed alternative before a surface
        // producer or retained earlier value may contribute a selection.
        if let checked_trees::FlowOperandCompletion::Selected(alternatives) = capture.completion {
            let rows = control.operand_alternatives.span(alternatives)?;
            if rows.is_empty() || rows.iter().any(|row| !row.source_arm.is_valid()) {
                return None;
            }
            pending.extend(
                rows.iter()
                    .map(|row| Work::Visit(row.value, projection.clone())),
            );
            continue;
        }
        if projection.is_empty() && !capture.value_source.is_valid() {
            let mut constructors = receipts.selections.iter().filter(|receipt| {
                receipt.occurrence == capture.expression && receipt.interface_identity == expected
            });
            if let Some(receipt) = constructors.next() {
                if constructors.any(|other| other.implementation != receipt.implementation)
                    || selected.is_some_and(|selected| selected != receipt.implementation)
                {
                    return None;
                }
                selected = Some(receipt.implementation);
                continue;
            }
        }
        let predecessors = super::super::completed_values::predecessors_in_control(
            program,
            control,
            value,
            &projection,
            super::super::stored_values::StoredObservation::Binding,
            active_statements,
        )?;
        if predecessors.is_empty() {
            return None;
        }
        pending.extend(
            predecessors
                .into_iter()
                .map(|(value, projection)| Work::Visit(value, projection)),
        );
    }
    selected
}
