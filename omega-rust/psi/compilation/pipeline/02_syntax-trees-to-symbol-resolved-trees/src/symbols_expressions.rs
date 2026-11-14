/*
    Symbol assignment inside expression tables: `references` resolves names,
    cases, receivers and calls against the lexical value frontier, and
    `traversal` walks every expression a statement, signature or contract owns.
*/

mod references {
    use crate::tables::body_expressions;
    use diagnostics::Diagnostic;
    use symbols::{JoinedPath, SymbolHandle, SymbolKind, SymbolTable};

    use super::super::expression_paths::{
        needs_declared_projection, resolve_expression_table_call_target_symbol,
        resolve_expression_table_member_symbol, resolve_expression_table_receiver_path_symbols,
        stamp_receiver_path_symbols_in_table,
    };
    use super::super::lookup::{
        child_symbol_by_kinds, diagnostic_path_source_span, top_level_symbol_for_source,
    };
    use super::super::scope::MachineScope;
    use super::super::scoped_paths::{
        resolve_path_members_from_head, resolve_state_scoped_table_path,
        resolve_state_scoped_table_path_member_symbols,
    };
    use super::super::targets::assign_call_machine_argument_symbols;
    use std::collections::BTreeSet;
    use std::collections::HashSet;
    use symbol_resolved_trees::data::DataMember;
    use symbol_resolved_trees::data::TypeParameterKind;
    use symbol_resolved_trees::expression::ExpressionHandle;
    use symbol_resolved_trees::expression::ExpressionNode;
    use symbol_resolved_trees::expression::ExpressionTable;
    use symbol_resolved_trees::expression::TableCallExpression;
    use symbol_resolved_trees::expression::TableNamePath;
    use symbol_resolved_trees::expression::TableStructLiteral;
    use symbol_resolved_trees::expression::TableStructLiteralField;
    use symbol_resolved_trees::name::DiagnosticName;
    use symbol_resolved_trees::signature::StateParameter;
    use symbol_resolved_trees::statement::Statement;
    use symbol_resolved_trees::types::TypeReference;

    /*
        A case is a member of its carrier, never an expected-type-dependent bare
        value. Reject the exact unresolved spelling here so no later consumer can
        reinterpret it as an implicit constructor.
    */
    pub(crate) fn reject_unqualified_case_values(
        program: &symbol_resolved_trees::SymbolResolvedTrees,
    ) -> Result<(), Vec<Diagnostic>> {
        let case_names = program
            .data_definitions
            .iter()
            .flat_map(|definition| program.data_members(definition.members))
            .filter_map(|member| match member {
                DataMember::Variant(case) => Some(case.name.as_str()),
                DataMember::Field(_) => None,
            })
            .collect::<BTreeSet<_>>();
        let path_heads = body_expressions(program)
            .iter_expressions()
            .filter_map(|(_, expression)| match expression {
                ExpressionNode::Call(call) => Some(call.receiver),
                ExpressionNode::Member(member) => Some(member.receiver),
                _ => None,
            })
            .filter(|handle| handle.is_valid())
            .collect::<HashSet<_>>();
        let diagnostics = body_expressions(program)
            .iter_expressions()
            .filter_map(|(expression_handle, expression)| {
                if path_heads.contains(&expression_handle) {
                    return None;
                }
                let ExpressionNode::Name(path) = expression else {
                    return None;
                };
                let [name] = body_expressions(program).name_path_members(path.members) else {
                    return None;
                };
                (!path.head_symbol.is_valid()
                    && !path.symbol.is_valid()
                    && body_expressions(program)
                        .authored_expression_exposure(expression_handle)
                        .is_some()
                    && case_names.contains(name.as_str()))
                .then(|| {
                    Diagnostic::error(format!(
                        "case value `{}` requires its carrier-qualified `Type::{}` path",
                        name.as_str(),
                        name.as_str(),
                    ))
                    .with_source_span(name.source_span())
                })
            })
            .collect::<Vec<_>>();
        if diagnostics.is_empty() {
            Ok(())
        } else {
            Err(diagnostics)
        }
    }

    /*
        The spelled member names of a `self`-rooted receiver path, root -> leaf
        (`["self", "p"]` for the receiver `self.p`). `None` for non-place receivers
        (calls, literals, indexed). Mirrors the state-call plan's receiver walk and
        validation's `receiver_member_chain` at this layer.
    */
    fn spelled_receiver_chain(
        expression_table: &ExpressionTable,
        receiver: ExpressionHandle,
    ) -> Option<Vec<String>> {
        if !receiver.is_valid() {
            return None;
        }
        match expression_table.expression(receiver) {
            ExpressionNode::Name(path) if !path.static_application.is_valid() => {
                let members = expression_table.name_path_members(path.members);
                (!members.is_empty()).then(|| {
                    members
                        .iter()
                        .map(|member| member.as_str().to_string())
                        .collect()
                })
            }
            ExpressionNode::Member(member) => {
                let mut chain = spelled_receiver_chain(expression_table, member.receiver)?;
                chain.push(member.member.as_str().to_string());
                Some(chain)
            }
            ExpressionNode::Borrow(inner) => spelled_receiver_chain(expression_table, inner.target),
            _ => None,
        }
    }

    /*
        The leaf FIELD symbol of a NESTED `self`-rooted receiver chain: walk the
        chain's declared field types to the leaf's OWNER type, then look up the leaf
        field in that data's symbol. `["self", "p", "a"]` -> field `a` in `PairD`'s
        data symbol. `None` for direct (`len < 3`) chains -- those resolve by the
        normal machine-child lookup -- or when any hop is unresolvable. Rung 2b of
        the receiver-place staircase.
    */
    fn nested_receiver_leaf_field_symbol(
        symbols: &SymbolTable,
        machine: &MachineScope<'_>,
        chain: &[String],
    ) -> SymbolHandle {
        if chain.len() < 3 {
            return SymbolHandle::invalid();
        }
        let borrowed: Vec<&str> = chain.iter().map(String::as_str).collect();
        let leaf = borrowed[borrowed.len() - 1];
        let Some(owner_type) = machine.nested_self_chain_type(&borrowed[..borrowed.len() - 1])
        else {
            return SymbolHandle::invalid();
        };
        let owner_data = owner_type;
        if !owner_data.is_valid() {
            return SymbolHandle::invalid();
        }
        child_symbol_by_kinds(
            symbols,
            owner_data,
            &[SymbolKind::Field, SymbolKind::Variant],
            leaf,
        )
    }

