/*
Returned reference custody, loan provenance, and region correspondence across
call boundaries (`machine_calls::reference_result_custody`).
*/
use crate::CheckingInput;
use crate::frozen_program::expression_table;
use crate::queries::type_applications::DeclarationScopes;
use crate::value_custody::storage_contents::ContentsRequirement;
use checked_trees::BorrowLoanOwnerSegment;
use checked_trees::CheckedMemberIdentity;
use checked_trees::CheckedUnitStructuralArgumentSourcePlan;
use checked_trees::{
    CheckFacts, CheckedReferenceResultSourcePlan, CheckedStructuralAccess,
    CheckedUnitStructuralResultBindingPlan, ReferenceCustodyFacts, ReferenceCustodyRow,
};
use facts::PlaceSegment;
use language_semantics::Multiplicity;
use language_semantics::ReferenceAccess;
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TransitionExit, TransitionTargetNode};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(crate) mod stored_carrier_sources {

    use checked_trees::CheckedMemberIdentity;
    use checked_trees::{
        CheckFacts, CheckedStructuralValueHandle, CheckedStructuralValueKind,
        CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralPathSegment,
    };
    use typed_trees::data::DataMember;

    pub enum StoredCarrierSource {
        Place(CheckedUnitStructuralArgumentPlan),
        Image {
            value: CheckedStructuralValueHandle,
        },
        Borrow {
            value: CheckedStructuralValueHandle,
            source: CheckedUnitStructuralArgumentPlan,
        },
        Call {
            value: CheckedStructuralValueHandle,
            path: Vec<CheckedUnitStructuralPathSegment>,
        },
    }

    pub fn stored_carrier_sources(
        program: &dyn crate::CheckingInput,
        facts: &CheckFacts,
        value: CheckedStructuralValueHandle,
        selection: &[CheckedUnitStructuralPathSegment],
    ) -> Option<Vec<StoredCarrierSource>> {
        let mut output = Vec::new();
        select(
            program,
            facts,
            value,
            selection,
            &mut Vec::new(),
            &mut output,
        )?;
        Some(output)
    }

    fn select(
        program: &dyn crate::CheckingInput,
        facts: &CheckFacts,
        value: CheckedStructuralValueHandle,
        selection: &[CheckedUnitStructuralPathSegment],
        active: &mut Vec<CheckedStructuralValueHandle>,
        output: &mut Vec<StoredCarrierSource>,
    ) -> Option<()> {
        let values = &facts.values.structural_values;
        if !values.nodes.is_valid(value) || active.contains(&value) {
            return None;
        }
        active.push(value);
        match &values.nodes.get(value).kind {
            CheckedStructuralValueKind::Reference {
                source,
                source_value,
                ..
            } => {
                if selection.is_empty() {
                    output.push(StoredCarrierSource::Borrow {
                        value,
                        source: source.clone(),
                    });
                } else {
                    let mut path = source.path.clone();
                    path.extend_from_slice(selection);
                    if source_value.is_valid() {
                        select(program, facts, *source_value, &path, active, output)?;
                    } else {
                        let mut source = source.clone();
                        source.path = path;
                        output.push(StoredCarrierSource::Place(source));
                    }
                }
            }
            CheckedStructuralValueKind::ViewSubslice { source, .. } => {
                if !selection.is_empty() {
                    return None;
                }
                let mut suppliers = Vec::new();
                select(program, facts, *source, &[], active, &mut suppliers)?;
                let mut borrows_supplier = false;
                for supplier in suppliers {
                    if matches!(supplier, StoredCarrierSource::Image { .. }) {
                        output.push(StoredCarrierSource::Image { value });
                    } else {
                        borrows_supplier = true;
                    }
                }
                if borrows_supplier {
                    let child = values.nodes.get(*source);
                    let access = crate::slice_view_shell(
                        program,
                        values.nodes.get(value).type_reference,
                        &[],
                    )?
                    .access;
                    output.push(StoredCarrierSource::Borrow {
                    value,
                    source: CheckedUnitStructuralArgumentPlan {
                        projection_evaluations: Default::default(),
                        source: checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralExpression { value_dependency: None, expression: child.expression, logical_root_type_identity: None },
                        path: Vec::new(),
                        type_identity: crate::stored_structural_type_identity(program, child.type_reference, &[], &[]),
                        access: match access {
                            language_core::ReferenceAccess::Shared => checked_trees::CheckedStructuralAccess::SharedBorrow,
                            language_core::ReferenceAccess::Mutable => checked_trees::CheckedStructuralAccess::MutableBorrow,
                            _ => return None,
                        },
                    },
                });
                }
            }
            CheckedStructuralValueKind::BorrowedSliceView { source, .. }
            | CheckedStructuralValueKind::MutableSliceView { source, .. } => {
                if !selection.is_empty() {
                    return None;
                }
                output.push(StoredCarrierSource::Borrow {
                    value,
                    source: source.clone(),
                });
            }
            CheckedStructuralValueKind::NativeCodeLayout { .. } => {
                let [CheckedUnitStructuralPathSegment::Field(selected)] = selection else {
                    return None;
                };
                let data =
                    program.closed_data_definition(values.nodes.get(value).type_reference)?;
                let mut fields =
                    program
                        .data_members(data)
                        .iter()
                        .filter_map(|member| match member {
                            DataMember::Field(field) => Some(field),
                            _ => None,
                        });
                let region = fields.nth(1)?;
                if CheckedMemberIdentity::of_member(region.identity, region.name.clone())
                    != *selected
                {
                    return None;
                }
                output.push(StoredCarrierSource::Image { value });
            }
            CheckedStructuralValueKind::ByteLiteralView { .. } => {
                if !selection.is_empty() {
                    return None;
                }
                output.push(StoredCarrierSource::Image { value });
            }
            CheckedStructuralValueKind::Place(source)
            | CheckedStructuralValueKind::CopiedStructuralPlace { source, .. } => {
                let mut source = source.clone();
                source.path.extend_from_slice(selection);
                output.push(StoredCarrierSource::Place(source));
            }
            CheckedStructuralValueKind::Projection { source, path, .. } => {
                let mut path = path.clone();
                path.extend_from_slice(selection);
                select(program, facts, *source, &path, active, output)?;
            }
            CheckedStructuralValueKind::Dispatch { arms, .. } => {
                let arms = values.dispatch_arms.span(*arms)?;
                if arms.is_empty() {
                    return None;
                }
                for arm in arms {
                    select(program, facts, arm.value, selection, active, output)?;
                }
            }
            CheckedStructuralValueKind::FixedArray { elements } => {
                let (index, suffix) = selection.split_first()?;
                match index {
                    CheckedUnitStructuralPathSegment::FixedIndex(element_index) => {
                        let child = elements.get(usize::try_from(*element_index).ok()?)?;
                        select(program, facts, *child, suffix, active, output)?;
                    }
                    CheckedUnitStructuralPathSegment::ComputedIndex(_)
                    | CheckedUnitStructuralPathSegment::RuntimeIndex(_) => {
                        if elements.is_empty() {
                            return None;
                        }
                        for child in elements {
                            select(program, facts, *child, suffix, active, output)?;
                        }
                    }
                    _ => return None,
                }
            }
            checked_trees::record_or_structural_case_pattern!(data_symbol, fields, ..) => {
                let data = program.data_definition_by_symbol(*data_symbol)?;
                let (field_identity, suffix, payload) = match selection {
                    [
                        CheckedUnitStructuralPathSegment::Case(case),
                        CheckedUnitStructuralPathSegment::Field(field),
                        suffix @ ..,
                    ] => {
                        let CheckedStructuralValueKind::StructuralCase { case: selected, .. } =
                            &values.nodes.get(value).kind
                        else {
                            return None;
                        };
                        let variant =
                            program
                                .data_members(data)
                                .iter()
                                .find_map(|member| match member {
                                    DataMember::Variant(variant)
                                        if CheckedMemberIdentity::of_member(
                                            variant.identity,
                                            variant.name.clone(),
                                        ) == *case =>
                                    {
                                        Some(variant)
                                    }
                                    _ => None,
                                })?;
                        program
                            .data_payload_fields(variant)
                            .iter()
                            .find(|declaration| {
                                CheckedMemberIdentity::of_member(
                                    declaration.identity,
                                    declaration.name.clone(),
                                ) == *field
                            })?;
                        let actual =
                            program
                                .data_members(data)
                                .iter()
                                .find_map(|member| match member {
                                    DataMember::Variant(variant) if variant.symbol == *selected => {
                                        Some(variant)
                                    }
                                    _ => None,
                                })?;
                        if actual.symbol != variant.symbol {
                            active.pop();
                            return Some(());
                        }
                        (field, suffix, Some(variant))
                    }
                    [CheckedUnitStructuralPathSegment::Field(field), suffix @ ..] => {
                        (field, suffix, None)
                    }
                    _ => return None,
                };
                let declaration = if let Some(variant) = payload {
                    program.data_payload_fields(variant).iter().find(|field| {
                        CheckedMemberIdentity::of_member(field.identity, field.name.clone())
                            == *field_identity
                    })?
                } else {
                    program
                        .data_members(data)
                        .iter()
                        .find_map(|member| match member {
                            DataMember::Field(field)
                                if CheckedMemberIdentity::of_member(
                                    field.identity,
                                    field.name.clone(),
                                ) == *field_identity =>
                            {
                                Some(field)
                            }
                            _ => None,
                        })?
                };
                let field = values
                    .record_fields
                    .span(*fields)?
                    .iter()
                    .find(|field| field.field == declaration.symbol)?;
                let checked_trees::CheckedStructuralRecordFieldValue::Structural(child) =
                    field.value
                else {
                    return None;
                };
                select(program, facts, child, suffix, active, output)?;
            }
            CheckedStructuralValueKind::Call { .. } => {
                output.push(StoredCarrierSource::Call {
                    value,
                    path: selection.to_vec(),
                });
            }
            CheckedStructuralValueKind::Case(case) => {
                let [CheckedUnitStructuralPathSegment::Case(requested), ..] = selection else {
                    return None;
                };
                let data = program.closed_data_definition(case.type_reference)?;
                let selected =
                    program
                        .data_members(data)
                        .iter()
                        .find_map(|member| match member {
                            DataMember::Variant(variant) if variant.symbol == case.case => {
                                Some(variant)
                            }
                            _ => None,
                        })?;
                if CheckedMemberIdentity::of_member(selected.identity, selected.name.clone())
                    == *requested
                    || !super::record_carrier_sources(
                        program,
                        case.type_reference,
                        checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                            symbol: values.nodes.get(value).state,
                        },
                    )?
                    .iter()
                    .any(|leaf| leaf.path == selection)
                {
                    return None;
                }
            }
            _ => return None,
        }
        active.pop();
        Some(())
    }
}
pub use stored_carrier_sources::{StoredCarrierSource, stored_carrier_sources};
pub(crate) mod call_reception {
    use super::{RecordLeafLoans, StoredCarrierSource, local_record_loans, record_carrier_sources};
    use checked_trees::{CheckFacts, CheckedUnitStructuralArgumentSourcePlan, FlowCallResult};
    use symbols::SymbolHandle;

    pub struct RecordCallReception {
        pub residual_paths: Vec<Vec<checked_trees::CheckedUnitStructuralPathSegment>>,
    }

    pub(crate) fn retained_call<'facts>(
        facts: &'facts CheckFacts,
        machine: SymbolHandle,
        state: SymbolHandle,
        call: &checked_trees::FlowCallFact,
    ) -> Result<&'facts checked_trees::FlowCallFact, &'static str> {
        let flow = super::state_flow(facts, machine, state)
            .ok_or("reference reception has no unique caller flow")?;
        let mut calls = facts
            .flow
            .control
            .calls
            .span(flow.calls)
            .ok_or("reference reception has a stale call span")?
            .iter()
            .filter(|candidate| {
                candidate.statement_index == call.statement_index
                    && candidate.call_ordinal == call.call_ordinal
            });
        let retained = calls
            .next()
            .ok_or("reference reception has no matching call occurrence")?;
        if calls.next().is_some() || retained != call {
            return Err("reference reception differs from its checked call occurrence");
        }
        Ok(retained)
    }

    pub fn record_call_reception(
        program: &dyn crate::CheckingInput,
        facts: &CheckFacts,
        machine: SymbolHandle,
        state: &typed_trees::state::State,
        call: &checked_trees::FlowCallFact,
    ) -> Result<RecordCallReception, &'static str> {
        let occurrence = retained_call(facts, machine, state.symbol, call)?;
        let statement_index = u32::try_from(occurrence.statement_index)
            .map_err(|_| "reference record call index exceeds u32")?;
        let expression = occurrence.authored_expression;
        let return_type = crate::call_target_return_type(program, occurrence.target_symbol)
            .ok_or("reference reception has no destination result contract")?;
        let flow = super::state_flow(facts, machine, state.symbol)
            .ok_or("reference reception has no unique caller flow")?;
        let paths = super::captured_returns::call_result_paths(program, facts, occurrence)
            .ok_or("reference result has no complete captured origin paths")?;
        let returned: Vec<_> = record_carrier_sources(
            program,
            return_type,
            CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                symbol: state.symbol,
            },
        )
        .ok_or("reference result has no declared carrier frontier")?
        .into_iter()
        .filter(|leaf| paths.contains(&leaf.path))
        .collect();
        let anonymous = || RecordCallReception {
            residual_paths: returned.iter().map(|leaf| leaf.path.clone()).collect(),
        };
        let Some(local) = program
            .statement_table
            .statement_at(state, statement_index as usize)
            .and_then(typed_trees::statement::StatementNode::as_local_data)
        else {
            return Ok(anonymous());
        };
        let statement = facts
            .flow
            .control
            .statements
            .span(flow.statements)
            .ok_or("reference reception has a stale statement span")?
            .iter()
            .find(|statement| statement.statement_index == statement_index as usize)
            .ok_or("reference reception has no receiving statement")?;
        let receives = occurrence.authored_result != FlowCallResult::Discarded
            && facts.flow.control.operands.is_valid(statement.stored_value)
            && facts
                .flow
                .control
                .operands
                .get(statement.stored_value)
                .expression
                == expression
            && facts.semantic.places.is_valid(statement.stored_destination)
            && {
                let destination = facts.semantic.places.get(statement.stored_destination);
                destination.root == facts::PlaceRoot::Symbol(local.symbol)
                    && facts
                        .semantic
                        .place_segments
                        .span(destination.segments)
                        .ok_or("reference reception has stale destination segments")?
                        .is_empty()
            };
        if !receives {
            return Ok(anonymous());
        }
        let Some(root) = facts.values.structural_values.root_for_expression(
            state.symbol,
            statement_index,
            expression,
        ) else {
            return Ok(anonymous());
        };
        if root.machine != machine {
            return Err("reference reception root belongs to another machine");
        }
        let Some(leaves) = record_carrier_sources(
            program,
            local.type_reference,
            CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                symbol: local.symbol,
            },
        ) else {
            return Ok(anonymous());
        };
        let mut received = Vec::new();
        for leaf in leaves {
            for source in super::stored_carrier_sources(program, facts, root.root, &leaf.path)
                .ok_or("reference reception has no complete stored carrier sources")?
            {
                let StoredCarrierSource::Call { value, path } = source else {
                    continue;
                };
                let node = facts.values.structural_values.nodes.get(value);
                if node.expression == expression {
                    let checked_trees::CheckedStructuralValueKind::Call { source_call } = node.kind
                    else {
                        return Err("reference reception source is not a captured call");
                    };
                    if node.machine != machine
                        || node.state != state.symbol
                        || node.statement_index != statement_index
                        || !facts.flow.control.calls.is_valid(source_call)
                    {
                        return Err("reference reception source has a different call coordinate");
                    }
                    let occurrence = facts.flow.control.calls.get(source_call);
                    if occurrence.authored_expression != expression
                        || occurrence.statement_index != statement_index as usize
                        || occurrence.target_symbol != call.target_symbol
                    {
                        return Err("reference reception source differs from its authored call");
                    }
                    if returned.iter().any(|leaf| leaf.path == path) {
                        received.push((leaf.path.clone(), path));
                    }
                }
            }
        }
        if received.is_empty() {
            return Ok(anonymous());
        }
        let groups = local_record_loans(program, facts, machine, state, statement_index)
            .ok_or("reference reception has no complete local loan groups")?;
        let mut covered = vec![false; returned.len()];
        for (receiving_path, result_path) in received {
            let (returned_index, returned_leaf) = returned
                .iter()
                .enumerate()
                .find(|(_, leaf)| leaf.path == result_path)
                .ok_or("reference reception names an undeclared result leaf")?;
            let (receiving_leaf, group): &RecordLeafLoans = groups
                .iter()
                .find(|(leaf, _)| leaf.path == receiving_path)
                .ok_or("reference reception has no loan group for its receiving leaf")?;
            if receiving_leaf.source.type_identity != returned_leaf.source.type_identity
                || receiving_leaf.source.access != returned_leaf.source.access
            {
                return Err("reference reception changes a leaf type or access");
            }
            let mut formed = false;
            for (_, loan) in facts
                .borrow
                .loan_group(*group)
                .ok_or("reference reception has a stale loan group")?
            {
                if loan.formation_expression != expression {
                    continue;
                }
                if loan.kind
                    != returned_leaf
                        .source
                        .access
                        .borrow_access_kind()
                        .ok_or("reference reception leaf has no borrowed access")?
                {
                    return Err("reference reception loan changes the returned access");
                }
                formed = true;
            }
            if !formed {
                return Err("reference reception leaf has no loan from this call formation");
            }
            covered[returned_index] = true;
        }
        Ok(RecordCallReception {
            residual_paths: returned
                .into_iter()
                .zip(covered)
                .filter_map(|(leaf, covered)| (!covered).then_some(leaf.path))
                .collect(),
        })
    }
}
pub use call_reception::{RecordCallReception, record_call_reception};
pub(crate) mod captured_returns {

    pub(crate) mod completion_calls {
        use arena::Handle;
        use checked_trees::{FlowCallFact, FlowControlFacts, FlowOperandFact};
        use std::collections::BTreeMap;
        use symbols::SymbolHandle;

        pub(crate) type Identity = (u32, u32);
        pub(crate) type ExpressionSite = (usize, Identity);
        pub(crate) type StateCalls = BTreeMap<ExpressionSite, Option<Handle<FlowCallFact>>>;

        fn key<T>(handle: Handle<T>) -> Identity {
            (handle.arena_index(), handle.generation())
        }

        /*
        Memoized call lookups for one captured-return question. Each map fills
        lazily on first use, so a question only pays for the states, invocations
        and targets it actually reaches. The owner of the question holds this
        table by value and lends it `&mut` to the capture walk; every lookup
        reads the same flow-control facts, which callers pass in explicitly.
        */
        #[derive(Default)]
        pub(crate) struct CompletionCalls {
            states: BTreeMap<Identity, Option<StateCalls>>,
            owners: BTreeMap<Identity, Vec<SymbolHandle>>,
            targets: BTreeMap<Identity, Option<Option<SymbolHandle>>>,
        }

        impl CompletionCalls {
            fn prepare_state(
                control: &FlowControlFacts,
                state: SymbolHandle,
            ) -> Option<StateCalls> {
                let flow = control.unique_state_by_symbol(state)?;
                control.calls.span(flow.calls)?;
                let mut calls = StateCalls::new();
                for handle in flow.calls.handles() {
                    let call = control.calls.get(handle);
                    if !call.authored_expression.is_valid()
                        || call.authored_dispatch_role
                            == checked_trees::FlowCallDispatchRole::SavedSubjectRead
                    {
                        continue;
                    }
                    let site = (call.statement_index, key(call.authored_expression));
                    calls
                        .entry(site)
                        .and_modify(|previous| *previous = None)
                        .or_insert(Some(handle));
                }
                Some(calls)
            }

            pub(crate) fn ordinary(
                &mut self,
                control: &FlowControlFacts,
                operand: &FlowOperandFact,
            ) -> Option<Option<Handle<FlowCallFact>>> {
                let calls = self
                    .states
                    .entry(key(operand.state_symbol))
                    .or_insert_with(|| Self::prepare_state(control, operand.state_symbol))
                    .as_ref()?;
                if !operand.expression.is_valid() {
                    return Some(None);
                }
                match calls.get(&(operand.statement_index, key(operand.expression))) {
                    Some(Some(call)) => Some(Some(*call)),
                    Some(None) => None,
                    None => Some(None),
                }
            }

            pub(crate) fn invocation<'facts>(
                &mut self,
                control: &'facts FlowControlFacts,
                state: SymbolHandle,
                invocation: Handle<FlowCallFact>,
            ) -> Option<&'facts FlowCallFact> {
                control.unique_state_by_symbol(state)?;
                if !control.calls.is_valid(invocation) {
                    return None;
                }
                self.owners
                    .entry(key(invocation))
                    .or_insert_with(|| {
                        let mut owners = control
                            .call_owners(invocation)
                            .into_iter()
                            .map(|row| row.state_symbol)
                            .collect::<Vec<_>>();
                        owners.sort_by_key(|state| key(*state));
                        owners
                    })
                    .binary_search_by_key(&key(state), |candidate| key(*candidate))
                    .is_ok()
                    .then(|| control.calls.get(invocation))
            }

            pub(crate) fn known_target(
                &self,
                target: SymbolHandle,
            ) -> Option<Option<Option<SymbolHandle>>> {
                self.targets.get(&key(target)).copied()
            }

