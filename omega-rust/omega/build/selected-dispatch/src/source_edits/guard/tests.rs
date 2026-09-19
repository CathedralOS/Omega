use super::{
    GraphGuard, MAX_NODES, StaticMachineArgument, SymbolHandle, TypedTrees, bindings, expressions,
    types,
};
use arena::HandleSpan;
use semantic_vocabulary::PackageKeyIdentity;
use source::{SourceId, SourceMap, SourceOrigin, SourceSpan, Span};
use std::path::PathBuf;
use std::sync::Arc;
use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
use typed_trees::constant::ConstDeclaration;
use typed_trees::data::{DataField, DataMember, TypeParameter, TypeParameterKind};
use typed_trees::expression::{
    ExpressionHandle, ExpressionNode, MatchPattern, TableCastExpression, TableMatchArm,
    TableMatchExpression, TableNamePath, TableStructLiteral, TableStructLiteralField,
    TableUnaryExpression, UnaryOperator,
};
use typed_trees::name::Identifier;
use typed_trees::signature::StateParameter;
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TableLocalData};
use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

#[test]
fn match_subject_pattern_and_unselected_arm_mutations_invalidate_source_custody() {
    let mut program = TypedTrees::default();
    let subject = program
        .expression_table
        .insert(ExpressionNode::Boolean(false));
    let pattern = program
        .expression_table
        .insert(ExpressionNode::Boolean(false));
    let selected = program
        .expression_table
        .insert(ExpressionNode::Boolean(false));
    let fallback = program
        .expression_table
        .insert(ExpressionNode::Boolean(false));
    let arms = program.expression_table.insert_match_arms([
        typed_trees::expression::TableMatchArm {
            pattern: typed_trees::expression::MatchPattern::Value(pattern),
            value: selected,
            source_span: Default::default(),
        },
        typed_trees::expression::TableMatchArm {
            pattern: typed_trees::expression::MatchPattern::Wildcard,
            value: fallback,
            source_span: Default::default(),
        },
    ]);
    let root = program.expression_table.insert(ExpressionNode::Match(
        typed_trees::expression::TableMatchExpression { subject, arms },
    ));
    let guard = GraphGuard::capture(&program, &[root], &[], &[], &[]).expect("dispatch custody");
    guard.validate(&program).expect("unchanged dispatch");
    for child in [subject, pattern, selected, fallback] {
        *program.expression_table.expression_mut(child) = ExpressionNode::Boolean(true);
        assert!(
            guard.validate(&program).is_err(),
            "every authored branch remains in custody"
        );
        *program.expression_table.expression_mut(child) = ExpressionNode::Boolean(false);
        guard.validate(&program).expect("restored dispatch");
    }
}

#[test]
fn nested_expression_and_argument_span_mutations_are_detected() {
    let mut program = TypedTrees::default();
    let first = program
        .expression_table
        .insert(ExpressionNode::Boolean(false));
    let second = program
        .expression_table
        .insert(ExpressionNode::Boolean(true));
    let arguments = program.expression_table.insert_expression_handles([first]);
    let root = program
        .expression_table
        .insert(ExpressionNode::ArrayLiteral(arguments));
    let guard = GraphGuard::capture(&program, &[root], &[], &[], &[]).unwrap();
    guard.validate(&program).unwrap();
    *program.expression_table.expression_mut(first) = ExpressionNode::Boolean(true);
    assert!(guard.validate(&program).is_err());
    *program.expression_table.expression_mut(first) = ExpressionNode::Boolean(false);
    program
        .expression_table
        .set_expression_handle_at_offset(arguments, 0, second);
    assert!(guard.validate(&program).is_err());
}

#[test]
fn same_handle_operand_parameter_and_nested_type_mutations_are_detected() {
    let mut program = TypedTrees::default();
    let mut symbols = SymbolTableBuilder::new();
    let symbol = symbols.insert_root(SymbolKind::Parameter, SymbolNameRef::Borrowed("operand"));
    program.symbols = symbols.finish();
    let primitive = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let reference = program
        .type_reference_table
        .insert(TypeReferenceNode::Slice {
            element_type: primitive,
        });
    let parameter = program.state_parameters.insert(StateParameter {
        symbol,
        name: Identifier::generated("operand"),
        type_reference: reference,
        ..StateParameter::default()
    });
    let root = program
        .expression_table
        .insert(ExpressionNode::Name(TableNamePath {
            symbol,
            head_symbol: symbol,
            ..TableNamePath::default()
        }));
    let guard = GraphGuard::capture(&program, &[root], &[], &[], &[]).unwrap();
    program.type_reference_table.substitute_node(
        primitive,
        TypeReferenceNode::Named {
            symbol: SymbolHandle::invalid(),
            name: Identifier::generated("u64"),
        },
    );
    assert!(guard.validate(&program).is_err());
    program
        .type_reference_table
        .substitute_node(primitive, TypeReferenceNode::Unit);
    program.state_parameters.get_mut(parameter).type_reference = primitive;
    assert!(guard.validate(&program).is_err());
}

#[test]
fn cyclic_graphs_are_finite_and_unrelated_nodes_do_not_invalidate_custody() {
    let mut program = TypedTrees::default();
    let root = program
        .expression_table
        .insert(ExpressionNode::Boolean(false));
    *program.expression_table.expression_mut(root) = ExpressionNode::Unary(TableUnaryExpression {
        operator: UnaryOperator::LogicalNot,
        operand: root,
    });
    let guard = GraphGuard::capture(&program, &[root], &[], &[], &[]).unwrap();
    program
        .expression_table
        .insert(ExpressionNode::Boolean(true));
    guard.validate(&program).unwrap();
    assert_eq!(guard.expressions.len(), 1);
}

