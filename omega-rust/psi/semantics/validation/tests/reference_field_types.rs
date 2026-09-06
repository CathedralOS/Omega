//! Declared reference correspondence only: these queries do not establish a
//! loan, permit a reference move, or prove a field's current storage provenance.

use source::SourceMap;
use source_files_to_tokens::Lexer;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::DataMember;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::name::Identifier;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};
use validation::checked_argument_matches_type_reference;

fn typed_source(source: &str) -> TypedTrees {
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize reference fields");
    let mut sources = SourceMap::default();
    let source_id = sources
        .add("reference_fields.omg".into(), source.to_owned())
        .source_id;
    let syntax = tokens_to_syntax_trees::parse_syntax_trees_with_id(source_id, &tokens)
        .expect("parse reference fields");
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees_with_sources(
        &syntax,
        std::sync::Arc::new(sources),
    )
    .expect("resolve reference fields");
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type reference fields")
}

fn fixture(actual: &str, required: &str, projection: &str) -> TypedTrees {
    typed_source(&format!(
        "data Context {{ value: u64; }}
         data Other {{ value: u64; }}
         data Carrier {{ context: {actual}; spare: {actual}; }}
         data Foreign {{ context: {actual}; }}
         data Outer {{ inner: Carrier; }}
         machine inspect(carrier: &mut Carrier, outer: &mut Outer) {{
             let selected: {actual} = {projection};
         }}
         machine require(value: {required}) {{}}"
    ))
}

fn member(program: &TypedTrees, spelling: &str) -> ExpressionHandle {
    program
        .expression_table
        .iter_expressions()
        .find_map(|(handle, expression)| {
            (matches!(expression, ExpressionNode::Member(_))
                && program.expression_table.display_name(handle) == spelling)
                .then_some(handle)
        })
        .expect("fixture member expression")
}

fn required_type(program: &TypedTrees) -> TypeReferenceHandle {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "require")
        .expect("required type declaration");
    program.state_parameters(&program.machine_states(machine)[0])[0].type_reference
}

fn field(program: &TypedTrees, owner: &str, name: &str) -> (SymbolHandle, TypeReferenceHandle) {
    let data = program
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == owner)
        .expect("field owner");
    program
        .data_members(data)
        .iter()
        .find_map(|member| match member {
            DataMember::Field(field) if field.name.as_str() == name => {
                Some((field.symbol, field.type_reference))
            }
            _ => None,
        })
        .expect("declared field")
}

fn matches(program: &TypedTrees, spelling: &str) -> bool {
    checked_argument_matches_type_reference(
        program,
        member(program, spelling),
        required_type(program),
    )
}

#[test]
fn direct_and_nested_reference_fields_obey_the_forwarding_access_matrix() {
    for projection in ["carrier.context", "outer.inner.context"] {
        for (actual, required, accepted) in [
            ("&Context", "&Context", true),
            ("&Context", "&mut Context", false),
            ("&Context", "&write Context", false),
            ("&mut Context", "&Context", true),
            ("&mut Context", "&mut Context", true),
            // Mutable-to-write-only attenuation requires explicit syntax.
            ("&mut Context", "&write Context", false),
            ("&write Context", "&Context", false),
            ("&write Context", "&mut Context", false),
            ("&write Context", "&write Context", true),
        ] {
            let program = fixture(actual, required, projection);
            assert_eq!(
                matches(&program, projection),
                accepted,
                "{projection}: {actual} -> {required}"
            );
        }
    }
}

#[test]
fn reference_fields_cannot_match_unrelated_referees_through_member_syntax() {
    for projection in ["carrier.context", "outer.inner.context"] {
        for actual in ["&Context", "&mut Context", "&write Context"] {
            for required in ["&Other", "&mut Other", "&write Other", "&u64", "&[u8]"] {
                let program = fixture(actual, required, projection);
                assert!(
                    !matches(&program, projection),
                    "{projection}: {actual} -> {required}"
                );
            }
        }
    }
}

