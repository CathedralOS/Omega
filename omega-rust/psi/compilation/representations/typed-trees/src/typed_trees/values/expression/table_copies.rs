//! Copying expression subtrees into a table, from another table or from the
//! table itself, with the symbol remap that completes a lexical-scope clone.
//!
//! Every subtree copy runs through [`ExpressionTable::copy_subtree`]; the
//! public entry points differ only in the [`CopySource`] they read and the
//! struct-literal field filter they apply. The walk appends children before
//! their parent and reserves or inserts each span in walk order, so a copy
//! appends the same rows whichever table it reads. Each copy retains one new
//! handle per source occurrence: normalized DAG edges remain shared within
//! the clone, while separate invocations and distinct authored nodes stay
//! independent. Expanding a shared operand back into a tree would repeat its
//! effects or detach later projections from its captured value.

use crate::name::Identifier;
use crate::typed_trees::remapped;
use crate::typed_trees::values::expression::{
    ExpressionHandle, ExpressionNode, ExpressionTable, MatchPattern, TableAtomicExpression,
    TableBinaryExpression, TableBorrowExpression, TableCallExpression, TableCastExpression,
    TableIndexedExpression, TableMatchArm, TableMatchExpression, TableMemberExpression,
    TableNamePath, TableRangeExpression, TableStructLiteral, TableStructLiteralField,
    TableUnaryExpression,
};
use arena::{Handle, HandleSpan};

#[derive(Default)]
struct CopyGraph {
    expressions: Vec<(ExpressionHandle, ExpressionHandle)>,
    subjects: Vec<(
        Handle<super::CompletedValueCapture>,
        Handle<super::CompletedValueCapture>,
    )>,
}
use symbols::SymbolHandle;

/// Where a subtree copy reads the rows it appends.
#[derive(Clone, Copy)]
enum CopySource<'source> {
    /// The destination table itself: the copy lands beside its original.
    Own,
    /// A separate table, which the copy leaves untouched.
    Table(&'source ExpressionTable),
}

impl<'source> CopySource<'source> {
    /// The table to read. The result borrows `destination` even for a
    /// separate table, so every read ends before the walk appends again.
    fn table<'table>(self, destination: &'table ExpressionTable) -> &'table ExpressionTable
    where
        'source: 'table,
    {
        match self {
            Self::Own => destination,
            Self::Table(source) => source,
        }
    }
}

/// The struct-literal field filter of an unfiltered copy.
fn retain_every_field(_: &TableStructLiteral, _: &TableStructLiteralField) -> bool {
    true
}

impl ExpressionTable {
    /// Copy the invocation and its selected-map occurrence together. A map
    /// qualifier owns a distinct telescope; only an operand selection shares
    /// the invocation's application. Neither may retain the template's handle.
    pub fn copy_call_static_applications_from(
        &mut self,
        source: &ExpressionTable,
        application: crate::static_applications::StaticMachineApplicationHandle,
        dispatch: Option<crate::typed_trees::StaticRequirementDispatch>,
    ) -> (
        crate::static_applications::StaticMachineApplicationHandle,
        Option<crate::typed_trees::StaticRequirementDispatch>,
    ) {
        self.copy_call_static_applications(CopySource::Table(source), application, dispatch)
    }

    fn copy_call_static_applications(
        &mut self,
        source: CopySource<'_>,
        application: crate::static_applications::StaticMachineApplicationHandle,
        mut dispatch: Option<crate::typed_trees::StaticRequirementDispatch>,
    ) -> (
        crate::static_applications::StaticMachineApplicationHandle,
        Option<crate::typed_trees::StaticRequirementDispatch>,
    ) {
        let copied = self.copy_static_machine_application(source, application);
        if let Some(dispatch) = &mut dispatch {
            dispatch.selection_application = if dispatch.selection_application == application {
                copied
            } else {
                self.copy_static_machine_application(source, dispatch.selection_application)
            };
        }
        (copied, dispatch)
    }

    /// Rebase both retained source telescopes into the cloned lexical owner.
    pub fn remap_call_static_applications(
        &mut self,
        application: crate::static_applications::StaticMachineApplicationHandle,
        dispatch: Option<&crate::typed_trees::StaticRequirementDispatch>,
        symbols: &[(SymbolHandle, SymbolHandle)],
    ) {
        let selected = dispatch.map_or(Default::default(), |dispatch| {
            dispatch.selection_application
        });
        if application.is_valid() {
            self.remap_static_application(application, symbols);
        }
        if selected.is_valid() && selected != application {
            self.remap_static_application(selected, symbols);
        }
    }

