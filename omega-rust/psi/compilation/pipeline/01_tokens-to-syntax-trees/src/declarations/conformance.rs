use super::namespace::take_optional_semicolon;
use crate::contracts::conformance::parse_conformance::parse_conformance_trait_application;
use crate::declarations::machines::parse_machine;
use crate::input::token_cursor::{Input, ParseResult, parse_path_handle_span};
use crate::parameters::parse_generic_parameters::GenericParameterSyntax;
use crate::parameters::parse_generic_parameters::parse_generic_parameters;
use arena::{Handle, HandleSpan};
use syntax_trees::SyntaxTrees;
use syntax_trees::item::{ConformanceBody, ConformanceMember, Item, State};
use tokens::{KeywordKind, PunctuationKind};

pub(super) fn parse_conformance<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    alias: syntax_trees::identifier::Identifier,
    rest: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, Item> {
    let (generic_parameters, rest) =
        parse_generic_parameters(syntax_trees, rest, GenericParameterSyntax::StaticBinders)?;
    let mut rest = rest.take_punctuation(PunctuationKind::Colon, ":")?;
    let subject = if rest.at_contextual("satisfies") {
        syntax_trees::item::ConformanceSubject::Subjectless
    } else {
        let (subject, next) = rest.take_identifier()?;
        rest = next;
        syntax_trees::item::ConformanceSubject::Carrier(subject)
    };
    rest = rest.take_contextual("satisfies")?;
    let ((trait_name, trait_lifetime_arguments, trait_arguments), rest) =
        parse_conformance_trait_application(syntax_trees, rest)?;
    let ((), rest) = crate::parameters::contracts::parse_machine_parameter_contracts(
        syntax_trees,
        generic_parameters.type_parameters,
        rest,
    )?;
    let (body, rest) = if rest.at_punctuation(PunctuationKind::LeftBrace) {
        parse_conformance_body(syntax_trees, rest)?
    } else if matches!(subject, syntax_trees::item::ConformanceSubject::Carrier(_)) {
        (
            ConformanceBody::AttachedRequirementMachines,
            take_optional_semicolon(rest)?,
        )
    } else {
        return Err(rest.error_here(
            "a carrierless name-first conformance owns one complete closed implementation; add `{ ... }` after the selected trait application",
        ));
    };
    Ok((
        Item::Conformance(syntax_trees::item::ConformanceItem {
            is_public: false,
            lifetime_parameters: generic_parameters.lifetime_parameters,
            type_parameters: generic_parameters.type_parameters,
            subject,
            trait_name,
            trait_lifetime_arguments,
            trait_arguments,
            alias: Some(alias),
            body,
        }),
        rest,
    ))
}

fn parse_conformance_body<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, ConformanceBody> {
    let mut input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
    let mut member_start = Handle::invalid();
    let mut member_count = 0u32;

    while !input.at_punctuation(PunctuationKind::RightBrace) {
        let (member, rest) = if input.at_keyword(KeywordKind::Machine) {
            let after_machine = input.take_keyword(KeywordKind::Machine, "machine")?;
            let (mut machine, rest) = parse_machine(syntax_trees, after_machine)?;
            if machine.service_reach_is_installation_bound {
                return Err(rest.error_here(
                    "`reaches <= Bound` is legal only on a top-level bodyless `boundary machine` requirement",
                ));
            }
            if machine.spelling.is_some() {
                return Err(rest.error_here(
                    "a conformance member supplies an implementation and cannot declare an \
                     operator token; the requirement owns the token binding",
                ));
            }
            if machine.attached_data.is_some() {
                return Err(input.error_here(
                    "a conformance-block machine names only its requirement slot; the enclosing conformance supplies the carrier",
                ));
            }
            if machine.bodyless {
                return Err(input.error_here(
                    "a conformance-block machine requires a checked body; use an explicit reference row to share an existing realization",
                ));
            }
            if !machine.satisfies.is_empty() {
                return Err(input.error_here(
                    "a conformance-block machine already belongs to its enclosing conformance; remove its nested `satisfies` clause",
                ));
            }
            normalize_conformance_machine_entry(syntax_trees, &mut machine);
            (ConformanceMember::Machine(machine), rest)
        } else {
            let (declaring_trait, rest) = input.take_identifier()?;
            let rest = rest.take_punctuation(PunctuationKind::ColonColon, "::")?;
            let (requirement, rest) = rest.take_identifier()?;
            let rest = rest.take_punctuation(PunctuationKind::Equal, "=")?;
            let (target, rest) = parse_path_handle_span(rest, |member| {
                syntax_trees.items.append_identifier_path_member(member)
            })?;
            let rest = rest.take_punctuation(PunctuationKind::Semicolon, ";")?;
            (
                ConformanceMember::Reference {
                    declaring_trait,
                    requirement,
                    target,
                },
                rest,
            )
        };

        let handle = syntax_trees.items.append_conformance_member(member);
        if member_count == 0 {
            member_start = handle;
        }
        member_count = member_count
            .checked_add(1)
            .expect("conformance member span count overflow");
        input = rest;
    }

    let input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
    let members = if member_count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(member_start, member_count)
    };
    Ok((ConformanceBody::Closed { members }, input))
}

fn normalize_conformance_machine_entry(
    syntax_trees: &mut SyntaxTrees,
    machine: &mut syntax_trees::item::Machine,
) {
    let requirement_name = machine.name.clone();
    let state_handles = syntax_trees.items.state_handles(machine.states).to_vec();
    if let Some(first) = state_handles.first().copied() {
        let state = syntax_trees.items.state_mut(first);
        if state.name.as_str() == "entry" && !state.name.is_source_backed() {
            state.name = requirement_name;
        }
        return;
    }

    let state = syntax_trees.items.insert_state(&State {
        name: requirement_name,
        parameters: HandleSpan::empty(),
        return_type: syntax_trees::types::TypeReferenceHandle::invalid(),
        contracts: HandleSpan::empty(),
        statements: HandleSpan::empty(),
    });
    let state = syntax_trees.items.append_state_handle(state);
    machine.states = HandleSpan::from_parts(state, 1);
}
