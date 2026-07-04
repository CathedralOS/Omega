use omega_checked_trees::CheckedTrees;
use omega_checked_trees::expression::{
    BinaryExpression, CallExpression, Expression, ExpressionHandle, ExpressionNode,
    ExpressionTable, IndexedExpression, MemberExpression, NamePath,
};
use omega_checked_trees::state::State;
use omega_checked_trees::statement::StatementNode;
use omega_core::arena::Arena;
use omega_core::symbols::SymbolHandle;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub(super) struct Binding {
    pub(super) symbol: SymbolHandle,
    pub(super) name: omega_checked_trees::name::Identifier,
    pub(super) value: Expression,
}

impl Default for Binding {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: omega_checked_trees::name::Identifier::generated_static(""),
            value: Expression::Integer(0),
        }
    }
}

pub(super) trait BindingScope {
    fn find_path_binding(&self, path: &NamePath) -> Option<&Binding>;
}

impl BindingScope for [Binding] {
    fn find_path_binding(&self, path: &NamePath) -> Option<&Binding> {
        self.iter()
            .find(|binding| binding_matches_path(binding, path))
    }
}

impl BindingScope for Arena<Binding> {
    fn find_path_binding(&self, path: &NamePath) -> Option<&Binding> {
        self.iter()
            .map(|(_, binding)| binding)
            .find(|binding| binding_matches_path(binding, path))
    }
}

pub(super) struct ScopedBindings<'scope, Parent: BindingScope + ?Sized> {
    pub(super) parent: &'scope Parent,
    pub(super) locals: &'scope Arena<Binding>,
}

impl<Parent: BindingScope + ?Sized> BindingScope for ScopedBindings<'_, Parent> {
    fn find_path_binding(&self, path: &NamePath) -> Option<&Binding> {
        self.parent.find_path_binding(path).or_else(|| {
            self.locals
                .iter()
                .map(|(_, binding)| binding)
                .find(|binding| binding_matches_path(binding, path))
        })
    }
}

fn binding_matches_path(binding: &Binding, path: &NamePath) -> bool {
    if binding.symbol.is_valid() && path.head_symbol().is_valid() {
        return binding.symbol == path.head_symbol();
    }

    path.first().is_some_and(|name| *name == binding.name)
}

pub(super) fn simple_local_bindings(
    program: &CheckedTrees,
    state: &State,
    statement_index: usize,
) -> Arena<Binding> {
    let local_binding_capacity = program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .take(statement_index)
        .filter(|statement| {
            matches!(
                statement,
                StatementNode::LocalData(local_data) if local_data.initial_value.is_valid()
            )
        })
        .count();
    let mut bindings = Arena::with_capacity(local_binding_capacity);

    let statements = program.statement_table.statements(state.statement_nodes);
    for (declaration_index, statement) in statements.iter().enumerate().take(statement_index) {
        let StatementNode::LocalData(local_data) = statement else {
            continue;
        };
        if !local_data.initial_value.is_valid() {
            continue;
        }
        // CAPTURE semantics: a `let` captures its initializer's VALUE at the
        // declaration. When a member field the initializer reads is REASSIGNED
        // between the declaration and this use point, substituting the
        // initializer would re-read the post-write value (`let t = self.v;
        // self.v = 0; nums[i] = t` silently wrote 0 -- the stale-fold family).
        // Skip the binding so the local resolves through its captured slot.
        // A field written only BEFORE the declaration still folds -- the
        // don't-over-block rule (runtime_spawn_interleaved_join depends on it).
        if initializer_field_reassigned_between(
            &program.expression_table,
            statements,
            declaration_index + 1,
            statement_index,
            local_data.initial_value,
        ) {
            continue;
        }
        let Some(value) = simple_local_binding_value_from_table(
            &program.expression_table,
            local_data.initial_value,
        ) else {
            continue;
        };
        bindings.insert(Binding {
            symbol: local_data.symbol,
            name: local_data.name.clone(),
            value,
        });
    }

    bindings
}

/// Whether a member FIELD the initializer reads is assigned by any statement in
/// `[start, end)` -- the capture-violation test above. Matched by field NAME
/// (conservative: a same-named field on another struct only suppresses a fold,
/// never miscompiles).
pub fn initializer_field_reassigned_between(
    expressions: &ExpressionTable,
    statements: &[StatementNode],
    start: usize,
    end: usize,
    initial_value: ExpressionHandle,
) -> bool {
    let mut read_fields = Vec::new();
    collect_member_field_names(expressions, initial_value, &mut read_fields);
    if read_fields.is_empty() {
        return false;
    }
    statements
        .iter()
        .skip(start)
        .take(end.saturating_sub(start))
        .any(|statement| {
            let StatementNode::Assignment(assignment) = statement else {
                return false;
            };
            let Some(target_field) = assignment_member_field_name(expressions, assignment.target)
            else {
                return false;
            };
            read_fields.iter().any(|field| *field == target_field)
        })
}

/// Collects the member FIELD NAMES an expression reads (`self.v` -> `v`),
/// walking nested receivers and operands.
fn collect_member_field_names(
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
    out: &mut Vec<omega_checked_trees::name::Identifier>,
) {
    match expressions.expression(expression) {
        ExpressionNode::Member(member) => {
            out.push(member.member.clone());
            collect_member_field_names(expressions, member.receiver, out);
        }
        ExpressionNode::Mutable(inner) => collect_member_field_names(expressions, *inner, out),
        ExpressionNode::Binary(binary) => {
            collect_member_field_names(expressions, binary.left, out);
            collect_member_field_names(expressions, binary.right, out);
        }
        ExpressionNode::Cast(cast) => collect_member_field_names(expressions, cast.value, out),
        ExpressionNode::Indexed(indexed) => {
            collect_member_field_names(expressions, indexed.collection, out);
            collect_member_field_names(expressions, indexed.index, out);
        }
        ExpressionNode::Unary(unary) => collect_member_field_names(expressions, unary.operand, out),
        ExpressionNode::Range(range) => {
            collect_member_field_names(expressions, range.start, out);
            collect_member_field_names(expressions, range.end, out);
        }
        ExpressionNode::Call(call) => {
            for argument in expressions.expression_handles(call.arguments) {
                collect_member_field_names(expressions, *argument, out);
            }
        }
        _ => {}
    }
}