    /*
        The callee STATE symbol for a method called on a NESTED `self`-rooted
        receiver (`self.p.a.stored()` -> `BoxI::stored`'s state): walk the full
        receiver chain to the leaf TYPE, then resolve the method on the machine
        attached to that type. `None` for direct chains or unresolvable hops.
    */
    fn nested_receiver_call_target_symbol(
        symbols: &SymbolTable,
        machine: &MachineScope<'_>,
        receiver_chain: &[String],
        target: &DiagnosticName,
    ) -> SymbolHandle {
        if receiver_chain.len() < 3 {
            return SymbolHandle::invalid();
        }
        let borrowed: Vec<&str> = receiver_chain.iter().map(String::as_str).collect();
        let Some(leaf_type) = machine.nested_self_chain_type(&borrowed) else {
            return SymbolHandle::invalid();
        };
        machine.attached_call_target(symbols, leaf_type, target)
    }

    pub(super) fn assign_call_symbol(
        symbols: &SymbolTable,
        machine: &MachineScope<'_>,
        parameters: &[StateParameter],
        state_symbol: SymbolHandle,
        expression_table: &mut ExpressionTable,
        child_type_references: &arena::Arena<TypeReference>,
        receiver: ExpressionHandle,
        call: &TableCallExpression,
        expression: ExpressionHandle,
    ) {
        let (head_symbol, symbol) = resolve_expression_table_receiver_path_symbols(
            symbols,
            machine,
            state_symbol,
            expression_table,
            receiver,
        );
        if symbol.is_valid() {
            stamp_receiver_path_symbols_in_table(expression_table, receiver, head_symbol, symbol);
        }

        let mut target_symbol = resolve_expression_table_call_target_symbol(
            machine,
            parameters,
            state_symbol,
            call,
            expression_table,
            child_type_references,
            symbols,
        );
        if !target_symbol.is_valid()
            && !needs_declared_projection(expression_table, receiver)
            && let Some(receiver_chain) = spelled_receiver_chain(expression_table, receiver)
        {
            target_symbol =
                nested_receiver_call_target_symbol(symbols, machine, &receiver_chain, &call.target);
        }
        if let ExpressionNode::Call(call) = expression_table.expression_mut(expression) {
            call.target_symbol = target_symbol;
            assign_call_machine_argument_symbols(
                symbols,
                machine,
                state_symbol,
                &call.target,
                target_symbol,
                &mut call.machine_arguments,
            );
        }
    }

    pub(in crate::symbols) fn assign_member_symbol(
        symbols: &SymbolTable,
        machine: &MachineScope<'_>,
        state_symbol: SymbolHandle,
        expression_table: &mut ExpressionTable,
        receiver: ExpressionHandle,
        member_name: &DiagnosticName,
        expression: ExpressionHandle,
    ) {
        if matches!(expression_table.expression(expression),
        ExpressionNode::Member(member) if member.case_variant.is_some())
        {
            return;
        }
        let mut member_symbol = resolve_expression_table_member_symbol(
            symbols,
            machine,
            state_symbol,
            expression_table,
            receiver,
            member_name,
        );
        if !member_symbol.is_valid()
            && let Some(mut chain) = spelled_receiver_chain(expression_table, receiver)
        {
            chain.push(member_name.as_str().to_string());
            member_symbol = nested_receiver_leaf_field_symbol(symbols, machine, &chain);
        }
        let case_variant = if let ExpressionNode::SavedSubject(saved) =
            expression_table.expression(receiver)
        {
            expression_table
                .name_path_node(saved.pattern_head)
                .filter(|head| {
                    head.symbol.is_valid() && symbols.get(head.symbol).kind == SymbolKind::Variant
                })
                .and_then(|head| {
                    expression_table
                        .name_path_members(head.members)
                        .last()
                        .cloned()
                })
        } else {
            None
        };
        if let (symbol, ExpressionNode::Member(member)) =
            (member_symbol, expression_table.expression_mut(expression))
            && symbol.is_valid()
        {
            member.member_symbol = symbol;
            member.case_variant = case_variant;
        }
    }

    pub(in crate::symbols) fn assign_membership_symbol(
        symbols: &SymbolTable,
        expression_table: &mut ExpressionTable,
        domain: arena::HandleSpan<DiagnosticName>,
        expression: ExpressionHandle,
    ) {
        let name = expression_table.name_path_members(domain).joined_path();
        let members = expression_table.name_path_members(domain);
        let reference_span = diagnostic_path_source_span(members);
        let selected = crate::symbols::membership_selection(symbols, &name, reference_span);
        let (domain_symbol, case_type_symbol, case_symbol) = match selected {
            Ok(Some(crate::symbols::MembershipSelection::Domain(domain))) => {
                (domain, SymbolHandle::invalid(), SymbolHandle::invalid())
            }
            Ok(Some(crate::symbols::MembershipSelection::Case { owner, case })) => {
                (SymbolHandle::invalid(), owner, case)
            }
            Ok(None) | Err(_) => (
                SymbolHandle::invalid(),
                SymbolHandle::invalid(),
                SymbolHandle::invalid(),
            ),
        };
        if let ExpressionNode::Membership(membership) = expression_table.expression_mut(expression)
        {
            membership.domain_symbol = domain_symbol;
            membership.case_type_symbol = case_type_symbol;
            membership.case_symbol = case_symbol;
        }
    }

    /*
        Select only bindings already in the lexical value frontier. Later locals
        in the state arena must not capture an earlier reference.
    */
    pub(in crate::symbols) fn lexical_value_head(
        machine: &MachineScope<'_>,
        parameters: &[StateParameter],
        member: &str,
        has_suffix: bool,
    ) -> SymbolHandle {
        machine
            .prior_statements
            .iter()
            .rev()
            .find_map(|statement| match statement {
                Statement::LocalData(local) if local.name.as_str() == member => Some(local.symbol),
                _ => None,
            })
            .or_else(|| {
                parameters
                    .iter()
                    .find(|parameter| parameter.name.as_str() == member)
                    .map(|parameter| parameter.symbol)
            })
            .or_else(|| {
                machine
                    .type_parameters
                    .iter()
                    .find(|parameter| {
                        parameter.name.as_str() == member
                            && (matches!(
                                parameter.kind,
                                TypeParameterKind::Type
                                    | TypeParameterKind::Const { .. }
                                    | TypeParameterKind::Value { .. }
                            ) || has_suffix)
                    })
                    .map(|parameter| parameter.symbol)
            })
            .unwrap_or_else(SymbolHandle::invalid)
    }

