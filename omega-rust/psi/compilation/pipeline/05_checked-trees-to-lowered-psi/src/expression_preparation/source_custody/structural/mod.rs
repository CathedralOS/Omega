//! Reconstruct construction, selected ownership and ordered dispatch from authored nodes.
//! Establishing a local place does not necessarily establish a new value origin:
//! whole affine moves preserve the original provenance. Replay traces the authored
//! initializer chain separately from checking each place's establishment and transfer.
//!
//! `validate` walks the retained value nodes as a worklist. Records, case
//! payloads and structural selections replay in `record`, `structural_case`
//! and `dispatch`, which queue their structural children back onto the same
//! worklist through `Replay`.

use crate::lowering_error::LoweringError;
use crate::lowering_error::unsupported;
use checked_trees::CheckedStructuralValueHandle;
use checked_trees::expression::{ExpressionHandle, ExpressionNode, MatchPattern};
use checked_trees::statement::StatementNode;
use checked_trees::types::{PrimitiveType, TypeReferenceHandle};
use checked_trees::{
    CheckedScalarComputationHandle, CheckedScalarDispatchPattern, CheckedScalarExpressionRole,
    CheckedStructuralValueKind, CheckedTrees, CheckedUnitEffectOperationPlan,
};
use symbols::SymbolHandle;

pub(crate) mod byte_literals;
mod dispatch;
mod owned_places;
mod record;
mod slice_view;
mod structural_case;
mod view_element_copy;
pub(crate) use owned_places::validate as validate_owned_place;
pub(crate) use owned_places::validate_transfer as validate_owned_transfer;
#[cfg(test)]
mod owned_case_tests;
mod owned_selection;
mod shared_borrow;
#[cfg(test)]
mod tests;

/// The arm of an enclosing structural selection a value sits under;
/// invalid outside every selection.
type SourceArm = arena::Handle<checked_trees::expression::TableMatchArm>;

/// Zero synthesis is justified by source omission, never by a zeroable type
/// alone. An omitted member retains its constructor occurrence for identity.
#[derive(Clone, Copy)]
enum ValueOccurrence {
    Authored(ExpressionHandle),
    ImplicitZero(ExpressionHandle),
}

impl ValueOccurrence {
    fn expression(self) -> ExpressionHandle {
        match self {
            Self::Authored(expression) | Self::ImplicitZero(expression) => expression,
        }
    }
}

/// One retained value node awaiting replay: the node, its authored
/// occurrence and destination type, the selection arm it sits under, and the
/// projected leaf a producing call feeds.
type PendingValue = (
    CheckedStructuralValueHandle,
    ValueOccurrence,
    TypeReferenceHandle,
    SourceArm,
    Option<ExpressionHandle>,
);

/// The construction a kind-specific replay extends: the state it belongs to
/// and the worklist and scalar operand roles the replay adds to.
struct Replay<'r> {
    machine: SymbolHandle,
    state: SymbolHandle,
    statement_index: u32,
    pending: &'r mut Vec<PendingValue>,
    operand_roles: &'r mut Vec<CheckedScalarExpressionRole>,
}

/// A whole call value is already established by its ordinary call operation.
/// Rejoin that producer to the exact registered expression before replaying
/// its authored target and operands; storage placement grants no source identity.
pub(crate) fn validate_call_value(
    checked: &CheckedTrees,
    machine: SymbolHandle,
    state: SymbolHandle,
    expression: ExpressionHandle,
    operation: &CheckedUnitEffectOperationPlan,
    parameters: &[checked_trees::CheckedUnitStructuralParameterPlan],
) -> Result<(), LoweringError> {
    let call = operation.call().ok_or(LoweringError::Unsupported(
        "structural value producer is not a call",
    ))?;
    let result = call.structural_result().ok_or(LoweringError::Unsupported(
        "structural value call has no result",
    ))?;
    let root = checked
        .facts
        .values
        .structural_values
        .root_for_expression(state, result.statement_index, expression)
        .ok_or(LoweringError::Unsupported(
            "structural call value lost its registered expression",
        ))?;
    let CheckedStructuralValueKind::Call { source_call } = checked
        .facts
        .values
        .structural_values
        .nodes
        .get(root.root)
        .kind
    else {
        return unsupported("structural call value changed its producer kind");
    };
    if root.machine != machine
        || checked
            .normalized_type_identity(root.type_reference)
            .into_string()
            != result.type_identity
        || !checked.facts.flow.control.calls.is_valid(source_call)
        || call.is_boundary()
    {
        return unsupported("structural call value changed its source or type");
    }
    let source_call = checked.facts.flow.control.calls.get(source_call);
    if source_call.authored_expression != expression
        || source_call.statement_index != call.coordinate.statement_index as usize
        || source_call.call_ordinal != call.coordinate.call_ordinal as usize
    {
        return unsupported("structural call value changed its source occurrence");
    }
    crate::emission::call_source_custody::validate_operation(
        checked, machine, state, operation, parameters,
    )
}

