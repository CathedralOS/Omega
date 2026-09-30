//! The index-equality decision table. Each test pins one family of rows:
//! which pairs of dependent-type index arguments denote the same index once a
//! call's binder substitutions apply, which positions are closed, and which
//! established fact operands name an index argument. The asymmetries are
//! deliberate and pinned here: a bound literal never matches a Boolean
//! expression while an open `true` does, an open name compares a name leaf by
//! spelling but an integer leaf by value, and a fact operand's resolved symbol
//! overrides a spelling match that an index argument would accept.
use super::{
    BoundIndexArgument, BoundIndexSubstitutions, ExpressionSubstitution,
    bound_index_arguments_equal, bound_index_expressions_equal, expression_uses_binder,
    is_closed_index_argument, substituted_expression_matches_index_argument,
};
use arena::HandleSpan;
use language_semantics::const_value::CanonicalConstValue;
use numerics::literals::{IntegerLiteral, IntegerRadix};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableBinaryExpression, TableMemberExpression,
    TableNamePath,
};
use typed_trees::name::Identifier;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// The callee's first declared index binder.
const A: SymbolHandle = SymbolHandle::from_arena_index(1);
/// The callee's second declared index binder.
const B: SymbolHandle = SymbolHandle::from_arena_index(2);
/// A caller-side subject.
const N: SymbolHandle = SymbolHandle::from_arena_index(3);
/// Another caller-side subject.
const M: SymbolHandle = SymbolHandle::from_arena_index(4);
/// A callee value parameter an established fact names.
const P: SymbolHandle = SymbolHandle::from_arena_index(5);
const UNRESOLVED: SymbolHandle = SymbolHandle::invalid();

fn subject(symbol: SymbolHandle) -> BoundIndexArgument {
    BoundIndexArgument::Subject(symbol)
}

fn literal(spelling: &str) -> BoundIndexArgument {
    BoundIndexArgument::Literal(spelling.to_owned())
}

#[derive(Default)]
struct Indices {
    program: TypedTrees,
}

