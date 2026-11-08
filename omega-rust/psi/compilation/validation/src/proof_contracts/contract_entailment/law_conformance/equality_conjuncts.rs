//! Read complete equality statements in their declaring parameter telescope.
//!
//! A sealed Id and a selected Boolean comparison remain distinct relations.
//! Only the existing same-carrier builtin scalar bridge can make the latter
//! available as identity. These views describe obligations, never their proof.

use symbols::SymbolHandle;
use typed_trees::expression::{BinaryOperator, ExpressionHandle};
use typed_trees::signature::StateParameter;
use typed_trees::type_identity::{TypeIdentityRequest, normalized_expression_identity};

use crate::mathematical_applications::{IdentityApplication, identity_application_at};

#[derive(Clone, Copy)]
pub(crate) struct EqualityConjunct<'scope> {
    owner: SymbolHandle,
    parameters: &'scope [StateParameter],
    expression: ExpressionHandle,
    meaning: EqualityMeaning,
}

#[derive(Clone, Copy)]
enum EqualityMeaning {
    Identity(IdentityApplication),
    TypeEquality,
    BooleanPredicate,
}

impl<'scope> EqualityConjunct<'scope> {
    pub(crate) fn expression(self) -> ExpressionHandle {
        self.expression
    }

    fn scope(self) -> (SymbolHandle, &'scope [StateParameter]) {
        (self.owner, self.parameters)
    }

    pub(crate) fn identity(self) -> Option<IdentityApplication> {
        let EqualityMeaning::Identity(identity) = self.meaning else {
            return None;
        };
        Some(identity)
    }
}

pub(crate) fn equality_conjunct<'scope>(
    program: &dyn crate::CheckingInput,
    owner: SymbolHandle,
    parameters: &'scope [StateParameter],
    expression: ExpressionHandle,
) -> Option<EqualityConjunct<'scope>> {
    if super::super::structural_terms::is_case_observation(program, expression) {
        return None;
    }
    let meaning = if let Ok(identity) = identity_application_at(program, expression) {
        EqualityMeaning::Identity(identity)
    } else if let Some(binary) = program.expression_table.binary_node(expression) {
        if binary.operator != BinaryOperator::Equal {
            return None;
        }
        if program.static_type_predicate_operands(expression).is_some() {
            EqualityMeaning::TypeEquality
        } else {
            super::super::structural_terms::builtin_identity_operand_references(
                program, owner, parameters, expression,
            )
            .map_or(EqualityMeaning::BooleanPredicate, |references| {
                EqualityMeaning::Identity(IdentityApplication {
                    carrier: references[0],
                    subjects: [binary.left, binary.right],
                })
            })
        }
    } else {
        // Preparation binds selected comparisons to ordinary calls. Their
        // declared Bool result retains the complete predicate: a namespace,
        // value receiver, arguments and static/evidence selections keep their
        // original roles. No comparison spelling or argument layout is needed,
        // and no Boolean call supplies value identity.
        program.expression_table.call_node(expression)?;
        let reference = crate::parameter_expression_result_type_reference(
            program, owner, parameters, expression,
        )?;
        if program.primitive_type_reference(reference)
            != Some(typed_trees::types::PrimitiveType::Bool)
        {
            return None;
        }
        EqualityMeaning::BooleanPredicate
    };
    Some(EqualityConjunct {
        owner,
        parameters,
        expression,
        meaning,
    })
}

/// One declaration reuses graph scratch across all its clause candidates.
/// Epochs make a tiny law visit cost proportional to its reached expressions,
/// rather than clearing the unrelated expression-arena prefix per comparison.
#[derive(Default)]
pub(crate) struct SourceCorrespondence {
    visits: Vec<SourceVisit>,
    epoch: u32,
}

#[derive(Clone, Copy, Default)]
struct SourceVisit {
    epoch: u32,
    state: u8,
    expanded_work: usize,
}

const MAX_SOURCE_MATCH_WORK: usize = 4096;

