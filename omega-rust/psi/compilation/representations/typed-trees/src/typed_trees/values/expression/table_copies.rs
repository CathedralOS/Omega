//! Copying expression subtrees into a table, from another table or from the
//! table itself, including member-suffixed copies and the symbol remap that
//! completes a lexical-scope clone.

use crate::name::Identifier;
use crate::typed_trees::values::expression::{
    ExpressionHandle, ExpressionNode, ExpressionTable, MatchPattern, TableAtomicExpression,
    TableBinaryExpression, TableBorrowExpression, TableCallExpression, TableCastExpression,
    TableIndexedExpression, TableMatchArm, TableMatchExpression, TableMemberExpression,
    TableNamePath, TableRangeExpression, TableStructLiteral, TableStructLiteralField,
    TableUnaryExpression,
};
use arena::{Handle, HandleSpan};
use symbols::SymbolHandle;

impl ExpressionTable {
    pub fn copy_from(
        &mut self,
        source: &ExpressionTable,
        expression: ExpressionHandle,
    ) -> ExpressionHandle {
        self.copy_from_filtering_struct_literal_fields(source, expression, &|_, _| true)
    }

    /// Copy one expression graph while omitting rejected struct-literal fields
    /// before their values are visited. This is intentionally a copy-time
    /// filter: consumers which iterate the destination arena must never observe
    /// the omitted value subtrees, even as unreachable nodes.
    pub fn copy_from_filtering_struct_literal_fields(
        &mut self,
        source: &ExpressionTable,
        expression: ExpressionHandle,
        retain: &impl Fn(&TableStructLiteral, &TableStructLiteralField) -> bool,
    ) -> ExpressionHandle {
        let source_span = source.source_span(expression);
        let copied = match source.expression(expression) {
            ExpressionNode::Match(dispatch) => {
                let subject = self.copy_from_filtering_struct_literal_fields(
                    source,
                    dispatch.subject,
                    retain,
                );
                let source_arms = source.match_arms(dispatch.arms).to_vec();
                let mut arms = Vec::with_capacity(source_arms.len());
                for arm in source_arms {
                    let pattern = match arm.pattern {
                        MatchPattern::Value(value) => MatchPattern::Value(
                            self.copy_from_filtering_struct_literal_fields(source, value, retain),
                        ),
                        MatchPattern::Wildcard => MatchPattern::Wildcard,
                    };
                    let value =
                        self.copy_from_filtering_struct_literal_fields(source, arm.value, retain);
                    arms.push(TableMatchArm {
                        pattern,
                        value,
                        source_span: arm.source_span,
                    });
                }
                let arms = self.insert_match_arms(arms);
                self.insert(ExpressionNode::Match(TableMatchExpression {
                    subject,
                    arms,
                }))
            }
            ExpressionNode::ArrayLiteral(source_values) => {
                let values = self
                    .copy_expression_handles_from_slice_filtering_struct_literal_fields(
                        source,
                        source.expression_handles(*source_values),
                        retain,
                    );
                self.insert(ExpressionNode::ArrayLiteral(values))
            }
            ExpressionNode::Atomic(atomic) => {
                let value =
                    self.copy_from_filtering_struct_literal_fields(source, atomic.value, retain);
                let result = atomic
                    .result
                    .is_valid()
                    .then(|| {
                        self.copy_from_filtering_struct_literal_fields(
                            source,
                            atomic.result,
                            retain,
                        )
                    })
                    .unwrap_or_else(ExpressionHandle::invalid);
                self.insert(ExpressionNode::Atomic(TableAtomicExpression {
                    value,
                    result,
                    ordering: atomic.ordering,
                    result_custody: atomic.result_custody,
                }))
            }
            ExpressionNode::Binary(binary) => {
                let left =
                    self.copy_from_filtering_struct_literal_fields(source, binary.left, retain);
                let right =
                    self.copy_from_filtering_struct_literal_fields(source, binary.right, retain);
                self.insert(ExpressionNode::Binary(TableBinaryExpression {
                    left,
                    operator: binary.operator,
                    right,
                }))
            }
            ExpressionNode::Boolean(value) => self.insert(ExpressionNode::Boolean(*value)),
            ExpressionNode::Cast(cast) => {
                let value =
                    self.copy_from_filtering_struct_literal_fields(source, cast.value, retain);
                let target_type = cast.target_type;
                let target_label = self.copy_name_path_members(source, cast.target_label);
                let semantic_domain = self.copy_name_path_members(source, cast.semantic_domain);
                self.insert(ExpressionNode::Cast(TableCastExpression {
                    value,
                    target_type,
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
                let receiver = call
                    .receiver
                    .is_valid()
                    .then(|| {
                        self.copy_from_filtering_struct_literal_fields(
                            source,
                            call.receiver,
                            retain,
                        )
                    })
                    .unwrap_or_else(ExpressionHandle::invalid);
                let arguments = self
                    .copy_expression_handles_from_slice_filtering_struct_literal_fields(
                        source,
                        source.expression_handles(call.arguments),
                        retain,
                    );
                self.insert(ExpressionNode::Call(TableCallExpression {
                    receiver,
                    target_symbol: call.target_symbol,
                    target: call.target.clone(),
                    static_requirement_dispatch: call.static_requirement_dispatch.clone(),
                    static_machine_parameter: call.static_machine_parameter,
                    machine_arguments: call.machine_arguments.clone(),
                    quotient_operation: call.quotient_operation.clone(),
                    private_layout_operation: call.private_layout_operation.clone(),
                    layout_query: call.layout_query.clone(),
                    arguments,
                    evidence_arguments: call.evidence_arguments.clone(),
                    operational_acknowledgement: call.operational_acknowledgement,
                }))
            }
            ExpressionNode::Float(value) => self.insert(ExpressionNode::Float(value.clone())),
            ExpressionNode::UndeterminedNaN(format) => {
                self.insert(ExpressionNode::UndeterminedNaN(*format))
            }
            ExpressionNode::Indexed(indexed) => {
                let collection = self.copy_from_filtering_struct_literal_fields(
                    source,
                    indexed.collection,
                    retain,
                );
                let index =
                    self.copy_from_filtering_struct_literal_fields(source, indexed.index, retain);
                self.insert(ExpressionNode::Indexed(TableIndexedExpression {
                    collection,
                    index,
                }))
            }
            ExpressionNode::Integer(value) => self.insert(ExpressionNode::Integer(value.clone())),
            ExpressionNode::Member(member) => {
                let receiver =
                    self.copy_from_filtering_struct_literal_fields(source, member.receiver, retain);
                self.insert(ExpressionNode::Member(TableMemberExpression {
                    receiver,
                    member_symbol: member.member_symbol,
                    member: member.member.clone(),
                    case_variant: member.case_variant.clone(),
                }))
            }
            ExpressionNode::Borrow(inner_expression) => {
                let target = self.copy_from_filtering_struct_literal_fields(
                    source,
                    inner_expression.target,
                    retain,
                );
                self.insert(ExpressionNode::Borrow(TableBorrowExpression {
                    target,
                    access: inner_expression.access,
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
                let start = range
                    .start
                    .is_valid()
                    .then(|| {
                        self.copy_from_filtering_struct_literal_fields(source, range.start, retain)
                    })
                    .unwrap_or_else(ExpressionHandle::invalid);
                let end = range
                    .end
                    .is_valid()
                    .then(|| {
                        self.copy_from_filtering_struct_literal_fields(source, range.end, retain)
                    })
                    .unwrap_or_else(ExpressionHandle::invalid);
                self.insert(ExpressionNode::Range(TableRangeExpression {
                    start,
                    end,
                    end_inclusive: range.end_inclusive,
                }))
            }
            ExpressionNode::StructLiteral(struct_literal) => {
                let fields =
                    self.copy_struct_literal_fields_filtering(source, struct_literal, retain);
                self.insert(ExpressionNode::StructLiteral(TableStructLiteral {
                    type_name: struct_literal.type_name.clone(),
                    type_symbol: struct_literal.type_symbol,
                    case_name: struct_literal.case_name.clone(),
                    case_symbol: struct_literal.case_symbol,
                    fields,
                }))
            }
            ExpressionNode::String(value) => self.insert(ExpressionNode::String(value.clone())),
            ExpressionNode::Unary(unary) => {
                let operand =
                    self.copy_from_filtering_struct_literal_fields(source, unary.operand, retain);
                self.insert(ExpressionNode::Unary(TableUnaryExpression {
                    operator: unary.operator,
                    operand,
                }))
            }
            ExpressionNode::ZeroValue(type_reference) => {
                self.insert(ExpressionNode::ZeroValue(*type_reference))
            }
        };
        self.set_source_span(copied, source_span);
        self.attach_authored_selection_occurrences(
            copied,
            source.authored_selection_occurrences(expression),
        );
        copied
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

        let node = self.expression(root).clone();
        match node {
            ExpressionNode::Match(dispatch) => {
                self.remap_symbols_in_inner(dispatch.subject, symbols, visited);
                let arms = self.match_arms(dispatch.arms).to_vec();
                for arm in arms {
                    if let MatchPattern::Value(pattern) = arm.pattern {
                        self.remap_symbols_in_inner(pattern, symbols, visited);
                    }
                    self.remap_symbols_in_inner(arm.value, symbols, visited);
                }
            }
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
                let arguments = self.expression_handles(call.arguments).to_vec();
                for argument in arguments {
                    self.remap_symbols_in_inner(argument, symbols, visited);
                }
                let ExpressionNode::Call(call) = self.expression_mut(root) else {
                    unreachable!();
                };
                call.target_symbol = remapped(call.target_symbol, symbols);
                for argument in &mut call.machine_arguments {
                    argument.symbol = remapped(argument.symbol, symbols);
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
            ExpressionNode::Boolean(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::UndeterminedNaN(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::String(_)
            | ExpressionNode::ZeroValue(_) => {}
        }
    }

    pub fn copy_expression_handles_from(
        &mut self,
        source: &ExpressionTable,
        expressions: HandleSpan<ExpressionHandle>,
    ) -> HandleSpan<ExpressionHandle> {
        self.copy_expression_handles_from_slice(source, source.expression_handles(expressions))
    }

    pub fn copy_expression_handles_from_slice(
        &mut self,
        source: &ExpressionTable,
        expressions: &[ExpressionHandle],
    ) -> HandleSpan<ExpressionHandle> {
        let copied = self.reserve_expression_handles(
            expressions
                .len()
                .try_into()
                .expect("expression handle span count overflow"),
        );

        for (offset, expression) in expressions.iter().enumerate() {
            let expression = self.copy_from(source, *expression);
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

    fn copy_expression_handles_from_slice_filtering_struct_literal_fields(
        &mut self,
        source: &ExpressionTable,
        expressions: &[ExpressionHandle],
        retain: &impl Fn(&TableStructLiteral, &TableStructLiteralField) -> bool,
    ) -> HandleSpan<ExpressionHandle> {
        let copied = self.reserve_expression_handles(
            expressions
                .len()
                .try_into()
                .expect("expression handle span count overflow"),
        );

        for (offset, expression) in expressions.iter().enumerate() {
            let expression =
                self.copy_from_filtering_struct_literal_fields(source, *expression, retain);
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
        source: &ExpressionTable,
        members: HandleSpan<Identifier>,
    ) -> HandleSpan<Identifier> {
        let mut copied = HandleSpan::empty();

        for member in source.name_path_members(members) {
            self.name_path_members
                .append_to_span(&mut copied, member.clone());
        }

        copied
    }

    fn copy_name_path_member_symbols(
        &mut self,
        source: &ExpressionTable,
        member_symbols: HandleSpan<SymbolHandle>,
    ) -> HandleSpan<SymbolHandle> {
        let mut copied = HandleSpan::empty();

        for member_symbol in source.name_path_member_symbols(member_symbols) {
            self.name_path_member_symbols
                .append_to_span(&mut copied, *member_symbol);
        }

        copied
    }

    fn copy_own_name_path_members(
        &mut self,
        members: HandleSpan<Identifier>,
    ) -> HandleSpan<Identifier> {
        let mut copied = HandleSpan::empty();

        for offset in 0..members.count() {
            let member = self.name_path_member_at_offset(members, offset).clone();
            self.name_path_members.append_to_span(&mut copied, member);
        }

        copied
    }

    fn copy_own_name_path_member_symbols(
        &mut self,
        member_symbols: HandleSpan<SymbolHandle>,
    ) -> HandleSpan<SymbolHandle> {
        let mut copied = HandleSpan::empty();

        for offset in 0..member_symbols.count() {
            let member_symbol = *self.name_path_member_symbol_at_offset(member_symbols, offset);
            self.name_path_member_symbols
                .append_to_span(&mut copied, member_symbol);
        }

        copied
    }

    fn copy_own_name_path_members_with_index_suffix(
        &mut self,
        members: HandleSpan<Identifier>,
        index: i64,
    ) -> HandleSpan<Identifier> {
        if members.is_empty() {
            return HandleSpan::empty();
        }

        let mut copied = HandleSpan::empty();
        let last_offset = members.count() - 1;

        for offset in 0..members.count() {
            let member = self.name_path_member_at_offset(members, offset);
            let member = if offset == last_offset {
                Identifier::generated(format!("{member}[{index}]"))
            } else {
                member.clone()
            };

            self.name_path_members.append_to_span(&mut copied, member);
        }

        copied
    }

    fn copy_own_name_path_member_symbols_with_index_suffix(
        &mut self,
        member_symbols: HandleSpan<SymbolHandle>,
    ) -> HandleSpan<SymbolHandle> {
        if member_symbols.is_empty() {
            return HandleSpan::empty();
        }

        self.copy_own_name_path_member_symbols(member_symbols)
    }

    fn name_path_member_at_offset(
        &self,
        members: HandleSpan<Identifier>,
        offset: u32,
    ) -> &Identifier {
        self.name_path_members.get(Handle::from_parts(
            members
                .start()
                .arena_index()
                .checked_add(offset)
                .expect("name path member index overflow"),
            members.start().generation(),
        ))
    }

    fn name_path_member_symbol_at_offset(
        &self,
        member_symbols: HandleSpan<SymbolHandle>,
        offset: u32,
    ) -> &SymbolHandle {
        self.name_path_member_symbols.get(Handle::from_parts(
            member_symbols
                .start()
                .arena_index()
                .checked_add(offset)
                .expect("name path member symbol index overflow"),
            member_symbols.start().generation(),
        ))
    }

    fn copy_struct_literal_fields_filtering(
        &mut self,
        source: &ExpressionTable,
        literal: &TableStructLiteral,
        retain: &impl Fn(&TableStructLiteral, &TableStructLiteralField) -> bool,
    ) -> HandleSpan<TableStructLiteralField> {
        let retained = source
            .struct_fields(literal.fields)
            .iter()
            .filter(|field| retain(literal, field))
            .collect::<Vec<_>>();
        let copied = self.reserve_struct_fields(
            retained
                .len()
                .try_into()
                .expect("struct literal field span count overflow"),
        );

        for (offset, field) in retained.into_iter().enumerate() {
            let value = self.copy_from_filtering_struct_literal_fields(source, field.value, retain);
            self.set_struct_field_at_offset(
                copied,
                offset
                    .try_into()
                    .expect("struct literal field span count overflow"),
                TableStructLiteralField {
                    name: field.name.clone(),
                    field_symbol: field.field_symbol,
                    value,
                },
            );
        }

        copied
    }

    fn copy_own_expression_handles(
        &mut self,
        expressions: HandleSpan<ExpressionHandle>,
    ) -> HandleSpan<ExpressionHandle> {
        let copied = self.reserve_expression_handles(expressions.count());

        for offset in 0..expressions.count() {
            let expression = self.expression_handle_at_offset(expressions, offset);
            let expression = self.insert_copy(expression);
            self.set_expression_handle_at_offset(copied, offset, expression);
        }

        copied
    }

    fn copy_own_struct_literal_fields(
        &mut self,
        fields: HandleSpan<TableStructLiteralField>,
    ) -> HandleSpan<TableStructLiteralField> {
        let copied = self.reserve_struct_fields(fields.count());

        for offset in 0..fields.count() {
            let field = self.struct_field_at_offset(fields, offset).clone();
            let value = self.insert_copy(field.value);
            self.set_struct_field_at_offset(
                copied,
                offset,
                TableStructLiteralField {
                    name: field.name,
                    field_symbol: field.field_symbol,
                    value,
                },
            );
        }

        copied
    }

    pub fn copy_name_path_members_with_member_suffix(
        &mut self,
        members: HandleSpan<Identifier>,
        suffix_members: HandleSpan<Identifier>,
        suffix_start_offset: u32,
    ) -> HandleSpan<Identifier> {
        let suffix_count = suffix_members.count().saturating_sub(suffix_start_offset);
        let copied = self.reserve_name_path_members(
            members
                .count()
                .checked_add(suffix_count)
                .expect("name path member span count overflow"),
        );

        for offset in 0..members.count() {
            let member = self.name_path_member_at_offset(members, offset).clone();
            self.set_name_path_member_at_offset(copied, offset, member);
        }

        for (target_offset, offset) in (suffix_start_offset..suffix_members.count()).enumerate() {
            let member = self
                .name_path_member_at_offset(suffix_members, offset)
                .clone();
            self.set_name_path_member_at_offset(
                copied,
                members
                    .count()
                    .checked_add(
                        target_offset
                            .try_into()
                            .expect("name path member span count overflow"),
                    )
                    .expect("name path member span count overflow"),
                member,
            );
        }

        copied
    }

    pub fn copy_name_path_member_symbols_with_member_suffix(
        &mut self,
        member_symbols: HandleSpan<SymbolHandle>,
        suffix_member_symbols: HandleSpan<SymbolHandle>,
        suffix_start_offset: u32,
    ) -> HandleSpan<SymbolHandle> {
        let suffix_count = suffix_member_symbols
            .count()
            .saturating_sub(suffix_start_offset);
        let copied = self.reserve_name_path_member_symbols(
            member_symbols
                .count()
                .checked_add(suffix_count)
                .expect("name path member symbol span count overflow"),
        );

        for offset in 0..member_symbols.count() {
            let member_symbol = *self.name_path_member_symbol_at_offset(member_symbols, offset);
            self.set_name_path_member_symbol_at_offset(copied, offset, member_symbol);
        }

        for (target_offset, offset) in
            (suffix_start_offset..suffix_member_symbols.count()).enumerate()
        {
            let member_symbol =
                *self.name_path_member_symbol_at_offset(suffix_member_symbols, offset);
            self.set_name_path_member_symbol_at_offset(
                copied,
                member_symbols
                    .count()
                    .checked_add(
                        target_offset
                            .try_into()
                            .expect("name path member symbol span count overflow"),
                    )
                    .expect("name path member symbol span count overflow"),
                member_symbol,
            );
        }

        copied
    }

    pub fn insert_copy_with_member_suffix(
        &mut self,
        expression: ExpressionHandle,
        suffix_members: HandleSpan<Identifier>,
        suffix_member_symbols: HandleSpan<SymbolHandle>,
        suffix_start_offset: u32,
    ) -> ExpressionHandle {
        if suffix_start_offset >= suffix_members.count() {
            return expression;
        }

        let occurrences = self
            .authored_selection_occurrences(expression)
            .collect::<Vec<_>>();
        let copied = match self.expression(expression).clone() {
            ExpressionNode::Name(path) => {
                let members = self.copy_name_path_members_with_member_suffix(
                    path.members,
                    suffix_members,
                    suffix_start_offset,
                );
                let member_symbols = self.copy_name_path_member_symbols_with_member_suffix(
                    path.member_symbols,
                    suffix_member_symbols,
                    suffix_start_offset,
                );
                self.insert(ExpressionNode::Name(TableNamePath {
                    members,
                    member_symbols,
                    head_symbol: path.head_symbol,
                    symbol: SymbolHandle::invalid(),
                }))
            }
            ExpressionNode::Borrow(target) => {
                let access = target.access;
                let target = self.insert_copy_with_member_suffix(
                    target.target,
                    suffix_members,
                    suffix_member_symbols,
                    suffix_start_offset,
                );
                self.insert(ExpressionNode::Borrow(TableBorrowExpression {
                    target,
                    access,
                }))
            }
            ExpressionNode::Indexed(indexed) => {
                if let Some(path) = self.copy_indexed_expression_path(indexed) {
                    let members = self.copy_name_path_members_with_member_suffix(
                        path.members,
                        suffix_members,
                        suffix_start_offset,
                    );
                    let member_symbols = self.copy_name_path_member_symbols_with_member_suffix(
                        path.member_symbols,
                        suffix_member_symbols,
                        suffix_start_offset,
                    );
                    self.insert(ExpressionNode::Name(TableNamePath {
                        members,
                        member_symbols,
                        head_symbol: path.head_symbol,
                        symbol: SymbolHandle::invalid(),
                    }))
                } else {
                    let copied = self.insert_copy(expression);
                    self.insert_member_suffix_chain(
                        copied,
                        suffix_members,
                        suffix_member_symbols,
                        suffix_start_offset,
                    )
                }
            }
            // Member receivers (and anything else) get the member-suffix
            // chain. The old catch-all copied the receiver WITHOUT the suffix
            // -- silently dropping it -- so an alias substitution through here
            // resolved to the RECEIVER's place instead of the member's (the
            // append_place_suffix clobber class; the suffix must never be
            // dropped).
            _ => {
                let copied = self.insert_copy(expression);
                self.insert_member_suffix_chain(
                    copied,
                    suffix_members,
                    suffix_member_symbols,
                    suffix_start_offset,
                )
            }
        };
        self.attach_authored_selection_occurrences(copied, occurrences);
        copied
    }

    fn insert_member_suffix_chain(
        &mut self,
        expression: ExpressionHandle,
        suffix_members: HandleSpan<Identifier>,
        suffix_member_symbols: HandleSpan<SymbolHandle>,
        suffix_start_offset: u32,
    ) -> ExpressionHandle {
        let mut expression = expression;
        for offset in suffix_start_offset..suffix_members.count() {
            let member = self
                .name_path_member_at_offset(suffix_members, offset)
                .clone();
            let member_symbol = if offset < suffix_member_symbols.count() {
                *self.name_path_member_symbol_at_offset(suffix_member_symbols, offset)
            } else {
                SymbolHandle::invalid()
            };
            expression = self.insert(ExpressionNode::Member(TableMemberExpression {
                receiver: expression,
                member_symbol,
                member,
                case_variant: None,
            }));
        }
        expression
    }

    pub fn insert_copy(&mut self, expression: ExpressionHandle) -> ExpressionHandle {
        let source_span = self.source_span(expression);
        let occurrences = self
            .authored_selection_occurrences(expression)
            .collect::<Vec<_>>();
        let copied = match self.expression(expression).clone() {
            ExpressionNode::Match(dispatch) => {
                let subject = self.insert_copy(dispatch.subject);
                let source_arms = self.match_arms(dispatch.arms).to_vec();
                let mut arms = Vec::with_capacity(source_arms.len());
                for arm in source_arms {
                    let pattern = match arm.pattern {
                        MatchPattern::Value(value) => MatchPattern::Value(self.insert_copy(value)),
                        MatchPattern::Wildcard => MatchPattern::Wildcard,
                    };
                    let value = self.insert_copy(arm.value);
                    arms.push(TableMatchArm {
                        pattern,
                        value,
                        source_span: arm.source_span,
                    });
                }
                let arms = self.insert_match_arms(arms);
                self.insert(ExpressionNode::Match(TableMatchExpression {
                    subject,
                    arms,
                }))
            }
            ExpressionNode::ArrayLiteral(values) => {
                let values = self.copy_own_expression_handles(values);
                self.insert(ExpressionNode::ArrayLiteral(values))
            }
            ExpressionNode::Atomic(atomic) => {
                let value = self.insert_copy(atomic.value);
                let result = atomic
                    .result
                    .is_valid()
                    .then(|| self.insert_copy(atomic.result))
                    .unwrap_or_else(ExpressionHandle::invalid);
                self.insert(ExpressionNode::Atomic(TableAtomicExpression {
                    value,
                    result,
                    ordering: atomic.ordering,
                    result_custody: atomic.result_custody,
                }))
            }
            ExpressionNode::Binary(binary) => {
                let left = self.insert_copy(binary.left);
                let right = self.insert_copy(binary.right);
                self.insert(ExpressionNode::Binary(TableBinaryExpression {
                    left,
                    operator: binary.operator,
                    right,
                }))
            }
            ExpressionNode::Boolean(value) => self.insert(ExpressionNode::Boolean(value)),
            ExpressionNode::Cast(cast) => {
                let value = self.insert_copy(cast.value);
                let target_type = cast.target_type;
                let target_label = self.copy_own_name_path_members(cast.target_label);
                let semantic_domain = self.copy_own_name_path_members(cast.semantic_domain);
                self.insert(ExpressionNode::Cast(TableCastExpression {
                    value,
                    target_type,
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
                let receiver = call
                    .receiver
                    .is_valid()
                    .then(|| self.insert_copy(call.receiver))
                    .unwrap_or_else(ExpressionHandle::invalid);
                let arguments = self.copy_own_expression_handles(call.arguments);
                self.insert(ExpressionNode::Call(TableCallExpression {
                    receiver,
                    target_symbol: call.target_symbol,
                    target: call.target,
                    static_requirement_dispatch: call.static_requirement_dispatch,
                    static_machine_parameter: call.static_machine_parameter,
                    machine_arguments: call.machine_arguments,
                    quotient_operation: call.quotient_operation,
                    private_layout_operation: call.private_layout_operation,
                    layout_query: call.layout_query,
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
                let collection = self.insert_copy(indexed.collection);
                let index = self.insert_copy(indexed.index);
                self.insert(ExpressionNode::Indexed(TableIndexedExpression {
                    collection,
                    index,
                }))
            }
            ExpressionNode::Integer(value) => self.insert(ExpressionNode::Integer(value)),
            ExpressionNode::Member(member) => {
                let receiver = self.insert_copy(member.receiver);
                self.insert(ExpressionNode::Member(TableMemberExpression {
                    receiver,
                    member_symbol: member.member_symbol,
                    member: member.member,
                    case_variant: member.case_variant,
                }))
            }
            ExpressionNode::Borrow(inner_expression) => {
                let target = self.insert_copy(inner_expression.target);
                self.insert(ExpressionNode::Borrow(TableBorrowExpression {
                    target,
                    access: inner_expression.access,
                }))
            }
            ExpressionNode::Name(path) => {
                let members = self.copy_own_name_path_members(path.members);
                let member_symbols = self.copy_own_name_path_member_symbols(path.member_symbols);
                self.insert(ExpressionNode::Name(TableNamePath {
                    members,
                    member_symbols,
                    head_symbol: path.head_symbol,
                    symbol: path.symbol,
                }))
            }
            ExpressionNode::Range(range) => {
                let start = range
                    .start
                    .is_valid()
                    .then(|| self.insert_copy(range.start))
                    .unwrap_or_else(ExpressionHandle::invalid);
                let end = range
                    .end
                    .is_valid()
                    .then(|| self.insert_copy(range.end))
                    .unwrap_or_else(ExpressionHandle::invalid);
                self.insert(ExpressionNode::Range(TableRangeExpression {
                    start,
                    end,
                    end_inclusive: range.end_inclusive,
                }))
            }
            ExpressionNode::StructLiteral(struct_literal) => {
                let fields = self.copy_own_struct_literal_fields(struct_literal.fields);
                self.insert(ExpressionNode::StructLiteral(TableStructLiteral {
                    type_name: struct_literal.type_name,
                    type_symbol: struct_literal.type_symbol,
                    case_name: struct_literal.case_name,
                    case_symbol: struct_literal.case_symbol,
                    fields,
                }))
            }
            ExpressionNode::String(value) => self.insert(ExpressionNode::String(value)),
            ExpressionNode::Unary(unary) => {
                let operand = self.insert_copy(unary.operand);
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
        self.attach_authored_selection_occurrences(copied, occurrences);
        copied
    }

    fn copy_indexed_expression_path(
        &mut self,
        indexed: TableIndexedExpression,
    ) -> Option<TableNamePath> {
        let ExpressionNode::Integer(index) = self.expression(indexed.index) else {
            return None;
        };
        // An index beyond i64 cannot name a real element; treat it like any
        // non-constant index (no synthetic const-index path).
        let index = index.value_i64()?;

        let base = match self.expression(indexed.collection).clone() {
            ExpressionNode::Name(path) => path,
            ExpressionNode::Indexed(inner_indexed) => {
                self.copy_indexed_expression_path(inner_indexed)?
            }
            _ => return None,
        };
        let members = self.copy_own_name_path_members_with_index_suffix(base.members, index);
        let member_symbols =
            self.copy_own_name_path_member_symbols_with_index_suffix(base.member_symbols);
        if members.is_empty() {
            return None;
        }

        Some(TableNamePath {
            members,
            member_symbols,
            head_symbol: base.head_symbol,
            symbol: SymbolHandle::invalid(),
        })
    }
}

fn remapped(symbol: SymbolHandle, symbols: &[(SymbolHandle, SymbolHandle)]) -> SymbolHandle {
    symbols
        .iter()
        .find_map(|(source, target)| (*source == symbol).then_some(*target))
        .unwrap_or(symbol)
}