impl Indices {
    fn named(&mut self, symbol: SymbolHandle, name: &str) -> TypeReferenceHandle {
        self.program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol,
                name: Identifier::from(name),
            })
    }

    fn constant(&mut self, expression: ExpressionHandle) -> TypeReferenceHandle {
        self.program
            .type_reference_table
            .insert(TypeReferenceNode::ConstExpression(expression))
    }

    fn opaque(&mut self) -> TypeReferenceHandle {
        self.program
            .type_reference_table
            .insert(TypeReferenceNode::Unit)
    }

    fn path(
        &mut self,
        symbol: SymbolHandle,
        head_symbol: SymbolHandle,
        members: &[&str],
    ) -> ExpressionHandle {
        let mut span = HandleSpan::empty();
        for member in members {
            self.program
                .expression_table
                .push_name_path_member(&mut span, Identifier::from(*member));
        }
        self.program
            .expression_table
            .insert(ExpressionNode::Name(TableNamePath {
                members: span,
                member_symbols: HandleSpan::empty(),
                head_symbol,
                symbol,
            }))
    }

    /// A resolved single-member name.
    fn name(&mut self, symbol: SymbolHandle, spelling: &str) -> ExpressionHandle {
        self.path(symbol, UNRESOLVED, &[spelling])
    }

    /// An unresolved single-member name: only its spelling identifies it.
    fn atom(&mut self, spelling: &str) -> ExpressionHandle {
        self.path(UNRESOLVED, UNRESOLVED, &[spelling])
    }

    fn integer(&mut self, value: i64) -> ExpressionHandle {
        self.program
            .expression_table
            .insert(ExpressionNode::Integer(IntegerLiteral::from_value(value)))
    }

    fn hexadecimal(&mut self, digits: &str) -> ExpressionHandle {
        let literal = IntegerLiteral::from_parts(false, IntegerRadix::Hexadecimal, digits)
            .expect("hexadecimal digits");
        self.program
            .expression_table
            .insert(ExpressionNode::Integer(literal))
    }

    fn boolean(&mut self, value: bool) -> ExpressionHandle {
        self.program
            .expression_table
            .insert(ExpressionNode::Boolean(value))
    }

    fn binary(
        &mut self,
        left: ExpressionHandle,
        operator: BinaryOperator,
        right: ExpressionHandle,
    ) -> ExpressionHandle {
        self.program
            .expression_table
            .insert(ExpressionNode::Binary(TableBinaryExpression {
                left,
                operator,
                right,
            }))
    }

    fn member(&mut self, receiver: ExpressionHandle, member: &str) -> ExpressionHandle {
        self.program
            .expression_table
            .insert(ExpressionNode::Member(TableMemberExpression {
                receiver,
                member_symbol: UNRESOLVED,
                member: Identifier::from(member),
                case_variant: None,
            }))
    }

    /// `left + 0`, the shape a callee spells as `Indexed<A + 0>`.
    fn plus_zero(&mut self, left: ExpressionHandle) -> ExpressionHandle {
        let zero = self.integer(0);
        self.binary(left, BinaryOperator::Add, zero)
    }

    /// Both argument orders must agree: every rule here is symmetric.
    fn equal(
        &self,
        left: TypeReferenceHandle,
        right: TypeReferenceHandle,
        substitutions: &BoundIndexSubstitutions,
    ) -> bool {
        let forward = bound_index_arguments_equal(&self.program, &[left], &[right], substitutions);
        let backward = bound_index_arguments_equal(&self.program, &[right], &[left], substitutions);
        assert_eq!(forward, backward, "index equality must not depend on side");
        forward
    }

    fn expressions_equal(
        &self,
        left: ExpressionHandle,
        right: ExpressionHandle,
        substitutions: &BoundIndexSubstitutions,
    ) -> bool {
        let forward = bound_index_expressions_equal(&self.program, left, right, substitutions);
        let backward = bound_index_expressions_equal(&self.program, right, left, substitutions);
        assert_eq!(
            forward, backward,
            "leaf-wise equality must not depend on side"
        );
        forward
    }

    fn closed(
        &self,
        argument: TypeReferenceHandle,
        substitutions: &BoundIndexSubstitutions,
    ) -> bool {
        super::index_argument_term(&self.program, argument, substitutions).is_closed()
    }

    fn fact_matches(
        &self,
        operand: ExpressionHandle,
        parameters: &[ExpressionSubstitution<'_>],
        argument: TypeReferenceHandle,
        substitutions: &BoundIndexSubstitutions,
    ) -> bool {
        substituted_expression_matches_index_argument(
            &self.program,
            operand,
            parameters,
            argument,
            substitutions,
        )
    }
}

#[test]
fn argument_lists_compare_only_under_a_binding_and_matching_lengths() {
    let mut indices = Indices::default();
    let open = indices.named(N, "N");
    let bound = indices.named(A, "A");
    let other = indices.named(M, "M");
    let binding = vec![(A, subject(N))];
    let program = &indices.program;
    assert!(bound_index_arguments_equal(
        program,
        &[bound],
        &[open],
        &binding
    ));
    assert!(
        !bound_index_arguments_equal(program, &[open], &[open], &Vec::new()),
        "without a binding the spelling-identity path decides, not this one"
    );
    assert!(!bound_index_arguments_equal(program, &[], &[], &binding));
    assert!(!bound_index_arguments_equal(
        program,
        &[bound],
        &[open, open],
        &binding
    ));
    assert!(bound_index_arguments_equal(
        program,
        &[bound, open],
        &[open, open],
        &binding
    ));
    assert!(!bound_index_arguments_equal(
        program,
        &[bound, open],
        &[open, other],
        &binding
    ));
}