pub(crate) fn validate(
    checked: &CheckedTrees,
    machine: SymbolHandle,
    state: SymbolHandle,
    operation: &CheckedUnitEffectOperationPlan,
) -> Result<(), LoweringError> {
    let CheckedUnitEffectOperationPlan::EstablishStructuralValue {
        result,
        value,
        calls,
        operand_source: operand_position,
        discard_result_on_return,
    } = operation
    else {
        return unsupported("structural construction operation absent");
    };
    let (owner, source) =
        crate::expression_preparation::source_custody::authored_state(checked, state)?;
    let plans = &checked.facts.values.structural_values;
    if !plans.nodes.is_valid(*value) {
        return unsupported("structural construction has a stale value root");
    }
    let retained_expression = plans.nodes.get(*value).expression;
    let is_call_argument = matches!(
        operand_position,
        Some(checked_trees::CheckedArrayConstructionSource::CallArgument { .. })
    );
    // The operand coordinate owns an argument regardless of whether the
    // surrounding call is a statement, initializer, return or assignment.
    // Reconstruct its exact authored call and formal instead of treating the
    // enclosing destination as the copied argument's destination.
    let (expression, reference) = authored_destination(
        checked,
        machine,
        state,
        result.statement_index,
        retained_expression,
        operand_position,
    )?;
    // Absence and ambiguity are different: a duplicate key must not turn a
    // selected owner into a fresh construction with fabricated provenance.
    if checked
        .facts
        .flow
        .ownership
        .owned_selections
        .iter()
        .filter(|(_, receipt)| {
            receipt.state == state && receipt.statement_ordinal == result.statement_index
        })
        .count()
        > 1
    {
        return unsupported("structural value has ambiguous selected ownership receipts");
    }
    let root = plans
        .root_for_expression(state, result.statement_index, expression)
        .ok_or(LoweringError::Unsupported(
            "structural construction has no unique source root",
        ))?;
    if root.machine != machine
        || root.state != state
        || root.expression != expression
        || root.type_reference != reference
    {
        return unsupported("structural construction substituted its owner or result type");
    }
    // The established value is the root itself, or -- for a whole array
    // literal assigned element by element -- one of its elements under that
    // element's own authored occurrence and declared type.
    let element = root.root != *value;
    let (expression, reference) = if element {
        replaced_array_element(checked, source, result.statement_index, root, *value)
            .filter(|_| !is_call_argument)
            .ok_or(LoweringError::Unsupported(
                "structural construction substituted its value root",
            ))?
    } else {
        (expression, reference)
    };
    // A selection receipt belongs to the statement's destination, not to a
    // construction used as one of its call operands. The operand has already
    // rejoined its own authored expression and formal above.
    let selection = if is_call_argument {
        None
    } else {
        checked
            .facts
            .flow
            .ownership
            .owned_selection_at(state, result.statement_index)
            .map(|(_, receipt)| receipt)
    };
    // A selected owner transfers one whole value; no element roster names it.
    if element && selection.is_some() {
        return unsupported("array element establishment carries a selection receipt");
    }
    // A `&T` establishment keeps the authored reference type on its root but
    // joins the referent's structural carrier: the planned identity names the
    // record or the borrowed view's slice, not the borrow shell.
    let view_carrier = shared_slice_view_referent(checked, reference)
        .or_else(|| mutable_view_carrier(checked, reference));
    let carrier = shared_borrow_record_referent(checked, reference)
        .or(view_carrier)
        .unwrap_or(reference);
    // The root above retains the exact authored type. Structural storage has
    // a separate identity: qualifications govern source operations, not the
    // carrier's layout. Rejoin that identity through the same constraint-only
    // projection used for structural parameters, while checking custody below
    // against the qualified type. In particular, do not erase reference shells.
    let storage_carrier = crate::unit::attached_unit::structural_carrier_type(checked, carrier)?;
    // A `&mut`-referee establishment joins affine loan custody of the
    // referent's storage; its borrow shell still spells `Unrestricted`, so
    // the expected multiplicity follows the referee lane, not the shell's.
    let expected_multiplicity =
        if validation::reference_result_custody::mutable_borrowed_parts(&checked.typed, reference)
            .is_some()
        {
            language_semantics::Multiplicity::Affine
        } else {
            checked.type_multiplicity(reference)
        };
    if owner.symbol != machine
        || checked.normalized_type_identity(storage_carrier).as_str() != result.type_identity
        || expected_multiplicity != result.multiplicity
        || !(validation::has_plain_owned_contents_with_numeric_constraints(&checked.typed, carrier)
            // ZII construction proves its declared empty-byte predicates through
            // the zeroed carrier rule. The dedicated replay below still rejoins
            // the declaration; this grants no arbitrary byte constructor.
            || (matches!(plans.nodes.get(*value).kind,
                    CheckedStructuralValueKind::ZeroedStructural { .. })
                && validation::zeroed_structural_parts(&checked.typed, carrier))
            // Copying an established owned subtree retains its bounded byte
            // predicates. The worklist below independently reconstructs the
            // exact source path and type; constructors must still prove their
            // own establishment and do not receive this shape admission.
            || (matches!(plans.nodes.get(*value).kind,
                    CheckedStructuralValueKind::CopiedStructuralPlace { .. })
                && result.multiplicity == language_semantics::Multiplicity::Unrestricted
                && validation::has_plain_owned_contents_with_represented_constraints(
                    &checked.typed, carrier,
                ))
            || validation::has_cleanup_owned_contents(&checked.typed, carrier)
            // A linear record is an owned construction too: its disposal is
            // the binding's terminal obligation, exactly as stage 04's
            // record-value gate admits it.
            || validation::has_linear_owned_contents(&checked.typed, carrier)
            // A construction, like stage 04's constructed-value binding, may
            // carry scalar members under an exact-interval domain; each member's
            // range obligation proves it.
            || validation::has_structural_construction_contents(&checked.typed, carrier)
            || validation::reference_result_custody::is_reference_record(&checked.typed, reference)
            || validation::has_owned_or_shared_view_contents(&checked.typed, carrier)
            || view_carrier.is_some()
            // A `&[T]` view's carrier is the borrowed slice itself: a member
            // projection copies the stored view whole while the elements'
            // custody stays with the view's owner — there are no owned
            // contents for this clause to name.
            || matches!(
                checked
                    .type_reference_table
                    .type_reference(carrier),
                checked_trees::types::TypeReferenceNode::Slice { .. }
            ))
    {
        return unsupported("structural construction substituted its owner or result type");
    }
    if let Some(receipt) = selection {
        owned_selection::validate_receipt(
            checked,
            machine,
            state,
            result.statement_index,
            receipt,
        )?;
    }
    // Only the initializer establishes the declared local's loans and owned
    // provenance. A call argument in that initializer owns a separate value;
    // its consumer validates the transfer into the exact call formal.
    if !is_call_argument
        && let Some(StatementNode::LocalData(local)) = checked
            .statement_table
            .statements(source.statement_nodes)
            .get(result.statement_index as usize)
    {
        if validation::reference_result_custody::local_owes_record_loans(
            &checked.typed,
            local.type_reference,
        ) {
            validation::reference_result_custody::local_record_loans(
                &checked.typed,
                &checked.facts,
                machine,
                source,
                result.statement_index,
            )
            .ok_or(LoweringError::Unsupported(
                "reference record has no exact captured source loans",
            ))?;
        }
        validate_local_ownership(
            checked,
            machine,
            state,
            result.statement_index,
            local.symbol,
            local.type_reference,
            result.multiplicity,
            *discard_result_on_return,
        )?;
    }
    let mut pending = vec![(
        *value,
        if expression.is_valid() {
            ValueOccurrence::Authored(expression)
        } else {
            ValueOccurrence::ImplicitZero(expression)
        },
        reference,
        arena::Handle::invalid(),
        None,
    )];
    let mut consumed_calls = Vec::new();
    let mut visited = Vec::new();
    let mut operand_roles = Vec::new();
    let mut selected_leaves = Vec::new();
    while let Some((handle, occurrence, reference, source_arm, projected_leaf)) = pending.pop() {
        let expression = occurrence.expression();
        if !plans.nodes.is_valid(handle) || visited.contains(&handle) {
            return unsupported("structural construction has stale or reused value nodes");
        }
        visited.push(handle);
        let node = plans.nodes.get(handle);
        if node.expression != expression {
            return unsupported("structural construction exchanged authored value occurrences");
        }
        if matches!(occurrence, ValueOccurrence::ImplicitZero(_))
            != matches!(
                node.kind,
                CheckedStructuralValueKind::ZeroedScalarArray { .. }
                    | CheckedStructuralValueKind::ZeroedStructural { .. }
            )
        {
            return unsupported("structural zero synthesis changed its source omission");
        }
        if matches!(
            node.kind,
            CheckedStructuralValueKind::BorrowedSliceView { .. }
                | CheckedStructuralValueKind::MutableSliceView { .. }
        ) && let Some(recast) = validation::slice_recast_source(&checked.typed, expression)
        {
            let view = validation::slice_view_shell(&checked.typed, reference, &[]).ok_or(
                LoweringError::Unsupported("slice recast lost its declared view"),
            )?;
            if view.access != recast.access
                || checked.normalized_type_identity(view.element)
                    != checked.normalized_type_identity(recast.element)
            {
                return unsupported("slice recast changed its declared element or access");
            }
        }
        match node.kind.clone() {
            // A borrowed `&[T]` or `&mut [T]` view rejoins its loan exactly:
            // the retained argument names the lent collection root and path
            // under the view's access, and `EstablishElementView` or
            // `EstablishMutableElementView` carries the runtime extent
            // descriptor.
            CheckedStructuralValueKind::BorrowedSliceView {
                source: argument,
                region,
            } => {
                slice_view::validate(
                    checked,
                    owner,
                    source,
                    result.statement_index,
                    expression,
                    &argument,
                    crate::expression_preparation::bindings::LoanAccess::Shared,
                    region,
                )?;
                if matches!(
                    region,
                    checked_trees::CheckedElementViewRegion::RepresentationBytes { .. }
                ) {
                    operand_roles
                        .push(CheckedScalarExpressionRole::RepresentationViewOffset { expression });
                }
            }
            CheckedStructuralValueKind::MutableSliceView {
                source: argument,
                region,
            } => {
                slice_view::validate(
                    checked,
                    owner,
                    source,
                    result.statement_index,
                    expression,
                    &argument,
                    crate::expression_preparation::bindings::LoanAccess::Mutable,
                    region,
                )?;
                if matches!(
                    region,
                    checked_trees::CheckedElementViewRegion::RepresentationBytes { .. }
                ) {
                    operand_roles
                        .push(CheckedScalarExpressionRole::RepresentationViewOffset { expression });
                }
            }
            CheckedStructuralValueKind::Place(argument) => {
                if matches!(
                    argument.source,
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::ByteSequenceLiteral { .. }
                ) {
                    byte_literals::validate(checked, expression, reference, &argument)?;
                } else if let Some(receipt) = selection {
                    if projected_leaf.is_some() {
                        return unsupported(
                            "projected selection root place bypassed its leaf validation",
                        );
                    }
                    owned_selection::validate_leaf(
                        checked, receipt, expression, source_arm, &argument,
                    )?;
                    selected_leaves.push(expression);
                } else {
                    // Moving a whole owned value preserves its carrier, including
                    // a sum's active case. Ordinary source/type and exact affine
                    // transfer replay apply to constructor operands of any plain
                    // owned carrier; no case selection or reconstruction occurs.
                    owned_places::validate(
                        checked,
                        machine,
                        state,
                        result.statement_index,
                        expression,
                        reference,
                        &argument,
                    )?;
                }
            }
            CheckedStructuralValueKind::Call { source_call } => {
                if !checked.facts.flow.control.calls.is_valid(source_call) {
                    return unsupported("record operand call coordinate is stale");
                }
                let captured = checked.facts.flow.control.calls.get(source_call);
                let ExpressionNode::Call(authored) =
                    checked.expression_table.expression(expression)
                else {
                    return unsupported("structural call operand has no authored invocation");
                };
                let mut matching = calls.iter().filter(|call| call.value == handle);
                let retained = matching.next().ok_or(LoweringError::Unsupported(
                    "structural operand call plan missing",
                ))?;
                if matching.next().is_some() || consumed_calls.contains(&handle) {
                    return unsupported("structural operand call was retained more than once");
                }
                let Some((retained_call, returned)) = retained
                    .operation()
                    .call()
                    .filter(|call| {
                        !call.is_boundary() && call.discard_result_on_return() == Some(false)
                    })
                    .and_then(|call| Some((call, call.structural_result()?)))
                else {
                    return unsupported("structural operand is not an ordinary owned result call");
                };
                let coordinate = &retained_call.coordinate;
                let target_state = &retained_call.target.state;
                let structural_arguments = &retained_call.structural_arguments;
                // Under a selection a call arm may only contribute a fresh
                // product: an owned structural argument would move a source's
                // custody inside one arm without a transfer the receipt names.
                // The projection-root lane is exempt because its call supplies
                // the moved child's once-evaluated root, and its own argument
                // custody is admitted by the call's independent checks.
                if selection.is_some()
                    && projected_leaf.is_none()
                    && structural_arguments.iter().any(|argument| {
                        argument.access == checked_trees::CheckedStructuralAccess::Owned
                    })
                {
                    return unsupported("selected ownership mixes fresh and existing obligations");
                }
                let exact = crate::expression_preparation::source_custody::flow_calls::retain_exact_flow_call(
                    checked,
                    machine,
                    state,
                    *coordinate,
                    *target_state,
                )?;
                if !std::ptr::eq(exact, captured) {
                    return unsupported("structural operand call is outside its source state");
                }
                if captured.authored_expression != expression
                    || captured.target_symbol != authored.target_symbol
                    || *target_state != authored.target_symbol
                    || coordinate.statement_index != result.statement_index
                    || captured.statement_index != result.statement_index as usize
                    || coordinate.call_ordinal as usize != captured.call_ordinal
                    || returned.type_identity
                        != checked.normalized_type_identity(reference).as_str()
                    || returned.multiplicity != checked.type_multiplicity(reference)
                {
                    return unsupported(
                        "structural operand substituted its call or returned carrier",
                    );
                }
                let source_call = crate::emission::call_source_custody::authored::locate_source(
                    checked,
                    state,
                    *coordinate,
                )?;
                crate::emission::call_source_custody::validate_observations(
                    checked,
                    machine,
                    state,
                    *coordinate,
                    &source_call,
                )?;
                crate::unit::attached_unit::validate_operand_preparation(
                    checked,
                    machine,
                    state,
                    retained.operands(),
                    retained.operation(),
                )?;
                consumed_calls.push(handle);
            }
            CheckedStructuralValueKind::ScalarCasePlace {
                source: argument, ..
            } => {
                shared_borrow::validate_scalar_case_place(
                    checked,
                    owner,
                    source,
                    result.statement_index,
                    expression,
                    reference,
                    &argument,
                )?;
            }
            CheckedStructuralValueKind::ViewElementCopy { reads } => {
                view_element_copy::validate(checked, source, expression, reference, &reads)?;
            }
            CheckedStructuralValueKind::CopiedStructuralPlace {
                source: argument, ..
            } => {
                shared_borrow::validate_copied_place(
                    checked,
                    owner,
                    source,
                    result.statement_index,
                    expression,
                    reference,
                    &argument,
                )?;
            }
            CheckedStructuralValueKind::ZeroedScalarArray {
                element,
                element_count,
            } => {
                // A synthesized zero field has no authored value to replay —
                // its whole contract is the declared `[scalar; N]` carrier
                // and the planner's literal-zero element in the element's
                // own primitive.
                let expected = validation::unwrapped_type_reference(&checked.typed, reference)
                    .ok_or(LoweringError::Unsupported(
                        "zeroed scalar array carrier missing",
                    ))?;
                let checked_trees::types::TypeReferenceNode::FixedArray {
                    element_type,
                    length: checked_trees::types::FixedArrayLength::Literal(length),
                } = checked.type_reference_table.type_reference(expected)
                else {
                    return unsupported("zeroed scalar array substituted its carrier");
                };
                if u64::try_from(*length).ok() != Some(element_count) {
                    return unsupported("zeroed scalar array changed its leaf count");
                }
                let Some(primitive) = checked.primitive_type_reference(*element_type) else {
                    return unsupported("zeroed scalar array element is not a primitive");
                };
                let computation = checked.facts.values.scalar_computations.nodes.get(element);
                let checked_trees::CheckedScalarComputationKind::Value(
                    checked_trees::CheckedScalarExpression::IntegerLiteral { literal },
                ) = &computation.kind
                else {
                    return unsupported("zeroed scalar array element is not a literal zero");
                };
                if computation.primitive_type != primitive
                    || !literal.value_bignum().is_some_and(|value| value.is_zero())
                {
                    return unsupported("zeroed scalar array changed its zero leaf");
                }
            }
            CheckedStructuralValueKind::ScalarArray { elements } => {
                // The supplied array replays through the same scalar-array
                // producer the planner used: its exact declared carrier
                // flattens the authored literal to row-major leaves, and each
                // retained computation must rejoin its leaf at its ordinal.
                if !matches!(
                    checked.expression_table.expression(expression),
                    ExpressionNode::ArrayLiteral(_)
                ) {
                    return unsupported("scalar array lost its authored constructor");
                }
                let array = validation::scalar_array_elements(
                    &checked.typed,
                    machine,
                    expression,
                    reference,
                )
                .ok_or(LoweringError::Unsupported(
                    "scalar array substituted its closed primitive carrier",
                ))?;
                if !array.projections.is_empty() || array.elements.len() != elements.len() {
                    return unsupported("scalar array changed its leaf roster");
                }
                for (ordinal, ((leaf, primitive), handle)) in
                    array.elements.iter().zip(elements.iter()).enumerate()
                {
                    let role = CheckedScalarExpressionRole::StructuralArrayElement {
                        expression,
                        element_ordinal: u32::try_from(ordinal).map_err(|_| {
                            LoweringError::Unsupported("scalar array leaf ordinal overflow")
                        })?,
                    };
                    if validate_operand(
                        checked,
                        machine,
                        state,
                        result.statement_index,
                        role,
                        *handle,
                        *leaf,
                    )? != *primitive
                    {
                        return unsupported("scalar array leaf changed its carrier");
                    }
                    operand_roles.push(role);
                }
            }
            CheckedStructuralValueKind::ByteLiteralView { bytes } => {
                // Image-backed descriptors are values too. Rejoin the exact
                // authored bytes and declared correspondence; Terminal checks
                // the destination predicates from the actual source separately.
                if !matches!(checked.expression_table.expression(expression),
                    ExpressionNode::String(authored) if authored.as_ref() == bytes.as_slice())
                    || !validation::checked_argument_matches_type_reference(
                        &checked.typed,
                        expression,
                        reference,
                    )
                {
                    return unsupported("literal view changed its authored bytes or carrier");
                }
            }
            CheckedStructuralValueKind::ZeroedStructural { .. } => {
                // The synthesized zero's whole contract is the declared
                // member carrier itself being zero-constructible.
                let expected = validation::unwrapped_type_reference(&checked.typed, reference)
                    .ok_or(LoweringError::Unsupported(
                        "zeroed structural carrier missing",
                    ))?;
                if !validation::zeroed_structural_parts(&checked.typed, expected) {
                    return unsupported("zeroed structural substituted its carrier");
                }
            }
            CheckedStructuralValueKind::Reference { source: argument } => {
                if argument.access == checked_trees::CheckedStructuralAccess::SharedBorrow {
                    // A `&T` selection leaf replays its authored `&place`
                    // against the retained root/path instead of the
                    // owned-ingress map used by mutable reference carriers.
                    shared_borrow::validate(
                        checked,
                        owner,
                        source,
                        result.statement_index,
                        expression,
                        reference,
                        &argument,
                    )?;
                } else {
                    let expected = validation::reference_result_custody::initializer_source(
                        &checked.typed,
                        source,
                        expression,
                        reference,
                    )
                    .ok_or(LoweringError::Unsupported(
                        "reference field has no exact authored ingress",
                    ))?;
                    if argument != expected {
                        return unsupported("reference field substituted its ingress");
                    }
                }
            }
            CheckedStructuralValueKind::Record {
                data_symbol,
                fields,
            } => {
                record::validate(
                    checked,
                    &mut Replay {
                        machine,
                        state,
                        statement_index: result.statement_index,
                        pending: &mut pending,
                        operand_roles: &mut operand_roles,
                    },
                    expression,
                    reference,
                    source_arm,
                    data_symbol,
                    fields,
                )?;
            }
            CheckedStructuralValueKind::FixedArray { elements } => {
                let ExpressionNode::ArrayLiteral(literal) =
                    checked.expression_table.expression(expression)
                else {
                    return unsupported("array establishment lost its authored constructor");
                };
                let expected = validation::unwrapped_type_reference(&checked.typed, reference)
                    .ok_or(LoweringError::Unsupported("array carrier missing"))?;
                let checked_trees::types::TypeReferenceNode::FixedArray { element_type, .. } =
                    checked.type_reference_table.type_reference(expected)
                else {
                    return unsupported("array establishment substituted its carrier");
                };
                let authored = checked.expression_table.expression_handles(*literal);
                if authored.len() != elements.len() {
                    return unsupported("array establishment changed its element roster");
                }
                for (element, authored_element) in elements.iter().zip(authored.iter()) {
                    pending.push((
                        *element,
                        ValueOccurrence::Authored(*authored_element),
                        *element_type,
                        source_arm,
                        None,
                    ));
                }
            }
            CheckedStructuralValueKind::Case(construction) => {
                if construction.expression != expression
                    || checked.normalized_type_identity(construction.type_reference)
                        != checked.normalized_type_identity(reference)
                {
                    return unsupported(
                        "structural construction substituted its exact fresh case owner",
                    );
                }
                let fields =
                    crate::expression_preparation::source_custody::case_sources::construction(
                        checked,
                        &construction,
                    )?;
                for (ordinal, (expression, handle)) in fields.iter().enumerate() {
                    let role = CheckedScalarExpressionRole::StructuralValueField {
                        expression: construction.expression,
                        field_ordinal: u32::try_from(ordinal).map_err(|_| {
                            LoweringError::Unsupported("case field ordinal overflow")
                        })?,
                    };
                    operand_roles.push(role);
                    validate_operand(
                        checked,
                        machine,
                        state,
                        result.statement_index,
                        role,
                        *handle,
                        *expression,
                    )?;
                }
            }
            CheckedStructuralValueKind::StructuralCase {
                data_symbol,
                case,
                fields,
            } => {
                structural_case::validate(
                    checked,
                    &mut Replay {
                        machine,
                        state,
                        statement_index: result.statement_index,
                        pending: &mut pending,
                        operand_roles: &mut operand_roles,
                    },
                    expression,
                    reference,
                    source_arm,
                    data_symbol,
                    case,
                    fields,
                )?;
            }
            CheckedStructuralValueKind::Dispatch { subject, arms } => {
                dispatch::validate(
                    checked,
                    &mut Replay {
                        machine,
                        state,
                        statement_index: result.statement_index,
                        pending: &mut pending,
                        operand_roles: &mut operand_roles,
                    },
                    expression,
                    reference,
                    subject,
                    arms,
                )?;
            }
            CheckedStructuralValueKind::Projection {
                source: source_handle,
                path,
                type_identity,
            } => {
                // A projected leaf moves one affine child of an existing root.
                // The path, arm, and source evidence replay against the
                // transfer receipt; the root node itself is a whole place or
                // the producing call, validated under the root's own type.
                if !plans.nodes.is_valid(source_handle) || visited.contains(&source_handle) {
                    return unsupported("projected selection has a stale or reused root node");
                }
                let source_node = plans.nodes.get(source_handle).clone();
                if selection.is_none() {
                    // A projection under a copied shared-borrow root names a
                    // field of the copy's provenance — nothing moves, so the
                    // transfer catalog legitimately records no receipt.
                    if let CheckedStructuralValueKind::CopiedStructuralPlace {
                        source: copied,
                        ..
                    } = &source_node.kind
                    {
                        let (authored_machine, authored) =
                            crate::expression_preparation::source_custody::authored_state(
                                checked, state,
                            )?;
                        let root = owned_selection::validate_copied_projection(
                            checked,
                            authored_machine,
                            authored,
                            expression,
                            copied,
                            &path,
                            &type_identity,
                        )?;
                        // The projection's type/path agreement does not prove
                        // which same-typed root it observes. Replay the copied
                        // node normally to rejoin its exact authored occurrence,
                        // source place and unrestricted copy permission.
                        pending.push((
                            source_handle,
                            ValueOccurrence::Authored(root.expression),
                            root.reference,
                            source_arm,
                            None,
                        ));
                        continue;
                    }
                }
                let transfer = match selection {
                    Some(receipt) => owned_selection::ProjectionTransfer::Selection {
                        receipt,
                        source_arm,
                    },
                    None => owned_selection::ProjectionTransfer::Statement {
                        statement: result.statement_index,
                    },
                };
                let root = owned_selection::validate_projection(
                    checked,
                    owner,
                    source,
                    transfer,
                    expression,
                    &source_node,
                    &path,
                    &type_identity,
                )?;
                selected_leaves.push(expression);
                if matches!(source_node.kind, CheckedStructuralValueKind::Call { .. }) {
                    pending.push((
                        source_handle,
                        ValueOccurrence::Authored(root.expression),
                        root.reference,
                        source_arm,
                        Some(expression),
                    ));
                } else {
                    visited.push(source_handle);
                }
            }
        }
    }
    if let Some(receipt) = selection {
        let transfers = checked
            .facts
            .flow
            .ownership
            .selection_transfers
            .span(receipt.transfers)
            .ok_or(LoweringError::Unsupported(
                "selected ownership has a stale transfer span",
            ))?;
        if transfers.len() != selected_leaves.len()
            || transfers
                .iter()
                .any(|transfer| !selected_leaves.contains(&transfer.expression))
        {
            return unsupported(
                "selected ownership differs from the executable value alternatives",
            );
        }
    }
    if consumed_calls.len() != calls.len() {
        return unsupported("structural value has unaccounted call plans");
    }
    let transition = matches!(
        checked
            .statement_table
            .statements(source.statement_nodes)
            .get(result.statement_index as usize),
        Some(StatementNode::Transition(_))
    );
    let mut seen_roles = Vec::new();
    for (_, scalar_root) in checked
        .facts
        .values
        .scalar_computations
        .roots
        .iter()
        .filter(|(_, root)| {
            root.state == state
                && root.statement_ordinal == result.statement_index
                && matches!(
                    root.role,
                    CheckedScalarExpressionRole::StructuralValueSubject { .. }
                        | CheckedScalarExpressionRole::RecordField { .. }
                        | CheckedScalarExpressionRole::StructuralArrayElement { .. }
                        | CheckedScalarExpressionRole::StructuralValuePattern { .. }
                        | CheckedScalarExpressionRole::StructuralValueField { .. }
                        | CheckedScalarExpressionRole::RepresentationViewOffset { .. }
                )
        })
    {
        if scalar_root.machine != machine || seen_roles.contains(&scalar_root.role) {
            return unsupported("structural construction has duplicated or foreign scalar roots");
        }
        seen_roles.push(scalar_root.role);
        if operand_roles.contains(&scalar_root.role) || transition {
            continue;
        }
        if role_belongs_to_value(checked, expression, scalar_root.role)?
            || !sibling_owns_role(
                checked,
                machine,
                state,
                result.statement_index,
                expression,
                scalar_root.role,
            )?
        {
            return unsupported("structural construction acquired an unauthored scalar root");
        }
    }
    // A combined transition has two disjoint value scopes at one statement.
    // Reconstruct every retained operand against those authored scopes instead
    // of treating the sibling's roots as operands of this selected value.
    if matches!(
        checked
            .statement_table
            .statements(source.statement_nodes)
            .get(result.statement_index as usize),
        Some(StatementNode::Transition(_))
    ) {
        for (_, root) in checked
            .facts
            .values
            .scalar_computations
            .roots
            .iter()
            .filter(|(_, root)| {
                root.state == state
                    && root.statement_ordinal == result.statement_index
                    && matches!(
                        root.role,
                        CheckedScalarExpressionRole::StructuralValueSubject { .. }
                            | CheckedScalarExpressionRole::StructuralValuePattern { .. }
                            | CheckedScalarExpressionRole::StructuralValueField { .. }
                            | CheckedScalarExpressionRole::RepresentationViewOffset { .. }
                            | CheckedScalarExpressionRole::StructuralArrayElement { .. }
                            | CheckedScalarExpressionRole::RecordField { .. }
                    )
            })
        {
            let (expression, _) =
                operand_source(checked, state, result.statement_index, root.role)?;
            validate_operand(
                checked,
                machine,
                state,
                result.statement_index,
                root.role,
                root.root,
                expression,
            )?;
        }
    }
    Ok(())
}