            pub(crate) fn record_target(
                &mut self,
                target: SymbolHandle,
                result: Option<Option<SymbolHandle>>,
            ) -> Option<Option<SymbolHandle>> {
                self.targets.insert(key(target), result);
                result
            }
        }

        #[cfg(test)]
        mod tests {
            use super::*;
            use checked_trees::{FlowCallDispatchRole, FlowStateFact};
            use typed_trees::expression::ExpressionHandle;

            #[test]
            fn captured_return_call_roster_keeps_exact_sites_and_ambiguity() {
                let mut control = FlowControlFacts::default();
                let state = SymbolHandle::from_parts(1, 1);
                let expression = ExpressionHandle::from_parts(10, 1);
                let ordinary = |statement_index, authored_expression| FlowCallFact {
                    statement_index,
                    authored_expression,
                    ..FlowCallFact::default()
                };
                let calls = control.calls.insert_many([
                    ordinary(1, expression),
                    FlowCallFact {
                        authored_dispatch_role: FlowCallDispatchRole::SavedSubjectRead,
                        ..ordinary(1, expression)
                    },
                    ordinary(2, expression),
                    ordinary(3, expression),
                    ordinary(3, expression),
                    ordinary(1, ExpressionHandle::from_parts(10, 2)),
                ]);
                control.states.insert(FlowStateFact {
                    state_symbol: state,
                    calls,
                    ..FlowStateFact::default()
                });
                let mut prepared = CompletionCalls::default();
                let mut operand = FlowOperandFact {
                    state_symbol: state,
                    statement_index: 1,
                    expression,
                    ..FlowOperandFact::default()
                };
                assert_eq!(
                    prepared.ordinary(&control, &operand),
                    Some(Some(calls.handle_at(0)))
                );
                operand.statement_index = 2;
                assert_eq!(
                    prepared.ordinary(&control, &operand),
                    Some(Some(calls.handle_at(2)))
                );
                operand.statement_index = 3;
                assert_eq!(prepared.ordinary(&control, &operand), None);
                operand.statement_index = 4;
                assert_eq!(prepared.ordinary(&control, &operand), Some(None));
                operand.statement_index = 1;
                operand.expression = ExpressionHandle::from_parts(10, 2);
                assert_eq!(
                    prepared.ordinary(&control, &operand),
                    Some(Some(calls.handle_at(5)))
                );
                operand.state_symbol = SymbolHandle::from_parts(1, 2);
                assert_eq!(prepared.ordinary(&control, &operand), None);
            }

            #[test]
            fn captured_return_invocation_keeps_owner_and_generation_checks() {
                let mut control = FlowControlFacts::default();
                let state = SymbolHandle::from_parts(1, 1);
                let foreign = SymbolHandle::from_parts(2, 1);
                let calls = control.calls.insert_many([FlowCallFact::default()]);
                control.states.insert(FlowStateFact {
                    state_symbol: state,
                    calls,
                    ..FlowStateFact::default()
                });
                control.states.insert(FlowStateFact {
                    state_symbol: foreign,
                    ..FlowStateFact::default()
                });
                let mut prepared = CompletionCalls::default();
                assert!(
                    prepared
                        .invocation(&control, state, calls.start())
                        .is_some()
                );
                assert!(
                    prepared
                        .invocation(&control, foreign, calls.start())
                        .is_none()
                );
                assert!(
                    prepared
                        .invocation(
                            &control,
                            state,
                            Handle::from_parts(
                                calls.start().arena_index(),
                                calls.start().generation() + 1
                            )
                        )
                        .is_none()
                );
                drop(prepared);
                control.states.insert(FlowStateFact {
                    state_symbol: state,
                    calls,
                    ..FlowStateFact::default()
                });
                assert!(
                    CompletionCalls::default()
                        .invocation(&control, state, calls.start())
                        .is_none()
                );
            }

            #[test]
            fn captured_return_target_retains_negative_and_known_empty_classification() {
                let mut prepared = CompletionCalls::default();
                let unavailable = SymbolHandle::from_parts(1, 1);
                let empty = SymbolHandle::from_parts(1, 2);
                let mut classifications = 0;
                let mut target = |target, result: Option<Option<SymbolHandle>>| {
                    if let Some(known) = prepared.known_target(target) {
                        return known;
                    }
                    classifications += 1;
                    prepared.record_target(target, result)
                };
                for _ in 0..2 {
                    assert_eq!(target(unavailable, None), None);
                    assert_eq!(target(empty, Some(None)), Some(None));
                }
                assert_eq!(classifications, 2);
            }
        }
    }
    pub(crate) mod completions {

        use arena::Handle;
        use checked_trees::{FlowControlFacts, FlowOperandCompletion, FlowOperandFact};
        use std::collections::BTreeMap;
        use symbols::SymbolHandle;

        #[derive(Clone, Debug, PartialEq, Eq)]
        pub(crate) enum Coverage {
            Call(SymbolHandle),
            All(Vec<usize>),
            Any(Vec<usize>),
        }

        #[derive(Clone, Debug, Default, PartialEq, Eq)]
        pub(crate) struct ReturnCoverage {
            nodes: Vec<Coverage>,
            root: usize,
        }

        impl ReturnCoverage {
            pub(crate) fn captured(
                control: &FlowControlFacts,
                operand: &FlowOperandFact,
                calls: impl FnMut(&FlowOperandFact) -> Option<Vec<SymbolHandle>>,
            ) -> Option<Self> {
                let mut builder = Builder::new(control, calls);
                builder.graph.root = builder.operand(operand)?;
                Some(builder.graph)
            }

            pub(crate) fn invocation(
                control: &FlowControlFacts,
                call: &checked_trees::FlowCallFact,
                target: Option<SymbolHandle>,
                calls: impl FnMut(&FlowOperandFact) -> Option<Vec<SymbolHandle>>,
            ) -> Option<Self> {
                let mut builder = Builder::new(control, calls);
                let mut required = Vec::new();
                control.operands.span(call.operands)?;
                for operand in call.operands.handles() {
                    required.push(builder.value(operand)?);
                }
                if let Some(target) = target {
                    required.push(builder.push(Coverage::Call(target)));
                }
                builder.graph.root = builder.push(Coverage::All(required));
                Some(builder.graph)
            }

            /// A selected body supplies completion, while the relation owner still
            /// validates every body equation and retains every possible origin.
            pub(super) fn alternatives(
                states: impl IntoIterator<Item = SymbolHandle>,
            ) -> Option<Self> {
                let mut nodes: Vec<_> = states.into_iter().map(Coverage::Call).collect();
                if nodes.is_empty() {
                    return None;
                }
                let root = nodes.len();
                nodes.push(Coverage::Any((0..root).collect()));
                Some(Self { nodes, root })
            }

            pub(crate) fn dependencies(&self) -> impl Iterator<Item = SymbolHandle> + '_ {
                self.nodes.iter().filter_map(|node| match node {
                    Coverage::Call(state) => Some(*state),
                    Coverage::All(_) | Coverage::Any(_) => None,
                })
            }

            pub(crate) fn established(&self, returning: impl Fn(SymbolHandle) -> bool) -> bool {
                let mut values = Vec::with_capacity(self.nodes.len());
                for node in &self.nodes {
                    values.push(match node {
                        Coverage::Call(state) => returning(*state),
                        Coverage::All(children) => children.iter().all(|child| values[*child]),
                        Coverage::Any(children) => children.iter().any(|child| values[*child]),
                    });
                }
                values.get(self.root).copied().unwrap_or(false)
            }
        }

        pub(crate) struct Builder<'facts, Calls> {
            control: &'facts FlowControlFacts,
            calls: Calls,
            graph: ReturnCoverage,
            values: BTreeMap<(u32, u32), Option<usize>>,
            prefixes: BTreeMap<(u32, u32), Option<usize>>,
        }

        impl<'facts, Calls> Builder<'facts, Calls>
        where
            Calls: FnMut(&FlowOperandFact) -> Option<Vec<SymbolHandle>>,
        {
            fn new(control: &'facts FlowControlFacts, calls: Calls) -> Self {
                Self {
                    control,
                    calls,
                    graph: ReturnCoverage::default(),
                    values: BTreeMap::new(),
                    prefixes: BTreeMap::new(),
                }
            }

            fn push(&mut self, completion: Coverage) -> usize {
                let index = self.graph.nodes.len();
                self.graph.nodes.push(completion);
                index
            }

            fn value(&mut self, value: Handle<FlowOperandFact>) -> Option<usize> {
                let key = (value.arena_index(), value.generation());
                if let Some(result) = self.values.get(&key) {
                    return *result;
                }
                if !self.control.operands.is_valid(value) {
                    return None;
                }
                self.values.insert(key, None);
                let result = self.operand(self.control.operands.get(value))?;
                self.values.insert(key, Some(result));
                Some(result)
            }

            fn prefix(
                &mut self,
                arm: Handle<checked_trees::FlowOperandAlternative>,
            ) -> Option<usize> {
                let key = (arm.arena_index(), arm.generation());
                if let Some(result) = self.prefixes.get(&key) {
                    return *result;
                }
                if !self.control.operand_alternatives.is_valid(arm) {
                    return None;
                }
                self.prefixes.insert(key, None);
                let arm = *self.control.operand_alternatives.get(arm);
                let mut inputs = Vec::new();
                if arm.prior_evaluation.is_valid() {
                    inputs.push(self.prefix(arm.prior_evaluation)?);
                } else if arm.evaluation_inputs.is_empty() {
                    return None;
                }
                self.control.operands.span(arm.evaluation_inputs)?;
                for input in arm.evaluation_inputs.handles() {
                    inputs.push(self.value(input)?);
                }
                let result = self.push(Coverage::All(inputs));
                self.prefixes.insert(key, Some(result));
                Some(result)
            }

            fn operand(&mut self, operand: &FlowOperandFact) -> Option<usize> {
                let mut required = Vec::new();
                for state in (self.calls)(operand)? {
                    required.push(self.push(Coverage::Call(state)));
                }
                let inputs = self.control.operands.span(operand.evaluated_inputs)?;
                let guaranteed = operand.conditional_inputs_from.unwrap_or(inputs.len());
                inputs.get(..guaranteed)?;
                for input in operand.evaluated_inputs.handles().take(guaranteed) {
                    required.push(self.value(input)?);
                }
                for source in [operand.backing, operand.value_source] {
                    if source.is_valid() {
                        required.push(self.value(source)?);
                    }
                }
                match operand.completion {
                    FlowOperandCompletion::Constructed(children) => {
                        for child in self.control.operand_children.span(children)? {
                            required.push(self.value(child.value)?);
                        }
                    }
                    FlowOperandCompletion::Selected(alternatives) => {
                        self.control.operand_alternatives.span(alternatives)?;
                        let mut selected = Vec::new();
                        for alternative in alternatives.handles() {
                            let prefix = self.prefix(alternative)?;
                            let value = self
                                .value(self.control.operand_alternatives.get(alternative).value)?;
                            selected.push(self.push(Coverage::All(vec![prefix, value])));
                        }
                        required.push(self.push(Coverage::Any(selected)));
                    }
                    FlowOperandCompletion::Copied { copy, .. } => {
                        if !self.control.copied_values.is_valid(copy) {
                            return None;
                        }
                        required.push(self.value(self.control.copied_values.get(copy).source)?);
                    }
                    FlowOperandCompletion::InvocationResult { invocation, .. } => {
                        if !self.control.calls.is_valid(invocation) {
                            return None;
                        }
                        let call = self.control.calls.get(invocation);
                        self.control.operands.span(call.operands)?;
                        for input in call.operands.handles() {
                            required.push(self.value(input)?);
                        }
                    }
                    FlowOperandCompletion::Observation
                    | FlowOperandCompletion::IncomingContents { .. } => {}
                }
                Some(self.push(Coverage::All(required)))
            }
        }

        #[cfg(test)]
        mod tests {
            use super::*;
            use checked_trees::{FlowCallFact, FlowOperandAlternative, FlowOperandChild};

            fn call(control: &mut FlowControlFacts, state: u32) -> Handle<FlowOperandFact> {
                let invocation = control.calls.insert(FlowCallFact {
                    target_symbol: SymbolHandle::from_parts(state, 1),
                    ..FlowCallFact::default()
                });
                control.operands.insert(FlowOperandFact {
                    completion: FlowOperandCompletion::InvocationResult {
                        invocation,
                        result_type: Handle::invalid(),
                        path: arena::HandleSpan::empty(),
                    },
                    ..FlowOperandFact::default()
                })
            }

            fn captured(control: &FlowControlFacts, value: &FlowOperandFact) -> ReturnCoverage {
                ReturnCoverage::captured(control, value, |operand| {
                    Some(match operand.completion {
                        FlowOperandCompletion::InvocationResult { invocation, .. } => {
                            vec![control.calls.get(invocation).target_symbol]
                        }
                        _ => Vec::new(),
                    })
                })
                .expect("the actual capture graph is complete")
            }

            #[test]
            fn captured_return_completion_requires_every_constructed_child() {
                let mut control = FlowControlFacts::default();
                let recursive = call(&mut control, 1);
                let grounded = call(&mut control, 2);
                let inner = control.operand_children.insert_many([FlowOperandChild {
                    value: recursive,
                    ..FlowOperandChild::default()
                }]);
                let inner = control.operands.insert(FlowOperandFact {
                    completion: FlowOperandCompletion::Constructed(inner),
                    ..FlowOperandFact::default()
                });
                let children = control.operand_children.insert_many([
                    FlowOperandChild {
                        value: inner,
                        ..FlowOperandChild::default()
                    },
                    FlowOperandChild {
                        value: grounded,
                        ..FlowOperandChild::default()
                    },
                ]);
                let value = FlowOperandFact {
                    completion: FlowOperandCompletion::Constructed(children),
                    ..FlowOperandFact::default()
                };
                let completion = captured(&control, &value);
                assert!(!completion.established(|state| state.arena_index() == 2));
                assert!(completion.established(|_| true));
                assert_eq!(completion.dependencies().count(), 2);
            }

            #[test]
            fn captured_return_completion_keeps_selected_prefix_separate_from_arm_values() {
                let mut control = FlowControlFacts::default();
                let subject = call(&mut control, 1);
                let recursive = call(&mut control, 2);
                let constant = control.operands.insert(FlowOperandFact::default());
                let prefix = control
                    .operands
                    .insert_many([*control.operands.get(subject)]);
                let mut alternatives = arena::HandleSpan::empty();
                let prior = control.operand_alternatives.append_to_span(
                    &mut alternatives,
                    FlowOperandAlternative {
                        evaluation_inputs: prefix,
                        value: recursive,
                        ..FlowOperandAlternative::default()
                    },
                );
                control.operand_alternatives.append_to_span(
                    &mut alternatives,
                    FlowOperandAlternative {
                        prior_evaluation: prior,
                        value: constant,
                        ..FlowOperandAlternative::default()
                    },
                );
                let value = FlowOperandFact {
                    completion: FlowOperandCompletion::Selected(alternatives),
                    ..FlowOperandFact::default()
                };
                let completion = captured(&control, &value);
                assert!(!completion.established(|state| state.arena_index() == 2));
                assert!(completion.established(|state| state.arena_index() == 1));
            }

            #[test]
            fn captured_return_completion_requires_pattern_before_selected_constant() {
                let mut control = FlowControlFacts::default();
                let subject = control.operands.insert(FlowOperandFact::default());
                let pattern = call(&mut control, 1);
                let constant = control.operands.insert(FlowOperandFact::default());
                let inputs = control.operands.insert_many([
                    *control.operands.get(subject),
                    *control.operands.get(pattern),
                ]);
                let alternatives =
                    control
                        .operand_alternatives
                        .insert_many([FlowOperandAlternative {
                            evaluation_inputs: inputs,
                            value: constant,
                            ..FlowOperandAlternative::default()
                        }]);
                let value = FlowOperandFact {
                    completion: FlowOperandCompletion::Selected(alternatives),
                    ..FlowOperandFact::default()
                };
                assert!(!captured(&control, &value).established(|_| false));
                assert!(captured(&control, &value).established(|_| true));
            }

            #[test]
            fn captured_return_completion_distinguishes_formed_empty_from_missing_selection() {
                let control = FlowControlFacts::default();
                let empty = FlowOperandFact {
                    completion: FlowOperandCompletion::Constructed(arena::HandleSpan::empty()),
                    ..FlowOperandFact::default()
                };
                assert!(captured(&control, &empty).established(|_| false));
                let missing = FlowOperandFact {
                    completion: FlowOperandCompletion::Selected(arena::HandleSpan::empty()),
                    ..FlowOperandFact::default()
                };
                assert!(!captured(&control, &missing).established(|_| false));
            }
            #[test]
            fn captured_return_coverage_known_empty_result_keeps_evaluated_inputs() {
                let mut control = FlowControlFacts::default();
                let child = call(&mut control, 1);
                let no_descriptors = call(&mut control, 3);
                let inputs = control.operands.insert_many([*control.operands.get(child)]);
                let value = *control.operands.get(no_descriptors);
                let FlowOperandCompletion::InvocationResult { invocation, .. } = value.completion
                else {
                    panic!("the descriptor retains its actual invocation");
                };
                control.calls.get_mut(invocation).operands = inputs;
                let coverage = ReturnCoverage::captured(&control, &value, |operand| {
                    let FlowOperandCompletion::InvocationResult { invocation, .. } =
                        operand.completion
                    else {
                        return Some(Vec::new());
                    };
                    let target = control.calls.get(invocation).target_symbol;
                    Some(if target.arena_index() == 3 {
                        Vec::new()
                    } else {
                        vec![target]
                    })
                })
                .expect("both completed inputs are retained");
                assert!(!coverage.established(|_| false));
                assert!(coverage.established(|state| state.arena_index() == 1));
                assert_eq!(
                    coverage.dependencies().collect::<Vec<_>>(),
                    vec![SymbolHandle::from_parts(1, 1)]
                );
            }
        }
    }
    pub(crate) mod descriptor_sources {
        use super::{Captures, TypeReferenceHandle};
        use crate::completed_storage::FrozenReferenceSelection;
        use arena::Handle;
        use checked_trees::FlowOperandFact;
        use facts::PlaceSegment;

        impl<'capture> Captures<'capture> {
            #[allow(clippy::too_many_arguments)]
            pub(crate) fn completed_descriptor_sources(
                &self,
                state: &typed_trees::state::State,
                operand: Handle<FlowOperandFact>,
                path: &[PlaceSegment],
                expected: TypeReferenceHandle,
                suffix: &crate::LocalWriteSource,
                current: Handle<checked_trees::FlowStorageSnapshot>,
                active: &mut Vec<(Handle<FlowOperandFact>, Vec<PlaceSegment>)>,
            ) -> Option<Vec<crate::LocalWriteSource>> {
                let query = (operand, path.to_vec());
                let control = &self.facts.flow.control;
                if active.len() >= super::super::MAX_REFERENCE_ORIGIN_DEPTH
                    || active.contains(&query)
                    || !control.operands.is_valid(operand)
                {
                    return None;
                }
                let actual = control.operands.get(operand);
                let supplied = crate::project_type_reference_from_segments(
                    self.program,
                    actual.type_reference,
                    path,
                )?;
                let supplied = crate::reference_shell(self.program, supplied)?;
                let expected_shell = crate::reference_shell(self.program, expected)?;
                if !(supplied.access == expected_shell.access
                    || supplied.access == language_semantics::ReferenceAccess::Mutable)
                    || !crate::storage_origin_types_match(
                        self.program,
                        supplied.referee,
                        expected_shell.referee,
                    )
                {
                    return None;
                }
                active.push(query);
                let selected = (|| {
                    match crate::completed_storage::frozen_reference_leaf(
                        self.program,
                        control,
                        operand,
                        path,
                    ) {
                        FrozenReferenceSelection::Unavailable => return None,
                        FrozenReferenceSelection::Endpoint { leaf, .. }
                            if !leaf.referents.is_empty() =>
                        {
                            return self.descriptor_referent_sources(leaf.referents, suffix);
                        }
                        _ => {}
                    }
                    let Some(stored) = crate::completed_storage::reference_captures_at_path(
                        self.program,
                        control,
                        operand,
                        path,
                        current,
                    ) else {
                        let incoming = self.completed_operand_source(actual)?;
                        if incoming.state_symbol != state.symbol {
                            return None;
                        }
                        let mut place = crate::canonical_place_from_expression_in_state(
                            self.program,
                            state.symbol,
                            incoming.statement_index,
                            incoming.expression,
                        )?;
                        let root = place.root.as_symbol()?;
                        if !self.ingress_present(state, root)? {
                            return None;
                        }
                        place.segments.extend_from_slice(path);
                        let origin = crate::LocalWriteSource {
                            invocation_output: None,
                            invocation_result: None,
                            is_image_storage: false,
                            recasts: Vec::new(),
                            representation: None,
                            path: self.program.symbols.name(root).to_owned(),
                            collection_coarse: false,
                            root,
                            segments: place.segments,
                        };
                        return Some(vec![origin.substitute_storage(self.program, suffix)?]);
                    };
                    let descriptors = stored.captures()?;
                    if descriptors.is_empty() {
                        return None;
                    }
                    let mut output = Vec::new();
                    for selected in descriptors {
                        let descriptor = control.operands.get(selected.capture);
                        let sources = if !selected.projection.is_empty() {
                            self.completed_descriptor_sources(
                                state,
                                selected.capture,
                                &selected.projection,
                                expected,
                                suffix,
                                current,
                                active,
                            )?
                        } else if !descriptor.referents.is_empty() {
                            self.descriptor_referent_sources(descriptor.referents, suffix)?
                        } else {
                            if !control.operand_retentions.is_valid(descriptor.retention) {
                                return None;
                            }
                            control.retained_roots(descriptor.retention)?;
                            let profile = control.operand_retentions.get(descriptor.retention);
                            let suppliers = control
                                .operand_retained_suppliers
                                .span(profile.descriptor_sources)?;
                            if suppliers.is_empty() {
                                return None;
                            }
                            let mut sources = Vec::new();
                            for supplier in suppliers {
                                sources.extend(self.completed_descriptor_sources(
                                    state,
                                    supplier.operand,
                                    control.operand_retained_paths.span(supplier.projection)?,
                                    expected,
                                    suffix,
                                    current,
                                    active,
                                )?);
                            }
                            sources
                        };
                        if sources.is_empty() {
                            return None;
                        }
                        for source in sources {
                            if !output.contains(&source) {
                                output.push(source);
                            }
                        }
                        if output.len() > super::super::MAX_REFERENCE_ORIGIN_LEAVES {
                            return None;
                        }
                    }
                    Some(output)
                })();
                active.pop();
                selected
            }

            fn descriptor_referent_sources(
                &self,
                referents: arena::HandleSpan<checked_trees::FlowOperandReferent>,
                suffix: &crate::LocalWriteSource,
            ) -> Option<Vec<crate::LocalWriteSource>> {
                let control = &self.facts.flow.control;
                let mut sources = Vec::new();
                for referent in control.operand_referents.span(referents)? {
                    let root = referent.root.as_symbol()?;
                    let origin = crate::LocalWriteSource {
                        invocation_output: None,
                        invocation_result: None,
                        is_image_storage: false,
                        recasts: Vec::new(),
                        representation: None,
                        path: self.program.symbols.name(root).to_owned(),
                        collection_coarse: false,
                        root,
                        segments: control
                            .operand_referent_segments
                            .span(referent.segments)?
                            .to_vec(),
                    };
                    let origin = origin.substitute_storage(self.program, suffix)?;
                    if !sources.contains(&origin) {
                        sources.push(origin);
                    }
                }
                (!sources.is_empty()).then_some(sources)
            }
        }
    }
    mod family_returns {
        //! A portable interface returns through its complete retained body roster.
        //! The interface owns no flow captures. Each body's equation supplies actual
        //! origins in the common telescope, and every body remains a dependency.

        use super::{Captures, CheckedReturnedReferencePlan, State, completions::ReturnCoverage};

        /// None inside Some means an ordinary state. A missing or incompatible family
        /// is unavailable data, never an empty list of returned descriptors.
        pub(super) fn entries<'a>(
            program: &'a dyn crate::CheckingInput,
            state: &State,
        ) -> Option<Option<Vec<&'a State>>> {
            let machine = program.machine_holding_state(state.symbol)?;
            if !machine.target_family_interface {
                return Some(None);
            }
            if program.machine_states(machine).first()?.symbol != state.symbol {
                return None;
            }
            let parameters = program.state_parameters(state);
            let mut entries = Vec::new();
            for member in crate::target_family_members(program, machine)? {
                let body = program.machine(member.machine)?;
                if !body.body_is_present || !body.supply_mode.is_checked_body() {
                    return None;
                }
                let entry = program.machine_states(body).first()?;
                let actual = program.state_parameters(entry);
                if program.normalized_type_identity(entry.return_type)
                    != program.normalized_type_identity(state.return_type)
                    || actual.len() != parameters.len()
                    || actual.iter().zip(parameters).any(|(actual, expected)| {
                        actual.is_const != expected.is_const
                            || actual.is_mutable != expected.is_mutable
                            || actual.relevance != expected.relevance
                            || program.normalized_type_identity(actual.type_reference)
                                != program.normalized_type_identity(expected.type_reference)
                    })
                {
                    return None;
                }
                entries.push(entry);
            }
            Some(Some(entries))
        }

        impl Captures<'_> {
            pub(super) fn family_return(
                &mut self,
                entries: &[&State],
            ) -> Option<Vec<CheckedReturnedReferencePlan>> {
                let relations = self.relations?;
                let observed = self.observed.as_deref_mut()?;
                let mut output = Vec::new();
                let mut returning = false;
                for entry in entries {
                    returning |= crate::normal_completion_possible(self.program, entry)?;
                    for source in relations.approximation(entry.symbol, observed) {
                        if !output.contains(&source) {
                            output.push(source);
                            if output.len() > super::super::MAX_REFERENCE_ORIGIN_LEAVES {
                                return None;
                            }
                        }
                    }
                }
                if returning {
                    observed.completed(ReturnCoverage::alternatives(
                        entries.iter().map(|entry| entry.symbol),
                    )?);
                } else {
                    observed.nonreturning();
                }
                Some(output)
            }
        }
    }
    pub(crate) mod incoming_origins {
        use super::{CapturedReturnOrigin, Captures, TypeReferenceHandle, frame_origin_fits_loan};

        pub(crate) struct IncomingLeaf {
            pub(crate) carrier: TypeReferenceHandle,
            pub(crate) reference: super::super::DeclaredCarrierReference,
            pub(crate) segments: Vec<facts::PlaceSegment>,
        }

        impl<'capture> Captures<'capture> {
            pub(crate) fn invocation_carrier(
                &self,
                state: &typed_trees::state::State,
                incoming: &checked_trees::IncomingReferenceIdentity,
            ) -> Option<(
                TypeReferenceHandle,
                Vec<checked_trees::CheckedReferenceOriginProjection>,
            )> {
                let checked_trees::IncomingReferenceOrigin::Invocation { expression, target } =
                    incoming.origin
                else {
                    return None;
                };
                if incoming.state_symbol != state.symbol
                    || incoming.machine_symbol != self.flow(state.symbol)?.machine_symbol
                    || !matches!(self.program.expression_table.expression(expression), typed_trees::expression::ExpressionNode::Call(call) if crate::call_target_entry_symbol(self.program, crate::call_target_symbol(self.program, call)) == crate::call_target_entry_symbol(self.program, target))
                {
                    return None;
                }
                let machine = self.program.machine_holding_state(state.symbol)?;
                let carrier = crate::expression_result_type_reference(
                    self.program,
                    machine,
                    state,
                    expression,
                )?;
                let owner = self.facts.borrow.owner_segments.span(incoming.owner_path)?;
                let leaf = super::super::declared_input_reference(self.program, carrier, owner, 0)?;
                let mut selected = super::super::origin_place_projection(
                    self.program,
                    carrier,
                    leaf.path.iter().cloned(),
                )?;
                if selected.type_identity(self.program)
                    != self.program.normalized_type_identity(incoming.carrier_type)
                {
                    return None;
                }
                Some((carrier, leaf.path))
            }

            pub(crate) fn captured_parameter_origin(
                &self,
                state: &typed_trees::state::State,
                leaf: &checked_trees::CheckedReferenceResultSourcePlan,
                origin: &CapturedReturnOrigin,
            ) -> Option<checked_trees::CheckedReferenceOriginSourcePlan> {
                let (index, parameter) = self
                    .program
                    .state_parameters(state)
                    .iter()
                    .filter(|parameter| {
                        super::super::is_mutable_structural_parameter(self.program, parameter)
                    })
                    .enumerate()
                    .find(|(_, parameter)| parameter.symbol == origin.source.root)?;
                let ingress = match crate::reference_shell(self.program, parameter.type_reference) {
                    Some(shell) => {
                        if !super::super::reference_access_can_supply(
                            shell.access.into(),
                            leaf.source.access,
                        ) {
                            return None;
                        }
                        let referee = crate::peel_declared_constraints(self.program, shell.referee);
                        let view = matches!(
                            self.program.type_reference_table.type_reference(referee),
                            typed_trees::types::TypeReferenceNode::Slice { .. }
                        ) || crate::byte_sequence_carrier(
                            self.program,
                            parameter.type_reference,
                            &[],
                        )
                        .is_some_and(|carrier| carrier.is_borrowed());
                        if view {
                            parameter.type_reference
                        } else {
                            shell.referee
                        }
                    }
                    None => parameter.type_reference,
                };
                let mut captured = self.frame_origin_path(ingress, &[], &origin.source)?;
                let expected = origin
                    .presentation
                    .as_ref()
                    .map_or(leaf.source.type_identity, |presentation| {
                        presentation.source_type
                    });
                if self.program.normalized_type_identity(captured.presentation) != expected
                    && let Some(shell) = crate::reference_shell(self.program, captured.presentation)
                {
                    if !super::super::reference_access_can_supply(
                        shell.access.into(),
                        leaf.source.access,
                    ) || self.program.normalized_type_identity(captured.backing)
                        != self.program.normalized_type_identity(captured.presentation)
                    {
                        return None;
                    }
                    captured
                        .path
                        .push(checked_trees::CheckedReferenceOriginProjection::Exact(
                            checked_trees::CheckedUnitStructuralPathSegment::Referent,
                        ));
                    captured.backing = shell.referee;
                    captured.presentation = shell.referee;
                }
                self.publish_frame_origin(
                    state,
                    leaf,
                    origin,
                    captured,
                    u32::try_from(index).ok()?,
                    Vec::new(),
                )
            }

            pub(crate) fn incoming_leaf(
                &self,
                state: &typed_trees::state::State,
                loan: &checked_trees::BorrowReturnedLoanFact,
            ) -> Option<IncomingLeaf> {
                if !self
                    .facts
                    .borrow
                    .incoming_references
                    .is_valid(loan.incoming_reference)
                {
                    return None;
                }
                let incoming = self
                    .facts
                    .borrow
                    .incoming_references
                    .get(loan.incoming_reference);
                let symbol = incoming.parameter_symbol();
                if incoming.state_symbol != state.symbol
                    || incoming.machine_symbol != self.flow(state.symbol)?.machine_symbol
                    || loan.root != facts::PlaceRoot::Symbol(symbol)
                {
                    return None;
                }
                let parameter = self.program.state_parameter_by_symbol(state, symbol)?;
                let carrier =
                    super::super::input_carrier_type(self.program, parameter.type_reference)?;
                if carrier != incoming.carrier_type {
                    return None;
                }
                let owner = self.facts.borrow.owner_segments.span(incoming.owner_path)?;
                let reference = super::super::declared_carrier_references(self.program, carrier)?
                    .into_iter()
                    .find(|reference| {
                        super::super::origin_owner_path(
                            self.program,
                            carrier,
                            reference.path.iter().cloned(),
                        )
                        .as_deref()
                            == Some(owner)
                    })?;
                let segments = super::super::origin_place_segments(
                    self.program,
                    carrier,
                    reference.path.iter().cloned(),
                )?;
                Some(IncomingLeaf {
                    carrier,
                    reference,
                    segments,
                })
            }

            pub(crate) fn origin_matches_returned_loan(
                &self,
                state: &typed_trees::state::State,
                loan: &checked_trees::BorrowReturnedLoanFact,
                origin: &CapturedReturnOrigin,
            ) -> bool {
                let extent = origin.extent;
                let origin = &origin.source;
                if loan.lineage == checked_trees::BorrowLoanLineage::LifetimeDependency
                    || loan.root != facts::PlaceRoot::Symbol(origin.root)
                {
                    return false;
                }
                let Some(region) = self.facts.borrow.access_segments.span(loan.segments) else {
                    return false;
                };
                if self.trace_return_state(state) {
                    eprintln!(
                        "CHECKSITE completion-origin-region state={:?} source={:?} extent={:?} region={:?}",
                        state.symbol, origin, extent, region
                    );
                }
                if !loan.incoming_reference.is_valid() {
                    return frame_origin_fits_loan(origin, extent, region);
                }
                let Some(incoming) = self.incoming_leaf(state, loan) else {
                    return false;
                };
                let (root, captured) = origin
                    .recasts
                    .first()
                    .map_or((origin.root, origin.segments.as_slice()), |formation| {
                        (formation.root, formation.segments.as_slice())
                    });
                let Some(relative) = captured.strip_prefix(incoming.segments.as_slice()) else {
                    return false;
                };
                if root != origin.root {
                    return false;
                }
                let mut referent = origin.clone();
                referent.segments = relative.to_vec();
                referent.recasts.clear();
                frame_origin_fits_loan(&referent, extent, region)
            }
        }
    }
    pub(crate) mod input_views {
        use super::{CapturedReturnPresentation, Captures, CheckedReferenceOriginProjection};
        use arena::Handle;
        use checked_trees::{
            CheckedStructuralValueKind, CheckedUnitStructuralPathSegment, FlowOperandFact,
        };
        use semantic_vocabulary::ReferenceOriginExtent;
        use symbols::SymbolHandle;
        use typed_trees::types::TypeReferenceHandle;

        #[derive(PartialEq, Eq)]
        pub(crate) struct CapturedArgumentOrigin {
            pub(crate) source: crate::LocalWriteSource,
            pub(crate) presentation: CapturedReturnPresentation,
            pub(crate) extent: ReferenceOriginExtent,
        }

        impl<'capture> Captures<'capture> {
            #[allow(clippy::too_many_arguments)]
            pub(crate) fn argument_return_sources(
                &mut self,
                state: &typed_trees::state::State,
                reference: TypeReferenceHandle,
                operand: Handle<FlowOperandFact>,
                current: Handle<checked_trees::FlowStorageSnapshot>,
                relative: &crate::LocalWriteSource,
                presentation: &CapturedReturnPresentation,
                active: &mut Vec<SymbolHandle>,
                visiting: &mut Vec<Handle<FlowOperandFact>>,
            ) -> Option<Vec<CapturedArgumentOrigin>> {
                let control = &self.facts.flow.control;
                if !control.operands.is_valid(operand)
                    || visiting.contains(&operand)
                    || visiting.len() >= super::super::MAX_REFERENCE_ORIGIN_DEPTH
                {
                    return None;
                }
                if crate::reference_shell(self.program, reference).is_none() {
                    return self.ordinary_argument_sources(
                        state,
                        reference,
                        operand,
                        current,
                        relative,
                        presentation,
                        active,
                    );
                }
                let _actual = control.operands.get(operand);
                let stored = crate::completed_storage::reference_sources_at_path(
                    self.program,
                    control,
                    operand,
                    &[],
                    current,
                );
                let mut selections = Vec::new();
                descriptor_alternatives(stored, &mut selections)?;
                visiting.push(operand);
                let result = (|| {
                    let mut output = Vec::new();
                    for selection in selections {
                        let selection = match selection {
                            crate::completed_storage::StoredOperand::Incoming(place) => {
                                let source = self.incoming_argument_source(
                                    state,
                                    reference,
                                    place,
                                    relative,
                                    presentation,
                                )?;
                                if !output.contains(&source) {
                                    output.push(source);
                                }
                                if output.len() > super::super::MAX_REFERENCE_ORIGIN_LEAVES {
                                    return None;
                                }
                                continue;
                            }
                            crate::completed_storage::StoredOperand::Value {
                                capture,
                                reference,
                                projection,
                                slot,
                            } => crate::completed_storage::StoredCapture {
                                capture,
                                reference,
                                projection,
                                slot,
                            },
                            crate::completed_storage::StoredOperand::Unavailable
                            | crate::completed_storage::StoredOperand::Alternatives(_) => {
                                return None;
                            }
                        };
                        let capture = control.operands.get(selection.capture);
                        let (path, _) = super::super::source_paths::structural_path(
                            self.program,
                            capture.type_reference,
                            &selection.projection,
                        )?;
                        let descriptor = self.completed_operand_source(
                            self.completed_operand_leaf(capture, &path)?,
                        )?;
                        let enclosure = if control.operand_retentions.is_valid(descriptor.retention)
                        {
                            control
                                .operand_retentions
                                .get(descriptor.retention)
                                .enclosing_storage
                        } else {
                            Handle::invalid()
                        };
                        let values = &self.facts.values.structural_values;
                        let formation =
                            values.with_state_nodes(descriptor.state_symbol, |handles| {
                                let mut nodes = handles
                                    .iter()
                                    .map(|handle| values.nodes.get(*handle))
                                    .filter(|value| {
                                        value.expression == descriptor.expression
                                            && value.statement_index as usize
                                                == descriptor.statement_index
                                    });
                                let value = nodes.next()?;
                                nodes.next().is_none().then_some(value)
                            });
                        let view = formation.filter(|value| {
                            matches!(
                                value.kind,
                                CheckedStructuralValueKind::Reference {
                                    storage: checked_trees::CheckedReferenceStorage::View { .. },
                                    ..
                                }
                            )
                        });
                        if let Some(view) = view {
                            if !control.operands.is_valid(enclosure)
                                || view.state != descriptor.state_symbol
                                || view.machine
                                    != self.flow(descriptor.state_symbol)?.machine_symbol
                                || view.statement_index as usize != descriptor.statement_index
                                || self.program.normalized_type_identity(view.type_reference)
                                    != self
                                        .program
                                        .normalized_type_identity(descriptor.type_reference)
                            {
                                return None;
                            }
                            let CheckedStructuralValueKind::Reference {
                                storage,
                                backing_type,
                                ref source,
                                ..
                            } = view.kind
                            else {
                                return None;
                            };
                            let physical = control.operands.get(enclosure);
                            let physical_shell =
                                crate::reference_shell(self.program, physical.type_reference)?;
                            let target =
                                crate::reference_shell(self.program, view.type_reference)?.referee;
                            if source.type_identity
                                != crate::stored_structural_type_identity(
                                    self.program,
                                    backing_type,
                                    &[],
                                    &[],
                                )
                                || self
                                    .program
                                    .normalized_type_identity(physical_shell.referee)
                                    != self.program.normalized_type_identity(backing_type)
                            {
                                return None;
                            }
                            let (suffix, endpoint, extent) = super::super::reference_origin_path(
                                self.program,
                                target,
                                &relative.segments,
                            )?;
                            if self.program.normalized_type_identity(endpoint)
                                != presentation.source_type
                            {
                                return None;
                            }
                            let mut path = vec![CheckedReferenceOriginProjection::Exact(
                                CheckedUnitStructuralPathSegment::RepresentationView {
                                    type_identity: crate::stored_structural_type_identity(
                                        self.program,
                                        target,
                                        &[],
                                        &[],
                                    ),
                                    storage,
                                },
                            )];
                            path.extend(suffix);
                            path.extend(presentation.path.iter().cloned());
                            let presented = CapturedReturnPresentation {
                                path,
                                source_type: self.program.normalized_type_identity(backing_type),
                                endpoint_type: presentation.endpoint_type,
                                result_type: presentation.result_type,
                                extent: presentation.extent,
                            };
                            let mut physical_relative = relative.clone();
                            physical_relative.segments.clear();
                            for mut source in self.argument_return_sources(
                                state,
                                physical.type_reference,
                                enclosure,
                                current,
                                &physical_relative,
                                &presented,
                                active,
                                visiting,
                            )? {
                                if extent == ReferenceOriginExtent::WithinRegion {
                                    source.extent = extent;
                                }
                                if !output.contains(&source) {
                                    output.push(source);
                                }
                            }
                        } else {
                            if enclosure.is_valid() && formation.is_none() {
                                return None;
                            }
                            let sources = if selection.projection.is_empty() {
                                self.captured_input_sources(
                                    state,
                                    reference,
                                    selection.capture,
                                    current,
                                    relative,
                                    active,
                                )
                            } else {
                                self.completed_descriptor_sources(
                                    state,
                                    selection.capture,
                                    &selection.projection,
                                    reference,
                                    relative,
                                    current,
                                    &mut Vec::new(),
                                )
                            };
                            let sources = match sources {
                                Some(sources) => sources
                                    .into_iter()
                                    .map(|source| CapturedArgumentOrigin {
                                        source,
                                        presentation: presentation.clone(),
                                        extent: ReferenceOriginExtent::Projected,
                                    })
                                    .collect(),
                                None => self.invocation_argument_sources(
                                    state,
                                    reference,
                                    descriptor,
                                    relative,
                                    presentation,
                                    active,
                                    visiting,
                                )?,
                            };
                            for source in sources {
                                if !output.contains(&source) {
                                    output.push(source);
                                }
                            }
                        }
                        if output.len() > super::super::MAX_REFERENCE_ORIGIN_LEAVES {
                            return None;
                        }
                    }
                    Some(output)
                })();
                visiting.pop();
                result
            }

            fn incoming_argument_source(
                &self,
                state: &typed_trees::state::State,
                reference: TypeReferenceHandle,
                place: crate::CanonicalPlace,
                relative: &crate::LocalWriteSource,
                presentation: &CapturedReturnPresentation,
            ) -> Option<CapturedArgumentOrigin> {
                let root = place.root.as_symbol()?;
                if !self.ingress_present(state, root)? {
                    return None;
                }
                let parameter = self.program.state_parameter_by_symbol(state, root)?;
                let slot = crate::project_type_reference_from_segments(
                    self.program,
                    parameter.type_reference,
                    &place.segments,
                )?;
                let supplied = crate::reference_shell(self.program, slot)?;
                let expected = crate::reference_shell(self.program, reference)?;
                if !crate::storage_origin_types_match(
                    self.program,
                    supplied.referee,
                    expected.referee,
                ) || !super::super::reference_access_can_supply(
                    supplied.access.into(),
                    expected.access.into(),
                ) {
                    return None;
                }
                let source = crate::LocalWriteSource {
                    invocation_output: None,
                    invocation_result: None,
                    is_image_storage: false,
                    recasts: Vec::new(),
                    representation: None,
                    path: self.program.symbols.name(root).to_owned(),
                    collection_coarse: false,
                    root,
                    segments: place.segments,
                }
                .substitute_storage(self.program, relative)?;
                Some(CapturedArgumentOrigin {
                    source,
                    presentation: presentation.clone(),
                    extent: ReferenceOriginExtent::Projected,
                })
            }

            #[allow(clippy::too_many_arguments)]
            fn ordinary_argument_sources(
                &mut self,
                state: &typed_trees::state::State,
                reference: TypeReferenceHandle,
                operand: Handle<FlowOperandFact>,
                current: Handle<checked_trees::FlowStorageSnapshot>,
                relative: &crate::LocalWriteSource,
                presentation: &CapturedReturnPresentation,
                active: &mut Vec<SymbolHandle>,
            ) -> Option<Vec<CapturedArgumentOrigin>> {
                Some(
                    self.captured_input_sources(
                        state, reference, operand, current, relative, active,
                    )?
                    .into_iter()
                    .map(|source| CapturedArgumentOrigin {
                        source,
                        presentation: presentation.clone(),
                        extent: ReferenceOriginExtent::Projected,
                    })
                    .collect(),
                )
            }
        }

        fn descriptor_alternatives(
            selected: crate::completed_storage::StoredOperand,
            output: &mut Vec<crate::completed_storage::StoredOperand>,
        ) -> Option<()> {
            use crate::completed_storage::StoredOperand;
            match selected {
                StoredOperand::Unavailable => return None,
                StoredOperand::Alternatives(alternatives) => {
                    if alternatives.is_empty() {
                        return None;
                    }
                    for alternative in alternatives {
                        descriptor_alternatives(alternative, output)?;
                    }
                }
                selected => output.push(selected),
            }
            Some(())
        }
    }
    pub(crate) mod invocation_inputs {
        use super::input_views::CapturedArgumentOrigin;
        use super::{CapturedReturnPresentation, Captures, SymbolHandle};
        use arena::Handle;
        use checked_trees::{
            CheckedReferenceOriginProjection, CheckedReferenceOriginRoot,
            CheckedUnitStructuralPathSegment, FlowOperandCompletion, FlowOperandFact,
        };
        use semantic_vocabulary::ReferenceOriginExtent;

        struct CapturedInvocationInput {
            operand: Handle<FlowOperandFact>,
            reference: typed_trees::types::TypeReferenceHandle,
            source: crate::LocalWriteSource,
            presentation: CapturedReturnPresentation,
        }

        impl<'capture> Captures<'capture> {
            pub(super) fn invocation_borrow_sources(
                &mut self,
                state: &typed_trees::state::State,
                descriptor: &FlowOperandFact,
            ) -> Option<(bool, Vec<super::CapturedInvocationBorrowSource>)> {
                let FlowOperandCompletion::InvocationResult { invocation, .. } =
                    descriptor.completion
                else {
                    return None;
                };
                let call =
                    self.calls
                        .invocation(&self.facts.flow.control, state.symbol, invocation)?;
                if !crate::call_target_has_checked_body_contract(self.program, call.target_symbol) {
                    return None;
                }
                let shell = crate::reference_shell(self.program, descriptor.type_reference)?;
                let referee = self.program.normalized_type_identity(shell.referee);
                let relative = crate::LocalWriteSource {
                    invocation_output: None,
                    invocation_result: None,
                    is_image_storage: false,
                    recasts: Vec::new(),
                    representation: None,
                    path: String::new(),
                    collection_coarse: false,
                    root: SymbolHandle::invalid(),
                    segments: Vec::new(),
                };
                let presentation = CapturedReturnPresentation {
                    path: Vec::new(),
                    source_type: referee,
                    endpoint_type: referee,
                    result_type: referee,
                    extent: ReferenceOriginExtent::Projected,
                };
                let (origins, inputs) = self.invocation_argument_sources_with_inputs(
                    state,
                    descriptor.type_reference,
                    descriptor,
                    &relative,
                    &presentation,
                    &mut Vec::new(),
                    &mut Vec::new(),
                    true,
                )?;
                let mut output = Vec::new();
                for input in inputs {
                    if !input.presentation.path.is_empty() {
                        return None;
                    }
                    let outer_reference =
                        crate::reference_shell(self.program, input.reference).is_some();
                    let (source_path, projection) = if let Some(boundary) =
                        super::input_descriptor_boundary(
                            self.program,
                            input.reference,
                            &input.source.segments,
                        )? {
                        let actual = self.facts.flow.control.operands.get(input.operand);
                        (
                            super::super::loan_place_segments(
                                self.program,
                                actual.type_reference,
                                &boundary.path,
                            )?,
                            input
                                .source
                                .segments
                                .strip_prefix(boundary.prefix.as_slice())?
                                .to_vec(),
                        )
                    } else {
                        if !outer_reference {
                            return None;
                        }
                        (Vec::new(), input.source.segments)
                    };
                    output.push(super::CapturedInvocationBorrowSource {
                        operand: input.operand,
                        source_path,
                        projection,
                        extent: input.presentation.extent,
                    });
                }
                Some((!origins.is_empty(), output))
            }

            #[allow(clippy::too_many_arguments)]
            pub(crate) fn invocation_argument_sources(
                &mut self,
                state: &typed_trees::state::State,
                reference: typed_trees::types::TypeReferenceHandle,
                descriptor: &FlowOperandFact,
                relative: &crate::LocalWriteSource,
                presentation: &CapturedReturnPresentation,
                active: &mut Vec<SymbolHandle>,
                visiting: &mut Vec<Handle<FlowOperandFact>>,
            ) -> Option<Vec<CapturedArgumentOrigin>> {
                self.invocation_argument_sources_with_inputs(
                    state,
                    reference,
                    descriptor,
                    relative,
                    presentation,
                    active,
                    visiting,
                    false,
                )
                .map(|(sources, _)| sources)
            }

            #[allow(clippy::too_many_arguments)]
            fn invocation_argument_sources_with_inputs(
                &mut self,
                state: &typed_trees::state::State,
                reference: typed_trees::types::TypeReferenceHandle,
                descriptor: &FlowOperandFact,
                relative: &crate::LocalWriteSource,
                presentation: &CapturedReturnPresentation,
                active: &mut Vec<SymbolHandle>,
                visiting: &mut Vec<Handle<FlowOperandFact>>,
                retain_inputs: bool,
            ) -> Option<(Vec<CapturedArgumentOrigin>, Vec<CapturedInvocationInput>)> {
                let FlowOperandCompletion::InvocationResult {
                    invocation,
                    result_type,
                    path,
                } = descriptor.completion
                else {
                    return None;
                };
                let result = (|| {
                    let control = &self.facts.flow.control;
                    let call = self
                        .calls
                        .invocation(&self.facts.flow.control, state.symbol, invocation)
                        .ok_or("returned descriptor has no exact performed invocation")?;
                    if descriptor.state_symbol != state.symbol
                        || descriptor.statement_index != call.statement_index
                        || descriptor.expression != call.authored_expression
                    {
                        return Err("returned descriptor differs from its invocation occurrence");
                    }
                    let declared = crate::call_target_return_type(self.program, call.target_symbol)
                        .ok_or("returned invocation has no result contract")?;
                    if self.program.normalized_type_identity(declared)
                        != self.program.normalized_type_identity(result_type)
                    {
                        return Err("returned descriptor changes its invocation result type");
                    }
                    let selected_path = control
                        .operand_retained_paths
                        .span(path)
                        .ok_or("returned descriptor has an unavailable result path")?;
                    let selected = crate::project_type_reference_from_segments(
                        self.program,
                        declared,
                        selected_path,
                    )
                    .ok_or("returned descriptor has no typed result leaf")?;
                    if self.program.normalized_type_identity(selected)
                        != self
                            .program
                            .normalized_type_identity(descriptor.type_reference)
                    {
                        return Err("returned descriptor changes its selected result leaf");
                    }
                    let shell = crate::reference_shell(self.program, selected)
                        .ok_or("returned invocation leaf is not a reference")?;
                    let expected = crate::reference_shell(self.program, reference)
                        .ok_or("returned invocation input is not a reference")?;
                    if !crate::storage_origin_types_match(
                        self.program,
                        shell.referee,
                        expected.referee,
                    ) || !super::super::reference_access_can_supply(
                        shell.access.into(),
                        expected.access.into(),
                    ) {
                        return Err("returned invocation changes the input's referent or access");
                    }
                    let (suffix, endpoint, extent) = super::super::reference_origin_path(
                        self.program,
                        shell.referee,
                        &relative.segments,
                    )
                    .ok_or("returned invocation has no typed consuming projection")?;
                    if suffix.iter().any(|segment| {
                        matches!(
                            segment,
                            CheckedReferenceOriginProjection::Exact(
                                CheckedUnitStructuralPathSegment::Referent
                            )
                        )
                    }) {
                        return Err(
                            "returned invocation cannot replace a current descriptor-slot read",
                        );
                    }
                    if self.program.normalized_type_identity(endpoint) != presentation.source_type {
                        return Err(
                            "returned invocation projection changes its receiving presentation",
                        );
                    }
                    let parameters =
                        crate::call_target_parameters(self.program, call.target_symbol)
                            .ok_or("returned invocation has no parameter roster")?;
                    let operands = control
                        .operands
                        .span(call.argument_operands())
                        .ok_or("returned invocation has no complete actual captures")?;
                    if parameters.len() != operands.len() {
                        return Err("returned invocation changes its actual parameter roster");
                    }
                    let sources = self
                        .target_sources(call.target_symbol, active)
                        .ok_or("returned invocation has no checked origin relation")?;
                    let mut output = Vec::new();
                    let mut captured_inputs = Vec::new();
                    for candidate in sources {
                        if super::loan_place_segments(self.program, declared, &candidate.path)
                            .ok_or("returned invocation origin has no result path")?
                            != selected_path
                        {
                            continue;
                        }
                        if !super::super::reference_access_can_supply(
                            candidate.source.access,
                            shell.access.into(),
                        ) {
                            return Err("returned invocation origin changes reference access");
                        }
                        let CheckedReferenceOriginRoot::Parameter { index } = candidate.source.root
                        else {
                            return Err("returned invocation has an independent, non-input origin");
                        };
                        let (position, parameter) =
                            super::super::origin_formal(self.program, parameters, index)
                                .ok_or("returned invocation origin has no matching input")?;
                        let input = call
                            .argument_operands()
                            .handles()
                            .nth(position)
                            .ok_or("returned invocation origin has no matching input")?;
                        let operand = &operands[position];
                        if operand.state_symbol != state.symbol
                            || operand.statement_index != call.statement_index
                        {
                            return Err("returned invocation input belongs to another evaluation");
                        }
                        let (source, mut composed) = self
                            .checked_input_origin(
                                parameter,
                                &candidate.source,
                                self.program.normalized_type_identity(shell.referee),
                            )
                            .ok_or("returned invocation input has no checked typed projection")?;
                        composed.path.extend(suffix.iter().cloned());
                        composed.path.extend(presentation.path.iter().cloned());
                        composed.endpoint_type = presentation.endpoint_type;
                        composed.result_type = presentation.result_type;
                        if extent == ReferenceOriginExtent::WithinRegion
                            || presentation.extent == ReferenceOriginExtent::WithinRegion
                        {
                            composed.extent = ReferenceOriginExtent::WithinRegion;
                        }
                        let inputs = self
                            .argument_return_sources(
                                state,
                                parameter.type_reference,
                                input,
                                call.invocation_storage,
                                &source,
                                &composed,
                                active,
                                visiting,
                            )
                            .ok_or("returned invocation input has no complete captured origins")?;
                        if retain_inputs {
                            captured_inputs.push(CapturedInvocationInput {
                                operand: input,
                                reference: parameter.type_reference,
                                source,
                                presentation: composed.clone(),
                            });
                        }
                        for mut input in inputs {
                            if composed.extent == ReferenceOriginExtent::WithinRegion {
                                input.extent = ReferenceOriginExtent::WithinRegion;
                            }
                            if !output.contains(&input) {
                                output.push(input);
                            }
                            if output.len() > super::super::MAX_REFERENCE_ORIGIN_LEAVES {
                                return Err("returned invocation exceeds the origin roster limit");
                            }
                        }
                    }
                    Ok((output, captured_inputs))
                })();
                if let Err(reason) = &result
                    && crate::trace_flags::check_sites()
                {
                    eprintln!(
                        "CHECKSITE returned-invocation-input state={:?} statement={} expression={:?} invocation={invocation:?} path={path:?} refusal={reason}",
                        state.symbol, descriptor.statement_index, descriptor.expression,
                    );
                }
                result.ok()
            }
        }
    }
    pub(crate) mod invocation_returns {
        use super::super::MAX_REFERENCE_ORIGIN_LEAVES;
        use super::{
            CapturedReturnOrigin, CapturedReturnPresentation, Captures,
            CheckedReferenceOriginProjection, CheckedReturnedReferencePlan,
            CheckedUnitStructuralPathSegment, SymbolHandle, TypeReferenceHandle, loan_owner_path,
            loan_place_segments,
        };

        impl<'capture> Captures<'capture> {
            pub(crate) fn invocation_return_sources(
                &mut self,
                state: &typed_trees::state::State,
                transition_target: typed_trees::statement::TransitionTargetHandle,
                call: &checked_trees::FlowCallFact,
                loans: &[checked_trees::BorrowReturnedLoanFact],
                active: &mut Vec<SymbolHandle>,
            ) -> Option<Vec<CheckedReturnedReferencePlan>> {
                let relative = self.target_sources(call.target_symbol, active)?;
                let leaves = self.reference_leaves(state)?;
                if self.trace_return_state(state) {
                    eprintln!(
                        "CHECKSITE completion-invocation-origins state={:?} call={:?} relative={:?} leaves={:?}",
                        state.symbol,
                        call,
                        relative,
                        leaves
                            .iter()
                            .map(|leaf| (&leaf.path, &leaf.source))
                            .collect::<Vec<_>>()
                    );
                }
                let return_type = crate::call_target_return_type(self.program, call.target_symbol)?;
                let target_leaves = super::super::record_carrier_sources(
                    self.program,
                    return_type,
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                        parameter_index: 0,
                    },
                )?;
                let parameters = crate::call_target_parameters(self.program, call.target_symbol)?;
                let operands = self
                    .facts
                    .flow
                    .control
                    .operands
                    .span(call.argument_operands())?;
                if parameters.len() != operands.len() {
                    return None;
                }
                let mut origins = Vec::new();
                let mut independent = Vec::new();
                for candidate in relative {
                    let leaf = leaves.iter().find(|leaf| leaf.path == candidate.path)?;
                    if candidate.source.access != leaf.source.access {
                        return None;
                    }
                    let result_path =
                        loan_place_segments(self.program, state.return_type, &candidate.path)?;
                    let checked_trees::CheckedReferenceOriginRoot::Parameter { index } =
                        &candidate.source.root
                    else {
                        independent.push(candidate);
                        continue;
                    };
                    let (position, parameter) =
                        super::super::origin_formal(self.program, parameters, *index)?;
                    let operand = operands.get(position)?;
                    if operand.state_symbol != state.symbol
                        || operand.statement_index != call.statement_index
                    {
                        return None;
                    }
                    let target_leaf = target_leaves
                        .iter()
                        .find(|leaf| leaf.path == candidate.path)?;
                    let input_origin = self.checked_input_origin(
                        parameter,
                        &candidate.source,
                        target_leaf.source.type_identity,
                    );
                    if input_origin.is_none() && self.trace_return_state(state) {
                        eprintln!(
                            "CHECKSITE completion-input-origin state={:?} parameter={:?} candidate={:?} result_type={:?}",
                            state.symbol, parameter, candidate, target_leaf.source.type_identity
                        );
                    }
                    let (relative, presentation) = input_origin?;
                    let inputs = self.argument_return_sources(
                        state,
                        parameter.type_reference,
                        call.argument_operands().handles().nth(position)?,
                        call.invocation_storage,
                        &relative,
                        &presentation,
                        active,
                        &mut Vec::new(),
                    );
                    if inputs.is_none() && self.trace_return_state(state) {
                        eprintln!(
                            "CHECKSITE completion-input-sources state={:?} parameter={:?} operand={:?} relative={:?}",
                            state.symbol, parameter, operand, relative
                        );
                    }
                    for input in inputs? {
                        let origin = CapturedReturnOrigin {
                            result_path: result_path.clone(),
                            source: input.source,
                            extent: if candidate.source.extent
                                == semantic_vocabulary::ReferenceOriginExtent::WithinRegion
                                || input.extent
                                    == semantic_vocabulary::ReferenceOriginExtent::WithinRegion
                            {
                                semantic_vocabulary::ReferenceOriginExtent::WithinRegion
                            } else {
                                semantic_vocabulary::ReferenceOriginExtent::Projected
                            },
                            presentation: Some(input.presentation),
                        };
                        if !origins.contains(&origin) {
                            origins.push(origin);
                            if origins.len() > MAX_REFERENCE_ORIGIN_LEAVES {
                                return None;
                            }
                        }
                    }
                }
                let site_loans: Vec<_> = loans
                    .iter()
                    .filter(|loan| {
                        loan.lineage != checked_trees::BorrowLoanLineage::LifetimeDependency
                            && loan.statement_index == call.statement_index
                            && loan.transition_target == transition_target
                    })
                    .cloned()
                    .collect();
                let mut output = Vec::new();
                for leaf in leaves {
                    let result_path =
                        loan_place_segments(self.program, state.return_type, &leaf.path)?;
                    let owner_path = crate::corresponding_loan_owner_path(
                        self.program,
                        state.return_type,
                        &loan_owner_path(self.program, state.return_type, &leaf.path)?,
                    )?;
                    let mut captured = false;
                    for candidate in independent
                        .iter()
                        .filter(|candidate| candidate.path == leaf.path)
                    {
                        let checked_trees::CheckedReferenceOriginRoot::Invocation { carrier_type } =
                            &candidate.source.root
                        else {
                            return None;
                        };
                        if candidate.source.entry_reference_hops != 0 {
                            return None;
                        }
                        let mut supplied = false;
                        for loan in site_loans.iter().filter(|loan| {
                            self.returned_loan_owner_path(state, loan).as_deref()
                                == Some(owner_path.as_slice())
                                && self
                                    .facts
                                    .borrow
                                    .incoming_references
                                    .is_valid(loan.incoming_reference)
                        }) {
                            let incoming = self
                                .facts
                                .borrow
                                .incoming_references
                                .get(loan.incoming_reference);
                            if incoming.state_symbol != state.symbol
                                || !matches!(incoming.origin, checked_trees::IncomingReferenceOrigin::Invocation { expression, target }
                            if expression == call.authored_expression
                                && crate::call_target_entry_symbol(self.program, target) == call.target_symbol)
                                || leaf.source.access.borrow_access_kind()? != loan.kind
                            {
                                continue;
                            }
                            let (invocation_carrier, descriptor_path) =
                                self.invocation_carrier(state, incoming)?;
                            let prefix = descriptor_path;
                            if self.program.normalized_type_identity(invocation_carrier)
                                != *carrier_type
                                || !candidate.source.path.starts_with(&prefix)
                            {
                                continue;
                            }
                            let mut projection = super::super::origin_place_projection(
                                self.program,
                                invocation_carrier,
                                candidate.source.path.iter().cloned(),
                            )?;
                            if projection.type_identity(self.program)
                                != candidate.source.type_identity
                            {
                                return None;
                            }
                            for mut source in self.return_origin(
                                state,
                                loan,
                                &leaf,
                                self.facts.borrow.access_segments.span(loan.segments)?,
                                &site_loans,
                            )? {
                                if candidate.source.extent
                                    == semantic_vocabulary::ReferenceOriginExtent::WithinRegion
                                {
                                    source.extent =
                                        semantic_vocabulary::ReferenceOriginExtent::WithinRegion;
                                }
                                let returned = CheckedReturnedReferencePlan {
                                    path: leaf.path.clone(),
                                    source,
                                };
                                if !output.contains(&returned) {
                                    output.push(returned);
                                }
                                supplied = true;
                            }
                        }
                        if !supplied {
                            return None;
                        }
                        captured = true;
                    }
                    let leaf_origins = origins
                        .iter()
                        .filter(|origin| origin.result_path == result_path);
                    let mut invocation_input = false;
                    if leaf_origins.clone().next().is_some() {
                        for loan in site_loans.iter().filter(|loan| {
                            self.returned_loan_owner_path(state, loan).as_deref()
                                == Some(owner_path.as_slice())
                                && self
                                    .facts
                                    .borrow
                                    .incoming_references
                                    .is_valid(loan.incoming_reference)
                        }) {
                            let incoming = self
                                .facts
                                .borrow
                                .incoming_references
                                .get(loan.incoming_reference);
                            if incoming.state_symbol != state.symbol
                                || loan.root
                                    != facts::PlaceRoot::Expression(call.authored_expression)
                                || !matches!(incoming.origin, checked_trees::IncomingReferenceOrigin::Invocation { expression, target }
                            if expression == call.authored_expression
                                && crate::call_target_entry_symbol(self.program, target) == call.target_symbol)
                                || leaf.source.access.borrow_access_kind()? != loan.kind
                            {
                                continue;
                            }
                            let (carrier, descriptor) = self.invocation_carrier(state, incoming)?;
                            if self.program.normalized_type_identity(carrier)
                                != self.program.normalized_type_identity(return_type)
                                || descriptor
                                    != leaf
                                        .path
                                        .iter()
                                        .cloned()
                                        .map(CheckedReferenceOriginProjection::Exact)
                                        .collect::<Vec<_>>()
                            {
                                return None;
                            }
                            invocation_input = true;
                        }
                    }
                    for origin in leaf_origins {
                        let mut covered = invocation_input;
                        if invocation_input {
                            let source = self.captured_parameter_origin(state, &leaf, origin)?;
                            let returned = CheckedReturnedReferencePlan {
                                path: leaf.path.clone(),
                                source,
                            };
                            if !output.contains(&returned) {
                                output.push(returned);
                            }
                        }
                        for loan in site_loans.iter().filter(|loan| {
                            loan.root == facts::PlaceRoot::Symbol(origin.source.root)
                                && self.returned_loan_owner_path(state, loan).as_deref()
                                    == Some(owner_path.as_slice())
                                && self.origin_matches_returned_loan(state, loan, origin)
                        }) {
                            if leaf.source.access.borrow_access_kind()? != loan.kind {
                                return None;
                            }
                            covered = true;
                            for source in self.return_origin_with_frame(
                                state,
                                loan,
                                &leaf,
                                self.facts.borrow.access_segments.span(loan.segments)?,
                                &site_loans,
                                Some(&origins),
                            )? {
                                let returned = CheckedReturnedReferencePlan {
                                    path: leaf.path.clone(),
                                    source,
                                };
                                if !output.contains(&returned) {
                                    output.push(returned);
                                }
                            }
                        }
                        if !covered {
                            if self.trace_return_state(state) {
                                eprintln!(
                                    "CHECKSITE completion-uncovered-origin state={:?} leaf={:?} source={:?} extent={:?} loans={:?} loan_segments={:?}",
                                    state.symbol,
                                    leaf.path,
                                    origin.source,
                                    origin.extent,
                                    site_loans,
                                    site_loans
                                        .iter()
                                        .map(|loan| self
                                            .facts
                                            .borrow
                                            .access_segments
                                            .span(loan.segments))
                                        .collect::<Vec<_>>()
                                );
                            }
                            return None;
                        }
                        captured = true;
                    }
                    if !captured && self.call_paths(call, active)?.contains(&leaf.path) {
                        return None;
                    }
                }
                Some(output)
            }

            pub(crate) fn checked_input_origin(
                &self,
                parameter: &typed_trees::signature::StateParameter,
                origin: &checked_trees::CheckedReferenceOriginSourcePlan,
                result_type: typed_trees::type_identity::TypeIdentityId,
            ) -> Option<(crate::LocalWriteSource, CapturedReturnPresentation)> {
                let ingress =
                    super::super::stored_loan_parts(self.program, parameter.type_reference)
                        .map_or(parameter.type_reference, |(referent, _)| referent);
                let mut entered = parameter.type_reference;
                let mut path = origin.path.as_slice();
                for _ in 0..origin.entry_reference_hops {
                    let (
                        CheckedReferenceOriginProjection::Exact(
                            CheckedUnitStructuralPathSegment::Referent,
                        ),
                        remaining,
                    ) = path.split_first()?
                    else {
                        return None;
                    };
                    entered = crate::reference_shell(self.program, entered)?.referee;
                    path = remaining;
                }
                if origin.entry_reference_hops != 0
                    && self.program.normalized_type_identity(entered)
                        != self.program.normalized_type_identity(ingress)
                {
                    return None;
                }
                let interpretation = path
                    .iter()
                    .position(|projection| {
                        matches!(
                            projection,
                            CheckedReferenceOriginProjection::Exact(
                                CheckedUnitStructuralPathSegment::RepresentationView { .. }
                            )
                        )
                    })
                    .unwrap_or(path.len());
                let (physical, presentation) = path.split_at(interpretation);
                let mut projected = super::super::origin_place_projection(
                    self.program,
                    ingress,
                    physical.iter().cloned(),
                )?;
                let source_type = projected.type_identity(self.program);
                if !source_type.is_valid()
                    || (presentation.is_empty() && source_type != origin.type_identity)
                {
                    return None;
                }
                let source = crate::LocalWriteSource {
                    invocation_output: None,
                    invocation_result: None,
                    is_image_storage: false,
                    recasts: Vec::new(),
                    representation: None,
                    path: self.program.symbols.name(parameter.symbol).to_owned(),
                    collection_coarse: false,
                    root: parameter.symbol,
                    segments: projected.segments,
                };
                Some((
                    source,
                    CapturedReturnPresentation {
                        path: presentation.to_vec(),
                        source_type,
                        endpoint_type: origin.type_identity,
                        result_type,
                        extent: origin.extent,
                    },
                ))
            }

            pub(crate) fn captured_input_sources(
                &mut self,
                state: &typed_trees::state::State,
                reference: TypeReferenceHandle,
                operand_handle: arena::Handle<checked_trees::FlowOperandFact>,
                current: arena::Handle<checked_trees::FlowStorageSnapshot>,
                relative: &crate::LocalWriteSource,
                active: &mut Vec<SymbolHandle>,
            ) -> Option<Vec<crate::LocalWriteSource>> {
                let actual = self.facts.flow.control.operands.get(operand_handle);
                let mut completed = *self.completed_operand_source(actual)?;
                completed.storage_snapshot = current;
                let operand = &completed;
                let mut relative = relative.clone();
                let selected = operand;
                let outer_reference = crate::reference_shell(self.program, reference).is_some();
                let carrier =
                    super::super::input_carrier_type(self.program, reference).unwrap_or(reference);
                if let Some(boundary) =
                    super::input_descriptor_boundary(self.program, reference, &relative.segments)?
                {
                    let path = boundary.path;
                    let segments = boundary.prefix;
                    if !self.expression_leaf_present(
                        state,
                        operand.statement_index,
                        operand.expression,
                        reference,
                        &path,
                        Some(operand),
                        active,
                    )? {
                        return Some(Vec::new());
                    }
                    relative.segments = relative
                        .segments
                        .strip_prefix(segments.as_slice())?
                        .to_vec();
                    let expected = crate::project_type_reference_from_segments(
                        self.program,
                        carrier,
                        &segments,
                    )?;
                    let actual_path =
                        loan_place_segments(self.program, actual.type_reference, &path)?;
                    return self.completed_descriptor_sources(
                        state,
                        operand_handle,
                        &actual_path,
                        expected,
                        &relative,
                        current,
                        &mut Vec::new(),
                    );
                }
                if !outer_reference {
                    return None;
                }
                let referents = self
                    .facts
                    .flow
                    .control
                    .operand_referents
                    .span(selected.referents)?;
                if referents.is_empty() {
                    return self.completed_descriptor_sources(
                        state,
                        operand_handle,
                        &[],
                        reference,
                        &relative,
                        current,
                        &mut Vec::new(),
                    );
                }
                let mut output = Vec::new();
                for referent in referents {
                    let root = referent.root.as_symbol()?;
                    let source = crate::LocalWriteSource {
                        invocation_output: None,
                        invocation_result: None,
                        is_image_storage: false,
                        recasts: Vec::new(),
                        representation: None,
                        path: self.program.symbols.name(root).to_owned(),
                        collection_coarse: false,
                        root,
                        segments: self
                            .facts
                            .flow
                            .control
                            .operand_referent_segments
                            .span(referent.segments)?
                            .to_vec(),
                    };
                    let source = source.substitute_storage(self.program, &relative)?;
                    if !output.contains(&source) {
                        output.push(source);
                    }
                }
                Some(output)
            }

            pub(crate) fn completed_operand_source(
                &self,
                mut operand: &'capture checked_trees::FlowOperandFact,
            ) -> Option<&'capture checked_trees::FlowOperandFact> {
                let mut visited = Vec::new();
                loop {
                    let source = if operand.value_source.is_valid() {
                        operand.value_source
                    } else if crate::reference_shell(self.program, operand.type_reference).is_none()
                        && self
                            .facts
                            .flow
                            .control
                            .operand_retentions
                            .is_valid(operand.retention)
                    {
                        let retained = self
                            .facts
                            .flow
                            .control
                            .operand_retentions
                            .get(operand.retention);
                        if !retained.stored_projection.is_empty() {
                            break;
                        }
                        retained.stored_value
                    } else {
                        break;
                    };
                    if !source.is_valid() {
                        break;
                    }
                    if visited.contains(&source)
                        || !self.facts.flow.control.operands.is_valid(source)
                    {
                        return None;
                    }
                    visited.push(source);
                    let value = self.facts.flow.control.operands.get(source);
                    if !crate::storage_origin_types_match(
                        self.program,
                        operand.type_reference,
                        value.type_reference,
                    ) {
                        return None;
                    }
                    operand = value;
                }
                Some(operand)
            }

            pub(crate) fn completed_operand_leaf(
                &self,
                mut operand: &'capture checked_trees::FlowOperandFact,
                mut path: &[CheckedUnitStructuralPathSegment],
            ) -> Option<&'capture checked_trees::FlowOperandFact> {
                while !path.is_empty() {
                    operand = self.completed_operand_source(operand)?;
                    let mut selected = None;
                    for child in self
                        .facts
                        .flow
                        .control
                        .operand_children
                        .span(operand.constructed_children())?
                    {
                        let (destination, _) = super::super::source_paths::structural_path(
                            self.program,
                            operand.type_reference,
                            self.facts
                                .flow
                                .control
                                .operand_child_paths
                                .span(child.path)?,
                        )?;
                        if !destination.is_empty() && path.starts_with(&destination) {
                            if selected.is_some()
                                || !self.facts.flow.control.operands.is_valid(child.value)
                            {
                                return None;
                            }
                            selected = Some((
                                self.facts.flow.control.operands.get(child.value),
                                destination.len(),
                            ));
                        }
                    }
                    let (child, length) = selected?;
                    if child.state_symbol != operand.state_symbol
                        || child.statement_index != operand.statement_index
                    {
                        return None;
                    }
                    operand = child;
                    path = &path[length..];
                }
                Some(operand)
            }
        }
    }
    pub(crate) mod loaded_origins {
        use super::{Captures, is_mutable_structural_parameter, reference_access_can_supply};
        use checked_trees::{
            BorrowReturnedLoanFact, CheckedReferenceOriginProjection, CheckedReferenceOriginRoot,
            CheckedReferenceOriginSourcePlan, CheckedReferenceResultSourcePlan,
            CheckedUnitStructuralPathSegment,
        };
        use facts::PlaceSegment;

        impl<'capture> Captures<'capture> {
            pub(crate) fn loaded_descriptor_origins(
                &self,
                state: &typed_trees::state::State,
                loan: &BorrowReturnedLoanFact,
                leaf: &CheckedReferenceResultSourcePlan,
                segments: &[PlaceSegment],
            ) -> Option<Vec<CheckedReferenceOriginSourcePlan>> {
                let result = self.loaded_descriptor_origins_inner(state, loan, leaf, segments);
                if let Err(reason) = &result
                    && crate::trace_flags::check_sites()
                {
                    eprintln!(
                        "CHECKSITE returned-descriptor-load state={:?} identity={:?} formation={:?} result_path={:?} refusal={reason}",
                        state.symbol, loan.incoming_reference, loan.formation_expression, leaf.path,
                    );
                }
                result.ok()
            }

            fn loaded_descriptor_origins_inner(
                &self,
                state: &typed_trees::state::State,
                loan: &BorrowReturnedLoanFact,
                leaf: &CheckedReferenceResultSourcePlan,
                segments: &[PlaceSegment],
            ) -> Result<Vec<CheckedReferenceOriginSourcePlan>, &'static str> {
                let flow = self
                    .flow(state.symbol)
                    .ok_or("the returned descriptor has no exact state flow")?;
                let loaded = super::super::completed_descriptor_load(
                    self.program,
                    self.facts,
                    flow,
                    loan.incoming_reference,
                )?;
                let incoming = self
                    .facts
                    .borrow
                    .incoming_references
                    .get(loan.incoming_reference);
                if loan.root != incoming.place_root() {
                    return Err("the returned pointer differs from its completed load namespace");
                }
                let capture = self.facts.flow.control.operands.get(loaded.capture);
                let result_path =
                    super::loan_place_segments(self.program, state.return_type, &leaf.path)
                        .ok_or("the returned descriptor has no exact result leaf")?;
                let mut sources = vec![loaded.sources];
                let mut output = Vec::new();
                while let Some(source) = sources.pop() {
                    let slot = match source {
                        crate::completed_storage::StoredOperand::Incoming(slot) => slot,
                        crate::completed_storage::StoredOperand::Alternatives(alternatives)
                            if !alternatives.is_empty() =>
                        {
                            sources.extend(alternatives);
                            continue;
                        }
                        _ => {
                            return Err(
                                "the saved pointer has an unavailable or non-entry source alternative",
                            );
                        }
                    };
                    let (parameter_index, parameter) = self
                        .program
                        .state_parameters(state)
                        .iter()
                        .filter(|parameter| {
                            is_mutable_structural_parameter(self.program, parameter)
                        })
                        .enumerate()
                        .find(|(_, parameter)| {
                            slot.root == facts::PlaceRoot::Symbol(parameter.symbol)
                        })
                        .ok_or("the saved pointer's source is not an exact structural input")?;
                    let carrier =
                        super::super::input_carrier_type(self.program, parameter.type_reference)
                            .ok_or("the saved pointer's input has no readable carrier")?;
                    let (mut path, descriptor_type, slot_extent) =
                        super::super::reference_origin_path(self.program, carrier, &slot.segments)
                            .ok_or("the saved pointer's source slot has no exact typed path")?;
                    if slot_extent != semantic_vocabulary::ReferenceOriginExtent::Projected
                        || !crate::storage_origin_types_match(
                            self.program,
                            descriptor_type,
                            capture.type_reference,
                        )
                    {
                        return Err(
                            "the saved pointer's source slot differs from its loaded descriptor",
                        );
                    }
                    let descriptors =
                        super::super::declared_carrier_references(self.program, descriptor_type)
                            .ok_or(
                                "the saved pointer's source slot has no complete descriptor shape",
                            )?;
                    let [descriptor] = descriptors.as_slice() else {
                        return Err(
                            "the saved pointer's source slot does not select one descriptor",
                        );
                    };
                    if !descriptor.path.is_empty()
                        || !reference_access_can_supply(descriptor.access, leaf.source.access)
                    {
                        return Err(
                            "the saved pointer's descriptor changes its source path or access",
                        );
                    }
                    let selected = if descriptor.crosses_reference {
                        path.push(CheckedReferenceOriginProjection::Exact(
                            CheckedUnitStructuralPathSegment::Referent,
                        ));
                        super::stored_loan_parts(self.program, descriptor_type)
                            .ok_or("the saved pointer has no declared referent")?
                            .0
                    } else {
                        descriptor_type
                    };
                    let (suffix, endpoint, extent) =
                        super::super::reference_origin_path(self.program, selected, segments)
                            .ok_or("the saved pointer cannot project its captured referent")?;
                    let extent = self
                        .result_origin_extent(
                            endpoint,
                            state.return_type,
                            &result_path,
                            leaf.source.type_identity,
                            extent,
                        )
                        .ok_or(
                            "the saved pointer's captured referent does not supply the result",
                        )?;
                    path.extend(suffix);
                    let source = CheckedReferenceOriginSourcePlan {
                        root: CheckedReferenceOriginRoot::Parameter {
                            index: u32::try_from(parameter_index)
                                .map_err(|_| "the saved pointer's input ordinal exceeds u32")?,
                        },
                        entry_reference_hops: 0,
                        path,
                        extent,
                        type_identity: self.program.normalized_type_identity(endpoint),
                        access: leaf.source.access,
                    };
                    if !output.contains(&source) {
                        output.push(source);
                    }
                    if output.len() > super::super::MAX_REFERENCE_ORIGIN_LEAVES {
                        return Err("the saved pointer exceeds the returned-origin roster limit");
                    }
                }
                if output.is_empty() {
                    return Err("the saved pointer has no complete input origin alternatives");
                }
                Ok(output)
            }
        }
    }
    pub(crate) mod presence {
        use super::{
            Captures, CheckedReferenceOriginProjection, CheckedStructuralValueKind,
            CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralPathSegment,
            ExpressionHandle, StoredCarrierSource, SymbolHandle, TypeReferenceHandle,
            loan_place_segments, stored_carrier_sources,
        };
        use crate::reference_result_custody::origin_formal;

        impl<'capture> Captures<'capture> {
            pub(crate) fn call_paths(
                &mut self,
                call: &checked_trees::FlowCallFact,
                active: &mut Vec<SymbolHandle>,
            ) -> Option<Vec<Vec<CheckedUnitStructuralPathSegment>>> {
                let parameters = crate::call_target_parameters(self.program, call.target_symbol)?;
                let sources = self.target_sources(call.target_symbol, active)?;
                let operands = self
                    .facts
                    .flow
                    .control
                    .operands
                    .span(call.argument_operands())?;
                let mut paths = Vec::new();
                for source in sources {
                    let parameter_index = match source.source.root {
                        checked_trees::CheckedReferenceOriginRoot::Parameter { index } => index,
                        checked_trees::CheckedReferenceOriginRoot::Invocation { .. } => {
                            if !paths.contains(&source.path) {
                                paths.push(source.path);
                            }
                            continue;
                        }
                    };
                    let (position, parameter) =
                        origin_formal(self.program, parameters, parameter_index)?;
                    let operand = operands.get(position)?;
                    let present = if crate::reference_shell(self.program, parameter.type_reference)
                        .is_some()
                    {
                        true
                    } else {
                        let path = source
                            .source
                            .path
                            .iter()
                            .take_while(|projection| {
                                !matches!(
                                    projection,
                                    CheckedReferenceOriginProjection::Exact(
                                        CheckedUnitStructuralPathSegment::Referent
                                    )
                                )
                            })
                            .map(|projection| match projection {
                                CheckedReferenceOriginProjection::Exact(segment) => {
                                    Some(segment.clone())
                                }
                                CheckedReferenceOriginProjection::Element => None,
                            })
                            .collect::<Option<Vec<_>>>()?;
                        self.expression_leaf_present(
                            self.program.state_by_symbol(operand.state_symbol)?,
                            operand.statement_index,
                            operand.expression,
                            parameter.type_reference,
                            &path,
                            Some(operand),
                            active,
                        )?
                    };
                    if present && !paths.contains(&source.path) {
                        paths.push(source.path);
                    }
                }
                Some(paths)
            }

            pub(crate) fn expression_leaf_present(
                &mut self,
                state: &typed_trees::state::State,
                statement_index: usize,
                expression: ExpressionHandle,
                reference: TypeReferenceHandle,
                path: &[CheckedUnitStructuralPathSegment],
                capture: Option<&checked_trees::FlowOperandFact>,
                active: &mut Vec<SymbolHandle>,
            ) -> Option<bool> {
                let flow = self.flow(state.symbol)?;
                let mut calls = self
                    .facts
                    .flow
                    .control
                    .calls
                    .span(flow.calls)?
                    .iter()
                    .filter(|call| {
                        call.statement_index == statement_index
                            && call.authored_expression == expression
                    });
                if let Some(call) = calls.next() {
                    if calls.next().is_some() {
                        return None;
                    }
                    return Some(
                        self.call_paths(call, active)?
                            .iter()
                            .any(|candidate| candidate.as_slice() == path),
                    );
                }
                let values = &self.facts.values.structural_values;
                let root = values
                    .root_for_expression(
                        state.symbol,
                        u32::try_from(statement_index).ok()?,
                        expression,
                    )
                    .map(|root| root.root)
                    .or_else(|| {
                        values.nodes.iter().find_map(|(handle, node)| {
                            (node.state == state.symbol
                                && node.statement_index as usize == statement_index
                                && node.expression == expression
                                && self.program.normalized_type_identity(node.type_reference)
                                    == self.program.normalized_type_identity(reference))
                            .then_some(handle)
                        })
                    });
                let Some(root) = root else {
                    let mut place = crate::canonical_place_from_expression_in_state(
                        self.program,
                        state.symbol,
                        statement_index,
                        expression,
                    )?;
                    place.extend_segments(&loan_place_segments(self.program, reference, path)?);
                    return self.place_leaf_present(
                        state,
                        statement_index,
                        place.root,
                        &place.segments,
                        capture,
                        active,
                    );
                };
                let mut present = false;
                for source in stored_carrier_sources(self.program, self.facts, root, path)? {
                    let supplied = match source {
                        StoredCarrierSource::Borrow { .. } => true,
                        StoredCarrierSource::Image { .. } => false,
                        StoredCarrierSource::Place(source) => match source.source {
                            CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } => {
                                let reference = super::super::storage_owner_type_reference(
                                    self.program,
                                    state,
                                    symbol,
                                    u32::try_from(statement_index).ok()?,
                                )?;
                                self.place_leaf_present(
                                    state,
                                    statement_index,
                                    facts::PlaceRoot::Symbol(symbol),
                                    &self.captured_place_segments(reference, &source.path)?,
                                    capture,
                                    active,
                                )?
                            }
                            CheckedUnitStructuralArgumentSourcePlan::Parameter {
                                parameter_index,
                            } => {
                                let (_, parameter) = origin_formal(
                                    self.program,
                                    self.program.state_parameters(state),
                                    parameter_index,
                                )?;
                                self.ingress_present(state, parameter.symbol)?
                            }
                            _ => return None,
                        },
                        StoredCarrierSource::Call { value, path } => {
                            let CheckedStructuralValueKind::Call { source_call } =
                                values.nodes.get(value).kind
                            else {
                                return None;
                            };
                            if !self.facts.flow.control.calls.is_valid(source_call) {
                                return None;
                            }
                            self.call_paths(self.facts.flow.control.calls.get(source_call), active)?
                                .contains(&path)
                        }
                    };
                    present |= supplied;
                }
                Some(present)
            }

            fn captured_place_segments(
                &self,
                reference: TypeReferenceHandle,
                path: &[CheckedUnitStructuralPathSegment],
            ) -> Option<Vec<facts::PlaceSegment>> {
                let coordinates = path
                    .iter()
                    .filter(|segment| {
                        !matches!(segment, CheckedUnitStructuralPathSegment::Referent)
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                let segments = loan_place_segments(self.program, reference, &coordinates)?;
                let (physical, _) = super::super::source_paths::structural_path(
                    self.program,
                    reference,
                    &segments,
                )?;
                (physical == path).then_some(segments)
            }

            pub(crate) fn place_leaf_present(
                &mut self,
                state: &typed_trees::state::State,
                before: usize,
                root: facts::PlaceRoot,
                segments: &[facts::PlaceSegment],
                capture: Option<&checked_trees::FlowOperandFact>,
                active: &mut Vec<SymbolHandle>,
            ) -> Option<bool> {
                let control = &self.facts.flow.control;
                let mut place = crate::CanonicalPlace {
                    root,
                    segments: segments.to_vec(),
                };
                let mut observation = capture;
                let mut sources = Vec::new();
                while let Some(capture) = observation {
                    let source = capture.value_source;
                    if !source.is_valid()
                        || crate::reference_shell(self.program, capture.type_reference).is_some()
                    {
                        break;
                    }
                    if !control.operands.is_valid(source) || sources.contains(&source) {
                        return None;
                    }
                    let base = crate::canonical_place_from_expression_in_state(
                        self.program,
                        capture.state_symbol,
                        capture.statement_index,
                        capture.expression,
                    )?;
                    if place.root != base.root || !place.segments.starts_with(&base.segments) {
                        return None;
                    }
                    let suffix = &place.segments[base.segments.len()..];
                    if (0..suffix.len()).any(|length| {
                        crate::project_type_reference_from_segments(
                            self.program,
                            capture.type_reference,
                            &suffix[..length],
                        )
                        .is_some_and(|reference| {
                            crate::reference_shell(self.program, reference).is_some()
                        })
                    }) {
                        break;
                    }
                    let earlier = control.operands.get(source);
                    let mut prior = crate::canonical_place_from_expression_in_state(
                        self.program,
                        earlier.state_symbol,
                        earlier.statement_index,
                        earlier.expression,
                    )?;
                    prior.segments.extend_from_slice(suffix);
                    place = prior;
                    observation = Some(earlier);
                    sources.push(source);
                }
                let flow = self.flow(state.symbol)?;
                let snapshot = if let Some(capture) = observation {
                    capture.storage_snapshot
                } else {
                    let order = self
                        .program
                        .statement_table
                        .admitted_execution_order(state.statement_nodes);
                    let position = order.position(before).or_else(|| {
                        (before
                            == self
                                .program
                                .statement_table
                                .statements(state.statement_nodes)
                                .len())
                        .then_some(order.indices().len())
                    })?;
                    if let Some(previous) = order.indices()[..position].last() {
                        control
                            .statements
                            .span(flow.statements)?
                            .iter()
                            .find(|statement| statement.statement_index == *previous)?
                            .storage_snapshot
                    } else {
                        Default::default()
                    }
                };
                let Some(stored) = crate::completed_storage::at_snapshot(
                    self.program,
                    control,
                    snapshot,
                    &place,
                    crate::completed_storage::StoredObservation::Binding,
                ) else {
                    return self.ingress_present(state, place.root.as_symbol()?);
                };
                let captures = stored.captures()?;
                let mut present = false;
                for selected in captures {
                    let value = control.operands.get(selected.capture);
                    if crate::reference_shell(self.program, value.type_reference).is_some() {
                        let values = crate::completed_storage::reference_captures_at_path(
                            self.program,
                            control,
                            selected.capture,
                            &selected.projection,
                            snapshot,
                        );
                        let Some(values) = values else {
                            let referents = control.operand_referents.span(value.referents)?;
                            if referents.is_empty() {
                                return None;
                            }
                            for referent in referents {
                                present |=
                                    self.ingress_present(state, referent.root.as_symbol()?)?;
                            }
                            continue;
                        };
                        for descriptor in values.captures()? {
                            let descriptor_value = control.operands.get(descriptor.capture);
                            if descriptor.projection.is_empty()
                                && crate::reference_shell(
                                    self.program,
                                    descriptor_value.type_reference,
                                )
                                .is_some()
                            {
                                present |= !control
                                    .retained_roots(descriptor_value.retention)?
                                    .is_empty();
                            } else {
                                let (path, _) = super::super::source_paths::structural_path(
                                    self.program,
                                    descriptor_value.type_reference,
                                    &descriptor.projection,
                                )?;
                                present |= self.expression_leaf_present(
                                    self.program
                                        .state_by_symbol(descriptor_value.state_symbol)?,
                                    descriptor_value.statement_index,
                                    descriptor_value.expression,
                                    descriptor_value.type_reference,
                                    &path,
                                    Some(descriptor_value),
                                    active,
                                )?;
                            }
                        }
                    } else {
                        let (path, _) = super::super::source_paths::structural_path(
                            self.program,
                            value.type_reference,
                            &selected.projection,
                        )?;
                        present |= self.expression_leaf_present(
                            self.program.state_by_symbol(value.state_symbol)?,
                            value.statement_index,
                            value.expression,
                            value.type_reference,
                            &path,
                            Some(value),
                            active,
                        )?;
                    }
                }
                Some(present)
            }

            pub(crate) fn ingress_present(
                &self,
                state: &typed_trees::state::State,
                symbol: SymbolHandle,
            ) -> Option<bool> {
                let parameter = self
                    .program
                    .state_parameters(state)
                    .iter()
                    .find(|parameter| parameter.symbol == symbol)?;
                let flow = self.flow(state.symbol)?;
                super::super::stored_carrier_ingress_is_complete(
                    self.program,
                    self.facts.view(),
                    flow.machine_symbol,
                    state.symbol,
                    symbol,
                    parameter.type_reference,
                )
                .then_some(true)
            }
        }
    }
    pub(crate) mod relations {

        use super::{
            Captures, CheckedReturnedReferencePlan, SymbolHandle, completions::ReturnCoverage,
        };
        use checked_trees::CheckFacts;

        /*
        What one evaluation of a return equation (or of the root question)
        observed: the states it read approximations of, the coverage of each
        completed value, and whether it proved the state cannot return. The
        fixed-point loop in `complete` owns one per evaluation and lends it
        `&mut` to the capture walk, then moves the fields out.
        */
        #[derive(Default)]
        pub(crate) struct EquationObservations {
            dependencies: Vec<SymbolHandle>,
            completions: Vec<ReturnCoverage>,
            nonreturning: bool,
        }

        impl EquationObservations {
            pub(crate) fn require(&mut self, state: SymbolHandle) {
                if !self.dependencies.contains(&state) {
                    self.dependencies.push(state);
                }
            }

            pub(crate) fn completed(&mut self, completion: ReturnCoverage) {
                for dependency in completion.dependencies() {
                    self.require(dependency);
                }
                self.completions.push(completion);
            }

            pub(crate) fn nonreturning(&mut self) {
                self.nonreturning = true;
            }
        }

        pub(crate) struct ReturnEquation {
            state: SymbolHandle,
            sources: Vec<CheckedReturnedReferencePlan>,
            dependencies: Vec<SymbolHandle>,
            callers: Vec<SymbolHandle>,
            completions: Vec<ReturnCoverage>,
            nonreturning: bool,
            candidates: Option<Vec<CheckedReturnedReferencePlan>>,
            productive: bool,
        }

        #[derive(Default)]
        pub(crate) struct ReturnRelations {
            rows: Vec<ReturnEquation>,
        }

        fn key(state: SymbolHandle) -> (u32, u32) {
            (state.arena_index(), state.generation())
        }

        impl ReturnRelations {
            pub(crate) fn productive(&self, state: SymbolHandle) -> bool {
                self.position(state)
                    .is_some_and(|position| self.rows[position].productive)
            }

            fn position(&self, state: SymbolHandle) -> Option<usize> {
                self.rows
                    .binary_search_by_key(&key(state), |row| key(row.state))
                    .ok()
            }

            pub(crate) fn approximation(
                &self,
                state: SymbolHandle,
                observed: &mut EquationObservations,
            ) -> Vec<CheckedReturnedReferencePlan> {
                observed.require(state);
                match self.position(state) {
                    Some(position) => self.rows[position].sources.clone(),
                    None => Vec::new(),
                }
            }

            fn edges(&self) -> Option<Vec<Vec<usize>>> {
                self.rows
                    .iter()
                    .map(|row| {
                        row.dependencies
                            .iter()
                            .map(|state| self.position(*state))
                            .collect()
                    })
                    .collect()
            }

            fn completion_established(&self, position: usize) -> bool {
                self.rows[position].completions.iter().any(|completion| {
                    completion.established(|state| {
                        self.position(state)
                            .is_some_and(|index| self.rows[index].productive)
                    })
                })
            }
        }

        pub(crate) fn complete<T>(
            program: &dyn crate::CheckingInput,
            facts: &CheckFacts,
            question: impl Fn(&mut Captures<'_>) -> Option<T>,
        ) -> Option<T> {
            let mut calls = super::completion_calls::CompletionCalls::default();
            let mut relations = ReturnRelations::default();
            let mut answer = None;
            let mut question_dependencies = Vec::new();
            let mut question_coverage = Vec::new();
            let mut question_nonreturning = false;
            let mut question_dirty = true;
            let mut pending = Vec::new();
            loop {
                let mut evaluated = Vec::new();
                let mut required = Vec::new();
                for state in std::mem::take(&mut pending) {
                    let position = relations.position(state)?;
                    let row = &relations.rows[position];
                    let mut observed = EquationObservations::default();
                    let mut captures = Captures {
                        program,
                        facts,
                        calls: &mut calls,
                        relations: Some(&relations),
                        observed: Some(&mut observed),
                    };
                    let sources = captures
                        .returned(program.state_by_symbol(row.state)?, &mut Vec::new())
                        .or_else(|| {
                            refusal("captured return equation is unavailable", Some(row.state))
                        })?;
                    if row.sources.iter().any(|source| !sources.contains(source)) {
                        return refusal(
                            "captured return alternatives are not monotone",
                            Some(row.state),
                        );
                    }
                    let dependencies = observed.dependencies;
                    required.extend(dependencies.iter().copied());
                    evaluated.push(EvaluatedEquation {
                        position,
                        sources,
                        dependencies,
                        completions: observed.completions,
                        nonreturning: observed.nonreturning,
                    });
                }
                if question_dirty {
                    question_dirty = false;
                    let mut observed = EquationObservations::default();
                    answer = Some(
                        question(&mut Captures {
                            program,
                            facts,
                            calls: &mut calls,
                            relations: Some(&relations),
                            observed: Some(&mut observed),
                        })
                        .or_else(|| refusal("completed return question is unavailable", None))?,
                    );
                    question_coverage = observed.completions;
                    question_nonreturning = observed.nonreturning;
                    for dependency in observed.dependencies {
                        required.push(dependency);
                        if !question_dependencies.contains(&dependency) {
                            question_dependencies.push(dependency);
                            question_dirty = true;
                        }
                    }
                }
                let mut affected = Vec::new();
                let mut new_edges = Vec::new();
                let mut evaluated_states = Vec::with_capacity(evaluated.len());
                for evaluated in evaluated {
                    let row = &mut relations.rows[evaluated.position];
                    evaluated_states.push(row.state);
                    let mut changed = false;
                    for source in evaluated.sources {
                        if !row.sources.contains(&source) {
                            row.sources.push(source);
                            changed = true;
                        }
                    }
                    if evaluated.nonreturning && !row.nonreturning {
                        row.nonreturning = true;
                        changed = true;
                    }
                    for completion in evaluated.completions {
                        if !row.completions.contains(&completion) {
                            row.completions.push(completion);
                            changed = true;
                        }
                    }
                    for dependency in evaluated.dependencies {
                        if !row.dependencies.contains(&dependency) {
                            row.dependencies.push(dependency);
                            new_edges.push((dependency, row.state));
                            changed = true;
                        }
                    }
                    if changed {
                        affected.push(row.state);
                    }
                }
                required.sort_by_key(|state| key(*state));
                required.dedup();
                let new_states = required
                    .into_iter()
                    .filter(|state| relations.position(*state).is_none())
                    .collect::<Vec<_>>();
                let membership_changed = !new_states.is_empty();
                for state in new_states {
                    pending.push(state);
                    relations.rows.push(ReturnEquation {
                        state,
                        sources: Vec::new(),
                        dependencies: Vec::new(),
                        callers: Vec::new(),
                        completions: Vec::new(),
                        nonreturning: false,
                        candidates: None,
                        productive: false,
                    });
                    affected.push(state);
                }
                if membership_changed {
                    relations.rows.sort_by_key(|row| key(row.state));
                }
                let graph_changed = !new_edges.is_empty();
                for (dependency, caller) in new_edges {
                    let position = relations.position(dependency)?;
                    let callers = &mut relations.rows[position].callers;
                    if !callers.contains(&caller) {
                        callers.push(caller);
                    }
                }
                if graph_changed {
                    let edges = relations.edges()?;
                    for component in
                        crate::machine_calls::graph_components::strongly_connected_components(
                            &edges,
                        )
                    {
                        let cyclic = component.len() > 1
                            || component
                                .first()
                                .is_some_and(|position| edges[*position].contains(position));
                        if !cyclic {
                            continue;
                        }
                        for position in component {
                            let row = &mut relations.rows[position];
                            if row.candidates.is_none() {
                                row.candidates =
                                    Some(candidates(program, facts, row.state).or_else(|| {
                                        refusal(
                                            "recursive returned-loan frontier is unavailable",
                                            Some(row.state),
                                        )
                                    })?);
                            }
                            let candidates = row.candidates.as_ref()?;
                            if let Some(source) = row
                                .sources
                                .iter()
                                .find(|source| !candidates.contains(source))
                            {
                                if crate::trace_flags::check_sites() {
                                    eprintln!(
                                        "CHECKSITE captured-return-equation state={:?} refusal=recursive-source-outside-capture source={source:?} candidates={candidates:?}",
                                        row.state,
                                    );
                                }
                                return None;
                            }
                        }
                    }
                }
                for state in &affected {
                    let row = &relations.rows[relations.position(*state)?];
                    if let Some(candidates) = &row.candidates
                        && row
                            .sources
                            .iter()
                            .any(|source| !candidates.contains(source))
                    {
                        return refusal(
                            "recursive source is outside its captured frontier",
                            Some(*state),
                        );
                    }
                }
                let newly_productive = evaluated_states
                    .into_iter()
                    .filter_map(|state| {
                        let position = relations.position(state)?;
                        (!relations.rows[position].productive
                            && relations.completion_established(position))
                        .then_some(position)
                    })
                    .collect::<Vec<_>>();
                for position in newly_productive {
                    relations.rows[position].productive = true;
                    affected.push(relations.rows[position].state);
                }
                affected.sort_by_key(|state| key(*state));
                affected.dedup();
                for state in &affected {
                    let row = &relations.rows[relations.position(*state)?];
                    pending.extend(row.callers.iter().copied());
                }
                pending.sort_by_key(|state| key(*state));
                pending.dedup();
                if question_dependencies.iter().any(|state| {
                    affected
                        .binary_search_by_key(&key(*state), |state| key(*state))
                        .is_ok()
                }) {
                    question_dirty = true;
                }
                if question_dirty || !pending.is_empty() {
                    continue;
                }
                if let Some(row) = relations
                    .rows
                    .iter()
                    .find(|row| !row.productive && !row.nonreturning)
                {
                    return refusal(
                        "recursive return has no independently covered value boundary",
                        Some(row.state),
                    );
                }
                if !question_nonreturning
                    && !question_coverage.is_empty()
                    && !question_coverage.iter().any(|coverage| {
                        coverage.established(|state| {
                            relations
                                .position(state)
                                .is_some_and(|position| relations.rows[position].productive)
                        })
                    })
                {
                    return refusal(
                        "root return has no independently covered value boundary",
                        None,
                    );
                }
                return answer;
            }
        }

        pub(crate) struct EvaluatedEquation {
            position: usize,
            sources: Vec<CheckedReturnedReferencePlan>,
            dependencies: Vec<SymbolHandle>,
            completions: Vec<ReturnCoverage>,
            nonreturning: bool,
        }

        fn refusal<T>(reason: &'static str, state: Option<SymbolHandle>) -> Option<T> {
            if crate::trace_flags::check_sites() {
                eprintln!("CHECKSITE captured-return-equation state={state:?} refusal={reason}");
            }
            None
        }

        fn candidates(
            program: &dyn crate::CheckingInput,
            facts: &CheckFacts,
            state: SymbolHandle,
        ) -> Option<Vec<CheckedReturnedReferencePlan>> {
            let state = program.state_by_symbol(state)?;
            if let Some(entries) = super::family_returns::entries(program, state)? {
                let mut output = Vec::new();
                for entry in entries {
                    for candidate in candidates(program, facts, entry.symbol)? {
                        if !output.contains(&candidate) {
                            output.push(candidate);
                            if output.len() > super::super::MAX_REFERENCE_ORIGIN_LEAVES {
                                return None;
                            }
                        }
                    }
                }
                return Some(output);
            }
            if !crate::normal_completion_possible(program, state)? {
                return Some(Vec::new());
            }
            let mut calls = super::completion_calls::CompletionCalls::default();
            let captures = Captures {
                program,
                facts,
                calls: &mut calls,
                relations: None,
                observed: None,
            };
            let borrowed = facts.borrow.unique_state_borrow_by_symbol(state.symbol)?;
            let loans = facts.borrow.returned_loans.span(borrowed.returned_loans)?;
            let mut output = Vec::new();
            for leaf in captures.reference_leaves(state)? {
                let owner_path = crate::corresponding_loan_owner_path(
                    program,
                    state.return_type,
                    &super::loan_owner_path(program, state.return_type, &leaf.path)?,
                )?;
                for loan in loans.iter().filter(|loan| {
                    loan.lineage != checked_trees::BorrowLoanLineage::LifetimeDependency
                        && captures.returned_loan_owner_path(state, loan).as_deref()
                            == Some(owner_path.as_slice())
                }) {
                    if leaf.source.access.borrow_access_kind()? != loan.kind {
                        return None;
                    }
                    for source in captures.return_origin(
                        state,
                        loan,
                        &leaf,
                        facts.borrow.access_segments.span(loan.segments)?,
                        loans,
                    )? {
                        let candidate = CheckedReturnedReferencePlan {
                            path: leaf.path.clone(),
                            source,
                        };
                        if !output.contains(&candidate) {
                            output.push(candidate);
                            if output.len() > super::super::MAX_REFERENCE_ORIGIN_LEAVES {
                                return None;
                            }
                        }
                    }
                }
            }
            Some(output)
        }

        #[cfg(test)]
        mod tests {
            use super::*;
            use checked_trees::{FlowOperandCompletion, FlowOperandFact};

            #[test]
            fn captured_return_question_checks_direct_root_before_publication() {
                let program = typed_trees::TypedTrees::default();
                let facts = CheckFacts::default();
                let selected_without_value = FlowOperandFact {
                    completion: FlowOperandCompletion::Selected(arena::HandleSpan::empty()),
                    ..FlowOperandFact::default()
                };
                let missing =
                    ReturnCoverage::captured(&facts.flow.control, &selected_without_value, |_| {
                        Some(Vec::new())
                    })
                    .expect("the missing alternative is represented, not silently removed");
                let result = complete(&program, &facts, |captures| {
                    captures.observed.as_deref_mut()?.completed(missing.clone());
                    Some(7)
                });
                assert_eq!(result, None);

                let empty = FlowOperandFact {
                    completion: FlowOperandCompletion::Constructed(arena::HandleSpan::empty()),
                    ..FlowOperandFact::default()
                };
                let covered =
                    ReturnCoverage::captured(&facts.flow.control, &empty, |_| Some(Vec::new()))
                        .expect("the formed empty constructor has complete coverage");
                assert_eq!(
                    complete(&program, &facts, |captures| {
                        captures.observed.as_deref_mut()?.completed(covered.clone());
                        Some(7)
                    }),
                    Some(7)
                );
                assert_eq!(complete(&program, &facts, |_| Some(7)), Some(7));
            }

            #[test]
            fn captured_return_question_preserves_proved_nonreturning_root() {
                let program = typed_trees::TypedTrees::default();
                let facts = CheckFacts::default();
                assert_eq!(
                    complete(&program, &facts, |captures| {
                        captures.observed.as_deref_mut()?.nonreturning();
                        Some(Vec::<CheckedReturnedReferencePlan>::new())
                    }),
                    Some(Vec::new())
                );
            }
        }
    }

    use super::{
        StoredCarrierSource, is_mutable_structural_parameter, loan_owner_path, loan_place_segments,
        record_carrier_sources, reference_access_can_supply, stored_carrier_sources,
        stored_loan_parts,
    };
    use crate::frozen_program::expression_table;
    use checked_trees::IncomingReferenceOrigin;
    use checked_trees::{
        CheckFacts, CheckedReferenceOriginProjection, CheckedReferenceOriginSourcePlan,
        CheckedReferenceResultSourcePlan, CheckedReturnedReferencePlan, CheckedStructuralValueKind,
        CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralPathSegment,
    };
    use semantic_vocabulary::ReferenceOriginExtent;
    use symbols::SymbolHandle;
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::state::State;
    use typed_trees::statement::{StatementNode, TransitionExit, TransitionTargetNode};
    use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

    pub fn returned_sources(
        program: &dyn crate::CheckingInput,
        facts: &CheckFacts,
        state: &State,
    ) -> Option<Vec<CheckedReturnedReferencePlan>> {
        relations::complete(program, facts, |captures| {
            captures.returned(state, &mut Vec::new())
        })
    }

    pub fn call_sources(
        program: &dyn crate::CheckingInput,
        facts: &CheckFacts,
        target: SymbolHandle,
    ) -> Option<Vec<CheckedReturnedReferencePlan>> {
        relations::complete(program, facts, |captures| {
            captures.target_sources(target, &mut Vec::new())
        })
    }

    struct CapturedInputDescriptor {
        path: Vec<CheckedUnitStructuralPathSegment>,
        prefix: Vec<facts::PlaceSegment>,
    }

    fn input_descriptor_boundary(
        program: &dyn crate::CheckingInput,
        reference: TypeReferenceHandle,
        segments: &[facts::PlaceSegment],
    ) -> Option<Option<CapturedInputDescriptor>> {
        let outer_reference = crate::reference_shell(program, reference).is_some();
        let carrier = super::input_carrier_type(program, reference).unwrap_or(reference);
        let mut boundary = None;
        for leaf in super::declared_carrier_references(program, carrier)? {
            let prefix = super::origin_place_segments(program, carrier, leaf.path.iter().cloned())?;
            if outer_reference && prefix.is_empty() {
                continue;
            }
            if segments.starts_with(&prefix) {
                if boundary.is_some() {
                    return None;
                }
                boundary = Some((leaf, prefix));
            }
        }
        let Some((leaf, prefix)) = boundary else {
            return Some(None);
        };
        Some(Some(CapturedInputDescriptor {
            path: leaf
                .path
                .into_iter()
                .map(|projection| match projection {
                    CheckedReferenceOriginProjection::Exact(segment) => Some(segment),
                    CheckedReferenceOriginProjection::Element => None,
                })
                .collect::<Option<Vec<_>>>()?,
            prefix,
        }))
    }

    // Keep the captured input and its projection together. The borrowing checker
    // still checks the input's permission and retained parents.
    pub struct CapturedInvocationBorrowSource {
        pub operand: arena::Handle<checked_trees::FlowOperandFact>,
        pub source_path: Vec<facts::PlaceSegment>,
        pub projection: Vec<facts::PlaceSegment>,
        pub extent: ReferenceOriginExtent,
    }

    pub fn invocation_borrow_sources(
        program: &dyn crate::CheckingInput,
        facts: &CheckFacts,
        state: &State,
        descriptor: &checked_trees::FlowOperandFact,
    ) -> Option<Vec<CapturedInvocationBorrowSource>> {
        let (established, sources) = relations::complete(program, facts, |captures| {
            captures.invocation_borrow_sources(state, descriptor)
        })?;
        (established && !sources.is_empty()).then_some(sources)
    }

    pub(crate) fn terminal_call_sources(
        program: &dyn crate::CheckingInput,
        facts: &CheckFacts,
        state: &State,
        call: &checked_trees::FlowCallFact,
    ) -> Option<Vec<CheckedReturnedReferencePlan>> {
        relations::complete(program, facts, |captures| {
            let flow = captures.flow(state.symbol)?;
            let machine = program.machine_by_symbol(flow.machine_symbol)?;
            let target = crate::transition_call_target(
                program,
                machine,
                state,
                call.statement_index,
                call.call_ordinal,
            )?;
            if captures.return_call(
                state,
                call.statement_index,
                target,
                ExpressionHandle::invalid(),
            )? != call
            {
                return None;
            }
            captures.normal_returns(
                state,
                &mut vec![state.symbol],
                Some((call.statement_index, target)),
            )
        })
    }

    pub(crate) fn call_result_paths(
        program: &dyn crate::CheckingInput,
        facts: &CheckFacts,
        occurrence: &checked_trees::FlowCallFact,
    ) -> Option<Vec<Vec<CheckedUnitStructuralPathSegment>>> {
        relations::complete(program, facts, |captures| {
            captures.call_paths(occurrence, &mut Vec::new())
        })
    }

    pub(crate) struct Captures<'capture> {
        program: &'capture dyn crate::CheckingInput,
        facts: &'capture CheckFacts,
        calls: &'capture mut completion_calls::CompletionCalls,
        relations: Option<&'capture relations::ReturnRelations>,
        observed: Option<&'capture mut relations::EquationObservations>,
    }

    pub(crate) struct FrameOrigin {
        path: Vec<CheckedReferenceOriginProjection>,
        backing: TypeReferenceHandle,
        presentation: TypeReferenceHandle,
        extent: ReferenceOriginExtent,
    }

    #[derive(Clone, PartialEq, Eq)]
    pub(crate) struct CapturedReturnOrigin {
        result_path: Vec<facts::PlaceSegment>,
        source: crate::LocalWriteSource,
        extent: ReferenceOriginExtent,
        presentation: Option<CapturedReturnPresentation>,
    }

    #[derive(Clone, PartialEq, Eq)]
    pub(crate) struct CapturedReturnPresentation {
        path: Vec<CheckedReferenceOriginProjection>,
        source_type: typed_trees::type_identity::TypeIdentityId,
        endpoint_type: typed_trees::type_identity::TypeIdentityId,
        result_type: typed_trees::type_identity::TypeIdentityId,
        extent: ReferenceOriginExtent,
    }

    impl<'capture> Captures<'capture> {
        fn trace_return_state(&self, state: &State) -> bool {
            if !crate::trace_flags::trace_completion() {
                return false;
            }
            let Ok(selected) = std::env::var("OMEGA_COMPILER_TRACE_COMPLETION") else {
                return false;
            };
            let name = self
                .program
                .machine_by_own_or_state_symbol(state.symbol)
                .map_or(state.name.as_str(), |machine| machine.name.as_str());
            selected
                .split(',')
                .any(|selected| selected == "*" || selected == name)
        }

        fn target_sources(
            &mut self,
            target: SymbolHandle,
            _active: &mut Vec<SymbolHandle>,
        ) -> Option<Vec<CheckedReturnedReferencePlan>> {
            if let Some(sources) =
                super::declared_returns::installed_context_sources(self.program, target).ok()?
            {
                return Some(sources);
            }
            if !crate::call_target_has_checked_body_contract(self.program, target) {
                let sources = super::declared_returns::returned_sources(
                    self.program,
                    crate::call_target_parameters(self.program, target)?,
                    crate::call_target_return_type(self.program, target)?,
                )
                .ok()?;
                return Some(sources);
            }
            let state = crate::call_target_state(self.program, target)?;
            if let Some(relations) = self.relations {
                let observed = self.observed.as_deref_mut()?;
                return Some(relations.approximation(state.symbol, observed));
            }
            None
        }

        fn returned(
            &mut self,
            state: &State,
            active: &mut Vec<SymbolHandle>,
        ) -> Option<Vec<CheckedReturnedReferencePlan>> {
            if active.contains(&state.symbol) {
                return Some(Vec::new());
            }
            if active.len() >= super::MAX_REFERENCE_ORIGIN_DEPTH {
                return None;
            }
            if let Some(entries) = family_returns::entries(self.program, state)? {
                return self.family_return(&entries);
            }
            if !crate::normal_completion_possible(self.program, state)? {
                if let Some(observed) = self.observed.as_deref_mut() {
                    observed.nonreturning();
                }
                return Some(Vec::new());
            }
            let result = self.normal_returns(state, active, None);
            if self.trace_return_state(state) {
                eprintln!(
                    "CHECKSITE completion-returned-sources state={:?} active={:?} result={:?}",
                    state.symbol, active, result
                );
            }
            result
        }

        fn normal_returns(
            &mut self,
            state: &State,
            active: &mut Vec<SymbolHandle>,
            selected_site: Option<(usize, typed_trees::statement::TransitionTargetHandle)>,
        ) -> Option<Vec<CheckedReturnedReferencePlan>> {
            let borrowed = self
                .facts
                .borrow
                .unique_state_borrow_by_symbol(state.symbol)?;
            let loans = self
                .facts
                .borrow
                .returned_loans
                .span(borrowed.returned_loans)?;
            let leaves = self.reference_leaves(state)?;
            let statements = typed_trees::statement::reachable_state_statements(
                self.program
                    .statement_table
                    .statements(state.statement_nodes),
            );
            let mut output = Vec::new();
            let mut completed = false;
            for (statement_index, statement) in statements.iter().enumerate() {
                if selected_site.is_some_and(|(selected, _)| selected != statement_index) {
                    continue;
                }
                let mut sites = Vec::new();
                match statement {
                    StatementNode::Expression(expression)
                        if statement_index + 1 == statements.len() =>
                    {
                        if !crate::expression_normal_completion_possible(self.program, *expression)?
                        {
                            continue;
                        }
                        let target = Default::default();
                        let call =
                            self.ordinary_return_call(state, statement_index, target, *expression)?;
                        sites.push((target, Some(*expression), call));
                    }
                    StatementNode::Transition(transition) => {
                        if matches!(transition.exit, TransitionExit::Crash(_)) {
                            continue;
                        }
                        for target in transition.targets() {
                            if selected_site.is_some_and(|(_, selected)| selected != target) {
                                continue;
                            }
                            match self.program.statement_table.transition_target(target) {
                                TransitionTargetNode::Value(expression) => {
                                    if !crate::expression_normal_completion_possible(
                                        self.program,
                                        *expression,
                                    )? {
                                        continue;
                                    }
                                    let call = self.ordinary_return_call(
                                        state,
                                        statement_index,
                                        target,
                                        *expression,
                                    )?;
                                    sites.push((target, Some(*expression), call));
                                }
                                TransitionTargetNode::Named { .. } => {
                                    let call = self.return_call(
                                        state,
                                        statement_index,
                                        target,
                                        ExpressionHandle::invalid(),
                                    )?;
                                    if let Some(destination) =
                                        crate::call_target_state(self.program, call.target_symbol)
                                        && self.flow(destination.symbol)?.machine_symbol
                                            == self.flow(state.symbol)?.machine_symbol
                                        && !crate::normal_completion_possible(
                                            self.program,
                                            destination,
                                        )?
                                    {
                                        continue;
                                    }
                                    sites.push((target, None, Some(call)));
                                }
                                TransitionTargetNode::Terminal => return None,
                            }
                        }
                    }
                    _ => continue,
                }
                for (transition_target, expression, call) in sites {
                    completed = true;
                    if let Some(call) = call {
                        if !self.observe_return_invocation(call)? {
                            continue;
                        }
                        for source in self.invocation_return_sources(
                            state,
                            transition_target,
                            call,
                            loans,
                            active,
                        )? {
                            if !output.contains(&source) {
                                output.push(source);
                                if output.len() > super::MAX_REFERENCE_ORIGIN_LEAVES {
                                    return None;
                                }
                            }
                        }
                        continue;
                    }
                    let expression = expression?;
                    for leaf in &leaves {
                        let owner_path = crate::corresponding_loan_owner_path(
                            self.program,
                            state.return_type,
                            &loan_owner_path(self.program, state.return_type, &leaf.path)?,
                        )?;
                        let mut captured = false;
                        for loan in loans.iter().filter(|loan| {
                            loan.lineage != checked_trees::BorrowLoanLineage::LifetimeDependency
                                && loan.statement_index == statement_index
                                && loan.transition_target == transition_target
                        }) {
                            if self.returned_loan_owner_path(state, loan)? != owner_path {
                                continue;
                            }
                            if leaf.source.access.borrow_access_kind()? != loan.kind {
                                return None;
                            }
                            captured = true;
                            for origin in
                                self.return_origins(state, loan, leaf, Some(expression), loans)?
                            {
                                let source = CheckedReturnedReferencePlan {
                                    path: leaf.path.clone(),
                                    source: origin,
                                };
                                if !output.contains(&source) {
                                    output.push(source);
                                    if output.len() > super::MAX_REFERENCE_ORIGIN_LEAVES {
                                        return None;
                                    }
                                }
                            }
                        }
                        if !captured {
                            let capture = self.completed_return_value(
                                state,
                                statement_index,
                                transition_target,
                                expression,
                            )?;
                            let present = self.expression_leaf_present(
                                state,
                                statement_index,
                                expression,
                                state.return_type,
                                &leaf.path,
                                Some(capture),
                                active,
                            )?;
                            if present {
                                return None;
                            }
                        }
                    }
                    if self.observed.is_some() {
                        let value = self.completed_return_value(
                            state,
                            statement_index,
                            transition_target,
                            expression,
                        )?;
                        let facts = self.facts;
                        let coverage = completions::ReturnCoverage::captured(
                            &facts.flow.control,
                            value,
                            |operand| self.completion_calls(operand),
                        )?;
                        self.observed.as_deref_mut()?.completed(coverage);
                    }
                }
            }
            completed.then_some(output)
        }

        fn observe_return_invocation(
            &mut self,
            call: &checked_trees::FlowCallFact,
        ) -> Option<bool> {
            if self.observed.is_some() {
                let facts = self.facts;
                let target = self.completion_target(call)?;
                let coverage = completions::ReturnCoverage::invocation(
                    &facts.flow.control,
                    call,
                    target,
                    |operand| self.completion_calls(operand),
                )?;
                let ready = coverage.established(|state| {
                    self.relations
                        .is_some_and(|relations| relations.productive(state))
                });
                self.observed.as_deref_mut()?.completed(coverage);
                return Some(ready);
            }
            Some(true)
        }

        fn completion_target(
            &mut self,
            call: &checked_trees::FlowCallFact,
        ) -> Option<Option<SymbolHandle>> {
            if let Some(known) = self.calls.known_target(call.target_symbol) {
                return known;
            }
            let classified = self.classify_completion_target(call.target_symbol);
            self.calls.record_target(call.target_symbol, classified)
        }

        fn classify_completion_target(
            &mut self,
            target: SymbolHandle,
        ) -> Option<Option<SymbolHandle>> {
            if !crate::call_target_has_checked_body_contract(self.program, target) {
                return Some(None);
            }
            let state = crate::call_target_state(self.program, target)?;
            if self.reference_leaves(state)?.is_empty() {
                Some(None)
            } else {
                Some(Some(state.symbol))
            }
        }

        fn completion_calls(
            &mut self,
            operand: &checked_trees::FlowOperandFact,
        ) -> Option<Vec<SymbolHandle>> {
            let control = &self.facts.flow.control;
            let call = if let checked_trees::FlowOperandCompletion::InvocationResult {
                invocation,
                ..
            } = operand.completion
            {
                Some(
                    self.calls
                        .invocation(control, operand.state_symbol, invocation)?,
                )
            } else {
                self.calls
                    .ordinary(control, operand)?
                    .map(|call| control.calls.get(call))
            };
            let mut targets = Vec::new();
            if let Some(call) = call
                && let Some(target) = self.completion_target(call)?
            {
                targets.push(target);
            }
            Some(targets)
        }

        fn ordinary_return_call(
            &mut self,
            state: &State,
            statement_index: usize,
            target: typed_trees::statement::TransitionTargetHandle,
            expression: ExpressionHandle,
        ) -> Option<Option<&'capture checked_trees::FlowCallFact>> {
            let Some(call) = expression_table(self.program).call_node(expression) else {
                return Some(None);
            };
            let callable = crate::call_target_symbol(self.program, call);
            if crate::call_target_parameters(self.program, callable).is_none()
                || crate::call_target_return_type(self.program, callable).is_none()
            {
                return Some(None);
            }
            Some(Some(self.return_call(
                state,
                statement_index,
                target,
                expression,
            )?))
        }

        fn return_call(
            &mut self,
            state: &State,
            statement_index: usize,
            target: typed_trees::statement::TransitionTargetHandle,
            expression: ExpressionHandle,
        ) -> Option<&'capture checked_trees::FlowCallFact> {
            let flow = self.flow(state.symbol)?;
            let machine = self.program.machine_by_symbol(flow.machine_symbol)?;
            let mut calls = self
            .facts
            .flow
            .control
            .calls
            .span(flow.calls)?
            .iter()
            .filter(|call| {
                call.statement_index == statement_index
                    && call.authored_expression == expression
                    && (!target.is_valid()
                        || crate::transition_call_target(
                            self.program,
                            machine,
                            state,
                            statement_index,
                            call.call_ordinal,
                        ) == Some(target))
                    && (!expression.is_valid() || {
                        expression_table(self.program).call_node(expression).is_some_and(|source| {
                                crate::call_target_entry_symbol(
                                    self.program,
                                    crate::call_target_symbol(self.program, source),
                                ) == call.target_symbol
                                    && matches!(
                                        crate::find_call_site_in_state(
                                            self.program, machine, state,
                                            statement_index, call.call_ordinal,
                                        ),
                                        Some(crate::CallSite::Expression { expression: actual, call: selected })
                                            if actual == expression && std::ptr::eq(selected, source)
                                    )
                            })
                    })
            });
            let call = calls.next()?;
            calls.next().is_none().then_some(call)
        }

        fn return_origins(
            &mut self,
            state: &State,
            loan: &checked_trees::BorrowReturnedLoanFact,
            leaf: &CheckedReferenceResultSourcePlan,
            expression: Option<ExpressionHandle>,
            loans: &[checked_trees::BorrowReturnedLoanFact],
        ) -> Option<Vec<CheckedReferenceOriginSourcePlan>> {
            if loan.lineage == checked_trees::BorrowLoanLineage::LifetimeDependency {
                return None;
            }
            let values = &self.facts.values.structural_values;
            if let Some(expression) = expression {
                let root = values
                    .root_for_expression(
                        state.symbol,
                        u32::try_from(loan.statement_index).ok()?,
                        expression,
                    )
                    .map(|root| root.root);
                if let Some(root) = root {
                    let sources =
                        stored_carrier_sources(self.program, self.facts, root, &leaf.path)?;
                    if sources.iter().any(|source| match source {
                        StoredCarrierSource::Image { value } => {
                            let image = values.nodes.get(*value);
                            image.state == state.symbol
                                && image.statement_index as usize == loan.statement_index
                                && image.expression == loan.formation_expression
                        }
                        _ => false,
                    }) {
                        return Some(Vec::new());
                    }
                }
            }
            let segments = self.facts.borrow.access_segments.span(loan.segments)?;
            if let Some(source) = self.return_origin(state, loan, leaf, segments, loans) {
                return Some(source);
            }
            if loan.incoming_reference.is_valid() {
                return None;
            }
            let operand = self.completed_return_operand(
                state,
                loan.statement_index,
                loan.transition_target,
                expression?,
                &leaf.path,
            )?;
            if operand.expression != loan.formation_expression {
                return None;
            }
            let referents = self
                .facts
                .flow
                .control
                .operand_referents
                .span(operand.referents)?;
            if referents.is_empty() {
                return None;
            }
            let owner_path = self.returned_loan_owner_path(state, loan)?;
            let mut output = Vec::new();
            for referent in referents {
                let captured = self
                    .facts
                    .flow
                    .control
                    .operand_referent_segments
                    .span(referent.segments)?;
                if !loans.iter().any(|candidate| {
                    candidate.lineage != checked_trees::BorrowLoanLineage::LifetimeDependency
                        && candidate.statement_index == loan.statement_index
                        && candidate.transition_target == loan.transition_target
                        && candidate.formation_expression == operand.expression
                        && candidate.root == referent.root
                        && candidate.kind == loan.kind
                        && !candidate.incoming_reference.is_valid()
                        && self.returned_loan_owner_path(state, candidate).as_deref()
                            == Some(owner_path.as_slice())
                        && self
                            .facts
                            .borrow
                            .access_segments
                            .span(candidate.segments)
                            .is_some_and(|segments| captured_address_refines(segments, captured))
                }) {
                    return None;
                }
                if referent.root == loan.root && captured_address_refines(segments, captured) {
                    for origin in self.return_origin(state, loan, leaf, captured, loans)? {
                        if !output.contains(&origin) {
                            output.push(origin);
                        }
                    }
                }
            }
            (!output.is_empty()).then_some(output)
        }

        fn completed_return_operand(
            &self,
            state: &State,
            statement_index: usize,
            transition_target: typed_trees::statement::TransitionTargetHandle,
            expression: ExpressionHandle,
            path: &[CheckedUnitStructuralPathSegment],
        ) -> Option<&'capture checked_trees::FlowOperandFact> {
            let operand =
                self.completed_return_value(state, statement_index, transition_target, expression)?;
            self.completed_operand_leaf(operand, path)
        }

        fn completed_return_value(
            &self,
            state: &State,
            statement_index: usize,
            transition_target: typed_trees::statement::TransitionTargetHandle,
            expression: ExpressionHandle,
        ) -> Option<&'capture checked_trees::FlowOperandFact> {
            let flow = self.flow(state.symbol)?;
            let control = &self.facts.flow.control;
            let mut exits = control.exits.span(flow.exits)?.iter().filter(|exit| {
                exit.machine_symbol == flow.machine_symbol
                    && exit.state_symbol == state.symbol
                    && exit.statement_index == statement_index
                    && exit.transition_target == transition_target
            });
            let exit = exits.next()?;
            if exits.next().is_some() || !control.operands.is_valid(exit.returned_value) {
                return None;
            }
            let operand = control.operands.get(exit.returned_value);
            if operand.state_symbol != state.symbol
                || operand.statement_index != statement_index
                || operand.expression != expression
                || self
                    .program
                    .normalized_type_identity(operand.type_reference)
                    != self.program.normalized_type_identity(state.return_type)
            {
                return None;
            }
            Some(operand)
        }

        fn return_origin(
            &self,
            state: &State,
            loan: &checked_trees::BorrowReturnedLoanFact,
            leaf: &CheckedReferenceResultSourcePlan,
            segments: &[facts::PlaceSegment],
            loans: &[checked_trees::BorrowReturnedLoanFact],
        ) -> Option<Vec<CheckedReferenceOriginSourcePlan>> {
            self.return_origin_with_frame(state, loan, leaf, segments, loans, None)
        }

        fn return_origin_with_frame(
            &self,
            state: &State,
            loan: &checked_trees::BorrowReturnedLoanFact,
            leaf: &CheckedReferenceResultSourcePlan,
            segments: &[facts::PlaceSegment],
            loans: &[checked_trees::BorrowReturnedLoanFact],
            captured: Option<&[CapturedReturnOrigin]>,
        ) -> Option<Vec<CheckedReferenceOriginSourcePlan>> {
            if loan.lineage == checked_trees::BorrowLoanLineage::LifetimeDependency {
                return None;
            }
            if self
                .facts
                .borrow
                .incoming_references
                .is_valid(loan.incoming_reference)
                && matches!(
                    self.facts
                        .borrow
                        .incoming_references
                        .get(loan.incoming_reference)
                        .origin,
                    IncomingReferenceOrigin::DescriptorLoad { .. }
                )
            {
                return self.loaded_descriptor_origins(state, loan, leaf, segments);
            }
            if loan.incoming_reference.is_valid()
                && self
                    .facts
                    .borrow
                    .incoming_references
                    .is_valid(loan.incoming_reference)
            {
                let incoming = self
                    .facts
                    .borrow
                    .incoming_references
                    .get(loan.incoming_reference);
                if let IncomingReferenceOrigin::Invocation { expression, target } = incoming.origin
                {
                    if incoming.state_symbol != state.symbol
                        || loan.root != facts::PlaceRoot::Expression(expression)
                        || !matches!(expression_table(self.program).expression(expression), typed_trees::expression::ExpressionNode::Call(call) if crate::call_target_symbol(self.program, call) == target)
                    {
                        return None;
                    }
                    let (invocation_carrier, descriptor_path) =
                        self.invocation_carrier(state, incoming)?;
                    let shell = crate::reference_shell(self.program, incoming.carrier_type)?;
                    if !reference_access_can_supply(shell.access.into(), leaf.source.access) {
                        return None;
                    }
                    let references =
                        super::declared_carrier_references(self.program, incoming.carrier_type)?;
                    let [reference] = references.as_slice() else {
                        return None;
                    };
                    if !reference.path.is_empty() {
                        return None;
                    }
                    let (selected, prefix) = if reference.crosses_reference {
                        (
                            shell.referee,
                            vec![CheckedReferenceOriginProjection::Exact(
                                CheckedUnitStructuralPathSegment::Referent,
                            )],
                        )
                    } else {
                        (incoming.carrier_type, Vec::new())
                    };
                    let (suffix, endpoint, extent) =
                        super::reference_origin_path(self.program, selected, segments)?;
                    let identity = self.program.normalized_type_identity(endpoint);
                    if identity != leaf.source.type_identity {
                        return None;
                    }
                    return Some(vec![CheckedReferenceOriginSourcePlan {
                        root: checked_trees::CheckedReferenceOriginRoot::Invocation {
                            carrier_type: self.program.normalized_type_identity(invocation_carrier),
                        },
                        entry_reference_hops: 0,
                        path: descriptor_path
                            .into_iter()
                            .chain(prefix)
                            .chain(suffix)
                            .collect(),
                        extent,
                        type_identity: identity,
                        access: leaf.source.access,
                    }]);
                }
            }
            let (parameter_index, parameter) = self
                .program
                .state_parameters(state)
                .iter()
                .filter(|parameter| is_mutable_structural_parameter(self.program, parameter))
                .enumerate()
                .find(|(_, parameter)| loan.root == facts::PlaceRoot::Symbol(parameter.symbol))?;
            let mut path = Vec::new();
            let entry_reference_hops = 0;
            let mut incoming_segments = Vec::new();
            let mut selected = parameter.type_reference;
            if loan.incoming_reference.is_valid() {
                let selected_incoming = self.incoming_leaf(state, loan)?;
                let carrier = selected_incoming.carrier;
                let incoming_leaf = selected_incoming.reference;
                if !reference_access_can_supply(incoming_leaf.access, leaf.source.access) {
                    return None;
                }
                let incoming_path = selected_incoming.segments;
                selected = crate::project_type_reference_from_segments(
                    self.program,
                    carrier,
                    &incoming_path,
                )?;
                if incoming_leaf.crosses_reference {
                    selected = stored_loan_parts(self.program, selected)?.0;
                }
                path.extend(incoming_leaf.path);
                if incoming_leaf.crosses_reference {
                    path.push(CheckedReferenceOriginProjection::Exact(
                        CheckedUnitStructuralPathSegment::Referent,
                    ));
                }
                incoming_segments = incoming_path;
            } else {
                let shell = crate::reference_shell(self.program, selected)?;
                if !reference_access_can_supply(shell.access.into(), leaf.source.access) {
                    return None;
                }
                if self.program.normalized_type_identity(selected) != leaf.source.type_identity {
                    selected = shell.referee;
                }
            }
            let mut segments = segments.to_vec();
            facts::compose_captured_view_segments(&mut segments)?;
            let parameter_index = u32::try_from(parameter_index).ok()?;
            let result_path = loan_place_segments(self.program, state.return_type, &leaf.path)?;
            let inferred = if captured.is_none() {
                crate::CallFrameResolver::new(self.program)?
                    .reference_result_leaf_origins(state.symbol)
                    .map(|origins| {
                        origins
                            .into_iter()
                            .map(|origin| CapturedReturnOrigin {
                                result_path: origin.result_path,
                                source: origin.source,
                                extent: ReferenceOriginExtent::Projected,
                                presentation: None,
                            })
                            .collect::<Vec<_>>()
                    })
            } else {
                None
            };
            let origins = captured.or(inferred.as_deref());
            let has_presentation = origins.as_ref().is_some_and(|origins| {
                origins.iter().any(|origin| {
                    origin.result_path == result_path
                        && (!origin.source.recasts.is_empty()
                            || origin.source.representation.is_some())
                })
            }) || self.facts.values.structural_values.nodes.iter().any(
                |(_, value)| {
                    value.expression == loan.formation_expression
                        && matches!(
                            value.kind,
                            CheckedStructuralValueKind::Reference {
                                storage: checked_trees::CheckedReferenceStorage::View { .. },
                                ..
                            }
                        )
                },
            );
            if captured.is_none()
                && !has_presentation
                && let Some((suffix, endpoint, extent)) =
                    super::reference_origin_path(self.program, selected, &segments)
            {
                let identity = self.program.normalized_type_identity(endpoint);
                if let Some(extent) = self.result_origin_extent(
                    endpoint,
                    state.return_type,
                    &result_path,
                    leaf.source.type_identity,
                    extent,
                ) {
                    path.extend(suffix);
                    return Some(vec![CheckedReferenceOriginSourcePlan {
                        root: checked_trees::CheckedReferenceOriginRoot::Parameter {
                            index: parameter_index,
                        },
                        entry_reference_hops,
                        path,
                        extent,
                        type_identity: identity,
                        access: leaf.source.access,
                    }]);
                }
            }
            if let Some(origins) = origins {
                let owner_path = self.returned_loan_owner_path(state, loan)?;
                for origin in origins
                    .iter()
                    .filter(|origin| origin.result_path == result_path)
                {
                    if !origin.source.root.is_valid()
                        || !loans.iter().any(|candidate| {
                            candidate.root == facts::PlaceRoot::Symbol(origin.source.root)
                                && candidate.kind == loan.kind
                                && self.returned_loan_owner_path(state, candidate).as_deref()
                                    == Some(owner_path.as_slice())
                                && self.origin_matches_returned_loan(state, candidate, origin)
                        })
                    {
                        return None;
                    }
                }
                let matching = origins
                    .iter()
                    .filter(|origin| {
                        origin.result_path == result_path
                            && loan.root == facts::PlaceRoot::Symbol(origin.source.root)
                            && self.origin_matches_returned_loan(state, loan, origin)
                    })
                    .collect::<Vec<_>>();
                if !matching.is_empty() {
                    let mut output = Vec::new();
                    for origin in matching {
                        let captured =
                            self.frame_origin_path(selected, &incoming_segments, &origin.source)?;
                        let source = self.publish_frame_origin(
                            state,
                            leaf,
                            origin,
                            captured,
                            parameter_index,
                            path.clone(),
                        )?;
                        if !output.contains(&source) {
                            output.push(source);
                        }
                    }
                    return (!output.is_empty()).then_some(output);
                }
            }
            if captured.is_some() {
                return None;
            }
            let (suffix, mut endpoint, extent) =
                super::reference_origin_path(self.program, selected, &segments)?;
            if extent == ReferenceOriginExtent::WithinRegion
                && self.program.normalized_type_identity(endpoint) != leaf.source.type_identity
                && !self.collection_region_presents_result(
                    endpoint,
                    state.return_type,
                    &result_path,
                )
            {
                return None;
            }
            path.extend(suffix);
            if path.is_empty()
                && stored_loan_parts(self.program, parameter.type_reference).is_none()
                && crate::reference_shell(self.program, parameter.type_reference)
                    .is_some_and(|shell| shell.referee == endpoint)
            {
                endpoint = parameter.type_reference;
            }
            Some(vec![CheckedReferenceOriginSourcePlan {
                root: checked_trees::CheckedReferenceOriginRoot::Parameter {
                    index: parameter_index,
                },
                entry_reference_hops,
                path,
                extent: semantic_vocabulary::ReferenceOriginExtent::WithinRegion,
                type_identity: self.program.normalized_type_identity(endpoint),
                access: leaf.source.access,
            }])
        }

        fn result_origin_extent(
            &self,
            source: TypeReferenceHandle,
            result: TypeReferenceHandle,
            result_path: &[facts::PlaceSegment],
            result_identity: typed_trees::type_identity::TypeIdentityId,
            extent: ReferenceOriginExtent,
        ) -> Option<ReferenceOriginExtent> {
            let source_identity = self.program.normalized_type_identity(source);
            if !source_identity.is_valid() {
                return None;
            }
            if source_identity == result_identity {
                return Some(extent);
            }
            self.collection_region_presents_result(source, result, result_path)
                .then_some(ReferenceOriginExtent::WithinRegion)
        }

        fn collection_region_presents_result(
            &self,
            source: TypeReferenceHandle,
            result: TypeReferenceHandle,
            result_path: &[facts::PlaceSegment],
        ) -> bool {
            let Some(result) =
                crate::project_type_reference_from_segments(self.program, result, result_path)
                    .and_then(|result| crate::slice_view_shell(self.program, result, &[]))
            else {
                return false;
            };
            let source =
                crate::reference_shell(self.program, source).map_or(source, |shell| shell.referee);
            let source = crate::peel_declared_constraints(self.program, source);
            let (TypeReferenceNode::FixedArray { element_type, .. }
            | TypeReferenceNode::Slice { element_type }) =
                self.program.type_reference_table.type_reference(source)
            else {
                return false;
            };
            let source_element = self.program.normalized_type_identity(*element_type);
            let result_element = self.program.normalized_type_identity(result.element);
            source_element.is_valid()
                && result_element.is_valid()
                && source_element == result_element
        }

        fn publish_frame_origin(
            &self,
            state: &State,
            leaf: &CheckedReferenceResultSourcePlan,
            origin: &CapturedReturnOrigin,
            captured: FrameOrigin,
            parameter_index: u32,
            path: Vec<CheckedReferenceOriginProjection>,
        ) -> Option<CheckedReferenceOriginSourcePlan> {
            let retained_extent = if origin.extent == ReferenceOriginExtent::WithinRegion
                || captured.extent == ReferenceOriginExtent::WithinRegion
            {
                ReferenceOriginExtent::WithinRegion
            } else {
                ReferenceOriginExtent::Projected
            };
            let mut source_identity = self.program.normalized_type_identity(captured.backing);
            let extent = if let Some(presentation) = &origin.presentation
                && (!presentation.path.is_empty()
                    || presentation.extent == ReferenceOriginExtent::WithinRegion)
            {
                if self.program.normalized_type_identity(captured.presentation)
                    != presentation.source_type
                    || presentation.result_type != leaf.source.type_identity
                    || !presentation.endpoint_type.is_valid()
                {
                    return None;
                }
                source_identity = presentation.endpoint_type;
                retained_extent
            } else {
                self.result_origin_extent(
                    captured.presentation,
                    state.return_type,
                    &origin.result_path,
                    leaf.source.type_identity,
                    retained_extent,
                )?
            };
            let mut complete = path;
            complete.extend(captured.path);
            if let Some(presentation) = &origin.presentation {
                complete.extend(presentation.path.iter().cloned());
            }
            Some(CheckedReferenceOriginSourcePlan {
                root: checked_trees::CheckedReferenceOriginRoot::Parameter {
                    index: parameter_index,
                },
                entry_reference_hops: 0,
                path: complete,
                extent,
                type_identity: source_identity,
                access: leaf.source.access,
            })
        }

        fn frame_origin_path(
            &self,
            mut selected: TypeReferenceHandle,
            incoming: &[facts::PlaceSegment],
            origin: &crate::LocalWriteSource,
        ) -> Option<FrameOrigin> {
            if origin.recasts.is_empty() {
                if origin.representation.is_some() {
                    return None;
                }
                let (path, backing, extent) = super::reference_origin_path(
                    self.program,
                    selected,
                    origin.segments.strip_prefix(incoming)?,
                )?;
                return Some(FrameOrigin {
                    path,
                    backing,
                    presentation: backing,
                    extent,
                });
            }
            let mut path = Vec::new();
            let mut extent = ReferenceOriginExtent::Projected;
            let mut access_backing = TypeReferenceHandle::invalid();
            for (ordinal, recast) in origin.recasts.iter().enumerate() {
                if recast.root != origin.root || access_backing.is_valid() {
                    return None;
                }
                let selectors = if ordinal == 0 {
                    recast.segments.strip_prefix(incoming)?
                } else {
                    let (previous, selectors) = recast.representation.as_ref()?;
                    if self.program.normalized_type_identity(*previous)
                        != self.program.normalized_type_identity(selected)
                    {
                        return None;
                    }
                    selectors
                };
                let (projection, endpoint, selection_extent) =
                    super::reference_origin_path(self.program, selected, selectors)?;
                if selection_extent == ReferenceOriginExtent::WithinRegion {
                    extent = ReferenceOriginExtent::WithinRegion;
                }
                path.extend(projection);
                selected = endpoint;
                if let Some(shell) = crate::reference_shell(self.program, selected) {
                    path.push(CheckedReferenceOriginProjection::Exact(
                        CheckedUnitStructuralPathSegment::Referent,
                    ));
                    selected = shell.referee;
                }
                if let crate::ReferenceViewFormation::ExtentView(_) = recast.formation {
                    let (storage, view) = recast.formation.extent_view_types(self.program)?;
                    let storage = crate::reference_shell(self.program, storage)?.referee;
                    if self.program.normalized_type_identity(
                        self.program
                            .type_reference_table
                            .peeled_type_reference(selected),
                    ) != self.program.normalized_type_identity(
                        self.program
                            .type_reference_table
                            .peeled_type_reference(storage),
                    ) || self.program.normalized_type_identity(view)
                        != self.program.normalized_type_identity(recast.target_type)
                    {
                        return None;
                    }
                    access_backing = selected;
                    extent = ReferenceOriginExtent::WithinRegion;
                    selected = recast.target_type;
                    continue;
                }
                let mut formations =
                    self.facts
                        .values
                        .structural_values
                        .nodes
                        .iter()
                        .filter_map(|(_, value)| {
                            if value.expression != recast.formation.expression() {
                                return None;
                            }
                            let CheckedStructuralValueKind::Reference {
                                storage,
                                backing_type,
                                ..
                            } = &value.kind
                            else {
                                return None;
                            };
                            Some((value, *storage, *backing_type))
                        });
                let (formation, storage, backing_type) = formations.next()?;
                let authored =
                    crate::reference_formation_source(self.program, recast.formation.expression())?;
                let expected_storage = if authored.interpretation.is_access_view() {
                    let view = self.program.closed_data_definition(recast.target_type)?;
                    let mut plans = self
                        .program
                        .access_view_plans
                        .iter()
                        .filter(|plan| plan.data_symbol == view.symbol);
                    let (Some(plan), None) = (plans.next(), plans.next()) else {
                        return None;
                    };
                    authored
                        .interpretation
                        .access_storage(self.program, selected, plan)?
                } else {
                    crate::reference_view_storage(self.program, selected, recast.target_type)?
                };
                let backing_identity =
                    crate::stored_structural_type_identity(self.program, backing_type, &[], &[]);
                let selected_identity =
                    crate::stored_structural_type_identity(self.program, selected, &[], &[]);
                if formations.next().is_some()
                    || !backing_identity.is_valid()
                    || !selected_identity.is_valid()
                    || backing_identity != selected_identity
                    || storage != expected_storage
                    || self.program.normalized_type_identity(
                        crate::reference_shell(self.program, formation.type_reference)?.referee,
                    ) != self.program.normalized_type_identity(recast.target_type)
                {
                    return None;
                }
                self.program.state_by_symbol(formation.state)?;
                if authored.interpretation.is_access_view() {
                    access_backing = selected;
                    extent = ReferenceOriginExtent::WithinRegion;
                }
                if matches!(storage, checked_trees::CheckedReferenceStorage::View { .. }) {
                    path.push(CheckedReferenceOriginProjection::Exact(
                        CheckedUnitStructuralPathSegment::RepresentationView {
                            type_identity: crate::stored_structural_type_identity(
                                self.program,
                                recast.target_type,
                                &[],
                                &[],
                            ),
                            storage,
                        },
                    ));
                }
                selected = recast.target_type;
            }
            let (presentation, selectors) = origin.representation.as_ref()?;
            if self.program.normalized_type_identity(*presentation)
                != self.program.normalized_type_identity(selected)
            {
                return None;
            }
            if access_backing.is_valid() {
                if !selectors.is_empty() {
                    return None;
                }
                return Some(FrameOrigin {
                    path,
                    backing: access_backing,
                    presentation: selected,
                    extent,
                });
            }
            let (suffix, endpoint, selection_extent) =
                super::reference_origin_path(self.program, selected, selectors)?;
            if selection_extent == ReferenceOriginExtent::WithinRegion {
                extent = ReferenceOriginExtent::WithinRegion;
            }
            path.extend(suffix);
            Some(FrameOrigin {
                path,
                backing: endpoint,
                presentation: endpoint,
                extent,
            })
        }

        fn flow(&self, state: SymbolHandle) -> Option<&'capture checked_trees::FlowStateFact> {
            self.facts.flow.control.unique_state_by_symbol(state)
        }

        fn returned_loan_owner_path(
            &self,
            state: &State,
            loan: &checked_trees::BorrowReturnedLoanFact,
        ) -> Option<Vec<checked_trees::BorrowLoanOwnerSegment>> {
            crate::corresponding_loan_owner_path(
                self.program,
                state.return_type,
                self.facts.borrow.owner_segments.span(loan.owner_path)?,
            )
        }

        fn reference_leaves(&self, state: &State) -> Option<Vec<CheckedReferenceResultSourcePlan>> {
            record_carrier_sources(
                self.program,
                state.return_type,
                CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                    symbol: state.symbol,
                },
            )
        }
    }

    fn frame_origin_fits_loan(
        origin: &crate::LocalWriteSource,
        extent: ReferenceOriginExtent,
        loan: &[facts::PlaceSegment],
    ) -> bool {
        let within = |candidate: &[facts::PlaceSegment]| {
            captured_address_refines(candidate, loan)
                || (extent == ReferenceOriginExtent::WithinRegion
                    && matches!(loan.split_last(), Some((facts::PlaceSegment::OpaqueRange, parent))
                    if captured_address_refines(candidate, parent)))
                || candidate
                    .get(..loan.len())
                    .is_some_and(|prefix| captured_address_refines(loan, prefix))
                || match (candidate.split_last(), loan.split_last()) {
                    (
                        Some((facts::PlaceSegment::OpaqueRange, candidate)),
                        Some((facts::PlaceSegment::OpaqueRange, loan)),
                    ) => captured_address_refines(loan, candidate),
                    _ => false,
                }
        };
        if within(&origin.segments) {
            return true;
        }
        origin
            .recasts
            .first()
            .is_some_and(|formation| formation.root == origin.root && within(&formation.segments))
    }

    fn captured_address_refines(
        segments: &[facts::PlaceSegment],
        captured: &[facts::PlaceSegment],
    ) -> bool {
        use facts::PlaceSegment;
        let mut segments = segments.to_vec();
        if facts::compose_captured_view_segments(&mut segments).is_none() {
            return false;
        }
        let mut remaining = segments.as_slice();
        for &coordinate in captured {
            remaining = match (remaining, coordinate) {
                (
                    [
                        PlaceSegment::FixedRange { start, end },
                        PlaceSegment::Index { .. },
                        suffix @ ..,
                    ],
                    PlaceSegment::FixedIndex { index },
                ) if *start <= index && index < *end => suffix,
                ([PlaceSegment::Index { .. }, suffix @ ..], PlaceSegment::FixedIndex { .. }) => {
                    suffix
                }
                ([expected, suffix @ ..], actual)
                    if *expected == actual && actual != PlaceSegment::OpaqueRange =>
                {
                    suffix
                }
                _ => return false,
            };
        }
        remaining.is_empty()
    }

    #[cfg(test)]
    pub(crate) fn paths_overlap(
        left: &[facts::PlaceSegment],
        right: &[facts::PlaceSegment],
    ) -> bool {
        for (left, right) in left.iter().zip(right) {
            if left == right {
                continue;
            }
            return !matches!(
                (left, right),
                (
                    facts::PlaceSegment::Field { .. },
                    facts::PlaceSegment::Field { .. }
                ) | (
                    facts::PlaceSegment::Case { .. },
                    facts::PlaceSegment::Case { .. }
                ) | (
                    facts::PlaceSegment::FixedIndex { .. },
                    facts::PlaceSegment::FixedIndex { .. }
                )
            );
        }
        true
    }

    #[cfg(test)]
    mod tests {
        use super::{captured_address_refines, paths_overlap};
        use facts::PlaceSegment;

        #[test]
        fn opaque_windows_overlap_without_exact_address_identity() {
            let unknown = [PlaceSegment::OpaqueRange];
            let fixed = [PlaceSegment::FixedRange { start: 1, end: 4 }];
            assert!(paths_overlap(&unknown, &unknown));
            assert!(paths_overlap(&unknown, &fixed));
            assert!(!captured_address_refines(&unknown, &unknown));
            assert!(!captured_address_refines(&unknown, &fixed));
            assert!(!captured_address_refines(&fixed, &unknown));
            assert!(captured_address_refines(&fixed, &fixed));
        }
    }
}
pub(crate) mod completed_loads {
    use arena::Handle;
    use checked_trees::{
        CheckFacts, FlowOperandFact, FlowStateFact, IncomingReferenceIdentity,
        IncomingReferenceOrigin,
    };
    use facts::PlaceRoot;

