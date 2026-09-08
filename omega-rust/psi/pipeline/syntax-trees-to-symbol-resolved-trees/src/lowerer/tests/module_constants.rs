use super::lower_syntax_trees;
use language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure;
use source::{SourceId, SourceSpan, Span};
use source_files_to_tokens::Lexer;
use syntax_trees::{
    SyntaxTrees,
    identifier::Identifier,
    types::{ConstArgumentOrigin, TypeReferenceNode},
};
use tokens_to_syntax_trees::parse_syntax_trees_into_with_id;

fn normalized(public: bool) -> SyntaxTrees {
    let mut syntax = SyntaxTrees::default();
    let module = format!(
        "module combat; pub const SIZE: u64 = 2; {}data Use {{ buffer: Buffer<SIZE>; }}",
        if public { "pub " } else { "" }
    );
    for (source_id, text) in [
        (SourceId(1), "pub data Buffer<const N: u64> { value: u64; }"),
        (SourceId(2), module.as_str()),
    ] {
        let tokens = Lexer::new(text)
            .tokenize()
            .expect("tokenize constant index custody");
        parse_syntax_trees_into_with_id(&mut syntax, source_id, &tokens)
            .expect("parse constant index custody");
    }
    crate::normalize_generic_data(syntax).expect("normalize exact module constant index")
}

fn origin(syntax: &SyntaxTrees) -> ConstArgumentOrigin {
    syntax
        .type_references
        .named_nodes_from(0)
        .into_iter()
        .find_map(|(handle, _)| {
            syntax
                .type_references
                .const_argument_origin(handle)
                .cloned()
        })
        .expect("retained constant origin")
}

#[test]
fn normalized_constant_argument_retains_actual_owner_exposure() {
    for public in [false, true] {
        let syntax = normalized(public);
        let origin = origin(&syntax);
        let program = lower_syntax_trees(&syntax).expect("resolve constant origin");
        let selected = program
            .const_declarations
            .iter()
            .next()
            .expect("constant declaration")
            .symbol;
        let occurrences = program.authored_declaration_selections().iter().filter(|selection| {
            selection.source_span() == origin.reference
                && matches!(selection.target(), symbol_resolved_trees::AuthoredDeclarationSelectionTarget::Resolved(target) if target.selected_symbol() == selected)
        }).collect::<Vec<_>>();
        assert!(!occurrences.is_empty());
        let expected = if public {
            AuthoredDeclarationSelectionExposure::PublicInterface
        } else {
            AuthoredDeclarationSelectionExposure::PrivateImplementation
        };
        assert!(
            occurrences
                .iter()
                .all(|selection| selection.exposure() == expected),
            "a public generic template cannot promote a private use's occurrence exposure"
        );
    }
}