/// The authored occurrence and declared element type of `value` when it is one
/// leaf of the whole array literal `root` an assignment stores: the statement
/// sequence establishes each leaf separately and replaces the array one
/// element window at a time. The leaf's position in the retained roster must
/// be its position in the authored literal.
fn replaced_array_element(
    checked: &CheckedTrees,
    source: &checked_trees::state::State,
    statement_index: u32,
    root: &checked_trees::CheckedStructuralValueRoot,
    value: CheckedStructuralValueHandle,
) -> Option<(ExpressionHandle, TypeReferenceHandle)> {
    let Some(StatementNode::Assignment(assignment)) = checked
        .statement_table
        .statements(source.statement_nodes)
        .get(statement_index as usize)
    else {
        return None;
    };
    if assignment.value != root.expression {
        return None;
    }
    let leaves = crate::unit::attached_unit::array_literal_leaves(
        checked,
        root.root,
        root.expression,
        root.type_reference,
    )?;
    let leaf = leaves.iter().find(|leaf| leaf.value == value)?;
    Some((leaf.expression, leaf.type_reference))
}

/// Rejoin the retained occurrence to its authored destination or exact call formal.
fn authored_destination(
    checked: &CheckedTrees,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement_index: u32,
    retained_expression: ExpressionHandle,
    operand_position: &Option<checked_trees::CheckedArrayConstructionSource>,
) -> Result<(ExpressionHandle, TypeReferenceHandle), LoweringError> {
    let (owner, source) =
        crate::expression_preparation::source_custody::authored_state(checked, state)?;
    Ok(
        if let Some(checked_trees::CheckedArrayConstructionSource::CallArgument {
            call_ordinal,
            parameter_position,
        }) = operand_position
        {
            let call = crate::emission::call_source_custody::authored::locate_source(
                checked,
                state,
                checked_trees::CheckedUnitCallCoordinate {
                    statement_index,
                    call_ordinal: *call_ordinal,
                },
            )?;
            let signature = crate::emission::call_source_custody::authored::target_signature(
                checked,
                machine,
                call.source_target,
            )?;
            let parameter = signature
                .parameters
                .get(*parameter_position as usize)
                .ok_or(LoweringError::Unsupported(
                    "structural construction lost its authored formal",
                ))?;
            let argument = call
                .structural_argument(checked, *parameter_position, parameter.is_self)
                .filter(|argument| *argument == retained_expression)
                .ok_or(LoweringError::Unsupported(
                    "structural construction lost its authored destination",
                ))?;
            (argument, parameter.type_reference)
        } else {
            match checked
                .statement_table
                .statements(source.statement_nodes)
                .get(statement_index as usize)
            {
                // A `let mut` declaration without an initializer keys its
                // zeroed establishment on the invalid handle; the destination
                // type is the local's own declared type either way.
                Some(StatementNode::LocalData(local)) => {
                    (local.initial_value, local.type_reference)
                }
                Some(StatementNode::Expression(expression)) => (*expression, source.return_type),
                // `place = <construction>` establishes the value that replaces a
                // structural field; its destination type is the authored place's own
                // declared type, reconstructed here rather than read from the root.
                // Peeling a policy here would certify a different value type even
                // when the underlying storage shape is identical. The one shell
                // assignment itself removes is a reference: writing through
                // `referent: &mut T` replaces the referent, a `T`.
                Some(StatementNode::Assignment(assignment)) => (
                    assignment.value,
                    validation::declared_place_type_raw(
                        &checked.typed,
                        owner,
                        Some(source),
                        assignment.target,
                    )
                    .map(|destination| {
                        validation::assignment_value_type(&checked.typed, destination)
                    })
                    .ok_or(LoweringError::Unsupported(
                        "structural construction lost its authored destination",
                    ))?,
                ),
                Some(StatementNode::Transition(transition))
                    if transition.exit == checked_trees::statement::TransitionExit::Ordinary =>
                {
                    let mut selected = [transition.target, transition.continuation]
                        .into_iter()
                        .filter(|target| target.is_valid())
                        .filter_map(|target| {
                            match checked.statement_table.transition_target(target) {
                                checked_trees::statement::TransitionTargetNode::Value(
                                    expression,
                                ) if *expression == retained_expression => {
                                    Some((*expression, source.return_type))
                                }
                                checked_trees::statement::TransitionTargetNode::Named {
                                    path,
                                    arguments,
                                    ..
                                } => {
                                    let argument_position = checked
                                        .statement_table
                                        .expression_handles(*arguments)
                                        .iter()
                                        .position(|expression| {
                                            *expression == retained_expression
                                        })?;
                                    let target_state = checked
                                        .typed
                                        .machine_states(owner)
                                        .iter()
                                        .find(|state| state.symbol == path.symbol)?;
                                    let parameter = checked
                                        .typed
                                        .state_parameters(target_state)
                                        .iter()
                                        .filter(|parameter| !parameter.is_self)
                                        .nth(argument_position)?;
                                    Some((retained_expression, parameter.type_reference))
                                }
                                _ => None,
                            }
                        });
                    let destination = selected.next().ok_or(LoweringError::Unsupported(
                        "structural return exchanged its authored destination",
                    ))?;
                    if selected.next().is_some() {
                        return unsupported("structural return has ambiguous source destinations");
                    }
                    destination
                }
                _ => return unsupported("structural construction lost its authored destination"),
            }
        },
    )
}