#[test]
fn bound_binders_compare_through_their_bound_indices() {
    let mut indices = Indices::default();
    let first = indices.named(A, "A");
    let second = indices.named(B, "B");
    for (substitutions, expected, rule) in [
        (vec![(A, subject(N)), (B, subject(N))], true, "same subject"),
        (
            vec![(A, subject(N)), (B, subject(M))],
            false,
            "different subjects",
        ),
        (
            vec![(A, literal("7")), (B, literal("7"))],
            true,
            "same literal spelling",
        ),
        (
            vec![(A, literal("7")), (B, literal("07"))],
            false,
            "literals compare by spelling",
        ),
        (
            vec![(A, subject(N)), (B, literal("7"))],
            false,
            "subject never equals literal",
        ),
        (
            vec![(A, literal("7")), (A, subject(N)), (B, literal("7"))],
            true,
            "first binding wins",
        ),
    ] {
        assert_eq!(
            indices.equal(first, second, &substitutions),
            expected,
            "{rule}"
        );
    }
}

#[test]
fn bound_binder_meets_open_name_by_subject_symbol_or_literal_spelling() {
    let mut indices = Indices::default();
    let binder = indices.named(A, "A");
    let caller = indices.named(N, "N");
    let respelled_caller = indices.named(N, "renamed");
    let same_spelling_other_subject = indices.named(M, "N");
    let seven = indices.named(UNRESOLVED, "7");
    let padded_seven = indices.named(UNRESOLVED, "07");
    let bound_subject = vec![(A, subject(N))];
    let bound_literal = vec![(A, literal("7"))];
    assert!(indices.equal(binder, caller, &bound_subject));
    assert!(
        indices.equal(binder, respelled_caller, &bound_subject),
        "symbol decides"
    );
    assert!(!indices.equal(binder, same_spelling_other_subject, &bound_subject));
    assert!(!indices.equal(binder, seven, &bound_subject));
    assert!(indices.equal(binder, seven, &bound_literal));
    assert!(
        !indices.equal(binder, padded_seven, &bound_literal),
        "spelling, not value"
    );
    assert!(!indices.equal(binder, caller, &bound_literal));
}

#[test]
fn open_names_compare_by_symbol_and_spelling() {
    let mut indices = Indices::default();
    let caller = indices.named(N, "N");
    let caller_again = indices.named(N, "N");
    let respelled = indices.named(N, "renamed");
    let other_subject = indices.named(M, "N");
    let seven = indices.named(UNRESOLVED, "7");
    let seven_again = indices.named(UNRESOLVED, "7");
    let padded_seven = indices.named(UNRESOLVED, "07");
    let unrelated_binding = vec![(B, subject(M))];
    assert!(indices.equal(caller, caller_again, &unrelated_binding));
    assert!(!indices.equal(caller, respelled, &unrelated_binding));
    assert!(!indices.equal(caller, other_subject, &unrelated_binding));
    assert!(indices.equal(seven, seven_again, &unrelated_binding));
    assert!(!indices.equal(seven, padded_seven, &unrelated_binding));
}

#[test]
fn other_type_shapes_denote_only_themselves() {
    let mut indices = Indices::default();
    let unit = indices.opaque();
    let unit_again = indices.opaque();
    let binder = indices.named(A, "A");
    let caller = indices.named(N, "N");
    let seven = indices.integer(7);
    let constant = indices.constant(seven);
    let binding = vec![(A, subject(N))];
    assert!(indices.equal(unit, unit, &binding));
    assert!(!indices.equal(unit, unit_again, &binding));
    assert!(!indices.equal(unit, binder, &binding));
    assert!(!indices.equal(unit, caller, &binding));
    assert!(!indices.equal(unit, constant, &binding));
}