    fn remap_static_application(
        &mut self,
        handle: crate::static_applications::StaticMachineApplicationHandle,
        symbols: &[(SymbolHandle, SymbolHandle)],
    ) {
        let application = self.static_machine_application_mut(handle);
        application.lexical_owner = remapped(application.lexical_owner, symbols);
        for argument in self.static_machine_application_arguments_mut(handle) {
            argument.remap_symbols(symbols);
        }
        let subject_expressions = self
            .static_machine_application_bindings(handle)
            .iter()
            .filter_map(|binding| match &binding.value {
                crate::static_applications::StaticMachineBindingValue::RuntimeValue {
                    expression,
                    ..
                } if expression.is_valid() => Some(*expression),
                _ => None,
            })
            .collect::<Vec<_>>();
        for binding in self.static_machine_application_bindings_mut(handle) {
            binding.remap_symbols(symbols);
        }
        for expression in subject_expressions {
            self.remap_symbols_in(expression, symbols);
        }
        for argument in self.static_machine_application_selected_machines_mut(handle) {
            argument.remap_symbols(symbols);
        }
    }

    fn copy_static_machine_application(
        &mut self,
        source: CopySource<'_>,
        application: crate::static_applications::StaticMachineApplicationHandle,
    ) -> crate::static_applications::StaticMachineApplicationHandle {
        if !application.is_valid() {
            return application;
        }
        let origin = source.table(self);
        let declaration = origin.static_machine_application(application).clone();
        let arguments = origin
            .static_machine_application_arguments(application)
            .to_vec();
        let bindings = origin
            .static_machine_application_bindings(application)
            .to_vec();
        let selected = origin
            .static_machine_application_selected_machines(application)
            .to_vec();
        let lifetimes = origin
            .static_machine_application_lifetimes(application)
            .to_vec();
        let copy = self.retain_static_machine_application(
            declaration.registration_operation,
            declaration.target,
            &arguments,
        );
        self.static_machine_application_mut(copy).lexical_owner = declaration.lexical_owner;
        self.static_machine_application_mut(copy)
            .binding_declaration = declaration.binding_declaration;
        self.static_machine_application_mut(copy)
            .conformance_selection = declaration.conformance_selection.map(|mut selection| {
            // Raw operands are copied and rewritten below. Derived scopes and
            // callback views must be rebuilt in that new occurrence.
            selection.application = None;
            selection
        });
        self.set_static_machine_application_bindings(copy, &bindings);
        self.set_static_machine_application_selected_machines(copy, &selected);
        self.set_static_machine_application_lifetimes(copy, &lifetimes);
        copy
    }

    pub fn copy_from(
        &mut self,
        source: &ExpressionTable,
        expression: ExpressionHandle,
    ) -> ExpressionHandle {
        self.copy_subtree(CopySource::Table(source), expression, &retain_every_field)
    }

    /// Copy one expression graph within this table. The copy's rows follow
    /// every existing row and share no span with the original.
    pub fn insert_copy(&mut self, expression: ExpressionHandle) -> ExpressionHandle {
        self.copy_subtree(CopySource::Own, expression, &retain_every_field)
    }

    /// Remap lexical identities throughout one expression graph. The graph's
    /// arena handles and authored names stay stable; only normalized symbols
    /// change. This is the second half of cloning a lexical scope after
    /// [`Self::copy_from`] has made its payload handles independent.
    pub fn remap_symbols_in(
        &mut self,
        root: ExpressionHandle,
        symbols: &[(SymbolHandle, SymbolHandle)],
    ) {
        let mut visited = Vec::new();
        self.remap_symbols_in_inner(root, symbols, &mut visited);
    }