    pub(crate) mod prepared_slots {
        use super::{activation_matches_load, complete_descriptor_load, unchanged_input_slot};
        use arena::{Arena, Handle};
        use checked_trees::{
            BorrowLoanFact, CheckFacts, FlowBorrowActivationFact, IncomingReferenceIdentity,
            IncomingReferenceOrigin,
        };

        pub(crate) struct Slot {
            place: crate::CanonicalPlace,
            statement_index: usize,
        }

        pub(crate) struct PreparedSlot {
            identity: Handle<IncomingReferenceIdentity>,
            answer: Result<Slot, &'static str>,
        }

        #[derive(Clone, Copy, Default)]
        pub(crate) enum Activation<'facts> {
            #[default]
            Missing,
            Unique(&'facts FlowBorrowActivationFact),
            Ambiguous,
        }

        pub struct PreparedDescriptorLoadSlots<'facts> {
            loans: &'facts Arena<BorrowLoanFact>,
            incoming: &'facts Arena<IncomingReferenceIdentity>,
            slots: SlotRoster,
        }

        pub(crate) enum SlotRoster {
            All(Vec<Option<PreparedSlot>>),
            State(Vec<PreparedSlot>),
            UnavailableState,
        }

        impl SlotRoster {
            fn get(&self, identity: Handle<IncomingReferenceIdentity>) -> Option<&PreparedSlot> {
                match self {
                    Self::All(slots) => slot_index(identity)
                        .and_then(|index| slots.get(index))
                        .and_then(Option::as_ref),
                    Self::UnavailableState => None,
                    Self::State(slots) => slots
                        .binary_search_by_key(&handle_key(identity), |slot| {
                            handle_key(slot.identity)
                        })
                        .ok()
                        .map(|index| &slots[index]),
                }
                .filter(|slot| slot.identity == identity)
            }
        }