#[test]
fn bound_binder_meets_const_expression_by_the_expression_own_spelling() {
    let mut indices = Indices::default();
    let binder = indices.named(A, "A");
    let caller_leaf = indices.name(N, "N");
    let caller = indices.constant(caller_leaf);
    let caller_head_leaf = indices.path(UNRESOLVED, N, &["N", "field"]);
    let caller_head = indices.constant(caller_head_leaf);
    let other_leaf = indices.name(M, "M");
    let other = indices.constant(other_leaf);
    let seven_leaf = indices.integer(7);
    let seven = indices.constant(seven_leaf);
    let hex_seven_leaf = indices.hexadecimal("7");
    let hex_seven = indices.constant(hex_seven_leaf);
    let eight_leaf = indices.integer(8);
    let eight = indices.constant(eight_leaf);
    let seven_atom_leaf = indices.atom("7");
    let seven_atom = indices.constant(seven_atom_leaf);
    let padded_atom_leaf = indices.atom("07");
    let padded_atom = indices.constant(padded_atom_leaf);
    let word_atom_leaf = indices.atom("x");
    let word_atom = indices.constant(word_atom_leaf);
    let true_leaf = indices.boolean(true);
    let true_constant = indices.constant(true_leaf);
    let true_atom_leaf = indices.atom("true");
    let true_atom = indices.constant(true_atom_leaf);
    let binder_leaf = indices.name(A, "A");
    let binder_constant = indices.constant(binder_leaf);
    let sum_leaf = indices.plus_zero(caller_leaf);
    let sum = indices.constant(sum_leaf);

    let bound_subject = vec![(A, subject(N))];
    assert!(indices.equal(binder, caller, &bound_subject));
    assert!(
        indices.equal(binder, caller_head, &bound_subject),
        "head symbol counts"
    );
    assert!(!indices.equal(binder, other, &bound_subject));
    assert!(!indices.equal(binder, seven, &bound_subject));
    assert!(
        !indices.equal(binder, sum, &bound_subject),
        "a leaf, not a compound"
    );
    assert!(
        !indices.equal(binder, binder_constant, &bound_subject),
        "the expression side keeps its own spelling against a named binder"
    );

    let bound_literal = vec![(A, literal("7"))];
    assert!(indices.equal(binder, seven, &bound_literal));
    assert!(
        indices.equal(binder, hex_seven, &bound_literal),
        "integer leaf by value"
    );
    assert!(!indices.equal(binder, eight, &bound_literal));
    assert!(indices.equal(binder, seven_atom, &bound_literal));
    assert!(
        indices.equal(binder, padded_atom, &bound_literal),
        "name leaf by value too"
    );
    assert!(!indices.equal(binder, word_atom, &bound_literal));
    assert!(!indices.equal(binder, caller, &bound_literal));

    let bound_true = vec![(A, literal("true"))];
    assert!(
        !indices.equal(binder, true_constant, &bound_true),
        "a bound literal never matches a Boolean expression"
    );
    assert!(
        indices.equal(binder, true_atom, &bound_true),
        "but matches its spelling"
    );

    let bound_word = vec![(A, literal("x"))];
    assert!(indices.equal(binder, word_atom, &bound_word));
}

