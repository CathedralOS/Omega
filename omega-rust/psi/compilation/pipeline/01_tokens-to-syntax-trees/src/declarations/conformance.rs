use crate::contracts::conformance::parse_conformance::parse_conformance_trait_application;
use crate::declarations::machines::{MachineEntryNaming, parse_machine};
use crate::input::token_cursor::{Input, ParseResult, parse_path_handle_span};
use crate::parameters::parse_generic_parameters::GenericParameterSyntax;
use crate::parameters::parse_generic_parameters::parse_generic_parameters;
use arena::HandleSpan;
use syntax_trees::SyntaxTrees;
use syntax_trees::item::{ConformanceMember, Item};
use tokens::{KeywordKind, PunctuationKind};

/// `Name<binders> satisfies Trait<Args> { rows }`. The name identifies a
/// closed map of machines; data never satisfies a trait, so there is no
/// `Name: Data` subject and no bodyless form whose rows come from machines
/// attached elsewhere.
pub(super) fn parse_conformance<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    alias: syntax_trees::identifier::Identifier,
    rest: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, Item> {
    let (generic_parameters, rest) =
        parse_generic_parameters(syntax_trees, rest, GenericParameterSyntax::StaticBinders)?;
    if rest.at_punctuation(PunctuationKind::Colon) {
        return Err(rest.error_here(
            "data does not satisfy traits; declare a named implementation `Name satisfies Trait<Data> { ... }` that names each data type as an ordinary trait argument and binds every requirement row",
        ));
    }
    let rest = rest.take_contextual("satisfies")?;
    let ((trait_name, trait_lifetime_arguments, trait_arguments), rest) =
        parse_conformance_trait_application(syntax_trees, rest)?;
    let ((), rest) = crate::parameters::contracts::parse_machine_parameter_contracts(
        syntax_trees,
        generic_parameters.type_parameters,
        rest,
    )?;
    if !rest.at_punctuation(PunctuationKind::LeftBrace) {
        return Err(rest.error_here(
            "a named implementation owns one complete closed map; add `{ ... }` binding every requirement after the selected trait application",
        ));
    }
    let (members, rest) = parse_conformance_members(syntax_trees, rest)?;
    Ok((
        Item::Conformance(syntax_trees::item::ConformanceItem {
            is_public: false,
            lifetime_parameters: generic_parameters.lifetime_parameters,
            type_parameters: generic_parameters.type_parameters,
            trait_name,
            trait_lifetime_arguments,
            trait_arguments,
            alias: Some(alias),
            members,
        }),
        rest,
    ))
}

fn parse_conformance_members<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, HandleSpan<ConformanceMember>> {
    let mut input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
    let mut members = crate::handle_spans::HandleSpanBuilder::new();

    while !input.at_punctuation(PunctuationKind::RightBrace) {
        let (member, rest) = if input.at_keyword(KeywordKind::Machine) {
            let after_machine = input.take_keyword(KeywordKind::Machine, "machine")?;
            let (machine, rest) = parse_machine(
                syntax_trees,
                after_machine,
                MachineEntryNaming::ImplementationMember,
            )?;
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
                    "a conformance-block machine names only its requirement slot; declare its data parameters explicitly",
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
        members.push(handle);
        input = rest;
    }

    let input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
    Ok((members.finish(), input))
}
