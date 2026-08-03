mod bindings;
mod call_targets;
mod folding;
mod helper_stack;

use self::bindings::{
    Binding, BindingScope, ScopedBindings, append_name_suffix, simple_local_bindings,
};
use self::call_targets::{resolve_call_target_machine, resolve_call_target_state};
use self::folding::{
    IntegerLanding, boolean_and, boolean_not, boolean_or, expressions_equivalent,
    fold_binary_expression,
};
use self::helper_stack::HelperStateStack;
use crate::StateValueRole;
use omega_core::arena::Arena;
use psi_checked_trees::CheckedTrees;
use psi_checked_trees::expression::{
    BinaryExpression, CallExpression, Expression, IndexedExpression, MemberExpression,
    StructLiteral, StructLiteralField, UnaryOperator,
};
use psi_checked_trees::machine::Machine;
use psi_checked_trees::state::State;
use psi_checked_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
use psi_symbols::SymbolHandle;
use std::sync::Arc;

pub fn simplify_expression(
    program: &CheckedTrees,
    machine: &Machine,
    expression: &Expression,
) -> Expression {
    let bindings: &[Binding] = &[];
    refill_helper_expansion_fuel();
    simplify_expression_with_bindings(program, machine, expression, bindings, false, 0, None)
}

pub fn simplify_state_expression(
    program: &CheckedTrees,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    expression: &Expression,
) -> Expression {
    simplify_state_expression_for_role(
        program,
        machine,
        state,
        statement_index,
        StateValueRole::AssignmentValue,
        expression,
    )
}

pub fn simplify_state_expression_for_role(
    program: &CheckedTrees,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    role: StateValueRole,
    expression: &Expression,
) -> Expression {
    let bindings = simple_local_bindings(program, machine, state, statement_index);
    // Never fold a call-result local into its use site, for ANY role. The call's
    // result is materialized once into that local's own call-result slot at the `let`
    // statement; substituting the call expression into a later statement (e.g.
    // `let index = self.f(c); ... = max(.., index + 1)`) re-references a call that is
    // not collected there, so its result slot can't be resolved and the value/write
    // fails to lower (especially in a dispatched callee). Keeping the local a Name
    // lets it resolve to its populated slot. (Previously only Call/Transition
    // arguments preserved call locals; AssignmentValue folded them -- the source of
    // the dispatched carve_room `max(.., self.cell_index(cell) + 1)` failure.)
    let preserve_call_locals = true;
    let landing = statement_destination_landing(program, machine, state, statement_index, role);
    refill_helper_expansion_fuel();
    simplify_expression_with_bindings(
        program,
        machine,
        expression,
        &bindings,
        preserve_call_locals,
        0,
        landing,
    )
}

/// The DESTINATION landing of a statement's value expression (CM2, ch5
/// "Constants: Two Phases"): the constant lands at the first typed site, and
/// for an assignment value that site is the declared type of what is being
/// written -- the `let` local's annotation or the assignment target's field.
/// Binding substitution erases the operand Names before the fold sees them,
/// so the destination is the reliable place to read the landed type from.
fn statement_destination_landing(
    program: &CheckedTrees,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    role: StateValueRole,
) -> Option<IntegerLanding> {
    if role != StateValueRole::AssignmentValue {
        return None;
    }
    let statement = program
        .statement_table
        .statements(state.statement_nodes)
        .get(statement_index)?;
    match statement {
        StatementNode::LocalData(local) => {
            landing_from_type_reference(program, local.type_reference)
        }
        StatementNode::Assignment(assignment) => {
            let target = program.expression_table.to_tree(assignment.target);
            derive_integer_landing(program, machine, &target, 0)
        }
        _ => None,
    }
}

/// Re-entrant simplification depth cap: the guarded-helper comparison and
/// helper-value inlining feed their EXPANSIONS back through the simplifier,
/// and a RECURSIVE callee grows its arguments one constructor-read per round
/// (`a.prev`, `a.prev.prev`, ..) with the cycle stack freshly popped between
/// rounds -- unbounded. Past the cap the expression returns UNSIMPLIFIED,
/// which is this canonicalizer's ordinary "no match" behavior (never
/// unsound, merely unfolded). Mirrors instruction selection's
/// MAX_BINDING_SUBSTITUTION_DEPTH.
const REENTRANT_SIMPLIFY_DEPTH_LIMIT: usize = 32;

/// One shared WORK budget per top-level simplify entry (the hang half of the
/// pending length_reverse repro): the re-entrant helper expansion is
/// depth-capped above, but it BRANCHES at every level -- exponential TIME on
/// adversarial shapes even though each path terminates. Every
/// `helper_state_model` build spends one unit; at zero every further
/// expansion declines (`None`), which callers already treat as "no fold"
/// (never unsound, merely unfolded). The entry points refill; a
/// thread_local keeps the many interior call sites signature-stable (plan
/// workers are per-thread, and every entry refills, so no cross-expression
/// bleed).
const HELPER_EXPANSION_FUEL_BUDGET: u64 = 20_000;