#[test]
fn open_name_meets_const_expression_by_symbol_spelling_or_literal_value() {
    let mut indices = Indices::default();
    let caller = indices.named(N, "N");
    let unresolved_caller = indices.named(UNRESOLVED, "N");
    let seven = indices.named(UNRESOLVED, "7");
    let hex_seven = indices.named(UNRESOLVED, "0x7");
    let open_true = indices.named(UNRESOLVED, "true");
    let one = indices.named(UNRESOLVED, "1");
    let unrelated_binding = vec![(B, subject(M))];
    let mut expression_argument = |build: &dyn Fn(&mut Indices) -> ExpressionHandle| {
        let expression = build(&mut indices);
        indices.constant(expression)
    };
    let resolved = expression_argument(&|indices| indices.name(N, "N"));
    let respelled = expression_argument(&|indices| indices.name(N, "renamed"));
    let head_only = expression_argument(&|indices| indices.path(UNRESOLVED, N, &["N", "x"]));
    let other_symbol_same_spelling = expression_argument(&|indices| indices.name(M, "N"));
    let other_symbol_other_spelling = expression_argument(&|indices| indices.name(M, "X"));
    let spelled = expression_argument(&|indices| indices.atom("N"));
    let dotted = expression_argument(&|indices| indices.path(UNRESOLVED, UNRESOLVED, &["N", "x"]));
    let integer_seven = expression_argument(&|indices| indices.integer(7));
    let integer_eight = expression_argument(&|indices| indices.integer(8));
    let seven_atom = expression_argument(&|indices| indices.atom("7"));
    let padded_atom = expression_argument(&|indices| indices.atom("07"));
    let boolean_true = expression_argument(&|indices| indices.boolean(true));
    let boolean_false = expression_argument(&|indices| indices.boolean(false));
    let compound = expression_argument(&|indices| {
        let leaf = indices.name(N, "N");
        indices.plus_zero(leaf)
    });

    for (open, expression, expected, rule) in [
        (caller, resolved, true, "same symbol"),
        (caller, respelled, true, "symbol decides over spelling"),
        (caller, head_only, true, "head symbol counts"),
        (
            caller,
            other_symbol_same_spelling,
            true,
            "spelling fallback",
        ),
        (
            caller,
            other_symbol_other_spelling,
            false,
            "neither symbol nor spelling",
        ),
        (unresolved_caller, spelled, true, "single-member spelling"),
        (
            unresolved_caller,
            resolved,
            true,
            "spelling with a resolved leaf",
        ),
        (
            unresolved_caller,
            dotted,
            false,
            "a dotted path never spells a name",
        ),
        (seven, integer_seven, true, "integer leaf by value"),
        (hex_seven, integer_seven, true, "radix spelling by value"),
        (seven, integer_eight, false, "different value"),
        (seven, seven_atom, true, "name leaf by spelling"),
        (
            seven,
            padded_atom,
            false,
            "name leaf by spelling, never by value",
        ),
        (open_true, boolean_true, true, "Boolean spelling"),
        (open_true, boolean_false, false, "other Boolean"),
        (one, boolean_true, false, "only true/false spell Booleans"),
        (caller, compound, false, "a compound is not a leaf"),
    ] {
        assert_eq!(
            indices.equal(open, expression, &unrelated_binding),
            expected,
            "{rule}"
        );
    }
}

#[test]
fn const_expressions_substitute_binders_at_leaf_positions() {
    let mut indices = Indices::default();
    let binder_leaf = indices.name(A, "A");
    let second_binder_leaf = indices.name(B, "B");
    let caller_leaf = indices.name(N, "N");
    let callee_sum = indices.plus_zero(binder_leaf);
    let second_callee_sum = indices.plus_zero(second_binder_leaf);
    let caller_sum = indices.plus_zero(caller_leaf);
    let caller_difference = {
        let zero = indices.integer(0);
        indices.binary(caller_leaf, BinaryOperator::Subtract, zero)
    };
    let seven_leaf = indices.integer(7);
    let seven_sum = indices.plus_zero(seven_leaf);
    let callee_member = indices.member(binder_leaf, "x");
    let caller_member = indices.member(caller_leaf, "x");
    let caller_other_member = indices.member(caller_leaf, "y");
    let callee = indices.constant(callee_sum);
    let caller = indices.constant(caller_sum);

    let bound_subject = vec![(A, subject(N))];
    assert!(
        indices.equal(callee, caller, &bound_subject),
        "`A + 0` is `N + 0`"
    );
    assert!(indices.expressions_equal(callee_sum, caller_sum, &bound_subject));
    assert!(!indices.expressions_equal(callee_sum, caller_difference, &bound_subject));
    assert!(!indices.expressions_equal(callee_sum, caller_sum, &vec![(A, subject(M))]));
    assert!(indices.expressions_equal(callee_member, caller_member, &bound_subject));
    assert!(!indices.expressions_equal(callee_member, caller_other_member, &bound_subject));
    assert!(indices.expressions_equal(
        callee_sum,
        second_callee_sum,
        &vec![(A, subject(N)), (B, subject(N))]
    ));
    assert!(indices.expressions_equal(callee_sum, seven_sum, &vec![(A, literal("7"))]));

    // The leaf key is the resolved symbol, falling back to the head symbol.
    let head_bound = indices.path(UNRESOLVED, A, &["A"]);
    let symbol_overrides_head = indices.path(M, A, &["A"]);
    assert!(indices.expressions_equal(head_bound, caller_leaf, &bound_subject));
    assert!(!indices.expressions_equal(symbol_overrides_head, caller_leaf, &bound_subject));
}