        impl<'facts> PreparedDescriptorLoadSlots<'facts> {
            pub fn new(program: &dyn crate::CheckingInput, facts: &'facts CheckFacts) -> Self {
                Self::prepare(program, facts)
            }

            pub fn for_state(
                program: &dyn crate::CheckingInput,
                facts: checked_trees::CheckFactsRef<'facts>,
                state: &checked_trees::FlowStateFact,
            ) -> Self {
                let slots = facts
                    .flow
                    .control
                    .unique_state(state.machine_symbol, state.state_symbol)
                    .map(|state| SlotRoster::State(state_slots(program, facts, state)))
                    .unwrap_or(SlotRoster::UnavailableState);
                Self {
                    loans: &facts.borrow.loans,
                    incoming: &facts.borrow.incoming_references,
                    slots,
                }
            }

            fn prepare(program: &dyn crate::CheckingInput, facts: &'facts CheckFacts) -> Self {
                let incoming = &facts.borrow.incoming_references;
                let capacity = incoming.storage_slice().len();
                let mut activations = vec![Activation::Missing; capacity];
                let mut states = facts
                    .flow
                    .control
                    .states
                    .iter()
                    .map(|(_, state)| state)
                    .collect::<Vec<_>>();
                states.sort_unstable_by_key(|state| {
                    state_key(state.machine_symbol, state.state_symbol)
                });
                for index in 0..states.len() {
                    let Some(state) = unique_state_at(&states, index) else {
                        continue;
                    };
                    for activation in facts
                        .flow
                        .borrow_lifetimes
                        .activations
                        .span_or_empty(state.borrow_activations)
                    {
                        let Some((identity, _)) = matching_load(facts.view(), state, activation)
                        else {
                            continue;
                        };
                        let Some(slot) =
                            slot_index(identity).and_then(|index| activations.get_mut(index))
                        else {
                            continue;
                        };
                        *slot = match *slot {
                            Activation::Missing => Activation::Unique(activation),
                            Activation::Unique(_) | Activation::Ambiguous => Activation::Ambiguous,
                        };
                    }
                }
                let mut slots = Vec::with_capacity(capacity);
                slots.resize_with(capacity, || None);
                for (identity, source) in incoming.iter() {
                    let IncomingReferenceOrigin::DescriptorLoad { expression } = source.origin
                    else {
                        continue;
                    };
                    let Some(index) = slot_index(identity) else {
                        continue;
                    };
                    let answer = (|| {
                        let state = states
                            .binary_search_by_key(
                                &state_key(source.machine_symbol, source.state_symbol),
                                |state| state_key(state.machine_symbol, state.state_symbol),
                            )
                            .ok()
                            .and_then(|index| unique_state_at(&states, index))
                            .ok_or("the descriptor load has no unique completed state")?;
                        let activation = match activations.get(index) {
                            Some(Activation::Unique(activation)) => *activation,
                            Some(Activation::Ambiguous) => {
                                return Err("the incoming pointer has no unique completed load");
                            }
                            Some(Activation::Missing) | None => {
                                return Err("the incoming pointer has no actual load activation");
                            }
                        };
                        completed_slot(program, facts.view(), state, expression, activation)
                    })();
                    slots[index] = Some(PreparedSlot { identity, answer });
                }
                Self {
                    loans: &facts.borrow.loans,
                    incoming,
                    slots: SlotRoster::All(slots),
                }
            }

