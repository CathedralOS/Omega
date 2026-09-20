use crate::bodies::trait_default::parse_trait_default_machine_body;
use crate::contracts::signature::parse_signature_clauses;
use crate::input::token_cursor::{Input, ParseResult};
use crate::parameters::parse_generic_parameters::GenericParameterSyntax;
use crate::parameters::parse_generic_parameters::parse_generic_parameters;
use crate::parameters::parse_parameters::{
    parse_optional_parameters, parse_optional_return_type, parse_parameter,
};
use crate::type_syntax::parse_type::parse_type_reference_handle;
use arena::{Handle, HandleSpan};
use syntax_trees::SyntaxTrees;
use syntax_trees::identifier::Identifier;
use syntax_trees::item::{StateSignature, TraitDefinition};
use tokens::{KeywordKind, PunctuationKind};

pub(super) fn parse_trait_definition<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
    is_boundary: bool,
) -> ParseResult<'tokens, 'source, TraitDefinition> {
    let input = input.take_contextual("trait")?;
    let (name, mut input) = input.take_identifier()?;
    let (generic_parameters, next) = parse_generic_parameters(
        syntax_trees,
        input,
        GenericParameterSyntax::TraitRequirements,
    )?;
    input = next;
    let type_parameters = generic_parameters.type_parameters;
    // A transparent refinement heads with `= Base` in place of a `:` parent
    // list: it names a structural bound over an existing base conformance,
    // never a new conformance target.
    let (refines, next) = if input.at_punctuation(PunctuationKind::Equal) {
        let after_equal = input.take_punctuation(PunctuationKind::Equal, "=")?;
        let (base, rest) = parse_type_reference_handle(syntax_trees, after_equal)?;
        (Some(base), rest)
    } else {
        (None, input)
    };
    input = next;
    let (parents, next) = if refines.is_some() {
        if input.at_punctuation(PunctuationKind::Colon) {
            return Err(input.error_here(
                "a transparent refinement declares its base with `= Base`; the `:` parent list belongs to ordinary traits",
            ));
        }
        (HandleSpan::empty(), input)
    } else {
        parse_trait_parents(syntax_trees, input)?
    };
    input = next;
    if refines.is_some() && input.at_contextual("where") {
        return Err(input.error_here(
            "a transparent refinement takes no `where` clauses; bounds on its parameters belong to the base trait",
        ));
    }
    let ((), next) = parse_proposition_parameter_contracts(syntax_trees, type_parameters, input)?;
    input = next;
    let mut conformance_bounds = generic_parameters.conformance_bounds;
    if input.at_contextual("where") {
        let (mut bounds, next) =
            crate::contracts::conformance::parse_conformance::parse_generic_conformance_bounds(
                syntax_trees,
                input,
            )?;
        input = next;
        conformance_bounds.append(&mut bounds);
    }
    input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
    let mut required_trait_start = Handle::invalid();
    let mut required_trait_count = 0u32;
    let mut machine_start = Handle::invalid();
    let mut machine_count = 0u32;
    let mut refinement_clauses = Vec::new();

    if refines.is_some() {
        while !input.at_punctuation(PunctuationKind::RightBrace) {
            if input.at_contextual("requires") {
                return Err(input.error_here(
                    "a refinement narrows its base contract; it cannot add `requires` requirements",
                ));
            }
            if input.at_contextual("invariant") {
                return Err(input.error_here(
                    "the `invariant` clause is retired: a refinement narrows operational axes only",
                ));
            }
            input = input.take_keyword(KeywordKind::Machine, "machine")?;
            let ((), next) =
                parse_refinement_clause(syntax_trees, input, refines, &mut refinement_clauses)?;
            input = next;
        }
        input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
        return Ok((
            TraitDefinition {
                is_boundary,
                is_public: false,
                name,
                lifetime_parameters: generic_parameters.lifetime_parameters,
                type_parameters,
                conformance_bounds,
                parents,
                requires: HandleSpan::empty(),
                machines: HandleSpan::empty(),
                refines,
                refinement_clauses,
            },
            input,
        ));
    }

    while !input.at_punctuation(PunctuationKind::RightBrace) {
        if input.at_contextual("invariant") {
            return Err(input.error_here(
                "the `invariant` clause is retired: traits publish proof obligations through \
                 explicit `requires` and `ensures`; value-wide facts belong to the carrier's \
                 default domain",
            ));
        }

        if input.at_contextual("requires") {
            let (required_trait, rest) = parse_trait_requirement(input)?;
            let handle = syntax_trees
                .items
                .append_identifier_path_member(required_trait);
            if required_trait_count == 0 {
                required_trait_start = handle;
            }
            required_trait_count = required_trait_count
                .checked_add(1)
                .expect("trait requirement span count overflow");
            input = rest;
            continue;
        }

        // The `default` KEYWORD IS KILLED (owner, 2026-07-18): a trait
        // machine WITH A BODY is the default -- body presence is the
        // marker. The old spelling refuses with direction.
        if input.at_contextual("default") {
            return Err(input.error_here(
                "the `default` keyword is retired: a trait machine with a body IS \
                 the default -- drop the keyword and keep the body",
            ));
        }
        let spelling = if input.at_contextual("operator") {
            input = input.take_contextual("operator")?;
            if input
                .tokens
                .first()
                .is_some_and(|token| token.punctuation().is_some())
            {
                let (spelling, rest) =
                    crate::declarations::operator::parse_operator_spelling(input)?;
                input = rest;
                Some(spelling)
            } else {
                None
            }
        } else {
            input = input.take_keyword(KeywordKind::Machine, "machine")?;
            None
        };
        let (mut signature, rest) = parse_trait_machine_signature(syntax_trees, input)?;
        signature.spelling = spelling;
        let (
            (
                service_reaches,
                service_reach_keyword_source_spans,
                service_reach_is_installation_bound,
                invokes,
                suspends_keyword_source_spans,
                blocks_keyword_source_spans,
                suspends,
                blocks,
                contracts,
                terminates_guarantee,
                where_facts,
            ),
            rest,
        ) = parse_signature_clauses(syntax_trees, rest, true)?;
        // Body presence = the default marker.
        let is_default = rest.at_punctuation(PunctuationKind::LeftBrace);
        if !signature.native_callback_parameters.is_empty() && (!is_boundary || is_default) {
            return Err(rest.error_here(
                "`native callback ... from ...` is permitted only on a bodyless boundary-trait requirement",
            ));
        }
        signature.is_default = is_default;
        if service_reach_is_installation_bound && (!is_boundary || is_default) {
            return Err(rest.error_here(
                "`reaches <= Bound` is permitted only on a bodyless boundary-trait requirement",
            ));
        }
        signature.service_reach_is_installation_bound = service_reach_is_installation_bound;
        signature.service_reach_keyword_source_spans = service_reach_keyword_source_spans;
        signature.service_reaches = service_reaches;
        signature.invokes = invokes;
        signature.suspends_keyword_source_spans = suspends_keyword_source_spans;
        signature.blocks_keyword_source_spans = blocks_keyword_source_spans;
        signature.suspends = suspends;
        signature.blocks = blocks;
        signature.contracts = contracts;
        signature.terminates_guarantee = terminates_guarantee;
        signature.where_facts = where_facts;
        let (default_body, next) = if is_default {
            parse_trait_default_machine_body(syntax_trees, rest)?
        } else {
            (
                HandleSpan::empty(),
                rest.take_punctuation(PunctuationKind::Semicolon, ";")?,
            )
        };
        signature.default_body = default_body;
        let handle = syntax_trees.items.insert_state_signature(&signature);
        let handle = syntax_trees.items.append_state_signature_handle(handle);
        if machine_count == 0 {
            machine_start = handle;
        }
        machine_count = machine_count
            .checked_add(1)
            .expect("trait machine signature span count overflow");
        input = next;
    }

    input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
    let requires = if required_trait_count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(required_trait_start, required_trait_count)
    };
    let machines = if machine_count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(machine_start, machine_count)
    };
    Ok((
        TraitDefinition {
            is_boundary,
            is_public: false,
            name,
            lifetime_parameters: generic_parameters.lifetime_parameters,
            type_parameters,
            conformance_bounds,
            parents,
            requires,
            machines,
            refines,
            refinement_clauses,
        },
        input,
    ))
}