    pub(in crate::symbols) fn assign_name_symbol(
        symbols: &SymbolTable,
        machine: &MachineScope<'_>,
        parameters: &[StateParameter],
        state_symbol: SymbolHandle,
        expression_table: &mut ExpressionTable,
        path: &TableNamePath,
        expression: ExpressionHandle,
    ) {
        assign_name_in_role(
            symbols,
            machine,
            parameters,
            state_symbol,
            expression_table,
            path,
            expression,
            NameRole::Value,
        );
    }

    pub(in crate::symbols) fn assign_pattern_head_symbol(
        symbols: &SymbolTable,
        machine: &MachineScope<'_>,
        parameters: &[StateParameter],
        state_symbol: SymbolHandle,
        expression_table: &mut ExpressionTable,
        path: &TableNamePath,
        expression: ExpressionHandle,
    ) {
        assign_name_in_role(
            symbols,
            machine,
            parameters,
            state_symbol,
            expression_table,
            path,
            expression,
            NameRole::PatternHead,
        );
    }

    /*
        A selector keeps declaration identity; a value use may construct a
        payload-free case. Namespace and lexical-supplier resolution is shared.
    */
    #[derive(Clone, Copy, PartialEq, Eq)]
    pub(in crate::symbols) enum NameRole {
        Value,
        PatternHead,
    }

    fn assign_name_in_role(
        symbols: &SymbolTable,
        machine: &MachineScope<'_>,
        parameters: &[StateParameter],
        state_symbol: SymbolHandle,
        expression_table: &mut ExpressionTable,
        path: &TableNamePath,
        expression: ExpressionHandle,
        role: NameRole,
    ) {
        if path.static_application.is_valid() {
            return;
        }
        let mut lookup_state = state_symbol;
        if let [member, suffix @ ..] = expression_table.name_path_members(path.members) {
            let symbol =
                lexical_value_head(machine, parameters, member.as_str(), !suffix.is_empty());
            if symbol.is_valid() {
                let member_symbols = resolve_path_members_from_head(symbols, symbol, suffix);
                let leaf = member_symbols
                    .last()
                    .copied()
                    .unwrap_or_else(SymbolHandle::invalid);
                for (offset, member_symbol) in (0u32..).zip(member_symbols) {
                    expression_table.set_name_path_member_symbol_at_offset(
                        path.member_symbols,
                        offset,
                        member_symbol,
                    );
                }
                if let ExpressionNode::Name(path) = expression_table.expression_mut(expression) {
                    path.head_symbol = symbol;
                    path.symbol = leaf;
                }
                return;
            }
            lookup_state = SymbolHandle::invalid();
        }

        let (head_symbol, mut symbol) = resolve_state_scoped_table_path(
            symbols,
            machine.symbol,
            lookup_state,
            expression_table,
            path,
            None,
        );
        if role == NameRole::Value
            && (!head_symbol.is_valid()
                || matches!(
                    symbols.get(head_symbol).kind,
                    SymbolKind::Data | SymbolKind::Module
                ))
            && normalize_qualified_case_value(symbols, expression_table, path, expression)
        {
            return;
        }

        let mut member_symbols = resolve_state_scoped_table_path_member_symbols(
            symbols,
            machine.symbol,
            lookup_state,
            expression_table,
            path,
        );
        if role == NameRole::PatternHead && !symbol.is_valid() {
            let members = expression_table.name_path_members(path.members);
            if members.len() >= 2 {
                let joined = members.joined_path();
                let reference = diagnostic_path_source_span(members);
                if let Ok(Some(crate::symbols::MembershipSelection::Domain(domain))) =
                    crate::symbols::membership_selection(symbols, &joined, reference)
                {
                    symbol = domain;
                }
            }
        }
        if !symbol.is_valid() && head_symbol.is_valid() {
            let members = expression_table.name_path_members(path.members);
            if let [_, suffix @ .., target] = members {
                let mut owner_path = resolve_path_members_from_head(symbols, head_symbol, suffix);
                if let Some(owner) = owner_path.last() {
                    let selected = machine.attached_call_target(symbols, *owner, target);
                    if selected.is_valid() {
                        owner_path.push(selected);
                        member_symbols = owner_path;
                        symbol = selected;
                    }
                }
            }
        }
        if symbol.is_valid()
            && member_symbols.len() == path.members.count() as usize
            && member_symbols.last().is_some_and(|last| last.is_valid())
        {
            for (offset, member_symbol) in member_symbols.into_iter().enumerate() {
                expression_table.set_name_path_member_symbol_at_offset(
                    path.member_symbols,
                    offset
                        .try_into()
                        .expect("name path member symbol count overflow"),
                    member_symbol,
                );
            }
        }
        if head_symbol.is_valid()
            && let ExpressionNode::Name(path) = expression_table.expression_mut(expression)
        {
            path.head_symbol = head_symbol;
            path.symbol = symbol;
        }
    }

    /*
        Qualified payload-free case values and braces share construction after lexical binding has
        selected the value frontier. Package aliases need no invented declaration
        symbols: the existing constructor selector retains their source authority.
        Membership has a separate domain-path node and never enters this value route.
    */
    pub(in crate::symbols) fn normalize_qualified_case_value(
        symbols: &SymbolTable,
        expression_table: &mut ExpressionTable,
        path: &TableNamePath,
        expression: ExpressionHandle,
    ) -> bool {
        let members = expression_table.name_path_members(path.members);
        if members.len() < 2 {
            return false;
        }
        let name = members.joined_path();
        let source_span = diagnostic_path_source_span(members);
        let (owner, case_symbol, case) =
            match crate::symbols::qualified_payload_free_case_type(symbols, &name, source_span) {
                Ok(Some(selected)) => selected,
                Ok(None) => return false,
                Err(_) => return true,
            };
        let case_name = members
            .last()
            .expect("case path has at least two members")
            .clone();
        let owner_name = &name[..name.len() - case.len() - 2];
        *expression_table.expression_mut(expression) =
            ExpressionNode::StructLiteral(TableStructLiteral {
                application_type: Default::default(),
                type_name: DiagnosticName::new(owner_name, source_span),
                type_symbol: owner,
                case_name: Some(case_name),
                case_symbol: Some(case_symbol),
                fields: Default::default(),
            });
        true
    }