impl SourceCorrespondence {
    /// Match statements under complete positional binders. Identity may reverse
    /// endpoints; a selected comparison remains that exact Boolean expression.
    /// The canonical declaration identity retains calls, carriers and static
    /// operands, and supplies no proof that distinct expressions are equivalent.
    pub(crate) fn matching_conjuncts(
        &mut self,
        program: &dyn crate::CheckingInput,
        required: EqualityConjunct<'_>,
        required_binders: &[(SymbolHandle, String)],
        provided: EqualityConjunct<'_>,
        provided_binders: &[(SymbolHandle, String)],
    ) -> bool {
        match (required.meaning, provided.meaning) {
            (
                EqualityMeaning::Identity(required_identity),
                EqualityMeaning::Identity(provided_identity),
            ) => {
                let carrier = |reference, binders| {
                    program.type_identity(TypeIdentityRequest {
                        binders,
                        ..TypeIdentityRequest::ordinary(reference)
                    })
                };
                if carrier(required_identity.carrier, required_binders)
                    != carrier(provided_identity.carrier, provided_binders)
                {
                    return false;
                }
                (self.matching_values(
                    program,
                    required.scope(),
                    required_identity.subjects[0],
                    required_binders,
                    provided.scope(),
                    provided_identity.subjects[0],
                    provided_binders,
                ) && self.matching_values(
                    program,
                    required.scope(),
                    required_identity.subjects[1],
                    required_binders,
                    provided.scope(),
                    provided_identity.subjects[1],
                    provided_binders,
                )) || (self.matching_values(
                    program,
                    required.scope(),
                    required_identity.subjects[0],
                    required_binders,
                    provided.scope(),
                    provided_identity.subjects[1],
                    provided_binders,
                ) && self.matching_values(
                    program,
                    required.scope(),
                    required_identity.subjects[1],
                    required_binders,
                    provided.scope(),
                    provided_identity.subjects[0],
                    provided_binders,
                ))
            }
            (EqualityMeaning::TypeEquality, EqualityMeaning::TypeEquality)
            | (EqualityMeaning::BooleanPredicate, EqualityMeaning::BooleanPredicate) => self
                .matching_values(
                    program,
                    required.scope(),
                    required.expression,
                    required_binders,
                    provided.scope(),
                    provided.expression,
                    provided_binders,
                ),
            _ => false,
        }
    }