thread_local! {
    static HELPER_EXPANSION_FUEL: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

fn refill_helper_expansion_fuel() {
    HELPER_EXPANSION_FUEL.with(|fuel| fuel.set(HELPER_EXPANSION_FUEL_BUDGET));
}

fn spend_helper_expansion_fuel() -> bool {
    HELPER_EXPANSION_FUEL.with(|fuel| {
        let remaining = fuel.get();
        if remaining == 0 {
            return false;
        }
        fuel.set(remaining - 1);
        true
    })
}

fn simplify_expression_with_bindings(
    program: &CheckedTrees,
    machine: &Machine,
    expression: &Expression,
    bindings: &(impl BindingScope + ?Sized),
    preserve_call_locals: bool,
    depth: usize,
    // The DESTINATION landing (CM2): the declared type the whole expression's
    // value lands into. Propagated only along same-typed spines (binary
    // operands, `Mutable` wrappers); every differently-typed subtree (call
    // arguments, indices, struct fields, ...) gets `None`.
    landing: Option<IntegerLanding>,
) -> Expression {
    if depth >= REENTRANT_SIMPLIFY_DEPTH_LIMIT {
        return expression.clone();
    }
    match expression {
        // Atomic wrappers contain a compiler-authored operation shape. Algebraic
        // folding inside it can erase the result place or CAS operands before
        // instruction selection, so the carrier is intentionally opaque here.
        Expression::Atomic(_) => expression.clone(),
        Expression::ArrayLiteral(values) => Expression::ArrayLiteral(Arc::from(
            values
                .iter()
                .map(|value| {
                    simplify_expression_with_bindings(
                        program,
                        machine,
                        value,
                        bindings,
                        preserve_call_locals,
                        depth,
                        None,
                    )
                })
                .collect::<Arc<[_]>>(),
        )),
        Expression::Binary(binary) => simplify_binary_expression(
            program,
            machine,
            binary,
            bindings,
            preserve_call_locals,
            depth,
            landing,
        ),
        Expression::Boolean(_)
        | Expression::Float(_)
        | Expression::Integer(_)
        | Expression::String(_) => expression.clone(),
        Expression::Call(call) => simplify_call_expression(
            program,
            machine,
            call,
            bindings,
            preserve_call_locals,
            depth,
        ),
        Expression::Cast(cast) => {
            Expression::Cast(Box::new(psi_checked_trees::expression::CastExpression {
                value: simplify_expression_with_bindings(
                    program,
                    machine,
                    &cast.value,
                    bindings,
                    preserve_call_locals,
                    depth,
                    None,
                ),
                target_type: cast.target_type.clone(),
                target_label: cast.target_label.clone(),
                domain: cast.domain,
                form: cast.form,
            }))
        }
        Expression::Indexed(indexed) => Expression::Indexed(Box::new(IndexedExpression {
            collection: simplify_collection_expression(
                program,
                machine,
                &indexed.collection,
                bindings,
                preserve_call_locals,
                depth,
            ),
            index: simplify_index_expression(
                program,
                machine,
                &indexed.index,
                bindings,
                preserve_call_locals,
                depth,
            ),
        })),
        Expression::Range(range) => {
            Expression::Range(Box::new(psi_checked_trees::expression::RangeExpression {
                start: range.start.as_ref().map(|start| {
                    Box::new(simplify_expression_with_bindings(
                        program,
                        machine,
                        start,
                        bindings,
                        preserve_call_locals,
                        depth,
                        None,
                    ))
                }),
                end: range.end.as_ref().map(|end| {
                    Box::new(simplify_expression_with_bindings(
                        program,
                        machine,
                        end,
                        bindings,
                        preserve_call_locals,
                        depth,
                        None,
                    ))
                }),
                end_inclusive: range.end_inclusive,
            }))
        }
        Expression::Member(member) => {
            let receiver = simplify_expression_with_bindings(
                program,
                machine,
                &member.receiver,
                bindings,
                preserve_call_locals,
                depth,
                None,
            );
            // Struct-literal field extraction: `Holder { f: X }.f` simplifies to
            // `X`. A local bound to a struct literal simplifies to that literal,
            // so a field read of it must select the field's initializer rather
            // than leave a struct-literal-rooted member access that downstream
            // place resolution cannot place (it finds no storage and leaves the
            // target zero). This is the single shared normalization point for
            // every selection consumer; recursing through this arm folds nested
            // chains too (`Msg { body: &c }.body.value` -> `(&c).value`). Decision
            // 15 stage 2: a borrow-carrying `data` value's field read resolves to
            // its borrowed source, including reads that dereference it further.
            if let Expression::StructLiteral(literal) = &receiver
                && let Some(field) = literal
                    .fields
                    .iter()
                    .find(|field| field.name == member.member)
            {
                return field.value.clone();
            }
            Expression::Member(Box::new(MemberExpression {
                receiver,
                member_symbol: member.member_symbol,
                member: member.member.clone(),
                case_variant: member.case_variant.clone(),
            }))
        }
        Expression::Mutable(inner) => {
            Expression::Mutable(Box::new(simplify_expression_with_bindings(
                program,
                machine,
                inner,
                bindings,
                preserve_call_locals,
                depth,
                landing,
            )))
        }
        Expression::Unary(unary) => {
            let operand = simplify_expression_with_bindings(
                program,
                machine,
                &unary.operand,
                bindings,
                preserve_call_locals,
                depth,
                None,
            );
            match unary.operator {
                UnaryOperator::LogicalNot => boolean_not(operand),
            }
        }
        Expression::Name(path) => bindings
            .find_path_binding(path)
            .and_then(|binding| {
                if preserve_call_locals && matches!(binding.value, Expression::Call(_)) {
                    return None;
                }
                Some(append_name_suffix(&binding.value, &path[1..]))
            })
            .unwrap_or_else(|| expression.clone()),
        Expression::StructLiteral(struct_literal) => Expression::StructLiteral(StructLiteral {
            type_name: struct_literal.type_name.clone(),
            case_name: struct_literal.case_name.clone(),
            fields: Arc::from(
                struct_literal
                    .fields
                    .iter()
                    .map(|field| StructLiteralField {
                        name: field.name.clone(),
                        value: simplify_expression_with_bindings(
                            program,
                            machine,
                            &field.value,
                            bindings,
                            preserve_call_locals,
                            depth,
                            None,
                        ),
                    })
                    .collect::<Arc<[_]>>(),
            ),
        }),
        Expression::ZeroValue(_) => expression.clone(),
    }
}

/// Simplify an `Indexed` node's INDEX. A bare-Name local whose binding is a
/// COMPUTED value (`let m = self.k + 1; self.arr[m]`) must NOT fold back: a
/// computed index (`arr[k+1]`) has no lowering -- the index must be a plain
/// place or a constant -- and state-storage keeps exactly this local's slot
/// (the runtime-index carve-out in collection.rs), so the Name resolves to a
/// populated slot. Constant and place bindings still fold (they lower, and
/// their locals stay elidable).
/// Simplify an `Indexed` node's COLLECTION. A bare-Name local whose binding
/// is an AGGREGATE literal (`let g = [[9, 8], [6, 5]]; ... g[a][b]`) must NOT
/// fold back: an array/struct literal in collection position has no place to
/// index (the indexed resolvers need a slot), and state-storage keeps exactly
/// such aggregate locals slotted. Scalar and place bindings still fold.
fn simplify_collection_expression(
    program: &CheckedTrees,
    machine: &Machine,
    collection: &Expression,
    bindings: &(impl BindingScope + ?Sized),
    preserve_call_locals: bool,
    depth: usize,
) -> Expression {
    if let Expression::Name(path) = collection
        && let Some(binding) = bindings.find_path_binding(path)
        && path.len() == 1
        && matches!(
            binding.value,
            Expression::ArrayLiteral(_) | Expression::StructLiteral(_)
        )
    {
        return collection.clone();
    }
    simplify_expression_with_bindings(
        program,
        machine,
        collection,
        bindings,
        preserve_call_locals,
        depth,
        None,
    )
}

fn simplify_index_expression(
    program: &CheckedTrees,
    machine: &Machine,
    index: &Expression,
    bindings: &(impl BindingScope + ?Sized),
    preserve_call_locals: bool,
    depth: usize,
) -> Expression {
    if let Expression::Name(path) = index
        && let Some(binding) = bindings.find_path_binding(path)
        && path.len() == 1
        && matches!(
            binding.value,
            Expression::Binary(_) | Expression::Unary(_) | Expression::Cast(_)
        )
    {
        return index.clone();
    }
    simplify_expression_with_bindings(
        program,
        machine,
        index,
        bindings,
        preserve_call_locals,
        depth,
        None,
    )
}

fn simplify_binary_expression(
    program: &CheckedTrees,
    machine: &Machine,
    binary: &BinaryExpression,
    bindings: &(impl BindingScope + ?Sized),
    preserve_call_locals: bool,
    depth: usize,
    landing: Option<IntegerLanding>,
) -> Expression {
    let left = simplify_expression_with_bindings(
        program,
        machine,
        &binary.left,
        bindings,
        preserve_call_locals,
        depth,
        landing,
    );
    // A shift COUNT is not a value of the subject's landed type -- keep the
    // landing off it (counts are exact anonymous values; F8 rules their
    // range semantics).
    let right_landing = if matches!(
        binary.operator,
        psi_checked_trees::expression::BinaryOperator::ShiftLeft
            | psi_checked_trees::expression::BinaryOperator::ShiftRight
    ) {
        None
    } else {
        landing
    };
    let right = simplify_expression_with_bindings(
        program,
        machine,
        &binary.right,
        bindings,
        preserve_call_locals,
        depth,
        right_landing,
    );

    if let Some(expression) = simplify_guarded_helper_comparison(
        program,
        machine,
        binary.operator,
        &left,
        &right,
        bindings,
        depth,
    ) {
        // GROWTH edge: the expansion re-enters the simplifier with material
        // the comparison machinery synthesized; count it against the
        // re-entrancy budget (a recursive callee grows its arguments one
        // constructor-read per round).
        return simplify_expression_with_bindings(
            program,
            machine,
            &expression,
            bindings,
            preserve_call_locals,
            depth + 1,
            None,
        );
    }

    if let Some(folded) = reflexive_comparison_fold(program, binary.operator, &left, &right) {
        return folded;
    }

    let landing = landing.or_else(|| integer_fold_landing(program, machine, binary, &left, &right));
    fold_binary_expression(binary.operator, left, right, landing)
}

/// The landed integer type of an about-to-fold binary expression (CM2, ch5
/// "Constants: Two Phases"), derived from the ORIGINAL (pre-substitution)
/// operands: binding substitution replaces `a` with its constant before the
/// fold, but `binary.left` still spells `Name(a)`, whose DECLARED type says
/// how the constant landed. Only derived when both simplified operands are
/// integer literals (a fold is actually about to happen); `None` keeps the
/// transitional bare-i64 window.
fn integer_fold_landing(
    program: &CheckedTrees,
    machine: &Machine,
    binary: &BinaryExpression,
    left: &Expression,
    right: &Expression,
) -> Option<IntegerLanding> {
    if !matches!(
        (left, right),
        (Expression::Integer(_), Expression::Integer(_))
    ) {
        return None;
    }
    derive_integer_landing(program, machine, &binary.left, 0)
        .or_else(|| derive_integer_landing(program, machine, &binary.right, 0))
}

/// Bounded recursion cap for landing derivation: original operand trees are
/// shallow; anything deeper is not worth walking for a fold hint.
const LANDING_DERIVATION_DEPTH_LIMIT: usize = 8;

fn derive_integer_landing(
    program: &CheckedTrees,
    machine: &Machine,
    expression: &Expression,
    depth: usize,
) -> Option<IntegerLanding> {
    if depth >= LANDING_DERIVATION_DEPTH_LIMIT {
        return None;
    }
    match expression {
        Expression::Name(path) => {
            let type_reference = declared_type_reference(program, machine, path.symbol())?;
            landing_from_type_reference(program, type_reference)
        }
        Expression::Member(member) => {
            let type_reference = field_declared_type_reference(program, member.member_symbol)?;
            landing_from_type_reference(program, type_reference)
        }
        Expression::Cast(cast) => {
            let primitive = program.primitive_type_reference(cast.target_type)?;
            if !primitive.accepts_integer_literal() {
                return None;
            }
            Some(IntegerLanding {
                primitive,
                domain: cast.domain,
            })
        }
        Expression::Binary(inner) => {
            derive_integer_landing(program, machine, &inner.left, depth + 1)
                .or_else(|| derive_integer_landing(program, machine, &inner.right, depth + 1))
        }
        _ => None,
    }
}

fn landing_from_type_reference(
    program: &CheckedTrees,
    type_reference: psi_checked_trees::types::TypeReferenceHandle,
) -> Option<IntegerLanding> {
    // An inferred `let` has no annotation -- its handle is invalid.
    if !type_reference.is_valid() {
        return None;
    }
    let primitive = program.primitive_type_reference(type_reference)?;
    if !primitive.accepts_integer_literal() {
        return None;
    }
    Some(IntegerLanding {
        primitive,
        domain: program
            .type_reference_table
            .arithmetic_domain(type_reference),
    })
}

/// The declared type of a symbol in this machine: a state parameter or a
/// `let` local (scanned across the machine's states -- symbols are unique),
/// falling back to a data FIELD (a resolved `self.x` path carries the field
/// symbol as its tail).
fn declared_type_reference(
    program: &CheckedTrees,
    machine: &Machine,
    symbol: SymbolHandle,
) -> Option<psi_checked_trees::types::TypeReferenceHandle> {
    if !symbol.is_valid() {
        return None;
    }
    for state in program.machine_states(machine) {
        for parameter in program.state_parameters(state) {
            if parameter.symbol == symbol {
                return Some(parameter.type_reference);
            }
        }
        for statement in program.statement_table.statements(state.statement_nodes) {
            if let StatementNode::LocalData(local) = statement
                && local.symbol == symbol
            {
                return Some(local.type_reference);
            }
        }
    }
    field_declared_type_reference(program, symbol)
}

fn field_declared_type_reference(
    program: &CheckedTrees,
    symbol: SymbolHandle,
) -> Option<psi_checked_trees::types::TypeReferenceHandle> {
    if !symbol.is_valid() {
        return None;
    }
    for data in program.data_definitions() {
        for member in program.data_members(data) {
            if let psi_checked_trees::data::DataMember::Field(field) = member
                && field.symbol == symbol
            {
                return Some(field.type_reference);
            }
        }
    }
    None
}

/// The TYPE-GATED reflexive fold: `x == x -> true` / `x != x -> false` for
/// structurally-equal operands -- valid ONLY when the operand provably cannot
/// be NaN. For floats the identity is IEEE-false (`NaN != NaN` is TRUE; the
/// canonical isNaN idiom `f != f` used to fold silently to `false` here), so
/// a float-typed or untypeable operand falls through to a REAL runtime
/// compare (the ucomis* path handles NaN correctly).
fn reflexive_comparison_fold(
    program: &CheckedTrees,
    operator: psi_checked_trees::expression::BinaryOperator,
    left: &Expression,
    right: &Expression,
) -> Option<Expression> {
    use psi_checked_trees::expression::BinaryOperator::{Equal, NotEqual};

    if !matches!(operator, Equal | NotEqual) || left != right {
        return None;
    }
    if !reflexive_operand_provably_not_nan(program, left) {
        return None;
    }
    Some(Expression::Boolean(matches!(operator, Equal)))
}

/// Whether a structurally-self-compared operand provably cannot be NaN:
/// non-float literals; a FLOAT LITERAL (a concrete spelled value is never
/// NaN); or a data-FIELD member path whose declared primitive type is
/// non-float (resolved by the member's field SYMBOL). Anything else -- float
/// fields, locals, params, indexed elements, calls -- stays a runtime
/// compare: correct for every type, merely unfolded.
fn reflexive_operand_provably_not_nan(program: &CheckedTrees, operand: &Expression) -> bool {
    match operand {
        Expression::Boolean(_)
        | Expression::Integer(_)
        | Expression::String(_)
        | Expression::Float(_) => true,
        Expression::Member(member) => member_field_primitive(program, member.member_symbol)
            .is_some_and(|primitive| {
                !matches!(
                    primitive,
                    psi_checked_trees::types::PrimitiveType::F32
                        | psi_checked_trees::types::PrimitiveType::F64
                )
            }),
        _ => false,
    }
}

/// The declared primitive type of the data FIELD a member symbol names, or
/// `None` when the symbol is not a data field (or not primitive-typed).
fn member_field_primitive(
    program: &CheckedTrees,
    member_symbol: SymbolHandle,
) -> Option<psi_checked_trees::types::PrimitiveType> {
    field_declared_type_reference(program, member_symbol)
        .and_then(|type_reference| program.primitive_type_reference(type_reference))
}

fn simplify_call_expression(
    program: &CheckedTrees,
    machine: &Machine,
    call: &CallExpression,
    bindings: &(impl BindingScope + ?Sized),
    preserve_call_locals: bool,
    depth: usize,
) -> Expression {
    let receiver = call.receiver.as_ref().map(|receiver| {
        simplify_expression_with_bindings(
            program,
            machine,
            receiver,
            bindings,
            preserve_call_locals,
            depth,
            None,
        )
    });
    let simplified_arguments: Arc<[_]> = call
        .arguments
        .iter()
        .map(|argument| {
            simplify_expression_with_bindings(
                program,
                machine,
                argument,
                bindings,
                preserve_call_locals,
                depth,
                None,
            )
        })
        .collect();

    if let Some(target_machine) =
        resolve_call_target_machine(program, machine, receiver.as_ref(), call.target_symbol)
        && let Some(state) = resolve_call_target_state(program, target_machine, call)
    {
        let parameters = program.state_parameters(state);
        let mut argument_bindings = Arena::with_capacity(parameters.len());
        for (parameter, argument) in parameters.iter().zip(simplified_arguments.iter()) {
            argument_bindings.insert(Binding {
                symbol: parameter.symbol,
                name: parameter.name.clone(),
                value: argument.clone(),
            });
        }
        if let Some(value) =
            helper_state_value(state, program, target_machine, &argument_bindings, depth)
        {
            // GROWTH edge: the inlined helper value is new material under
            // new bindings.
            return simplify_expression_with_bindings(
                program,
                machine,
                &value,
                &argument_bindings,
                preserve_call_locals,
                depth + 1,
                None,
            );
        }
    }

    Expression::Call(Box::new(CallExpression {
        receiver: receiver.map(Box::new),
        target_symbol: call.target_symbol,
        target: call.target.clone(),
        arguments: simplified_arguments,
        operational_acknowledgement: call.operational_acknowledgement,
    }))
}

fn simplify_guarded_helper_comparison(
    program: &CheckedTrees,
    machine: &Machine,
    operator: psi_checked_trees::expression::BinaryOperator,
    left: &Expression,
    right: &Expression,
    bindings: &(impl BindingScope + ?Sized),
    depth: usize,
) -> Option<Expression> {
    use psi_checked_trees::expression::BinaryOperator::{Equal, NotEqual};

    if !matches!(operator, Equal | NotEqual) {
        return None;
    }

    if let Expression::Call(call) = left
        && let Some(condition) =
            simplify_helper_call_comparison(program, machine, call, right, bindings, depth)
    {
        return Some(match operator {
            Equal => condition,
            NotEqual => boolean_not(condition),
            _ => unreachable!(),
        });
    }

    if let Expression::Call(call) = right
        && let Some(condition) =
            simplify_helper_call_comparison(program, machine, call, left, bindings, depth)
    {
        return Some(match operator {
            Equal => condition,
            NotEqual => boolean_not(condition),
            _ => unreachable!(),
        });
    }

    None
}

fn simplify_helper_call_comparison(
    program: &CheckedTrees,
    machine: &Machine,
    call: &CallExpression,
    expected: &Expression,
    bindings: &(impl BindingScope + ?Sized),
    depth: usize,
) -> Option<Expression> {
    let receiver = call.receiver.as_ref().map(|receiver| {
        simplify_expression_with_bindings(program, machine, receiver, bindings, false, depth, None)
    });
    let target_machine =
        resolve_call_target_machine(program, machine, receiver.as_ref(), call.target_symbol)?;
    let state = resolve_call_target_state(program, target_machine, call)?;
    let parameters = program.state_parameters(state);
    let mut argument_bindings = Arena::with_capacity(parameters.len());
    for (parameter, argument) in parameters.iter().zip(call.arguments.iter()) {
        argument_bindings.insert(Binding {
            symbol: parameter.symbol,
            name: parameter.name.clone(),
            value: simplify_expression_with_bindings(
                program, machine, argument, bindings, false, depth, None,
            ),
        });
    }

    helper_state_match_condition(
        state,
        program,
        target_machine,
        &argument_bindings,
        expected,
        depth,
    )
}

fn helper_state_value(
    state: &State,
    program: &CheckedTrees,
    machine: &Machine,
    bindings: &(impl BindingScope + ?Sized),
    depth: usize,
) -> Option<Expression> {
    let helper = helper_state_model(state, program, machine, bindings, depth)?;
    let mut transitions = helper.transitions.iter().map(|(_, transition)| transition);
    let transition = transitions.next()?;
    if transitions.next().is_some() {
        return None;
    }
    if !matches!(transition.guard, Expression::Boolean(true)) {
        return None;
    }
    Some(transition.value.clone())
}

fn helper_state_match_condition(
    state: &State,
    program: &CheckedTrees,
    machine: &Machine,
    bindings: &(impl BindingScope + ?Sized),
    expected: &Expression,
    depth: usize,
) -> Option<Expression> {
    helper_state_match_condition_with_stack(
        state,
        program,
        machine,
        bindings,
        expected,
        &mut HelperStateStack::with_capacity(program.machine_states.len()),
        depth,
    )
}

fn helper_state_match_condition_with_stack(
    state: &State,
    program: &CheckedTrees,
    machine: &Machine,
    bindings: &(impl BindingScope + ?Sized),
    expected: &Expression,
    stack: &mut HelperStateStack,
    depth: usize,
) -> Option<Expression> {
    if state.symbol.is_valid() && stack.contains(state.symbol) {
        return None;
    }
    let pushed = state.symbol.is_valid();
    if pushed {
        stack.push(state.symbol);
    }

    let helper = match helper_state_model(state, program, machine, bindings, depth) {
        Some(helper) => helper,
        None => {
            if pushed {
                stack.pop();
            }
            return None;
        }
    };
    let mut covered = Expression::Boolean(false);
    let mut matched = Expression::Boolean(false);

    for (_, transition) in helper.transitions.iter() {
        let effective_guard = boolean_and(boolean_not(covered.clone()), transition.guard.clone());
        if let Some(value_matches) = expression_match_condition_with_stack(
            program,
            machine,
            &transition.value,
            expected,
            stack,
            depth,
        ) {
            matched = boolean_or(matched, boolean_and(effective_guard.clone(), value_matches));
        }
        covered = boolean_or(covered, effective_guard);
    }

    if pushed {
        stack.pop();
    }
    Some(matched)
}

fn expression_match_condition_with_stack(
    program: &CheckedTrees,
    machine: &Machine,
    expression: &Expression,
    expected: &Expression,
    stack: &mut HelperStateStack,
    depth: usize,
) -> Option<Expression> {
    if expressions_equivalent(expression, expected) {
        return Some(Expression::Boolean(true));
    }

    let Expression::Call(call) = expression else {
        return None;
    };

    let receiver = call.receiver.as_deref();
    let target_machine =
        resolve_call_target_machine(program, machine, receiver, call.target_symbol)?;
    let state = resolve_call_target_state(program, target_machine, call)?;
    let parameters = program.state_parameters(state);
    let mut argument_bindings = Arena::with_capacity(parameters.len());
    for (parameter, argument) in parameters.iter().zip(call.arguments.iter()) {
        argument_bindings.insert(Binding {
            symbol: parameter.symbol,
            name: parameter.name.clone(),
            value: argument.clone(),
        });
    }

    helper_state_match_condition_with_stack(
        state,
        program,
        target_machine,
        &argument_bindings,
        expected,
        stack,
        depth,
    )
}

fn helper_state_model(
    state: &State,
    program: &CheckedTrees,
    machine: &Machine,
    bindings: &(impl BindingScope + ?Sized),
    depth: usize,
) -> Option<HelperStateModel> {
    // GROWTH edge: each model build expands the callee's body under new
    // argument bindings.
    let depth = depth + 1;
    if depth >= REENTRANT_SIMPLIFY_DEPTH_LIMIT {
        return None;
    }
    // The shared per-entry WORK budget (see HELPER_EXPANSION_FUEL_BUDGET):
    // depth bounds each path, fuel bounds the exponential branching.
    if !spend_helper_expansion_fuel() {
        return None;
    }
    let statements = program.statement_table.statements(state.statement_nodes);
    let local_binding_capacity = statements
        .iter()
        .filter(|statement| matches!(statement, StatementNode::LocalData(_)))
        .count();
    let transition_capacity = statements
        .iter()
        .filter(|statement| {
            matches!(
                statement,
                StatementNode::Transition(_) | StatementNode::Expression(_)
            )
        })
        .count();
    let mut local_bindings = Arena::with_capacity(local_binding_capacity);
    let mut transitions = Arena::with_capacity(transition_capacity);
    let mut saw_terminal_expression = false;

    for statement in statements {
        let scoped_bindings = ScopedBindings {
            parent: bindings,
            locals: &local_bindings,
        };

        match statement {
            StatementNode::AssemblyFact(_) => {}
            StatementNode::LocalData(local) => {
                if saw_terminal_expression {
                    return None;
                }
                if !local.initial_value.is_valid() {
                    return None;
                }
                let initial_value = program.expression_table.to_tree(local.initial_value);
                let value = simplify_expression_with_bindings(
                    program,
                    machine,
                    &initial_value,
                    &scoped_bindings,
                    false,
                    depth,
                    None,
                );
                local_bindings.insert(Binding {
                    symbol: local.symbol,
                    name: local.name.clone(),
                    value,
                });
            }
            StatementNode::Transition(transition) => {
                if saw_terminal_expression {
                    return None;
                }
                if transition.continuation.is_valid() {
                    return None;
                }
                let guard = match transition.guard {
                    TransitionGuardNode::Always => Expression::Boolean(true),
                    TransitionGuardNode::When(expression) => {
                        let expression = program.expression_table.to_tree(expression);
                        simplify_expression_with_bindings(
                            program,
                            machine,
                            &expression,
                            &scoped_bindings,
                            false,
                            depth,
                            None,
                        )
                    }
                };
                let TransitionTargetNode::Value(value) =
                    program.statement_table.transition_target(transition.target)
                else {
                    return None;
                };
                let value = program.expression_table.to_tree(*value);
                let value = simplify_expression_with_bindings(
                    program,
                    machine,
                    &value,
                    &scoped_bindings,
                    false,
                    depth,
                    None,
                );
                transitions.insert(HelperTransition { guard, value });
            }
            StatementNode::Expression(expression) => {
                let expression = program.expression_table.to_tree(*expression);
                let value = simplify_expression_with_bindings(
                    program,
                    machine,
                    &expression,
                    &scoped_bindings,
                    false,
                    depth,
                    None,
                );
                transitions.insert(HelperTransition {
                    guard: Expression::Boolean(true),
                    value,
                });
                saw_terminal_expression = true;
            }
            StatementNode::Assignment(_) | StatementNode::Call(_) => {
                return None;
            }
        }
    }

    if transitions.is_empty() {
        return None;
    }

    Some(HelperStateModel { transitions })
}

#[derive(Debug, Clone)]
struct HelperStateModel {
    transitions: Arena<HelperTransition>,
}

#[derive(Debug, Clone)]
struct HelperTransition {
    guard: Expression,
    value: Expression,
}

impl Default for HelperTransition {
    fn default() -> Self {
        Self {
            guard: Expression::Boolean(true),
            value: Expression::Integer(psi_numerics::literals::IntegerLiteral::zero()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::simplify_expression;
    use psi_checked_trees::CheckedTrees;
    use psi_checked_trees::expression::{
        BinaryExpression, BinaryOperator, CallExpression, Expression, NamePath,
    };
    use psi_checked_trees::machine::Machine;
    use psi_checked_trees::name::Identifier;
    use psi_checked_trees::signature::StateParameter;
    use psi_checked_trees::state::State;
    use psi_checked_trees::statement::{
        StatementNode, TableLocalData, TableTransition, TransitionGuardNode, TransitionTargetNode,
    };
    use psi_checked_trees::types::{TypeReferenceHandle, TypeReferenceNode};
    use psi_symbols::SymbolHandle;
    use std::sync::Arc;

    #[test]
    fn simplifies_guarded_helper_call_comparison_to_guard_expression() {
        let machine_symbol = SymbolHandle::from_arena_index(1);
        let helper_symbol = SymbolHandle::from_arena_index(2);
        let roll_symbol = SymbolHandle::from_arena_index(3);
        let is_quiet_symbol = SymbolHandle::from_arena_index(4);
        let is_fountain_symbol = SymbolHandle::from_arena_index(5);
        let is_reward_symbol = SymbolHandle::from_arena_index(6);
        let event_action_symbol = SymbolHandle::from_arena_index(7);
        let quiet_symbol = SymbolHandle::from_arena_index(8);
        let fountain_symbol = SymbolHandle::from_arena_index(9);
        let reward_symbol = SymbolHandle::from_arena_index(10);
        let enemy_symbol = SymbolHandle::from_arena_index(11);

        let mut helper = State {
            symbol: helper_symbol,
            name: "event_action".into(),
            parameters: Default::default(),
            return_type: TypeReferenceHandle::invalid(),
            contracts: Default::default(),
            statement_nodes: Default::default(),
        };

        let mut machine = Machine {
            symbol: machine_symbol,
            name: "RoomEvents".into(),
            attached_data: None,
            contracts: Default::default(),
            service_reaches: Default::default(),
            owned_data: Default::default(),
            satisfies: Default::default(),
            states: Default::default(),
            ..Machine::default()
        };
        let mut program = CheckedTrees::default();
        push_state_statements(
            &mut program,
            &mut helper,
            [
                TestStatement::LocalData {
                    symbol: is_quiet_symbol,
                    name: "is_quiet".into(),
                    type_reference: TypeReferenceNode::Named {
                        symbol: SymbolHandle::invalid(),
                        name: "bool".into(),
                    },
                    initial_value: Some(Expression::Binary(Box::new(BinaryExpression {
                        left: name("roll", roll_symbol),
                        operator: BinaryOperator::Less,
                        right: Expression::Integer(
                            psi_numerics::literals::IntegerLiteral::from_value(20),
                        ),
                    }))),
                },
                TestStatement::LocalData {
                    symbol: is_fountain_symbol,
                    name: "is_fountain".into(),
                    type_reference: TypeReferenceNode::Named {
                        symbol: SymbolHandle::invalid(),
                        name: "bool".into(),
                    },
                    initial_value: Some(Expression::Binary(Box::new(BinaryExpression {
                        left: name("roll", roll_symbol),
                        operator: BinaryOperator::Less,
                        right: Expression::Integer(
                            psi_numerics::literals::IntegerLiteral::from_value(30),
                        ),
                    }))),
                },
                TestStatement::LocalData {
                    symbol: is_reward_symbol,
                    name: "is_reward".into(),
                    type_reference: TypeReferenceNode::Named {
                        symbol: SymbolHandle::invalid(),
                        name: "bool".into(),
                    },
                    initial_value: Some(Expression::Binary(Box::new(BinaryExpression {
                        left: name("roll", roll_symbol),
                        operator: BinaryOperator::Less,
                        right: Expression::Integer(
                            psi_numerics::literals::IntegerLiteral::from_value(60),
                        ),
                    }))),
                },
                TestStatement::TransitionValue {
                    target: resolved_path_expression(
                        &["RoomEventAction", "Quiet"],
                        event_action_symbol,
                        quiet_symbol,
                    ),
                    guard: Some(name("is_quiet", is_quiet_symbol)),
                },
                TestStatement::TransitionValue {
                    target: resolved_path_expression(
                        &["RoomEventAction", "Fountain"],
                        event_action_symbol,
                        fountain_symbol,
                    ),
                    guard: Some(name("is_fountain", is_fountain_symbol)),
                },
                TestStatement::TransitionValue {
                    target: resolved_path_expression(
                        &["RoomEventAction", "Reward"],
                        event_action_symbol,
                        reward_symbol,
                    ),
                    guard: Some(name("is_reward", is_reward_symbol)),
                },
                TestStatement::TransitionValue {
                    target: resolved_path_expression(
                        &["RoomEventAction", "Enemy"],
                        event_action_symbol,
                        enemy_symbol,
                    ),
                    guard: None,
                },
            ],
        );
        let roll_type = named_type_reference(&mut program, "u32");
        program.typed.push_state_parameter(
            &mut helper,
            StateParameter {
                symbol: roll_symbol,
                name: "roll".into(),
                type_reference: roll_type,
                is_const: true,
                is_mutable: false,
                is_self: false,
            },
        );
        program.typed.push_machine_state(&mut machine, helper);
        program.typed.push_machine(machine.clone());

        let quiet_guard = Expression::Binary(Box::new(BinaryExpression {
            left: Expression::Call(Box::new(CallExpression {
                receiver: None,
                target_symbol: helper_symbol,
                target: "event_action".into(),
                arguments: Arc::from(vec![name("roll", roll_symbol)].into_boxed_slice()),
                operational_acknowledgement: Default::default(),
            })),
            operator: BinaryOperator::Equal,
            right: resolved_path_expression(
                &["RoomEventAction", "Quiet"],
                event_action_symbol,
                quiet_symbol,
            ),
        }));

        let fountain_guard = Expression::Binary(Box::new(BinaryExpression {
            left: Expression::Call(Box::new(CallExpression {
                receiver: None,
                target_symbol: helper_symbol,
                target: "event_action".into(),
                arguments: Arc::from(vec![name("roll", roll_symbol)].into_boxed_slice()),
                operational_acknowledgement: Default::default(),
            })),
            operator: BinaryOperator::Equal,
            right: resolved_path_expression(
                &["RoomEventAction", "Fountain"],
                event_action_symbol,
                fountain_symbol,
            ),
        }));

        assert_eq!(
            simplify_expression(&program, &machine, &quiet_guard),
            Expression::Binary(Box::new(BinaryExpression {
                left: name("roll", roll_symbol),
                operator: BinaryOperator::Less,
                right: Expression::Integer(psi_numerics::literals::IntegerLiteral::from_value(20)),
            }))
        );

        let reward_guard = Expression::Binary(Box::new(BinaryExpression {
            left: Expression::Call(Box::new(CallExpression {
                receiver: None,
                target_symbol: helper_symbol,
                target: "event_action".into(),
                arguments: Arc::from(vec![name("roll", roll_symbol)].into_boxed_slice()),
                operational_acknowledgement: Default::default(),
            })),
            operator: BinaryOperator::Equal,
            right: resolved_path_expression(
                &["RoomEventAction", "Reward"],
                event_action_symbol,
                reward_symbol,
            ),
        }));

        let enemy_guard = Expression::Binary(Box::new(BinaryExpression {
            left: Expression::Call(Box::new(CallExpression {
                receiver: None,
                target_symbol: helper_symbol,
                target: "event_action".into(),
                arguments: Arc::from(vec![name("roll", roll_symbol)].into_boxed_slice()),
                operational_acknowledgement: Default::default(),
            })),
            operator: BinaryOperator::Equal,
            right: resolved_path_expression(
                &["RoomEventAction", "Enemy"],
                event_action_symbol,
                enemy_symbol,
            ),
        }));

        assert_eq!(
            simplify_expression(&program, &machine, &fountain_guard),
            Expression::Binary(Box::new(BinaryExpression {
                left: Expression::Binary(Box::new(BinaryExpression {
                    left: name("roll", roll_symbol),
                    operator: BinaryOperator::GreaterOrEqual,
                    right: Expression::Integer(psi_numerics::literals::IntegerLiteral::from_value(
                        20
                    )),
                })),
                operator: BinaryOperator::And,
                right: Expression::Binary(Box::new(BinaryExpression {
                    left: name("roll", roll_symbol),
                    operator: BinaryOperator::Less,
                    right: Expression::Integer(psi_numerics::literals::IntegerLiteral::from_value(
                        30
                    )),
                })),
            }))
        );

        assert_eq!(
            simplify_expression(&program, &machine, &reward_guard),
            Expression::Binary(Box::new(BinaryExpression {
                left: Expression::Binary(Box::new(BinaryExpression {
                    left: name("roll", roll_symbol),
                    operator: BinaryOperator::GreaterOrEqual,
                    right: Expression::Integer(psi_numerics::literals::IntegerLiteral::from_value(
                        30
                    )),
                })),
                operator: BinaryOperator::And,
                right: Expression::Binary(Box::new(BinaryExpression {
                    left: name("roll", roll_symbol),
                    operator: BinaryOperator::Less,
                    right: Expression::Integer(psi_numerics::literals::IntegerLiteral::from_value(
                        60
                    )),
                })),
            }))
        );

        assert_eq!(
            simplify_expression(&program, &machine, &enemy_guard),
            Expression::Binary(Box::new(BinaryExpression {
                left: name("roll", roll_symbol),
                operator: BinaryOperator::GreaterOrEqual,
                right: Expression::Integer(psi_numerics::literals::IntegerLiteral::from_value(60)),
            }))
        );
    }

    #[test]
    fn simplifies_impossible_integer_range_conjunction_to_false() {
        let machine = Machine {
            symbol: SymbolHandle::invalid(),
            name: "main".into(),
            attached_data: None,
            contracts: Default::default(),
            service_reaches: Default::default(),
            owned_data: Default::default(),
            satisfies: Default::default(),
            states: Default::default(),
            ..Machine::default()
        };
        let program = CheckedTrees::default();
        let roll_symbol = SymbolHandle::from_arena_index(99);

        let expression = Expression::Binary(Box::new(BinaryExpression {
            left: Expression::Binary(Box::new(BinaryExpression {
                left: name("roll", roll_symbol),
                operator: BinaryOperator::GreaterOrEqual,
                right: Expression::Integer(psi_numerics::literals::IntegerLiteral::from_value(20)),
            })),
            operator: BinaryOperator::And,
            right: Expression::Binary(Box::new(BinaryExpression {
                left: name("roll", roll_symbol),
                operator: BinaryOperator::Less,
                right: Expression::Integer(psi_numerics::literals::IntegerLiteral::from_value(20)),
            })),
        }));

        assert_eq!(
            simplify_expression(&program, &machine, &expression),
            Expression::Boolean(false)
        );
    }

    #[test]
    fn simplifies_negated_redundant_integer_range_conjunction_to_single_comparison() {
        let machine = Machine {
            symbol: SymbolHandle::invalid(),
            name: "main".into(),
            attached_data: None,
            contracts: Default::default(),
            service_reaches: Default::default(),
            owned_data: Default::default(),
            satisfies: Default::default(),
            states: Default::default(),
            ..Machine::default()
        };
        let program = CheckedTrees::default();
        let health_symbol = SymbolHandle::from_arena_index(100);

        let expression = Expression::Binary(Box::new(BinaryExpression {
            left: Expression::Binary(Box::new(BinaryExpression {
                left: Expression::Binary(Box::new(BinaryExpression {
                    left: name("health", health_symbol),
                    operator: BinaryOperator::GreaterOrEqual,
                    right: Expression::Integer(psi_numerics::literals::IntegerLiteral::from_value(
                        0,
                    )),
                })),
                operator: BinaryOperator::And,
                right: Expression::Binary(Box::new(BinaryExpression {
                    left: name("health", health_symbol),
                    operator: BinaryOperator::Greater,
                    right: Expression::Integer(psi_numerics::literals::IntegerLiteral::from_value(
                        0,
                    )),
                })),
            })),
            operator: BinaryOperator::Equal,
            right: Expression::Boolean(false),
        }));

        assert_eq!(
            simplify_expression(&program, &machine, &expression),
            Expression::Binary(Box::new(BinaryExpression {
                left: name("health", health_symbol),
                operator: BinaryOperator::LessOrEqual,
                right: Expression::Integer(psi_numerics::literals::IntegerLiteral::from_value(0)),
            }))
        );
    }

    enum TestStatement {
        LocalData {
            symbol: SymbolHandle,
            name: Identifier,
            type_reference: TypeReferenceNode,
            initial_value: Option<Expression>,
        },
        TransitionValue {
            target: Expression,
            guard: Option<Expression>,
        },
    }

    fn push_state_statements<const N: usize>(
        program: &mut CheckedTrees,
        state: &mut State,
        statements: [TestStatement; N],
    ) {
        for statement in statements {
            let statement = match statement {
                TestStatement::LocalData {
                    symbol,
                    name,
                    type_reference,
                    initial_value,
                } => {
                    let type_reference = program.typed.type_reference_table.insert(type_reference);
                    let initial_value = initial_value
                        .as_ref()
                        .map(|value| program.typed.expression_table.insert_tree(value))
                        .unwrap_or_else(psi_checked_trees::expression::ExpressionHandle::invalid);

                    StatementNode::LocalData(TableLocalData {
                        symbol,
                        name,
                        type_reference,
                        initial_value,
                        is_mutable: false,
                    })
                }
                TestStatement::TransitionValue { target, guard } => {
                    let target = program.typed.expression_table.insert_tree(&target);
                    let target = program
                        .typed
                        .statement_table
                        .insert_transition_target(TransitionTargetNode::Value(target));
                    let guard = guard
                        .as_ref()
                        .map(|guard| {
                            TransitionGuardNode::When(
                                program.typed.expression_table.insert_tree(guard),
                            )
                        })
                        .unwrap_or(TransitionGuardNode::Always);

                    StatementNode::Transition(TableTransition {
                        target,
                        continuation: psi_checked_trees::statement::TransitionTargetHandle::invalid(
                        ),
                        guard,
                    })
                }
            };
            program
                .typed
                .statement_table
                .push_statement(&mut state.statement_nodes, statement);
        }
    }

    fn name(name: &str, symbol: SymbolHandle) -> Expression {
        Expression::Name(NamePath::resolved(vec![name.into()], symbol, symbol))
    }

    fn named_type_reference(program: &mut CheckedTrees, name: &str) -> TypeReferenceHandle {
        program
            .typed
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: SymbolHandle::invalid(),
                name: name.into(),
            })
    }

    fn resolved_path_expression(
        segments: &[&str],
        head_symbol: SymbolHandle,
        symbol: SymbolHandle,
    ) -> Expression {
        Expression::Name(NamePath::resolved(
            segments
                .iter()
                .map(|segment| Identifier::from(*segment))
                .collect(),
            head_symbol,
            symbol,
        ))
    }
}