    pub(in crate::symbols) fn assign_struct_literal_symbols(
        symbols: &SymbolTable,
        expression_table: &mut ExpressionTable,
        expression: ExpressionHandle,
    ) {
        let ExpressionNode::StructLiteral(literal) =
            expression_table.expression(expression).clone()
        else {
            return;
        };
        let (type_name, case_name, type_symbol) = if let Some(case_name) = &literal.case_name {
            (
                literal.type_name.clone(),
                Some(case_name.clone()),
                top_level_symbol_for_source(symbols, SymbolKind::Data, &literal.type_name),
            )
        } else if let Ok(Some((symbol, case))) = crate::symbols::constructor_type(
            symbols,
            literal.type_name.as_str(),
            literal.type_name.source_span(),
        ) {
            let name = literal.type_name.as_str();
            let source_span = literal.type_name.source_span();
            match case {
                Some(case) => {
                    let owner = &name[..name.len() - case.len() - 2];
                    let mut case_span = source_span;
                    if case_span.span.end >= case.len() {
                        case_span.span.start = case_span.span.end - case.len();
                    }
                    (
                        DiagnosticName::new(owner, source_span),
                        Some(DiagnosticName::new(case, case_span)),
                        symbol,
                    )
                }
                None => (literal.type_name.clone(), None, symbol),
            }
        } else {
            (literal.type_name.clone(), None, SymbolHandle::invalid())
        };
        let case_symbol = case_name.as_ref().map(|case_name| {
            if type_symbol.is_valid() {
                child_symbol_by_kinds(
                    symbols,
                    type_symbol,
                    &[SymbolKind::Variant],
                    case_name.as_str(),
                )
            } else {
                SymbolHandle::invalid()
            }
        });
        let field_symbols = expression_table
            .struct_fields(literal.fields)
            .iter()
            .map(|field| {
                let case_field = case_symbol
                    .filter(|case_symbol| case_symbol.is_valid())
                    .map(|case_symbol| {
                        child_symbol_by_kinds(
                            symbols,
                            case_symbol,
                            &[SymbolKind::Field],
                            field.name.as_str(),
                        )
                    })
                    .unwrap_or_else(SymbolHandle::invalid);
                if case_field.is_valid() {
                    case_field
                } else if type_symbol.is_valid() {
                    child_symbol_by_kinds(
                        symbols,
                        type_symbol,
                        &[SymbolKind::Field],
                        field.name.as_str(),
                    )
                } else {
                    SymbolHandle::invalid()
                }
            })
            .collect::<Vec<_>>();

        for (offset, field_symbol) in field_symbols.into_iter().enumerate() {
            let field = expression_table.struct_fields(literal.fields)[offset].clone();
            expression_table.set_struct_field_at_offset(
                literal.fields,
                offset
                    .try_into()
                    .expect("struct literal field count overflow"),
                TableStructLiteralField {
                    field_symbol,
                    ..field
                },
            );
        }
        if let ExpressionNode::StructLiteral(literal) = expression_table.expression_mut(expression)
        {
            literal.type_name = type_name;
            literal.case_name = case_name;
            literal.type_symbol = type_symbol;
            literal.case_symbol = case_symbol;
        }
    }
}
mod traversal {

    use arena::HandleSpan;
    use symbols::{SymbolHandle, SymbolTable};

    use super::references::{
        assign_call_symbol, assign_member_symbol, assign_membership_symbol, assign_name_symbol,
        assign_pattern_head_symbol, assign_struct_literal_symbols,
    };
    use crate::symbols::expression_paths::assign_pattern_field_symbols;
    use crate::symbols::scope::MachineScope;
    use symbol_resolved_trees::expression::ExpressionHandle;
    use symbol_resolved_trees::expression::ExpressionNode;
    use symbol_resolved_trees::expression::ExpressionTable;
    use symbol_resolved_trees::expression::MatchPattern;
    use symbol_resolved_trees::expression::StaticMachineArgument;
    use symbol_resolved_trees::expression::TableMatchArm;
    use symbol_resolved_trees::expression::TableMatchExpression;
    use symbol_resolved_trees::signature::StateParameter;
    use symbol_resolved_trees::types::TypeReference;

    /*
        Public entry into a walk: reborrow the shared tables and scope into the
        visit context, then run the named method on the carried value.
    */
    macro_rules! context_wrapped {
        ($name:ident ($parameter:ident : $ty:ty) => $method:ident) => {
            pub(in crate::symbols) fn $name(
                symbols: &SymbolTable,
                machine: &MachineScope<'_>,
                parameters: &[StateParameter],
                state_symbol: SymbolHandle,
                expression_table: &mut ExpressionTable,
                child_type_references: &mut arena::Arena<TypeReference>,
                $parameter: $ty,
            ) {
                ExpressionSymbolContext {
                    symbols,
                    machine,
                    parameters,
                    state_symbol,
                    expression_table,
                    child_type_references,
                    declaration_references: &[],
                }
                .$method($parameter);
            }
        };
    }

    context_wrapped! {
        assign_pattern_head_symbols(expression: ExpressionHandle) => assign_pattern_head
    }

    context_wrapped! {
        assign_static_argument_type_symbols(
            argument: &StaticMachineArgument
        ) => assign_static_argument_type_symbols
    }

