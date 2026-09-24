//! A declared domain is a constructor in the shared matcher: both authored
//! spellings (`u64::AtMost<N>` and `u64 in AtMost<N>`) reach the same
//! declaration, index binders follow the family's declared telescope, and a
//! carrier that disagrees with the declaration declines rather than matches.

use crate::preparation::type_equations::{
    EquationTemplate, classify_type_equations, complete_equation_arguments,
};
use diagnostics::Diagnostic;
use source::SourceId;
use source_files_to_tokens::Lexer;
use std::collections::HashMap;
use syntax_trees::SyntaxTrees;
use syntax_trees::identifier::Identifier;
use syntax_trees::item::{DataMember, Item, TypeParameterKind};
use syntax_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};
use tokens_to_syntax_trees::parse_syntax_trees_into_with_id;

fn parse(source: &str) -> SyntaxTrees {
    let mut syntax = SyntaxTrees::new(SourceId::default());
    let tokens = Lexer::new(source).tokenize().expect("tokenize fixture");
    parse_syntax_trees_into_with_id(&mut syntax, SourceId::default(), &tokens)
        .expect("parse fixture");
    syntax
}

fn data_definition<'syntax>(
    syntax: &'syntax SyntaxTrees,
    name: &str,
) -> &'syntax syntax_trees::item::DataDefinition {
    syntax
        .root_item_handles()
        .iter()
        .find_map(|handle| {
            let Item::Data(definition) = syntax.root_item(*handle) else {
                return None;
            };
            (definition.name.as_str() == name).then_some(definition)
        })
        .unwrap_or_else(|| panic!("fixture declares data `{name}`"))
}

/// `owner`'s first field as a generic application: its head name and the
/// supplied argument handles.
fn field_application(syntax: &SyntaxTrees, owner: &str) -> (Identifier, Vec<TypeReferenceHandle>) {
    let definition = data_definition(syntax, owner);
    let [DataMember::Field(field)] = syntax.items.data_members(definition.members) else {
        panic!("fixture `{owner}` holds one field")
    };
    let TypeReferenceNode::Generic {
        base_name,
        arguments,
        ..
    } = syntax.type_references.type_reference(field.type_reference)
    else {
        panic!("fixture field is a generic application")
    };
    (
        base_name.clone(),
        syntax
            .type_references
            .type_reference_handles(*arguments)
            .to_vec(),
    )
}

/// Run `declaration`'s where equations against the arguments `holder`'s field
/// supplies; the returned syntax owns the completed tuple's handles.
fn complete(
    source: &str,
    declaration: &str,
    holder: &str,
) -> Result<(SyntaxTrees, Vec<TypeReferenceHandle>), Diagnostic> {
    let mut syntax = parse(source);
    let (base_name, supplied) = field_application(&syntax, holder);
    let (template_parameters, parameter_names, const_parameter_types, type_equations, name) = {
        let definition = data_definition(&syntax, declaration);
        let parameters = syntax
            .items
            .type_parameters(definition.type_parameters)
            .to_vec();
        let parameter_names = parameters
            .iter()
            .map(|parameter| parameter.name.as_str().to_owned())
            .collect::<Vec<_>>();
        let const_parameter_types = parameters
            .iter()
            .map(|parameter| match parameter.kind {
                TypeParameterKind::Const { type_reference } => Some(type_reference),
                _ => None,
            })
            .collect::<Vec<_>>();
        let type_equations = classify_type_equations(
            &syntax,
            &parameters,
            syntax.items.proof_facts(definition.where_facts),
        );
        (
            definition.type_parameters,
            parameter_names,
            const_parameter_types,
            type_equations,
            definition.name.as_str().to_owned(),
        )
    };
    let mut warnings = Vec::new();
    let tuple = complete_equation_arguments(
        &mut syntax,
        EquationTemplate {
            kind: "data",
            name: &name,
            parameters: template_parameters,
            parameter_names: &parameter_names,
            const_parameter_types: &const_parameter_types,
            type_equations: &type_equations,
        },
        &base_name,
        &supplied,
        &HashMap::new(),
        None,
        &mut warnings,
    )?;
    Ok((syntax, tuple))
}

fn named(syntax: &SyntaxTrees, reference: TypeReferenceHandle) -> Option<String> {
    match syntax.type_references.type_reference(reference) {
        TypeReferenceNode::Named(name) => Some(name.as_str().to_owned()),
        _ => None,
    }
}

const CAPACITY_TEMPLATE: &str = r#"
    domain<const Capacity: u64> u64::AtMost<Capacity>;

    data TinyBytes<Length, const Capacity: u64>
    where
        Length == u64::AtMost<Capacity>
    {
        storage: [u8; Capacity];
        length: Length;
    }
"#;