    /// Source structure has a canonical key only after every infix operation
    /// retains builtin meaning in this declaration. Selected calls retain their
    /// resolved target and full static application in the shared key. A raw
    /// selected infix instead uses its checked structural application below.
    fn canonical_source_value(
        &mut self,
        program: &dyn crate::CheckingInput,
        statement: (SymbolHandle, &[StateParameter]),
        expression: ExpressionHandle,
        binders: &[(SymbolHandle, String)],
    ) -> Option<String> {
        if self.epoch == u32::MAX {
            self.visits.fill(SourceVisit::default());
            self.epoch = 0;
        }
        self.epoch += 1;
        let mut pending = vec![(expression, false)];
        let mut remaining_graph_work = MAX_SOURCE_MATCH_WORK;
        while let Some((expression, completed)) = pending.pop() {
            remaining_graph_work = remaining_graph_work.checked_sub(1)?;
            if !expression.is_valid() {
                continue; // An absent receiver is a structural call edge.
            }
            if !program.expression_table.expression_is_valid(expression) {
                return None;
            }
            let position = expression.arena_index() as usize;
            if self.visits.len() <= position {
                self.visits.resize(position + 1, SourceVisit::default());
            }
            if completed {
                let mut expanded_work = 1usize;
                crate::queries::expression_traversal::children(
                    program,
                    program.expression_table.expression(expression),
                    |child| {
                        if child.is_valid() {
                            expanded_work = expanded_work.saturating_add(
                                self.visits[child.arena_index() as usize].expanded_work,
                            );
                        }
                    },
                );
                if expanded_work > MAX_SOURCE_MATCH_WORK {
                    return None;
                }
                self.visits[position] = SourceVisit {
                    epoch: self.epoch,
                    state: 2,
                    expanded_work,
                };
                continue;
            }
            let visit = &mut self.visits[position];
            match (visit.epoch == self.epoch, visit.state) {
                (true, 1) => return None,
                (true, 2) => continue,
                _ => {
                    *visit = SourceVisit {
                        epoch: self.epoch,
                        state: 1,
                        expanded_work: 0,
                    }
                }
            }
            pending.push((expression, true));
            let node = program.expression_table.expression(expression);
            if let Some(binary) = program.expression_table.binary_node(expression)
                && program.static_type_predicate_operands(expression).is_none()
                && let Some(spelling) =
                    crate::proof_contracts::parameter_expression_meaning::spelled_binary_operator(
                        binary.operator,
                    )
            {
                let operands = [binary.left, binary.right].map(|operand| {
                    crate::parameter_expression_result_type_reference(
                        program,
                        statement.0,
                        statement.1,
                        operand,
                    )
                });
                if !typed_trees::operator::has_builtin_spelled_expression_meaning(
                    program,
                    statement.0,
                    expression,
                    spelling,
                    &operands,
                ) {
                    return None;
                }
            }
            crate::queries::expression_traversal::children(program, node, |child| {
                pending.push((child, false))
            });
        }
        if !crate::proof_contracts::contract_results::expression_has_source_builtin_index_meaning(
            program, expression,
        ) {
            return None;
        }
        normalized_expression_identity(program.typed_program(), expression, binders)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn matching_values(
        &mut self,
        program: &dyn crate::CheckingInput,
        required: (SymbolHandle, &[StateParameter]),
        required_expression: ExpressionHandle,
        required_binders: &[(SymbolHandle, String)],
        provided: (SymbolHandle, &[StateParameter]),
        provided_expression: ExpressionHandle,
        provided_binders: &[(SymbolHandle, String)],
    ) -> bool {
        let required_key =
            self.canonical_source_value(program, required, required_expression, required_binders);
        let provided_key =
            self.canonical_source_value(program, provided, provided_expression, provided_binders);
        if !self.matching_operation_payloads(
            program,
            required,
            required_expression,
            required_binders,
            provided,
            provided_expression,
            provided_binders,
        ) {
            return false;
        }
        if let (Some(required), Some(provided)) = (required_key, provided_key) {
            return required == provided;
        }
        let term = |statement: (SymbolHandle, &[StateParameter]), expression| {
            super::super::structural_term(
                program,
                None,
                Some((statement.0, statement.1)),
                expression,
                None,
            )
        };
        let (Some(required_term), Some(provided_term)) = (
            term(required, required_expression),
            term(provided, provided_expression),
        ) else {
            return false;
        };
        let (Some(required_roles), Some(provided_roles)) = (
            parameter_roles(required.1, required_binders),
            parameter_roles(provided.1, provided_binders),
        ) else {
            return false;
        };
        if required_roles.len() != provided_roles.len() {
            return false;
        }
        let mut bindings = Vec::with_capacity(required_roles.len());
        for ((required_key, required_parameter), (provided_key, provided_parameter)) in
            required_roles.into_iter().zip(provided_roles)
        {
            if required_key != provided_key {
                return false;
            }
            bindings.push((
                required_parameter.name.as_str().to_owned(),
                super::super::StructuralTerm::Variable(provided_parameter.name.as_str().to_owned()),
            ));
        }
        let variables = required
            .1
            .iter()
            .map(|parameter| parameter.name.as_str().to_owned())
            .collect::<Vec<_>>();
        super::slot_bindings::diagnostic_shape_match(
            program,
            &required_term,
            &provided_term,
            &variables,
            &mut bindings,
        )
    }
    /// The shared index key intentionally omits some operation payloads. Check
    /// those before either key correspondence or selected structural fallback.
    /// This walks source value positions, preserving each call's source owner;
    /// declaration namespaces are classified by the existing callable query.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn matching_operation_payloads(
        &mut self,
        program: &dyn crate::CheckingInput,
        required_scope: (SymbolHandle, &[StateParameter]),
        required: ExpressionHandle,
        required_binders: &[(SymbolHandle, String)],
        provided_scope: (SymbolHandle, &[StateParameter]),
        provided: ExpressionHandle,
        provided_binders: &[(SymbolHandle, String)],
    ) -> bool {
        use typed_trees::expression::ExpressionNode;
        let type_matches = |left, right| {
            if !program.type_reference_table.contains_type_reference(left)
                || !program.type_reference_table.contains_type_reference(right)
            {
                return !left.is_valid() && !right.is_valid();
            }
            program.type_identity(TypeIdentityRequest {
                binders: required_binders,
                ..TypeIdentityRequest::ordinary(left)
            }) == program.type_identity(TypeIdentityRequest {
                binders: provided_binders,
                ..TypeIdentityRequest::ordinary(right)
            })
        };
        let mut pending = vec![(required, provided, 0usize)];
        let mut required_children = Vec::new();
        let mut provided_children = Vec::new();
        let mut remaining_work = MAX_SOURCE_MATCH_WORK;
        while let Some((required, provided, depth)) = pending.pop() {
            let Some(remaining) = remaining_work.checked_sub(1) else {
                return false;
            };
            remaining_work = remaining;
            if !required.is_valid() && !provided.is_valid() {
                continue;
            }
            if depth >= 128
                || !program.expression_table.expression_is_valid(required)
                || !program.expression_table.expression_is_valid(provided)
            {
                return false;
            }
            let left = program.expression_table.expression(required);
            let right = program.expression_table.expression(provided);
            let call_value_difference = matches!(
                (left, right),
                (ExpressionNode::Call(_), _) | (_, ExpressionNode::Call(_))
            ) && !matches!(
                (left, right),
                (
                    ExpressionNode::Call(_),
                    ExpressionNode::Call(_) | ExpressionNode::Binary(_)
                ) | (ExpressionNode::Binary(_), ExpressionNode::Call(_))
            );
            if call_value_difference {
                if !neutral_operation_payloads(program, required, &mut remaining_work)
                    || !neutral_operation_payloads(program, provided, &mut remaining_work)
                {
                    return false;
                }
                // This grants no equation. Only the subsequent checked
                // closed-value normalization can compare these different forms.
                continue;
            }
            match (left, right) {
                (ExpressionNode::Call(left), ExpressionNode::Call(right)) => {
                    // Proof labels are not resolved value binders in this
                    // view. Their spelling cannot identify relevant evidence.
                    if !left.evidence_arguments.is_empty()
                        || !right.evidence_arguments.is_empty()
                        || left.static_machine_parameter != right.static_machine_parameter
                        || !selected_dispatches_match(
                            program,
                            required_scope.0,
                            left,
                            provided_scope.0,
                            right,
                        )
                        || !type_matches(
                            left.requirement_application,
                            right.requirement_application,
                        )
                        || !type_matches(left.exact_coercion_target, right.exact_coercion_target)
                        || left.quotient_operation != right.quotient_operation
                        || left.private_layout_operation != right.private_layout_operation
                        || !installed_interrupt_payloads_match(
                            left.installed_interrupt_operation.as_ref(),
                            right.installed_interrupt_operation.as_ref(),
                        )
                        || left.layout_query != right.layout_query
                        || left.native_code_layout != right.native_code_layout
                        || left.build_selection.is_valid()
                        || right.build_selection.is_valid()
                        || left.operational_acknowledgement.acknowledges_suspend
                            != right.operational_acknowledgement.acknowledges_suspend
                        || left.operational_acknowledgement.acknowledges_block
                            != right.operational_acknowledgement.acknowledges_block
                    {
                        return false;
                    }
                }
                (ExpressionNode::Call(call), ExpressionNode::Binary(_))
                | (ExpressionNode::Binary(_), ExpressionNode::Call(call)) => {
                    if !neutral_call_payload(call) {
                        return false;
                    }
                }
                (ExpressionNode::Cast(left), ExpressionNode::Cast(right)) => {
                    let table = &program.expression_table;
                    let types = &program.type_reference_table;
                    let left_arguments =
                        types.type_reference_handles(left.semantic_domain_arguments);
                    let right_arguments =
                        types.type_reference_handles(right.semantic_domain_arguments);
                    if !type_matches(left.target_type, right.target_type)
                        || !type_matches(left.result_type, right.result_type)
                        || table.name_path_members(left.target_label)
                            != table.name_path_members(right.target_label)
                        || left.domain != right.domain
                        || table.name_path_members(left.semantic_domain)
                            != table.name_path_members(right.semantic_domain)
                        || left.semantic_domain_symbol != right.semantic_domain_symbol
                        || left.semantic_domain_id != right.semantic_domain_id
                        || left.form != right.form
                        || left_arguments.len() != right_arguments.len()
                        || !left_arguments
                            .iter()
                            .zip(right_arguments)
                            .all(|(left, right)| type_matches(*left, *right))
                    {
                        return false;
                    }
                }
                (ExpressionNode::Member(left), ExpressionNode::Member(right)) => {
                    if left.member_symbol != right.member_symbol
                        || left.case_variant != right.case_variant
                    {
                        return false;
                    }
                }
                (ExpressionNode::StructLiteral(left), ExpressionNode::StructLiteral(right)) => {
                    let fields = |literal: &typed_trees::expression::TableStructLiteral| {
                        program.expression_table.struct_fields(literal.fields)
                    };
                    if left.type_symbol != right.type_symbol
                        || left.case_symbol != right.case_symbol
                        || fields(left).len() != fields(right).len()
                        || !fields(left)
                            .iter()
                            .zip(fields(right))
                            .all(|(left, right)| left.field_symbol == right.field_symbol)
                    {
                        return false;
                    }
                }
                (ExpressionNode::Match(_), _)
                | (_, ExpressionNode::Match(_))
                | (ExpressionNode::Cast(_), _)
                | (_, ExpressionNode::Cast(_))
                | (ExpressionNode::Member(_), _)
                | (_, ExpressionNode::Member(_))
                | (ExpressionNode::StructLiteral(_), _)
                | (_, ExpressionNode::StructLiteral(_))
                | (ExpressionNode::Call(_), _)
                | (_, ExpressionNode::Call(_)) => return false,
                _ => {}
            }
            let children = |node: &ExpressionNode, owner, out: &mut Vec<ExpressionHandle>| {
                if let ExpressionNode::Call(call) = node {
                    if call.receiver.is_valid()
                        && !super::super::structural_terms::static_call_receiver(
                            program, owner, call,
                        )
                    {
                        out.push(call.receiver);
                    }
                    out.extend_from_slice(
                        program.expression_table.expression_handles(call.arguments),
                    );
                } else {
                    crate::queries::expression_traversal::children(program, node, |child| {
                        out.push(child)
                    });
                }
            };
            required_children.clear();
            provided_children.clear();
            children(left, required_scope.0, &mut required_children);
            children(right, provided_scope.0, &mut provided_children);
            if required_children.len() != provided_children.len() {
                return false;
            }
            pending.extend(
                required_children
                    .iter()
                    .zip(&provided_children)
                    .map(|(left, right)| (*left, *right, depth + 1)),
            );
        }
        true
    }
}