#[test]
fn stale_nonzero_handles_cannot_recover_equal_dummy_nodes() {
    let mut program = TypedTrees::default();
    let expression = program.expression_table.insert(ExpressionNode::default());
    let reference = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let expressions = GraphGuard::capture(&program, &[expression], &[], &[], &[]).unwrap();
    let types = GraphGuard::capture(&program, &[], &[reference], &[], &[]).unwrap();
    program.expression_table.clear();
    program.type_reference_table = typed_trees::types::TypeReferenceTable::new();
    assert!(expressions.validate(&program).is_err());
    assert!(types.validate(&program).is_err());
}

#[test]
fn detached_original_static_arguments_keep_exact_const_binder_type_custody() {
    let mut program = TypedTrees::default();
    let mut symbols = SymbolTableBuilder::new();
    let symbol = symbols.insert_root(SymbolKind::TypeParameter, SymbolNameRef::Borrowed("Count"));
    program.symbols = symbols.finish();
    let reference = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: SymbolHandle::invalid(),
            name: Identifier::generated("u64"),
        });
    program
        .data_type_parameters
        .insert(typed_trees::data::TypeParameter {
            symbol,
            name: Identifier::generated("Count"),
            kind: typed_trees::data::TypeParameterKind::Const {
                type_reference: reference,
            },
            ..typed_trees::data::TypeParameter::default()
        });
    let argument = StaticMachineArgument {
        path: Box::default(),
        application: None,
        type_reference: Default::default(),
        const_literal: None,
        evidence_projection: None,
        symbol,
    };
    let guard = GraphGuard::capture(&program, &[], &[], &[], &[argument]).unwrap();
    guard.validate(&program).unwrap();
    program.type_reference_table.substitute_node(
        reference,
        TypeReferenceNode::Named {
            symbol: SymbolHandle::invalid(),
            name: Identifier::generated("u32"),
        },
    );
    assert!(guard.validate(&program).is_err());
}

// ---------------------------------------------------------------------------
// Record-side one-field substitution matrix over the retained `GraphGuard`.
//
// `GraphGuard::validate` is the containing-identity replay: it recaptures the
// four root rosters against the settled program and compares every retained
// field wholesale. Every representable field of the guard is substituted
// independently below — each roots roster (omission, duplication, reordering,
// foreign and inert entries), each snapshot roster, and each snapshot row
// field, the latter through rows captured under a program mutated in exactly
// that aspect. Substitutions either reject on recapture or are named
// canonicalizations of inert envelope content.
// ---------------------------------------------------------------------------

fn expression_rows(program: &TypedTrees, roots: &[ExpressionHandle]) -> Vec<expressions::Snapshot> {
    GraphGuard::capture(program, roots, &[], &[], &[])
        .expect("expression row capture")
        .expressions
}

fn expression_row(program: &TypedTrees, root: ExpressionHandle) -> expressions::Snapshot {
    expression_rows(program, &[root])
        .into_iter()
        .next()
        .expect("the root row captures first")
}

fn type_rows(program: &TypedTrees, roots: &[TypeReferenceHandle]) -> Vec<types::Snapshot> {
    GraphGuard::capture(program, &[], roots, &[], &[])
        .expect("type row capture")
        .types
}

fn type_row(program: &TypedTrees, root: TypeReferenceHandle) -> types::Snapshot {
    type_rows(program, &[root])
        .into_iter()
        .next()
        .expect("the root row captures first")
}

fn binding_rows(program: &TypedTrees, roots: &[SymbolHandle]) -> Vec<bindings::Snapshot> {
    GraphGuard::capture(program, &[], &[], roots, &[])
        .expect("binding row capture")
        .bindings
}

fn binding_row(program: &TypedTrees, symbol: SymbolHandle) -> bindings::Snapshot {
    binding_rows(program, &[symbol])
        .into_iter()
        .next()
        .expect("one binding row")
}

fn static_argument(symbol: SymbolHandle) -> StaticMachineArgument {
    StaticMachineArgument {
        path: Box::default(),
        application: None,
        type_reference: Default::default(),
        const_literal: None,
        evidence_projection: None,
        symbol,
    }
}

#[test]
fn guard_expression_roots_reject_every_one_field_substitution() {
    let mut program = TypedTrees::default();
    let left = program
        .expression_table
        .insert(ExpressionNode::Boolean(false));
    let right = program
        .expression_table
        .insert(ExpressionNode::Boolean(true));
    let extra = program
        .expression_table
        .insert(ExpressionNode::Boolean(false));
    let guard = GraphGuard::capture(&program, &[left, right], &[], &[], &[]).unwrap();
    guard.validate(&program).unwrap();

    let mut mutated = guard.clone();
    mutated.expression_roots.pop();
    assert!(
        mutated.validate(&program).is_err(),
        "omitted expression root"
    );
    let mut mutated = guard.clone();
    mutated.expression_roots[0] = extra;
    assert!(
        mutated.validate(&program).is_err(),
        "foreign expression root"
    );
    let mut mutated = guard.clone();
    mutated.expression_roots.swap(0, 1);
    assert!(
        mutated.validate(&program).is_err(),
        "reordered expression roots"
    );
    let mut mutated = guard.clone();
    mutated.expression_roots.push(extra);
    assert!(
        mutated.validate(&program).is_err(),
        "appended uncovered expression root"
    );
    // An already-covered or invalid root is inert envelope content: traversal
    // deduplicates it and the retained roster compares verbatim.
    let mut mutated = guard.clone();
    mutated.expression_roots.push(left);
    mutated
        .validate(&program)
        .expect("duplicated covered root canonicalizes");
    let mut mutated = guard.clone();
    mutated.expression_roots.push(ExpressionHandle::invalid());
    mutated
        .validate(&program)
        .expect("invalid expression root is inert");
    // A covered child substituted for its parent is still a different root.
    let mut program = TypedTrees::default();
    let operand = program
        .expression_table
        .insert(ExpressionNode::Boolean(false));
    let root = program
        .expression_table
        .insert(ExpressionNode::Unary(TableUnaryExpression {
            operator: UnaryOperator::LogicalNot,
            operand,
        }));
    let guard = GraphGuard::capture(&program, &[root], &[], &[], &[]).unwrap();
    let mut mutated = guard.clone();
    mutated.expression_roots[0] = operand;
    assert!(
        mutated.validate(&program).is_err(),
        "a covered child substituted for its parent root"
    );
}