/// One `machine *` or `machine Base::requirement` narrowing clause of a
/// transparent refinement. A clause names an existing base requirement and
/// carries only the operational axes the spec narrows: `reaches`, `suspends`,
/// `blocks`, `terminates`. Parameters, results, generic arguments, bodies,
/// contract clauses, and installation-bound reach rows all belong to the base
/// declaration, not the bound.
fn parse_refinement_clause<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    mut input: Input<'tokens, 'source>,
    refines: Option<syntax_trees::types::TypeReferenceHandle>,
    clauses: &mut Vec<syntax_trees::item::TraitRefinementClause>,
) -> ParseResult<'tokens, 'source, ()> {
    let requirement = if input.at_punctuation(PunctuationKind::Asterisk) {
        input = input.take_punctuation(PunctuationKind::Asterisk, "*")?;
        None
    } else {
        let mut segments = Vec::new();
        let (member, rest) = input.take_identifier()?;
        segments.push(member);
        input = rest;
        while input.at_punctuation(PunctuationKind::ColonColon) {
            input = input.take_punctuation(PunctuationKind::ColonColon, "::")?;
            let (member, rest) = input.take_identifier()?;
            segments.push(member);
            input = rest;
        }
        let qualifier = &segments[..segments.len() - 1];
        if !qualifier.is_empty() {
            let base_name =
                refines.and_then(
                    |base| match syntax_trees.type_references.type_reference(base) {
                        syntax_trees::types::TypeReferenceNode::Named(name) => Some(name.clone()),
                        syntax_trees::types::TypeReferenceNode::Generic { base_name, .. } => {
                            Some(base_name.clone())
                        }
                        _ => None,
                    },
                );
            let qualifies_base = qualifier
                .iter()
                .all(|segment| Some(segment.clone()) == base_name || segment.as_str() == "Self");
            if !qualifies_base || qualifier.len() > 1 {
                return Err(input.error_here(
                    "a refinement clause qualifies its requirement with the base trait name or `Self`",
                ));
            }
        }
        Some(segments.last().expect("nonempty segments").clone())
    };
    if input.at_punctuation(PunctuationKind::LeftParen)
        || input.at_punctuation(PunctuationKind::Arrow)
        || input.at_punctuation(PunctuationKind::Less)
    {
        return Err(input.error_here(
            "a refinement clause names an existing base requirement; it declares no parameters, generics, or result",
        ));
    }
    let mut signature = StateSignature {
        name: requirement
            .clone()
            .unwrap_or_else(|| Identifier::generated("*")),
        spelling: None,
        lifetime_parameters: Vec::new(),
        type_parameters: HandleSpan::empty(),
        is_default: false,
        parameters: HandleSpan::empty(),
        native_callback_parameters: Vec::new(),
        return_type: Handle::invalid(),
        service_reach_is_installation_bound: false,
        service_reach_keyword_source_spans: Vec::new(),
        service_reaches: HandleSpan::empty(),
        invokes: HandleSpan::empty(),
        suspends_keyword_source_spans: Vec::new(),
        blocks_keyword_source_spans: Vec::new(),
        suspends: false,
        blocks: false,
        contracts: HandleSpan::empty(),
        default_body: HandleSpan::empty(),
        terminates_guarantee: false,
        where_facts: HandleSpan::empty(),
    };
    let mut authored_suspends = false;
    let mut authored_blocks = false;
    let mut authored_terminates = false;
    let mut authored_reaches = false;
    loop {
        if input.at_contextual("reaches") {
            if authored_reaches {
                return Err(input.error_here("duplicate `reaches` clause on a refinement member"));
            }
            authored_reaches = true;
            signature
                .service_reach_keyword_source_spans
                .push(input.current_source_span());
            input = input.take_contextual("reaches")?;
            if input.at_punctuation(PunctuationKind::LessEqual) {
                return Err(input.error_here(
                    "`reaches <= Bound` is an installation row; a refinement narrows the authored reach set",
                ));
            }
            let mut reach_start = Handle::invalid();
            let mut reach_count = 0u32;
            while !input.at_punctuation(PunctuationKind::Semicolon) {
                let (row, rest) = input.take_identifier()?;
                let handle = syntax_trees.items.append_identifier_path_member(row);
                if reach_count == 0 {
                    reach_start = handle;
                }
                reach_count = reach_count
                    .checked_add(1)
                    .expect("refinement reach row count overflow");
                input = rest;
                if input.at_punctuation(PunctuationKind::Plus) {
                    input = input.take_punctuation(PunctuationKind::Plus, "+")?;
                }
            }
            signature.service_reaches = if reach_count == 0 {
                HandleSpan::empty()
            } else {
                HandleSpan::from_parts(reach_start, reach_count)
            };
            input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
            continue;
        }
        if input.at_contextual("suspends") {
            if authored_suspends {
                return Err(input.error_here("duplicate `suspends` clause on a refinement member"));
            }
            authored_suspends = true;
            signature
                .suspends_keyword_source_spans
                .push(input.current_source_span());
            input = input.take_contextual("suspends")?;
            if input.at_punctuation(PunctuationKind::Semicolon) {
                signature.suspends = true;
            } else {
                let (value, rest) = parse_refinement_axis_bool(input)?;
                signature.suspends = value;
                if !rest.at_punctuation(PunctuationKind::Semicolon) {
                    return Err(rest.error_here(
                        "a `suspends` refinement clause reads `suspends;`, `suspends true;`, or `suspends false;`",
                    ));
                }
                input = rest;
            }
            input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
            continue;
        }
        if input.at_contextual("blocks") {
            if authored_blocks {
                return Err(input.error_here("duplicate `blocks` clause on a refinement member"));
            }
            authored_blocks = true;
            signature
                .blocks_keyword_source_spans
                .push(input.current_source_span());
            input = input.take_contextual("blocks")?;
            if input.at_punctuation(PunctuationKind::Semicolon) {
                signature.blocks = true;
            } else {
                let (value, rest) = parse_refinement_axis_bool(input)?;
                signature.blocks = value;
                if !rest.at_punctuation(PunctuationKind::Semicolon) {
                    return Err(rest.error_here(
                        "a `blocks` refinement clause reads `blocks;`, `blocks true;`, or `blocks false;`",
                    ));
                }
                input = rest;
            }
            input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
            continue;
        }
        if input.at_contextual("terminates") {
            if authored_terminates {
                return Err(
                    input.error_here("duplicate `terminates` clause on a refinement member")
                );
            }
            authored_terminates = true;
            signature.terminates_guarantee = true;
            input = input.take_contextual("terminates")?;
            input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
            continue;
        }
        break;
    }
    clauses.push(syntax_trees::item::TraitRefinementClause {
        requirement,
        signature,
    });
    Ok(((), input))
}

