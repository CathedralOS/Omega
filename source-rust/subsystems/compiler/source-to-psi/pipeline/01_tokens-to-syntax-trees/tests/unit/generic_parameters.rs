use crate::ParseError;
use crate::frontend_fixture::Lexer;
use crate::parameters::generics::{
    GenericParameterSyntax, ParsedGenericParameters, parse_generic_parameters,
};
use crate::token_input::cursor::Input;
use source_text::SourceId;
use syntax_trees::SyntaxTrees;
use syntax_trees::item::{Item, MachineParameterContract, TypeParameterKind};

const SYNTAX_CHOICES: [GenericParameterSyntax; 6] = [
    GenericParameterSyntax::TypeAndConst,
    GenericParameterSyntax::StaticBinders,
    GenericParameterSyntax::DataDeclaration,
    GenericParameterSyntax::TraitRequirements,
    GenericParameterSyntax::RequirementSignature,
    GenericParameterSyntax::MachineDeclaration,
];

fn parse_parameters(
    source: &str,
    syntax: GenericParameterSyntax,
) -> Result<(SyntaxTrees, ParsedGenericParameters), ParseError> {
    let tokens = Lexer::new(source).tokenize().unwrap();
    let source_id = SourceId(7);
    let mut trees = SyntaxTrees::new(source_id);

    let input = Input::new(source_id, &tokens, &mut trees);
    let (parameters, rest) = parse_generic_parameters(&mut trees, input, syntax)?;
    assert!(rest.tokens.is_empty());
    Ok((trees, parameters))
}

#[test]
fn inline_callable_bounds_are_rejected_and_inferred_binders_remain_admitted() {
    for syntax in SYNTAX_CHOICES {
        assert!(parse_parameters("<const Callback: machine()>", syntax).is_err());
    }
    assert!(
        parse_parameters(
            "<const Callback: machine()>",
            GenericParameterSyntax::MathematicalDefinition,
        )
        .is_err()
    );
    assert!(
        parse_parameters(
            "<const Callback: machine>",
            GenericParameterSyntax::MachineDeclaration,
        )
        .is_ok()
    );
}

#[test]
fn relation_family_binders_remain_trait_only() {
    for syntax in SYNTAX_CHOICES {
        let result = parse_parameters("<C, R: C -> C -> |proof|::Type>", syntax);
        let family = result.as_ref().is_ok_and(|(trees, parameters)| {
            matches!(
                trees.items.type_parameters(parameters.type_parameters)[1].kind,
                TypeParameterKind::Proposition { .. }
            )
        });
        assert_eq!(
            family,
            matches!(syntax, GenericParameterSyntax::TraitRequirements)
        );
        assert!(parse_parameters("<proposition Relation>", syntax).is_err());
    }
}

#[test]
fn conformance_and_value_binders_keep_distinct_admission() {
    for syntax in SYNTAX_CHOICES {
        let conformance = parse_parameters("<Evidence satisfies Trait<Value>>", syntax);
        assert_eq!(
            conformance.is_ok(),
            matches!(
                syntax,
                GenericParameterSyntax::TraitRequirements
                    | GenericParameterSyntax::RequirementSignature
                    | GenericParameterSyntax::MachineDeclaration
                    | GenericParameterSyntax::DataDeclaration
            )
        );
        if let Ok((trees, parameters)) = conformance {
            assert_eq!(parameters.conformance_bounds.len(), 1);
            if matches!(
                syntax,
                GenericParameterSyntax::TraitRequirements
                    | GenericParameterSyntax::RequirementSignature
                    | GenericParameterSyntax::DataDeclaration
            ) {
                assert!(matches!(
                    trees.items.type_parameters(parameters.type_parameters)[0].kind,
                    TypeParameterKind::Implementation
                ));
            } else {
                assert!(parameters.type_parameters.is_empty());
            }
        }
        assert!(parse_parameters("<Evidence: Value satisfies Trait>", syntax).is_err());
        let value = parse_parameters("<Count: usize>", syntax);
        assert_eq!(
            value.is_ok(),
            matches!(
                syntax,
                GenericParameterSyntax::DataDeclaration
                    | GenericParameterSyntax::RequirementSignature
                    | GenericParameterSyntax::MachineDeclaration
            )
        );
        if let Ok((trees, parameters)) = value {
            assert!(matches!(
                trees.items.type_parameters(parameters.type_parameters)[0].kind,
                TypeParameterKind::Value { .. }
            ));
        }
    }
}