            pub fn slot(
                &self,
                loan: &BorrowLoanFact,
            ) -> Result<&crate::CanonicalPlace, &'static str> {
                if self.loans.handle_of(loan).is_none()
                    || !self.incoming.is_valid(loan.incoming_reference)
                {
                    return Err("the descriptor has no incoming load identity");
                }
                let incoming = self.incoming.get(loan.incoming_reference);
                let IncomingReferenceOrigin::DescriptorLoad { expression } = incoming.origin else {
                    return Err("the descriptor does not originate at a completed load");
                };
                if loan.storage_owner != facts::PlaceRoot::Expression(expression)
                    || loan.formation_expression != expression
                {
                    return Err("the loan does not own the completed load's descriptor home");
                }
                let prepared = self
                    .slots
                    .get(loan.incoming_reference)
                    .ok_or("the descriptor has no prepared source-slot association")?;
                let slot = prepared.answer.as_ref().map_err(|reason| *reason)?;
                if slot.statement_index != loan.statement_index {
                    return Err("the descriptor loan differs from its completed read statement");
                }
                Ok(&slot.place)
            }
        }

        fn state_slots(
            program: &dyn crate::CheckingInput,
            facts: checked_trees::CheckFactsRef<'_>,
            state: &checked_trees::FlowStateFact,
        ) -> Vec<PreparedSlot> {
            let mut activations = facts
                .flow
                .borrow_lifetimes
                .activations
                .span_or_empty(state.borrow_activations)
                .iter()
                .filter_map(|activation| {
                    matching_load(facts, state, activation)
                        .map(|(identity, expression)| (identity, expression, activation))
                })
                .collect::<Vec<_>>();
            activations.sort_unstable_by_key(|(identity, _, _)| handle_key(*identity));
            activations
                .chunk_by(|left, right| left.0 == right.0)
                .map(|group| {
                    let (identity, expression, activation) = group[0];
                    let answer = if group.len() != 1 {
                        Err("the incoming pointer has no unique completed load")
                    } else {
                        completed_slot(program, facts, state, expression, activation)
                    };
                    PreparedSlot { identity, answer }
                })
                .collect()
        }

        fn matching_load(
            facts: checked_trees::CheckFactsRef<'_>,
            state: &checked_trees::FlowStateFact,
            activation: &FlowBorrowActivationFact,
        ) -> Option<(
            Handle<IncomingReferenceIdentity>,
            typed_trees::expression::ExpressionHandle,
        )> {
            if !facts.borrow.loans.is_valid(activation.loan) {
                return None;
            }
            let identity = facts.borrow.loans.get(activation.loan).incoming_reference;
            if !facts.borrow.incoming_references.is_valid(identity) {
                return None;
            }
            let source = facts.borrow.incoming_references.get(identity);
            let IncomingReferenceOrigin::DescriptorLoad { expression } = source.origin else {
                return None;
            };
            (source.machine_symbol == state.machine_symbol
                && source.state_symbol == state.state_symbol
                && activation_matches_load(facts, identity, expression, activation))
            .then_some((identity, expression))
        }

        fn completed_slot(
            program: &dyn crate::CheckingInput,
            facts: checked_trees::CheckFactsRef<'_>,
            state: &checked_trees::FlowStateFact,
            expression: typed_trees::expression::ExpressionHandle,
            activation: &FlowBorrowActivationFact,
        ) -> Result<Slot, &'static str> {
            let completed =
                complete_descriptor_load(program, facts, state, expression, activation)?;
            let statement_index = facts
                .flow
                .control
                .operands
                .get(completed.capture)
                .statement_index;
            Ok(Slot {
                place: unchanged_input_slot(completed.sources)?,
                statement_index,
            })
        }

        fn handle_key<T>(handle: Handle<T>) -> (u32, u32) {
            (handle.arena_index(), handle.generation())
        }

        fn slot_index<T>(handle: Handle<T>) -> Option<usize> {
            usize::try_from(handle.arena_index().checked_sub(1)?).ok()
        }

        fn state_key(
            machine: symbols::SymbolHandle,
            state: symbols::SymbolHandle,
        ) -> (u32, u32, u32, u32) {
            (
                machine.arena_index(),
                machine.generation(),
                state.arena_index(),
                state.generation(),
            )
        }

        fn unique_state_at<'facts>(
            states: &[&'facts checked_trees::FlowStateFact],
            index: usize,
        ) -> Option<&'facts checked_trees::FlowStateFact> {
            let state = *states.get(index)?;
            let key = state_key(state.machine_symbol, state.state_symbol);
            let same = |other: &&checked_trees::FlowStateFact| {
                state_key(other.machine_symbol, other.state_symbol) == key
            };
            if index
                .checked_sub(1)
                .and_then(|previous| states.get(previous))
                .is_some_and(same)
                || states.get(index + 1).is_some_and(same)
            {
                return None;
            }
            Some(state)
        }

        #[cfg(test)]
        pub(crate) mod tests {
            use super::*;

            #[test]
            fn prepared_descriptor_slots_keep_arena_and_generation_identity() {
                let expression = typed_trees::expression::ExpressionHandle::from_parts(7, 2);
                let mut incoming = Arena::new();
                let identity = incoming.insert(IncomingReferenceIdentity {
                    origin: IncomingReferenceOrigin::DescriptorLoad { expression },
                    ..Default::default()
                });
                let mut loans = Arena::new();
                let handle = loans.insert(BorrowLoanFact {
                    incoming_reference: identity,
                    storage_owner: facts::PlaceRoot::Expression(expression),
                    formation_expression: expression,
                    statement_index: 3,
                    ..Default::default()
                });
                let expected = crate::CanonicalPlace {
                    root: facts::PlaceRoot::Symbol(symbols::SymbolHandle::from_parts(5, 9)),
                    segments: Vec::new(),
                };
                let mut prepared = PreparedDescriptorLoadSlots {
                    loans: &loans,
                    incoming: &incoming,
                    slots: SlotRoster::All(vec![Some(PreparedSlot {
                        identity,
                        answer: Ok(Slot {
                            place: expected.clone(),
                            statement_index: 3,
                        }),
                    })]),
                };
                assert_eq!(prepared.slot(loans.get(handle)), Ok(&expected));
                let copied = loans.get(handle).clone();
                assert!(prepared.slot(&copied).is_err());
                let SlotRoster::All(slots) = &mut prepared.slots else {
                    panic!("test uses the full-pass roster");
                };
                slots[0].as_mut().unwrap().identity =
                    Handle::from_parts(identity.arena_index(), identity.generation() + 1);
                assert!(prepared.slot(loans.get(handle)).is_err());
                let SlotRoster::All(slots) = &mut prepared.slots else {
                    panic!("test uses the full-pass roster");
                };
                let entry = slots[0].as_mut().unwrap();
                entry.identity = identity;
                entry.answer = Err("different completed input slots");
                for _ in 0..2 {
                    assert_eq!(
                        prepared.slot(loans.get(handle)),
                        Err("different completed input slots")
                    );
                }
            }

            #[test]
            fn prepared_descriptor_states_reject_duplicates_without_merging_generations() {
                let machine = symbols::SymbolHandle::from_parts(4, 3);
                let first = checked_trees::FlowStateFact {
                    machine_symbol: machine,
                    state_symbol: symbols::SymbolHandle::from_parts(9, 2),
                    ..Default::default()
                };
                let newer = checked_trees::FlowStateFact {
                    state_symbol: symbols::SymbolHandle::from_parts(9, 5),
                    ..first.clone()
                };
                let duplicate = first.clone();
                let mut states = vec![&newer, &first, &duplicate];
                states.sort_unstable_by_key(|state| {
                    state_key(state.machine_symbol, state.state_symbol)
                });
                assert!(unique_state_at(&states, 0).is_none());
                assert!(unique_state_at(&states, 1).is_none());
                assert!(std::ptr::eq(unique_state_at(&states, 2).unwrap(), &newer));
            }

            #[test]
            fn prepared_descriptor_question_uses_a_closed_state_roster() {
                let identity = Handle::from_parts(900_000, 12);
                let roster = SlotRoster::State(vec![PreparedSlot {
                    identity,
                    answer: Err("different completed input slots"),
                }]);
                assert_eq!(
                    roster.get(identity).unwrap().answer.as_ref().err(),
                    Some(&"different completed input slots")
                );
                assert!(
                    roster
                        .get(Handle::from_parts(identity.arena_index(), 13))
                        .is_none()
                );
                assert!(roster.get(Handle::from_parts(2, 12)).is_none());
                assert_eq!(
                    match &roster {
                        SlotRoster::State(slots) => slots.len(),
                        SlotRoster::All(_) | SlotRoster::UnavailableState => unreachable!(),
                    },
                    1
                );
                let empty = SlotRoster::State(Vec::new());
                assert!(empty.get(identity).is_none());
            }
        }
    }
    pub use prepared_slots::PreparedDescriptorLoadSlots;

    pub fn completed_descriptor_load_slot(
        program: &dyn crate::CheckingInput,
        facts: &CheckFacts,
        loan: &checked_trees::BorrowLoanFact,
    ) -> Result<crate::CanonicalPlace, &'static str> {
        if !facts
            .borrow
            .incoming_references
            .is_valid(loan.incoming_reference)
        {
            return Err("the descriptor has no incoming load identity");
        }
        let incoming = facts
            .borrow
            .incoming_references
            .get(loan.incoming_reference);
        let IncomingReferenceOrigin::DescriptorLoad { expression } = incoming.origin else {
            return Err("the descriptor does not originate at a completed load");
        };
        if loan.storage_owner != PlaceRoot::Expression(expression)
            || loan.formation_expression != expression
        {
            return Err("the loan does not own the completed load's descriptor home");
        }
        let state = facts
            .flow
            .control
            .unique_state(incoming.machine_symbol, incoming.state_symbol)
            .ok_or("the descriptor load has no unique completed state")?;
        let loaded = completed_descriptor_load(program, facts, state, loan.incoming_reference)?;
        let capture = facts.flow.control.operands.get(loaded.capture);
        if capture.statement_index != loan.statement_index {
            return Err("the descriptor loan differs from its completed read statement");
        }
        unchanged_input_slot(loaded.sources)
    }

    fn unchanged_input_slot(
        source: crate::completed_storage::StoredOperand,
    ) -> Result<crate::CanonicalPlace, &'static str> {
        let mut pending = vec![source];
        let mut selected = None;
        while let Some(source) = pending.pop() {
            match source {
                crate::completed_storage::StoredOperand::Incoming(slot) => {
                    if !slot.root.is_valid()
                        || !slot
                            .segments
                            .iter()
                            .all(crate::storage_path_relations::place_segment_has_static_identity)
                    {
                        return Err("the input slot has an unresolved captured selector");
                    }
                    if selected.as_ref().is_some_and(|previous| *previous != slot) {
                        return Err("the completed load can read different input slots");
                    }
                    selected = Some(slot);
                }
                crate::completed_storage::StoredOperand::Alternatives(alternatives)
                    if !alternatives.is_empty() =>
                {
                    pending.extend(alternatives);
                }
                _ => return Err("the completed load has no unchanged input-slot source"),
            }
        }
        selected.ok_or("the completed load has no source slot")
    }

    pub struct CompletedDescriptorLoad {
        pub capture: Handle<FlowOperandFact>,
        pub sources: crate::completed_storage::StoredOperand,
    }

    fn activation_matches_load(
        facts: checked_trees::CheckFactsRef<'_>,
        identity: Handle<IncomingReferenceIdentity>,
        expression: typed_trees::expression::ExpressionHandle,
        activation: &checked_trees::FlowBorrowActivationFact,
    ) -> bool {
        if !facts.borrow.loans.is_valid(activation.loan) {
            return false;
        }
        let loan = facts.borrow.loans.get(activation.loan);
        loan.incoming_reference == identity
            && loan.storage_owner == PlaceRoot::Expression(expression)
            && loan.formation_expression == expression
    }

    pub fn completed_descriptor_load(
        program: &dyn crate::CheckingInput,
        facts: &CheckFacts,
        state: &FlowStateFact,
        identity: Handle<IncomingReferenceIdentity>,
    ) -> Result<CompletedDescriptorLoad, &'static str> {
        if !facts.borrow.incoming_references.is_valid(identity) {
            return Err("the loaded pointer has no checked incoming identity");
        }
        let incoming = facts.borrow.incoming_references.get(identity);
        let IncomingReferenceOrigin::DescriptorLoad { expression } = incoming.origin else {
            return Err("the incoming pointer has no completed descriptor-load source");
        };
        if incoming.machine_symbol != state.machine_symbol
            || incoming.state_symbol != state.state_symbol
        {
            return Err("the descriptor load belongs to another invocation");
        }
        let mut activations = facts
            .flow
            .borrow_lifetimes
            .activations
            .span_or_empty(state.borrow_activations)
            .iter()
            .filter(|activation| {
                activation_matches_load(facts.view(), identity, expression, activation)
            });
        let activation = activations
            .next()
            .ok_or("the incoming pointer has no actual load activation")?;
        if activations.next().is_some() {
            return Err("the incoming pointer has no unique completed load");
        }
        complete_descriptor_load(program, facts.view(), state, expression, activation)
    }

    fn complete_descriptor_load(
        program: &dyn crate::CheckingInput,
        facts: checked_trees::CheckFactsRef<'_>,
        state: &FlowStateFact,
        expression: typed_trees::expression::ExpressionHandle,
        activation: &checked_trees::FlowBorrowActivationFact,
    ) -> Result<CompletedDescriptorLoad, &'static str> {
        let control = &facts.flow.control;
        if !control.operands.is_valid(activation.formation_value) {
            return Err("the incoming pointer has no unique completed load");
        }
        let capture = control.operands.get(activation.formation_value);
        let descriptor_loan = capture.descriptor_permission.loaded_loan();
        let descriptor_read = capture.descriptor_permission.loaded_read();
        if descriptor_loan != activation.loan
            || capture.expression != expression
            || capture.state_symbol != state.state_symbol
            || !control.value_observations.is_valid(descriptor_read)
            || !control.operands.is_valid(capture.backing)
        {
            return Err("the descriptor activation lost its completed source association");
        }
        let read = control.value_observations.get(descriptor_read);
        if read.expression != expression
            || read.receiver != capture.backing
            || read.places.is_empty()
        {
            return Err("the descriptor activation has no authenticated source read");
        }
        let receiver = control.operands.get(capture.backing);
        let source = crate::canonical_place_from_expression_in_state(
            program,
            state.state_symbol,
            capture.statement_index,
            expression,
        )
        .ok_or("the completed descriptor read has no exact source slot")?;
        let parent = crate::canonical_place_from_expression_in_state(
            program,
            state.state_symbol,
            receiver.statement_index,
            receiver.expression,
        )
        .ok_or("the completed descriptor read has no exact receiver slot")?;
        if source.root != parent.root || !source.segments.starts_with(&parent.segments) {
            return Err("the completed descriptor read does not project its actual receiver");
        }
        let sources = crate::completed_storage::reference_sources_at_path(
            program,
            control,
            capture.backing,
            &source.segments[parent.segments.len()..],
            capture.storage_snapshot,
        );
        Ok(CompletedDescriptorLoad {
            capture: activation.formation_value,
            sources,
        })
    }
}
pub use completed_loads::{
    CompletedDescriptorLoad, PreparedDescriptorLoadSlots, completed_descriptor_load,
    completed_descriptor_load_slot,
};
#[cfg(test)]
pub(crate) mod declared_frontier_tests {
    use super::{DeclaredCarrierReference, declared_carrier_references};
    use checked_trees::{
        CheckedReferenceOriginProjection as Projection, CheckedStructuralAccess,
        CheckedUnitStructuralPathSegment as Segment,
    };
    use language_core::ReferenceAccess;
    use language_semantics::{DataSupplyMode, Multiplicity};
    use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
    use typed_trees::{
        TypedTrees,
        data::{DataDefinition, DataField, DataMember, DataProperties, TypeParameter},
        name::Identifier,
        type_identity::TypeIdentityId,
        types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode},
    };

    fn array(
        program: &mut TypedTrees,
        element_type: TypeReferenceHandle,
        length: usize,
    ) -> TypeReferenceHandle {
        program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type,
                length: FixedArrayLength::Literal(length),
            })
    }

    pub(crate) type Row = (
        Vec<Projection>,
        TypeIdentityId,
        CheckedStructuralAccess,
        bool,
    );
    fn rows(frontier: Option<Vec<DeclaredCarrierReference>>) -> Option<Vec<Row>> {
        frontier.map(|leaves| {
            leaves
                .into_iter()
                .map(|leaf| {
                    (
                        leaf.path,
                        leaf.type_identity,
                        leaf.access,
                        leaf.crosses_reference,
                    )
                })
                .collect()
        })
    }

    fn repeated_elements(
        program: &TypedTrees,
        element: TypeReferenceHandle,
        length: usize,
    ) -> Option<Vec<DeclaredCarrierReference>> {
        let mut output = Vec::new();
        for index in 0..length {
            for mut leaf in declared_carrier_references(program, element)? {
                leaf.path.insert(
                    0,
                    Projection::Exact(Segment::FixedIndex(u64::try_from(index).ok()?)),
                );
                output.push(leaf);
            }
        }
        Some(output)
    }

    #[test]
    fn fixed_array_frontier_matches_repeated_element_walks() {
        let mut builder = SymbolTableBuilder::default();
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("frontier_test"));
        let declarations = builder
            .insert_children(
                root,
                [
                    (SymbolKind::Data, SymbolNameRef::Static("EmptyLinear")),
                    (SymbolKind::Data, SymbolNameRef::Static("Opaque")),
                    (SymbolKind::Data, SymbolNameRef::Static("Envelope")),
                ],
            )
            .handles()
            .collect::<Vec<_>>();
        let parameter = builder
            .insert_children(
                declarations[2],
                [(SymbolKind::TypeParameter, SymbolNameRef::Static("T"))],
            )
            .start();
        let mut program = TypedTrees {
            symbols: builder.finish(),
            ..TypedTrees::default()
        };
        let unit = program.type_reference_table.insert(TypeReferenceNode::Unit);
        let slice = program
            .type_reference_table
            .insert(TypeReferenceNode::Slice { element_type: unit });
        let mut elements = Vec::new();
        for access in [
            ReferenceAccess::Shared,
            ReferenceAccess::Mutable,
            ReferenceAccess::WriteOnly,
        ] {
            for referee in [unit, slice] {
                elements.push(
                    program
                        .type_reference_table
                        .insert(TypeReferenceNode::Reference {
                            referee,
                            access,
                            lifetime: Some(Identifier::from("loan")),
                        }),
                );
            }
        }
        for (symbol, name, supply_mode) in [
            (declarations[0], "EmptyLinear", DataSupplyMode::CheckedShape),
            (declarations[1], "Opaque", DataSupplyMode::BoundaryOpaque),
        ] {
            program.push_data_definition(DataDefinition {
                symbol,
                name: Identifier::from(name),
                supply_mode,
                properties: DataProperties {
                    multiplicity: Multiplicity::Linear,
                    ..DataProperties::default()
                },
                ..DataDefinition::default()
            });
            elements.push(
                program
                    .type_reference_table
                    .insert(TypeReferenceNode::Named {
                        symbol,
                        name: Identifier::from(name),
                    }),
            );
        }
        let parameter_type = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: parameter,
                name: Identifier::from("T"),
            });
        let nested = array(&mut program, parameter_type, 3);
        let parameters = program.data_type_parameters.insert_many([TypeParameter {
            symbol: parameter,
            name: Identifier::from("T"),
            ..TypeParameter::default()
        }]);
        let members = program
            .data_members
            .insert_many([DataMember::Field(DataField {
                identity: Some(17),
                name: Identifier::from("items"),
                type_reference: nested,
                ..DataField::default()
            })]);
        program.push_data_definition(DataDefinition {
            symbol: declarations[2],
            name: Identifier::from("Envelope"),
            type_parameters: parameters,
            members,
            ..DataDefinition::default()
        });
        let arguments = program
            .type_reference_table
            .insert_type_reference_handles([elements[0]]);
        let applied = program
            .type_reference_table
            .insert(TypeReferenceNode::Generic {
                base_symbol: declarations[2],
                base_name: Identifier::from("Envelope"),
                lifetime_arguments: Vec::new(),
                arguments,
            });
        elements.push(applied);
        let generic = declared_carrier_references(&program, applied).unwrap();
        assert_eq!(generic.len(), 3);
        assert_eq!(
            generic[2].path,
            [
                Projection::Exact(Segment::Field(
                    typed_trees::data::DataMemberIdentity::of_member(
                        Some(17),
                        Identifier::from("items"),
                    )
                )),
                Projection::Exact(Segment::FixedIndex(2)),
            ]
        );
        let direct = declared_carrier_references(&program, elements[0]).unwrap();
        assert_eq!(generic[2].type_identity, direct[0].type_identity);
        for element in elements {
            for length in 0..=4 {
                let original = rows(repeated_elements(&program, element, length));
                let reference = array(&mut program, element, length);
                assert_eq!(
                    rows(declared_carrier_references(&program, reference)),
                    original
                );
            }
        }
        let opaque = program
            .type_reference_table
            .find_named_type_reference(declarations[1])
            .unwrap();
        assert!(declared_carrier_references(&program, opaque).is_none());
        let zero = array(&mut program, opaque, 0);
        assert_eq!(
            rows(declared_carrier_references(&program, zero)),
            Some(Vec::new())
        );
        let one = array(&mut program, opaque, 1);
        assert!(declared_carrier_references(&program, one).is_none());
        let excessive = array(&mut program, applied, super::MAX_REFERENCE_ORIGIN_LEAVES);
        assert!(declared_carrier_references(&program, excessive).is_none());
    }
}
pub(crate) mod declared_returns {
    use super::{is_mutable_structural_parameter, loan_owner_path, record_carrier_sources};
    use crate::borrow_contracts::{
        ViewReturnInputSource, ViewReturnSource, resolve_signature_view_return_source,
    };
    use checked_trees::{
        CheckedReferenceOriginProjection, CheckedReferenceOriginSourcePlan,
        CheckedReturnedReferencePlan, CheckedUnitStructuralArgumentSourcePlan,
        CheckedUnitStructuralPathSegment,
    };
    use semantic_vocabulary::ReferenceOriginExtent;
    use typed_trees::signature::StateParameter;
    use typed_trees::types::TypeReferenceHandle;