#[test]
fn guard_type_roots_reject_every_one_field_substitution() {
    let mut program = TypedTrees::default();
    let unit = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let slice = program
        .type_reference_table
        .insert(TypeReferenceNode::Slice { element_type: unit });
    let named = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: SymbolHandle::invalid(),
            name: Identifier::generated("u64"),
        });
    let guard = GraphGuard::capture(&program, &[], &[unit, slice], &[], &[]).unwrap();
    guard.validate(&program).unwrap();

    let mut mutated = guard.clone();
    mutated.type_roots.pop();
    assert!(mutated.validate(&program).is_err(), "omitted type root");
    let mut mutated = guard.clone();
    mutated.type_roots[0] = named;
    assert!(mutated.validate(&program).is_err(), "foreign type root");
    let mut mutated = guard.clone();
    mutated.type_roots.swap(0, 1);
    assert!(mutated.validate(&program).is_err(), "reordered type roots");
    let mut mutated = guard.clone();
    mutated.type_roots.push(named);
    assert!(
        mutated.validate(&program).is_err(),
        "appended uncovered type root"
    );
    let mut mutated = guard.clone();
    mutated.type_roots.push(unit);
    mutated
        .validate(&program)
        .expect("duplicated covered root canonicalizes");
    let mut mutated = guard.clone();
    mutated.type_roots.push(TypeReferenceHandle::invalid());
    mutated
        .validate(&program)
        .expect("invalid type root is inert");
    // A covered child substituted for its parent is still a different root.
    let mut mutated = guard.clone();
    mutated.type_roots[1] = unit;
    assert!(
        mutated.validate(&program).is_err(),
        "a covered element type substituted for its slice root"
    );
}

#[test]
fn guard_symbol_roots_reject_every_one_field_substitution() {
    let mut program = TypedTrees::default();
    let mut symbols = SymbolTableBuilder::new();
    let first = symbols.insert_root(SymbolKind::Trait, SymbolNameRef::Borrowed("First"));
    let second = symbols.insert_root(SymbolKind::Trait, SymbolNameRef::Borrowed("Second"));
    let foreign = symbols.insert_root(SymbolKind::Trait, SymbolNameRef::Borrowed("Foreign"));
    program.symbols = symbols.finish();
    let guard = GraphGuard::capture(&program, &[], &[], &[first, second], &[]).unwrap();
    guard.validate(&program).unwrap();

    let mut mutated = guard.clone();
    mutated.symbol_roots.pop();
    assert!(mutated.validate(&program).is_err(), "omitted symbol root");
    let mut mutated = guard.clone();
    mutated.symbol_roots[0] = foreign;
    assert!(mutated.validate(&program).is_err(), "foreign symbol root");
    let mut mutated = guard.clone();
    mutated.symbol_roots.swap(0, 1);
    assert!(
        mutated.validate(&program).is_err(),
        "reordered symbol roots"
    );
    let mut mutated = guard.clone();
    mutated.symbol_roots.push(foreign);
    assert!(
        mutated.validate(&program).is_err(),
        "appended uncovered symbol root"
    );
    let mut mutated = guard.clone();
    mutated.symbol_roots.push(first);
    mutated
        .validate(&program)
        .expect("duplicated covered root canonicalizes");
    let mut mutated = guard.clone();
    mutated.symbol_roots.push(SymbolHandle::invalid());
    mutated
        .validate(&program)
        .expect("invalid symbol root is inert");
}