#[test]
fn callable_binders_distinguish_identity_defaults_from_authored_contracts() {
    for syntax in [
        GenericParameterSyntax::DataDeclaration,
        GenericParameterSyntax::TraitRequirements,
    ] {
        let (trees, parameters) = parse_parameters("<machine Selected>", syntax).unwrap();
        let TypeParameterKind::Machine { contract } =
            &trees.items.type_parameters(parameters.type_parameters)[0].kind
        else {
            panic!("machine binder");
        };
        assert_eq!(
            matches!(
                contract,
                Some(MachineParameterContract::RequirementIdentity)
            ),
            matches!(syntax, GenericParameterSyntax::TraitRequirements)
        );
        if matches!(syntax, GenericParameterSyntax::DataDeclaration) {
            assert!(contract.is_none());
        }
    }
    for (declaration, ending) in [
        ("data", "{ value: u64; }"),
        ("data", "= u64;"),
        ("trait", "{}"),
    ] {
        let source = format!(
            "{declaration} Carrier<machine Selected> requires machine Selected(value: u64) -> u64; {ending}"
        );
        let trees = crate::tests::parse_source(&source);
        let parameters = match trees.root_items().next().unwrap() {
            Item::Data(data) => data.type_parameters,
            Item::TypeAlias(alias) => alias.type_parameters,
            Item::Trait(trait_definition) => trait_definition.type_parameters,
            _ => panic!("data, transparent alias or trait"),
        };
        assert!(matches!(
            trees.items.type_parameters(parameters)[0].kind,
            TypeParameterKind::Machine {
                contract: Some(MachineParameterContract::Structural(_))
            }
        ));
        for clauses in [
            "",
            "requires machine Selected(value: u64) -> u64; requires machine Selected(value: u64) -> u64;",
        ] {
            if declaration == "trait" && clauses.is_empty() {
                continue;
            }
            let source = format!("{declaration} Carrier<machine Selected> {clauses} {ending}");
            let tokens = Lexer::new(&source).tokenize().unwrap();
            assert!(
                crate::parse_syntax_trees(&tokens).is_err(),
                "missing or duplicated contracts must refuse"
            );
        }
    }
}

#[test]
fn evidence_bounds_retain_explicit_trait_lifetimes_separately_from_static_slots() {
    for syntax in [
        GenericParameterSyntax::MachineDeclaration,
        GenericParameterSyntax::DataDeclaration,
        GenericParameterSyntax::TraitRequirements,
    ] {
        let (trees, parameters) = parse_parameters(
            "<'held, Value, Selected satisfies Buffered<'held, Value, 3u64>>",
            syntax,
        )
        .unwrap();
        assert_eq!(parameters.lifetime_parameters.len(), 1);
        let [bound] = parameters.conformance_bounds.as_slice() else {
            panic!("one bound");
        };
        assert_eq!(bound.trait_name.as_str(&trees), "Buffered");
        assert_eq!(bound.lifetime_arguments.len(), 1);
        assert_eq!(bound.lifetime_arguments[0].as_str(&trees), "held");
        assert_eq!(
            trees
                .type_references
                .type_reference_handles(bound.arguments)
                .len(),
            2
        );
        assert!(
            parse_parameters("<'held, Selected satisfies Buffered<Value, 'held>>", syntax,)
                .is_err()
        );
    }
}