    /*
        Assign static-argument type symbols for every machine argument the call
        carries.
    */
    #[allow(clippy::too_many_arguments)]
    pub(in crate::symbols) fn assign_static_arguments_type_symbols(
        symbols: &SymbolTable,
        machine: &MachineScope<'_>,
        parameters: &[StateParameter],
        state_symbol: SymbolHandle,
        expression_table: &mut ExpressionTable,
        child_type_references: &mut arena::Arena<TypeReference>,
        machine_arguments: &[StaticMachineArgument],
    ) {
        for argument in machine_arguments {
            assign_static_argument_type_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                argument,
            );
        }
    }

    pub(in crate::symbols) fn assign_statement_expression_symbols(
        symbols: &SymbolTable,
        machine: &MachineScope<'_>,
        parameters: &[StateParameter],
        state_symbol: SymbolHandle,
        expression_table: &mut ExpressionTable,
        child_type_references: &mut arena::Arena<TypeReference>,
        expression: ExpressionHandle,
    ) {
        if !expression.is_valid() {
            return;
        }

        assign_expression_table_symbols(
            symbols,
            machine,
            parameters,
            state_symbol,
            expression_table,
            child_type_references,
            expression,
        );
    }

    /*
        Alias RHS references retain their exact authored occurrences independently
        from substituted caller inputs. Only those references use declaration lookup;
        the same expression walk resolves every other leaf in its ordinary context.
    */
    #[allow(clippy::too_many_arguments)]
    pub(in crate::symbols) fn assign_expression_with_declaration_references(
        symbols: &SymbolTable,
        machine: &MachineScope<'_>,
        parameters: &[StateParameter],
        state_symbol: SymbolHandle,
        expression_table: &mut ExpressionTable,
        child_type_references: &mut arena::Arena<TypeReference>,
        expression: ExpressionHandle,
        declaration_references: &[source::SourceSpan],
    ) {
        ExpressionSymbolContext {
            symbols,
            machine,
            parameters,
            state_symbol,
            expression_table,
            child_type_references,
            declaration_references,
        }
        .assign_expression_table_symbols(expression);
    }

    context_wrapped! {
        assign_expression_span_symbols(
            expressions: HandleSpan<ExpressionHandle>
        ) => assign_expression_span_symbols
    }

    context_wrapped! {
        assign_expression_table_symbols(
            expression: ExpressionHandle
        ) => assign_expression_table_symbols
    }

    /*
        Each recursive visit reborrows the same tables and lexical scope. Keep the
        indexed span reads and independent type-reference checks at their visit sites:
        earlier recursive visits may mutate the tables that later visits read.
    */
    struct ExpressionSymbolContext<'visit, 'scope> {
        symbols: &'visit SymbolTable,
        machine: &'visit MachineScope<'scope>,
        parameters: &'visit [StateParameter],
        state_symbol: SymbolHandle,
        expression_table: &'visit mut ExpressionTable,
        child_type_references: &'visit mut arena::Arena<TypeReference>,
        declaration_references: &'visit [source::SourceSpan],
    }

    impl ExpressionSymbolContext<'_, '_> {
        /*
            Reassigns one type reference with this context's symbol table, machine
            scope, parameters, state symbol, and child arenas — every walk's call.
        */
        fn reassign_type_reference(&mut self, type_reference: arena::Handle<TypeReference>) {
            if !type_reference.is_valid() {
                return;
            }
            crate::symbols::type_references::reassign_type_reference_symbols_and_values(
                self.symbols,
                self.machine,
                self.parameters,
                self.state_symbol,
                self.expression_table,
                self.child_type_references,
                type_reference,
            );
        }

        fn assign_static_argument_type_symbols(&mut self, argument: &StaticMachineArgument) {
            if argument.type_reference.is_valid() {
                self.reassign_type_reference(argument.type_reference);
            }
            if let Some(application) = &argument.application {
                for nested in &application.arguments {
                    self.assign_static_argument_type_symbols(nested);
                }
                if let Some(member) = self
                    .expression_table
                    .static_member(application.member)
                    .cloned()
                {
                    self.assign_static_argument_type_symbols(&member);
                }
            }
        }

        fn assign_declaration_argument_symbols(&mut self, argument: &mut StaticMachineArgument) {
            if argument.type_reference.is_valid() {
                self.reassign_type_reference(argument.type_reference);
            }
            crate::symbols::targets::assign_static_argument_symbol(
                self.symbols,
                self.machine.symbol,
                self.machine.type_parameters,
                argument,
                true,
            );
            if let Some(application) = &mut argument.application {
                for nested in &mut application.arguments {
                    self.assign_declaration_argument_symbols(nested);
                }
                self.assign_member_argument_symbols(argument.symbol, application.member);
            }
        }

        fn assign_member_argument_symbols(
            &mut self,
            owner: SymbolHandle,
            handle: arena::Handle<StaticMachineArgument>,
        ) {
            let Some(mut member) = self.expression_table.static_member(handle).cloned() else {
                return;
            };
            // The member itself belongs to its applied owner, not the lexical
            // namespace. Its supplied arguments still have ordinary lexical scope.
            // A known data declaration already owns its member identities;
            // only their substituted types need the later application owner.
            // Type-dependent selections remain deferred when no such owner exists.
            if self.symbols.symbols().nodes().is_valid(owner)
                && matches!(
                    self.symbols.get(owner).kind,
                    symbols::SymbolKind::Data | symbols::SymbolKind::Variant
                )
            {
                let selected = crate::symbols::scoped_paths::resolve_path_members_from_head(
                    self.symbols,
                    owner,
                    &member.path,
                );
                member.symbol = selected
                    .last()
                    .copied()
                    .filter(|_| !member.path.is_empty())
                    .filter(|_| selected.len() == member.path.len() + 1)
                    .filter(|selected| {
                        member.path.last().is_some_and(|name| {
                            self.symbols
                                .source_reference_may_select_symbol(name.source_span(), *selected)
                        })
                    })
                    .unwrap_or_else(SymbolHandle::invalid);
            }
            if let Some(application) = &mut member.application {
                for argument in &mut application.arguments {
                    self.assign_declaration_argument_symbols(argument);
                }
                self.assign_member_argument_symbols(member.symbol, application.member);
            }
            *self
                .expression_table
                .static_member_mut(handle)
                .expect("selected member belongs to this expression table") = member;
        }

        fn assign_expression_span_symbols(&mut self, expressions: HandleSpan<ExpressionHandle>) {
            let count = expressions.count();
            for offset in 0..count {
                let expression =
                    self.expression_table.expression_handles(expressions)[offset as usize];
                self.assign_expression_table_symbols(expression);
            }
        }

        fn assign_pattern_head(&mut self, expression: ExpressionHandle) {
            if !expression.is_valid() {
                return;
            }
            if let ExpressionNode::Name(path) = self.expression_table.expression(expression).clone()
            {
                if self
                    .declaration_references
                    .contains(&self.expression_table.source_span(expression))
                {
                    let declaration = self.machine.declaration_scope(SymbolHandle::invalid(), &[]);
                    assign_pattern_head_symbol(
                        self.symbols,
                        &declaration,
                        &[],
                        SymbolHandle::invalid(),
                        self.expression_table,
                        &path,
                        expression,
                    );
                    return;
                }
                assign_pattern_head_symbol(
                    self.symbols,
                    self.machine,
                    self.parameters,
                    self.state_symbol,
                    self.expression_table,
                    &path,
                    expression,
                );
            } else {
                self.assign_expression_table_symbols(expression);
            }
        }

        fn assign_expression_table_symbols(&mut self, expression: ExpressionHandle) {
            if !expression.is_valid() {
                return;
            }

            match self.expression_table.expression(expression).clone() {
                ExpressionNode::Match(mut dispatch) => {
                    self.assign_expression_table_symbols(dispatch.subject);
                    for offset in 0..dispatch.arms.count() {
                        let mut arm =
                            self.expression_table.match_arms(dispatch.arms)[offset as usize];
                        let dependencies = self
                            .expression_table
                            .match_pattern_dependencies(arm.pattern)
                            .collect::<Vec<_>>();
                        let head = match arm.pattern {
                            MatchPattern::Value(head) => head,
                            MatchPattern::Destructure(pattern) => pattern.head,
                            MatchPattern::Wildcard => Default::default(),
                        };
                        for pattern in dependencies {
                            if pattern == head {
                                self.assign_pattern_head(pattern);
                            } else {
                                self.assign_expression_table_symbols(pattern);
                            }
                        }
                        self.normalize_payload_free_case_pattern(&mut dispatch, &mut arm);
                        self.normalize_domain_pattern(&mut dispatch, &mut arm);
                        self.expression_table
                            .match_arm_mut(dispatch.arms.handle_at(offset))
                            .pattern = arm.pattern;
                        if let MatchPattern::Destructure(pattern) = arm.pattern {
                            self.reassign_type_reference(pattern.application_type);
                            assign_pattern_field_symbols(
                                self.symbols,
                                self.expression_table,
                                pattern,
                            );
                        }
                        self.assign_expression_table_symbols(arm.value);
                    }
                    *self.expression_table.expression_mut(expression) =
                        ExpressionNode::Match(dispatch);
                }
                ExpressionNode::SavedSubject(saved) => {
                    if self.expression_table.match_subject(saved.capture).purpose
                        == symbol_resolved_trees::expression::SubjectCapturePurpose::Transition
                    {
                        let subject = self.expression_table.match_subject(saved.capture).subject;
                        self.assign_expression_table_symbols(subject);
                        self.assign_pattern_head(saved.pattern_head);
                    }
                }
                ExpressionNode::Atomic(atomic) => {
                    self.assign_expression_table_symbols(atomic.result);
                    self.assign_expression_table_symbols(atomic.value);
                }
                ExpressionNode::ArrayLiteral(values) => {
                    self.assign_expression_span_symbols(values);
                }
                ExpressionNode::Binary(binary) => {
                    self.assign_expression_table_symbols(binary.left);
                    self.assign_expression_table_symbols(binary.right);
                }
                ExpressionNode::Boolean(_)
                | ExpressionNode::Float(_)
                | ExpressionNode::UndeterminedNaN(_)
                | ExpressionNode::Integer(_)
                | ExpressionNode::String(_) => {}
                ExpressionNode::Cast(cast) => {
                    self.assign_expression_table_symbols(cast.value);
                    self.reassign_type_reference(cast.target_type);
                    for handle in cast.semantic_domain_arguments.handles() {
                        self.reassign_type_reference(handle);
                    }
                }
                ExpressionNode::Call(call) => {
                    if call.callee.is_valid() {
                        self.assign_expression_table_symbols(call.callee);
                    }
                    for argument in &call.machine_arguments {
                        self.assign_static_argument_type_symbols(argument);
                    }
                    if call.requirement_application.is_valid() {
                        self.reassign_type_reference(call.requirement_application);
                    }
                    if call.receiver.is_valid() {
                        self.assign_expression_table_symbols(call.receiver);
                    }
                    self.assign_expression_span_symbols(call.arguments);
                    assign_call_symbol(
                        self.symbols,
                        self.machine,
                        self.parameters,
                        self.state_symbol,
                        self.expression_table,
                        self.child_type_references,
                        call.receiver,
                        &call,
                        expression,
                    );
                }
                ExpressionNode::Indexed(indexed) => {
                    self.assign_expression_table_symbols(indexed.collection);
                    self.assign_expression_table_symbols(indexed.index);
                }
                ExpressionNode::Range(range) => {
                    if range.start.is_valid() {
                        self.assign_expression_table_symbols(range.start);
                    }
                    if range.end.is_valid() {
                        self.assign_expression_table_symbols(range.end);
                    }
                }
                ExpressionNode::Member(member) => {
                    let receiver = member.receiver;
                    let member_name = member.member.clone();
                    self.assign_expression_table_symbols(receiver);
                    assign_member_symbol(
                        self.symbols,
                        self.machine,
                        self.state_symbol,
                        self.expression_table,
                        receiver,
                        &member_name,
                        expression,
                    );
                }
                ExpressionNode::Membership(membership) => {
                    self.assign_expression_table_symbols(membership.value);
                    assign_membership_symbol(
                        self.symbols,
                        self.expression_table,
                        membership.domain,
                        expression,
                    );
                }
                ExpressionNode::Borrow(inner) => {
                    self.assign_expression_table_symbols(inner.target);
                }
                ExpressionNode::Unary(unary) => {
                    self.assign_expression_table_symbols(unary.operand);
                }
                ExpressionNode::Name(path) => {
                    if path.static_application.is_valid()
                        && let Some(mut selected) = self
                            .expression_table
                            .static_member(path.static_application)
                            .cloned()
                    {
                        self.assign_declaration_argument_symbols(&mut selected);
                        *self
                            .expression_table
                            .static_member_mut(path.static_application)
                            .expect("selected declaration application belongs to this table") =
                            selected;
                    }
                    if self
                        .declaration_references
                        .contains(&self.expression_table.source_span(expression))
                    {
                        let declaration =
                            self.machine.declaration_scope(SymbolHandle::invalid(), &[]);
                        assign_name_symbol(
                            self.symbols,
                            &declaration,
                            &[],
                            SymbolHandle::invalid(),
                            self.expression_table,
                            &path,
                            expression,
                        );
                        return;
                    }
                    assign_name_symbol(
                        self.symbols,
                        self.machine,
                        self.parameters,
                        self.state_symbol,
                        self.expression_table,
                        &path,
                        expression,
                    );
                }
                ExpressionNode::StructLiteral(struct_literal) => {
                    self.reassign_type_reference(struct_literal.application_type);
                    let count = struct_literal.fields.count();
                    for offset in 0..count {
                        let field = &self.expression_table.struct_fields(struct_literal.fields)
                            [offset as usize];
                        self.assign_expression_table_symbols(field.value);
                    }
                    assign_struct_literal_symbols(self.symbols, self.expression_table, expression);
                }
                ExpressionNode::TypeExpression(type_reference)
                | ExpressionNode::ZeroValue(type_reference) => {
                    self.reassign_type_reference(type_reference);
                }
            }
        }

        /// A bare, resolved payload-free case observes the same case as empty
        /// braces. Complete its subject capture here, where the selected symbol
        /// and its real payload are known, before typing consumes the pattern.
        /// Value calls and applied heads keep their own representation; no
        /// spelling lookup or implicit waiver can substitute for this selection.
        fn normalize_payload_free_case_pattern(
            &mut self,
            dispatch: &mut TableMatchExpression,
            arm: &mut TableMatchArm,
        ) {
            use symbol_resolved_trees::expression::{
                ExpressionHandle, ExpressionNode, MatchPattern, TableDestructurePattern,
            };
            let MatchPattern::Value(head) = arm.pattern else {
                return;
            };
            let Some(path) = self.expression_table.name_path_node(head) else {
                return;
            };
            let selected = path.symbol;
            if path.static_application.is_valid()
                || !selected.is_valid()
                || self.symbols.get(selected).kind != symbols::SymbolKind::Variant
                || self
                    .symbols
                    .child_handles(selected)
                    .is_none_or(|mut children| {
                        children
                            .any(|child| self.symbols.get(child).kind == symbols::SymbolKind::Field)
                    })
            {
                return;
            }
            if !dispatch.capture.is_valid() {
                dispatch.capture = self.expression_table.insert_match_subject(dispatch.subject);
            }
            let condition = self.expression_table.insert(ExpressionNode::Boolean(true));
            self.expression_table
                .set_source_span(condition, self.expression_table.source_span(head));
            if let Some(exposure) = self.expression_table.authored_expression_exposure(head) {
                self.expression_table
                    .set_authored_expression_exposure(condition, exposure);
            }
            if let Some(partition) = self.expression_table.compiler_selection_partition(head) {
                self.expression_table
                    .set_compiler_selection_partition(condition, partition);
            }
            arm.pattern = MatchPattern::Destructure(TableDestructurePattern {
                application_type: Default::default(),
                head,
                fields: HandleSpan::empty(),
                has_rest: false,
                guard: ExpressionHandle::invalid(),
                condition,
                proof_selectors: HandleSpan::empty(),
            });
        }

        /*
            A selected domain head is an ordinary predicate on the one captured
            subject. Normalize it where declaration identity is known, before the
            authored-selection ledger and typing consume the expression graph. The
            membership node therefore retains the same visibility, provenance and
            executable-predicate obligations as an authored `subject in Domain`.
        */
        fn normalize_domain_pattern(
            &mut self,
            dispatch: &mut TableMatchExpression,
            arm: &mut TableMatchArm,
        ) {
            use symbol_resolved_trees::expression::{
                BinaryOperator, ExpressionHandle, ExpressionNode, MatchPattern,
                TableBinaryExpression, TableDestructurePattern, TableMembershipExpression,
                TableSavedSubject,
            };
            use symbols::JoinedPath;
            let (head, previous) = match arm.pattern {
                MatchPattern::Value(head) => (head, None),
                MatchPattern::Destructure(pattern) => (pattern.head, Some(pattern)),
                MatchPattern::Wildcard => return,
            };
            let Some(path) = self.expression_table.name_path_node(head).cloned() else {
                return;
            };
            let members = self.expression_table.name_path_members(path.members);
            let domain = match crate::symbols::membership_selection(
                self.symbols,
                &members.joined_path(),
                self.expression_table.source_span(head),
            ) {
                Ok(Some(crate::symbols::MembershipSelection::Domain(domain))) => domain,
                Err(_) => SymbolHandle::invalid(),
                _ => return,
            };
            if let Some(previous) = previous {
                let mut condition = previous.condition;
                while let ExpressionNode::Binary(binary) =
                    self.expression_table.expression(condition)
                {
                    if binary.operator != BinaryOperator::And {
                        break;
                    }
                    condition = binary.left;
                }
                if let ExpressionNode::Membership(membership) =
                    self.expression_table.expression(condition)
                    && membership.domain_symbol == domain
                    && let ExpressionNode::SavedSubject(saved) =
                        self.expression_table.expression(membership.value)
                    && saved.capture == dispatch.capture
                    && saved.pattern_head == head
                {
                    return;
                }
            }
            if let ExpressionNode::Name(path) = self.expression_table.expression_mut(head) {
                path.symbol = domain;
            }
            if let Some(offset) = path.member_symbols.count().checked_sub(1) {
                self.expression_table.set_name_path_member_symbol_at_offset(
                    path.member_symbols,
                    offset,
                    domain,
                );
            }
            if !dispatch.capture.is_valid() {
                dispatch.capture = self.expression_table.insert_match_subject(dispatch.subject);
            }
            let saved =
                self.expression_table
                    .insert(ExpressionNode::SavedSubject(TableSavedSubject {
                        component: Default::default(),
                        capture: dispatch.capture,
                        pattern_head: head,
                    }));
            let condition = self.expression_table.insert(ExpressionNode::Membership(
                TableMembershipExpression {
                    is_pattern: false,
                    value: saved,
                    domain: path.members,
                    domain_symbol: domain,
                    case_type_symbol: SymbolHandle::invalid(),
                    case_symbol: SymbolHandle::invalid(),
                },
            ));
            self.expression_table
                .set_source_span(condition, self.expression_table.source_span(head));
            if let Some(exposure) = self.expression_table.authored_expression_exposure(head) {
                self.expression_table
                    .set_authored_expression_exposure(condition, exposure);
            }
            if let Some(partition) = self.expression_table.compiler_selection_partition(head) {
                self.expression_table
                    .set_compiler_selection_partition(condition, partition);
            }
            arm.pattern = MatchPattern::Destructure(if let Some(previous) = previous {
                let condition =
                    self.expression_table
                        .insert(ExpressionNode::Binary(TableBinaryExpression {
                            policy: numerics::arithmetic::ArithmeticDomain::Exact,
                            left: condition,
                            operator: BinaryOperator::And,
                            right: previous.condition,
                        }));
                TableDestructurePattern {
                    condition,
                    ..previous
                }
            } else {
                TableDestructurePattern {
                    application_type: Default::default(),
                    head,
                    fields: HandleSpan::empty(),
                    has_rest: true,
                    guard: ExpressionHandle::invalid(),
                    condition,
                    proof_selectors: HandleSpan::empty(),
                }
            });
        }
    }
}