#[test]
fn guard_static_roots_reject_every_one_field_substitution() {
    let mut program = TypedTrees::default();
    let mut symbols = SymbolTableBuilder::new();
    let first = symbols.insert_root(SymbolKind::Trait, SymbolNameRef::Borrowed("First"));
    let second = symbols.insert_root(SymbolKind::Trait, SymbolNameRef::Borrowed("Second"));
    let foreign = symbols.insert_root(SymbolKind::Trait, SymbolNameRef::Borrowed("Foreign"));
    program.symbols = symbols.finish();
    let first_argument = static_argument(first);
    let second_argument = static_argument(second);
    let guard = GraphGuard::capture(
        &program,
        &[],
        &[],
        &[],
        &[first_argument.clone(), second_argument.clone()],
    )
    .unwrap();
    guard.validate(&program).unwrap();

    let mut mutated = guard.clone();
    mutated.static_roots.pop();
    assert!(mutated.validate(&program).is_err(), "omitted static root");
    let mut mutated = guard.clone();
    mutated.static_roots[0].symbol = foreign;
    assert!(
        mutated.validate(&program).is_err(),
        "foreign static argument symbol"
    );
    let mut mutated = guard.clone();
    mutated.static_roots[0].application =
        Some(Box::new(typed_trees::expression::StaticSymbolApplication {
            lifetime_arguments: Box::default(),
            arguments: vec![static_argument(foreign)].into_boxed_slice(),
        }));
    assert!(
        mutated.validate(&program).is_err(),
        "a nested static application walks its own symbols"
    );
    // Static roots walk last-to-first and each argument queues its symbol:
    // a trailing duplicate of the earlier argument re-seats that symbol's
    // queue position and reorders the captured bindings, while duplicates
    // visited after their symbol was already queued deduplicate in place.
    let mut mutated = guard.clone();
    mutated.static_roots.push(first_argument.clone());
    assert!(
        mutated.validate(&program).is_err(),
        "a trailing duplicate of the earlier static root reorders captured bindings"
    );
    let mut mutated = guard.clone();
    mutated.static_roots.push(second_argument.clone());
    mutated
        .validate(&program)
        .expect("a duplicated trailing static root deduplicates");
    let mut mutated = guard.clone();
    mutated.static_roots.insert(0, first_argument.clone());
    mutated
        .validate(&program)
        .expect("a duplicated leading static root deduplicates");
    // The retained argument fields outside the walked symbol graph are
    // provenance, not custody: path, const literal, and evidence projection
    // compare verbatim against the same recorded roster.
    let mut mutated = guard.clone();
    mutated.static_roots[0].path = vec![Identifier::generated("forged")].into_boxed_slice();
    mutated
        .validate(&program)
        .expect("the static argument path is retained provenance");
    let mut mutated = guard.clone();
    mutated.static_roots[0].const_literal = Some(numerics::literals::IntegerLiteral::zero());
    mutated
        .validate(&program)
        .expect("the static const literal is retained provenance");
    let mut mutated = guard.clone();
    mutated.static_roots[0].evidence_projection =
        Some(typed_trees::expression::EvidenceProjection {
            term: Identifier::generated("term"),
            member: Identifier::generated("member"),
        });
    mutated
        .validate(&program)
        .expect("the static evidence projection is retained provenance");
    // A static root whose symbol is independently covered by the symbol
    // roster carries no unique custody: dropping it canonicalizes.
    let covered =
        GraphGuard::capture(&program, &[], &[], &[first], &[static_argument(first)]).unwrap();
    let mut mutated = covered.clone();
    mutated.static_roots.pop();
    mutated
        .validate(&program)
        .expect("a covered static root carries no unique custody");
    // An uncovered static root cannot be dropped through the same route.
    let uncovered = GraphGuard::capture(
        &program,
        &[],
        &[],
        &[second],
        &[first_argument, second_argument],
    )
    .unwrap();
    let mut mutated = uncovered;
    mutated.static_roots.pop();
    assert!(
        mutated.validate(&program).is_err(),
        "an uncovered static root is committed"
    );
}

#[test]
fn guard_snapshot_rosters_reject_every_one_field_substitution() {
    let mut program = TypedTrees::default();
    let left = program
        .expression_table
        .insert(ExpressionNode::Boolean(false));
    let right = program
        .expression_table
        .insert(ExpressionNode::Boolean(true));
    let twin = program
        .expression_table
        .insert(ExpressionNode::Boolean(false));
    let unit = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let slice = program
        .type_reference_table
        .insert(TypeReferenceNode::Slice { element_type: unit });
    let named = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: SymbolHandle::invalid(),
            name: Identifier::generated("u64"),
        });
    let mut symbols = SymbolTableBuilder::new();
    let first = symbols.insert_root(SymbolKind::Trait, SymbolNameRef::Borrowed("First"));
    let second = symbols.insert_root(SymbolKind::Trait, SymbolNameRef::Borrowed("Second"));
    let foreign = symbols.insert_root(SymbolKind::Trait, SymbolNameRef::Borrowed("Foreign"));
    program.symbols = symbols.finish();

    // Expression snapshot roster: capture order is committed.
    let guard = GraphGuard::capture(&program, &[left, right], &[], &[], &[]).unwrap();
    let mut mutated = guard.clone();
    mutated.expressions.pop();
    assert!(
        mutated.validate(&program).is_err(),
        "dropped expression row"
    );
    let mut mutated = guard.clone();
    let row = mutated.expressions[0].clone();
    mutated.expressions.push(row);
    assert!(
        mutated.validate(&program).is_err(),
        "duplicated expression row"
    );
    let mut mutated = guard.clone();
    mutated.expressions.swap(0, 1);
    assert!(
        mutated.validate(&program).is_err(),
        "reordered expression rows"
    );
    let mut mutated = guard.clone();
    mutated.expressions[0] = expression_row(&program, twin);
    assert!(
        mutated.validate(&program).is_err(),
        "foreign expression row with coinciding content"
    );

    // Type snapshot roster.
    let guard = GraphGuard::capture(&program, &[], &[unit, slice], &[], &[]).unwrap();
    let mut mutated = guard.clone();
    mutated.types.pop();
    assert!(mutated.validate(&program).is_err(), "dropped type row");
    let mut mutated = guard.clone();
    let row = mutated.types[0].clone();
    mutated.types.push(row);
    assert!(mutated.validate(&program).is_err(), "duplicated type row");
    let mut mutated = guard.clone();
    mutated.types.swap(0, 1);
    assert!(mutated.validate(&program).is_err(), "reordered type rows");
    let mut mutated = guard.clone();
    mutated.types[0] = type_row(&program, named);
    assert!(mutated.validate(&program).is_err(), "foreign type row");

    // Binding snapshot roster.
    let guard = GraphGuard::capture(&program, &[], &[], &[first, second], &[]).unwrap();
    let mut mutated = guard.clone();
    mutated.bindings.pop();
    assert!(mutated.validate(&program).is_err(), "dropped binding row");
    let mut mutated = guard.clone();
    let row = mutated.bindings[0].clone();
    mutated.bindings.push(row);
    assert!(
        mutated.validate(&program).is_err(),
        "duplicated binding row"
    );
    let mut mutated = guard.clone();
    mutated.bindings.swap(0, 1);
    assert!(
        mutated.validate(&program).is_err(),
        "reordered binding rows"
    );
    let mut mutated = guard.clone();
    mutated.bindings[0] = binding_row(&program, foreign);
    assert!(mutated.validate(&program).is_err(), "foreign binding row");
}

