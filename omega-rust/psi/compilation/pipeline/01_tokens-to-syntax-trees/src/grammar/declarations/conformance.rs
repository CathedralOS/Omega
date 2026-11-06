use crate::grammar::contracts::conformance::parse_conformance::parse_conformance_trait_application;
use crate::grammar::declarations::parse_declaration::{ParsedDeclaration, parse_item};
use crate::grammar::parameters::contracts::parse_machine_parameter_contracts;
use crate::grammar::parameters::contracts::validate_machine_parameter_contracts;
use crate::grammar::parameters::parse_generic_parameters::GenericParameterSyntax;
use crate::grammar::parameters::parse_generic_parameters::parse_generic_parameters;
use crate::tokens::token_cursor::{Input, ParseResult};
use arena::HandleSpan;
use syntax_trees::SyntaxTrees;
use syntax_trees::item::{ConformanceMember, Item, Machine, State};
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
    let ((), rest) =
        parse_machine_parameter_contracts(syntax_trees, generic_parameters.type_parameters, rest)?;
    validate_machine_parameter_contracts(syntax_trees, generic_parameters.type_parameters, rest)?;
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
    let mut members = crate::storage::handle_spans::HandleSpanBuilder::new();

    while !input.at_punctuation(PunctuationKind::RightBrace) {
        // Reuse ordinary machine parsing, including public/boundary modifiers.
        // This lookahead distinguishes a member from a retired reference row;
        // it does not decide supply or the meaning of a satisfaction edge.
        let head = if input.at_keyword(KeywordKind::Public) {
            input.restarted(input.tokens.get(1..).unwrap_or_default())
        } else {
            input
        };
        let head = if head.at_contextual("boundary") {
            head.restarted(head.tokens.get(1..).unwrap_or_default())
        } else {
            head
        };
        if !head.at_keyword(KeywordKind::Machine) {
            return Err(input.error_here(
                "a named implementation declares each requirement as an inline `machine` member; \
                 a reference row (`Trait::member = Existing::machine;`) is retired, so call the \
                 existing machine from the member's body",
            ));
        }
        let (declaration, rest) = parse_item(syntax_trees, input)?;
        let ParsedDeclaration::Item(Item::Machine(mut machine)) = declaration else {
            return Err(input.error_here("a named implementation member must be a machine"));
        };
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
        // The inherited map row and any independent exact satisfactions have
        // separate identities. Resolution disambiguates inherited slots;
        // ordinary checking proves every additional contract. A bodyless
        // member still needs admitted external supply at semantic checking.
        if syntax_trees
            .items
            .satisfies_clauses(machine.satisfies)
            .iter()
            .any(|edge| edge.requirement.is_none())
        {
            return Err(input
                .error_here("an inline member's satisfies clauses must name exact requirements"));
        }
        normalize_conformance_machine_entry(syntax_trees, &mut machine);
        let handle = syntax_trees
            .items
            .append_conformance_member(ConformanceMember::Machine(machine));
        members.push(handle);
        input = rest;
    }

    let input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
    Ok((members.finish(), input))
}

fn normalize_conformance_machine_entry(syntax_trees: &mut SyntaxTrees, machine: &mut Machine) {
    let requirement_name = machine.name.clone();
    let state_handles = syntax_trees.items.state_handles(machine.states).to_vec();
    if let Some(first) = state_handles.first().copied() {
        let state = syntax_trees.items.state(first);
        if state.name.as_str(syntax_trees) == "entry" && !state.name.is_source_backed() {
            syntax_trees.items.state_mut(first).name = requirement_name;
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