/// Candidate roots grant no authority: each other scope must rejoin an authored
/// destination or call formal before its own independent construction replay owns it.
fn sibling_owns_role(
    checked: &CheckedTrees,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement: u32,
    current: ExpressionHandle,
    role: CheckedScalarExpressionRole,
) -> Result<bool, LoweringError> {
    let plans = &checked.facts.values.structural_values;
    let (_, source) =
        crate::expression_preparation::source_custody::authored_state(checked, state)?;
    let authored_statement = checked
        .statement_table
        .statements(source.statement_nodes)
        .get(statement as usize);
    let control = &checked.facts.flow.control;
    let mut owners = 0;
    for (_, root) in plans.roots.iter().filter(|(_, root)| {
        root.state == state && root.statement_ordinal == statement && root.expression != current
    }) {
        if !role_belongs_to_value(checked, root.expression, role)? {
            continue;
        }
        if root.machine != machine
            || !plans.nodes.is_valid(root.root)
            || plans.nodes.get(root.root).expression != root.expression
            || plans
                .root_for_expression(state, statement, root.expression)
                .is_none()
        {
            return unsupported("structural sibling changed its exact retained root");
        }
        let mut destination = None;
        if !matches!(authored_statement, Some(StatementNode::Call(_))) {
            let outer =
                authored_destination(checked, machine, state, statement, root.expression, &None)?;
            if outer.0 == root.expression {
                destination = Some(outer.1);
            }
        }
        for (_, flow_state) in control.states.iter().filter(|(_, flow_state)| {
            flow_state.machine_symbol == machine && flow_state.state_symbol == state
        }) {
            let calls = control
                .calls
                .span(flow_state.calls)
                .ok_or(LoweringError::Unsupported(
                    "structural sibling has a stale call roster",
                ))?;
            for call in calls.iter().filter(|call| {
                call.statement_index == statement as usize && !control.is_retired(state, call)
            }) {
                // Semantic coordinates only nominate candidates. Unrelated scalar
                // calls need no structural-operand source reconstruction here.
                let is_candidate = if checked
                    .expression_table
                    .expression_is_valid(call.authored_expression)
                {
                    let ExpressionNode::Call(authored) = checked
                        .expression_table
                        .expression(call.authored_expression)
                    else {
                        continue;
                    };
                    authored.receiver == root.expression
                        || checked
                            .expression_table
                            .expression_handles(authored.arguments)
                            .contains(&root.expression)
                } else if let Some(StatementNode::Call(authored)) = authored_statement {
                    checked
                        .statement_table
                        .expression_handles(authored.arguments)
                        .contains(&root.expression)
                } else {
                    continue;
                };
                if !is_candidate {
                    continue;
                }
                let call_ordinal = u32::try_from(call.call_ordinal)
                    .map_err(|_| LoweringError::Unsupported("call ordinal overflow"))?;
                let authored = crate::emission::call_source_custody::authored::locate_source(
                    checked,
                    state,
                    checked_trees::CheckedUnitCallCoordinate {
                        statement_index: statement,
                        call_ordinal,
                    },
                )?;
                let signature = crate::emission::call_source_custody::authored::target_signature(
                    checked,
                    machine,
                    authored.source_target,
                )?;
                for (position, parameter) in signature.parameters.iter().enumerate() {
                    let position = u32::try_from(position)
                        .map_err(|_| LoweringError::Unsupported("argument position overflow"))?;
                    if authored.structural_argument(checked, position, parameter.is_self)
                        == Some(root.expression)
                        && destination.replace(parameter.type_reference).is_some()
                    {
                        return unsupported(
                            "structural sibling has ambiguous authored destinations",
                        );
                    }
                }
            }
        }
        if destination != Some(root.type_reference) {
            return unsupported("structural sibling lost its authored destination");
        }
        owners += 1;
    }
    Ok(owners == 1)
}