#[test]
fn guard_expression_rows_reject_every_single_field_substitution() {
    let mut program = TypedTrees::default();
    let site = program
        .expression_table
        .insert(ExpressionNode::Boolean(false));
    let twin = program
        .expression_table
        .insert(ExpressionNode::Boolean(false));
    let argument_a = program
        .expression_table
        .insert(ExpressionNode::Boolean(false));
    let argument_b = program
        .expression_table
        .insert(ExpressionNode::Boolean(true));
    let argument_foreign = program
        .expression_table
        .insert(ExpressionNode::Boolean(true));
    let mut symbols = SymbolTableBuilder::new();
    let member_a = symbols.insert_root(SymbolKind::Trait, SymbolNameRef::Borrowed("MemberA"));
    let member_b = symbols.insert_root(SymbolKind::Trait, SymbolNameRef::Borrowed("MemberB"));
    program.symbols = symbols.finish();
    let mut members = HandleSpan::empty();
    program
        .expression_table
        .push_name_path_member(&mut members, Identifier::generated("one"));
    program
        .expression_table
        .push_name_path_member(&mut members, Identifier::generated("two"));
    let mut member_symbols = HandleSpan::empty();
    program
        .expression_table
        .push_name_path_member_symbol(&mut member_symbols, member_a);
    let name = program
        .expression_table
        .insert(ExpressionNode::Name(TableNamePath {
            members,
            member_symbols,
            head_symbol: SymbolHandle::invalid(),
            symbol: SymbolHandle::invalid(),
        }));
    let field_value = program
        .expression_table
        .insert(ExpressionNode::Boolean(false));
    let field = TableStructLiteralField {
        name: Identifier::generated("field"),
        field_symbol: SymbolHandle::invalid(),
        value: field_value,
    };
    let fields = program.expression_table.insert_struct_fields([field]);
    let structure = program
        .expression_table
        .insert(ExpressionNode::StructLiteral(TableStructLiteral {
            fields,
            ..TableStructLiteral::default()
        }));
    let arguments = program
        .expression_table
        .insert_expression_handles([argument_a, argument_b]);
    let array = program
        .expression_table
        .insert(ExpressionNode::ArrayLiteral(arguments));
    let domain_argument = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let foreign_domain_argument = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: SymbolHandle::invalid(),
            name: Identifier::generated("u8"),
        });
    let domain_arguments = program
        .type_reference_table
        .insert_type_reference_handles([domain_argument]);
    let cast = program
        .expression_table
        .insert(ExpressionNode::Cast(TableCastExpression {
            value: site,
            target_type: domain_argument,
            result_type: domain_argument,
            target_label: HandleSpan::empty(),
            domain: numerics::arithmetic::ArithmeticDomain::Exact,
            semantic_domain: HandleSpan::empty(),
            semantic_domain_arguments: domain_arguments,
            semantic_domain_symbol: SymbolHandle::invalid(),
            semantic_domain_id: language_semantics::SemanticDomainId::NULL,
            form: language_core::cast_form::CastForm::Value,
        }));

    // `handle`: an identical node at a different handle is a different row.
    let guard = GraphGuard::capture(&program, &[site], &[], &[], &[]).unwrap();
    let mut mutated = guard.clone();
    mutated.expressions[0] = expression_row(&program, twin);
    assert!(mutated.validate(&program).is_err(), "expression row handle");

    // `node`: the same handle carrying a different node is a different row.
    let mut drifted = program.clone();
    *drifted.expression_table.expression_mut(site) = ExpressionNode::Boolean(true);
    let mut mutated = guard.clone();
    mutated.expressions[0] = expression_row(&drifted, site);
    assert!(mutated.validate(&program).is_err(), "expression row node");

    // `arguments`: equal span identity with different span contents is a
    // different row even though the node compares equal.
    let guard = GraphGuard::capture(&program, &[array], &[], &[], &[]).unwrap();
    let mut drifted = program.clone();
    drifted
        .expression_table
        .set_expression_handle_at_offset(arguments, 0, argument_foreign);
    let mut mutated = guard.clone();
    mutated.expressions[0] = expression_row(&drifted, array);
    assert!(
        mutated.validate(&program).is_err(),
        "expression row arguments"
    );

    // `names`: an equal members span whose stored names differ.
    let guard = GraphGuard::capture(&program, &[name], &[], &[], &[]).unwrap();
    let mut drifted = program.clone();
    drifted.expression_table.set_name_path_member_at_offset(
        members,
        0,
        Identifier::generated("forged"),
    );
    let mut mutated = guard.clone();
    mutated.expressions[0] = expression_row(&drifted, name);
    assert!(mutated.validate(&program).is_err(), "expression row names");

    // `members`: an equal member-symbols span whose member symbols differ.
    let mut drifted = program.clone();
    drifted
        .expression_table
        .set_name_path_member_symbol_at_offset(member_symbols, 0, member_b);
    let mut mutated = guard.clone();
    mutated.expressions[0] = expression_row(&drifted, name);
    assert!(
        mutated.validate(&program).is_err(),
        "expression row member symbols"
    );

    // `fields`: an equal struct-field span whose stored field rows differ.
    let guard = GraphGuard::capture(&program, &[structure], &[], &[], &[]).unwrap();
    let mut drifted = program.clone();
    drifted.expression_table.set_struct_field_at_offset(
        fields,
        0,
        TableStructLiteralField {
            name: Identifier::generated("forged"),
            field_symbol: SymbolHandle::invalid(),
            value: field_value,
        },
    );
    let mut mutated = guard.clone();
    mutated.expressions[0] = expression_row(&drifted, structure);
    assert!(mutated.validate(&program).is_err(), "expression row fields");

    // `type_arguments`: an equal domain-arguments span whose stored type
    // arguments differ.
    let guard = GraphGuard::capture(&program, &[cast], &[], &[], &[]).unwrap();
    let mut drifted = program.clone();
    drifted
        .type_reference_table
        .set_type_reference_handle_at_offset(domain_arguments, 0, foreign_domain_argument);
    let mut mutated = guard.clone();
    mutated.expressions[0] = expression_row(&drifted, cast);
    assert!(
        mutated.validate(&program).is_err(),
        "expression row type arguments"
    );

    // `match_arms`: retained arm rows differ under an equal arm span.
    let arm_span = |source_span| {
        let mut program = TypedTrees::default();
        let subject = program
            .expression_table
            .insert(ExpressionNode::Boolean(false));
        let pattern = program
            .expression_table
            .insert(ExpressionNode::Boolean(false));
        let value = program
            .expression_table
            .insert(ExpressionNode::Boolean(true));
        let arms = program.expression_table.insert_match_arms([TableMatchArm {
            pattern: MatchPattern::Value(pattern),
            value,
            source_span,
        }]);
        let root = program
            .expression_table
            .insert(ExpressionNode::Match(TableMatchExpression {
                subject,
                arms,
            }));
        (program, root)
    };
    let (honest, honest_root) = arm_span(SourceSpan::new(SourceId(0), Span::new(0, 4)));
    let (drifted, drifted_root) = arm_span(SourceSpan::new(SourceId(0), Span::new(1, 4)));
    let guard = GraphGuard::capture(&honest, &[honest_root], &[], &[], &[]).unwrap();
    let mut mutated = guard.clone();
    mutated.expressions[0] = expression_row(&drifted, drifted_root);
    assert!(
        mutated.validate(&honest).is_err(),
        "expression row match arms"
    );
}