#[test]
fn carrier_qualified_domain_head_binds_the_declared_index() {
    let source = format!(
        "{CAPACITY_TEMPLATE}
        data Holder {{ inferred: TinyBytes<u64::AtMost<256> >; }}"
    );
    let (syntax, tuple) = complete(&source, "TinyBytes", "Holder").expect("binds Capacity");
    assert_eq!(tuple.len(), 2);
    assert_eq!(named(&syntax, tuple[1]).as_deref(), Some("256"));
}

#[test]
fn constrained_spelling_binds_the_same_index() {
    let source = format!(
        "{CAPACITY_TEMPLATE}
        data Holder {{ inferred: TinyBytes<u64 in AtMost<256> >; }}"
    );
    let (syntax, tuple) = complete(&source, "TinyBytes", "Holder").expect("binds Capacity");
    assert_eq!(named(&syntax, tuple[1]).as_deref(), Some("256"));
}

#[test]
fn equivalent_explicit_index_selects_the_same_application() {
    let source = format!(
        "{CAPACITY_TEMPLATE}
        data Holder {{ inferred: TinyBytes<u64::AtMost<256>, 256>; }}"
    );
    let (syntax, tuple) = complete(&source, "TinyBytes", "Holder").expect("equation verifies");
    assert_eq!(tuple.len(), 2);
    assert_eq!(named(&syntax, tuple[1]).as_deref(), Some("256"));
}

#[test]
fn a_conflicting_explicit_index_reports_the_equation_binding() {
    let source = format!(
        "{CAPACITY_TEMPLATE}
        data Holder {{ inferred: TinyBytes<u64::AtMost<256>, 257>; }}"
    );
    let error = complete(&source, "TinyBytes", "Holder").unwrap_err();
    assert!(
        error.message.contains("explicit argument is 257"),
        "unexpected diagnostic: {}",
        error.message
    );
}

#[test]
fn a_domain_on_a_different_carrier_does_not_match() {
    let source = r#"
        domain<const Capacity: u64> i64::AtMost<Capacity>;

        data TinyBytes<Length, const Capacity: u64>
        where
            Length == i64::AtMost<Capacity>
        {
            storage: [u8; Capacity];
            length: Length;
        }

        data Holder { inferred: TinyBytes<u64 in AtMost<256> >; }
    "#;
    let error = complete(source, "TinyBytes", "Holder").unwrap_err();
    assert!(
        error
            .message
            .contains("requires a selected domain application declaration"),
        "unexpected diagnostic: {}",
        error.message
    );
}

#[test]
fn the_two_spellings_compare_equal_inside_a_bound_application() {
    let source = r#"
        domain<const Capacity: u64> u64::AtMost<Capacity>;

        data Wrap<Inside> { inside: Inside; }

        data Duo<Carrier, Wrapped>
        where
            Wrapped == Wrap<Carrier>
        {
            wrapped: Wrapped;
        }

        data Holder {
            inferred: Duo<u64 in AtMost<256>, Wrap<u64::AtMost<256> > >;
        }
    "#;
    let (_syntax, tuple) = complete(source, "Duo", "Holder")
        .expect("a carrier-qualified head equals the constrained spelling");
    assert_eq!(tuple.len(), 2);
}

#[test]
fn an_unbound_binder_recovers_the_constrained_spelling() {
    let source = r#"
        domain<const Capacity: u64> u64::AtMost<Capacity>;

        data Pair<Primary, Secondary, const Capacity: u64>
        where
            Primary == u64::AtMost<Capacity>,
            Secondary == u64::AtMost<Capacity>
        {
            first: Primary;
            second: Secondary;
        }

        data Holder { inferred: Pair<u64::AtMost<256> >; }
    "#;
    let (syntax, tuple) = complete(source, "Pair", "Holder").expect("recovers Secondary");
    assert_eq!(tuple.len(), 3);
    let TypeReferenceNode::Constrained {
        base_type,
        constraints,
    } = syntax.type_references.type_reference(tuple[1])
    else {
        panic!("the recovered binder constructs a constrained carrier")
    };
    assert_eq!(
        named(&syntax, *base_type).as_deref(),
        Some("u64"),
        "the recovered carrier is the declared target"
    );
    let [TypeConstraintNode::Domain(constraint)] = syntax.type_references.constraints(*constraints)
    else {
        panic!("the recovered binder carries the domain constraint")
    };
    assert_eq!(constraint.name.as_str(), "AtMost");
    let [argument] = *syntax
        .type_references
        .type_reference_handles(constraint.arguments)
    else {
        panic!("the recovered domain carries its index")
    };
    assert_eq!(named(&syntax, argument).as_deref(), Some("256"));
    assert_eq!(named(&syntax, tuple[2]).as_deref(), Some("256"));
}