    fn remap_symbols_in_inner(
        &mut self,
        root: ExpressionHandle,
        symbols: &[(SymbolHandle, SymbolHandle)],
        visited: &mut Vec<ExpressionHandle>,
    ) {
        if !root.is_valid() || visited.contains(&root) {
            return;
        }
        visited.push(root);

        if self.index_expression_authority(root).is_some() {
            let authority = self.index_expression_authority_mut(root);
            if let Some(operation) = &mut authority.operation {
                if let Some(application) = &mut operation.application {
                    application.remap_symbols(symbols);
                }
                for symbol in [
                    &mut operation.provider,
                    &mut operation.operator,
                    &mut operation.algebra_trait,
                ] {
                    if let Some((_, replacement)) =
                        symbols.iter().find(|(original, _)| original == symbol)
                    {
                        *symbol = *replacement;
                    }
                }
            }
        }

        let node = self.expression(root).clone();
        match node {
            ExpressionNode::Match(dispatch) => {
                self.remap_symbols_in_inner(dispatch.subject, symbols, visited);
                let arms = self.match_arms(dispatch.arms).to_vec();
                for arm in arms {
                    for pattern in self
                        .match_pattern_dependencies(arm.pattern)
                        .collect::<Vec<_>>()
                    {
                        self.remap_symbols_in_inner(pattern, symbols, visited);
                    }
                    if let MatchPattern::Destructure(pattern) = arm.pattern {
                        for position in 0..pattern.fields.count() {
                            let field = self.match_field_mut(pattern.fields.handle_at(position));
                            field.field_symbol = remapped(field.field_symbol, symbols);
                        }
                    }
                    self.remap_symbols_in_inner(arm.value, symbols, visited);
                }
            }
            // A read does not own its defining subject or head. Match remaps
            // those definitions; an ambient capture stays outside this clone.
            ExpressionNode::SavedSubject(_) => {}
            ExpressionNode::ArrayLiteral(values) => {
                let children = self.expression_handles(values).to_vec();
                for child in children {
                    self.remap_symbols_in_inner(child, symbols, visited);
                }
            }
            ExpressionNode::Atomic(atomic) => {
                self.remap_symbols_in_inner(atomic.value, symbols, visited);
                self.remap_symbols_in_inner(atomic.result, symbols, visited);
            }
            ExpressionNode::Binary(binary) => {
                self.remap_symbols_in_inner(binary.left, symbols, visited);
                self.remap_symbols_in_inner(binary.right, symbols, visited);
            }
            ExpressionNode::Cast(cast) => self.remap_symbols_in_inner(cast.value, symbols, visited),
            ExpressionNode::Call(call) => {
                self.remap_symbols_in_inner(call.receiver, symbols, visited);
                self.remap_call_static_applications(
                    call.authored_machine_application,
                    call.static_requirement_dispatch.as_ref(),
                    symbols,
                );
                let arguments = self.expression_handles(call.arguments).to_vec();
                for argument in arguments {
                    self.remap_symbols_in_inner(argument, symbols, visited);
                }
                let ExpressionNode::Call(call) = self.expression_mut(root) else {
                    unreachable!();
                };
                call.target_symbol = remapped(call.target_symbol, symbols);
                if let Some(query) = &mut call.native_code_layout {
                    query.operand.remap_symbols(symbols);
                }
                for argument in &mut call.machine_arguments {
                    argument.remap_symbols(symbols);
                }
            }
            ExpressionNode::Indexed(indexed) => {
                self.remap_symbols_in_inner(indexed.collection, symbols, visited);
                self.remap_symbols_in_inner(indexed.index, symbols, visited);
            }
            ExpressionNode::Member(member) => {
                self.remap_symbols_in_inner(member.receiver, symbols, visited);
                let ExpressionNode::Member(member) = self.expression_mut(root) else {
                    unreachable!();
                };
                member.member_symbol = remapped(member.member_symbol, symbols);
            }
            ExpressionNode::Borrow(inner) => {
                self.remap_symbols_in_inner(inner.target, symbols, visited)
            }
            ExpressionNode::Name(path) => {
                let ExpressionNode::Name(current) = self.expression_mut(root) else {
                    unreachable!();
                };
                current.head_symbol = remapped(path.head_symbol, symbols);
                current.symbol = remapped(path.symbol, symbols);
                for member_symbol in self
                    .name_path_member_symbols
                    .span_mut_or_empty(path.member_symbols)
                {
                    *member_symbol = remapped(*member_symbol, symbols);
                }
            }
            ExpressionNode::Range(range) => {
                self.remap_symbols_in_inner(range.start, symbols, visited);
                self.remap_symbols_in_inner(range.end, symbols, visited);
            }
            ExpressionNode::StructLiteral(literal) => {
                let children: Vec<_> = self
                    .struct_fields(literal.fields)
                    .iter()
                    .map(|field| field.value)
                    .collect();
                for child in children {
                    self.remap_symbols_in_inner(child, symbols, visited);
                }
            }
            ExpressionNode::Unary(unary) => {
                self.remap_symbols_in_inner(unary.operand, symbols, visited)
            }
            crate::spelled_literal_expression_pattern!() => {}
        }
    }