/// The member FIELD NAME an assignment target writes (`self.v = ..` -> `v`),
/// peeling `Mutable`/`Indexed` wrappers. A plain-Name target (a local) is None.
fn assignment_member_field_name(
    expressions: &ExpressionTable,
    target: ExpressionHandle,
) -> Option<omega_checked_trees::name::Identifier> {
    match expressions.expression(target) {
        ExpressionNode::Member(member) => Some(member.member.clone()),
        ExpressionNode::Mutable(inner) => assignment_member_field_name(expressions, *inner),
        ExpressionNode::Indexed(indexed) => {
            assignment_member_field_name(expressions, indexed.collection)
        }
        _ => None,
    }
}

fn simple_local_binding_value_from_table(
    table: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<Expression> {
    match table.expression(expression) {
        ExpressionNode::Binary(binary) => Some(Expression::Binary(Box::new(BinaryExpression {
            left: simple_local_binding_value_from_table(table, binary.left)?,
            operator: binary.operator,
            right: simple_local_binding_value_from_table(table, binary.right)?,
        }))),
        ExpressionNode::Boolean(value) => Some(Expression::Boolean(*value)),
        ExpressionNode::Float(value) => Some(Expression::Float(*value)),
        ExpressionNode::Integer(value) => Some(Expression::Integer(*value)),
        ExpressionNode::String(value) => Some(Expression::String(value.clone())),
        ExpressionNode::Indexed(indexed) => {
            // A RUNTIME-indexed read (`arr[i]` with a non-constant index) is
            // materialized once into the local's own slot at the `let`
            // statement -- the existing whole-value copy lowering. Folding it
            // back into a USE site (e.g. `self.acc + nums[i]` after the hoist
            // pass synthesized `let __h = nums[i]; self.acc + __h`) re-creates
            // an operand-position runtime-indexed read, which has no value
            // lowering and trips the emission blocker. Keep the local a Name so
            // it resolves to its materialized slot. A CONSTANT-index read
            // (`arr[0]`) is still folded -- it lowers as a plain place path,
            // exactly as before this carve-out. (Same rationale as the
            // call-result-local preservation in `simplify_state_expression`.)
            if !matches!(table.expression(indexed.index), ExpressionNode::Integer(_)) {
                return None;
            }
            Some(Expression::Indexed(Box::new(IndexedExpression {
                collection: simple_local_binding_value_from_table(table, indexed.collection)?,
                index: simple_local_binding_value_from_table(table, indexed.index)?,
            })))
        }
        ExpressionNode::Range(range) => Some(Expression::Range(Box::new(
            omega_checked_trees::expression::RangeExpression {
                start: range
                    .start
                    .is_valid()
                    .then(|| simple_local_binding_value_from_table(table, range.start))
                    .flatten()
                    .map(Box::new),
                end: range
                    .end
                    .is_valid()
                    .then(|| simple_local_binding_value_from_table(table, range.end))
                    .flatten()
                    .map(Box::new),
                end_inclusive: range.end_inclusive,
            },
        ))),
        ExpressionNode::Call(call) => Some(Expression::Call(Box::new(CallExpression {
            receiver: call.receiver.is_valid().then(|| {
                simple_local_binding_value_from_table(table, call.receiver).map(Box::new)
            })?,
            target_symbol: call.target_symbol,
            target: call.target.clone(),
            arguments: table
                .expression_handles(call.arguments)
                .iter()
                .map(|argument| simple_local_binding_value_from_table(table, *argument))
                .collect::<Option<Arc<[_]>>>()?,
        }))),
        ExpressionNode::Mutable(inner) => simple_local_binding_value_from_table(table, *inner)
            .map(|value| Expression::Mutable(Box::new(value))),
        ExpressionNode::Unary(unary) => simple_local_binding_value_from_table(table, unary.operand)
            .map(|operand| {
                Expression::Unary(Box::new(omega_checked_trees::expression::UnaryExpression {
                    operator: unary.operator,
                    operand,
                }))
            }),
        ExpressionNode::Name(path) => {
            Some(Expression::Name(NamePath::resolved_with_member_symbols(
                table.name_path_members(path.members).to_vec(),
                table.name_path_member_symbols(path.member_symbols).to_vec(),
                path.head_symbol,
                path.symbol,
            )))
        }
        ExpressionNode::Member(member) => {
            let receiver = simple_local_binding_value_from_table(table, member.receiver)?;
            Some(Expression::Member(Box::new(MemberExpression {
                receiver,
                member_symbol: member.member_symbol,
                member: member.member.clone(),
                case_variant: member.case_variant.clone(),
            })))
        }
        ExpressionNode::ArrayLiteral(_)
        | ExpressionNode::Cast(_)
        | ExpressionNode::StructLiteral(_) => None,
    }
}

pub(super) fn append_name_suffix(
    base: &Expression,
    suffix: &[omega_checked_trees::name::Identifier],
) -> Expression {
    let mut expression = base.clone();

    for member in suffix {
        expression = Expression::Member(Box::new(MemberExpression {
            receiver: expression,
            member_symbol: SymbolHandle::invalid(),
            member: member.clone(),
            case_variant: None,
        }));
    }

    expression
}