/// Copied clauses keep separate occurrence records for the same selected map.
/// Rejoin each record under its own owner before comparing the selected public
/// contract. The storage coordinate stays distinct and supplies no equality.
fn selected_dispatches_match(
    program: &dyn crate::CheckingInput,
    required_owner: SymbolHandle,
    required: &typed_trees::expression::TableCallExpression,
    provided_owner: SymbolHandle,
    provided: &typed_trees::expression::TableCallExpression,
) -> bool {
    if required.static_requirement_dispatch == provided.static_requirement_dispatch {
        return true;
    }
    let (Some(required_dispatch), Some(provided_dispatch)) = (
        required.static_requirement_dispatch.as_ref(),
        provided.static_requirement_dispatch.as_ref(),
    ) else {
        return false;
    };
    // Project only the occurrence coordinate locally. Derived equality still
    // checks every other field, including evidence, commitment and public row.
    let mut selected_contract = required_dispatch.clone();
    selected_contract.selection_application = provided_dispatch.selection_application;
    if selected_contract != *provided_dispatch {
        return false;
    }
    crate::machine_calls::calls::closed_requirement_dispatch_is_valid(
        program,
        required_owner,
        required.target_symbol,
        required_dispatch,
    ) && crate::machine_calls::calls::closed_requirement_dispatch_is_valid(
        program,
        provided_owner,
        provided.target_symbol,
        provided_dispatch,
    )
}