fn parse_refinement_axis_bool<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, bool> {
    if input.at_keyword(KeywordKind::True) {
        return Ok((true, input.take_keyword(KeywordKind::True, "true")?));
    }
    if input.at_keyword(KeywordKind::False) {
        return Ok((false, input.take_keyword(KeywordKind::False, "false")?));
    }
    Err(input.error_here("expected `true` or `false`"))
}

fn parse_proposition_parameter_contracts<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    type_parameters: HandleSpan<syntax_trees::item::TypeParameter>,
    mut input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, ()> {
    loop {
        if !input.at_contextual("where") {
            break;
        }
        let after_where = input.take_contextual("where")?;
        if !after_where.at_contextual("proposition") {
            break;
        }
        let after_proposition = after_where.take_contextual("proposition")?;
        let (name, after_name) = after_proposition.take_identifier()?;
        let Some(parameter_index) = syntax_trees
            .items
            .type_parameters(type_parameters)
            .iter()
            .position(|parameter| parameter.name == name)
        else {
            return Err(after_name.error_here(format!(
                "`where proposition {}` has no matching `<proposition {}>` parameter",
                name.as_str(),
                name.as_str(),
            )));
        };
        match &syntax_trees.items.type_parameters(type_parameters)[parameter_index].kind {
            syntax_trees::item::TypeParameterKind::Proposition { contract: None } => {}
            syntax_trees::item::TypeParameterKind::Proposition { contract: Some(_) } => {
                return Err(after_name.error_here(format!(
                    "proposition parameter `{}` already has a declaration-site signature",
                    name.as_str(),
                )));
            }
            _ => {
                return Err(after_name.error_here(format!(
                    "`{}` is not a proposition parameter; declare it as `<proposition {}>` first",
                    name.as_str(),
                    name.as_str(),
                )));
            }
        }
        let (parameters, rest) = parse_optional_parameters(syntax_trees, after_name)?;
        let rest = rest.take_punctuation(PunctuationKind::Semicolon, ";")?;
        syntax_trees.items.type_parameters_mut(type_parameters)[parameter_index].kind =
            syntax_trees::item::TypeParameterKind::Proposition {
                contract: Some(syntax_trees::item::PropositionParameterSignature {
                    name,
                    parameters,
                }),
            };
        input = rest;
    }

    if let Some(missing) = syntax_trees
        .items
        .type_parameters(type_parameters)
        .iter()
        .find(|parameter| {
            matches!(
                parameter.kind,
                syntax_trees::item::TypeParameterKind::Proposition { contract: None }
            )
        })
    {
        return Err(input.error_here(format!(
            "proposition parameter `{}` requires an authored declaration-site signature: write `where proposition {}(...)`;",
            missing.name.as_str(),
            missing.name.as_str(),
        )));
    }
    Ok(((), input))
}

