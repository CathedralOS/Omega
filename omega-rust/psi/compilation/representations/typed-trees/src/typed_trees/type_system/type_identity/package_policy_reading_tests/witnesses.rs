//! One witness for each form the renderer writes: what to render, and the
//! package owners its text must expose to the owner check.

use super::fixture::Fixture;
use crate::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, MatchFieldAction, MatchPattern, NamePath,
    StaticMachineArgument, StaticSymbolApplication, TableAtomicExpression, TableBorrowExpression,
    TableDestructurePattern, TableIndexedExpression, TableMatchArm, TableMatchExpression,
    TableMatchField, TableMemberExpression, TableRangeExpression, TableSavedSubject,
    TableStructLiteral, TableStructLiteralField, TableUnaryExpression, UnaryOperator,
};
use crate::name::Identifier;
use crate::statement::OutcomeProofSelector;
use crate::types::{
    DomainConstraint, DomainConstraintSubject, FixedArrayLength, OmegaLayoutGrammar,
    TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};
use language_core::atomic::{AtomicExpressionResultCustody, AtomicOrderingPlan, MemoryOrdering};
use language_semantics::CarryPermission;
use language_semantics::const_value::CanonicalConstValue;
use language_semantics::value_domain::ValueDomain;
use numerics::arithmetic::ArithmeticDomain;
use numerics::literals::{FloatFormat, FloatLiteral, IntegerLiteral};
use source::SourceSpan;
use symbols::SymbolHandle;

/// One form rendered through the real renderer, and the packages that its text
/// must expose to the owner check.
pub(super) struct Witness {
    pub(super) form: &'static str,
    pub(super) reference: TypeReferenceHandle,
    pub(super) binders: Vec<(SymbolHandle, String)>,
    pub(super) substitutions: Vec<(SymbolHandle, TypeReferenceHandle)>,
    pub(super) owners: &'static [u8],
}

pub(super) fn witness(
    form: &'static str,
    reference: TypeReferenceHandle,
    owners: &'static [u8],
) -> Witness {
    Witness {
        form,
        reference,
        binders: Vec::new(),
        substitutions: Vec::new(),
        owners,
    }
}

pub(super) fn type_witnesses(fixture: &mut Fixture) -> Vec<Witness> {
    let n = fixture.symbol("N");
    let mut witnesses = Vec::new();

    witnesses.push(witness(
        "unit",
        fixture.type_node(TypeReferenceNode::Unit),
        &[],
    ));
    witnesses.push(witness("builtin", fixture.u64_type, &[]));
    let payload = fixture.named("Payload");
    witnesses.push(witness("nominal", payload, &[1]));
    let source = fixture.named("Source");
    witnesses.push(witness("toolchain nominal", source, &[]));
    for access in [
        language_core::ReferenceAccess::Shared,
        language_core::ReferenceAccess::Mutable,
        language_core::ReferenceAccess::WriteOnly,
    ] {
        let reference = fixture.type_node(TypeReferenceNode::Reference {
            referee: payload,
            access,
            lifetime: None,
        });
        witnesses.push(witness("reference", reference, &[1]));
    }
    let record = fixture.named("Record");
    let slice = fixture.type_node(TypeReferenceNode::Slice {
        element_type: record,
    });
    witnesses.push(witness("slice", slice, &[2]));

    witnesses.push(witness(
        "array of a literal extent",
        fixture.array(FixedArrayLength::Literal(4)),
        &[],
    ));
    let bound = fixture.array(FixedArrayLength::ConstParameter {
        symbol: n,
        name: Identifier::generated("N"),
    });
    witnesses.push(Witness {
        binders: vec![(n, "type-parameter:1".to_owned())],
        ..witness("array of a bound extent", bound, &[])
    });
    let five = fixture.spelled("5");
    witnesses.push(Witness {
        substitutions: vec![(n, five)],
        ..witness("array of a substituted extent", bound, &[])
    });
    let width = fixture.symbol("WIDTH");
    witnesses.push(witness(
        "array of a declared extent",
        fixture.array(FixedArrayLength::ConstParameter {
            symbol: width,
            name: Identifier::generated("WIDTH"),
        }),
        &[3],
    ));
    let sum = {
        let width = fixture.name("WIDTH");
        let one = fixture.integer(1);
        fixture.binary(width, BinaryOperator::Add, one)
    };
    let extent = fixture.index(sum);
    witnesses.push(witness(
        "array of an expression extent",
        fixture.array(FixedArrayLength::Expression(extent)),
        &[3],
    ));
    witnesses.push(witness(
        "array of a call extent",
        fixture.array(FixedArrayLength::ConstCall {
            machine: fixture.symbol("size"),
            name: Identifier::generated("size"),
            source_span: SourceSpan::default(),
        }),
        &[3],
    ));

    let limit = fixture.named("LIMIT");
    let applied = fixture.generic("Container", vec![record, five, limit]);
    witnesses.push(witness(
        "generic with type and const operands",
        applied,
        &[1, 2],
    ));
    let open = {
        let width = fixture.name("WIDTH");
        let one = fixture.integer(1);
        let sum = fixture.binary(width, BinaryOperator::Add, one);
        fixture.index(sum)
    };
    let two = {
        let two = fixture.integer(2);
        fixture.index(two)
    };
    let computed = fixture.generic("Container", vec![payload, open, two]);
    witnesses.push(witness("generic with computed operands", computed, &[1, 3]));
    let canonical = fixture.spelled(&CanonicalConstValue::boolean(true).atom());
    let constant = fixture.generic("Container", vec![payload, canonical, five]);
    witnesses.push(witness("generic with a canonical constant", constant, &[1]));
    let lifetime_only = fixture.generic("Container", Vec::new());
    witnesses.push(witness("generic without operands", lifetime_only, &[1]));

    let hooks = {
        let two = fixture.integer(7);
        let seven = fixture.index(two);
        let arguments = fixture
            .program
            .type_reference_table
            .insert_type_reference_handles(vec![payload, five, seven]);
        fixture.type_node(TypeReferenceNode::DynamicTrait {
            symbol: fixture.symbol("Hooks"),
            name: Identifier::generated("Hooks"),
            arguments,
        })
    };
    witnesses.push(witness("dynamic trait", hooks, &[1, 3]));
    witnesses.push(witness("integer constant", five, &[]));
    witnesses.push(witness("canonical constant", canonical, &[]));

    witnesses
}