#[test]
fn guard_type_rows_reject_every_single_field_substitution() {
    let mut program = TypedTrees::default();
    let unit = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let twin = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let element = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let foreign_element = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: SymbolHandle::invalid(),
            name: Identifier::generated("u8"),
        });
    let generic_arguments = program
        .type_reference_table
        .insert_type_reference_handles([element]);
    let generic = program
        .type_reference_table
        .insert(TypeReferenceNode::Generic {
            base_symbol: SymbolHandle::invalid(),
            base_name: Identifier::generated("Generic"),
            lifetime_arguments: Vec::new(),
            arguments: generic_arguments,
        });
    let constraints = program
        .type_reference_table
        .insert_constraints([TypeConstraintNode::Named(Identifier::generated("Bound"))]);
    let constrained = program
        .type_reference_table
        .insert(TypeReferenceNode::Constrained {
            base_type: unit,
            constraints,
        });

    // `handle`: an identical node at a different handle is a different row.
    let guard = GraphGuard::capture(&program, &[], &[unit], &[], &[]).unwrap();
    let mut mutated = guard.clone();
    mutated.types[0] = type_row(&program, twin);
    assert!(mutated.validate(&program).is_err(), "type row handle");

    // `node`: the same handle carrying a different node is a different row.
    let mut drifted = program.clone();
    drifted.type_reference_table.substitute_node(
        unit,
        TypeReferenceNode::Named {
            symbol: SymbolHandle::invalid(),
            name: Identifier::generated("u32"),
        },
    );
    let mut mutated = guard.clone();
    mutated.types[0] = type_row(&drifted, unit);
    assert!(mutated.validate(&program).is_err(), "type row node");

    // `arguments`: an equal generic-arguments span whose stored type
    // arguments differ.
    let guard = GraphGuard::capture(&program, &[], &[generic], &[], &[]).unwrap();
    let mut drifted = program.clone();
    drifted
        .type_reference_table
        .set_type_reference_handle_at_offset(generic_arguments, 0, foreign_element);
    let mut mutated = guard.clone();
    mutated.types[0] = type_row(&drifted, generic);
    assert!(mutated.validate(&program).is_err(), "type row arguments");

    // `constraints`: an equal constraint span whose stored constraints differ.
    let guard = GraphGuard::capture(&program, &[], &[constrained], &[], &[]).unwrap();
    let mut drifted = program.clone();
    drifted.type_reference_table.set_constraint_at_offset(
        constraints,
        0,
        TypeConstraintNode::Named(Identifier::generated("Forged")),
    );
    let mut mutated = guard.clone();
    mutated.types[0] = type_row(&drifted, constrained);
    assert!(mutated.validate(&program).is_err(), "type row constraints");
}

