/*
    Symbol lookup for receiver paths, member accesses and call targets inside
    an expression table.

    There is no entry of its own; callers run these lookups in their own
    order. `expressions::references` uses all of them: `assign_call_symbol`
    resolves the receiver path with
    `resolve_expression_table_receiver_path_symbols`, writes the result onto
    the receiver with `stamp_receiver_path_symbols_in_table`, then selects the
    call target with `resolve_expression_table_call_target_symbol`;
    `assign_member_symbol` uses `resolve_expression_table_member_symbol`.
    `contracts` also calls `resolve_expression_table_call_target_symbol`.

    Children: `receivers` resolves receiver paths and members, `calls` selects
    a call target, `stamping` writes resolved symbols back into the table, and
    `projected_receivers` selects call targets for receivers reached through an
    index or a case payload (`needs_declared_projection` detects those).
    `scope` also reads `projected_receivers` directly.
*/

mod calls {
    use symbols::{SymbolHandle, SymbolKind, SymbolTable};

    use super::super::scope::MachineScope;
    use super::super::targets::resolve_call_target_symbol;
    use super::receivers::resolve_expression_table_receiver_path_symbols;

    pub(in crate::symbols) fn resolve_expression_table_call_target_symbol(
        machine: &MachineScope<'_>,
        parameters: &[symbol_resolved_trees::signature::StateParameter],
        state_symbol: SymbolHandle,
        call: &symbol_resolved_trees::expression::TableCallExpression,
        expression_table: &symbol_resolved_trees::expression::ExpressionTable,
        child_type_references: &arena::Arena<symbol_resolved_trees::types::TypeReference>,
        symbols: &SymbolTable,
    ) -> SymbolHandle {
        if call.receiver.is_valid() {
            if receiver_needs_result_type(expression_table, call.receiver) {
                return SymbolHandle::invalid();
            }
            if super::projected_receivers::needs_declared_projection(
                expression_table,
                call.receiver,
            ) {
                return super::projected_receivers::call_target(
                    machine,
                    parameters,
                    state_symbol,
                    call,
                    expression_table,
                    child_type_references,
                    symbols,
                );
            }
            let (_, receiver_symbol) = resolve_expression_table_receiver_path_symbols(
                symbols,
                machine.symbol,
                state_symbol,
                expression_table,
                call.receiver,
            );
            let resolved = resolve_call_target_symbol(
                machine,
                parameters,
                true,
                receiver_symbol,
                &call.target,
                child_type_references,
                symbols,
            );
            if resolved.is_valid() {
                return resolved;
            }

            if (!receiver_symbol.is_valid()
                || matches!(symbols.get(receiver_symbol).kind, SymbolKind::Domain))
                && let symbol_resolved_trees::expression::ExpressionNode::Name(path) =
                    expression_table.expression(call.receiver)
                && !path.static_application.is_valid()
                && let [owner] = expression_table.name_path_members(path.members)
            {
                return super::super::lookup::call_target_for_attached_data(
                    symbols,
                    owner.as_str(),
                    call.target.as_str(),
                    call.target.source_span(),
                );
            }
            return SymbolHandle::invalid();
        }

        resolve_call_target_symbol(
            machine,
            parameters,
            false,
            SymbolHandle::invalid(),
            &call.target,
            child_type_references,
            symbols,
        )
    }

    fn receiver_needs_result_type(
        expressions: &symbol_resolved_trees::expression::ExpressionTable,
        mut receiver: symbol_resolved_trees::expression::ExpressionHandle,
    ) -> bool {
        use symbol_resolved_trees::expression::ExpressionNode;
        loop {
            receiver = match expressions.expression(receiver) {
                ExpressionNode::Call(_) => return true,
                ExpressionNode::Member(member) => member.receiver,
                ExpressionNode::Indexed(indexed) => indexed.collection,
                ExpressionNode::Borrow(borrow) => borrow.target,
                _ => return false,
            };
        }
    }
}
pub(in crate::symbols) mod projected_receivers {
    /*
        Projected method candidates follow exact indexed and case-payload declarations.
        Bounds, case reachability, effects, and access remain later-stage obligations.
    */