#[test]
fn implicit_shared_borrow_of_an_owned_field_keeps_its_exact_referee() {
    for (required, accepted) in [
        ("&Context", true),
        ("&Other", false),
        ("&u64", false),
        ("&mut Context", false),
        ("&write Context", false),
    ] {
        let program = fixture("Context", required, "carrier.context");
        assert_eq!(matches(&program, "carrier.context"), accepted, "{required}");
    }
}

#[test]
fn missing_ordinary_member_symbols_still_resolve_within_the_nominal_declaration() {
    let mut program = fixture("&mut Context", "&mut Context", "outer.inner.context");
    for spelling in ["outer.inner", "outer.inner.context"] {
        let expression = member(&program, spelling);
        let ExpressionNode::Member(member) = program.expression_table.expression_mut(expression)
        else {
            unreachable!()
        };
        member.member_symbol = SymbolHandle::invalid();
    }
    assert!(matches(&program, "outer.inner.context"));
}

#[test]
fn conflicting_retained_field_symbols_cannot_use_the_old_field_spelling() {
    for owner in ["Carrier", "Foreign"] {
        let mut program = fixture("&Context", "&Context", "carrier.context");
        assert!(matches(&program, "carrier.context"));
        let name = if owner == "Carrier" {
            "spare"
        } else {
            "context"
        };
        let conflicting = field(&program, owner, name).0;
        let expression = member(&program, "carrier.context");
        let ExpressionNode::Member(member) = program.expression_table.expression_mut(expression)
        else {
            unreachable!()
        };
        member.member_symbol = conflicting;
        assert!(
            !matches(&program, "carrier.context"),
            "conflicting {owner}::{name}"
        );
    }
}

#[test]
fn conflicting_intermediate_selector_invalidates_the_complete_member_chain() {
    let mut program = fixture("&Context", "&Context", "outer.inner.context");
    assert!(matches(&program, "outer.inner.context"));
    let conflicting = field(&program, "Foreign", "context").0;
    let expression = member(&program, "outer.inner");
    let ExpressionNode::Member(member) = program.expression_table.expression_mut(expression) else {
        unreachable!()
    };
    member.member_symbol = conflicting;
    assert!(!matches(&program, "outer.inner.context"));
}

#[test]
fn absent_field_declarations_cannot_use_the_shared_member_fallback() {
    let mut program = fixture("&Context", "&Context", "carrier.context");
    let expression = member(&program, "carrier.context");
    let ExpressionNode::Member(member) = program.expression_table.expression_mut(expression) else {
        unreachable!()
    };
    member.member_symbol = SymbolHandle::invalid();
    member.member = Identifier::generated("missing");
    assert!(!checked_argument_matches_type_reference(
        &program,
        expression,
        required_type(&program)
    ));
}

#[test]
fn missing_conflicting_and_non_value_roots_cannot_recover_from_spelling() {
    for corruption in [
        "missing",
        "missing head",
        "missing symbol",
        "conflicting",
        "non-value",
    ] {
        let mut program = fixture("&Context", "&Context", "carrier.context");
        assert!(matches(&program, "carrier.context"));
        let foreign = program.machines()[1].symbol;
        let expression = member(&program, "carrier.context");
        let ExpressionNode::Member(member) = program.expression_table.expression(expression) else {
            unreachable!()
        };
        let receiver = member.receiver;
        let ExpressionNode::Name(path) = program.expression_table.expression_mut(receiver) else {
            unreachable!()
        };
        match corruption {
            "missing" => {
                path.symbol = SymbolHandle::invalid();
                path.head_symbol = SymbolHandle::invalid();
            }
            "missing head" => path.head_symbol = SymbolHandle::invalid(),
            "missing symbol" => path.symbol = SymbolHandle::invalid(),
            "conflicting" => path.head_symbol = foreign,
            "non-value" => {
                path.symbol = foreign;
                path.head_symbol = foreign;
            }
            _ => unreachable!(),
        }
        assert!(!matches(&program, "carrier.context"), "{corruption}");
    }
}

#[test]
fn missing_nominal_owner_identity_cannot_resolve_fields_by_type_name() {
    let mut program = fixture("&Context", "&Context", "outer.inner.context");
    assert!(matches(&program, "outer.inner.context"));
    let (_, inner_type) = field(&program, "Outer", "inner");
    program.type_reference_table.substitute_node(
        inner_type,
        TypeReferenceNode::Named {
            symbol: SymbolHandle::invalid(),
            name: Identifier::generated("Carrier"),
        },
    );
    assert!(!matches(&program, "outer.inner.context"));
}