    pub fn copy_expression_handles_from_slice(
        &mut self,
        source: &ExpressionTable,
        expressions: &[ExpressionHandle],
    ) -> HandleSpan<ExpressionHandle> {
        self.copy_expression_list(CopySource::Table(source), expressions, &retain_every_field)
    }

    /// Copy one ordered operand graph with shared capture identities. The
    /// statement owner closes its own definition handles after allocating it.
    pub(crate) fn copy_call_operands_from(
        &mut self,
        source: &ExpressionTable,
        expressions: &[ExpressionHandle],
    ) -> (
        HandleSpan<ExpressionHandle>,
        Vec<(
            Handle<super::CompletedValueCapture>,
            super::ValueCaptureOwner,
        )>,
    ) {
        let mut copies = CopyGraph::default();
        let expressions = self.copy_expression_list_with_occurrences(
            CopySource::Table(source),
            expressions,
            &retain_every_field,
            &mut copies,
        );
        (
            expressions,
            copies
                .subjects
                .into_iter()
                .map(|(original, capture)| (capture, source.value_capture(original).owner))
                .collect(),
        )
    }

    /// Relocate a statement within its existing lexical body without copying
    /// its operand graph. The source statement must be retired by the owner;
    /// every existing capture keeps its identity and its exact argument slot.
    pub fn relocate_statement_operand_captures(
        &mut self,
        source: crate::statement::StatementHandle,
        target: crate::statement::StatementHandle,
    ) {
        let captures = self
            .completed_values
            .iter()
            .filter_map(|(capture, value)| {
                matches!(value.owner,
                    super::ValueCaptureOwner::StatementArgument { call, .. } if call == source
                )
                .then_some((capture, value.owner))
            })
            .collect::<Vec<_>>();
        self.rebind_statement_operand_captures(&captures, source, target);
    }

    pub(crate) fn rebind_statement_operand_captures(
        &mut self,
        captures: &[(
            Handle<super::CompletedValueCapture>,
            super::ValueCaptureOwner,
        )],
        source: crate::statement::StatementHandle,
        target: crate::statement::StatementHandle,
    ) {
        for &(capture, owner) in captures {
            if let super::ValueCaptureOwner::StatementArgument { call, position } = owner
                && call == source
            {
                self.completed_values.get_mut(capture).owner =
                    super::ValueCaptureOwner::StatementArgument {
                        call: target,
                        position,
                    };
            }
        }
    }

    /// The one subtree copy behind `copy_from`, the filtered copy,
    /// `insert_copy` and the expression-handle copies. Each node and span is
    /// cloned out of `source` before anything is appended, which lets a copy
    /// read the table it writes. The copy keeps the node's source span and
    /// authored-selection occurrences.
    fn copy_subtree<RetainField>(
        &mut self,
        source: CopySource<'_>,
        expression: ExpressionHandle,
        retain: &RetainField,
    ) -> ExpressionHandle
    where
        RetainField: Fn(&TableStructLiteral, &TableStructLiteralField) -> bool,
    {
        self.copy_subtree_with_occurrences(source, expression, retain, &mut CopyGraph::default())
    }

    fn copy_expression_list<RetainField>(
        &mut self,
        source: CopySource<'_>,
        expressions: &[ExpressionHandle],
        retain: &RetainField,
    ) -> HandleSpan<ExpressionHandle>
    where
        RetainField: Fn(&TableStructLiteral, &TableStructLiteralField) -> bool,
    {
        self.copy_expression_list_with_occurrences(
            source,
            expressions,
            retain,
            &mut CopyGraph::default(),
        )
    }