#[test]
fn synthetic_instance_exclusion_does_not_hide_independent_same_value_field_origin() {
    use syntax_trees::item::{DataMember, Item};
    let mut syntax = SyntaxTrees::default();
    for (source_id, text) in [
        (SourceId(1), "pub data Buffer<const N: u64> { value: u64; }"),
        (
            SourceId(2),
            "module combat; pub const SIZE: u64 = 2; data Use { first: Buffer<SIZE>; independent: Buffer<SIZE>; }",
        ),
    ] {
        let tokens = Lexer::new(text)
            .tokenize()
            .expect("tokenize independent constant occurrences");
        parse_syntax_trees_into_with_id(&mut syntax, source_id, &tokens)
            .expect("parse independent constant occurrences");
    }
    let mut syntax =
        crate::normalize_generic_data(syntax).expect("normalize independent occurrences");
    let root = syntax.root_item_handles().iter().copied().find(|handle| matches!(syntax.root_item(*handle), Item::Data(data) if data.name.as_str() == "Use")).expect("source owner");
    let Item::Data(mut owner) = syntax.root_item(root).clone() else {
        panic!("data owner");
    };
    let fields = syntax.items.data_members(owner.members);
    let [DataMember::Field(first), DataMember::Field(independent)] = fields else {
        panic!("two independent fields");
    };
    let first_application = syntax
        .type_references
        .generic_application_origin(first.type_reference);
    let independent_application = syntax
        .type_references
        .generic_application_origin(independent.type_reference);
    let argument_origin = |application| {
        let TypeReferenceNode::Generic { arguments, .. } =
            syntax.type_references.type_reference(application)
        else {
            panic!("retained generic application");
        };
        let argument = syntax.type_references.type_reference_handles(*arguments)[0];
        syntax
            .type_references
            .const_argument_origin(argument)
            .expect("constant argument custody")
            .clone()
    };
    let derived = argument_origin(first_application);
    let independent = argument_origin(independent_application);
    assert_ne!(derived.reference, independent.reference);
    assert_eq!(derived.declaration, independent.declaration);
    assert_eq!(derived.initializer, independent.initializer);
    assert_eq!(
        derived.canonical_value_encoding,
        independent.canonical_value_encoding
    );

    // The first occurrence now describes this synthetic data's derivation;
    // the second remains a separate authored field occurrence in its API.
    owner.generic_instance = Some(first_application);
    owner.is_public = true;
    syntax.items.replace_item(root, Item::Data(owner));
    let program =
        lower_syntax_trees(&syntax).expect("resolve synthetic owner with independent field origin");
    let selected = program
        .const_declarations
        .iter()
        .next()
        .expect("selected constant")
        .symbol;
    let occurrences = program.authored_declaration_selections().iter().filter(|selection| {
        selection.source_span() == independent.reference
            && matches!(selection.target(), symbol_resolved_trees::AuthoredDeclarationSelectionTarget::Resolved(target) if target.selected_symbol() == selected)
    }).collect::<Vec<_>>();
    assert!(
        !occurrences.is_empty(),
        "same declaration and value do not identify an occurrence"
    );
    assert!(
        occurrences.iter().all(|selection| selection.exposure()
            == AuthoredDeclarationSelectionExposure::PublicInterface)
    );
}

#[test]
fn normalized_constant_argument_rejects_missing_and_drifted_declaration_custody() {
    let syntax = normalized(false);
    let original = origin(&syntax);
    for corruption in ["missing", "initializer", "encoding", "duplicate"] {
        let mut program = lower_syntax_trees(&syntax).expect("resolve original custody");
        let mut origin = original.clone();
        match corruption {
            "missing" => origin.declaration = SourceSpan::new(SourceId(9), Span::new(10, 14)),
            "initializer" => origin.initializer.span.start += 1,
            "encoding" => origin.canonical_value_encoding.push('x'),
            "duplicate" => {
                let declaration = program
                    .const_declarations
                    .iter()
                    .next()
                    .expect("retained declaration")
                    .clone();
                program.roots.const_declarations.push(declaration);
            }
            _ => unreachable!(),
        }
        let selection = crate::lowerer::PendingConstArgumentSelection {
            origin,
            exposure: AuthoredDeclarationSelectionExposure::PrivateImplementation,
        };
        assert!(
            crate::constant::finalize_const_argument_selections(&mut program, &[selection])
                .is_err(),
            "{corruption} custody must reject"
        );
    }
}

#[test]
fn rewritten_constant_payload_cannot_drift_from_its_origin() {
    let mut syntax = normalized(false);
    let arguments = syntax
        .type_references
        .named_nodes_from(0)
        .into_iter()
        .filter_map(|(handle, _)| {
            syntax
                .type_references
                .const_argument_origin(handle)
                .is_some()
                .then_some(handle)
        })
        .collect::<Vec<_>>();
    assert!(!arguments.is_empty());
    for argument in arguments {
        syntax.type_references.replace_type_reference(
            argument,
            TypeReferenceNode::Named(Identifier::generated("3")),
        );
    }
    assert!(
        lower_syntax_trees(&syntax).is_err(),
        "rewritten 3 cannot retain the declaration's exact 2 encoding"
    );
}

#[test]
fn dead_constant_argument_origins_do_not_become_authored_selections() {
    let mut syntax = normalized(false);
    let original = lower_syntax_trees(&syntax).expect("resolve live constant index");
    let dead = syntax
        .type_references
        .insert_named(Identifier::generated("3"));
    syntax
        .type_references
        .retain_const_argument_origin(dead, ConstArgumentOrigin::default());
    let program =
        lower_syntax_trees(&syntax).expect("unreachable type arena nodes are not lowered");
    assert_eq!(
        program.authored_declaration_selections(),
        original.authored_declaration_selections()
    );
}
