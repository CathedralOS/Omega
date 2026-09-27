use super::parse_generic_parameters::{
    GenericParameterSyntax, ParsedGenericParameters, parse_generic_parameters,
};
use crate::ParseError;
use crate::input::token_cursor::Input;
use source::SourceId;
use source_files_to_tokens::Lexer;
use syntax_trees::SyntaxTrees;
use syntax_trees::item::TypeParameterKind;

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