/// A role carries its constructor/match occurrence, including a synthesized
/// zero field whose scalar expression is absent. Only authored descendants of
/// another exactly rejoined scope may be delegated to that construction's replay.
/// A call's operand constructions have separate roots and own separate scopes.
fn role_belongs_to_value(
    checked: &CheckedTrees,
    expression: ExpressionHandle,
    role: CheckedScalarExpressionRole,
) -> Result<bool, LoweringError> {
    let mut pending = vec![expression];
    let mut visited = Vec::new();
    while let Some(expression) = pending.pop() {
        if !checked.expression_table.expression_is_valid(expression)
            || visited.contains(&expression)
        {
            return unsupported("structural argument sibling has stale or cyclic authored scope");
        }
        visited.push(expression);
        let node = checked.expression_table.expression(expression);
        match (role, node) {
            (
                CheckedScalarExpressionRole::RepresentationViewOffset { expression: owner },
                ExpressionNode::Cast(_) | ExpressionNode::Borrow(_),
            ) if owner == expression => return Ok(true),
            (
                CheckedScalarExpressionRole::RecordField {
                    expression: owner, ..
                }
                | CheckedScalarExpressionRole::StructuralValueField {
                    expression: owner, ..
                },
                ExpressionNode::StructLiteral(_),
            ) if owner == expression => return Ok(true),
            (
                CheckedScalarExpressionRole::StructuralArrayElement {
                    expression: owner, ..
                },
                ExpressionNode::ArrayLiteral(_),
            ) if owner == expression => return Ok(true),
            (
                CheckedScalarExpressionRole::StructuralValueSubject { expression: owner },
                ExpressionNode::Match(_),
            ) if owner == expression => return Ok(true),
            (
                CheckedScalarExpressionRole::StructuralValuePattern { source_arm },
                ExpressionNode::Match(dispatch),
            ) if source_arm.is_valid()
                && source_arm.generation() == dispatch.arms.start().generation()
                && source_arm
                    .arena_index()
                    .checked_sub(dispatch.arms.start().arena_index())
                    .is_some_and(|ordinal| {
                        (ordinal as usize)
                            < checked.expression_table.match_arms(dispatch.arms).len()
                    }) =>
            {
                return Ok(true);
            }
            _ => {}
        }
        match node {
            ExpressionNode::StructLiteral(literal) => pending.extend(
                checked
                    .expression_table
                    .struct_fields(literal.fields)
                    .iter()
                    .map(|field| field.value),
            ),
            ExpressionNode::Match(dispatch) => pending.extend(
                checked
                    .expression_table
                    .match_arms(dispatch.arms)
                    .iter()
                    .map(|arm| arm.value),
            ),
            ExpressionNode::ArrayLiteral(elements) => pending.extend(
                checked
                    .expression_table
                    .expression_handles(*elements)
                    .iter()
                    .copied(),
            ),
            _ => {}
        }
    }
    Ok(false)
}

fn validate_operand(
    checked: &CheckedTrees,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement: u32,
    role: CheckedScalarExpressionRole,
    handle: CheckedScalarComputationHandle,
    expression: ExpressionHandle,
) -> Result<PrimitiveType, LoweringError> {
    let plans = &checked.facts.values.scalar_computations;
    let mut roots = plans.roots.iter().map(|(_, root)| root).filter(|root| {
        root.state == state && root.statement_ordinal == statement && root.role == role
    });
    let root = roots.next().ok_or(LoweringError::Unsupported(
        "structural operand lost its computation root",
    ))?;
    if roots.next().is_some()
        || root.machine != machine
        || root.root != handle
        || !plans.nodes.is_valid(handle)
        || plans.nodes.get(handle).authored_root != expression
    {
        return unsupported("structural operand substituted its source computation");
    }
    crate::expression_preparation::source_custody::validate_computation_calls(
        checked, machine, state, statement, handle, expression,
    )?;
    crate::expression_preparation::source_custody::value_correspondence::validate(
        checked,
        state,
        statement,
        expression,
        plans.nodes.get(handle).primitive_type,
        &checked_trees::CheckedCallScalarArgument::Computation(handle),
    )?;
    Ok(plans.nodes.get(handle).primitive_type)
}