    fn copy_subtree_with_occurrences<RetainField>(
        &mut self,
        source: CopySource<'_>,
        expression: ExpressionHandle,
        retain: &RetainField,
        copies: &mut CopyGraph,
    ) -> ExpressionHandle
    where
        RetainField: Fn(&TableStructLiteral, &TableStructLiteralField) -> bool,
    {
        if !expression.is_valid() {
            return expression;
        }
        if let Some((_, copied)) = copies
            .expressions
            .iter()
            .find(|(original, _)| *original == expression)
        {
            return *copied;
        }
        let origin = source.table(self);
        let source_span = origin.source_span(expression);
        let authority = origin.index_expression_authority(expression).cloned();
        let query_carrier = matches!(origin.expression(expression), ExpressionNode::Call(call)
            if call.native_code_layout.is_some() || call.layout_query.is_some())
        .then(|| origin.constructed_value_type_reference(expression));
        let occurrences = origin
            .authored_selection_occurrences(expression)
            .collect::<Vec<_>>();
        let copied = match origin.expression(expression).clone() {
            ExpressionNode::Match(dispatch) => {
                let subject =
                    self.copy_subtree_with_occurrences(source, dispatch.subject, retain, copies);
                let capture = self.copy_match_subject(source, dispatch.capture, retain, copies);
                let source_arms = source.table(self).match_arms(dispatch.arms).to_vec();
                let mut arms = Vec::with_capacity(source_arms.len());
                for arm in source_arms {
                    let pattern = match arm.pattern {
                        MatchPattern::Value(value) => MatchPattern::Value(
                            self.copy_subtree_with_occurrences(source, value, retain, copies),
                        ),
                        MatchPattern::Wildcard => MatchPattern::Wildcard,
                        MatchPattern::Destructure(pattern) => MatchPattern::Destructure(
                            self.copy_destructure_pattern(source, pattern, retain, copies),
                        ),
                    };
                    let value =
                        self.copy_subtree_with_occurrences(source, arm.value, retain, copies);
                    arms.push(TableMatchArm {
                        pattern,
                        value,
                        source_span: arm.source_span,
                    });
                }
                let arms = self.insert_match_arms(arms);
                self.insert(ExpressionNode::Match(TableMatchExpression {
                    subject,
                    capture,
                    arms,
                }))
            }
            ExpressionNode::SavedSubject(saved) => {
                let copied_capture = copies
                    .subjects
                    .iter()
                    .find_map(|(original, copied)| (*original == saved.capture).then_some(*copied));
                if copied_capture.is_none() && matches!(source, CopySource::Own) {
                    // Copying only an arm subtree keeps its enclosing capture;
                    // it cannot create another subject evaluation or mutate the
                    // definition while alpha-rebasing this child.
                    self.insert(ExpressionNode::SavedSubject(saved))
                } else {
                    let capture = self.copy_match_subject(source, saved.capture, retain, copies);
                    let pattern_head = self.copy_subtree_with_occurrences(
                        source,
                        saved.pattern_head,
                        retain,
                        copies,
                    );
                    self.insert(ExpressionNode::SavedSubject(super::TableSavedSubject {
                        capture,
                        pattern_head,
                    }))
                }
            }
            ExpressionNode::ArrayLiteral(values) => {
                let values = self.copy_expression_span(source, values, retain, copies);
                self.insert(ExpressionNode::ArrayLiteral(values))
            }
            ExpressionNode::Atomic(atomic) => {
                let value =
                    self.copy_subtree_with_occurrences(source, atomic.value, retain, copies);
                let result = self.copy_optional_subtree(source, atomic.result, retain, copies);
                self.insert(ExpressionNode::Atomic(TableAtomicExpression {
                    value,
                    result,
                    ordering: atomic.ordering,
                    result_custody: atomic.result_custody,
                }))
            }
            ExpressionNode::Binary(binary) => {
                let left = self.copy_subtree_with_occurrences(source, binary.left, retain, copies);
                let right =
                    self.copy_subtree_with_occurrences(source, binary.right, retain, copies);
                self.insert(ExpressionNode::Binary(TableBinaryExpression {
                    left,
                    operator: binary.operator,
                    right,
                }))
            }
            ExpressionNode::Boolean(value) => self.insert(ExpressionNode::Boolean(value)),
            ExpressionNode::Cast(cast) => {
                let value = self.copy_subtree_with_occurrences(source, cast.value, retain, copies);
                let target_label = self.copy_name_path_members(source, cast.target_label);
                let semantic_domain = self.copy_name_path_members(source, cast.semantic_domain);
                self.insert(ExpressionNode::Cast(TableCastExpression {
                    value,
                    target_type: cast.target_type,
                    result_type: cast.result_type,
                    target_label,
                    domain: cast.domain,
                    semantic_domain,
                    semantic_domain_arguments: cast.semantic_domain_arguments,
                    semantic_domain_symbol: cast.semantic_domain_symbol,
                    semantic_domain_id: cast.semantic_domain_id,
                    form: cast.form,
                }))
            }
            ExpressionNode::Call(call) => {
                let receiver = self.copy_optional_subtree(source, call.receiver, retain, copies);
                let arguments = self.copy_expression_span(source, call.arguments, retain, copies);
                let root_binding = source
                    .table(self)
                    .root_binding_request(call.root_binding)
                    .cloned()
                    .map(|request| self.insert_root_binding_request(request))
                    .unwrap_or_default();
                let (authored_machine_application, static_requirement_dispatch) = self
                    .copy_call_static_applications(
                        source,
                        call.authored_machine_application,
                        call.static_requirement_dispatch,
                    );
                self.insert(ExpressionNode::Call(TableCallExpression {
                    authored_machine_application,
                    requirement_application: call.requirement_application,
                    exact_coercion_target: call.exact_coercion_target,
                    receiver,
                    target_symbol: call.target_symbol,
                    target: call.target,
                    static_requirement_dispatch,
                    static_machine_parameter: call.static_machine_parameter,
                    machine_arguments: call.machine_arguments,
                    quotient_operation: call.quotient_operation,
                    private_layout_operation: call.private_layout_operation,
                    layout_query: call.layout_query,
                    native_code_layout: call.native_code_layout,
                    root_binding,
                    arguments,
                    evidence_arguments: call.evidence_arguments,
                    operational_acknowledgement: call.operational_acknowledgement,
                }))
            }
            ExpressionNode::Float(value) => self.insert(ExpressionNode::Float(value)),
            ExpressionNode::UndeterminedNaN(format) => {
                self.insert(ExpressionNode::UndeterminedNaN(format))
            }
            ExpressionNode::Indexed(indexed) => {
                let collection =
                    self.copy_subtree_with_occurrences(source, indexed.collection, retain, copies);
                let index =
                    self.copy_subtree_with_occurrences(source, indexed.index, retain, copies);
                self.insert(ExpressionNode::Indexed(TableIndexedExpression {
                    collection,
                    index,
                }))
            }
            ExpressionNode::Integer(value) => self.insert(ExpressionNode::Integer(value)),
            ExpressionNode::Member(member) => {
                let receiver =
                    self.copy_subtree_with_occurrences(source, member.receiver, retain, copies);
                self.insert(ExpressionNode::Member(TableMemberExpression {
                    receiver,
                    member_symbol: member.member_symbol,
                    member: member.member,
                    case_variant: member.case_variant,
                }))
            }
            ExpressionNode::Borrow(borrow) => {
                let target =
                    self.copy_subtree_with_occurrences(source, borrow.target, retain, copies);
                self.insert(ExpressionNode::Borrow(TableBorrowExpression {
                    target,
                    access: borrow.access,
                }))
            }
            ExpressionNode::Name(path) => {
                let members = self.copy_name_path_members(source, path.members);
                let member_symbols =
                    self.copy_name_path_member_symbols(source, path.member_symbols);
                self.insert(ExpressionNode::Name(TableNamePath {
                    members,
                    member_symbols,
                    head_symbol: path.head_symbol,
                    symbol: path.symbol,
                }))
            }
            ExpressionNode::Range(range) => {
                let start = self.copy_optional_subtree(source, range.start, retain, copies);
                let end = self.copy_optional_subtree(source, range.end, retain, copies);
                self.insert(ExpressionNode::Range(TableRangeExpression {
                    start,
                    end,
                    end_inclusive: range.end_inclusive,
                }))
            }
            ExpressionNode::StructLiteral(literal) => {
                let fields = self.copy_struct_literal_fields(source, &literal, retain, copies);
                self.insert(ExpressionNode::StructLiteral(TableStructLiteral {
                    type_name: literal.type_name,
                    type_symbol: literal.type_symbol,
                    case_name: literal.case_name,
                    case_symbol: literal.case_symbol,
                    fields,
                }))
            }
            ExpressionNode::String(value) => self.insert(ExpressionNode::String(value)),
            ExpressionNode::Unary(unary) => {
                let operand =
                    self.copy_subtree_with_occurrences(source, unary.operand, retain, copies);
                self.insert(ExpressionNode::Unary(TableUnaryExpression {
                    operator: unary.operator,
                    operand,
                }))
            }
            ExpressionNode::ZeroValue(type_reference) => {
                self.insert(ExpressionNode::ZeroValue(type_reference))
            }
        };
        self.set_source_span(copied, source_span);
        if let Some(carrier) = query_carrier {
            self.set_constructed_value_type_reference(copied, carrier);
        }
        self.attach_authored_selection_occurrences(copied, occurrences);
        if let Some(mut authority) = authority {
            if let Some(operation) = &mut authority.operation {
                operation.expression = copied;
            }
            *self.index_expression_authority_mut(copied) = authority;
        }
        copies.expressions.push((expression, copied));
        // A call owns aliases among its children. Their copied parent only
        // exists after the children, so close that exact source back-edge now.
        for (original, capture) in &copies.subjects {
            if let super::ValueCaptureOwner::ExpressionArgument { call, position } =
                source.table(self).value_capture(*original).owner
                && call == expression
            {
                self.completed_values.get_mut(*capture).owner =
                    super::ValueCaptureOwner::ExpressionArgument {
                        call: copied,
                        position,
                    };
            }
        }
        copied
    }