fn parse_trait_parents<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    mut input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, HandleSpan<syntax_trees::types::TypeReferenceHandle>> {
    if !input.at_punctuation(PunctuationKind::Colon) {
        return Ok((HandleSpan::empty(), input));
    }

    input = input.take_punctuation(PunctuationKind::Colon, ":")?;
    let mut parents = Vec::new();
    loop {
        let (parent, rest) = parse_type_reference_handle(syntax_trees, input)?;
        parents.push(parent);
        input = rest;
        if !input.at_punctuation(PunctuationKind::Plus) {
            break;
        }
        input = input.take_punctuation(PunctuationKind::Plus, "+")?;
    }
    Ok((
        syntax_trees
            .type_references
            .insert_type_reference_handles(parents),
        input,
    ))
}

fn parse_trait_machine_signature<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, StateSignature> {
    let (name, input) = parse_trait_machine_name(input)?;
    let (generic_parameters, input) = parse_generic_parameters(
        syntax_trees,
        input,
        GenericParameterSyntax::RequirementSignature,
    )?;
    let ((parameters, native_callback_parameters), input) =
        parse_boundary_parameter_telescope(syntax_trees, input)?;
    let (return_type, input) = parse_optional_return_type(syntax_trees, input)?;
    let ((), input) = crate::parameters::contracts::parse_machine_parameter_contracts(
        syntax_trees,
        generic_parameters.type_parameters,
        input,
    )?;

    Ok((
        StateSignature {
            name,
            spelling: None,
            lifetime_parameters: generic_parameters.lifetime_parameters,
            type_parameters: generic_parameters.type_parameters,
            is_default: false,
            parameters,
            native_callback_parameters,
            return_type,
            service_reach_is_installation_bound: false,
            service_reach_keyword_source_spans: Vec::new(),
            service_reaches: HandleSpan::empty(),
            invokes: HandleSpan::empty(),
            suspends_keyword_source_spans: Vec::new(),
            blocks_keyword_source_spans: Vec::new(),
            suspends: false,
            blocks: false,
            contracts: HandleSpan::empty(),
            default_body: HandleSpan::empty(),
            terminates_guarantee: false,
            where_facts: HandleSpan::empty(),
        },
        input,
    ))
}