#[test]
fn missing_receiver_and_field_types_reject_reference_correspondence() {
    for field_name in ["inner", "context"] {
        let mut program = fixture("&Context", "&Context", "outer.inner.context");
        assert!(matches(&program, "outer.inner.context"));
        let owner = if field_name == "inner" {
            "Outer"
        } else {
            "Carrier"
        };
        let (_, type_reference) = field(&program, owner, field_name);
        program
            .type_reference_table
            .substitute_node(type_reference, TypeReferenceNode::Unit);
        assert!(
            !matches(&program, "outer.inner.context"),
            "missing {owner}::{field_name} type"
        );
    }
}

#[test]
fn missing_root_declared_type_cannot_resolve_a_same_spelled_member() {
    let mut program = fixture("&Context", "&Context", "carrier.context");
    assert!(matches(&program, "carrier.context"));
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let root_type = program.state_parameters(state)[0].type_reference;
    program
        .type_reference_table
        .substitute_node(root_type, TypeReferenceNode::Unit);
    assert!(!matches(&program, "carrier.context"));
}

#[test]
fn attached_self_reference_fields_use_the_exact_inherited_field_slot() {
    for access in ["", "mut ", "write "] {
        let mut program = typed_source(&format!(
            "data Context {{ value: u64; }}
             data Carrier {{ context: &{access}Context; spare: &{access}Context; }}
             machine Carrier::inspect(&mut self) {{ let selected: &{access}Context = self.context; }}
             machine require(value: &{access}Context) {{}}"
        ));
        assert!(matches(&program, "self.context"), "{access}");
        let expression = member(&program, "self.context");
        let conflicting = program
            .symbols
            .child_handles(program.machines()[0].symbol)
            .expect("attached machine children")
            .filter(|symbol| program.symbols.get(*symbol).kind == symbols::SymbolKind::Field)
            .nth(1)
            .expect("inherited spare field");
        let ExpressionNode::Member(member) = program.expression_table.expression_mut(expression)
        else {
            unreachable!()
        };
        member.member_symbol = conflicting;
        assert!(
            !matches(&program, "self.context"),
            "conflicting inherited {access}field"
        );
    }
}

#[test]
fn lifetime_spelling_does_not_change_reference_type_correspondence() {
    for (actual, required, accepted) in [
        ("&'stored Context", "&'argument Context", true),
        ("&'stored mut Context", "&'argument Context", true),
        ("&'stored mut Context", "&'argument mut Context", true),
        ("&'stored write Context", "&'argument write Context", true),
        ("&'stored mut Context", "&'argument write Context", false),
        ("&'stored write Context", "&'argument Context", false),
    ] {
        let program = typed_source(&format!(
            "data Context {{ value: u64; }}
             data Carrier<'stored> {{ context: {actual}; }}
             machine inspect<'stored>(carrier: &mut Carrier<'stored>) {{
                 let selected: {actual} = carrier.context;
             }}
             machine require<'argument>(value: {required}) {{}}"
        ));
        assert_eq!(
            matches(&program, "carrier.context"),
            accepted,
            "{actual} -> {required}"
        );
    }
}

#[test]
fn constraint_shells_preserve_reference_access_and_referee_checks() {
    for (actual, required, accepted) in [
        ("&u64 in Wrapping", "&u64 in Wrapping", true),
        ("&mut u64 in Wrapping", "&u64 in Wrapping", true),
        ("&write u64 in Wrapping", "&write u64 in Wrapping", true),
        ("&mut u64 in Wrapping", "&write u64 in Wrapping", false),
        ("&write u64 in Wrapping", "&u64 in Wrapping", false),
        ("&u64 in Wrapping", "&Other", false),
    ] {
        let program = fixture(actual, required, "carrier.context");
        assert_eq!(
            matches(&program, "carrier.context"),
            accepted,
            "{actual} -> {required}"
        );
    }
}