/// Retain each authored operand beside the type declared by its exact formal.
/// Receiver elision changes argument positions, never the destination type.
fn append_argument_scopes(
    checked: &CheckedTrees,
    machine: SymbolHandle,
    target: SymbolHandle,
    arguments: &[ExpressionHandle],
    receiver: ExpressionHandle,
    pending: &mut Vec<(ExpressionHandle, TypeReferenceHandle)>,
) -> Result<(), LoweringError> {
    let signature =
        crate::emission::call_source_custody::authored::target_signature(checked, machine, target)?;
    let nonself_count = signature
        .parameters
        .iter()
        .filter(|parameter| !parameter.is_self)
        .count();
    let explicit_self = arguments.len() > nonself_count;
    if arguments.len()
        != if explicit_self {
            signature.parameters.len()
        } else {
            nonself_count
        }
    {
        return unsupported("structural operand scope changed its authored argument count");
    }
    let mut explicit = arguments.iter();
    for parameter in signature.parameters {
        let expression = if parameter.is_self && !explicit_self {
            if !receiver.is_valid() {
                continue;
            }
            receiver
        } else {
            *explicit.next().ok_or(LoweringError::Unsupported(
                "structural operand argument missing",
            ))?
        };
        pending.push((expression, parameter.type_reference));
    }
    Ok(())
}

/// Locate semantic operands of actual constructors and matches beneath this
/// value, including erased initializers. Relevance controls runtime emission,
/// not whether retained source computations owe exact authored custody.
/// A retained role cannot authorize reading an unrelated expression in the arena.
pub(crate) fn operand_source(
    checked: &CheckedTrees,
    state: SymbolHandle,
    statement: u32,
    role: CheckedScalarExpressionRole,
) -> Result<(ExpressionHandle, PrimitiveType), LoweringError> {
    let (machine, source) =
        crate::expression_preparation::source_custody::authored_state(checked, state)?;
    let mut pending = Vec::new();
    match checked
        .statement_table
        .statements(source.statement_nodes)
        .get(statement as usize)
    {
        Some(StatementNode::LocalData(local)) => {
            pending.push((local.initial_value, local.type_reference));
        }
        Some(StatementNode::Assignment(assignment)) => pending.push(authored_destination(
            checked,
            machine.symbol,
            state,
            statement,
            assignment.value,
            &None,
        )?),
        Some(StatementNode::Expression(expression)) => {
            pending.push((*expression, source.return_type));
        }
        // A call statement's operands are its authored arguments; a
        // construction passed by value is evaluated there.
        Some(StatementNode::Call(call)) => append_argument_scopes(
            checked,
            machine.symbol,
            call.target_symbol,
            checked.statement_table.expression_handles(call.arguments),
            ExpressionHandle::invalid(),
            &mut pending,
        )?,
        Some(StatementNode::Transition(transition))
            if transition.exit == checked_trees::statement::TransitionExit::Ordinary =>
        {
            for target in [transition.target, transition.continuation]
                .into_iter()
                .filter(|target| target.is_valid())
            {
                match checked.statement_table.transition_target(target) {
                    checked_trees::statement::TransitionTargetNode::Value(expression) => {
                        pending.push((*expression, source.return_type));
                    }
                    checked_trees::statement::TransitionTargetNode::Named {
                        path,
                        arguments,
                        ..
                    } => append_argument_scopes(
                        checked,
                        machine.symbol,
                        path.symbol,
                        checked.statement_table.expression_handles(*arguments),
                        ExpressionHandle::invalid(),
                        &mut pending,
                    )?,
                    _ => {}
                }
            }
        }
        _ => return unsupported("structural operand has no authored value scope"),
    };
    let mut visited = Vec::new();
    while let Some((expression, declared)) = pending.pop() {
        if visited.contains(&expression) {
            return unsupported("structural operand has cyclic authored scope");
        }
        visited.push(expression);
        if !checked.expression_table.expression_is_valid(expression) {
            return unsupported("structural operand has stale authored scope");
        }
        if role == (CheckedScalarExpressionRole::RepresentationViewOffset { expression }) {
            let region =
                validation::slice_recast_byte_region(&checked.typed, machine, source, expression)
                    .ok_or(LoweringError::Unsupported(
                    "representation offset lost its authored recast",
                ))?;
            let primitive = validation::expression_result_type_reference(
                &checked.typed,
                machine,
                source,
                region.start,
            )
            .and_then(|reference| checked.primitive_type_reference(reference))
            .or_else(|| {
                matches!(
                    checked.expression_table.expression(region.start),
                    ExpressionNode::Integer(_)
                )
                .then_some(PrimitiveType::U64)
            })
            .ok_or(LoweringError::Unsupported(
                "representation offset lost its scalar carrier",
            ))?;
            return Ok((region.start, primitive));
        }
        if let CheckedScalarExpressionRole::StructuralArrayElement {
            expression: array,
            element_ordinal,
        } = role
            && array == expression
        {
            // The value's actual destination supplies its carrier whether it
            // is an argument, local, return or aggregate child. The array
            // owner alone reconstructs its ordered scalar leaf roster.
            let leaves = validation::scalar_array_elements(
                &checked.typed,
                machine.symbol,
                expression,
                declared,
            )
            .filter(|leaves| leaves.projections.is_empty())
            .ok_or(LoweringError::Unsupported(
                "scalar array operand lost its closed carrier",
            ))?;
            return leaves
                .elements
                .get(element_ordinal as usize)
                .copied()
                .ok_or(LoweringError::Unsupported(
                    "scalar array leaf ordinal escaped its array",
                ));
        }
        if let CheckedScalarExpressionRole::StructuralValueField {
            expression: constructor,
            field_ordinal,
        } = role
            && constructor == expression
        {
            // The constructor may carry structural payload siblings, so the
            // tolerant roster resolves first; the selected field still owes a
            // primitive carrier.
            let source =
                validation::structural_case_constructor(&checked.typed, expression, declared)
                    .ok_or(LoweringError::Unsupported(
                        "structural field lost its authored constructor",
                    ))?;
            return source
                .fields
                .get(field_ordinal as usize)
                .and_then(|(_, expression, declared)| {
                    checked
                        .primitive_type_reference(*declared)
                        .map(|primitive| (*expression, primitive))
                })
                .ok_or(LoweringError::Unsupported(
                    "structural field ordinal escaped its constructor",
                ));
        }
        if let CheckedScalarExpressionRole::RecordField {
            expression: owner,
            field_ordinal,
        } = role
            && owner == expression
            && let ExpressionNode::StructLiteral(literal) =
                checked.expression_table.expression(expression)
        {
            let field = checked
                .expression_table
                .struct_fields(literal.fields)
                .get(field_ordinal as usize)
                .ok_or(LoweringError::Unsupported("record operand ordinal missing"))?;
            let declaration = checked
                .data_definitions()
                .iter()
                .find(|data| data.symbol == literal.type_symbol)
                .and_then(|data| {
                    checked
                        .data_members(data)
                        .iter()
                        .find_map(|member| match member {
                            checked_trees::data::DataMember::Field(declaration)
                                if declaration.symbol == field.field_symbol =>
                            {
                                Some(declaration)
                            }
                            _ => None,
                        })
                })
                .ok_or(LoweringError::Unsupported(
                    "record operand declaration missing",
                ))?;
            return checked
                .primitive_type_reference(declaration.type_reference)
                .map(|primitive| (field.value, primitive))
                .ok_or(LoweringError::Unsupported(
                    "record operand has no scalar carrier",
                ));
        }
        // An initializer or return call evaluates constructors in its own
        // receiver/arguments. Reach only those authored children before
        // comparing the retained constructor and field identities.
        if let ExpressionNode::Call(call) = checked.expression_table.expression(expression) {
            append_argument_scopes(
                checked,
                machine.symbol,
                call.target_symbol,
                checked.expression_table.expression_handles(call.arguments),
                call.receiver,
                &mut pending,
            )?;
            continue;
        }
        // A nested record's scalar fields belong to that constructor, not the
        // enclosing field's ordinal. Reach them through authored field values
        // before comparing the retained owner/ordinal above on the next visit.
        if let ExpressionNode::StructLiteral(literal) =
            checked.expression_table.expression(expression)
        {
            let data = checked
                .data_definitions()
                .iter()
                .find(|data| data.symbol == literal.type_symbol)
                .ok_or(LoweringError::Unsupported(
                    "structural operand record declaration missing",
                ))?;
            for field in checked
                .expression_table
                .struct_fields(literal.fields)
                .iter()
                .rev()
            {
                let declaration = validation::exact_data_member_field(
                    &checked.typed,
                    data,
                    field.field_symbol,
                    field.name.as_str(),
                    literal.case_name.as_ref().map(|name| name.as_str()),
                )
                .ok_or(LoweringError::Unsupported(
                    "structural operand member declaration missing",
                ))?;
                pending.push((field.value, declaration.type_reference));
            }
            continue;
        }
        // Structural array children inherit the exact declared element type;
        // their own constructors retain their own operand identities.
        if let ExpressionNode::ArrayLiteral(elements) =
            checked.expression_table.expression(expression)
        {
            let reference = validation::unwrapped_type_reference(&checked.typed, declared).ok_or(
                LoweringError::Unsupported("structural array operand lost its type"),
            )?;
            let checked_trees::types::TypeReferenceNode::FixedArray { element_type, .. } =
                checked.type_reference_table.type_reference(reference)
            else {
                return unsupported("structural array operand changed its carrier");
            };
            pending.extend(
                checked
                    .expression_table
                    .expression_handles(*elements)
                    .iter()
                    .rev()
                    .map(|expression| (*expression, *element_type)),
            );
            continue;
        }
        let ExpressionNode::Match(dispatch) = checked.expression_table.expression(expression)
        else {
            continue;
        };
        let primitive = validation::expression_result_type_reference(
            &checked.typed,
            machine,
            source,
            dispatch.subject,
        )
        .and_then(|reference| validation::unwrapped_type_reference(&checked.typed, reference))
        .and_then(|reference| checked.primitive_type_reference(reference))
        .or_else(|| validation::match_subject_primitive_type(&checked.typed, dispatch));
        if role == (CheckedScalarExpressionRole::StructuralValueSubject { expression }) {
            return primitive
                .map(|primitive| (dispatch.subject, primitive))
                .ok_or(LoweringError::Unsupported(
                    "structural subject has no scalar carrier",
                ));
        }
        for (ordinal, arm) in checked
            .expression_table
            .match_arms(dispatch.arms)
            .iter()
            .enumerate()
        {
            let source_arm = arena::Handle::from_parts(
                dispatch
                    .arms
                    .start()
                    .arena_index()
                    .checked_add(u32::try_from(ordinal).map_err(|_| {
                        LoweringError::Unsupported("structural arm ordinal overflow")
                    })?)
                    .ok_or(LoweringError::Unsupported(
                        "structural arm identity overflow",
                    ))?,
                dispatch.arms.start().generation(),
            );
            if role == (CheckedScalarExpressionRole::StructuralValuePattern { source_arm })
                && let MatchPattern::Value(pattern) = arm.pattern
            {
                return primitive.map(|primitive| (pattern, primitive)).ok_or(
                    LoweringError::Unsupported("structural pattern has no scalar carrier"),
                );
            }
            pending.push((arm.value, declared));
        }
    }
    unsupported("structural operand role escaped its authored value scope")
}