    fn copy_match_subject<RetainField>(
        &mut self,
        source: CopySource<'_>,
        capture: Handle<super::CompletedValueCapture>,
        retain: &RetainField,
        copies: &mut CopyGraph,
    ) -> Handle<super::CompletedValueCapture>
    where
        RetainField: Fn(&TableStructLiteral, &TableStructLiteralField) -> bool,
    {
        if !capture.is_valid() {
            return capture;
        }
        if let Some((_, copied)) = copies
            .subjects
            .iter()
            .find(|(original, _)| *original == capture)
        {
            return *copied;
        }
        let captured = *source.table(self).value_capture(capture);
        let subject = self.copy_subtree_with_occurrences(source, captured.subject, retain, copies);
        let copied = if matches!(captured.owner, super::ValueCaptureOwner::Match) {
            self.insert_match_subject(subject)
        } else {
            let owner = match captured.owner {
                super::ValueCaptureOwner::ExpressionArgument { call, position } => {
                    let call = copies
                        .expressions
                        .iter()
                        .find(|(source, _)| *source == call)
                        .map(|(_, copied)| *copied)
                        .unwrap_or_else(ExpressionHandle::invalid);
                    super::ValueCaptureOwner::ExpressionArgument { call, position }
                }
                super::ValueCaptureOwner::StatementArgument { position, .. } => {
                    super::ValueCaptureOwner::StatementArgument {
                        call: crate::statement::StatementHandle::invalid(),
                        position,
                    }
                }
                owner => owner,
            };
            self.insert_call_operand_capture(subject, owner)
        };
        copies.subjects.push((capture, copied));
        copied
    }