#[test]
fn unbound_const_expression_leaves_compare_by_shape() {
    let mut indices = Indices::default();
    let seven = indices.integer(7);
    let seven_again = indices.integer(7);
    let hex_seven = indices.hexadecimal("7");
    let hex_seven_atom = indices.atom("0x7");
    let true_value = indices.boolean(true);
    let true_again = indices.boolean(true);
    let true_atom = indices.atom("true");
    let caller = indices.name(N, "N");
    let caller_again = indices.name(N, "N");
    let respelled = indices.name(N, "renamed");
    let no_binding = Vec::new();
    for (left, right, expected, rule) in [
        (seven, seven_again, true, "same integer spelling"),
        (
            seven,
            hex_seven,
            false,
            "integer leaves compare canonical spelling",
        ),
        (
            seven,
            hex_seven_atom,
            true,
            "integer against name leaf by value",
        ),
        (true_value, true_again, true, "same Boolean"),
        (
            true_value,
            true_atom,
            false,
            "no Boolean-name rule between index expressions",
        ),
        (caller, caller_again, true, "same resolved name"),
        (
            caller,
            respelled,
            false,
            "names compare symbols and spelling",
        ),
        (caller, seven, false, "different shapes"),
    ] {
        assert_eq!(
            indices.expressions_equal(left, right, &no_binding),
            expected,
            "{rule}"
        );
    }
    assert!(bound_index_expressions_equal(
        &indices.program,
        ExpressionHandle::invalid(),
        ExpressionHandle::invalid(),
        &no_binding
    ));
    assert!(!indices.expressions_equal(ExpressionHandle::invalid(), seven, &no_binding));
}

#[test]
fn bound_literals_close_positions_and_bound_subjects_keep_them_open() {
    let mut indices = Indices::default();
    let binder = indices.named(A, "A");
    let seven = indices.named(UNRESOLVED, "7");
    let negative = indices.named(UNRESOLVED, "-3");
    let word = indices.named(UNRESOLVED, "N");
    let open_true = indices.named(UNRESOLVED, "true");
    let canonical_true = indices.named(UNRESOLVED, &CanonicalConstValue::boolean(true).atom());
    let seven_leaf = indices.integer(7);
    let constant = indices.constant(seven_leaf);
    let unit = indices.opaque();
    let no_binding = Vec::new();
    assert!(
        indices.closed(binder, &vec![(A, literal("x"))]),
        "any bound literal"
    );
    assert!(!indices.closed(binder, &vec![(A, subject(N))]));
    assert!(!indices.closed(binder, &no_binding));
    assert!(indices.closed(seven, &vec![(A, subject(N))]));
    for (argument, expected, rule) in [
        (seven, true, "decimal"),
        (negative, true, "negative decimal"),
        (word, false, "a name"),
        (
            open_true,
            false,
            "a raw Boolean spelling is not a canonical atom",
        ),
        (canonical_true, true, "canonical atom"),
        (constant, false, "const expressions stay open"),
        (unit, false, "other shapes stay open"),
    ] {
        assert_eq!(indices.closed(argument, &no_binding), expected, "{rule}");
        assert_eq!(
            is_closed_index_argument(&indices.program, argument),
            expected,
            "{rule}"
        );
    }
}

