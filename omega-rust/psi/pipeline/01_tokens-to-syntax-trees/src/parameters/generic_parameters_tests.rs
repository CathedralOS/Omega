use super::parse_generic_parameters::{
    GenericParameterSyntax, ParsedGenericParameters, parse_generic_parameters,
};
use crate::ParseError;
use crate::input::token_cursor::Input;
use source::SourceId;
use source_files_to_tokens::Lexer;
use syntax_trees::SyntaxTrees;
use syntax_trees::item::{MachineParameterContract, TypeParameterKind};

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
    let (parameters, rest) =
        parse_generic_parameters(&mut trees, Input::new(source_id, &tokens), syntax)?;
    assert!(rest.tokens.is_empty());
    Ok((trees, parameters))
}

#[test]
fn all_generic_sites_retain_lifetimes_types_and_constants() {
    for syntax in SYNTAX_CHOICES {
        let (trees, parameters) =
            parse_parameters("<'buffer, Value [copy], const Count: usize>", syntax).unwrap();
        assert_eq!(parameters.lifetime_parameters.len(), 1);
        assert_eq!(parameters.lifetime_parameters[0].as_str(), "buffer");
        let types = trees.items.type_parameters(parameters.type_parameters);
        assert_eq!(types.len(), 2);
        assert!(matches!(types[0].kind, TypeParameterKind::Type));
        assert!(matches!(types[1].kind, TypeParameterKind::Const { .. }));
        assert!(parameters.conformance_bounds.is_empty());
    }
}

#[test]
fn machine_binders_preserve_requirement_identity_only_in_traits() {
    assert!(parse_parameters("<machine Operation>", GenericParameterSyntax::TypeAndConst).is_err());
    for syntax in [
        GenericParameterSyntax::StaticBinders,
        GenericParameterSyntax::DataDeclaration,
        GenericParameterSyntax::TraitRequirements,
        GenericParameterSyntax::MachineDeclaration,
    ] {
        let (trees, parameters) = parse_parameters("<machine Operation>", syntax).unwrap();
        let kind = &trees.items.type_parameters(parameters.type_parameters)[0].kind;
        if matches!(syntax, GenericParameterSyntax::TraitRequirements) {
            assert!(matches!(
                kind,
                TypeParameterKind::Machine {
                    contract: Some(MachineParameterContract::RequirementIdentity)
                }
            ));
        } else {
            assert!(matches!(
                kind,
                TypeParameterKind::Machine { contract: None }
            ));
        }
    }
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
        let conformance = parse_parameters("<Evidence: Value satisfies Trait>", syntax);
        assert_eq!(
            conformance.is_ok(),
            matches!(
                syntax,
                GenericParameterSyntax::TraitRequirements
                    | GenericParameterSyntax::MachineDeclaration
            )
        );
        if let Ok((_, parameters)) = conformance {
            assert_eq!(parameters.conformance_bounds.len(), 1);
            assert!(parameters.type_parameters.is_empty());
        }
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
fn non_conformance_sites_reject_satisfies_after_a_value_binder() {
    for syntax in [
        GenericParameterSyntax::DataDeclaration,
        GenericParameterSyntax::RequirementSignature,
    ] {
        let result = parse_parameters("<Evidence: Value satisfies Trait>", syntax);
        let Err(error) = result else {
            panic!("a `satisfies` tail is a conformance binder, not a value binder")
        };
        assert!(
            error
                .message
                .contains("conformance binder is not admitted in this parameter list"),
            "{:?}",
            error.message
        );
    }
}