    fn copy_destructure_pattern<RetainField>(
        &mut self,
        source: CopySource<'_>,
        pattern: super::TableDestructurePattern,
        retain: &RetainField,
        copies: &mut CopyGraph,
    ) -> super::TableDestructurePattern
    where
        RetainField: Fn(&TableStructLiteral, &TableStructLiteralField) -> bool,
    {
        let head = self.copy_subtree_with_occurrences(source, pattern.head, retain, copies);
        let mut fields = source.table(self).match_fields(pattern.fields).to_vec();
        for field in &mut fields {
            if let super::MatchFieldAction::Equal(value) = &mut field.action {
                *value = self.copy_subtree_with_occurrences(source, *value, retain, copies);
            }
        }
        let fields = self.insert_match_fields(fields);
        let guard = self.copy_subtree_with_occurrences(source, pattern.guard, retain, copies);
        let condition =
            self.copy_subtree_with_occurrences(source, pattern.condition, retain, copies);
        let selectors = source
            .table(self)
            .match_proof_selectors(pattern.proof_selectors)
            .to_vec();
        let proof_selectors = self.insert_match_proof_selectors(selectors);
        super::TableDestructurePattern {
            head,
            fields,
            has_rest: pattern.has_rest,
            guard,
            condition,
            proof_selectors,
        }
    }