    use arena::Arena;
    use symbol_resolved_trees::data::DataMember;
    use symbol_resolved_trees::expression::{
        ExpressionHandle, ExpressionNode, ExpressionTable, TableCallExpression,
    };
    use symbol_resolved_trees::signature::StateParameter;
    use symbol_resolved_trees::statement::Statement;
    use symbol_resolved_trees::types::TypeReference;
    use symbols::{SymbolHandle, SymbolKind, SymbolTable};

    use super::super::scope::MachineScope;

    pub(in crate::symbols) fn needs_declared_projection(
        table: &ExpressionTable,
        mut expression: ExpressionHandle,
    ) -> bool {
        loop {
            expression = match table.expression(expression) {
                ExpressionNode::Indexed(_) => return true,
                ExpressionNode::Member(member) if member.case_variant.is_some() => return true,
                ExpressionNode::Member(member) => member.receiver,
                ExpressionNode::Borrow(borrow) => borrow.target,
                _ => return false,
            };
        }
    }

    enum ReceiverType<'program> {
        Declared(&'program TypeReference),
        Data(SymbolHandle),
    }

    /*
        Every projected member-call repeats the same linear scans: the prior
        statements for a local receiver type, the parameter roster for a
        parameter receiver, the data-definition roster for each projection hop,
        and the attached-machine roster for the call target. The positions are
        stable within a program, so each roster is bucketed once — the statement,
        parameter and definition positions live in the program's
        `resolution_indexes`, and the attached roster carries its own by-owner
        index. The statement index only grows its watermark since a state's
        stamped prefix strictly appends.
    */