    pub fn installed_context_sources(
        program: &dyn crate::CheckingInput,
        target: symbols::SymbolHandle,
    ) -> Result<Option<Vec<CheckedReturnedReferencePlan>>, &'static str> {
        if crate::compiler_operations::static_interrupt_context(program, target)?.is_none() {
            return Ok(None);
        }
        let result = crate::call_target_return_type(program, target)
            .ok_or("installed context has no retained return type")?;
        invocation_returned_sources(program, result).map(Some)
    }

    pub fn returned_sources(
        program: &dyn crate::CheckingInput,
        parameters: &[StateParameter],
        return_type: TypeReferenceHandle,
    ) -> Result<Vec<CheckedReturnedReferencePlan>, &'static str> {
        if !crate::borrow_contracts::returns_borrow(program, return_type) {
            return Ok(Vec::new());
        }
        let relation = resolve_signature_view_return_source(program, parameters, return_type);
        let result_leaves = record_carrier_sources(
            program,
            return_type,
            CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index: 0 },
        )
        .ok_or("public result has no complete reference carrier roster")?;
        if result_leaves.is_empty() {
            return Ok(Vec::new());
        }
        let fields = match relation {
            ViewReturnSource::Fields { fields } => fields,
            ViewReturnSource::Parameter { parameter_index } => {
                let [leaf] = result_leaves.as_slice() else {
                    return Err("public single-input relation has multiple result leaves");
                };
                vec![crate::borrow_contracts::ViewReturnFieldSource {
                    lifetime_only: false,
                    owner_path: loan_owner_path(program, return_type, &leaf.path)
                        .ok_or("public result leaf has no loan owner path")?,
                    source_path: Vec::new(),
                    source: crate::borrow_contracts::ViewReturnInputSource::Referent,
                    source_type: parameters
                        .get(parameter_index)
                        .ok_or("public result lifetime names no parameter")?
                        .type_reference,
                    source_reference_hops: 0,
                    parameter_index,
                    kind: leaf
                        .source
                        .access
                        .borrow_access_kind()
                        .ok_or("public result leaf has no borrowed access")?,
                }]
            }
            ViewReturnSource::NotApplicable if result_leaves.is_empty() => return Ok(Vec::new()),
            ViewReturnSource::NotApplicable => {
                return invocation_sources(program, return_type, result_leaves);
            }
            ViewReturnSource::Ambiguous(_) => {
                return Err("public reference result lifetime relation is ambiguous");
            }
        };
        let mut output = Vec::new();
        for field in fields.into_iter().filter(|field| !field.lifetime_only) {
            let parameter = parameters
                .get(field.parameter_index)
                .ok_or("public result field names no parameter")?;
            if !is_mutable_structural_parameter(program, parameter) {
                return Err("public result region names a nonstructural parameter");
            }
            let parameter_index = u32::try_from(
                parameters[..field.parameter_index]
                    .iter()
                    .filter(|parameter| is_mutable_structural_parameter(program, parameter))
                    .count(),
            )
            .map_err(|_| "public result structural parameter ordinal overflows")?;
            let input_carrier = match field.source {
                ViewReturnInputSource::Referent => parameter.type_reference,
                ViewReturnInputSource::Stored => {
                    super::input_carrier_type(program, parameter.type_reference)
                        .ok_or("public input cannot expose stored descriptors")?
                }
            };
            let input = match field.source {
                ViewReturnInputSource::Referent => {
                    let input_leaves =
                        super::declared_carrier_references(program, input_carrier)
                            .ok_or("public input has no complete reference carrier roster")?;
                    input_leaves.into_iter().find(|leaf| {
                        super::origin_owner_path(program, input_carrier, leaf.path.iter().cloned())
                            .as_ref()
                            == Some(&field.source_path)
                    })
                }
                ViewReturnInputSource::Stored => super::declared_input_reference(
                    program,
                    input_carrier,
                    &field.source_path,
                    field.source_reference_hops,
                ),
            }
            .ok_or("public input lifetime path has no matching carrier leaf")?;
            let result = result_leaves
                .iter()
                .find(|leaf| {
                    loan_owner_path(program, return_type, &leaf.path).as_ref()
                        == Some(&field.owner_path)
                        && leaf.source.access.borrow_access_kind().as_ref() == Some(&field.kind)
                })
                .ok_or("public output lifetime path has no matching carrier leaf")?;
            if !super::reference_access_can_supply(input.access, result.source.access) {
                return Err("public input access does not supply returned access");
            }
            let mut path = Vec::new();
            let entry_reference_hops = 0;
            if field.source == ViewReturnInputSource::Stored {
                path.extend(input.path.iter().cloned());
                if input.crosses_reference {
                    path.push(CheckedReferenceOriginProjection::Exact(
                        CheckedUnitStructuralPathSegment::Referent,
                    ));
                }
            }
            let origin = CheckedReturnedReferencePlan {
                path: result.path.clone(),
                source: CheckedReferenceOriginSourcePlan {
                    root: checked_trees::CheckedReferenceOriginRoot::Parameter {
                        index: parameter_index,
                    },
                    entry_reference_hops,
                    path,
                    extent: ReferenceOriginExtent::WithinRegion,
                    type_identity: input.type_identity,
                    access: result.source.access,
                },
            };
            if !output.contains(&origin) {
                output.push(origin);
            }
        }
        result_leaves
            .iter()
            .all(|leaf| output.iter().any(|origin| origin.path == leaf.path))
            .then_some(output)
            .ok_or("public lifetime relation omits a returned reference leaf")
    }

    pub(crate) fn invocation_returned_sources(
        program: &dyn crate::CheckingInput,
        return_type: TypeReferenceHandle,
    ) -> Result<Vec<CheckedReturnedReferencePlan>, &'static str> {
        let leaves = record_carrier_sources(
            program,
            return_type,
            CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index: 0 },
        )
        .ok_or("invocation result has no complete reference carrier roster")?;
        invocation_sources(program, return_type, leaves)
    }

    fn invocation_sources(
        program: &dyn crate::CheckingInput,
        return_type: TypeReferenceHandle,
        result_leaves: Vec<checked_trees::CheckedReferenceResultSourcePlan>,
    ) -> Result<Vec<CheckedReturnedReferencePlan>, &'static str> {
        result_leaves
            .into_iter()
            .map(|leaf| {
                Ok(CheckedReturnedReferencePlan {
                    path: leaf.path.clone(),
                    source: CheckedReferenceOriginSourcePlan {
                        root: checked_trees::CheckedReferenceOriginRoot::Invocation {
                            carrier_type: program.normalized_type_identity(return_type),
                        },
                        entry_reference_hops: 0,
                        path: leaf
                            .source
                            .path
                            .iter()
                            .cloned()
                            .map(CheckedReferenceOriginProjection::Exact)
                            .collect(),
                        extent: ReferenceOriginExtent::Projected,
                        type_identity: leaf.source.type_identity,
                        access: leaf.source.access,
                    },
                })
            })
            .collect()
    }
}
pub(crate) mod source_paths {
    use checked_trees::CheckedMemberIdentity;
    use checked_trees::{CheckedReferenceOriginProjection, CheckedUnitStructuralPathSegment};
    use facts::PlaceSegment;
    use semantic_vocabulary::ReferenceOriginExtent;
    use typed_trees::data::DataMember;
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

    pub fn reference_origin_path(
        program: &dyn crate::CheckingInput,
        mut selected: TypeReferenceHandle,
        mut segments: &[PlaceSegment],
    ) -> Option<(
        Vec<CheckedReferenceOriginProjection>,
        TypeReferenceHandle,
        ReferenceOriginExtent,
    )> {
        let mut output = Vec::new();
        let is_element_selection = |expression: ExpressionHandle| {
            !expression.is_valid()
                || program.expression_table.expression_is_valid(expression)
                    && program.expression_table.range_node(expression).is_none()
        };
        loop {
            let selection = segments.iter().position(|segment| {
                matches!(
                    segment,
                    PlaceSegment::Index { .. }
                        | PlaceSegment::FixedIndex { .. }
                        | PlaceSegment::FixedRange { .. }
                        | PlaceSegment::OpaqueRange
                )
            });
            let prefix_length = selection.unwrap_or(segments.len());
            let (prefix, endpoint) =
                structural_path(program, selected, &segments[..prefix_length])?;
            output.extend(
                prefix
                    .into_iter()
                    .map(CheckedReferenceOriginProjection::Exact),
            );
            selected = endpoint;
            let Some(position) = selection else {
                return Some((output, selected, ReferenceOriginExtent::Projected));
            };
            selected = crate::peel_declared_constraints(program, selected);
            let trailing_window = position + 1 == segments.len()
                && match segments[position] {
                    PlaceSegment::FixedRange { .. } | PlaceSegment::OpaqueRange => true,
                    PlaceSegment::Index { expression } => !is_element_selection(expression),
                    _ => false,
                };
            if trailing_window {
                let collection = crate::reference_shell(program, selected)
                    .map_or(selected, |shell| {
                        crate::peel_declared_constraints(program, shell.referee)
                    });
                return matches!(
                    program.type_reference_table.type_reference(collection),
                    TypeReferenceNode::FixedArray { .. } | TypeReferenceNode::Slice { .. }
                )
                .then_some((output, selected, ReferenceOriginExtent::WithinRegion));
            }
            if let Some(shell) = crate::reference_shell(program, selected) {
                output.push(CheckedReferenceOriginProjection::Exact(
                    CheckedUnitStructuralPathSegment::Referent,
                ));
                selected = crate::peel_declared_constraints(program, shell.referee);
            }
            let (TypeReferenceNode::FixedArray { element_type, .. }
            | TypeReferenceNode::Slice { element_type }) =
                program.type_reference_table.type_reference(selected)
            else {
                return None;
            };
            selected = *element_type;
            let (projection, consumed) = match segments[position] {
                PlaceSegment::FixedIndex { index } => (
                    CheckedReferenceOriginProjection::Exact(
                        CheckedUnitStructuralPathSegment::FixedIndex(u64::try_from(index).ok()?),
                    ),
                    1,
                ),
                PlaceSegment::Index { expression } if is_element_selection(expression) => {
                    (CheckedReferenceOriginProjection::Element, 1)
                }
                PlaceSegment::FixedRange { start, end } => {
                    let projection = match segments.get(position.checked_add(1)?)? {
                        PlaceSegment::Index { expression }
                            if start < end && is_element_selection(*expression) =>
                        {
                            CheckedReferenceOriginProjection::Element
                        }
                        PlaceSegment::FixedIndex { index }
                            if *index < end.checked_sub(start)? =>
                        {
                            CheckedReferenceOriginProjection::Exact(
                                CheckedUnitStructuralPathSegment::FixedIndex(
                                    u64::try_from(start.checked_add(*index)?).ok()?,
                                ),
                            )
                        }
                        _ => return None,
                    };
                    (projection, 2)
                }
                PlaceSegment::OpaqueRange => {
                    match segments.get(position.checked_add(1)?)? {
                        PlaceSegment::Index { expression } if is_element_selection(*expression) => {
                        }
                        PlaceSegment::FixedIndex { .. } => {}
                        _ => return None,
                    }
                    (CheckedReferenceOriginProjection::Element, 2)
                }
                _ => return None,
            };
            output.push(projection);
            segments = &segments[position + consumed..];
        }
    }

    pub fn parameter_reference_origin_path(
        program: &dyn crate::CheckingInput,
        parameter: TypeReferenceHandle,
        segments: &[PlaceSegment],
    ) -> Option<(
        u32,
        Vec<CheckedReferenceOriginProjection>,
        TypeReferenceHandle,
        ReferenceOriginExtent,
    )> {
        let (path, endpoint, extent) = reference_origin_path(program, parameter, segments)?;
        let entry_reference_hops = u32::from(
            super::stored_loan_parts(program, parameter).is_some()
                && matches!(
                    path.first(),
                    Some(CheckedReferenceOriginProjection::Exact(
                        CheckedUnitStructuralPathSegment::Referent
                    ))
                ),
        );
        Some((entry_reference_hops, path, endpoint, extent))
    }

    pub(crate) fn structural_path(
        program: &dyn crate::CheckingInput,
        mut selected: TypeReferenceHandle,
        segments: &[PlaceSegment],
    ) -> Option<(Vec<CheckedUnitStructuralPathSegment>, TypeReferenceHandle)> {
        let mut path = Vec::new();
        let mut payload = None;
        for segment in segments {
            selected = crate::peel_declared_constraints(program, selected);
            while let Some(shell) = crate::reference_shell(program, selected) {
                selected = shell.referee;
                path.push(CheckedUnitStructuralPathSegment::Referent);
            }
            match segment {
                PlaceSegment::Case { variant } => {
                    let data = program.closed_data_definition(selected)?;
                    let case =
                        program
                            .data_members(data)
                            .iter()
                            .find_map(|member| match member {
                                DataMember::Variant(case) if case.symbol == *variant => Some(case),
                                _ => None,
                            })?;
                    path.push(CheckedUnitStructuralPathSegment::Case(
                        CheckedMemberIdentity::of_member(case.identity, case.name.clone()),
                    ));
                    payload = Some(case);
                }
                PlaceSegment::Field { symbol } => {
                    let field = if let Some(case) = payload.take() {
                        program
                            .data_payload_fields(case)
                            .iter()
                            .find(|field| field.symbol == *symbol)?
                    } else {
                        let data = program.closed_data_definition(selected)?;
                        program
                            .data_members(data)
                            .iter()
                            .find_map(|member| match member {
                                DataMember::Field(field) if field.symbol == *symbol => Some(field),
                                _ => None,
                            })?
                    };
                    path.push(CheckedUnitStructuralPathSegment::Field(
                        CheckedMemberIdentity::of_member(field.identity, field.name.clone()),
                    ));
                    selected = field.type_reference;
                }
                PlaceSegment::FixedIndex { index } => {
                    let TypeReferenceNode::FixedArray { element_type, .. } =
                        program.type_reference_table.type_reference(selected)
                    else {
                        return None;
                    };
                    selected = *element_type;
                    path.push(CheckedUnitStructuralPathSegment::FixedIndex(
                        u64::try_from(*index).ok()?,
                    ));
                }
                _ => return None,
            }
        }
        Some((path, selected))
    }
}
pub use captured_returns::call_sources as call_return_sources;
pub use captured_returns::returned_sources as captured_return_sources;
pub use captured_returns::{CapturedInvocationBorrowSource, invocation_borrow_sources};
pub use declared_returns::{
    installed_context_sources, returned_sources as declared_return_sources,
};
pub use source_paths::{parameter_reference_origin_path, reference_origin_path};

const MAX_REFERENCE_ORIGIN_LEAVES: usize = 4096;
const MAX_REFERENCE_ORIGIN_VISITS: usize = MAX_REFERENCE_ORIGIN_LEAVES * 2;
const MAX_REFERENCE_ORIGIN_DEPTH: usize = 128;

/*
    Whether a formal owns a slot in the caller's structural argument roster.
    Published reference origins count only these slots, and lowering indexes
    the same dense roster, so the rule has to be this one predicate: an erased
    formal has no runtime slot, and counting it shifts every later origin.
*/
pub fn is_mutable_structural_parameter(
    program: &dyn CheckingInput,
    parameter: &typed_trees::signature::StateParameter,
) -> bool {
    !parameter.is_const
        && (!parameter.relevance.is_erased() || parameter.is_mutable)
        && program
            .primitive_type_reference(parameter.type_reference)
            .is_none()
}

/*
    The authored position and formal named by a published origin's structural
    index. `None` when the callee has no such slot; callers must refuse then,
    because an origin that names no input proves nothing about the result.
*/
pub fn origin_formal<'parameters>(
    program: &dyn CheckingInput,
    parameters: &'parameters [typed_trees::signature::StateParameter],
    index: u32,
) -> Option<(usize, &'parameters typed_trees::signature::StateParameter)> {
    parameters
        .iter()
        .enumerate()
        .filter(|(_, parameter)| is_mutable_structural_parameter(program, parameter))
        .nth(usize::try_from(index).ok()?)
}

/*
    The inverse of `origin_formal`: the structural index a published origin
    carries for the formal `symbol`. The side that publishes an origin and the
    sides that read it count the same roster here, so an index names the same
    input on both. Counting an erased formal on one side only made the origin
    of the second of two inputs behind an erased one name no input at all.
*/
pub fn origin_index<'parameters>(
    program: &dyn CheckingInput,
    parameters: &'parameters [typed_trees::signature::StateParameter],
    symbol: SymbolHandle,
) -> Option<(u32, &'parameters typed_trees::signature::StateParameter)> {
    let (index, parameter) = parameters
        .iter()
        .filter(|parameter| is_mutable_structural_parameter(program, parameter))
        .enumerate()
        .find(|(_, parameter)| parameter.symbol == symbol)?;
    Some((u32::try_from(index).ok()?, parameter))
}

fn reference_origin_expansion_is_bounded(
    program: &dyn CheckingInput,
    reference: TypeReferenceHandle,
) -> bool {
    reference_origin_expansion_bound(program, reference, false)
}

/*
    `opaque_type_parameters` admits an unbound type parameter as a leaf that
    expands no further. Only the opaque-aware inventory asks for that; every
    other judgment still treats such a carrier as unbounded.
*/
fn reference_origin_expansion_bound(
    program: &dyn CheckingInput,
    reference: TypeReferenceHandle,
    opaque_type_parameters: bool,
) -> bool {
    #[allow(clippy::too_many_arguments)]
    fn visit(
        program: &dyn CheckingInput,
        reference: TypeReferenceHandle,
        depth: usize,
        visits: &mut usize,
        leaves: &mut usize,
        scopes: &mut DeclarationScopes,
        scope: usize,
        opaque_type_parameters: bool,
    ) -> Option<()> {
        *visits = visits.checked_add(1)?;
        if *visits > MAX_REFERENCE_ORIGIN_VISITS || depth > MAX_REFERENCE_ORIGIN_DEPTH {
            return None;
        }
        let (reference, scope) = scopes.head(program, reference, scope);
        if program.primitive_type_reference(reference).is_some() {
            return Some(());
        }
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Reference { .. } => {
                *leaves = leaves.checked_add(1)?;
                return (*leaves <= MAX_REFERENCE_ORIGIN_LEAVES).then_some(());
            }
            TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. } => {}
            TypeReferenceNode::Constrained { base_type, .. } => {
                return visit(
                    program,
                    *base_type,
                    depth + 1,
                    visits,
                    leaves,
                    scopes,
                    scope,
                    opaque_type_parameters,
                );
            }
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => {
                let before = *leaves;
                visit(
                    program,
                    *element_type,
                    depth + 1,
                    visits,
                    leaves,
                    scopes,
                    scope,
                    opaque_type_parameters,
                )?;
                let element_leaves = leaves.checked_sub(before)?;
                if element_leaves == 0 {
                    return Some(());
                }
                let typed_trees::types::FixedArrayLength::Literal(length) = length else {
                    return None;
                };
                *leaves = before.checked_add(element_leaves.checked_mul(*length)?)?;
                return (*leaves <= MAX_REFERENCE_ORIGIN_LEAVES).then_some(());
            }
            TypeReferenceNode::Slice { element_type } => {
                return visit(
                    program,
                    *element_type,
                    depth + 1,
                    visits,
                    leaves,
                    scopes,
                    scope,
                    opaque_type_parameters,
                );
            }
            _ => return Some(()),
        }
        if opaque_type_parameters
            && let TypeReferenceNode::Named { symbol, .. } =
                program.type_reference_table.type_reference(reference)
            && program.symbols.get(*symbol).kind == symbols::SymbolKind::TypeParameter
        {
            return Some(());
        }
        let (data, scope) = scopes.definition(program, reference, scope)?;
        for field in program.data_member_fields(program.data_members(data)) {
            if !field.relevance.is_erased() {
                visit(
                    program,
                    field.type_reference,
                    depth + 1,
                    visits,
                    leaves,
                    scopes,
                    scope,
                    opaque_type_parameters,
                )?;
            }
        }
        Some(())
    }
    visit(
        program,
        reference,
        0,
        &mut 0,
        &mut 0,
        &mut DeclarationScopes::default(),
        0,
        opaque_type_parameters,
    )
    .is_some()
}

pub fn is_reference_record(program: &dyn CheckingInput, reference: TypeReferenceHandle) -> bool {
    if !reference_origin_expansion_is_bounded(program, reference) {
        return false;
    }
    fn visit(
        program: &dyn CheckingInput,
        reference: TypeReferenceHandle,
        seen: &mut Vec<TypeReferenceHandle>,
        scopes: &mut DeclarationScopes,
        scope: usize,
    ) -> Option<bool> {
        let (reference, scope) = scopes.head(program, reference, scope);
        if crate::reference_shell(program, reference).is_some() {
            return Some(true);
        }
        if crate::has_contents(program, reference, &[], ContentsRequirement::NumericOwned) {
            return Some(false);
        }
        if let TypeReferenceNode::FixedArray { element_type, .. } =
            program.type_reference_table.type_reference(reference)
        {
            return visit(program, *element_type, seen, scopes, scope);
        }
        if let TypeReferenceNode::Constrained { base_type, .. } =
            program.type_reference_table.type_reference(reference)
        {
            return visit(program, *base_type, seen, scopes, scope);
        }
        let (data, scope) = scopes.definition(program, reference, scope)?;
        if seen.contains(&reference) {
            return None;
        }
        if data.supply_mode != language_semantics::DataSupplyMode::CheckedShape {
            return None;
        }
        seen.push(reference);
        let mut contains_reference = false;
        for field in program.data_member_fields(program.data_members(data)) {
            if field.relevance.is_erased() {
                continue;
            }
            contains_reference |= visit(program, field.type_reference, seen, scopes, scope)?;
        }
        seen.pop();
        Some(contains_reference)
    }
    if crate::reference_shell(program, reference).is_some() {
        return shared_borrowed_parts(program, reference).is_some();
    }
    visit(
        program,
        reference,
        &mut Vec::new(),
        &mut DeclarationScopes::default(),
        0,
    ) == Some(true)
}

