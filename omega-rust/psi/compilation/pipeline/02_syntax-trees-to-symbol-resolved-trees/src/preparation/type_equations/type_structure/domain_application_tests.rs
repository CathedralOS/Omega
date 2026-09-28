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
use syntax_trees::types::{TypeReferenceHandle, TypeReferenceNode};

fn parse(source: &str) -> SyntaxTrees {
    let mut syntax = SyntaxTrees::new(SourceId::default());
    let tokens = Lexer::new(source).tokenize().expect("tokenize fixture");
    tokens_to_syntax_trees::parse(&mut syntax, SourceId::default(), &tokens)
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