#[test]
fn fact_operands_match_open_names_by_resolved_symbol_first() {
    let mut indices = Indices::default();
    let caller = indices.named(N, "N");
    let unresolved_caller = indices.named(UNRESOLVED, "N");
    let seven = indices.named(UNRESOLVED, "7");
    let open_true = indices.named(UNRESOLVED, "true");
    let resolved = indices.name(N, "N");
    let other_symbol_same_spelling = indices.name(M, "N");
    let head_only = indices.path(UNRESOLVED, N, &["N", "x"]);
    let spelled = indices.atom("N");
    let integer_seven = indices.integer(7);
    let boolean_true = indices.boolean(true);
    let compound = indices.plus_zero(resolved);
    let no_binding = Vec::new();
    for (operand, argument, expected, rule) in [
        (resolved, caller, true, "same symbol"),
        (
            other_symbol_same_spelling,
            caller,
            false,
            "resolved symbols decide; no spelling fallback",
        ),
        (head_only, caller, true, "head symbol counts"),
        (
            spelled,
            caller,
            true,
            "spelling when the operand is unresolved",
        ),
        (
            other_symbol_same_spelling,
            unresolved_caller,
            true,
            "spelling when the index is unresolved",
        ),
        (integer_seven, seven, true, "integer by value"),
        (boolean_true, open_true, true, "Boolean spelling"),
        (compound, caller, false, "compound operand"),
    ] {
        assert_eq!(
            indices.fact_matches(operand, &[], argument, &no_binding),
            expected,
            "{rule}"
        );
    }
}

#[test]
fn fact_operands_substitute_call_parameters_and_bound_binders() {
    let mut indices = Indices::default();
    let caller = indices.named(N, "N");
    let binder = indices.named(A, "A");
    let unit = indices.opaque();
    let caller_leaf = indices.name(N, "N");
    let seven_leaf = indices.integer(7);
    let parameter = indices.name(P, "p");
    let unresolved_parameter = indices.atom("p");
    let parameter_sum = indices.plus_zero(parameter);
    let binder_leaf = indices.name(A, "A");
    let binder_sum = indices.plus_zero(binder_leaf);
    let callee_sum = indices.constant(binder_sum);
    let true_value = indices.boolean(true);
    let true_atom_leaf = indices.atom("true");
    let true_atom = indices.constant(true_atom_leaf);
    let parameters = [ExpressionSubstitution {
        symbol: P,
        name: "p",
        value: caller_leaf,
    }];
    let bound_subject = vec![(A, subject(N))];
    let no_binding = Vec::new();

    assert!(indices.fact_matches(parameter, &parameters, caller, &no_binding));
    assert!(indices.fact_matches(unresolved_parameter, &parameters, caller, &no_binding));
    assert!(!indices.fact_matches(parameter, &[], caller, &no_binding));
    assert!(indices.fact_matches(caller_leaf, &[], binder, &bound_subject));
    assert!(indices.fact_matches(parameter, &parameters, binder, &bound_subject));
    assert!(!indices.fact_matches(seven_leaf, &[], binder, &bound_subject));
    assert!(indices.fact_matches(seven_leaf, &[], binder, &vec![(A, literal("7"))]));
    assert!(
        indices.fact_matches(parameter_sum, &parameters, callee_sum, &bound_subject),
        "`p + 0` under `p -> N` is `A + 0` under `A -> N`"
    );
    assert!(
        indices.fact_matches(true_value, &[], true_atom, &no_binding),
        "fact operands compare Booleans to name spellings"
    );
    assert!(!indices.fact_matches(caller_leaf, &[], unit, &no_binding));
}

#[test]
fn binder_use_is_found_at_any_leaf() {
    let mut indices = Indices::default();
    let binder_leaf = indices.name(A, "A");
    let caller_leaf = indices.name(N, "N");
    let symbol_overrides_head = indices.path(M, A, &["A"]);
    let sum = indices.binary(caller_leaf, BinaryOperator::Add, binder_leaf);
    let member = indices.member(binder_leaf, "x");
    let binding = vec![(A, subject(N))];
    let program = &indices.program;
    assert!(expression_uses_binder(program, binder_leaf, &binding));
    assert!(expression_uses_binder(program, sum, &binding));
    assert!(expression_uses_binder(program, member, &binding));
    assert!(!expression_uses_binder(program, caller_leaf, &binding));
    assert!(!expression_uses_binder(
        program,
        symbol_overrides_head,
        &binding
    ));
}