pub fn initializer_source(
    program: &dyn CheckingInput,
    state: &State,
    expression: typed_trees::expression::ExpressionHandle,
    reference: TypeReferenceHandle,
) -> Option<checked_trees::CheckedUnitStructuralArgumentPlan> {
    let (referent, access) = stored_loan_parts(program, reference)?;
    if !matches!(
        expression_table(program).expression(expression),
        ExpressionNode::Name(_)
    ) {
        return borrowed_initializer_source(program, state, expression, reference);
    }
    let (path, name) = expression_table(program).single_name_member(expression)?;
    if !path.symbol.is_valid()
        || path.symbol != path.head_symbol
        || program.symbols.get(path.symbol).parent != state.symbol
    {
        return None;
    }

    let found = structural_parameter_position(program, state, path.symbol);
    if let Some((position, parameter)) = found {
        if parameter.name != *name
            || !reference_parameter_can_supply(program, parameter.type_reference, reference)
        {
            return None;
        }
        return Some(checked_trees::CheckedUnitStructuralArgumentPlan {
            projection_evaluations: Default::default(),
            source: CheckedUnitStructuralArgumentSourcePlan::Parameter {
                parameter_index: u32::try_from(position).ok()?,
            },
            path: Vec::new(),
            type_identity: crate::stored_structural_type_identity(program, referent, &[], &[]),
            access,
        });
    }
    let (_, local) = program.state_local_declaration(state, path.symbol)?;
    if local.is_mutable
        || local.name != *name
        || !reference_parameter_can_supply(program, local.type_reference, reference)
    {
        return None;
    }
    borrowed_initializer_source(program, state, local.initial_value, reference)
}

fn structural_parameter_position<'program>(
    program: &'program dyn CheckingInput,
    state: &'program State,
    symbol: SymbolHandle,
) -> Option<(usize, &'program typed_trees::signature::StateParameter)> {
    program
        .state_parameters(state)
        .iter()
        .filter(|parameter| is_mutable_structural_parameter(program, parameter))
        .enumerate()
        .find(|(_, parameter)| parameter.symbol == symbol)
}

fn reference_parameter_can_supply(
    program: &dyn CheckingInput,
    source: TypeReferenceHandle,
    result: TypeReferenceHandle,
) -> bool {
    if program.normalized_type_identity(source) == program.normalized_type_identity(result) {
        return true;
    }
    let TypeReferenceNode::Reference {
        referee: source_referee,
        access: language_core::ReferenceAccess::Mutable,
        ..
    } = program.type_reference_table.type_reference(source)
    else {
        return false;
    };
    let TypeReferenceNode::Reference {
        referee: result_referee,
        access: language_core::ReferenceAccess::Shared | language_core::ReferenceAccess::WriteOnly,
        ..
    } = program.type_reference_table.type_reference(result)
    else {
        return false;
    };
    program.normalized_type_identity(*source_referee)
        == program.normalized_type_identity(*result_referee)
}

pub fn borrowed_initializer_source(
    program: &dyn CheckingInput,
    state: &State,
    expression: typed_trees::expression::ExpressionHandle,
    reference: TypeReferenceHandle,
) -> Option<checked_trees::CheckedUnitStructuralArgumentPlan> {
    let (referent, access) = stored_loan_parts(program, reference)?;
    let borrow = expression_table(program).borrow_expression(expression)?;
    let authored_access = CheckedStructuralAccess::from(borrow.access);
    if authored_access != access {
        return None;
    }
    let (projections, cursor) = crate::queries::expression_traversal::projection_climb(
        program,
        borrow.target,
        MAX_REFERENCE_ORIGIN_DEPTH,
    )?;
    let path = expression_table(program).name_path_node(cursor)?;
    let [name] = expression_table(program).name_path_members(path.members) else {
        return None;
    };
    if !path.symbol.is_valid() || path.symbol != path.head_symbol {
        return None;
    }
    let found = structural_parameter_position(program, state, path.symbol);
    let (source, mut owner_type) = match found {
        Some((position, parameter)) => {
            if parameter.name != *name {
                return None;
            }
            let owner_type = if let Some((referee, parameter_access)) =
                stored_loan_parts(program, parameter.type_reference)
            {
                if !reference_access_can_supply(parameter_access, access) {
                    return None;
                }
                referee
            } else {
                parameter.type_reference
            };
            (
                CheckedUnitStructuralArgumentSourcePlan::Parameter {
                    parameter_index: u32::try_from(position).ok()?,
                },
                owner_type,
            )
        }
        None => {
            let (_, local) = program.state_local_declaration(state, path.symbol)?;
            let exclusive = matches!(
                access,
                CheckedStructuralAccess::MutableBorrow | CheckedStructuralAccess::WriteOnlyBorrow
            );
            if local.name != *name
                || local.relevance.is_erased()
                || (exclusive && !local.is_mutable)
                || crate::reference_shell(program, local.type_reference).is_some()
            {
                return None;
            }
            let source = if local.is_mutable
                && program
                    .primitive_type_reference(local.type_reference)
                    .is_some()
            {
                CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal {
                    symbol: local.symbol,
                }
            } else {
                CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                    symbol: local.symbol,
                }
            };
            (source, local.type_reference)
        }
    };
    let mut segments = Vec::with_capacity(projections.len() + 1);
    for projection in projections.iter().rev() {
        match expression_table(program).expression(*projection) {
            ExpressionNode::Member(member) => {
                let data = nominal_member_owner_type(program, owner_type)?;
                let field = crate::value_custody::places::exact_data_member_field(
                    program,
                    data,
                    member.member_symbol,
                    member.member.as_str(),
                    None,
                )?;
                if field.relevance.is_erased() {
                    return None;
                }
                segments.push(checked_trees::CheckedUnitStructuralPathSegment::Field(
                    CheckedMemberIdentity::of_member(field.identity, field.name.clone()),
                ));
                owner_type = field.type_reference;
            }
            ExpressionNode::Indexed(indexed) => {
                let index = expression_table(program).integer_node(indexed.index)?;
                segments.push(checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(
                    u64::try_from(index.value_i64()?).ok()?,
                ));
                owner_type = loop {
                    match program.type_reference_table.type_reference(owner_type) {
                        TypeReferenceNode::Constrained { base_type, .. } => owner_type = *base_type,
                        TypeReferenceNode::FixedArray { element_type, .. } => {
                            break *element_type;
                        }
                        _ => return None,
                    }
                };
            }
            _ => return None,
        }
    }
    let leaf = if let Some(shell) = crate::reference_shell(program, owner_type) {
        if !reference_parameter_can_supply(program, owner_type, reference) {
            return None;
        }
        segments.push(checked_trees::CheckedUnitStructuralPathSegment::Referent);
        shell.referee
    } else {
        owner_type
    };
    if program.normalized_type_identity(leaf) != program.normalized_type_identity(referent) {
        return None;
    }
    Some(checked_trees::CheckedUnitStructuralArgumentPlan {
        projection_evaluations: Default::default(),
        source,
        path: segments,
        type_identity: crate::stored_structural_type_identity(program, referent, &[], &[]),
        access,
    })
}

fn nominal_member_owner_type(
    program: &dyn CheckingInput,
    mut reference: TypeReferenceHandle,
) -> Option<&typed_trees::data::DataDefinition> {
    loop {
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            TypeReferenceNode::Reference { referee, .. } => reference = *referee,
            TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. } => {
                return program.closed_data_definition(reference);
            }
            _ => return None,
        }
    }
}

pub(crate) struct AggregateProjection {
    root: typed_trees::expression::ExpressionHandle,
    path: Vec<checked_trees::CheckedUnitStructuralPathSegment>,
    selected_type: TypeReferenceHandle,
}

fn aggregate_projection(
    program: &dyn CheckingInput,
    state: &State,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<AggregateProjection> {
    use checked_trees::CheckedUnitStructuralPathSegment as Segment;
    let mut projections = Vec::new();
    let mut root = expression;
    loop {
        if projections.len() >= MAX_REFERENCE_ORIGIN_DEPTH || projections.contains(&root) {
            return None;
        }
        match expression_table(program).expression(root) {
            ExpressionNode::Member(member) => {
                projections.push(root);
                root = member.receiver;
            }
            ExpressionNode::Indexed(indexed) => {
                projections.push(root);
                root = indexed.collection;
            }
            _ => break,
        }
    }
    let root_type = match expression_table(program).expression(root) {
        ExpressionNode::Name(name) => crate::state_root_type(program, state, name.symbol)?,
        ExpressionNode::Call(call) => {
            let (_, destination) =
                crate::machine_calls::calls::machine_state_by_symbol(program, call.target_symbol)
                    .or_else(|| {
                    let machine = program.machine_by_symbol(call.target_symbol)?;
                    Some((machine, program.machine_states(machine).first()?))
                })?;
            destination.return_type
        }
        _ => return None,
    };
    let root_type = input_carrier_type(program, root_type)?;
    let mut selected = root_type;
    let mut path = Vec::with_capacity(projections.len());
    for projection in projections.into_iter().rev() {
        selected = crate::peel_declared_constraints(program, selected);
        match expression_table(program).expression(projection) {
            ExpressionNode::Member(member) => {
                if crate::reference_shell(program, selected).is_some() {
                    return None;
                }
                let data = nominal_member_owner_type(program, selected)?;
                let field = crate::value_custody::places::exact_data_member_field(
                    program,
                    data,
                    member.member_symbol,
                    member.member.as_str(),
                    member.case_variant.as_ref().map(|variant| variant.as_str()),
                )?;
                if field.relevance.is_erased() {
                    return None;
                }
                if let Some(variant) = facts::payload_variant_for_field(program, field.symbol) {
                    let case = program
                        .data_members(data)
                        .iter()
                        .filter_map(|member| member.as_variant())
                        .find(|case| case.symbol == variant)?;
                    path.push(Segment::Case(CheckedMemberIdentity::of_member(
                        case.identity,
                        case.name.clone(),
                    )));
                }
                path.push(Segment::Field(CheckedMemberIdentity::of_member(
                    field.identity,
                    field.name.clone(),
                )));
                selected = field.type_reference;
            }
            ExpressionNode::Indexed(indexed) => {
                let (machine, _) =
                    crate::machine_calls::calls::machine_state_by_symbol(program, state.symbol)?;
                let index_type =
                    crate::declarations::operators::landed_integer_literal_type_reference(
                        program,
                        indexed.index,
                    );
                if !typed_trees::operator::has_builtin_spelled_expression_meaning(
                    program,
                    machine.symbol,
                    projection,
                    language_core::OperatorSpelling::Index,
                    &[Some(selected), index_type],
                ) {
                    return None;
                }
                let index = expression_table(program).integer_node(indexed.index)?;
                let index = usize::try_from(index.value_i64()?).ok()?;
                let TypeReferenceNode::FixedArray {
                    element_type,
                    length: typed_trees::types::FixedArrayLength::Literal(length),
                } = program.type_reference_table.type_reference(selected)
                else {
                    return None;
                };
                if index >= *length {
                    return None;
                }
                path.push(Segment::FixedIndex(u64::try_from(index).ok()?));
                selected = *element_type;
            }
            _ => return None,
        }
    }
    Some(AggregateProjection {
        root,
        path,
        selected_type: selected,
    })
}

fn reference_access_can_supply(
    source: CheckedStructuralAccess,
    result: CheckedStructuralAccess,
) -> bool {
    source == result
        || matches!(
            (source, result),
            (
                CheckedStructuralAccess::MutableBorrow,
                CheckedStructuralAccess::SharedBorrow | CheckedStructuralAccess::WriteOnlyBorrow
            )
        )
}

fn whole_reference_sources(
    program: &dyn CheckingInput,
    state: &State,
    call_frames: Option<&crate::CallFrameResolver<'_>>,
) -> Option<Vec<checked_trees::CheckedReferenceResultSourcePlan>> {
    let (referent, access) = stored_loan_parts(program, state.return_type)?;
    let prepared_frames;
    let call_frames = match call_frames {
        Some(call_frames) => call_frames,
        None => {
            prepared_frames = crate::CallFrameResolver::new(program)?;
            &prepared_frames
        }
    };
    let origins = call_frames.reference_result_origins(state.symbol)?;
    if origins.is_empty() || origins.len() > MAX_REFERENCE_ORIGIN_LEAVES {
        return None;
    }
    program.machine_holding_state(state.symbol)?;
    let mut sources = Vec::with_capacity(origins.len());
    for origin in origins {
        if !origin.root.is_valid()
            || origin.representation.is_some()
            || !origin.recasts.is_empty()
            || origin.segments.len() > MAX_REFERENCE_ORIGIN_DEPTH
        {
            return None;
        }
        let (parameter_index, parameter) = program
            .state_parameters(state)
            .iter()
            .filter(|parameter| is_mutable_structural_parameter(program, parameter))
            .enumerate()
            .find(|(_, parameter)| parameter.symbol == origin.root)?;
        let segments = &origin.segments;
        let mut selected = parameter.type_reference;
        let mut borrowed = false;
        let mut path = Vec::with_capacity(origin.segments.len() + 1);
        if let Some((referee, parameter_access)) = stored_loan_parts(program, selected) {
            if !reference_access_can_supply(parameter_access, access) {
                return None;
            }
            selected = referee;
            borrowed = true;
        }
        for segment in segments {
            if let Some((referee, leaf_access)) = stored_loan_parts(program, selected) {
                if !reference_access_can_supply(leaf_access, access) {
                    return None;
                }
                path.push(checked_trees::CheckedUnitStructuralPathSegment::Referent);
                selected = referee;
                borrowed = true;
            }
            match segment {
                PlaceSegment::Field { symbol } => {
                    if crate::reference_shell(program, selected).is_some() {
                        return None;
                    }
                    let data = nominal_member_owner_type(program, selected)?;
                    let field =
                        program
                            .data_members(data)
                            .iter()
                            .find_map(|member| match member {
                                typed_trees::data::DataMember::Field(field)
                                    if field.symbol == *symbol && !field.relevance.is_erased() =>
                                {
                                    Some(field)
                                }
                                _ => None,
                            })?;
                    path.push(checked_trees::CheckedUnitStructuralPathSegment::Field(
                        CheckedMemberIdentity::of_member(field.identity, field.name.clone()),
                    ));
                    selected = field.type_reference;
                }
                PlaceSegment::FixedIndex { index } => {
                    selected = loop {
                        match program.type_reference_table.type_reference(selected) {
                            TypeReferenceNode::Constrained { base_type, .. } => {
                                selected = *base_type
                            }
                            TypeReferenceNode::FixedArray { element_type, .. } => {
                                break *element_type;
                            }
                            _ => return None,
                        }
                    };
                    path.push(checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(
                        u64::try_from(*index).ok()?,
                    ));
                }
                _ => return None,
            }
        }
        if let Some((referee, leaf_access)) = stored_loan_parts(program, selected) {
            if !reference_access_can_supply(leaf_access, access) {
                return None;
            }
            path.push(checked_trees::CheckedUnitStructuralPathSegment::Referent);
            selected = referee;
            borrowed = true;
        }
        if !borrowed
            || program.normalized_type_identity(selected)
                != program.normalized_type_identity(referent)
        {
            return None;
        }
        let source = checked_trees::CheckedUnitStructuralArgumentPlan {
            projection_evaluations: Default::default(),
            source: CheckedUnitStructuralArgumentSourcePlan::Parameter {
                parameter_index: u32::try_from(parameter_index).ok()?,
            },
            path,
            type_identity: program.normalized_type_identity(referent),
            access,
        };
        let source = checked_trees::CheckedReferenceResultSourcePlan {
            path: Vec::new(),
            source,
        };
        if !sources.contains(&source) {
            if sources.len() == MAX_REFERENCE_ORIGIN_LEAVES {
                return None;
            }
            sources.push(source);
        }
    }
    Some(sources)
}

pub fn terminal_call_reference_sources(
    program: &dyn CheckingInput,
    facts: &CheckFacts,
    state: &State,
    call: &checked_trees::FlowCallFact,
    result: &CheckedUnitStructuralResultBindingPlan,
) -> Option<Vec<checked_trees::CheckedReturnedReferencePlan>> {
    if call.authored_expression.is_valid()
        || call.statement_index != result.statement_index as usize
        || stored_loan_parts(program, state.return_type).is_none()
        || result.type_identity != program.normalized_type_identity(state.return_type)
    {
        return None;
    }
    let transition = program
        .statement_table
        .statement_at(state, call.statement_index)?
        .as_transition()?;
    if transition.exit != TransitionExit::Ordinary || transition.continuation.is_valid() {
        return None;
    }
    let TransitionTargetNode::Named { path, .. } =
        program.statement_table.transition_target(transition.target)
    else {
        return None;
    };
    let target = program.state_by_symbol(call.target_symbol).or_else(|| {
        program
            .machine_by_symbol(call.target_symbol)
            .and_then(|machine| program.machine_states(machine).first())
    })?;
    if path.symbol != target.symbol
        && !program
            .machine_by_symbol(path.symbol)
            .is_some_and(|machine| {
                program
                    .machine_states(machine)
                    .first()
                    .is_some_and(|first| first.symbol == target.symbol)
            })
    {
        return None;
    }
    captured_returns::terminal_call_sources(program, facts, state, call)
}

pub fn record_carrier_sources(
    program: &dyn CheckingInput,
    reference: TypeReferenceHandle,
    carrier: CheckedUnitStructuralArgumentSourcePlan,
) -> Option<Vec<checked_trees::CheckedReferenceResultSourcePlan>> {
    declared_carrier_references(program, reference)?
        .into_iter()
        .map(|leaf| {
            let path = leaf
                .path
                .iter()
                .map(|projection| match projection {
                    checked_trees::CheckedReferenceOriginProjection::Exact(segment) => {
                        Some(segment.clone())
                    }
                    checked_trees::CheckedReferenceOriginProjection::Element => None,
                })
                .collect::<Option<Vec<_>>>()?;
            let mut source_path = path.clone();
            if leaf.crosses_reference {
                source_path.push(checked_trees::CheckedUnitStructuralPathSegment::Referent);
            }
            Some(checked_trees::CheckedReferenceResultSourcePlan {
                path,
                source: checked_trees::CheckedUnitStructuralArgumentPlan {
                    projection_evaluations: Default::default(),
                    source: carrier.clone(),
                    path: source_path,
                    type_identity: leaf.type_identity,
                    access: leaf.access,
                },
            })
        })
        .collect()
}

pub struct DeclaredCarrierReference {
    pub path: Vec<checked_trees::CheckedReferenceOriginProjection>,
    pub type_identity: typed_trees::type_identity::TypeIdentityId,
    pub access: CheckedStructuralAccess,
    pub crosses_reference: bool,
}

/*
    The declared reference leaves of a carrier type, together with every place
    whose type is an unbound type parameter. A generic body cannot see what such
    a parameter is instantiated with, so the place may or may not hold references:
    it is an opaque leaf, never an empty one.

    Callers choose what an opaque leaf means for them. `declared_carrier_references`
    refuses the whole carrier when one exists, which is what every caller that
    needs a complete inventory (custody, result origins, cleanup) relies on. A
    caller that can act on the known leaves alone must say why the opaque ones
    cannot reach its judgment.
*/
pub struct DeclaredCarrierInventory {
    pub known: Vec<DeclaredCarrierReference>,
    pub opaque: Vec<Vec<checked_trees::CheckedReferenceOriginProjection>>,
}

pub fn declared_carrier_references(
    program: &dyn CheckingInput,
    reference: TypeReferenceHandle,
) -> Option<Vec<DeclaredCarrierReference>> {
    let inventory = declared_carrier_inventory(program, reference)?;
    inventory.opaque.is_empty().then_some(inventory.known)
}

pub fn declared_carrier_inventory(
    program: &dyn CheckingInput,
    reference: TypeReferenceHandle,
) -> Option<DeclaredCarrierInventory> {
    use checked_trees::CheckedReferenceOriginProjection::{Element, Exact};
    type Path = Vec<checked_trees::CheckedReferenceOriginProjection>;
    fn visit(
        program: &dyn CheckingInput,
        reference: TypeReferenceHandle,
        path: &mut Path,
        output: &mut Vec<DeclaredCarrierReference>,
        opaque: &mut Vec<Path>,
        scopes: &mut DeclarationScopes,
        scope: usize,
    ) -> Option<()> {
        let (reference, scope) = scopes.head(program, reference, scope);
        if let Some(shell) = crate::reference_shell(program, reference)
            && (matches!(
                program
                    .type_reference_table
                    .type_reference(crate::peel_declared_constraints(program, shell.referee)),
                TypeReferenceNode::Slice { .. }
            ) || crate::byte_sequence_carrier(program, reference, &[])
                .is_some_and(|carrier| carrier.is_borrowed()))
        {
            output.push(DeclaredCarrierReference {
                path: path.clone(),
                type_identity: scopes.type_identity(program, reference, scope),
                access: shell.access.into(),
                crosses_reference: false,
            });
            return Some(());
        }
        if let Some((referent, access)) = stored_loan_parts(program, reference) {
            output.push(DeclaredCarrierReference {
                path: path.clone(),
                type_identity: scopes.type_identity(program, referent, scope),
                access,
                crosses_reference: true,
            });
            return Some(());
        }
        if crate::has_contents(program, reference, &[], ContentsRequirement::NumericOwned) {
            return Some(());
        }
        if let TypeReferenceNode::FixedArray {
            element_type,
            length: typed_trees::types::FixedArrayLength::Literal(length),
        } = program.type_reference_table.type_reference(reference)
        {
            if *length == 0 {
                return Some(());
            }
            let mut element_leaves = Vec::new();
            let mut element_opaque = Vec::new();
            visit(
                program,
                *element_type,
                &mut Vec::new(),
                &mut element_leaves,
                &mut element_opaque,
                scopes,
                scope,
            )?;
            u64::try_from(*length - 1).ok()?;
            for leaf in element_opaque {
                let mut leaf_path = path.clone();
                leaf_path.push(Element);
                leaf_path.extend(leaf);
                opaque.push(leaf_path);
            }
            if element_leaves.is_empty() {
                return Some(());
            }
            for element_index in 0..*length {
                path.push(Exact(
                    checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(
                        u64::try_from(element_index).ok()?,
                    ),
                ));
                for leaf in &element_leaves {
                    let mut leaf_path = path.clone();
                    leaf_path.extend_from_slice(&leaf.path);
                    output.push(DeclaredCarrierReference {
                        path: leaf_path,
                        type_identity: leaf.type_identity,
                        access: leaf.access,
                        crosses_reference: leaf.crosses_reference,
                    });
                }
                path.pop();
            }
            return Some(());
        }
        if let TypeReferenceNode::Slice { element_type } =
            program.type_reference_table.type_reference(reference)
        {
            path.push(Element);
            visit(program, *element_type, path, output, opaque, scopes, scope)?;
            path.pop();
            return Some(());
        }
        if let TypeReferenceNode::Constrained { base_type, .. } =
            program.type_reference_table.type_reference(reference)
        {
            return visit(program, *base_type, path, output, opaque, scopes, scope);
        }
        if let TypeReferenceNode::Named { symbol, .. } =
            program.type_reference_table.type_reference(reference)
            && crate::activation_claim_byte_carrier(program, *symbol).is_some()
        {
            return Some(());
        }
        // Only an unbound type parameter that no earlier rule settled reaches here;
        // its instantiation is unknown to this declaration, so it is opaque.
        if let TypeReferenceNode::Named { symbol, .. } =
            program.type_reference_table.type_reference(reference)
            && program.symbols.get(*symbol).kind == symbols::SymbolKind::TypeParameter
        {
            opaque.push(path.clone());
            return Some(());
        }
        let selected = scopes.definition(program, reference, scope);
        let (data, scope) = selected?;
        if data.supply_mode != language_semantics::DataSupplyMode::CheckedShape {
            return None;
        }
        for member in program.data_members(data) {
            let fields = match member {
                typed_trees::data::DataMember::Field(field) => std::slice::from_ref(field),
                typed_trees::data::DataMember::Variant(case) => {
                    path.push(Exact(
                        checked_trees::CheckedUnitStructuralPathSegment::Case(
                            CheckedMemberIdentity::of_member(case.identity, case.name.clone()),
                        ),
                    ));
                    program.data_payload_fields(case)
                }
            };
            for field in fields {
                if field.relevance.is_erased() {
                    continue;
                }
                path.push(Exact(
                    checked_trees::CheckedUnitStructuralPathSegment::Field(
                        CheckedMemberIdentity::of_member(field.identity, field.name.clone()),
                    ),
                ));
                visit(
                    program,
                    field.type_reference,
                    path,
                    output,
                    opaque,
                    scopes,
                    scope,
                )?;
                path.pop();
            }
            if matches!(member, typed_trees::data::DataMember::Variant(_)) {
                path.pop();
            }
        }
        Some(())
    }
    if !reference_origin_expansion_bound(program, reference, true) {
        return None;
    }
    let mut known = Vec::new();
    let mut opaque = Vec::new();
    visit(
        program,
        reference,
        &mut Vec::new(),
        &mut known,
        &mut opaque,
        &mut DeclarationScopes::default(),
        0,
    )?;
    Some(DeclaredCarrierInventory { known, opaque })
}

fn declared_input_reference(
    program: &dyn CheckingInput,
    reference: TypeReferenceHandle,
    owner_path: &[BorrowLoanOwnerSegment],
    reference_hops: usize,
) -> Option<DeclaredCarrierReference> {
    use checked_trees::CheckedReferenceOriginProjection::{Element, Exact};
    use checked_trees::CheckedUnitStructuralPathSegment as Segment;
    let mut scopes = DeclarationScopes::default();
    let mut scope = 0;
    let mut selected = reference;
    let mut remaining = owner_path;
    let mut hops = reference_hops;
    let mut shared = false;
    let mut payload = None;
    let mut path = Vec::new();
    for _ in 0..MAX_REFERENCE_ORIGIN_VISITS {
        (selected, scope) = scopes.head(program, selected, scope);
        if let Some(shell) = crate::reference_shell(program, selected) {
            let view = matches!(
                program
                    .type_reference_table
                    .type_reference(crate::peel_declared_constraints(program, shell.referee)),
                TypeReferenceNode::Slice { .. }
            ) || crate::byte_sequence_carrier(program, selected, &[])
                .is_some_and(|carrier| carrier.is_borrowed());
            if remaining.is_empty() && hops == 0 {
                let mut access = CheckedStructuralAccess::from(shell.access);
                if shared {
                    match access {
                        CheckedStructuralAccess::WriteOnlyBorrow => return None,
                        CheckedStructuralAccess::MutableBorrow => {
                            access = CheckedStructuralAccess::SharedBorrow
                        }
                        _ => {}
                    }
                }
                return Some(DeclaredCarrierReference {
                    path,
                    type_identity: scopes.type_identity(
                        program,
                        if view { selected } else { shell.referee },
                        scope,
                    ),
                    access,
                    crosses_reference: !view,
                });
            }
            if !shell.access.is_readable() || (!view && hops == 0) || payload.is_some() {
                return None;
            }
            shared |= shell.access == language_semantics::ReferenceAccess::Shared;
            if !view {
                hops -= 1;
                path.push(Exact(Segment::Referent));
            }
            selected = shell.referee;
            continue;
        }
        if let TypeReferenceNode::Constrained { base_type, .. } =
            program.type_reference_table.type_reference(selected)
        {
            selected = *base_type;
            continue;
        }
        let (segment, suffix) = remaining.split_first()?;
        match segment {
            BorrowLoanOwnerSegment::Case(symbol) => {
                if payload.is_some() {
                    return None;
                }
                let (definition, nested_scope) = scopes.definition(program, selected, scope)?;
                if definition.supply_mode != language_semantics::DataSupplyMode::CheckedShape {
                    return None;
                }
                scope = nested_scope;
                let case = program
                    .data_members(definition)
                    .iter()
                    .filter_map(|member| member.as_variant())
                    .find(|case| case.symbol == *symbol)?;
                path.push(Exact(Segment::Case(CheckedMemberIdentity::of_member(
                    case.identity,
                    case.name.clone(),
                ))));
                payload = Some(case);
            }
            BorrowLoanOwnerSegment::Field(symbol) => {
                let fields = if let Some(case) = payload.take() {
                    program.data_payload_fields(case)
                } else {
                    let (definition, nested_scope) = scopes.definition(program, selected, scope)?;
                    if definition.supply_mode != language_semantics::DataSupplyMode::CheckedShape {
                        return None;
                    }
                    scope = nested_scope;
                    let field = program
                        .data_members(definition)
                        .iter()
                        .filter_map(|member| member.as_field())
                        .find(|field| field.symbol == *symbol && !field.relevance.is_erased())?;
                    path.push(Exact(Segment::Field(CheckedMemberIdentity::of_member(
                        field.identity,
                        field.name.clone(),
                    ))));
                    selected = field.type_reference;
                    remaining = suffix;
                    continue;
                };
                let field = fields
                    .iter()
                    .find(|field| field.symbol == *symbol && !field.relevance.is_erased())?;
                path.push(Exact(Segment::Field(CheckedMemberIdentity::of_member(
                    field.identity,
                    field.name.clone(),
                ))));
                selected = field.type_reference;
            }
            BorrowLoanOwnerSegment::FixedIndex(index) => {
                if payload.is_some() {
                    return None;
                }
                let TypeReferenceNode::FixedArray {
                    element_type,
                    length: typed_trees::types::FixedArrayLength::Literal(length),
                } = program.type_reference_table.type_reference(selected)
                else {
                    return None;
                };
                if *index >= *length {
                    return None;
                }
                path.push(Exact(Segment::FixedIndex(u64::try_from(*index).ok()?)));
                selected = *element_type;
            }
            BorrowLoanOwnerSegment::DynamicIndex => {
                if payload.is_some() {
                    return None;
                }
                let (TypeReferenceNode::Slice { element_type }
                | TypeReferenceNode::FixedArray { element_type, .. }) =
                    program.type_reference_table.type_reference(selected)
                else {
                    return None;
                };
                path.push(Element);
                selected = *element_type;
            }
        }
        remaining = suffix;
    }
    None
}

pub fn stored_carrier_ingress_is_complete(
    program: &dyn CheckingInput,
    facts: checked_trees::CheckFactsRef<'_>,
    machine: SymbolHandle,
    state: SymbolHandle,
    parameter: SymbolHandle,
    reference: TypeReferenceHandle,
) -> bool {
    let Some(carrier) = input_carrier_type(program, reference) else {
        return false;
    };
    let Some(sources) = record_carrier_sources(
        program,
        carrier,
        CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol: parameter },
    ) else {
        return false;
    };
    let incoming = facts
        .borrow
        .incoming_references
        .iter()
        .filter_map(|(_, row)| {
            (row.machine_symbol == machine
                && row.state_symbol == state
                && row.parameter_symbol() == parameter
                && row.carrier_type == carrier)
                .then_some(row)
        })
        .collect::<Vec<_>>();
    if incoming.len() != sources.len() {
        return false;
    }
    sources.iter().all(|source| {
        let Some(path) = loan_owner_path(program, carrier, &source.path) else {
            return false;
        };
        incoming
            .iter()
            .filter(|row| {
                row.carrier_type == carrier
                    && facts.borrow.owner_segments.span(row.owner_path) == Some(path.as_slice())
            })
            .count()
            == 1
    })
}

pub fn input_carrier_type(
    program: &dyn CheckingInput,
    reference: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    let reference = crate::peel_declared_constraints(program, reference);
    match crate::reference_shell(program, reference) {
        Some(shell) if shell.access.is_readable() => Some(shell.referee),
        Some(_) => None,
        None => Some(reference),
    }
}

pub fn local_owes_record_loans(
    program: &dyn CheckingInput,
    reference: TypeReferenceHandle,
) -> bool {
    is_reference_record(program, reference)
        && shared_borrowed_parts(program, reference).is_none()
        && crate::stored_view_shell(program, reference, &[]).is_none()
}

pub type RecordLeafLoans = (
    checked_trees::CheckedReferenceResultSourcePlan,
    arena::HandleSpan<checked_trees::BorrowLoanFact>,
);

pub fn local_record_loans(
    program: &dyn CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &State,
    statement_index: u32,
) -> Option<Vec<RecordLeafLoans>> {
    let local = program
        .statement_table
        .statements(state.statement_nodes)
        .get(statement_index as usize)?
        .as_local_data()?;

    named_place_loans(
        program,
        facts,
        machine,
        state,
        local.symbol,
        statement_index,
        &[],
    )
}

pub fn storage_owner_type_reference(
    program: &dyn CheckingInput,
    state: &State,
    owner: SymbolHandle,
    statement_index: u32,
) -> Option<TypeReferenceHandle> {
    if let Some((ordinal, local)) = program.state_local_declaration(state, owner) {
        return (ordinal <= statement_index as usize).then_some(local.type_reference);
    }
    crate::canonical_place_type_reference(
        program,
        state.symbol,
        statement_index as usize,
        &crate::CanonicalPlace {
            root: facts::PlaceRoot::Symbol(owner),
            segments: Vec::new(),
        },
    )
}

pub fn named_place_loans(
    program: &dyn CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &State,
    owner: SymbolHandle,
    statement_index: u32,
    selection: &[checked_trees::CheckedUnitStructuralPathSegment],
) -> Option<Vec<RecordLeafLoans>> {
    let reference = storage_owner_type_reference(program, state, owner, statement_index)?;
    storage_place_loans(
        program,
        facts,
        machine,
        state,
        facts::PlaceRoot::Symbol(owner),
        reference,
        statement_index,
        selection,
    )
    .ok()
}

pub fn storage_place_loans(
    program: &dyn CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &State,
    owner: facts::PlaceRoot,
    reference: TypeReferenceHandle,
    statement_index: u32,
    selection: &[checked_trees::CheckedUnitStructuralPathSegment],
) -> Result<Vec<RecordLeafLoans>, &'static str> {
    let selected_owner_path = crate::corresponding_loan_owner_path(
        program,
        reference,
        &loan_owner_path(program, reference, selection)
            .ok_or("the selected storage has no typed loan path")?,
    )
    .ok_or("the selected loan path does not fit its storage type")?;
    let borrowing = facts
        .borrow
        .unique_state_borrow(machine, state.symbol)
        .ok_or("the storage has no unique borrow state")?;
    let loans = facts
        .borrow
        .storage_loans(borrowing, owner, statement_index as usize)
        .ok_or("the storage has no complete formation loan group")?;
    let flow = state_flow(facts, machine, state.symbol)
        .ok_or("the storage has no completed state flow")?;
    let order = program
        .statement_table
        .admitted_execution_order(state.statement_nodes);
    let homes = crate::completed_storage::InstalledLoanHomes {
        control: &facts.flow.control,
        semantic: &facts.semantic,
        borrow: &facts.borrow,
        statements: flow.statements,
        order: &order,
        state: state.symbol,
    };
    let activations = facts
        .flow
        .borrow_lifetimes
        .activations
        .span(flow.borrow_activations)
        .ok_or("the storage has no complete activation roster")?;
    let mut captured = Vec::new();
    for (handle, loan) in facts
        .borrow
        .loan_group(loans)
        .ok_or("the storage loan group contains an unavailable loan")?
    {
        let home_path = homes.owner_path(program, loan);
        if !crate::corresponding_loan_owner_path(
            program,
            reference,
            home_path.as_ref(),
        )
        .ok_or_else(|| {
            if crate::trace_flags::check_sites() {
                eprintln!(
                    "CHECKSITE storage-loan-home loan={handle:?} formation={:?} storage_owner={:?} root={:?} reference={reference:?} published_path={:?} home_path={home_path:?}",
                    loan.formation_expression, loan.storage_owner, loan.root,
                    facts.borrow.loan_owner_path(loan),
                );
            }
            "the recorded descriptor home path does not fit its storage type"
        })?
        .starts_with(&selected_owner_path)
        {
            continue;
        }
        let mut matching = activations
            .iter()
            .filter(|activation| activation.loan == handle);
        let activation = matching
            .next()
            .ok_or("storage descriptor loan has no activation")?;
        if matching.next().is_some() {
            return Err("storage descriptor loan has more than one activation");
        }
        if activation.source
            != (checked_trees::FlowInvalidationSource::Statement {
                statement_index: statement_index as usize,
            })
        {
            return Err(match activation.source {
                checked_trees::FlowInvalidationSource::Call { .. } => {
                    "storage descriptor loan was activated by a call rather than a statement"
                }
                checked_trees::FlowInvalidationSource::Statement { .. } => {
                    "storage descriptor loan was activated at another statement"
                }
            });
        }
        captured.push(handle);
    }
    record_leaf_loan_groups(
        program, facts, reference, owner, &captured, selection, homes,
    )
}

