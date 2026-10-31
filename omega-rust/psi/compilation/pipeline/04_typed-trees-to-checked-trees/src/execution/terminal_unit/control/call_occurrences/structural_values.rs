//! Shared occurrence accounting for an explicitly retained structural value.
//!
//! An ordinary statement and a scalar receiver dependency read the same value
//! graph. Source calls are counted once by their captured identity; discovering
//! another root or projection does not schedule another invocation. Selected
//! callers keep this walk inside the selected operand's own ledger.
use super::collecting::collect_structural_argument_calls;
use super::{CheckFacts, SymbolHandle};
use crate::flow::expressions;

pub(super) fn collect<'facts>(
    program: &dyn validation::CheckingInput,
    facts: &'facts CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement_index: usize,
    value: checked_trees::CheckedStructuralValueHandle,
    calls: &'facts [checked_trees::FlowCallFact],
    minimum_call_ordinal: u32,
    consumed: &mut super::CallOccurrenceLedger,
    structural: &mut Vec<&'facts checked_trees::FlowCallFact>,
) -> Option<()> {
    let mut pending = vec![value];
    let mut visited = Vec::new();
    while let Some(value) = pending.pop() {
        let plans = &facts.values.structural_values;
        if !plans.nodes.is_valid(value) {
            return None;
        }
        if visited.contains(&value) {
            continue;
        }
        visited.push(value);
        let node = plans.nodes.get(value);
        if matches!(
            node.kind,
            checked_trees::CheckedStructuralValueKind::Reference {
                storage: checked_trees::CheckedReferenceStorage::Placed
                    | checked_trees::CheckedReferenceStorage::ResidentPlaced { .. },
                ..
            }
        ) {
            let formation = validation::reference_formation_source(program, node.expression)?;
            if !formation.interpretation.is_placed() {
                return None;
            }
            let authored = program.expression_table.call_node(node.expression)?;
            let mut occurrences = calls
                .iter()
                .filter(|call| call.authored_expression == node.expression)
                .filter_map(|call| Some((facts.flow.control.calls.handle_of(call)?, call)));
            let (Some((handle, call)), None) = (occurrences.next(), occurrences.next()) else {
                return None;
            };
            if call.statement_index != statement_index
                || call.call_ordinal < minimum_call_ordinal as usize
                || call.target_symbol
                    != validation::call_target_entry_symbol(program, authored.target_symbol)
                || facts.flow.control.is_retired(state, call)
            {
                return None;
            }
            // The structural producer owns this authored factory
            // occurrence once. Execution establishes its captured loan;
            // it does not also invoke the bodyless declaration.
            consumed.record_minimum(Some(u32::try_from(call.call_ordinal).ok()?));
            consumed.record_once(handle);
        }
        match node.kind {
            checked_trees::CheckedStructuralValueKind::NativeCodeLayout { .. } => {}
            checked_trees::CheckedStructuralValueKind::QuotientCall {
                request,
                ref arguments,
                ..
            } => {
                // The sealed request owns this exact captured source
                // occurrence. Its selected ordinary call is emitted by
                // the retained value graph, not scheduled a second time.
                let mut occurrences = calls
                    .iter()
                    .filter(|call| call.authored_expression == request)
                    .filter_map(|call| Some((facts.flow.control.calls.handle_of(call)?, call)));
                let (handle, call) = occurrences.next()?;
                if occurrences.next().is_some()
                    || call.statement_index != statement_index
                    || call.call_ordinal < minimum_call_ordinal as usize
                    || facts.flow.control.is_retired(state, call)
                    || expressions(program)
                        .call_node(request)?
                        .quotient_operation
                        .is_none()
                {
                    return None;
                }
                consumed.record_minimum(Some(u32::try_from(call.call_ordinal).ok()?));
                consumed.record_once(handle);
                pending.extend(arguments.iter().rev().copied());
            }
            checked_trees::CheckedStructuralValueKind::Call { source_call } => {
                if !facts.flow.control.calls.is_valid(source_call) {
                    return None;
                }
                let call = facts.flow.control.calls.get(source_call);
                if call.authored_expression != node.expression
                    || call.statement_index != statement_index
                {
                    return None;
                }
                // Owner and projection roots can both retain this
                // captured occurrence. Discovery lists it once; the
                // operand schedule, not the number of root references,
                // owns its single execution and result establishment.
                if consumed.contains(&source_call) {
                    continue;
                }
                if call.call_ordinal < minimum_call_ordinal as usize
                    || facts.flow.control.is_retired(state, call)
                {
                    return None;
                }
                consumed.record_minimum(Some(u32::try_from(call.call_ordinal).ok()?));
                consumed.push(source_call);
                structural.push(call);
                let site = validation::find_call_site(
                    program,
                    machine,
                    state,
                    call.statement_index,
                    call.call_ordinal,
                )?;
                super::collecting::collect_argument_calls(
                    program,
                    facts,
                    machine,
                    state,
                    call,
                    validation::call_site_argument_expressions(program, &site),
                    calls,
                    consumed,
                )?;
                for operand in call.argument_operands().handles() {
                    let operand = facts.flow.control.operands.get(operand);
                    if let Some(root) = facts.values.structural_values.root_for_expression(
                        state,
                        u32::try_from(statement_index).ok()?,
                        operand.expression,
                    ) {
                        if root.machine != machine {
                            return None;
                        }
                        pending.push(root.root);
                    }
                }
            }
            checked_trees::CheckedStructuralValueKind::Record { fields, .. }
            | checked_trees::CheckedStructuralValueKind::StructuralCase { fields, .. } => {
                for field in plans.record_fields.span(fields)? {
                    if let checked_trees::CheckedStructuralRecordFieldValue::Structural(child) =
                        field.value
                    {
                        pending.push(child);
                    }
                }
            }
            checked_trees::CheckedStructuralValueKind::Dispatch { arms, .. } => {
                pending.extend(plans.dispatch_arms.span(arms)?.iter().map(|arm| arm.value));
            }
            checked_trees::CheckedStructuralValueKind::ViewSubslice {
                source,
                ref start,
                ref end,
                ..
            } => {
                pending.push(source);
                let mut visits = Vec::new();
                for value in start.iter().chain(end.iter()) {
                    if let checked_trees::CheckedCallScalarArgument::Computation(root) = value {
                        super::collecting::collect(
                            program,
                            facts,
                            state,
                            statement_index,
                            *root,
                            calls,
                            minimum_call_ordinal,
                            &mut visits,
                            consumed,
                        )?;
                    }
                }
            }
            checked_trees::CheckedStructuralValueKind::Projection {
                source,
                ref path,
                ref projection_evaluations,
                ..
            } => {
                for root in
                    checked_trees::CheckedUnitStructuralArgumentPlan::projection_computation_operands(
                        path, *projection_evaluations, &facts.values.scalar_computations.operands,
                    )
                {
                    super::collecting::collect_root_calls(
                        program,
                        facts,
                        state,
                        statement_index,
                        root,
                        calls,
                        minimum_call_ordinal,
                        consumed,
                    )?;
                }
                pending.push(source);
            }
            checked_trees::CheckedStructuralValueKind::QuotientAdaptation { source, .. } => {
                pending.push(source);
            }
            checked_trees::CheckedStructuralValueKind::FixedArray { ref elements } => {
                pending.extend(elements.iter().copied());
            }
            checked_trees::CheckedStructuralValueKind::Reference {
                source_value,
                selectors,
                ..
            } => {
                if source_value.is_valid() {
                    pending.push(source_value);
                }
                let mut visits = Vec::new();
                for root in facts.values.scalar_computations.operands.span(selectors)? {
                    super::collecting::collect(
                        program,
                        facts,
                        state,
                        statement_index,
                        *root,
                        calls,
                        minimum_call_ordinal,
                        &mut visits,
                        consumed,
                    )?;
                }
            }
            checked_trees::CheckedStructuralValueKind::BorrowedSliceView { ref source, .. }
            | checked_trees::CheckedStructuralValueKind::MutableSliceView { ref source, .. }
            | checked_trees::CheckedStructuralValueKind::CopiedStructuralPlace {
                ref source, ..
            }
            | checked_trees::CheckedStructuralValueKind::Place(ref source) => {
                collect_structural_argument_calls(
                    program,
                    facts,
                    state,
                    statement_index,
                    source,
                    calls,
                    minimum_call_ordinal,
                    &mut Vec::new(),
                    consumed,
                )?;
            }
            checked_trees::CheckedStructuralValueKind::Case(_)
            | checked_trees::CheckedStructuralValueKind::ZeroedScalarArray { .. }
            | checked_trees::CheckedStructuralValueKind::ScalarArray { .. }
            | checked_trees::CheckedStructuralValueKind::Scalar { .. }
            | checked_trees::CheckedStructuralValueKind::ZeroedStructural { .. }
            | checked_trees::CheckedStructuralValueKind::ByteLiteralView { .. } => {}
        }
    }
    Some(())
}
