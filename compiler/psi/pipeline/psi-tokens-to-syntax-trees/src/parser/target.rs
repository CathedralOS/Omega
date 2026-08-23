use crate::parser::input::{Input, ParseResult, parse_path_handle_span};
use psi_arena::{Handle, HandleSpan};
use psi_syntax_trees::SyntaxTrees;
use psi_syntax_trees::identifier::Identifier;
use psi_syntax_trees::item::{
    BoundaryMode, BoundaryPolicy, TargetDefinition, TargetHost, TargetHostSetting,
    TargetHostSettingValue,
};
use psi_tokens::{KeywordKind, PunctuationKind};

pub(super) fn parse_target_definition<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, TargetDefinition> {
    let (name, mut input) = input.take_identifier()?;
    input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
    let mut host = None;
    let mut boundary_policy_start = Handle::invalid();
    let mut boundary_policy_count = 0u32;

    while !input.at_punctuation(PunctuationKind::RightBrace) {
        if input.at_keyword(KeywordKind::Host) {
            input = input.take_keyword(KeywordKind::Host, "host")?;
            let (value, rest) = parse_target_host(syntax_trees, input)?;
            host = Some(value);
            input = rest;
        } else if input.at_contextual("subsystem") {
            // RETIRED (build_and_package_model.md addendum): image facts come
            // from build.omg's augmenting `build(b: &mut Build)` machine, not
            // an in-source config word.
            return Err(input.error_here(
                "`subsystem` no longer lives in target blocks: set it in build.omg -- `machine build(b: &mut Build) { b.subsystem = Subsystem::Gui; }`",
            ));
        } else if input.at_contextual("boundary") {
            input = input.take_contextual("boundary")?;
            let (value, rest) = parse_boundary_policy(syntax_trees, input)?;
            let handle = syntax_trees.items.append_boundary_policy(value);
            if boundary_policy_count == 0 {
                boundary_policy_start = handle;
            }
            boundary_policy_count = boundary_policy_count
                .checked_add(1)
                .expect("target boundary policy span count overflow");
            input = rest;
        } else {
            return Err(input.error_here("expected target item"));
        }
    }

    input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
    let boundary_policies = if boundary_policy_count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(boundary_policy_start, boundary_policy_count)
    };
    Ok((
        TargetDefinition {
            name,
            host,
            boundary_policies,
        },
        input,
    ))
}

fn parse_target_host<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, TargetHost> {
    let input = input.take_punctuation(PunctuationKind::Colon, ":")?;
    let (provider, mut input) = parse_path_handle_span(input, |member| {
        syntax_trees.items.append_identifier_path_member(member)
    })?;
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
    Ok((TargetHost { provider, settings }, input))
}

fn parse_boundary_policy<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, BoundaryPolicy> {
    let (mode, input) = if input.at_contextual("unchecked") {
        (BoundaryMode::Unchecked, input.take_contextual("unchecked")?)
    } else {
        (BoundaryMode::Checked, input)
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
        parse_path_handle_span(input, |member| {
            syntax_trees.items.append_identifier_path_member(member)
        })?
    };

    Ok((BoundaryPolicy { mode, path }, input))
}