/// One `Parameter` binding fixture whose `StateParameter` row carries `name`.
fn parameter_program(parameter_name: &'static str) -> (TypedTrees, SymbolHandle) {
    let mut program = TypedTrees::default();
    let mut symbols = SymbolTableBuilder::new();
    let symbol = symbols.insert_root(SymbolKind::Parameter, SymbolNameRef::Borrowed("operand"));
    program.symbols = symbols.finish();
    let type_reference = program.type_reference_table.insert(TypeReferenceNode::Unit);
    program.state_parameters.insert(StateParameter {
        symbol,
        name: Identifier::generated(parameter_name),
        type_reference,
        ..StateParameter::default()
    });
    (program, symbol)
}

/// One `Field` binding fixture whose `DataMember::Field` row carries `name`.
fn field_program(field_name: &'static str) -> (TypedTrees, SymbolHandle) {
    let mut program = TypedTrees::default();
    let mut symbols = SymbolTableBuilder::new();
    let symbol = symbols.insert_root(SymbolKind::Field, SymbolNameRef::Borrowed("field"));
    program.symbols = symbols.finish();
    program.data_members.insert(DataMember::Field(DataField {
        symbol,
        name: Identifier::generated(field_name),
        type_reference: TypeReferenceHandle::invalid(),
        ..DataField::default()
    }));
    (program, symbol)
}

/// One `TypeParameter` binding fixture whose `TypeParameter` row carries `name`.
fn static_binder_program(binder_name: &'static str) -> (TypedTrees, SymbolHandle) {
    let mut program = TypedTrees::default();
    let mut symbols = SymbolTableBuilder::new();
    let symbol = symbols.insert_root(SymbolKind::TypeParameter, SymbolNameRef::Borrowed("Count"));
    program.symbols = symbols.finish();
    program.data_type_parameters.insert(TypeParameter {
        symbol,
        name: Identifier::generated(binder_name),
        kind: TypeParameterKind::Type,
        ..TypeParameter::default()
    });
    (program, symbol)
}

/// One `Const` binding fixture whose declaration selects `declared_type`.
fn const_program(use_second: bool) -> (TypedTrees, SymbolHandle) {
    let mut program = TypedTrees::default();
    let mut symbols = SymbolTableBuilder::new();
    let symbol = symbols.insert_root(SymbolKind::Const, SymbolNameRef::Borrowed("C"));
    program.symbols = symbols.finish();
    let first = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let second = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: SymbolHandle::invalid(),
            name: Identifier::generated("u8"),
        });
    program.const_declarations.insert(ConstDeclaration {
        symbol,
        declared_type: if use_second { second } else { first },
        ..ConstDeclaration::default()
    });
    (program, symbol)
}

/// One `Local` binding fixture whose statement row carries `is_mutable`.
fn local_program(is_mutable: bool) -> (TypedTrees, SymbolHandle) {
    let mut program = TypedTrees::default();
    let mut symbols = SymbolTableBuilder::new();
    let symbol = symbols.insert_root(SymbolKind::Local, SymbolNameRef::Borrowed("binding"));
    let machine = symbols.insert_root(SymbolKind::Machine, SymbolNameRef::Borrowed("Host"));
    program.symbols = symbols.finish();
    let mut statement_nodes = HandleSpan::empty();
    program.statement_table.push_statement(
        &mut statement_nodes,
        StatementNode::LocalData(TableLocalData {
            symbol,
            is_mutable,
            type_reference: TypeReferenceHandle::invalid(),
            ..TableLocalData::default()
        }),
    );
    program.machine_states.insert(State {
        symbol: machine,
        statement_nodes,
        ..State::default()
    });
    (program, symbol)
}

/// One trait binding fixture whose root symbol carries `kind` and `name`.
fn named_program(kind: SymbolKind, name: &'static str) -> (TypedTrees, SymbolHandle) {
    let mut program = TypedTrees::default();
    let mut symbols = SymbolTableBuilder::new();
    let symbol = symbols.insert_root(kind, SymbolNameRef::Borrowed(name));
    program.symbols = symbols.finish();
    (program, symbol)
}

/// One trait binding fixture whose symbol name retains `source_span`.
fn spanned_program(source_span: SourceSpan) -> (TypedTrees, SymbolHandle) {
    let mut program = TypedTrees::default();
    let mut symbols = SymbolTableBuilder::new();
    let symbol = symbols.insert_root(
        SymbolKind::Trait,
        SymbolNameRef::OwnedSource {
            value: "subject",
            source_span,
        },
    );
    program.symbols = symbols.finish();
    (program, symbol)
}

/// One trait binding fixture whose declared source file carries `package`.
fn packaged_program(package: PackageKeyIdentity) -> (TypedTrees, SymbolHandle) {
    let mut sources = SourceMap::default();
    let file = sources.add_with_metadata(
        PathBuf::from("owned/subject.omg"),
        String::from("subject"),
        PathBuf::from("owned"),
        Some(package),
        SourceOrigin::User,
    );
    let source_span = file.source_span(Span::new(0, 7));
    let mut program = TypedTrees::default();
    let mut symbols = SymbolTableBuilder::with_sources(Some(Arc::new(sources)));
    let symbol = symbols.insert_root(
        SymbolKind::Trait,
        SymbolNameRef::OwnedSource {
            value: "subject",
            source_span,
        },
    );
    program.symbols = symbols.finish();
    (program, symbol)
}