fn neutral_call_payload(call: &typed_trees::expression::TableCallExpression) -> bool {
    call.evidence_arguments.is_empty()
        && !call.static_machine_parameter.is_valid()
        && call.static_requirement_dispatch.is_none()
        && !call.requirement_application.is_valid()
        && !call.exact_coercion_target.is_valid()
        && call.quotient_operation.is_none()
        && call.private_layout_operation.is_none()
        && call.installed_interrupt_operation.is_none()
        && call.layout_query.is_none()
        && call.native_code_layout.is_none()
        && !call.build_selection.is_valid()
        && !call.operational_acknowledgement.acknowledges_suspend
        && !call.operational_acknowledgement.acknowledges_block
}

fn installed_interrupt_payloads_match(
    left: Option<&typed_trees::expression::InstalledInterruptOperationRequest>,
    right: Option<&typed_trees::expression::InstalledInterruptOperationRequest>,
) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(left), Some(right)) => {
            let same = super::super::structural_judgment::same_static_selection_payload;
            left == right
                && same(
                    std::slice::from_ref(&left.registrar),
                    std::slice::from_ref(&right.registrar),
                )
                && same(
                    std::slice::from_ref(&left.timer_slot),
                    std::slice::from_ref(&right.timer_slot),
                )
        }
        _ => false,
    }
}