pub(crate) use references::reject_unqualified_case_values;
pub(super) use references::{
    NameRole, assign_member_symbol, assign_membership_symbol, assign_name_symbol,
    assign_struct_literal_symbols, lexical_value_head, normalize_qualified_case_value,
};
pub(super) use traversal::{
    assign_expression_span_symbols, assign_expression_table_symbols,
    assign_expression_with_declaration_references, assign_pattern_head_symbols,
    assign_statement_expression_symbols, assign_static_arguments_type_symbols,
};

#[cfg(test)]
mod case_pattern_tests {
    use std::sync::Arc;
    use symbol_resolved_trees::expression::{ExpressionNode, MatchPattern, TableMatchExpression};

    fn resolved(text: &str) -> symbol_resolved_trees::SymbolResolvedTrees {
        let mut sources = source::SourceMap::default();
        let source = sources
            .add_with_metadata(
                "cases/main.omg".into(),
                text.to_owned(),
                "cases".into(),
                None,
                source::SourceOrigin::User,
            )
            .source_id;
        let tokens = source_files_to_tokens::Lexer::new(text).tokenize().unwrap();
        let syntax = tokens_to_syntax_trees::parse_syntax_trees_with_id(source, &tokens).unwrap();
        crate::resolution::resolve(crate::resolution::ResolutionRequest {
            syntax,
            sources: Some(Arc::new(sources)),
            top_level_bindings: Vec::new(),
            retained_base: None,
        })
        .unwrap()
    }

