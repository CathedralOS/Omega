use crate::parser::input::{parse_path, Input, ParseResult};
use omega_core::arena::{Handle, HandleSpan};
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
    let mut trust_policy_start = Handle::invalid();
    let mut trust_policy_count = 0u32;

    while !input.at_punctuation(PunctuationKind::RightBrace) {
        if input.at_keyword(KeywordKind::Host) {
            input = input.take_keyword(KeywordKind::Host, "host")?;
            let (value, rest) = parse_target_host(syntax_trees, input)?;
            host = Some(value);
            input = rest;
        } else if input.at_keyword(KeywordKind::Trust) {
            input = input.take_keyword(KeywordKind::Trust, "trust")?;
            let (value, rest) = parse_trust_policy(syntax_trees, input)?;
            let handle = syntax_trees.items.append_trust_policy(value);
            if trust_policy_count == 0 {
                trust_policy_start = handle;
            }
            trust_policy_count = trust_policy_count
                .checked_add(1)
                .expect("target trust policy span count overflow");
            input = rest;
        } else {
            return Err(input.error_here("expected target item"));
        }
    }

    input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
    let trust_policies = if trust_policy_count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(trust_policy_start, trust_policy_count)
    };
    Ok((
        TargetDefinition {
            name,
            host,
            trust_policies,
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
    let provider = syntax_trees
        .items
        .insert_identifier_path_members(provider.iter().cloned());
    input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
    let mut setting_start = Handle::invalid();
    let mut setting_count = 0u32;

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

        let handle = syntax_trees
            .items
            .append_target_host_setting(TargetHostSetting { name, value });
        if setting_count == 0 {
            setting_start = handle;
        }
        setting_count = setting_count
            .checked_add(1)
            .expect("target host setting span count overflow");
        input = rest;
    }

    input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
    let settings = if setting_count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(setting_start, setting_count)
    };
    Ok((
        TargetHost {
            provider,
            settings,
        },
        input,
    ))
}

fn parse_trust_policy<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, TrustPolicy> {
    let (mode, input) = if input.at_contextual("unchecked") {
        (TrustMode::Unchecked, input.take_contextual("unchecked")?)
    } else {
        (TrustMode::Checked, input)
    };

    let (path, input) = if input.at_keyword(KeywordKind::Host) {
        let input = input.take_keyword(KeywordKind::Host, "host")?;
        (
            syntax_trees
                .items
                .insert_identifier_path_members([Identifier::generated("host")]),
            input,
        )
    } else {
        let (path, input) = parse_path(input)?;
        (
            syntax_trees
                .items
                .insert_identifier_path_members(path.iter().cloned()),
            input,
        )
    };

    Ok((TrustPolicy { mode, path }, input))
}