fn parse_boundary_parameter_telescope<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<
    'tokens,
    'source,
    (
        HandleSpan<syntax_trees::item::StateParameterHandle>,
        Vec<syntax_trees::item::NativeCallbackParameterNode>,
    ),
> {
    if !input.at_punctuation(PunctuationKind::LeftParen) {
        return Ok(((HandleSpan::empty(), Vec::new()), input));
    }

    let mut input = input.take_punctuation(PunctuationKind::LeftParen, "(")?;
    let mut parameter_start = Handle::invalid();
    let mut parameter_count = 0u32;
    let mut native_callback_parameters = Vec::new();
    let mut native_ordinal = 0u32;

    while !input.at_punctuation(PunctuationKind::RightParen) {
        if input.at_contextual("native") {
            let input_after_native = input.take_contextual("native")?;
            let input_after_callback = input_after_native.take_contextual("callback")?;
            let (name, input_after_name) = input_after_callback.take_identifier()?;
            let input_after_from = input_after_name.take_contextual("from")?;
            let (binder, rest) = input_after_from.take_identifier()?;
            if native_callback_parameters
                .iter()
                .any(|prior: &syntax_trees::item::NativeCallbackParameterNode| prior.name == name)
            {
                return Err(rest.error_here(format!(
                    "native callback parameter `{}` is declared more than once",
                    name.as_str(),
                )));
            }
            native_callback_parameters.push(syntax_trees::item::NativeCallbackParameterNode {
                name,
                binder,
                native_ordinal,
            });
            input = rest;
        } else {
            let (parameter, rest) = parse_parameter(syntax_trees, input)?;
            let handle = syntax_trees.items.append_state_parameter_handle(parameter);
            if parameter_count == 0 {
                parameter_start = handle;
            }
            parameter_count = parameter_count
                .checked_add(1)
                .expect("state parameter span count overflow");
            input = rest;
        }
        native_ordinal = native_ordinal
            .checked_add(1)
            .expect("native parameter telescope count overflow");

        if input.at_punctuation(PunctuationKind::Comma) {
            input = input.take_punctuation(PunctuationKind::Comma, ",")?;
            continue;
        }
        break;
    }

    let input = input.take_punctuation(PunctuationKind::RightParen, ")")?;
    let parameters = if parameter_count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(parameter_start, parameter_count)
    };
    Ok(((parameters, native_callback_parameters), input))
}

fn parse_trait_machine_name<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, Identifier> {
    let (mut name, mut input) = if input.at_keyword(KeywordKind::SelfType) {
        let input = input.take_keyword(KeywordKind::SelfType, "Self")?;
        (Identifier::generated("Self"), input)
    } else {
        input.take_identifier()?
    };

    while input.at_punctuation(PunctuationKind::ColonColon) {
        input = input.take_punctuation(PunctuationKind::ColonColon, "::")?;
        let (member, next) = input.take_identifier()?;
        name = member;
        input = next;
    }

    Ok((name, input))
}

fn parse_trait_requirement<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, syntax_trees::identifier::Identifier> {
    let input = input.take_contextual("requires")?;
    let (required_trait, input) = input.take_identifier()?;
    let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
    Ok((required_trait, input))
}