    fn dispatch(program: &symbol_resolved_trees::SymbolResolvedTrees) -> TableMatchExpression {
        program
            .tables
            .bodies
            .expressions
            .iter_expressions()
            .find_map(|(_, node)| {
                if let ExpressionNode::Match(dispatch) = node {
                    Some(*dispatch)
                } else {
                    None
                }
            })
            .expect("one real source match")
    }

    #[test]
    fn bare_cases_keep_the_same_exact_subject_capture_as_empty_braces() {
        let source = "data Choice [copy] { case Left; case Right; }\n\
            machine choose(value: Choice) -> u32 {\n\
                match value { Choice::Left -> 1 Choice::Right -> 2 }\n\
            }";
        for braces in [false, true] {
            let source = if braces {
                source
                    .replace("Choice::Left ->", "Choice::Left {} ->")
                    .replace("Choice::Right ->", "Choice::Right {} ->")
            } else {
                source.to_owned()
            };
            let program = resolved(&source);
            let matched = dispatch(&program);
            let table = &program.tables.bodies.expressions;
            assert!(matched.capture.is_valid());
            assert_eq!(
                table.match_subject(matched.capture).subject,
                matched.subject
            );
            for arm in table.match_arms(matched.arms) {
                let MatchPattern::Destructure(pattern) = arm.pattern else {
                    panic!("case pattern")
                };
                assert!(pattern.fields.is_empty() && !pattern.has_rest);
                assert!(matches!(
                    table.expression(pattern.condition),
                    ExpressionNode::Boolean(true)
                ));
                let head = table.name_path_node(pattern.head).unwrap();
                assert_eq!(
                    program.symbols.get(head.symbol).kind,
                    symbols::SymbolKind::Variant
                );
                assert_eq!(
                    program
                        .symbols
                        .name(program.symbols.get(head.symbol).parent),
                    "Choice"
                );
            }
        }
    }