/// Closed-value normalization already owns value correspondence across source
/// forms. Before using it, refuse operations whose omitted metadata would need
/// a paired source judgment. Constructor identities/fields remain in the typed
/// structural terms compared by that existing normalization.
fn neutral_operation_payloads(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    remaining_work: &mut usize,
) -> bool {
    use typed_trees::expression::ExpressionNode;
    let mut pending = vec![(expression, 0usize)];
    while let Some((expression, depth)) = pending.pop() {
        let Some(remaining) = remaining_work.checked_sub(1) else {
            return false;
        };
        *remaining_work = remaining;
        if !expression.is_valid() {
            continue;
        }
        if depth >= 128 || !program.expression_table.expression_is_valid(expression) {
            return false;
        }
        let node = program.expression_table.expression(expression);
        match node {
            ExpressionNode::Call(call) if !neutral_call_payload(call) => return false,
            ExpressionNode::Cast(_) | ExpressionNode::Member(_) | ExpressionNode::Match(_) => {
                return false;
            }
            _ => {}
        }
        crate::queries::expression_traversal::children(program, node, |child| {
            pending.push((child, depth + 1))
        });
    }
    true
}

/// Join the retained symbol roster to positional roles once. Sorting is
/// bounded by this callable's telescope; repeated or missing symbols/roles
/// cannot turn an incomplete scope into a correspondence.
fn parameter_roles<'parameters, 'binders>(
    parameters: &'parameters [StateParameter],
    binders: &'binders [(SymbolHandle, String)],
) -> Option<Vec<(&'binders str, &'parameters StateParameter)>> {
    let mut by_symbol = binders
        .iter()
        .map(|(symbol, key)| (*symbol, key.as_str()))
        .collect::<Vec<_>>();
    by_symbol.sort_unstable_by_key(|(symbol, _)| (symbol.arena_index(), symbol.generation()));
    if by_symbol.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return None;
    }
    let mut roles = parameters
        .iter()
        .map(|parameter| {
            if !parameter.symbol.is_valid() {
                return None;
            }
            let position = by_symbol
                .binary_search_by_key(
                    &(
                        parameter.symbol.arena_index(),
                        parameter.symbol.generation(),
                    ),
                    |(symbol, _)| (symbol.arena_index(), symbol.generation()),
                )
                .ok()?;
            Some((by_symbol[position].1, parameter))
        })
        .collect::<Option<Vec<_>>>()?;
    roles.sort_unstable_by_key(|(key, _)| *key);
    (!roles.windows(2).any(|pair| pair[0].0 == pair[1].0)).then_some(roles)
}

/// Collect supported leaves in source order. The result says whether the
/// entire conjunction was supported; unsupported siblings never acquire a
/// matching-law receipt merely because another leaf was an identity.
pub(crate) fn collect_equality_conjuncts<'scope>(
    program: &dyn crate::CheckingInput,
    owner: SymbolHandle,
    parameters: &'scope [StateParameter],
    expression: ExpressionHandle,
    out: &mut Vec<EqualityConjunct<'scope>>,
) -> bool {
    let mut pending = vec![expression];
    let mut complete = true;
    while let Some(expression) = pending.pop() {
        if let Some(binary) = program.expression_table.binary_node(expression)
            && binary.operator == BinaryOperator::And
        {
            pending.extend([binary.right, binary.left]);
        } else if let Some(conjunct) = equality_conjunct(program, owner, parameters, expression) {
            out.push(conjunct);
        } else {
            complete = false;
        }
    }
    complete
}