pub(super) fn constraint_witnesses(fixture: &mut Fixture) -> Vec<Witness> {
    let payload = fixture.named("Payload");
    let eight = fixture.spelled("8");
    let domain = |subject, symbol: SymbolHandle, name: &str, arguments| DomainConstraint {
        name: Identifier::generated(name),
        arguments,
        subject,
        symbol,
        ..DomainConstraint::default()
    };
    let aligned = fixture.symbol("Aligned");
    let mut witnesses = Vec::new();
    fn add(
        witnesses: &mut Vec<Witness>,
        fixture: &mut Fixture,
        form: &'static str,
        constraints: Vec<TypeConstraintNode>,
        owners: &'static [u8],
    ) {
        let reference = fixture.constrained(constraints);
        witnesses.push(witness(form, reference, owners));
    }

    add(
        &mut witnesses,
        fixture,
        "named constraint",
        vec![TypeConstraintNode::Named(Identifier::generated("Even"))],
        &[],
    );
    add(
        &mut witnesses,
        fixture,
        "arithmetic domain",
        vec![TypeConstraintNode::ArithmeticDomain(
            ArithmeticDomain::Wrapping,
        )],
        &[],
    );
    add(
        &mut witnesses,
        fixture,
        "declared domain",
        vec![TypeConstraintNode::Domain(domain(
            DomainConstraintSubject::Declared,
            aligned,
            "Aligned",
            Vec::new(),
        ))],
        &[4],
    );
    add(
        &mut witnesses,
        fixture,
        "declared domain with operands",
        vec![TypeConstraintNode::Domain(domain(
            DomainConstraintSubject::Declared,
            aligned,
            "Aligned",
            vec![payload, eight],
        ))],
        &[1, 4],
    );
    for permission in CarryPermission::ALL {
        add(
            &mut witnesses,
            fixture,
            "carry domain",
            vec![TypeConstraintNode::Domain(domain(
                DomainConstraintSubject::Carry(permission),
                SymbolHandle::invalid(),
                "carry",
                Vec::new(),
            ))],
            &[],
        );
    }
    add(
        &mut witnesses,
        fixture,
        "value domain",
        vec![TypeConstraintNode::Domain(domain(
            DomainConstraintSubject::Value(ValueDomain::Finite),
            SymbolHandle::invalid(),
            "finite",
            Vec::new(),
        ))],
        &[],
    );
    add(
        &mut witnesses,
        fixture,
        "layout domain",
        vec![TypeConstraintNode::Domain(domain(
            DomainConstraintSubject::OmegaLayout {
                grammar: OmegaLayoutGrammar::Derived,
            },
            SymbolHandle::invalid(),
            "layout",
            vec![payload],
        ))],
        &[1],
    );
    add(
        &mut witnesses,
        fixture,
        "every kind of constraint at once",
        vec![
            TypeConstraintNode::Named(Identifier::generated("Even")),
            TypeConstraintNode::ArithmeticDomain(ArithmeticDomain::Saturating),
            TypeConstraintNode::Domain(domain(
                DomainConstraintSubject::Declared,
                aligned,
                "Aligned",
                vec![payload],
            )),
        ],
        &[1, 4],
    );
    witnesses
}

