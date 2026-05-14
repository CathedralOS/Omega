use crate::parser::input::{parse_path, Input, ParseResult};
use omega_syntax_trees::SyntaxTrees;
use omega_syntax_trees::identifier::Identifier;
use omega_syntax_trees::item::{
    TargetDefinition, TargetHost, TargetHostSetting, TargetHostSettingValue, TrustMode,
    TrustPolicy,
};
use omega_tokens::{KeywordKind, PunctuationKind};

pub(super) fn parse_target_definition<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, TargetDefinition> {
    let (name, mut input) = input.take_identifier()?;
    input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
    let mut host = None;
    let mut trust_policies = Vec::new();

    while !input.at_punctuation(PunctuationKind::RightBrace) {
        if input.at_keyword(KeywordKind::Host) {
            input = input.take_keyword(KeywordKind::Host, "host")?;
            let (value, rest) = parse_target_host(syntax_trees, input)?;
            host = Some(value);
            input = rest;
        } else if input.at_keyword(KeywordKind::Trust) {
            input = input.take_keyword(KeywordKind::Trust, "trust")?;
            let (value, rest) = parse_trust_policy(input)?;
            trust_policies.push(value);
            input = rest;
        } else {
            return Err(input.error_here("expected target item"));
        }
    }

    input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
    Ok((
        TargetDefinition {
            name,
            host,
            trust_policies: syntax_trees.items.insert_trust_policies(trust_policies),
        },
        input,
    ))
}

fn parse_target_host<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, TargetHost> {
    let input = input.take_punctuation(PunctuationKind::Colon, ":")?;
    let (provider, mut input) = parse_path(input)?;
    input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
    let mut settings = Vec::new();

    while !input.at_punctuation(PunctuationKind::RightBrace) {
        let (name, rest) = input.take_identifier()?;
        let rest = rest.take_punctuation(PunctuationKind::Equal, "=")?;
        let (value_name, rest) = rest.take_identifier()?;
        let (value, rest) = if rest.at_punctuation(PunctuationKind::LeftParen) {
            let rest = rest.take_punctuation(PunctuationKind::LeftParen, "(")?;
            let (argument_tokens, rest) = rest.skip_parenthesized_tokens_after_open()?;
            (
                TargetHostSettingValue::Call {
                    name: value_name,
                    argument_tokens,
                },
                rest,
            )
        } else {
            (TargetHostSettingValue::Named(value_name), rest)
        };

        settings.push(TargetHostSetting { name, value });
        input = rest;
    }

    input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
    Ok((
        TargetHost {
            provider,
            settings: syntax_trees.items.insert_target_host_settings(settings),
        },
        input,
    ))
}

fn parse_trust_policy<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, TrustPolicy> {
    let (mode, input) = if input.at_contextual("unchecked") {
        (TrustMode::Unchecked, input.take_contextual("unchecked")?)
    } else {
        (TrustMode::Checked, input)
    };

    let (path, input) = if input.at_keyword(KeywordKind::Host) {
        let input = input.take_keyword(KeywordKind::Host, "host")?;
        (omega_syntax_trees::identifier::IdentifierPath::from(vec![Identifier::generated("host")]), input)
    } else {
        parse_path(input)?
    };

    Ok((TrustPolicy { mode, path }, input))
}