    /// Copies an optional child; the invalid handle marks an absent one and
    /// stays invalid.
    fn copy_optional_subtree<RetainField>(
        &mut self,
        source: CopySource<'_>,
        expression: ExpressionHandle,
        retain: &RetainField,
        copies: &mut CopyGraph,
    ) -> ExpressionHandle
    where
        RetainField: Fn(&TableStructLiteral, &TableStructLiteralField) -> bool,
    {
        if expression.is_valid() {
            self.copy_subtree_with_occurrences(source, expression, retain, copies)
        } else {
            ExpressionHandle::invalid()
        }
    }

    fn copy_expression_span<RetainField>(
        &mut self,
        source: CopySource<'_>,
        expressions: HandleSpan<ExpressionHandle>,
        retain: &RetainField,
        copies: &mut CopyGraph,
    ) -> HandleSpan<ExpressionHandle>
    where
        RetainField: Fn(&TableStructLiteral, &TableStructLiteralField) -> bool,
    {
        let expressions = source.table(self).expression_handles(expressions).to_vec();
        self.copy_expression_list_with_occurrences(source, &expressions, retain, copies)
    }

    /// Reserves the copied span before copying its elements, so the span's
    /// rows precede every row its elements append.
    fn copy_expression_list_with_occurrences<RetainField>(
        &mut self,
        source: CopySource<'_>,
        expressions: &[ExpressionHandle],
        retain: &RetainField,
        copies: &mut CopyGraph,
    ) -> HandleSpan<ExpressionHandle>
    where
        RetainField: Fn(&TableStructLiteral, &TableStructLiteralField) -> bool,
    {
        let copied = self.reserve_expression_handles(
            expressions
                .len()
                .try_into()
                .expect("expression handle span count overflow"),
        );

        for (offset, expression) in expressions.iter().enumerate() {
            let expression =
                self.copy_subtree_with_occurrences(source, *expression, retain, copies);
            self.set_expression_handle_at_offset(
                copied,
                offset
                    .try_into()
                    .expect("expression handle span count overflow"),
                expression,
            );
        }

        copied
    }

    fn copy_name_path_members(
        &mut self,
        source: CopySource<'_>,
        members: HandleSpan<Identifier>,
    ) -> HandleSpan<Identifier> {
        let members = source.table(self).name_path_members(members).to_vec();
        let mut copied = HandleSpan::empty();

        for member in members {
            self.name_path_members.append_to_span(&mut copied, member);
        }

        copied
    }

    fn copy_name_path_member_symbols(
        &mut self,
        source: CopySource<'_>,
        member_symbols: HandleSpan<SymbolHandle>,
    ) -> HandleSpan<SymbolHandle> {
        let member_symbols = source
            .table(self)
            .name_path_member_symbols(member_symbols)
            .to_vec();
        let mut copied = HandleSpan::empty();

        for member_symbol in member_symbols {
            self.name_path_member_symbols
                .append_to_span(&mut copied, member_symbol);
        }

        copied
    }

    /// Filters every field before copying any value, then reserves the
    /// retained fields' span ahead of their values' rows.
    fn copy_struct_literal_fields<RetainField>(
        &mut self,
        source: CopySource<'_>,
        literal: &TableStructLiteral,
        retain: &RetainField,
        copies: &mut CopyGraph,
    ) -> HandleSpan<TableStructLiteralField>
    where
        RetainField: Fn(&TableStructLiteral, &TableStructLiteralField) -> bool,
    {
        let retained = source
            .table(self)
            .struct_fields(literal.fields)
            .iter()
            .filter(|field| retain(literal, field))
            .cloned()
            .collect::<Vec<_>>();
        let copied = self.reserve_struct_fields(
            retained
                .len()
                .try_into()
                .expect("struct literal field span count overflow"),
        );

        for (offset, field) in retained.into_iter().enumerate() {
            let value = self.copy_subtree_with_occurrences(source, field.value, retain, copies);
            self.set_struct_field_at_offset(
                copied,
                offset
                    .try_into()
                    .expect("struct literal field span count overflow"),
                TableStructLiteralField {
                    name: field.name,
                    field_symbol: field.field_symbol,
                    value,
                },
            );
        }

        copied
    }
}
