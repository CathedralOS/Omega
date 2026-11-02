use arena::HandleSpan;
use symbols::{SymbolHandle, SymbolTable};

use super::references::{
    assign_call_symbol, assign_member_symbol, assign_membership_symbol, assign_name_symbol,
    assign_struct_literal_symbols,
};
use crate::symbols::expression_paths::pattern_field_symbol;
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

/// Public entry into a walk: reborrow the shared tables and scope into the
/// visit context, then run the named method on the carried value.
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
    assign_static_argument_type_symbols(
        argument: &StaticMachineArgument
    ) => assign_static_argument_type_symbols
}

/// Assign static-argument type symbols for every machine argument the call
/// carries.
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

/// Alias RHS references retain their exact authored occurrences independently
/// from substituted caller inputs. Only those references use declaration lookup;
/// the same expression walk resolves every other leaf in its ordinary context.
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

// Each recursive visit reborrows the same tables and lexical scope. Keep the
// indexed span reads and independent type-reference checks at their visit sites:
// earlier recursive visits may mutate the tables that later visits read.
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
    /// Reassigns one type reference with this context's symbol table, machine
    /// scope, parameters, state symbol, and child arenas — every walk's call.
    fn reassign_type_reference(&mut self, type_reference: arena::Handle<TypeReference>) {
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

    fn assign_expression_span_symbols(&mut self, expressions: HandleSpan<ExpressionHandle>) {
        let count = expressions.count();
        for offset in 0..count {
            let expression = self.expression_table.expression_handles(expressions)[offset as usize];
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
                    let mut arm = self.expression_table.match_arms(dispatch.arms)[offset as usize];
                    let dependencies = self
                        .expression_table
                        .match_pattern_dependencies(arm.pattern)
                        .collect::<Vec<_>>();
                    for pattern in dependencies {
                        self.assign_expression_table_symbols(pattern);
                    }
                    self.normalize_domain_pattern(&mut dispatch, &mut arm);
                    self.expression_table
                        .match_arm_mut(dispatch.arms.handle_at(offset))
                        .pattern = arm.pattern;
                    if let MatchPattern::Destructure(pattern) = arm.pattern {
                        let head = self
                            .expression_table
                            .name_path_node(pattern.head)
                            .map(|path| path.symbol)
                            .unwrap_or_default();
                        for position in 0..pattern.fields.count() {
                            let field = pattern.fields.handle_at(position);
                            let spelling = self.expression_table.match_fields(pattern.fields)
                                [position as usize]
                                .member
                                .as_str();
                            let selected = pattern_field_symbol(self.symbols, head, spelling);
                            self.expression_table.match_field_mut(field).field_symbol = selected;
                        }
                    }
                    self.assign_expression_table_symbols(arm.value);
                }
                *self.expression_table.expression_mut(expression) = ExpressionNode::Match(dispatch);
            }
            // The definition's subject and each head are visited by Match.
            // An alias is not another use/evaluation of that source subtree.
            ExpressionNode::SavedSubject(_) => {}
            ExpressionNode::Atomic(atomic) => {
                // The instruction-observed prior has its own destination expression,
                // distinct from references inside the arithmetic-shaped carrier.
                // Resolve it in the same state scope so downstream consumers bind
                // the declared local by identity, never by retained spelling.
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
                // The source span rejoins the selected alias's original RHS
                // occurrence, not a spelling-based declaration classification.
                if self
                    .declaration_references
                    .contains(&self.expression_table.source_span(expression))
                {
                    let declaration = self.machine.declaration_scope(SymbolHandle::invalid(), &[]);
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

    /// A selected domain head is an ordinary predicate on the one captured
    /// subject. Normalize it where declaration identity is known, before the
    /// authored-selection ledger and typing consume the expression graph. The
    /// membership node therefore retains the same visibility, provenance and
    /// executable-predicate obligations as an authored `subject in Domain`.
    fn normalize_domain_pattern(
        &mut self,
        dispatch: &mut TableMatchExpression,
        arm: &mut TableMatchArm,
    ) {
        use symbol_resolved_trees::expression::{
            BinaryOperator, ExpressionHandle, ExpressionNode, MatchPattern, TableBinaryExpression,
            TableDestructurePattern, TableMembershipExpression, TableSavedSubject,
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
        // Domains are declaration-owned qualified names, not necessarily
        // children of their data carrier. Ordinary value-path lookup may
        // therefore leave this head unresolved. Use membership's one exact
        // selection entrance, including imports and ambiguity handling.
        let members = self.expression_table.name_path_members(path.members);
        let domain = match crate::symbols::membership_selection(
            self.symbols,
            &members.joined_path(),
            self.expression_table.source_span(head),
        ) {
            Ok(Some(crate::symbols::MembershipSelection::Domain(domain))) => domain,
            // Retain an ambiguous membership for the fallible authored-
            // selection ledger to diagnose before typing, never a case fallback.
            Err(_) => SymbolHandle::invalid(),
            _ => return,
        };
        // Symbol assignment can revisit a shared expression. The normalized
        // membership is the first test and authenticates this exact capture
        // and head, so normalization remains idempotent.
        if let Some(previous) = previous {
            let mut condition = previous.condition;
            while let ExpressionNode::Binary(binary) = self.expression_table.expression(condition) {
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
        let saved = self
            .expression_table
            .insert(ExpressionNode::SavedSubject(TableSavedSubject {
                capture: dispatch.capture,
                pattern_head: head,
            }));
        let condition =
            self.expression_table
                .insert(ExpressionNode::Membership(TableMembershipExpression {
                    value: saved,
                    domain: path.members,
                    domain_symbol: domain,
                    case_type_symbol: SymbolHandle::invalid(),
                    case_symbol: SymbolHandle::invalid(),
                }));
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