#[test]
fn guard_binding_rows_reject_every_single_field_substitution() {
    // `symbol`: the row's identity is its symbol — a foreign symbol's binding
    // row cannot stand in, and no same-content different-symbol row is
    // representable.
    let (program, symbol) = named_program(SymbolKind::Trait, "subject");
    let guard = GraphGuard::capture(&program, &[], &[], &[symbol], &[]).unwrap();
    let (foreign_program, foreign) = named_program(SymbolKind::Trait, "foreign");
    let mut mutated = guard.clone();
    mutated.bindings[0] = binding_row(&foreign_program, foreign);
    assert!(mutated.validate(&program).is_err(), "binding row symbol");

    // `declaration`: the same symbol name under a different kind is a
    // different retained declaration.
    let (drifted, drifted_symbol) = named_program(SymbolKind::Module, "subject");
    let mut mutated = guard.clone();
    mutated.bindings[0] = binding_row(&drifted, drifted_symbol);
    assert!(
        mutated.validate(&program).is_err(),
        "binding row declaration"
    );

    // `name`: the same declaration position under a different resolved
    // spelling is a different row.
    let (drifted, drifted_symbol) = named_program(SymbolKind::Trait, "forged");
    let mut mutated = guard.clone();
    mutated.bindings[0] = binding_row(&drifted, drifted_symbol);
    assert!(mutated.validate(&program).is_err(), "binding row name");

    // `source_span`: the same spelling under a different retained declaration
    // span is a different row.
    let (program, symbol) = spanned_program(SourceSpan::new(SourceId(0), Span::new(0, 7)));
    let guard = GraphGuard::capture(&program, &[], &[], &[symbol], &[]).unwrap();
    let (drifted, drifted_symbol) = spanned_program(SourceSpan::new(SourceId(0), Span::new(1, 7)));
    let mut mutated = guard.clone();
    mutated.bindings[0] = binding_row(&drifted, drifted_symbol);
    assert!(
        mutated.validate(&program).is_err(),
        "binding row source span"
    );

    // `package`: the same declaration span under a different reconciled
    // package identity is a different row.
    let (program, symbol) = packaged_program(PackageKeyIdentity::from_digest([1; 32]).unwrap());
    let guard = GraphGuard::capture(&program, &[], &[], &[symbol], &[]).unwrap();
    let (drifted, drifted_symbol) =
        packaged_program(PackageKeyIdentity::from_digest([2; 32]).unwrap());
    let mut mutated = guard.clone();
    mutated.bindings[0] = binding_row(&drifted, drifted_symbol);
    assert!(mutated.validate(&program).is_err(), "binding row package");

    // `parameters`: a substituted operand-parameter row is a different row.
    let (program, symbol) = parameter_program("operand");
    let guard = GraphGuard::capture(&program, &[], &[], &[symbol], &[]).unwrap();
    let (drifted, drifted_symbol) = parameter_program("forged");
    let mut mutated = guard.clone();
    mutated.bindings[0] = binding_row(&drifted, drifted_symbol);
    assert!(
        mutated.validate(&program).is_err(),
        "binding row parameters"
    );

    // `fields`: a substituted field-member row is a different row.
    let (program, symbol) = field_program("field");
    let guard = GraphGuard::capture(&program, &[], &[], &[symbol], &[]).unwrap();
    let (drifted, drifted_symbol) = field_program("forged");
    let mut mutated = guard.clone();
    mutated.bindings[0] = binding_row(&drifted, drifted_symbol);
    assert!(mutated.validate(&program).is_err(), "binding row fields");

    // `static_parameters`: a substituted static-binder row is a different row.
    let (program, symbol) = static_binder_program("Count");
    let guard = GraphGuard::capture(&program, &[], &[], &[symbol], &[]).unwrap();
    let (drifted, drifted_symbol) = static_binder_program("Forged");
    let mut mutated = guard.clone();
    mutated.bindings[0] = binding_row(&drifted, drifted_symbol);
    assert!(
        mutated.validate(&program).is_err(),
        "binding row static parameters"
    );

    // `types`: a substituted declared-type identity is a different row.
    let (program, symbol) = const_program(false);
    let guard = GraphGuard::capture(&program, &[], &[], &[symbol], &[]).unwrap();
    let (drifted, drifted_symbol) = const_program(true);
    let mut mutated = guard.clone();
    mutated.bindings[0] = binding_row(&drifted, drifted_symbol);
    assert!(mutated.validate(&program).is_err(), "binding row types");

    // `local_mutability`: a substituted mutability flag is a different row.
    let (program, symbol) = local_program(false);
    let guard = GraphGuard::capture(&program, &[], &[], &[symbol], &[]).unwrap();
    let (drifted, drifted_symbol) = local_program(true);
    let mut mutated = guard.clone();
    mutated.bindings[0] = binding_row(&drifted, drifted_symbol);
    assert!(
        mutated.validate(&program).is_err(),
        "binding row local mutability"
    );
}

#[test]
fn guard_root_budget_rejects_over_ceiling_rosters_on_capture_and_replay() {
    let mut program = TypedTrees::default();
    let leaf = program
        .expression_table
        .insert(ExpressionNode::Boolean(false));
    let over_budget = vec![leaf; MAX_NODES + 1];
    assert!(
        GraphGuard::capture(&program, &over_budget, &[], &[], &[]).is_err(),
        "an over-budget root roster cannot seal"
    );
    // A retained roster past the ceiling cannot even recompute identity.
    let mut guard = GraphGuard::capture(&program, &[leaf], &[], &[], &[]).unwrap();
    guard.expression_roots = over_budget;
    assert!(
        guard.validate(&program).is_err(),
        "an over-budget retained roster cannot replay"
    );
}