    fn peel<'program>(
        mut reference: &'program TypeReference,
        children: &'program Arena<TypeReference>,
    ) -> &'program TypeReference {
        loop {
            reference = match reference {
                TypeReference::Reference(reference) => children.get(reference.referee),
                TypeReference::Constrained(reference) => children.get(reference.base_type),
                _ => return reference,
            };
        }
    }

    impl ReceiverType<'_> {
        fn nominal(&self, children: &Arena<TypeReference>) -> Option<SymbolHandle> {
            match self {
                Self::Data(symbol) => Some(*symbol),
                Self::Declared(reference) => match peel(reference, children) {
                    TypeReference::Named { symbol, .. } => Some(*symbol),
                    _ => None,
                },
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn call_target(
        machine: &MachineScope<'_>,
        parameters: &[StateParameter],
        state_symbol: SymbolHandle,
        call: &TableCallExpression,
        table: &ExpressionTable,
        children: &Arena<TypeReference>,
        symbols: &SymbolTable,
    ) -> SymbolHandle {
        let resolve = || {
            let mut projections = Vec::new();
            let mut expression = call.receiver;
            let name = loop {
                expression = match table.expression(expression) {
                    node @ ExpressionNode::Indexed(indexed) => {
                        projections.push(node);
                        indexed.collection
                    }
                    node @ ExpressionNode::Member(member) => {
                        projections.push(node);
                        member.receiver
                    }
                    ExpressionNode::Borrow(borrow) => borrow.target,
                    ExpressionNode::Name(name) if !name.static_application.is_valid() => {
                        break name;
                    }
                    _ => return None,
                };
            };
            let [spelling] = table.name_path_members(name.members) else {
                return None;
            };
            let declaration = symbols.get(name.symbol);
            if !name.symbol.is_valid() || name.head_symbol != name.symbol {
                return None;
            }
            let mut receiver = if spelling.is_self_receiver()
                && name.symbol == machine.symbol
                && declaration.kind == SymbolKind::Machine
            {
                ReceiverType::Data(machine.attached_data_symbol)
            } else {
                if declaration.parent != state_symbol
                    || symbols.name(name.symbol) != spelling.as_str()
                {
                    return None;
                }
                let reference = match declaration.kind {
                    SymbolKind::Parameter => {
                        &parameters
                            .get(
                                machine
                                    .resolution_indexes
                                    .parameter_position(parameters, name.symbol)?
                                    as usize,
                            )?
                            .type_reference
                    }
                    SymbolKind::Local => {
                        let position = machine
                            .resolution_indexes
                            .local_statement_position(machine.prior_statements, name.symbol)?;
                        let Statement::LocalData(local) =
                            &machine.prior_statements[position as usize]
                        else {
                            return None;
                        };
                        &local.type_reference
                    }
                    _ => return None,
                };
                ReceiverType::Declared(reference)
            };
            for projection in projections.into_iter().rev() {
                receiver = match projection {
                    ExpressionNode::Indexed(_) => {
                        let ReceiverType::Declared(reference) = receiver else {
                            return None;
                        };
                        let element = match peel(reference, children) {
                            TypeReference::FixedArray(array) => array.element_type,
                            TypeReference::Slice(slice) => slice.element_type,
                            _ => return None,
                        };
                        if !element.is_valid() {
                            return None;
                        }
                        ReceiverType::Declared(children.get(element))
                    }
                    ExpressionNode::Member(member) => {
                        let owner = receiver.nominal(children)?;
                        if !owner.is_valid() {
                            return None;
                        }
                        let definition = &machine.data_definitions[machine
                            .resolution_indexes
                            .data_definition_position(machine.data_definitions, owner)?
                            as usize];
                        let members = machine
                            .data_members
                            .span_or_empty(definition.storage.members);
                        let payload = if let Some(case) = &member.case_variant {
                            let mut variants = members.iter().filter_map(|node| match node {
                                DataMember::Variant(variant) if variant.name == *case => {
                                    Some(variant)
                                }
                                _ => None,
                            });
                            let variant = variants.next()?;
                            if variants.next().is_some()
                                || !variant.symbol.is_valid()
                                || symbols.get(variant.symbol).parent != owner
                            {
                                return None;
                            }
                            Some((
                                variant.symbol,
                                machine.data_payload_fields.span_or_empty(variant.payload),
                            ))
                        } else {
                            None
                        };
                        let mut fields = members
                            .iter()
                            .filter_map(|node| match node {
                                DataMember::Field(field) if payload.is_none() => Some(field),
                                _ => None,
                            })
                            .chain(payload.into_iter().flat_map(|(_, fields)| fields))
                            .filter(|field| field.name == member.member);
                        let field = fields.next()?;
                        let field_owner = payload.map_or(owner, |(variant, _)| variant);
                        if fields.next().is_some()
                            || !field.symbol.is_valid()
                            || symbols.get(field.symbol).parent != field_owner
                            || (payload.is_some()
                                && member.member_symbol.is_valid()
                                && member.member_symbol != field.symbol)
                        {
                            return None;
                        }
                        ReceiverType::Declared(&field.type_reference)
                    }
                    _ => return None,
                };
            }
            let owner = receiver.nominal(children)?;
            if !owner.is_valid() || symbols.get(owner).kind != SymbolKind::Data {
                return None;
            }
            let target = machine.attached_call_target(symbols, owner, &call.target);
            target.is_valid().then_some(target)
        };
        resolve().unwrap_or_else(SymbolHandle::invalid)
    }
}
mod receivers {

    use symbols::{SymbolHandle, SymbolKind, SymbolTable};

    use super::super::lookup::{
        child_indexed_symbol_by_kinds, child_or_attached_data_child_symbol_by_kinds,
    };
    use super::super::scoped_paths::{invalid_symbol_pair, resolve_state_scoped_table_path};
    use symbol_resolved_trees::expression::ExpressionNode;

    /*
        The member kinds a receiver path may resolve through — fields, states,
        parameters and variants.
    */
    const RECEIVER_MEMBER_KINDS: &[SymbolKind] = &[
        SymbolKind::Field,
        SymbolKind::State,
        SymbolKind::Parameter,
        SymbolKind::Variant,
    ];

    /// A data head selects common fields; a case head selects only its payload.
    /// Sibling payload namespaces never participate, and duplicate spellings do
    /// not become declaration-order selection.
    pub(in crate::symbols) fn pattern_field_symbol(
        symbols: &SymbolTable,
        head: SymbolHandle,
        spelling: &str,
    ) -> SymbolHandle {
        if !head.is_valid() {
            return SymbolHandle::invalid();
        }
        let mut fields = symbols
            .child_handles(head)
            .into_iter()
            .flatten()
            .filter(|field| {
                symbols.get(*field).kind == SymbolKind::Field && symbols.name(*field) == spelling
            });
        let Some(field) = fields.next() else {
            return SymbolHandle::invalid();
        };
        if fields.next().is_some() {
            SymbolHandle::invalid()
        } else {
            field
        }
    }

    /*
        Retain the field identities selected by a resolved pattern head. Contracts
        and executable matches carry the same selectors into type checking.
    */
    pub(in crate::symbols) fn assign_pattern_field_symbols(
        symbols: &SymbolTable,
        expression_table: &mut symbol_resolved_trees::expression::ExpressionTable,
        pattern: symbol_resolved_trees::expression::TableDestructurePattern,
    ) {
        let head = expression_table
            .name_path_node(pattern.head)
            .map(|path| path.symbol)
            .unwrap_or_default();
        for field_handle in pattern.fields.handles() {
            let field = expression_table.match_fields(pattern.fields)
                [(field_handle.arena_index() - pattern.fields.start().arena_index()) as usize]
                .clone();
            expression_table.match_field_mut(field_handle).field_symbol =
                pattern_field_symbol(symbols, head, field.member.as_str());
        }
    }

    pub(in crate::symbols) fn resolve_expression_table_member_symbol(
        symbols: &SymbolTable,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        expression_table: &symbol_resolved_trees::expression::ExpressionTable,
        receiver: symbol_resolved_trees::expression::ExpressionHandle,
        member: &symbol_resolved_trees::name::DiagnosticName,
    ) -> SymbolHandle {
        if let ExpressionNode::SavedSubject(saved) = expression_table.expression(receiver) {
            if saved.component == language_core::PatternComponent::Case {
                return SymbolHandle::invalid();
            }
            let head = expression_table
                .name_path_node(saved.pattern_head)
                .map(|path| path.symbol)
                .unwrap_or_default();
            return pattern_field_symbol(symbols, head, member.as_str());
        }
        let (_, receiver_symbol) = receiver_path_symbols(
            symbols,
            machine_symbol,
            state_symbol,
            expression_table,
            receiver,
            None,
        );
        if !receiver_symbol.is_valid() {
            return SymbolHandle::invalid();
        }

        child_or_attached_data_child_symbol_by_kinds(
            symbols,
            receiver_symbol,
            RECEIVER_MEMBER_KINDS,
            member.as_str(),
        )
    }

    /* Resolve a receiver expression to its lexical head and selected leaf. */
    pub(in crate::symbols) fn resolve_expression_table_receiver_path_symbols(
        symbols: &SymbolTable,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        expression_table: &symbol_resolved_trees::expression::ExpressionTable,
        receiver: symbol_resolved_trees::expression::ExpressionHandle,
    ) -> (SymbolHandle, SymbolHandle) {
        receiver_path_symbols(
            symbols,
            machine_symbol,
            state_symbol,
            expression_table,
            receiver,
            None,
        )
    }

    /*
        One walk over a receiver expression. `indexed_last_member` carries the
        receiver's possible indexed tail: the outermost `Indexed` supplies it
        (`items[index]`), and only the final member's lookup consults the indexed
        declaration form. Names, members and borrows resolve the same way
        otherwise, and a nested index inside another index has no declaration form.
        A matched subject resolves through its pattern head, a declaration path
        rather than an indexed element, and its selected member through the
        structural head's own field roster.
    */
    fn receiver_path_symbols(
        symbols: &SymbolTable,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        expression_table: &symbol_resolved_trees::expression::ExpressionTable,
        receiver: symbol_resolved_trees::expression::ExpressionHandle,
        indexed_last_member: Option<i64>,
    ) -> (SymbolHandle, SymbolHandle) {
        use symbol_resolved_trees::expression::ExpressionNode;
        match expression_table.expression(receiver) {
            ExpressionNode::SavedSubject(saved) if indexed_last_member.is_none() => {
                receiver_path_symbols(
                    symbols,
                    machine_symbol,
                    state_symbol,
                    expression_table,
                    saved.pattern_head,
                    None,
                )
            }
            ExpressionNode::Name(path) => resolve_state_scoped_table_path(
                symbols,
                machine_symbol,
                state_symbol,
                expression_table,
                path,
                indexed_last_member,
            ),
            ExpressionNode::Member(member) => {
                let (head_symbol, receiver_symbol) = receiver_path_symbols(
                    symbols,
                    machine_symbol,
                    state_symbol,
                    expression_table,
                    member.receiver,
                    None,
                );
                if !receiver_symbol.is_valid() {
                    return invalid_symbol_pair();
                }
                let member_symbol = match indexed_last_member {
                    Some(index) => child_indexed_symbol_by_kinds(
                        symbols,
                        receiver_symbol,
                        RECEIVER_MEMBER_KINDS,
                        member.member.as_str(),
                        index,
                    ),
                    None if matches!(
                        expression_table.expression(member.receiver),
                        ExpressionNode::SavedSubject(_)
                    ) =>
                    {
                        pattern_field_symbol(symbols, receiver_symbol, member.member.as_str())
                    }
                    None => child_or_attached_data_child_symbol_by_kinds(
                        symbols,
                        receiver_symbol,
                        RECEIVER_MEMBER_KINDS,
                        member.member.as_str(),
                    ),
                };

                if member_symbol.is_valid() {
                    (head_symbol, member_symbol)
                } else {
                    invalid_symbol_pair()
                }
            }
            ExpressionNode::Borrow(inner) => receiver_path_symbols(
                symbols,
                machine_symbol,
                state_symbol,
                expression_table,
                inner.target,
                indexed_last_member,
            ),
            ExpressionNode::Indexed(indexed) if indexed_last_member.is_none() => {
                let ExpressionNode::Integer(index) = expression_table.expression(indexed.index)
                else {
                    return invalid_symbol_pair();
                };
                let Some(index) = index.value_i64() else {
                    return invalid_symbol_pair();
                };

                receiver_path_symbols(
                    symbols,
                    machine_symbol,
                    state_symbol,
                    expression_table,
                    indexed.collection,
                    Some(index),
                )
            }
            _ => invalid_symbol_pair(),
        }
    }
}
mod stamping {
    use symbol_resolved_trees::expression::ExpressionNode;
    use symbols::SymbolHandle;

    pub(in crate::symbols) fn stamp_receiver_path_symbols_in_table(
        expression_table: &mut symbol_resolved_trees::expression::ExpressionTable,
        expression: symbol_resolved_trees::expression::ExpressionHandle,
        head_symbol: SymbolHandle,
        symbol: SymbolHandle,
    ) {
        match expression_table.expression(expression).clone() {
            ExpressionNode::Name(_) => {
                if let ExpressionNode::Name(path) = expression_table.expression_mut(expression) {
                    path.head_symbol = head_symbol;
                    path.symbol = symbol;
                }
            }
            ExpressionNode::Indexed(indexed) => {
                stamp_receiver_path_symbols_in_table(
                    expression_table,
                    indexed.collection,
                    head_symbol,
                    head_symbol,
                );
            }
            ExpressionNode::Member(member) => {
                stamp_receiver_path_symbols_in_table(
                    expression_table,
                    member.receiver,
                    head_symbol,
                    head_symbol,
                );
            }
            ExpressionNode::Borrow(inner) => {
                stamp_receiver_path_symbols_in_table(
                    expression_table,
                    inner.target,
                    head_symbol,
                    symbol,
                );
            }
            _ => {}
        }
    }
}

pub(super) use calls::resolve_expression_table_call_target_symbol;
pub(super) use projected_receivers::needs_declared_projection;
pub(super) use receivers::{
    assign_pattern_field_symbols, pattern_field_symbol, resolve_expression_table_member_symbol,
    resolve_expression_table_receiver_path_symbols,
};
pub(super) use stamping::stamp_receiver_path_symbols_in_table;
