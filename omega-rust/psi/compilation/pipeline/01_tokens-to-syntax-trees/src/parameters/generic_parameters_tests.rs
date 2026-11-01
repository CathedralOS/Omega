use super::parse_generic_parameters::{
    GenericParameterSyntax, ParsedGenericParameters, parse_generic_parameters,
};
use crate::ParseError;
use crate::input::token_cursor::Input;
use source::SourceId;
use source_files_to_tokens::Lexer;
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
    let retained_source = Input::retain_source(&tokens);
    let input = Input::new(source_id, &tokens, &retained_source, &mut trees);
    let (parameters, rest) = parse_generic_parameters(&mut trees, input, syntax)?;
    assert!(rest.tokens.is_empty());
    Ok((trees, parameters))
}

#[test]
fn proposition_binders_remain_trait_only() {
    for syntax in SYNTAX_CHOICES {
        let result = parse_parameters("<proposition Relation>", syntax);
        assert_eq!(
            result.is_ok(),
            matches!(syntax, GenericParameterSyntax::TraitRequirements)
        );
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
                    | GenericParameterSyntax::MachineDeclaration
                    | GenericParameterSyntax::DataDeclaration
            )
        );
        if let Ok((trees, parameters)) = conformance {
            assert_eq!(parameters.conformance_bounds.len(), 1);
            if matches!(
                syntax,
                GenericParameterSyntax::TraitRequirements | GenericParameterSyntax::DataDeclaration
            ) {
                assert!(matches!(
                    trees.items.type_parameters(parameters.type_parameters)[0].kind,
                    TypeParameterKind::Implementation
                ));
            } else {
                assert!(parameters.type_parameters.is_empty());
            }
        }
        // The retired subject spelling rejects everywhere: a conformance names
        // code, so no binder carries a data subject.
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
fn callable_data_and_alias_binders_do_not_start_as_trait_identities() {
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
    for ending in ["{ value: u64; }", "= u64;"] {
        for (clause, nominal) in [
            ("requires machine Selected(value: u64) -> u64;", false),
            ("requires machine Selected satisfies Hook::step;", true),
        ] {
            let source = format!("data Carrier<machine Selected> {clause} {ending}");
            let tokens = Lexer::new(&source).tokenize().unwrap();
            let trees = crate::parse_syntax_trees(&tokens).expect("authored callable contract");
            let parameters = match trees.root_items().next().unwrap() {
                Item::Data(data) => data.type_parameters,
                Item::TypeAlias(alias) => alias.type_parameters,
                _ => panic!("data or transparent alias"),
            };
            let TypeParameterKind::Machine {
                contract: Some(contract),
            } = &trees.items.type_parameters(parameters)[0].kind
            else {
                panic!("retained contract");
            };
            assert_eq!(
                matches!(contract, MachineParameterContract::Nominal { .. }),
                nominal
            );
            assert_eq!(
                matches!(contract, MachineParameterContract::Structural(_)),
                !nominal
            );
        }
        for clauses in [
            "",
            "requires machine Selected(value: u64) -> u64; requires machine Selected(value: u64) -> u64;",
        ] {
            let source = format!("data Carrier<machine Selected> {clauses} {ending}");
            let tokens = Lexer::new(&source).tokenize().unwrap();
            assert!(
                crate::parse_syntax_trees(&tokens).is_err(),
                "missing or duplicated contracts must refuse"
            );
        }
    }
}