/// Recover the value's original establishment, not the destination place's.
/// The operation/edge receivers independently check each transfer schedule.
pub(crate) fn local_value_provenance(
    checked: &CheckedTrees,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement: usize,
) -> Result<language_semantics::PermissionProvenance, LoweringError> {
    use language_semantics::{PermissionEventSource, PermissionProvenance};
    let (_, source) =
        crate::expression_preparation::source_custody::authored_state(checked, state)?;
    let statements = checked.statement_table.statements(source.statement_nodes);
    if let Some((_, receipt)) = checked.facts.flow.ownership.owned_selection_at(
        state,
        u32::try_from(statement)
            .map_err(|_| LoweringError::Unsupported("local origin ordinal overflow"))?,
    ) {
        owned_selection::validate_receipt(
            checked,
            machine,
            state,
            receipt.statement_ordinal,
            receipt,
        )?;
        return Ok(PermissionProvenance::Unknown);
    }
    let Some(StatementNode::LocalData(local)) = statements.get(statement) else {
        return unsupported("owned local origin has no declaration");
    };
    // A declared local with no initializer keys its establishment on the
    // statement itself: the unit plan minted the home there, so the
    // expression walk has nothing to read and the recorded events carry
    // the declaration's own provenance.
    if !checked
        .typed
        .expression_table
        .expression_is_valid(local.initial_value)
        && crate::unit::attached_unit::bodies::statement_establishes_local(
            crate::unit::attached_unit::bodies::UnitPlans::published(
                &checked.facts.flow.terminal_unit_effects,
            ),
            machine,
            state,
            statement,
            local.symbol,
        )
    {
        return Ok(PermissionProvenance::Established {
            machine_symbol: machine,
            state_symbol: state,
            source: PermissionEventSource::Statement {
                statement_index: statement,
            },
        });
    }
    // An ordinary machine returns freshly established custody. Its operand
    // origins belong to the call's input transfers; propagating them through
    // the result would disagree with the checker's claim-provenance owner.
    // Whole local moves and aggregate construction below still preserve their
    // source value's provenance.
    if let ExpressionNode::Call(call) = checked.expression_table.expression(local.initial_value)
        && crate::expression_preparation::source_custody::authored_state(
            checked,
            call.target_symbol,
        )
        .is_ok()
    {
        return Ok(PermissionProvenance::Established {
            machine_symbol: machine,
            state_symbol: state,
            source: PermissionEventSource::Statement {
                statement_index: statement,
            },
        });
    }
    let origin = validation::expression_permission_provenance(
        &checked.typed,
        local.initial_value,
        &mut |expression| {
            // A stored `&`/`&mut` child borrows its target place rather than
            // minting fresh storage: checking resolves the borrow through the
            // canonical place of its target, so the loan's conserved origin
            // is the referee's provenance. Unwrap the shells the same way so
            // a record literal carrying `&mut owner` keys its establishment
            // on `owner`'s, exactly as the recorded event does.
            let mut operand = expression;
            while let ExpressionNode::Borrow(borrow) = checked.expression_table.expression(operand)
            {
                operand = borrow.target;
            }
            let ExpressionNode::Name(name) = checked.expression_table.expression(operand) else {
                return Ok(None);
            };
            if !name.symbol.is_valid() {
                return Err("owned local origin has an invalid source");
            }
            // Names of namespaces and constructors do not denote live storage.
            // Operand admission separately verifies their authored identity.
            if !matches!(
                checked.symbols.get(name.symbol).kind,
                symbols::SymbolKind::Local | symbols::SymbolKind::Parameter
            ) {
                return Ok(None);
            }
            if name.head_symbol != name.symbol || name.members.count() != 1 {
                // A borrow unwrapped above names a member place: the checker
                // keys the loan on that tracked path, which is not a whole
                // owned source here either. Only a bare member-path
                // initializer -- an actual whole-value move -- is an error.
                return if operand == expression && expression == local.initial_value {
                    Err("owned local origin is not a whole source")
                } else {
                    Ok(None)
                };
            }
            if let Some(parameter) = checked
                .state_parameters(source)
                .iter()
                .find(|parameter| parameter.symbol == name.symbol)
            {
                if checked.type_multiplicity(parameter.type_reference)
                    == language_semantics::Multiplicity::Unrestricted
                {
                    return Ok(None);
                }
                return Ok(Some(PermissionProvenance::Established {
                    machine_symbol: machine,
                    state_symbol: state,
                    source: PermissionEventSource::StateEntry,
                }));
            }
            let mut origins = statements
                .iter()
                .enumerate()
                .filter_map(|(ordinal, source)| match source {
                    StatementNode::LocalData(origin) if origin.symbol == name.symbol => {
                        Some((ordinal, origin))
                    }
                    _ => None,
                });
            let Some((ordinal, origin)) = origins.next() else {
                return Err("owned local origin has no source local");
            };
            if origins.next().is_some()
                || ordinal >= statement
                || (operand == expression
                    && expression == local.initial_value
                    && checked.normalized_type_identity(origin.type_reference)
                        != checked.normalized_type_identity(local.type_reference))
            {
                return Err("owned local origin changed its prior source declaration");
            }
            if checked.type_multiplicity(origin.type_reference)
                == language_semantics::Multiplicity::Unrestricted
            {
                return Ok(None);
            }
            // Local dependencies strictly precede their use, including constructor children.
            local_value_provenance(checked, machine, state, ordinal)
                .map(Some)
                .map_err(|error| match error {
                    LoweringError::Unsupported(reason) => reason,
                    _ => "owned local origin lost its prior provenance",
                })
        },
    )
    .map_err(LoweringError::Unsupported)?;
    Ok(origin.unwrap_or(PermissionProvenance::Established {
        machine_symbol: machine,
        state_symbol: state,
        source: PermissionEventSource::Statement {
            statement_index: statement,
        },
    }))
}