    #[test]
    fn a_same_named_foreign_case_keeps_its_actual_owner() {
        let program = resolved(
            "data Choice [copy] { case Left; case Right; }\n\
            data Other [copy] { case Left; }\n\
            machine choose(value: Choice) -> u32 {\n\
                match value { Other::Left -> 1 _ -> 2 }\n\
            }",
        );
        let table = &program.tables.bodies.expressions;
        let matched = dispatch(&program);
        let MatchPattern::Destructure(pattern) = table.match_arms(matched.arms)[0].pattern else {
            panic!("selected foreign case")
        };
        let selected = table.name_path_node(pattern.head).unwrap().symbol;
        assert_eq!(
            program.symbols.name(program.symbols.get(selected).parent),
            "Other",
            "normalization must not substitute the subject's same-spelled case"
        );
    }

    #[test]
    fn payload_cases_and_value_calls_are_not_empty_case_patterns() {
        for source in [
            "data Choice [copy] { case Payload(value: u32); }\n\
                machine choose(value: Choice) -> u32 { match value { Choice::Payload -> 1 _ -> 2 } }",
            "data Choice [copy] { case Left; }\n\
                machine candidate() -> Choice { Choice::Left {} }\n\
                machine choose(value: Choice) -> u32 { match value { candidate() -> 1 _ -> 2 } }",
        ] {
            let program = resolved(source);
            let table = &program.tables.bodies.expressions;
            let matched = dispatch(&program);
            assert!(!matched.capture.is_valid());
            assert!(
                matches!(
                    table.match_arms(matched.arms)[0].pattern,
                    MatchPattern::Value(_)
                ),
                "a payload or value computation cannot become an empty case observation"
            );
        }
    }
}