pub fn initial_reference_establishment(
    program: &dyn CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &State,
    owner: SymbolHandle,
    statement_index: u32,
) -> Option<Vec<RecordLeafLoans>> {
    let reference = if let Some(parameter) = program.state_parameter_by_symbol(state, owner) {
        if !parameter.is_mutable
            || crate::reference_shell(program, parameter.type_reference)?.access
                != ReferenceAccess::Shared
        {
            return None;
        }
        parameter.type_reference
    } else {
        let (ordinal, local) = program.state_local_declaration(state, owner)?;
        if ordinal >= statement_index as usize || local.initial_value.is_valid() {
            return None;
        }
        crate::reference_shell(program, local.type_reference)?;
        local.type_reference
    };
    if !crate::stores_reference_shape(program, reference, &[]) {
        return None;
    }
    let borrowing = facts.borrow.unique_state_borrow(machine, state.symbol)?;
    let owner_path = loan_owner_path(program, reference, &[])?;
    let order = program
        .statement_table
        .admitted_execution_order(state.statement_nodes);
    if facts.borrow.loan_group(borrowing.loans)?.any(|(_, loan)| {
        loan.storage_owner == facts::PlaceRoot::Symbol(owner)
            && facts.borrow.loan_owner_path(loan) == owner_path.as_slice()
            && order.precedes(loan.statement_index, statement_index as usize)
    }) {
        return None;
    }
    let established =
        named_place_loans(program, facts, machine, state, owner, statement_index, &[])?;
    let [(_, loans)] = established.as_slice() else {
        return None;
    };
    (!loans.is_empty()).then_some(established)
}

pub fn replaced_reference_carrier(
    program: &dyn CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &State,
    owner: facts::PlaceRoot,
    statement_index: u32,
    selection: &[checked_trees::CheckedUnitStructuralPathSegment],
) -> Option<CheckedReferenceResultSourcePlan> {
    replaced_stored_reference_carrier(
        program,
        facts,
        machine,
        state,
        owner,
        statement_index,
        selection,
    )
    .map(|(carrier, _)| carrier)
}

fn replaced_stored_reference_carrier(
    program: &dyn CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &State,
    owner: facts::PlaceRoot,
    statement_index: u32,
    selection: &[checked_trees::CheckedUnitStructuralPathSegment],
) -> Option<RecordLeafLoans> {
    let order = program
        .statement_table
        .admitted_execution_order(state.statement_nodes);
    let reference = crate::canonical_place_type_reference(
        program,
        state.symbol,
        statement_index as usize,
        &crate::CanonicalPlace {
            root: owner,
            segments: Vec::new(),
        },
    )?;
    let owner_path = loan_owner_path(program, reference, selection)?;
    let flow = state_flow(facts, machine, state.symbol)?;
    let homes = crate::completed_storage::InstalledLoanHomes {
        control: &facts.flow.control,
        semantic: &facts.semantic,
        borrow: &facts.borrow,
        statements: flow.statements,
        order: &order,
        state: state.symbol,
    };
    let mut formation = None;
    for weakening in facts
        .flow
        .borrow_lifetimes
        .weakenings
        .span(flow.borrow_weakenings)?
    {
        if weakening.reason != checked_trees::FlowBorrowWeakeningReason::LocalReassigned
            || weakening.source
                != (checked_trees::FlowInvalidationSource::Statement {
                    statement_index: statement_index as usize,
                })
        {
            continue;
        }
        let loan = facts.borrow.loans.get(weakening.loan);
        if loan.storage_owner != owner || homes.owner_path(program, loan).as_ref() != owner_path {
            continue;
        }
        if !order.precedes(loan.statement_index, statement_index as usize)
            || formation.is_some_and(|previous| previous != loan.statement_index)
        {
            return None;
        }
        formation = Some(loan.statement_index);
    }
    let groups = storage_place_loans(
        program,
        facts,
        machine,
        state,
        owner,
        reference,
        u32::try_from(formation?).ok()?,
        selection,
    )
    .ok()?;
    let [group] = groups.as_slice() else {
        return None;
    };
    if group.0.path != selection
        || (group.0.source.path != selection
            && group
                .0
                .source
                .path
                .strip_suffix(&[checked_trees::CheckedUnitStructuralPathSegment::Referent])
                != Some(selection))
        || !facts.borrow.loan_group(group.1)?.all(|(handle, _)| {
            loan_weakening(facts, machine, state.symbol, handle).is_some_and(|weakening| {
                weakening.reason == checked_trees::FlowBorrowWeakeningReason::LocalReassigned
                    && weakening.source
                        == (checked_trees::FlowInvalidationSource::Statement {
                            statement_index: statement_index as usize,
                        })
            })
        })
    {
        return None;
    }
    Some(group.clone())
}

fn record_leaf_loan_groups(
    program: &dyn CheckingInput,
    facts: &CheckFacts,
    reference: TypeReferenceHandle,
    owner: facts::PlaceRoot,
    loans: &[arena::Handle<checked_trees::BorrowLoanFact>],
    selection: &[checked_trees::CheckedUnitStructuralPathSegment],
    homes: crate::completed_storage::InstalledLoanHomes<'_>,
) -> Result<Vec<RecordLeafLoans>, &'static str> {
    let source = match owner {
        facts::PlaceRoot::Symbol(symbol) => {
            CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol }
        }
        facts::PlaceRoot::Expression(expression) => {
            CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
                value_dependency: None,
                expression,
                logical_root_type_identity: None,
            }
        }
        _ => return Err("stored reference owner has no executable source form"),
    };
    let sources = record_carrier_sources(program, reference, source)
        .ok_or("stored reference type has no complete descriptor frontier")?;
    let mut grouped_count = 0usize;
    let mut output = Vec::with_capacity(sources.len());
    for source in sources
        .into_iter()
        .filter(|source| source.path.starts_with(selection))
    {
        let declared_path = loan_owner_path(program, reference, &source.path)
            .ok_or("stored reference leaf has no typed owner path")?;
        let owner_path = crate::corresponding_loan_owner_path(program, reference, &declared_path)
            .ok_or("stored reference leaf does not fit its carrier")?;
        let access = source
            .source
            .access
            .borrow_access_kind()
            .ok_or("stored reference leaf has no borrowing access")?;
        let mut group = arena::HandleSpan::empty();
        let mut formation = None;
        let mut has_pointer = false;
        for handle in loans.iter().copied() {
            let loan = facts.borrow.loans.get(handle);
            let captured_path = crate::corresponding_loan_owner_path(
                program,
                reference,
                homes.owner_path(program, loan).as_ref(),
            )
            .ok_or("captured loan owner path does not fit its carrier")?;
            if captured_path != owner_path {
                continue;
            }
            let dependency = loan.lineage == checked_trees::BorrowLoanLineage::LifetimeDependency;
            if loan.storage_owner != owner
                || (!dependency && loan.kind != access)
                || formation.is_some_and(|statement| statement != loan.statement_index)
                || (!group.is_empty() && !group.is_followed_by(handle))
            {
                return Err("stored reference loans disagree on owner, access or formation group");
            }
            has_pointer |= !dependency;
            formation = Some(loan.statement_index);
            let start = if group.is_empty() {
                handle
            } else {
                group.start()
            };
            group = arena::HandleSpan::from_parts(
                start,
                group
                    .count()
                    .checked_add(1)
                    .ok_or("stored reference loan group exceeds its count representation")?,
            );
            grouped_count = grouped_count
                .checked_add(1)
                .ok_or("stored reference loan accounting exceeds usize")?;
        }
        if group.is_empty() {
            continue;
        }
        if !has_pointer {
            // Signature lifetime dependencies remain in the borrow facts.
            // They describe retained inputs, not an executable descriptor
            // to install in this value's storage.
            continue;
        }
        output.push((source, group));
    }
    if grouped_count != loans.len() {
        return Err("activated storage loan has no corresponding typed reference leaf");
    }
    Ok(output)
}

pub fn loan_place_segments(
    program: &dyn CheckingInput,
    root: TypeReferenceHandle,
    path: &[checked_trees::CheckedUnitStructuralPathSegment],
) -> Option<Vec<PlaceSegment>> {
    origin_place_segments(
        program,
        root,
        path.iter()
            .cloned()
            .map(checked_trees::CheckedReferenceOriginProjection::Exact),
    )
}

pub fn origin_place_segments(
    program: &dyn CheckingInput,
    root: TypeReferenceHandle,
    path: impl IntoIterator<Item = checked_trees::CheckedReferenceOriginProjection>,
) -> Option<Vec<PlaceSegment>> {
    let mut crosses_reference = false;
    let projected = origin_place_projection(
        program,
        root,
        path.into_iter().inspect(|projection| {
            crosses_reference |= matches!(
                projection,
                checked_trees::CheckedReferenceOriginProjection::Exact(
                    checked_trees::CheckedUnitStructuralPathSegment::Referent
                )
            );
        }),
    )?;
    (!crosses_reference).then_some(projected.segments)
}

pub(crate) struct OriginPlaceProjection {
    pub segments: Vec<PlaceSegment>,
    endpoint: TypeReferenceHandle,
    scope: usize,
    scopes: DeclarationScopes,
}

impl OriginPlaceProjection {
    pub(crate) fn type_identity(
        &mut self,
        program: &dyn CheckingInput,
    ) -> typed_trees::type_identity::TypeIdentityId {
        self.scopes
            .type_identity(program, self.endpoint, self.scope)
    }
}

pub(crate) fn origin_place_projection(
    program: &dyn CheckingInput,
    root: TypeReferenceHandle,
    path: impl IntoIterator<Item = checked_trees::CheckedReferenceOriginProjection>,
) -> Option<OriginPlaceProjection> {
    let mut selected = root;
    let mut payload = None;
    let mut segments = Vec::new();
    let mut scopes = DeclarationScopes::default();
    let mut scope = 0;
    for projection in path {
        (selected, scope) = scopes.head(program, selected, scope);
        if matches!(
            projection,
            checked_trees::CheckedReferenceOriginProjection::Exact(
                checked_trees::CheckedUnitStructuralPathSegment::Referent
            )
        ) {
            if payload.is_some() {
                return None;
            }
            let shell = crate::reference_shell(program, selected)?;
            (selected, scope) = scopes.head(program, shell.referee, scope);
            continue;
        }
        while let TypeReferenceNode::Constrained { base_type, .. }
        | TypeReferenceNode::Reference {
            referee: base_type, ..
        } = program.type_reference_table.type_reference(selected)
        {
            (selected, scope) = scopes.head(program, *base_type, scope);
        }
        let checked_trees::CheckedReferenceOriginProjection::Exact(segment) = projection else {
            if payload.is_some() {
                return None;
            }
            let (TypeReferenceNode::Slice { element_type }
            | TypeReferenceNode::FixedArray { element_type, .. }) =
                program.type_reference_table.type_reference(selected)
            else {
                return None;
            };
            selected = *element_type;
            segments.push(PlaceSegment::Index {
                expression: ExpressionHandle::invalid(),
            });
            continue;
        };
        let segment = &segment;
        match segment {
            checked_trees::CheckedUnitStructuralPathSegment::Case(identity) => {
                if payload.is_some() {
                    return None;
                }
                let (data, nested_scope) = scopes.definition(program, selected, scope)?;
                scope = nested_scope;
                let case = program
                    .data_members(data)
                    .iter()
                    .find_map(|member| match member {
                        typed_trees::data::DataMember::Variant(case)
                            if CheckedMemberIdentity::of_member(
                                case.identity,
                                case.name.clone(),
                            ) == *identity =>
                        {
                            Some(case)
                        }
                        _ => None,
                    })?;
                segments.push(PlaceSegment::Case {
                    variant: case.symbol,
                });
                payload = Some(case);
            }
            checked_trees::CheckedUnitStructuralPathSegment::Field(identity) => {
                let field = if let Some(case) = payload.take() {
                    program.data_payload_fields(case).iter().find(|field| {
                        CheckedMemberIdentity::of_member(field.identity, field.name.clone())
                            == *identity
                    })?
                } else {
                    let (data, nested_scope) = scopes.definition(program, selected, scope)?;
                    scope = nested_scope;
                    program
                        .data_members(data)
                        .iter()
                        .find_map(|member| match member {
                            typed_trees::data::DataMember::Field(field)
                                if CheckedMemberIdentity::of_member(
                                    field.identity,
                                    field.name.clone(),
                                ) == *identity =>
                            {
                                Some(field)
                            }
                            _ => None,
                        })?
                };
                segments.push(PlaceSegment::Field {
                    symbol: field.symbol,
                });
                selected = field.type_reference;
            }
            checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(index) => {
                if payload.is_some() {
                    return None;
                }
                let mut carrier = selected;
                loop {
                    match program.type_reference_table.type_reference(carrier) {
                        TypeReferenceNode::Constrained { base_type, .. } => carrier = *base_type,
                        TypeReferenceNode::FixedArray { element_type, .. }
                        | TypeReferenceNode::Slice { element_type } => {
                            selected = *element_type;
                            break;
                        }
                        _ => return None,
                    }
                }
                segments.push(PlaceSegment::FixedIndex {
                    index: usize::try_from(*index).ok()?,
                });
            }
            _ => return None,
        }
    }
    Some(OriginPlaceProjection {
        segments,
        endpoint: selected,
        scope,
        scopes,
    })
}

pub fn loan_owner_path(
    program: &dyn CheckingInput,
    root: TypeReferenceHandle,
    path: &[checked_trees::CheckedUnitStructuralPathSegment],
) -> Option<Vec<BorrowLoanOwnerSegment>> {
    origin_owner_path(
        program,
        root,
        path.iter()
            .cloned()
            .map(checked_trees::CheckedReferenceOriginProjection::Exact),
    )
}

pub fn origin_owner_path(
    program: &dyn CheckingInput,
    root: TypeReferenceHandle,
    path: impl IntoIterator<Item = checked_trees::CheckedReferenceOriginProjection>,
) -> Option<Vec<BorrowLoanOwnerSegment>> {
    origin_place_segments(program, root, path)?
        .into_iter()
        .map(|segment| match segment {
            PlaceSegment::Field { symbol } => Some(BorrowLoanOwnerSegment::Field(symbol)),
            PlaceSegment::FixedIndex { index } => Some(BorrowLoanOwnerSegment::FixedIndex(index)),
            PlaceSegment::Case { variant } => Some(BorrowLoanOwnerSegment::Case(variant)),
            PlaceSegment::Index { expression } if !expression.is_valid() => {
                Some(BorrowLoanOwnerSegment::DynamicIndex)
            }
            _ => None,
        })
        .collect()
}

fn statement_entry_constraints(
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement_index: u32,
) -> Option<&[checked_trees::FlowConstraintRef]> {
    let flow = state_flow(facts, machine, state)?;
    let entries = facts.flow.control.statements.span(flow.statements)?;
    let mut entries = entries
        .iter()
        .filter(|entry| entry.statement_index == statement_index as usize);
    let entry = entries.next()?;
    if entries.next().is_some() {
        return None;
    }
    facts
        .flow
        .contexts
        .constraint_refs
        .span(entry.entry_constraints)
}

fn active_record_loans(
    program: &dyn CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &State,
    statement_index: u32,
    declaration_statement: u32,
) -> Option<Vec<RecordLeafLoans>> {
    let order = program
        .statement_table
        .admitted_execution_order(state.statement_nodes);
    if !order.precedes(declaration_statement as usize, statement_index as usize) {
        return None;
    }
    let local = program
        .statement_table
        .statements(state.statement_nodes)
        .get(declaration_statement as usize)?
        .as_local_data()?;
    let constraints = statement_entry_constraints(facts, machine, state.symbol, statement_index)?;
    let mut loans = constraints
        .iter()
        .filter_map(|constraint| {
            let checked_trees::FlowConstraintKind::BorrowLoan { loan } = constraint.kind else {
                return None;
            };
            let captured = facts.borrow.loans.get(loan);
            (captured.storage_owner == facts::PlaceRoot::Symbol(local.symbol)).then_some(loan)
        })
        .collect::<Vec<_>>();
    loans.sort_unstable_by_key(|handle| (handle.arena_index(), handle.generation()));
    if loans.iter().any(|handle| {
        !order.precedes(
            facts.borrow.loans.get(*handle).statement_index,
            statement_index as usize,
        )
    }) {
        return None;
    }
    let flow = state_flow(facts, machine, state.symbol)?;
    record_leaf_loan_groups(
        program,
        facts,
        local.type_reference,
        facts::PlaceRoot::Symbol(local.symbol),
        &loans,
        &[],
        crate::completed_storage::InstalledLoanHomes {
            control: &facts.flow.control,
            semantic: &facts.semantic,
            borrow: &facts.borrow,
            statements: flow.statements,
            order: &order,
            state: state.symbol,
        },
    )
    .ok()
}

fn reference_record_call_sources<'program>(
    program: &'program dyn CheckingInput,
    facts: &CheckFacts,
    target: SymbolHandle,
) -> Option<(
    &'program [typed_trees::signature::StateParameter],
    Vec<checked_trees::CheckedReturnedReferencePlan>,
)> {
    let return_type = crate::call_target_return_type(program, target)?;
    let parameters = crate::call_target_parameters(program, target)?;
    if !is_reference_record(program, return_type) {
        return None;
    }
    let sources = captured_returns::call_sources(program, facts, target)?;
    Some((parameters, sources))
}

pub fn transient_call_record_loans(
    program: &dyn CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &State,
    call: &checked_trees::FlowCallFact,
    arguments: &[checked_trees::CheckedUnitStructuralArgumentPlan],
    residual_paths: &[Vec<checked_trees::CheckedUnitStructuralPathSegment>],
) -> bool {
    let Ok(occurrence) = call_reception::retained_call(facts, machine, state.symbol, call) else {
        return false;
    };
    let Some((parameters, sources)) =
        reference_record_call_sources(program, facts, occurrence.target_symbol)
    else {
        return false;
    };
    let Some(paths) = captured_returns::call_result_paths(program, facts, occurrence) else {
        return false;
    };
    !sources.is_empty()
        && residual_paths.iter().all(|path| paths.contains(path))
        && sources
            .iter()
            .filter(|source| residual_paths.contains(&source.path))
            .all(|source| {
                let parameter_index = match source.source.root {
                    checked_trees::CheckedReferenceOriginRoot::Parameter { index } => index,
                    checked_trees::CheckedReferenceOriginRoot::Invocation { .. } => {
                        return true;
                    }
                };
                let Some((_, parameter)) = origin_formal(program, parameters, parameter_index)
                else {
                    return false;
                };
                if crate::reference_shell(program, parameter.type_reference).is_some() {
                    let Some(actual) = arguments.get(parameter_index as usize) else {
                        return false;
                    };
                    let Some(parent) = actual.access.borrow_access_kind() else {
                        return false;
                    };
                    let Some(child) = source.source.access.borrow_access_kind() else {
                        return false;
                    };
                    parent.direct_reborrow_effect(&child).is_some()
                } else {
                    arguments
                        .get(parameter_index as usize)
                        .is_some_and(|actual| {
                            actual.type_identity
                                == crate::structural_type_identity(
                                    program,
                                    parameter.type_reference,
                                    &[],
                                    &[],
                                )
                        })
                }
            })
}

fn call_result_field_argument(
    program: &dyn CheckingInput,
    facts: &CheckFacts,
    state: &State,
    result: &CheckedUnitStructuralResultBindingPlan,
    expression: ExpressionHandle,
    destination: TypeReferenceHandle,
) -> Option<checked_trees::CheckedUnitStructuralArgumentPlan> {
    let projection = aggregate_projection(program, state, expression)?;
    let ExpressionNode::Call(produced) = expression_table(program).expression(projection.root)
    else {
        return None;
    };
    let callee = crate::call_target_state(program, produced.target_symbol)?;
    if result.type_identity != program.normalized_type_identity(callee.return_type) {
        return None;
    }
    let destination_leaves = declared_carrier_references(program, destination)?;
    let [destination_leaf] = destination_leaves.as_slice() else {
        return None;
    };
    if !destination_leaf.path.is_empty() || crate::reference_shell(program, destination).is_none() {
        return None;
    }
    if program.normalized_type_identity(projection.selected_type)
        != program.normalized_type_identity(destination)
    {
        return None;
    }
    let returned = captured_return_sources(program, facts, callee)?;
    let leaf = returned.iter().find(|leaf| leaf.path == projection.path)?;
    if leaf.source.type_identity != destination_leaf.type_identity
        || !reference_access_can_supply(leaf.source.access, destination_leaf.access)
    {
        return None;
    }
    let mut path = projection.path;
    if destination_leaf.crosses_reference {
        path.push(checked_trees::CheckedUnitStructuralPathSegment::Referent);
    }
    Some(checked_trees::CheckedUnitStructuralArgumentPlan {
        projection_evaluations: Default::default(),
        source: CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
            binding_ordinal: result.binding_ordinal,
        },
        path,
        type_identity: crate::structural_type_identity(program, destination, &[], &[]),
        access: destination_leaf.access,
    })
}

pub fn record_argument(
    program: &dyn CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &State,
    statement_index: u32,
    expression: typed_trees::expression::ExpressionHandle,
    result: &CheckedUnitStructuralResultBindingPlan,
    destination: TypeReferenceHandle,
) -> Option<checked_trees::CheckedUnitStructuralArgumentPlan> {
    if let Some(argument) =
        call_result_field_argument(program, facts, state, result, expression, destination)
    {
        return Some(argument);
    }
    let destination_leaves = declared_carrier_references(program, destination)?;
    let [destination_leaf] = destination_leaves.as_slice() else {
        return None;
    };
    if !destination_leaf.path.is_empty() || crate::reference_shell(program, destination).is_none() {
        return None;
    }
    let access = destination_leaf.access;
    let local = program
        .statement_table
        .statements(state.statement_nodes)
        .get(result.statement_index as usize)?
        .as_local_data()?;
    if !destination_leaf.crosses_reference
        && crate::stored_view_shell(program, local.type_reference, &[]).is_some()
    {
        return None;
    }
    if result.type_identity != program.normalized_type_identity(local.type_reference)
        || result.multiplicity != result_multiplicity(program, local.type_reference)
    {
        return None;
    }
    let projection = aggregate_projection(program, state, expression)?;
    let name = expression_table(program).name_path_node(projection.root)?;
    if name.symbol != local.symbol
        || name.head_symbol != local.symbol
        || !matches!(expression_table(program).name_path_members(name.members), [name] if *name == local.name)
    {
        return None;
    }
    let mut path = projection.path;
    if program.normalized_type_identity(projection.selected_type)
        != program.normalized_type_identity(destination)
    {
        return None;
    }
    let loans = active_record_loans(
        program,
        facts,
        machine,
        state,
        statement_index,
        result.statement_index,
    )?;
    let (source, _) = loans.iter().find(|(source, _)| source.path == path)?;
    if source.source.type_identity != destination_leaf.type_identity
        || !reference_access_can_supply(source.source.access, access)
    {
        return None;
    }
    if destination_leaf.crosses_reference {
        path.push(checked_trees::CheckedUnitStructuralPathSegment::Referent);
    }
    Some(checked_trees::CheckedUnitStructuralArgumentPlan {
        projection_evaluations: Default::default(),
        source: CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
            binding_ordinal: result.binding_ordinal,
        },
        path,
        type_identity: crate::structural_type_identity(program, destination, &[], &[]),
        access,
    })
}

pub fn classify_program_reference_custody(program: &dyn CheckingInput) -> ReferenceCustodyFacts {
    crate::classify_type_references(program, reference_custody, ReferenceCustodyFacts::with_rows)
}

fn reference_custody(
    program: &dyn CheckingInput,
    reference: TypeReferenceHandle,
) -> ReferenceCustodyRow {
    let stored = stored_loan_parts(program, reference);
    ReferenceCustodyRow {
        primitive_loan_referent: parts(program, reference)
            .map(|(referent, _)| referent)
            .unwrap_or_else(TypeReferenceHandle::invalid),
        stored_loan_referent: stored
            .map(|(referent, _)| referent)
            .unwrap_or_else(TypeReferenceHandle::invalid),
        stored_loan_access: stored
            .map(|(_, access)| access)
            .unwrap_or(CheckedStructuralAccess::Owned),
        mutable_borrow_referent: mutable_borrowed_parts(program, reference)
            .map(|(referent, _)| referent)
            .unwrap_or_else(TypeReferenceHandle::invalid),
        shared_borrow_referent: shared_borrowed_parts(program, reference)
            .map(|(referent, _)| referent)
            .unwrap_or_else(TypeReferenceHandle::invalid),
        is_reference_record: is_reference_record(program, reference),
        binding_multiplicity: binding_multiplicity(program, reference),
        result_multiplicity: result_multiplicity(program, reference),
    }
}

pub fn parts(
    program: &dyn CheckingInput,
    reference: TypeReferenceHandle,
) -> Option<(TypeReferenceHandle, CheckedStructuralAccess)> {
    let TypeReferenceNode::Reference {
        referee, access, ..
    } = program.type_reference_table.type_reference(reference)
    else {
        return None;
    };
    if !access.is_exclusive()
        || !matches!(
            program.type_reference_table.type_reference(*referee),
            TypeReferenceNode::Named { .. }
        )
        || program.primitive_type_reference(*referee).is_none()
    {
        return None;
    }
    Some((*referee, (*access).into()))
}

pub fn stored_loan_parts(
    program: &dyn CheckingInput,
    reference: TypeReferenceHandle,
) -> Option<(TypeReferenceHandle, CheckedStructuralAccess)> {
    crate::reference_shell(program, reference)
        .filter(|_| crate::stores_reference_shape(program, reference, &[]))
        .map(|shell| (shell.referee, shell.access.into()))
}

pub fn mutable_borrowed_parts(
    program: &dyn CheckingInput,
    reference: TypeReferenceHandle,
) -> Option<(TypeReferenceHandle, CheckedStructuralAccess)> {
    crate::reference_shell(program, reference)
        .filter(|shell| shell.access == ReferenceAccess::Mutable)
        .map(|shell| (shell.referee, CheckedStructuralAccess::MutableBorrow))
}

pub fn shared_borrowed_parts(
    program: &dyn CheckingInput,
    reference: TypeReferenceHandle,
) -> Option<(TypeReferenceHandle, CheckedStructuralAccess)> {
    stored_loan_parts(program, reference)
        .filter(|(_, access)| *access == CheckedStructuralAccess::SharedBorrow)
}

pub fn has_borrowed_parts(program: &dyn CheckingInput, reference: TypeReferenceHandle) -> bool {
    stored_loan_parts(program, reference).is_some()
        || mutable_borrowed_parts(program, reference).is_some()
}

pub fn shared_place_borrowed_parts(
    program: &dyn CheckingInput,
    reference: TypeReferenceHandle,
) -> Option<(TypeReferenceHandle, CheckedStructuralAccess)> {
    crate::reference_shell(program, reference)
        .filter(|shell| shell.access == ReferenceAccess::Shared)
        .map(|shell| (shell.referee, CheckedStructuralAccess::SharedBorrow))
}

pub fn path_has_nominal_receiver(
    program: &dyn CheckingInput,
    root: TypeReferenceHandle,
    path: &[PlaceSegment],
) -> bool {
    (0..path.len()).any(|length| {
        crate::project_type_reference_from_segments(program, root, &path[..length]).is_none_or(
            |reference| crate::nominal_cleanup_machine_for_type(program, reference, &[]).is_some(),
        )
    })
}

pub fn is_copyable_stored_value(
    program: &dyn CheckingInput,
    reference: TypeReferenceHandle,
) -> bool {
    reference.is_valid()
        && crate::has_contents(
            program,
            reference,
            &[],
            crate::ContentsRequirement::StoredValue,
        )
        && crate::type_satisfies_declared_property(
            program,
            &[],
            reference,
            crate::DeclaredPropertyRequirement::Copy,
        )
}

pub fn source_value_is_copied(program: &dyn CheckingInput, reference: TypeReferenceHandle) -> bool {
    is_copyable_stored_value(program, reference)
        && program.type_multiplicity(reference) == Multiplicity::Unrestricted
}

pub fn binding_multiplicity(
    program: &dyn CheckingInput,
    reference: TypeReferenceHandle,
) -> Multiplicity {
    match crate::reference_shell(program, reference) {
        Some(shell) if shell.access == ReferenceAccess::Shared => Multiplicity::Unrestricted,
        Some(_) => Multiplicity::Affine,
        None => result_multiplicity(program, reference),
    }
}

pub fn result_multiplicity(
    program: &dyn CheckingInput,
    reference: TypeReferenceHandle,
) -> Multiplicity {
    match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Constrained { base_type, .. } => {
            return result_multiplicity(program, *base_type);
        }
        TypeReferenceNode::FixedArray { element_type, .. } => {
            return result_multiplicity(program, *element_type);
        }
        _ => {}
    }
    if stored_loan_parts(program, reference).is_some()
        || mutable_borrowed_parts(program, reference).is_some()
        || (program.type_multiplicity(reference) == Multiplicity::Unrestricted
            && is_reference_record(program, reference))
    {
        Multiplicity::Affine
    } else {
        program.type_multiplicity(reference)
    }
}

pub fn source_parameter(program: &dyn CheckingInput, state: &State) -> Option<usize> {
    let sources = whole_reference_sources(program, state, None)?;
    if sources.iter().any(|source| !source.source.path.is_empty()) {
        return None;
    }
    let position = reference_sources_parameter(program, state, &sources)?;
    let parameter = program.state_parameters(state).get(position)?;
    stored_loan_parts(program, parameter.type_reference)?;
    Some(position)
}

pub fn source_leaf(
    program: &dyn CheckingInput,
    state: &State,
) -> Option<(usize, checked_trees::CheckedUnitStructuralArgumentPlan)> {
    parts(program, state.return_type)?;
    let sources = whole_reference_sources(program, state, None)?;
    let [source] = sources.as_slice() else {
        return None;
    };
    let CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } =
        source.source.source
    else {
        return None;
    };
    let (position, parameter) =
        origin_formal(program, program.state_parameters(state), parameter_index)?;
    if stored_loan_parts(program, parameter.type_reference).is_some()
        || !is_reference_record(program, parameter.type_reference)
        || !matches!(
            source.source.path.as_slice(),
            [
                _,
                ..,
                checked_trees::CheckedUnitStructuralPathSegment::Referent
            ]
        )
    {
        return None;
    }
    Some((position, source.source.clone()))
}

fn reference_sources_parameter(
    program: &dyn CheckingInput,
    state: &State,
    sources: &[checked_trees::CheckedReferenceResultSourcePlan],
) -> Option<usize> {
    let (_, access) = stored_loan_parts(program, state.return_type)?;
    let mut parameter = None;
    for source in sources {
        if !source.path.is_empty() || source.source.access != access {
            return None;
        }
        let CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } =
            source.source.source
        else {
            return None;
        };
        let (position, _) =
            origin_formal(program, program.state_parameters(state), parameter_index)?;
        if parameter.is_some_and(|prior| prior != position) {
            return None;
        }
        parameter = Some(position);
    }
    parameter
}

pub fn result_loans(
    program: &dyn CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &State,
    call: &checked_trees::FlowCallFact,
    result: &CheckedUnitStructuralResultBindingPlan,
) -> Option<arena::HandleSpan<checked_trees::BorrowLoanFact>> {
    if !result.local.is_named()
        && call.authored_expression.is_valid()
        && call.statement_index == result.statement_index as usize
        && !program
            .statement_table
            .statement_at(state, call.statement_index)
            .and_then(typed_trees::statement::StatementNode::as_local_data)
            .is_some_and(|local| local.initial_value == call.authored_expression)
        && crate::call_target_return_type(program, call.target_symbol)
            .is_some_and(|return_type| crate::reference_shell(program, return_type).is_some())
    {
        return Some(arena::HandleSpan::empty());
    }
    let statement = program
        .statement_table
        .statements(state.statement_nodes)
        .get(result.statement_index as usize)?;
    match statement {
        StatementNode::Call(authored)
            if authored.discards_result
                && !call.authored_expression.is_valid()
                && call.statement_index == result.statement_index as usize
                && call.call_ordinal == 0
                && crate::call_target_entry_symbol(program, authored.target_symbol)
                    == call.target_symbol =>
        {
            return Some(arena::HandleSpan::empty());
        }
        _ => {}
    }
    if !call.authored_expression.is_valid()
        || call.statement_index != result.statement_index as usize
        || crate::call_target_entry_symbol(
            program,
            crate::call_target_symbol(
                program,
                expression_table(program).call_node(call.authored_expression)?,
            ),
        ) != call.target_symbol
    {
        return None;
    }
    let return_type = crate::call_target_return_type(program, call.target_symbol)?;
    let shell = crate::reference_shell(program, return_type)?;
    let expected_kind = checked_trees::BorrowAccessKind::from(shell.access);
    let borrowing = facts.borrow.unique_state_borrow(machine, state.symbol)?;
    let mut group = arena::HandleSpan::empty();
    for (handle, loan) in facts.borrow.loan_group(borrowing.loans)? {
        if loan.statement_index != call.statement_index
            || loan.formation_expression != call.authored_expression
        {
            continue;
        }
        if expected_kind.direct_reborrow_effect(&loan.kind).is_none()
            || (!group.is_empty() && !group.is_followed_by(handle))
        {
            return None;
        }
        let start = if group.is_empty() {
            handle
        } else {
            group.start()
        };
        group = arena::HandleSpan::from_parts(start, group.count().checked_add(1)?);
    }
    (!group.is_empty()).then_some(group)
}

pub fn whole_local_release_boundary(
    program: &dyn CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &State,
    statement_index: u32,
) -> Option<u32> {
    let statements = program.statement_table.statements(state.statement_nodes);
    let local = statements.get(statement_index as usize)?.as_local_data()?;
    let loans = local_record_loans(program, facts, machine, state, statement_index)?;
    if !local.symbol.is_valid() {
        return None;
    }
    let order = program
        .statement_table
        .admitted_execution_order(state.statement_nodes);
    let mut boundary = order.successor(statement_index as usize)?;
    for (_, group) in loans {
        for (loan, _) in facts.borrow.loan_group(group)? {
            let incoming = release_statement(program, facts, machine, state.symbol, loan)? as usize;
            if order.position(incoming)? > order.position(boundary)? {
                boundary = incoming;
            }
        }
    }
    for index in order
        .indices()
        .iter()
        .copied()
        .skip(order.position(statement_index as usize)? + 1)
    {
        if crate::value_custody::source_liveness::statement_uses_symbol(
            program,
            &statements[index],
            local.symbol,
        ) {
            let incoming = order.successor(index)?;
            if order.position(incoming)? > order.position(boundary)? {
                boundary = incoming;
            }
        }
    }
    let boundary = u32::try_from(boundary).ok()?;
    Some(boundary)
}

pub fn local_storage_loans(
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    root: SymbolHandle,
) -> Option<Vec<arena::Handle<checked_trees::BorrowLoanFact>>> {
    storage_loans(facts, machine, state, facts::PlaceRoot::Symbol(root))
}

pub fn storage_loans(
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    root: facts::PlaceRoot,
) -> Option<Vec<arena::Handle<checked_trees::BorrowLoanFact>>> {
    let borrowing = facts.borrow.unique_state_borrow(machine, state)?;
    Some(
        facts
            .borrow
            .statement_resource_loans(borrowing)?
            .filter_map(|(handle, loan)| {
                (facts.borrow.state_owns_loan(borrowing, handle) && loan.root == root)
                    .then_some(handle)
            })
            .collect(),
    )
}

pub fn loan_weakening(
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    handle: arena::Handle<checked_trees::BorrowLoanFact>,
) -> Option<&checked_trees::FlowBorrowWeakeningFact> {
    let flow = state_flow(facts, machine, state)?;
    let mut weakenings = facts
        .flow
        .borrow_lifetimes
        .weakenings
        .span_or_empty(flow.borrow_weakenings)
        .iter()
        .filter(|weakening| weakening.loan == handle);
    let weakening = weakenings.next()?;
    weakenings.next().is_none().then_some(weakening)
}

pub fn release_statement(
    program: &dyn CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    handle: arena::Handle<checked_trees::BorrowLoanFact>,
) -> Option<u32> {
    let weakening = loan_weakening(facts, machine, state, handle)?;
    let checked_trees::FlowInvalidationSource::Statement { statement_index } = weakening.source
    else {
        return None;
    };
    let state = program.state_by_symbol(state)?;
    let order = program
        .statement_table
        .admitted_execution_order(state.statement_nodes);
    if !matches!(
        weakening.reason,
        checked_trees::FlowBorrowWeakeningReason::LastUseExpired
            | checked_trees::FlowBorrowWeakeningReason::StateExit
    ) || order.successor(facts.borrow.loans.get(handle).last_use_statement_index)
        != Some(statement_index)
    {
        return None;
    }
    u32::try_from(statement_index).ok()
}

pub fn release_boundary(
    program: &dyn CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    handle: arena::Handle<checked_trees::BorrowLoanFact>,
) -> Option<u32> {
    if let Some(boundary) = release_statement(program, facts, machine, state, handle) {
        return Some(boundary);
    }
    let state = program.state_by_symbol(state)?;
    let order = program
        .statement_table
        .admitted_execution_order(state.statement_nodes);
    let last = facts.borrow.loans.get(handle).last_use_statement_index;
    (order.successor(last) == Some(state.statement_nodes.len()))
        .then(|| u32::try_from(state.statement_nodes.len()).ok())
        .flatten()
}

fn state_flow(
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
) -> Option<&checked_trees::FlowStateFact> {
    facts.flow.control.unique_state(machine, state)
}