pub(super) fn expression_witnesses(fixture: &mut Fixture) -> Vec<Witness> {
    let mut witnesses = Vec::new();
    let mut add = |fixture: &mut Fixture,
                   form: &'static str,
                   expression: ExpressionHandle,
                   owners: &'static [u8]| {
        let reference = fixture.index(expression);
        witnesses.push(witness(form, reference, owners));
    };

    let name = fixture.name("LIMIT");
    add(fixture, "declared name", name, &[2]);
    let value = fixture.integer(7);
    add(fixture, "integer", value, &[]);
    let spelled =
        fixture
            .program
            .expression_table
            .insert_tree(&crate::expression::Expression::Name(NamePath::unresolved(
                vec![Identifier::generated("true")],
            )));
    add(fixture, "spelled Boolean name", spelled, &[]);
    let boolean = fixture.expression(ExpressionNode::Boolean(true));
    add(fixture, "Boolean", boolean, &[]);
    let float = fixture.expression(ExpressionNode::Float(FloatLiteral::new(1.5)));
    add(fixture, "float", float, &[]);
    let nan = fixture.expression(ExpressionNode::UndeterminedNaN(FloatFormat::F64));
    add(fixture, "undetermined NaN", nan, &[]);
    let string = fixture.expression(ExpressionNode::String(source::SourceBytes::from(
        &b"bytes"[..],
    )));
    add(fixture, "string", string, &[]);
    for operator in [UnaryOperator::BitwiseNot, UnaryOperator::LogicalNot] {
        let operand = fixture.name("WIDTH");
        let unary = fixture.expression(ExpressionNode::Unary(TableUnaryExpression {
            operator,
            operand,
        }));
        add(fixture, "unary", unary, &[3]);
    }
    for operator in [
        BinaryOperator::Add,
        BinaryOperator::And,
        BinaryOperator::BitwiseAnd,
        BinaryOperator::BitwiseOr,
        BinaryOperator::BitwiseXor,
        BinaryOperator::Divide,
        BinaryOperator::Equal,
        BinaryOperator::Greater,
        BinaryOperator::GreaterOrEqual,
        BinaryOperator::Less,
        BinaryOperator::LessOrEqual,
        BinaryOperator::Modulo,
        BinaryOperator::Multiply,
        BinaryOperator::NotEqual,
        BinaryOperator::Or,
        BinaryOperator::ShiftLeft,
        BinaryOperator::ShiftRight,
        BinaryOperator::Subtract,
        BinaryOperator::CaseMembership,
    ] {
        let (left, right) = (fixture.integer(1), fixture.name("LIMIT"));
        let binary = fixture.binary(left, operator, right);
        add(fixture, "binary", binary, &[2]);
    }

    // A selected algebra licenses reordering, so the operands sort and the
    // identity names the operation and the algebra beside them.
    let (left, right) = (fixture.name("LIMIT"), fixture.name("WIDTH"));
    let licensed = fixture.binary(left, BinaryOperator::Add, right);
    let selection = crate::expression::OpenIndexOperationSelection {
        expression: licensed,
        spelling: language_core::operator_spelling::OperatorSpelling::Add,
        operator: fixture.symbol("size"),
        operand_type: fixture.u64_type,
        provider: fixture.symbol("Selected"),
        application: None,
        algebra_trait: fixture.symbol("Hooks"),
        algebra_requirement: "add".to_owned(),
        algebra_alias: None,
        commutativity_licensed: true,
        associativity_licensed: true,
        distributes_over: Vec::new(),
        left_identity_literal: String::new(),
        right_identity_literal: String::new(),
    };
    fixture
        .program
        .expression_table
        .index_expression_authority_mut(licensed)
        .operation = Some(selection);
    add(
        fixture,
        "binary with a selected algebra",
        licensed,
        &[2, 3, 4],
    );

    let target = fixture.named("Count");
    let cast = {
        let value = crate::expression::Expression::Integer(IntegerLiteral::from_value(3));
        fixture
            .program
            .expression_table
            .insert_tree(&crate::expression::Expression::Cast(Box::new(
                crate::expression::CastExpression {
                    value,
                    target_type: target,
                    target_label: NamePath::unresolved(Vec::new()),
                    domain: ArithmeticDomain::Wrapping,
                    form: language_core::cast_form::CastForm::Value,
                },
            )))
    };
    add(fixture, "cast", cast, &[2]);

    let (receiver, argument) = (fixture.name("LIMIT"), fixture.integer(3));
    let plain = fixture.call(receiver, vec![argument], Vec::new());
    add(fixture, "call", plain, &[2, 3]);
    let selected = StaticMachineArgument {
        symbol: fixture.symbol("Selected"),
        application: Some(Box::new(StaticSymbolApplication {
            member: arena::Handle::invalid(),
            lifetime_arguments: Box::new([Identifier::generated("a")]),
            arguments: Box::new([StaticMachineArgument {
                const_literal: Some(IntegerLiteral::from_value(7)),
                ..StaticMachineArgument::default()
            }]),
        })),
        ..StaticMachineArgument::default()
    };
    let operand = fixture.named("Payload");
    let applied = fixture.call(
        ExpressionHandle::invalid(),
        Vec::new(),
        vec![
            selected,
            StaticMachineArgument {
                type_reference: operand,
                ..StaticMachineArgument::default()
            },
        ],
    );
    add(fixture, "call with static operands", applied, &[1, 3, 4]);

    let (collection, index) = (fixture.name("LIMIT"), fixture.integer(0));
    let indexed = fixture.expression(ExpressionNode::Indexed(TableIndexedExpression {
        collection,
        index,
    }));
    add(fixture, "indexed", indexed, &[2]);
    let receiver = fixture.name("LIMIT");
    let member = fixture.expression(ExpressionNode::Member(TableMemberExpression {
        receiver,
        member_symbol: SymbolHandle::invalid(),
        member: Identifier::generated("tag"),
        case_variant: Some(Identifier::generated("On")),
    }));
    add(fixture, "member", member, &[2]);
    for access in [
        language_core::ReferenceAccess::Shared,
        language_core::ReferenceAccess::Mutable,
        language_core::ReferenceAccess::WriteOnly,
    ] {
        let target = fixture.name("LIMIT");
        let borrow = fixture.expression(ExpressionNode::Borrow(TableBorrowExpression {
            target,
            access,
        }));
        add(fixture, "borrow", borrow, &[2]);
    }
    for end_inclusive in [true, false] {
        let (start, end) = (fixture.integer(0), fixture.name("WIDTH"));
        let range = fixture.expression(ExpressionNode::Range(TableRangeExpression {
            start,
            end,
            end_inclusive,
        }));
        add(fixture, "range", range, &[3]);
    }
    let (first, second) = (fixture.integer(1), fixture.name("LIMIT"));
    let elements = fixture
        .program
        .expression_table
        .insert_expression_handles([first, second]);
    let array = fixture.expression(ExpressionNode::ArrayLiteral(elements));
    add(fixture, "array literal", array, &[2]);
    let empty = fixture
        .program
        .expression_table
        .insert_expression_handles([]);
    let empty = fixture.expression(ExpressionNode::ArrayLiteral(empty));
    add(fixture, "empty array literal", empty, &[]);
    let value = fixture.name("WIDTH");
    let fields = fixture
        .program
        .expression_table
        .insert_struct_fields([TableStructLiteralField {
            name: Identifier::generated("width"),
            field_symbol: SymbolHandle::invalid(),
            value,
        }]);
    let literal = fixture.expression(ExpressionNode::StructLiteral(TableStructLiteral {
        application_type: Default::default(),
        type_name: Identifier::generated("Record"),
        type_symbol: fixture.symbol("Record"),
        case_name: Some(Identifier::generated("On")),
        case_symbol: None,
        fields,
    }));
    add(fixture, "struct literal", literal, &[2, 3]);
    let operand = fixture.named("Payload");
    let type_expression = fixture.expression(ExpressionNode::TypeExpression(operand));
    add(fixture, "type expression", type_expression, &[1]);
    let zero = fixture.expression(ExpressionNode::ZeroValue(operand));
    add(fixture, "zero value", zero, &[1]);
    let (value, result) = (fixture.name("LIMIT"), fixture.name("WIDTH"));
    let atomic = fixture.expression(ExpressionNode::Atomic(TableAtomicExpression {
        value,
        result,
        ordering: AtomicOrderingPlan::Load(MemoryOrdering::Receive),
        result_custody: AtomicExpressionResultCustody::Scalar,
    }));
    add(fixture, "atomic", atomic, &[2, 3]);

    // Derived declarations are named by the application that produced them.
    for (form, name) in [
        ("derived view", "Placed"),
        ("derived accessor by member identity", "Accessor"),
        ("derived accessor by field name", "Cursor"),
        ("derived operation state", "read_state"),
    ] {
        let derived = fixture.name(name);
        add(fixture, form, derived, &[1, 2, 3]);
    }
    let receiver = fixture.name("LIMIT");
    let operation = fixture.call_to("read_machine", receiver, Vec::new(), Vec::new());
    add(fixture, "derived operation machine", operation, &[1, 2, 3]);

    // A dispatch on values: a value arm and a wildcard arm.
    let (subject, wanted, chosen, otherwise) = (
        fixture.name("LIMIT"),
        fixture.integer(0),
        fixture.name("WIDTH"),
        fixture.integer(1),
    );
    let arms = fixture.program.expression_table.insert_match_arms([
        TableMatchArm {
            pattern: MatchPattern::Value(wanted),
            value: chosen,
            source_span: SourceSpan::default(),
        },
        TableMatchArm {
            pattern: MatchPattern::Wildcard,
            value: otherwise,
            source_span: SourceSpan::default(),
        },
    ]);
    let dispatch = fixture.expression(ExpressionNode::Match(TableMatchExpression {
        subject,
        arms,
        ..TableMatchExpression::default()
    }));
    add(fixture, "match on values", dispatch, &[2, 3]);

    // A dispatch that destructures its subject and reads it again in an arm.
    let subject = fixture.name("LIMIT");
    let capture = fixture
        .program
        .expression_table
        .insert_match_subject(subject);
    let head = fixture.name("Record");
    let saved = fixture.expression(ExpressionNode::SavedSubject(TableSavedSubject {
        component: Default::default(),
        capture,
        pattern_head: head,
    }));
    let wanted = fixture.name("WIDTH");
    let guard = fixture.name("Count");
    let (left, right, spare) = (
        fixture.symbol("left"),
        fixture.symbol("right"),
        fixture.symbol("spare"),
    );
    let fields = fixture.program.expression_table.insert_match_fields([
        TableMatchField {
            component: Default::default(),
            member: Identifier::generated("left"),
            field_symbol: left,
            action: MatchFieldAction::Bind(Identifier::generated("bound")),
        },
        TableMatchField {
            component: Default::default(),
            member: Identifier::generated("right"),
            field_symbol: right,
            action: MatchFieldAction::Equal(wanted),
        },
        TableMatchField {
            component: Default::default(),
            member: Identifier::generated("spare"),
            field_symbol: spare,
            action: MatchFieldAction::Waive,
        },
    ]);
    let selectors = fixture
        .program
        .expression_table
        .insert_match_proof_selectors([OutcomeProofSelector {
            output_field: Identifier::generated("left"),
            binding: Identifier::generated("bound"),
        }]);
    let unguarded = fixture.program.expression_table.insert_match_arms([
        TableMatchArm {
            pattern: MatchPattern::Destructure(TableDestructurePattern {
                application_type: Default::default(),
                head,
                fields,
                has_rest: true,
                guard,
                condition: ExpressionHandle::invalid(),
                proof_selectors: selectors,
            }),
            value: saved,
            source_span: SourceSpan::default(),
        },
        TableMatchArm {
            pattern: MatchPattern::Destructure(TableDestructurePattern {
                application_type: Default::default(),
                head,
                fields: Default::default(),
                has_rest: false,
                guard: ExpressionHandle::invalid(),
                condition: ExpressionHandle::invalid(),
                proof_selectors: Default::default(),
            }),
            value: head,
            source_span: SourceSpan::default(),
        },
    ]);
    let destructuring = fixture.expression(ExpressionNode::Match(TableMatchExpression {
        subject,
        capture,
        arms: unguarded,
    }));
    add(
        fixture,
        "match that destructures",
        destructuring,
        &[2, 3, 4],
    );
    witnesses
}
