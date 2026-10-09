//! Ordered value-pattern compatibility and coverage. Destination checks still
//! visit every result arm, including arms whose runtime execution is excluded.
//! Result loans and owned transfers are established by their mandatory borrow
//! and multiplicity passes. A second type-only source-shape gate cannot decide
//! that custody: it rejected ordinary selected constructors before those passes
//! could join their exact leaves. Pattern effects still keep their own admission.

use super::expression_result_type_reference as declared_value_type;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::expression::{
    ExpressionHandle, ExpressionNode, MatchPattern, TableMatchExpression,
};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

/// Find a retained subject carrier or an already-typed value-pattern peer.
/// Anonymous patterns never choose a default width.
pub fn match_subject_primitive_type(
    program: &dyn crate::CheckingInput,
    dispatch: &TableMatchExpression,
) -> Option<PrimitiveType> {
    retained_primitive(program, dispatch.subject).or_else(|| {
        program
            .expression_table
            .match_arms(dispatch.arms)
            .iter()
            .find_map(|arm| {
                if let MatchPattern::Value(value) = arm.pattern {
                    retained_primitive(program, value)
                } else {
                    None
                }
            })
    })
}

fn retained_primitive(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<PrimitiveType> {
    let reference = match program.expression_table.expression(expression) {
        ExpressionNode::Boolean(_) => return Some(PrimitiveType::Bool),
        ExpressionNode::Integer(_) => {
            crate::declarations::operators::landed_integer_literal_type_reference(
                program, expression,
            )?
        }
        ExpressionNode::Name(path) => super::named_value_type_reference(program, path)?,
        ExpressionNode::Call(call) => {
            crate::machine_calls::calls::resolved_call_result_type(program, call).or_else(|| {
                typed_trees::operator::resolve_named_expression_call(program, call)
                    .map(|operator| operator.return_type)
            })?
        }
        ExpressionNode::Cast(cast) => cast.target_type,
        ExpressionNode::Match(dispatch) => {
            return program
                .expression_table
                .match_arms(dispatch.arms)
                .iter()
                .find_map(|arm| retained_primitive(program, arm.value));
        }
        // Projections need their actual machine/state scope. The computation
        // owner has that scope and performs its ordinary operand query; do
        // not select a coincidentally compatible field from another machine.
        _ => return None,
    };
    crate::value_custody::places::unwrapped_type_reference(program, reference)
        .and_then(|reference| program.primitive_type_reference(reference))
}

/// Check scalar pattern compatibility, coverage, and every result arm before
/// an early evaluator erases the dispatch. The caller supplies a valid acyclic
/// expression graph; this check does not evaluate landed arm operations.
pub fn validate_match_dispatch(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    dispatch: &TableMatchExpression,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let arms = program.expression_table.match_arms(dispatch.arms);
    let case_dispatch = match_case_dispatch(program, machine, state, dispatch);
    let mut wildcard = false;
    let mut boolean_values = [false; 2];
    let subject_type = declared_value_type(program, machine, state, dispatch.subject);
    let subject_class = super::value_classification::value_class(
        program,
        Some(machine),
        Some(state),
        dispatch.subject,
    );
    let subject_boolean = subject_type
        .and_then(|reference| {
            crate::value_custody::places::unwrapped_type_reference(program, reference)
        })
        .and_then(|reference| program.primitive_type_reference(reference))
        == Some(PrimitiveType::Bool)
        || subject_class == Some(super::ValueClass::Boolean)
        || match_subject_primitive_type(program, dispatch) == Some(PrimitiveType::Bool);
    for arm in arms {
        match arm.pattern {
            MatchPattern::Wildcard => wildcard = true,
            MatchPattern::Value(pattern) => {
                if case_dispatch.is_some() {
                    continue;
                }
                if matches!(
                    program.expression_table.expression(pattern),
                    ExpressionNode::ArrayLiteral(_)
                        | ExpressionNode::StructLiteral(_)
                        | ExpressionNode::Range(_)
                        | ExpressionNode::Borrow(_)
                ) {
                    diagnostics.push(Diagnostic::error(
                        "match structural, range, and binding patterns are not supported by value dispatch yet",
                    ).with_source_span(arm.source_span));
                    continue;
                }
                if let Some(value) = program.expression_table.boolean_node(pattern) {
                    boolean_values[usize::from(*value)] = true;
                }
                if !is_scalar_value(program, machine, state, dispatch.subject)
                    || !is_scalar_value(program, machine, state, pattern)
                {
                    diagnostics.push(Diagnostic::error(
                        "match value patterns require scalar subjects and scalar pattern values; structural, domain, and case patterns are not supported yet",
                    ).with_source_span(arm.source_span));
                    continue;
                }
                check_compatible_values(
                    program,
                    machine,
                    state,
                    dispatch.subject,
                    pattern,
                    "match pattern is incompatible with its subject",
                    diagnostics,
                );
            }
        }
    }
    if !wildcard && !(subject_boolean && boolean_values == [true, true]) {
        diagnostics.push(Diagnostic::error(
            "match does not cover its subject; add a wildcard arm or both Boolean value alternatives",
        ).with_source_span(program.expression_table.source_span(expression)));
    }
    if arms.is_empty() {
        return;
    }
    // A declared peer supplies a destination for anonymous arms. If every arm
    // remains anonymous, its enclosing storage/call/result supplies that
    // destination instead; selecting a fixed integer width here would move the
    // language's literal landing boundary.
    let peer = arms
        .iter()
        .find(|arm| declared_value_type(program, machine, state, arm.value).is_some())
        .unwrap_or(&arms[0])
        .value;
    if let Some(destination) = declared_value_type(program, machine, state, peer) {
        for arm in arms {
            crate::proof_contracts::arithmetic_domains::validate_anonymous_integer_range(
                program,
                destination,
                arm.value,
                "typed match result",
                diagnostics,
            );
        }
    }
    for arm in arms {
        if selected_expression_transfers_owned(program, machine, state, arm.value)
            || matches!(arm.pattern, MatchPattern::Value(pattern)
                if selected_expression_transfers_owned(program, machine, state, pattern))
        {
            diagnostics.push(Diagnostic::error(
                "match selected pattern or arm transfers owned input custody; branch-local transfer and cleanup joins are not supported yet",
            ).with_source_span(arm.source_span));
        }
        check_compatible_values(
            program,
            machine,
            state,
            peer,
            arm.value,
            "match arms produce incompatible values",
            diagnostics,
        );
    }
}

/// Whether `value` is an exact projected place rooted at a reference carrier
/// (`self.inventory` under `&mut self`, `x.slot` through `x: &Inventory`).
/// Moving through such a place opens a borrowed-storage window the
/// multiplicity checker owns -- the join receives a whole owned value on
/// every agreeing edge while the hole stays with the caller's storage -- so
/// the transfer is branch-local evidence, not the owned input custody or
/// predecessor result custody this gate exists to reject. A bare reference
/// root (no projection) moves the borrow itself and keeps every rejection.
fn expression_moves_borrowed_place(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> bool {
    let mut expression = expression;
    let mut projected = false;
    loop {
        let next = match program.expression_table.expression(expression) {
            ExpressionNode::Member(member) => member.receiver,
            ExpressionNode::Indexed(indexed) => indexed.collection,
            _ => break,
        };
        projected = true;
        expression = next;
    }
    projected
        && declared_value_type(program, machine, state, expression).is_some_and(|reference| {
            matches!(
                program.type_reference_table.type_reference(reference),
                typed_trees::types::TypeReferenceNode::Reference { .. }
            )
        })
}

fn is_scalar_value(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> bool {
    if let Some(reference) = declared_value_type(program, machine, state, expression) {
        return crate::value_custody::places::unwrapped_type_reference(program, reference)
            .and_then(|reference| program.primitive_type_reference(reference))
            .is_some();
    }
    if let Some(class) =
        super::value_classification::value_class(program, Some(machine), Some(state), expression)
    {
        return matches!(
            class,
            super::ValueClass::Numeric | super::ValueClass::Boolean
        );
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Match(dispatch) => {
            let arms = program.expression_table.match_arms(dispatch.arms);
            !arms.is_empty()
                && arms
                    .iter()
                    .all(|arm| is_scalar_value(program, machine, state, arm.value))
        }
        ExpressionNode::Binary(_) => {
            crate::has_builtin_bound_expression_meaning(program, machine, Some(state), expression)
        }
        _ => false,
    }
}

fn selected_expression_transfers_owned(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    root: ExpressionHandle,
) -> bool {
    let requires_transfer = |reference: TypeReferenceHandle| {
        reference.is_valid()
            && program.type_multiplicity(reference)
                != language_semantics::Multiplicity::Unrestricted
    };
    let mut pending = vec![root];
    let mut visited = Vec::new();
    while let Some(expression) = pending.pop() {
        if !expression.is_valid() || visited.contains(&expression) {
            continue;
        }
        visited.push(expression);
        let node = program.expression_table.expression(expression);
        match node {
            ExpressionNode::Call(call) => {
                let parameters = crate::machine_calls::calls::machine_state_by_symbol(
                    program,
                    call.target_symbol,
                )
                .map(|(_, state)| program.state_parameters(state))
                .or_else(|| {
                    typed_trees::operator::resolve_named_expression_call(program, call)
                        .map(|operator| program.operator_parameters(operator))
                });
                let arguments = program.expression_table.expression_handles(call.arguments);
                if let Some(parameters) = parameters {
                    let supplied = parameters.iter().collect::<Vec<_>>();
                    // An argument rooted at a reference carrier moves borrowed
                    // storage, not the caller's owned custody -- the window
                    // checker and its arm-agreement rule own that hole. Any
                    // other transfer-requiring parameter still selects the
                    // plain rejection.
                    if supplied.len() == arguments.len()
                        && supplied
                            .iter()
                            .zip(arguments.iter())
                            .any(|(parameter, argument)| {
                                requires_transfer(parameter.type_reference)
                                    && !expression_moves_borrowed_place(
                                        program, machine, state, *argument,
                                    )
                            })
                    {
                        return true;
                    }
                    if supplied.len() != arguments.len()
                        && parameters
                            .iter()
                            .any(|parameter| requires_transfer(parameter.type_reference))
                    {
                        return true;
                    }
                }
            }
            ExpressionNode::Binary(binary) => {
                if [binary.left, binary.right].iter().any(|operand| {
                    declared_value_type(program, machine, state, *operand)
                        .is_some_and(requires_transfer)
                        && !expression_moves_borrowed_place(program, machine, state, *operand)
                }) {
                    return true;
                }
            }
            ExpressionNode::Unary(unary)
                if declared_value_type(program, machine, state, unary.operand)
                    .is_some_and(requires_transfer)
                    && !expression_moves_borrowed_place(program, machine, state, unary.operand) =>
            {
                return true;
            }
            _ => {}
        }
        crate::expression_traversal::children(program, node, |child| pending.push(child));
    }
    false
}

/// A discriminant dispatch over a scalar-payload sum subject: every arm is
/// either a wildcard or a case name classifier (`UnitResult::Ok`,
/// `UnitResult::Error`) whose owner is the subject's declared sum type. A case
/// classifier tests the tag only, like `x in UnitResult::Error`; its payload,
/// when one exists, is never bound or evaluated. Value dispatch lowers the
/// shape to ordered case-membership selections.
#[derive(Debug)]
pub struct MatchCaseDispatch {
    /// The checked expression carrying the subject's discriminant.
    pub subject: MatchCaseSubject,
    /// Ordered arms. `None` marks the wildcard fallback.
    pub arms: Vec<MatchCaseDispatchArm>,
}

/// One arm of a [`MatchCaseDispatch`].
#[derive(Debug)]
pub struct MatchCaseDispatchArm {
    /// The admitted case symbol, or `None` for the wildcard fallback arm.
    pub case: Option<SymbolHandle>,
    /// The arm's scalar result expression.
    pub value: ExpressionHandle,
}

/// The observable shape carrying a discriminant dispatch's subject.
#[derive(Debug)]
pub enum MatchCaseSubject {
    /// A self field, non-self parameter, or a field/fixed-index path into one.
    ParameterField,
    /// An immutable local holding the matched sum.
    ImmutableLocal {
        /// The local's declared symbol.
        symbol: SymbolHandle,
        /// Its declared type reference.
        type_reference: TypeReferenceHandle,
    },
    /// A fresh payloadless case construction matched whole.
    Constructor,
}

/// Classify `dispatch` as a discriminant dispatch over a scalar-payload sum
/// subject, or `None` when any requirement fails:
///
/// - every arm is a wildcard or a name-form pattern resolving to a case of
///   the subject's declared sum type;
/// - every arm value is a scalar (discriminant dispatch emits scalar
///   selections; structural results stay unsupported);
/// - the subject resolves to a place the pipeline can read a discriminant
///   from: a parameter field path, an immutable local, or a fresh case
///   construction.
///
/// Admission here is shared with checked-tree construction: a `None` result
/// means value dispatch's scalar gates still apply.
pub fn match_case_dispatch(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    dispatch: &TableMatchExpression,
) -> Option<MatchCaseDispatch> {
    let arms = program.expression_table.match_arms(dispatch.arms);
    if arms.is_empty() {
        return None;
    }
    let mut pattern_arms = 0usize;
    let mut dispatch_arms = Vec::with_capacity(arms.len());
    for arm in arms {
        match arm.pattern {
            MatchPattern::Wildcard => {
                dispatch_arms.push(MatchCaseDispatchArm {
                    case: None,
                    value: arm.value,
                });
            }
            MatchPattern::Value(pattern) => {
                let (owner_reference, selected) = case_pattern(program, pattern)?;
                let subject_reference =
                    declared_value_type(program, machine, state, dispatch.subject)
                        .map(|reference| {
                            crate::value_custody::places::assignment_value_type(program, reference)
                        })
                        .filter(|reference| reference.is_valid())?;
                if program.normalized_type_identity(subject_reference)
                    != program.normalized_type_identity(owner_reference)
                {
                    return None;
                }
                pattern_arms += 1;
                dispatch_arms.push(MatchCaseDispatchArm {
                    case: Some(selected),
                    value: arm.value,
                });
            }
        }
    }
    if pattern_arms == 0 {
        return None;
    }
    if arms
        .iter()
        .any(|arm| !is_scalar_value(program, machine, state, arm.value))
    {
        return None;
    }
    let subject = case_dispatch_subject(program, machine, state, dispatch.subject)?;
    Some(MatchCaseDispatch {
        subject,
        arms: dispatch_arms,
    })
}

/// Resolve a name-form match pattern to its owner type reference and case
/// symbol, or `None`. The owner's admission walls mirror
/// `scalar_case_constructor`: a closed, non-generic pure sum whose contents
/// stay within plain owned numeric-constraint shape.
fn case_pattern(
    program: &dyn crate::CheckingInput,
    pattern: ExpressionHandle,
) -> Option<(TypeReferenceHandle, SymbolHandle)> {
    // A bare case name and a fieldless case literal both spell the same
    // classifier: only the tag is tested, exactly like `in` membership.
    let (owner_symbol, selected) = match program.expression_table.expression(pattern) {
        ExpressionNode::Name(name) => (program.symbols.get(name.symbol).parent, name.symbol),
        ExpressionNode::StructLiteral(literal) => {
            if !program
                .expression_table
                .struct_fields(literal.fields)
                .is_empty()
            {
                return None;
            }
            (literal.type_symbol, literal.case_symbol?)
        }
        _ => return None,
    };
    let owner =
        crate::declarations::data_queries::data_definition_by_symbol(program, owner_symbol)?;
    let reference = program
        .type_reference_table
        .find_named_type_reference(owner.symbol)?;
    if !owner.type_parameters.is_empty()
        || !crate::has_contents(
            program,
            reference,
            crate::value_custody::storage_contents::ContentsRequirement::NumericOwned,
        )
        || program
            .data_members(owner)
            .iter()
            .any(|member| matches!(member, typed_trees::data::DataMember::Field(_)))
    {
        return None;
    }
    let case = program
        .data_members(owner)
        .iter()
        .find_map(|member| match member {
            typed_trees::data::DataMember::Variant(case) if case.symbol == selected => {
                Some(case.symbol)
            }
            _ => None,
        })?;
    Some((reference, case))
}

/// Resolve the place a discriminant dispatch reads its subject's case from.
/// Parameter-field paths reuse the transition-guard admission family; locals
/// reuse the case-membership place plan; fresh constructions carry their own
/// discriminant.
fn case_dispatch_subject(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    subject: ExpressionHandle,
) -> Option<MatchCaseSubject> {
    if parameter_case_subject(program, machine, state, subject).is_some() {
        return Some(MatchCaseSubject::ParameterField);
    }
    if let Some(path) = program.expression_table.name_path_node(subject)
        && path.head_symbol == path.symbol
        && program
            .expression_table
            .name_path_members(path.members)
            .len()
            == 1
        && let Some((_, local)) = program.state_local_declaration(state, path.symbol)
        && !local.is_mutable
        && local.type_reference.is_valid()
        && crate::has_contents(
            program,
            local.type_reference,
            crate::value_custody::storage_contents::ContentsRequirement::NumericOwned,
        )
    {
        return Some(MatchCaseSubject::ImmutableLocal {
            symbol: local.symbol,
            type_reference: local.type_reference,
        });
    }
    if crate::value_custody::scalar_case_constructor::scalar_case_constructor(program, subject)
        .is_some()
    {
        return Some(MatchCaseSubject::Constructor);
    }
    None
}

/// Admit a subject rooted at a parameter place: a bare parameter name, a
/// self-field projection of the enclosing machine's attached data, or a
/// field/fixed-index path into either. Member segments must name declared
/// fields or payload cases; array indexing requires retained builtin meaning
/// and a constant index.
fn parameter_case_subject(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    subject: ExpressionHandle,
) -> Option<()> {
    match program.expression_table.expression(subject) {
        ExpressionNode::Name(path) => {
            if program
                .state_parameters(state)
                .iter()
                .any(|parameter| parameter.symbol == path.symbol && path.head_symbol == path.symbol)
            {
                return Some(());
            }
            None
        }
        ExpressionNode::Member(member) => {
            // The binder already resolved the member: a case payload member
            // carries `case_variant`; a field member carries its field symbol.
            if member.case_variant.is_none() && !member.member_symbol.is_valid() {
                return None;
            }
            if matches!(
                program.expression_table.expression(member.receiver),
                ExpressionNode::Name(receiver) if receiver.symbol == machine.symbol
            ) {
                // A self-receiver member must be a real field of the
                // machine's attached data so a case path never skips the
                // field's own walls.
                crate::value_custody::places::exact_self_field(program, machine, subject)?;
                return Some(());
            }
            parameter_case_subject(program, machine, state, member.receiver)
        }
        ExpressionNode::Indexed(indexed) => {
            if !crate::value_custody::places::has_retained_builtin_index_meaning(program, subject) {
                return None;
            }
            let index = program
                .expression_table
                .constant_integer_value(indexed.index)?;
            if index < 0 {
                return None;
            }
            parameter_case_subject(program, machine, state, indexed.collection)
        }
        _ => None,
    }
}

fn check_compatible_values(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    peer: ExpressionHandle,
    value: ExpressionHandle,
    message: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(dispatch) = program.expression_table.match_node(value) {
        for arm in program.expression_table.match_arms(dispatch.arms) {
            check_compatible_values(
                program,
                machine,
                state,
                peer,
                arm.value,
                message,
                diagnostics,
            );
        }
        return;
    }
    if let Some(dispatch) = program.expression_table.match_node(peer) {
        for arm in program.expression_table.match_arms(dispatch.arms) {
            check_compatible_values(
                program,
                machine,
                state,
                arm.value,
                value,
                message,
                diagnostics,
            );
        }
        return;
    }
    let data_symbol = |expression| match program.expression_table.expression(expression) {
        ExpressionNode::StructLiteral(literal) if literal.type_symbol.is_valid() => {
            Some(literal.type_symbol)
        }
        _ => None,
    };
    let peer_data = data_symbol(peer);
    let value_data = data_symbol(value);
    // Comparing a reference-backed scalar observes its referent. Result
    // custody is checked separately and never established by this read view.
    let peer_type = declared_value_type(program, machine, state, peer)
        .map(|reference| crate::value_custody::places::assignment_value_type(program, reference));
    let value_type = declared_value_type(program, machine, state, value)
        .map(|reference| crate::value_custody::places::assignment_value_type(program, reference));
    let mismatch = if let (Some(peer_type), Some(value_type)) = (peer_type, value_type) {
        // Numeric range predicates may weaken at a join, but semantic policy
        // may not. The result query retains only predicates shared by every
        // arm; accepting these operands does not export either arm's range.
        program.normalized_type_identity(peer_type) != program.normalized_type_identity(value_type)
            && !matches!(
                (super::result_type::arithmetic_carrier(program, peer_type),
                 super::result_type::arithmetic_carrier(program, value_type)),
                (Some(peer), Some(value)) if peer == value
            )
    } else if let Some(reference) = peer_type.or(value_type) {
        let anonymous = if peer_type.is_some() { value } else { peer };
        crate::value_custody::literals::validate_suffix_landing(
            program,
            anonymous,
            reference,
            diagnostics,
        );
        !super::argument_matches_type_reference_handle(program, anonymous, reference)
    } else {
        let peer_class =
            super::value_classification::value_class(program, Some(machine), Some(state), peer);
        let value_class =
            super::value_classification::value_class(program, Some(machine), Some(state), value);
        let class_mismatch = peer_class
            .zip(value_class)
            .is_some_and(|(peer, value)| peer != value);
        let data_mismatch = match (peer_data, value_data) {
            (Some(peer), Some(value)) => peer != value,
            (Some(_), None) => {
                value_class.is_some()
                    || matches!(
                        program.expression_table.expression(value),
                        ExpressionNode::ArrayLiteral(_)
                    )
            }
            (None, Some(_)) => {
                peer_class.is_some()
                    || matches!(
                        program.expression_table.expression(peer),
                        ExpressionNode::ArrayLiteral(_)
                    )
            }
            (None, None) => false,
        };
        class_mismatch || data_mismatch
    };
    if mismatch {
        diagnostics.push(
            Diagnostic::error(message)
                .with_source_span(program.expression_table.source_span(value)),
        );
    }
}