pub(crate) fn validate_local_ownership(
    checked: &CheckedTrees,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement: u32,
    symbol: SymbolHandle,
    type_reference: TypeReferenceHandle,
    multiplicity: language_semantics::Multiplicity,
    discard_result_on_return: bool,
) -> Result<(), LoweringError> {
    use language_semantics::{Multiplicity, PermissionEventKind, PermissionEventSource};
    let (_, source) =
        crate::expression_preparation::source_custody::authored_state(checked, state)?;
    if !symbol.is_valid()
        || checked
            .state_parameters(source)
            .iter()
            .any(|parameter| parameter.symbol == symbol)
        || checked.state_local_declaration(source, symbol).is_none()
    {
        return unsupported("structural local has ambiguous source identity");
    }
    let establishment_source = PermissionEventSource::Statement {
        statement_index: statement as usize,
    };
    let provenance = if multiplicity == Multiplicity::Affine {
        local_value_provenance(checked, machine, state, statement as usize)?
    } else {
        language_semantics::PermissionProvenance::Unknown
    };
    let ownership = &checked.facts.flow.ownership;
    let selected_destination = ownership.owned_selection_at(state, statement).is_some();
    let mut selected_uses = 0;
    for (_, receipt) in ownership
        .owned_selections
        .iter()
        .filter(|(_, receipt)| receipt.state == state)
    {
        if ownership
            .selection_sources
            .span(receipt.sources)
            .is_none_or(|sources| sources.iter().any(|source| source.symbol == symbol))
        {
            owned_selection::validate_receipt(
                checked,
                machine,
                state,
                receipt.statement_ordinal,
                receipt,
            )?;
            selected_uses += 1;
        }
    }
    let mut establishments = 0;
    let mut drops = 0;
    let mut transfers = 0;
    let mut loan_events: Vec<(
        arena::Handle<checked_trees::BorrowLoanFact>,
        language_semantics::PermissionEventKind,
    )> = Vec::new();
    for (_, event) in ownership.permissions.iter().filter(|(_, event)| {
        event.machine_symbol == machine
            && event.state_symbol == state
            && event.root == facts::PlaceRoot::Symbol(symbol)
    }) {
        // Borrow loans share the local's event stream: `&a.first` records a
        // Shared Establish/Consume pair on `a` for the loan's whole range.
        // They transfer no ownership, so they cannot join the owned roster —
        // each must rejoin its exact recorded loan instead.
        if event.access != language_semantics::PermissionAccess::Owned {
            if !replays_loan_event(checked, machine, state, symbol, event, &mut loan_events) {
                return unsupported(
                    "structural local carried a borrow event with no recorded loan",
                );
            }
            continue;
        }
        if event.multiplicity != multiplicity
            || multiplicity != Multiplicity::Affine
            || event.claim_identity != language_semantics::PermissionClaimIdentity::Unknown
            || event.provenance
                != if selected_destination {
                    language_semantics::PermissionProvenance::Unknown
                } else {
                    provenance
                }
            || event.obligation_live
        {
            return unsupported("structural local changed its whole no-code ownership provenance");
        }
        let segments =
            ownership
                .segments
                .span(event.segments)
                .ok_or(LoweringError::Unsupported(
                    "structural local has a stale ownership path",
                ))?;
        let (_, selected_type) =
            validation::structural_owned_path(&checked.typed, type_reference, segments)
                .map_err(LoweringError::Unsupported)?;
        if checked.type_multiplicity(selected_type) != Multiplicity::Affine {
            return unsupported("structural local event changed its selected affine custody");
        }
        match event.kind {
            PermissionEventKind::Establish
                if segments.is_empty() && event.source == establishment_source =>
            {
                establishments += 1
            }
            PermissionEventKind::AffineDrop
                if segments.is_empty() && event.source == PermissionEventSource::StateExit =>
            {
                drops += 1
            }
            PermissionEventKind::Transfer => {
                // A subtree transfer does not retire its containing local.
                // Its consumer rejoins the exact typed path and permission;
                // emitted frontier replay disposes only the residual complement.
                transfers += usize::from(segments.is_empty());
            }
            _ => return unsupported("structural local changed its ownership event"),
        }
    }
    // A `&mut`-referee local's custody is its loan, not the owned axis: it
    // establishes no owned receipt and owes no owned final disposition — the
    // event loop above already replayed (and rejected any stray owned
    // events on) the borrow axis its lifecycle lives in.
    if validation::reference_result_custody::mutable_borrowed_parts(&checked.typed, type_reference)
        .is_some()
    {
        return Ok(());
    }
    if selected_uses > 1
        || establishments
            != usize::from(multiplicity == Multiplicity::Affine && !selected_destination)
        || drops > 1
        || (multiplicity == Multiplicity::Affine
            && drops == 0
            && transfers == 0
            && selected_uses == 0)
        || (selected_uses != 0 && (drops != 0 || transfers != 0))
        || (multiplicity != Multiplicity::Unrestricted
            && discard_result_on_return
            && (drops != 1 || transfers != 0))
    {
        return unsupported("structural local lost its exact establishment or final disposition");
    }
    Ok(())
}

/// Rejoin one borrow-axis permission event to its exact recorded loan. The
/// formation statement, access kind and borrowed path must all agree, each
/// loan supplies at most one `Establish`/`Consume` pair, and the event's
/// provenance must name the loan's own formation statement rather than a
/// substituted origin.
fn replays_loan_event(
    checked: &CheckedTrees,
    machine: SymbolHandle,
    state: SymbolHandle,
    root: SymbolHandle,
    event: &checked_trees::FlowPermissionEventFact,
    matched: &mut Vec<(
        arena::Handle<checked_trees::BorrowLoanFact>,
        language_semantics::PermissionEventKind,
    )>,
) -> bool {
    use language_semantics::{
        Multiplicity, PermissionAccess, PermissionClaimIdentity, PermissionEventKind,
        PermissionEventSource, PermissionProvenance,
    };
    if event.obligation_live {
        return false;
    }
    let borrow = &checked.facts.borrow;
    let Some(borrow_state) = borrow
        .states
        .iter()
        .map(|(_, fact)| fact)
        .find(|fact| fact.machine_symbol == machine && fact.state_symbol == state)
    else {
        return false;
    };
    let event_segments = checked.facts.flow.ownership.segments.span(event.segments);
    let expected_provenance =
        |loan: &checked_trees::BorrowLoanFact| PermissionProvenance::Established {
            machine_symbol: machine,
            state_symbol: state,
            source: PermissionEventSource::Statement {
                statement_index: loan.statement_index,
            },
        };
    let matched_loan = borrow.loans.iter().find(|(handle, loan)| {
        if matched.contains(&(*handle, event.kind))
            || !borrow.state_owns_loan(borrow_state, *handle)
            || loan.root_symbol != root
            || Some(borrow.loan_segments(loan)) != event_segments
            || event.provenance != expected_provenance(loan)
            || !matches!(
                event.claim_identity,
                PermissionClaimIdentity::Established {
                    machine_symbol: claim_machine,
                    state_symbol: claim_state,
                    source: PermissionEventSource::Statement { statement_index },
                    ..
                } if claim_machine == machine
                    && claim_state == state
                    && statement_index == loan.statement_index
            )
        {
            return false;
        }
        matches!(
            (event.access, event.multiplicity, &loan.kind),
            (
                PermissionAccess::Shared,
                Multiplicity::Unrestricted,
                checked_trees::BorrowAccessKind::Read
            ) | (
                PermissionAccess::Exclusive,
                Multiplicity::Affine,
                checked_trees::BorrowAccessKind::Mutable
                    | checked_trees::BorrowAccessKind::WriteOnly,
            )
        ) && match event.kind {
            PermissionEventKind::Establish => {
                event.source
                    == PermissionEventSource::Statement {
                        statement_index: loan.statement_index,
                    }
            }
            // A loan's Consume fires at its weakening boundary: `StateExit`
            // when the borrow's last use ends the state, otherwise the
            // statement after that last use -- the same boundary the borrow
            // lifetime weakening records in `aliases` replay.
            PermissionEventKind::Consume => {
                let Some(boundary) = loan.last_use_statement_index.checked_add(1) else {
                    return false;
                };
                let Ok((_, authored)) =
                    crate::expression_preparation::source_custody::authored_state(checked, state)
                else {
                    return false;
                };
                let expected = if boundary
                    == checked
                        .statement_table
                        .statements(authored.statement_nodes)
                        .len()
                {
                    PermissionEventSource::StateExit
                } else {
                    PermissionEventSource::Statement {
                        statement_index: boundary,
                    }
                };
                event.source == expected
            }
            _ => false,
        }
    });
    let Some((handle, _)) = matched_loan else {
        return false;
    };
    matched.push((handle, event.kind));
    true
}

/// The referent of a shared-borrow `&T` result/local type: `T` must be a named
/// record or a primitive, independently of whether a record contains loans.
/// A shared borrow observes the referent instead of moving its contents;
/// contained loans and linear claims stay with the referent's owner and never
/// become owned obligations of this join. A
/// primitive referent has no data declaration to resolve; its structural
/// carrier is the `PrimitiveScalar` shape the type catalog already registers,
/// and the borrow still denotes the original storage rather than a snapshot.
/// `None` for owned types, `&mut` carriers (which keep their own
/// `reference_result_custody` lane), and sums. Shared by source replay and
/// attached-unit registration, so it lives here rather than in a machine
/// producer.
pub(crate) fn shared_borrow_record_referent(
    checked: &CheckedTrees,
    reference: checked_trees::types::TypeReferenceHandle,
) -> Option<checked_trees::types::TypeReferenceHandle> {
    let checked_trees::types::TypeReferenceNode::Reference {
        referee, access, ..
    } = checked.type_reference_table.type_reference(reference)
    else {
        return None;
    };
    if *access != language_semantics::ReferenceAccess::Shared {
        return None;
    }
    if checked.primitive_type_reference(*referee).is_some() {
        return validation::has_linear_owned_contents(&checked.typed, *referee).then_some(*referee);
    }
    // `&[T]` names the slice view itself as the borrowed record: element custody
    // belongs to the view, so there are no plain owned contents to require.
    if let checked_trees::types::TypeReferenceNode::Slice { .. } =
        checked.type_reference_table.type_reference(*referee)
    {
        return Some(*referee);
    }
    let checked_trees::types::TypeReferenceNode::Named { symbol, .. } =
        checked.type_reference_table.type_reference(*referee)
    else {
        return None;
    };
    let record = checked
        .data_definitions()
        .iter()
        .find(|record| record.symbol == *symbol)?;
    if checked
        .data_members(record)
        .iter()
        .any(|member| matches!(member, checked_trees::data::DataMember::Variant(_)))
    {
        return None;
    }
    Some(*referee)
}

/// The carrier a mutable slice binding joins. Both ByteSequence and ElementView
/// catalogs identify the slice beneath the reference shell; access and affine
/// descriptor debt are independently retained on its result. Named and primitive
/// referees keep their reference shell, so this is not generic dereferencing.
pub(crate) fn mutable_view_carrier(
    checked: &CheckedTrees,
    reference: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    validation::slice_view_shell(&checked.typed, reference, &[])
        .filter(|view| view.access == language_semantics::ReferenceAccess::Mutable)
        .map(|view| view.slice)
}

/// The carrier of an actual shared slice view, including byte slices. This
/// queries source access independently of whether Terminal uses ByteSequence
/// or ElementView storage; mutable and write-only references cannot lend it.
pub(crate) fn shared_slice_view_referent(
    checked: &CheckedTrees,
    reference: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    validation::slice_view_shell(&checked.typed, reference, &[])
        .filter(|view| view.access == language_semantics::ReferenceAccess::Shared)
        .map(|view| view.slice)
}
