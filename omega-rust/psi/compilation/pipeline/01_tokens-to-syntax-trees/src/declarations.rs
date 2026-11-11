/*
Top-level and nested declaration grammar (`grammar::declarations`): dispatch
across capabilities, conformances, constants, data types, domains, mathematical
`let` definitions, machines, measures, namespaces, operators, propositions,
traits, and `use` imports.
*/

pub(crate) mod capability {
    use super::namespace::take_optional_semicolon;
    use crate::grammar::contracts::facts::parse_proof_facts_until;
    use crate::grammar::parameters::parse_parameters::parse_callable_signature;
    use crate::grammar::type_syntax::parse_type::parse_type_reference_handle;
    use crate::tokens::token_cursor::{Input, ParseResult};
    use syntax_trees::SyntaxTrees;
    use syntax_trees::item::{
        CapabilityContract, CapabilityContractKind, CapabilityDefinition, CapabilityField,
        CapabilityMember, CapabilityState,
    };
    use tokens::{KeywordKind, PunctuationKind};

    pub(crate) fn parse_capability_definition<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        input: Input<'tokens, 'source>,
    ) -> ParseResult<'tokens, 'source, CapabilityDefinition> {
        let (name, mut input) = input.take_identifier()?;
        input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
        let mut members = crate::storage::handle_spans::HandleSpanBuilder::new();

        while !input.at_punctuation(PunctuationKind::RightBrace) {
            if input.at_contextual("entry") {
                let (_, after_entry) = input.take_identifier()?;
                if !after_entry.at_punctuation(PunctuationKind::Colon) {
                    return Err(input.error_here(
                    "the legacy `capability { entry ... }` host scaffold is retired; declare a `boundary trait` requirement and realize it with an exact `satisfies Trait::requirement via Binding::...` machine",
                ));
                }
            }

            if input.at_keyword(KeywordKind::State) {
                input = input.take_keyword(KeywordKind::State, "state")?;
                let (state, rest) = parse_capability_state(syntax_trees, input)?;
                let handle = syntax_trees
                    .items
                    .append_capability_member(CapabilityMember::State(state));
                members.push(handle);
                input = rest;
            } else {
                let (field_name, rest) = input.take_identifier()?;
                let rest = rest.take_punctuation(PunctuationKind::Colon, ":")?;
                let (type_reference, rest) = parse_type_reference_handle(syntax_trees, rest)?;
                input = if rest.at_punctuation(PunctuationKind::Semicolon) {
                    rest.take_punctuation(PunctuationKind::Semicolon, ";")?
                } else {
                    rest
                };
                let handle = syntax_trees
                    .items
                    .append_capability_member(CapabilityMember::Field(CapabilityField {
                        name: field_name,
                        type_reference,
                    }));
                members.push(handle);
            }
        }

        input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
        Ok((
            CapabilityDefinition {
                name,
                members: members.finish(),
            },
            input,
        ))
    }

    fn parse_capability_state<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        input: Input<'tokens, 'source>,
    ) -> ParseResult<'tokens, 'source, CapabilityState> {
        let (signature, mut input) = parse_callable_signature(syntax_trees, input)?;
        let mut contracts = crate::storage::handle_spans::HandleSpanBuilder::new();

        if input.at_punctuation(PunctuationKind::LeftBrace) {
            input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
            while !input.at_punctuation(PunctuationKind::RightBrace) {
                let (contract, rest) = parse_capability_contract(syntax_trees, input)?;
                let handle = syntax_trees.items.append_capability_contract(contract);
                contracts.push(handle);
                input = rest;
            }
            input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
        } else {
            while !(input.at_keyword(KeywordKind::State)
                || input.at_contextual("entry")
                || input.at_punctuation(PunctuationKind::RightBrace)
                || input.tokens.is_empty())
            {
                let (contract, rest) = parse_capability_contract(syntax_trees, input)?;
                let handle = syntax_trees.items.append_capability_contract(contract);
                contracts.push(handle);
                input = rest;
            }
        }

        Ok((
            CapabilityState {
                signature,
                contracts: contracts.finish(),
            },
            input,
        ))
    }

    fn parse_capability_contract<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        input: Input<'tokens, 'source>,
    ) -> ParseResult<'tokens, 'source, CapabilityContract> {
        if input.at_contextual("requires") {
            let keyword_source_span = Some(input.current_source_span());
            let input = input.take_contextual("requires")?;
            let ((facts, token_count), input) =
                parse_proof_facts_until(syntax_trees, input, capability_contract_terminator)?;
            let input = take_optional_semicolon(input)?;
            return Ok((
                CapabilityContract {
                    kind: CapabilityContractKind::Requires,
                    keyword_source_span,
                    binding: None,
                    facts,
                    token_count,
                },
                input,
            ));
        }

        if input.at_contextual("ensures") {
            let keyword_source_span = Some(input.current_source_span());
            let input = input.take_contextual("ensures")?;
            let ((facts, token_count), input) =
                parse_proof_facts_until(syntax_trees, input, capability_contract_terminator)?;
            let input = take_optional_semicolon(input)?;
            return Ok((
                CapabilityContract {
                    kind: CapabilityContractKind::Ensures,
                    keyword_source_span,
                    binding: None,
                    facts,
                    token_count,
                },
                input,
            ));
        }

        if input.at_contextual("boundary") {
            return Err(input.error_here(
            "trailing `boundary host` and `boundary Name` contract clauses are retired; use a leading `boundary trait`, `boundary machine`, or `boundary operator` declaration and realize exact requirements with `satisfies Trait::requirement via Binding::...`",
        ));
        }

        Err(input.error_here("expected capability contract"))
    }

    fn capability_contract_terminator(input: Input<'_, '_>) -> bool {
        input.at_punctuation(PunctuationKind::Semicolon)
            || input.at_punctuation(PunctuationKind::RightBrace)
            || input.at_keyword(KeywordKind::State)
            || input.at_contextual("entry")
            || input.at_contextual("requires")
            || input.at_contextual("ensures")
            || input.at_contextual("boundary")
            || input.tokens.is_empty()
    }
}
pub(crate) mod conformance {
    use crate::grammar::contracts::conformance::parse_conformance::parse_conformance_trait_application;
    use crate::grammar::declarations::parse_declaration::{ParsedDeclaration, parse_item};
    use crate::grammar::parameters::contracts::parse_machine_parameter_contracts;
    use crate::grammar::parameters::contracts::validate_machine_parameter_contracts;
    use crate::grammar::parameters::parse_generic_parameters::GenericParameterSyntax;
    use crate::grammar::parameters::parse_generic_parameters::parse_generic_parameters;
    use crate::tokens::token_cursor::{Input, ParseResult, parse_path_handle_span};
    use arena::HandleSpan;
    use syntax_trees::SyntaxTrees;
    use syntax_trees::item::{ConformanceMember, Item, Machine, State};
    use tokens::{KeywordKind, PunctuationKind};

    pub(crate) fn parse_conformance<'tokens, 'source>(
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
        let ((), rest) = parse_machine_parameter_contracts(
            syntax_trees,
            generic_parameters.type_parameters,
            rest,
        )?;
        validate_machine_parameter_contracts(
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
        let mut members = crate::storage::handle_spans::HandleSpanBuilder::new();

        while !input.at_punctuation(PunctuationKind::RightBrace) {
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
                // A selection row fills one slot with an existing machine. It
                // carries no visibility or boundary marker of its own: both
                // belong to the selected declaration.
                let Ok((declaring_trait, rest)) = input.take_identifier() else {
                    return Err(input.error_here(
                        "a named implementation member is an inline `machine` or a selection \
                         row `Trait::member = Existing::machine;`",
                    ));
                };
                let rest = rest.take_punctuation(PunctuationKind::ColonColon, "::")?;
                let (requirement, rest) = rest.take_identifier()?;
                let rest = rest.take_punctuation(PunctuationKind::Equal, "=")?;
                let (target, rest) = parse_path_handle_span(rest, |member| {
                    syntax_trees.items.append_identifier_path_member(member)
                })?;
                let rest = rest.take_punctuation(PunctuationKind::Semicolon, ";")?;
                let handle =
                    syntax_trees
                        .items
                        .append_conformance_member(ConformanceMember::Selection {
                            declaring_trait,
                            requirement,
                            target,
                        });
                members.push(handle);
                input = rest;
                continue;
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
            if syntax_trees
                .items
                .satisfies_clauses(machine.satisfies)
                .iter()
                .any(|edge| edge.requirement.is_none())
            {
                return Err(input.error_here(
                    "an inline member's satisfies clauses must name exact requirements",
                ));
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
}
pub(crate) mod const_item {
    use crate::grammar::expressions::parse_expression::parse_expression_handle;
    use crate::grammar::type_syntax::parse_type::parse_type_reference_handle;
    use crate::tokens::token_cursor::{Input, ParseResult};
    use syntax_trees::SyntaxTrees;
    use syntax_trees::identifier::Identifier;
    use syntax_trees::item::ConstDefinition;
    use tokens::PunctuationKind;

    pub(crate) fn parse_const_definition<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        input: Input<'tokens, 'source>,
    ) -> ParseResult<'tokens, 'source, ConstDefinition> {
        let (first, input) = input.take_identifier()?;
        let (scope, name, input) = if input.at_punctuation(PunctuationKind::ColonColon) {
            let input = input.take_punctuation(PunctuationKind::ColonColon, "::")?;
            let (name, input) = input.take_identifier()?;
            (first, name, input)
        } else {
            (Identifier::generated(""), first, input)
        };
        let input = input.take_punctuation(PunctuationKind::Colon, ":")?;
        let (type_reference, input) = parse_type_reference_handle(syntax_trees, input)?;
        let input = input.take_punctuation(PunctuationKind::Equal, "=")?;
        let (value, input) =
            parse_expression_handle(syntax_trees, input.with_constant_position(true))?;
        let input = input.with_constant_position(false);
        let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
        Ok((
            ConstDefinition {
                scope,
                name,
                is_public: false,
                type_reference,
                value,
                pending_target_observation: false,
                normalization: None,
                qualification: None,
                pending_qualification: None,
            },
            input,
        ))
    }
}
pub(crate) mod data {
    use crate::grammar::contracts::conformance::parse_conformance::parse_conformance_trait_application;
    use crate::grammar::contracts::facts::parse_required_logical_proof_facts_until;
    use crate::grammar::parameters::binding_properties::parse_binding_relevance_brackets;
    use crate::grammar::parameters::contracts::parse_machine_parameter_contracts;
    use crate::grammar::parameters::contracts::validate_machine_parameter_contracts;
    use crate::grammar::parameters::parse_generic_parameters::GenericParameterSyntax;
    use crate::grammar::parameters::parse_generic_parameters::parse_generic_parameters;
    use crate::grammar::type_syntax::parse_type::{
        parse_type_reference_handle, parse_type_reference_handle_allowing_borrow,
    };
    use crate::grammar::type_syntax::properties::parse_property_brackets;
    use crate::storage::tables::{items, items_of};
    use crate::tokens::token_cursor::{Input, ParseResult, parse_path_handle_span};
    use arena::HandleSpan;
    use std::collections::HashSet;
    use syntax_trees::SyntaxTrees;
    use syntax_trees::item::{
        DataDefinition, DataField, DataMember, DataVariant, Item, QuotientDefinition,
        QuotientEquivalenceSelection, TypeAliasDefinition,
    };
    use tokens::PunctuationKind;

    pub(crate) fn parse_data_definition<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        input: Input<'tokens, 'source>,
    ) -> ParseResult<'tokens, 'source, Item> {
        let (name, mut input) = input.take_identifier()?;
        if name.as_str(syntax_trees) == "Slice" {
            return Err(input.error_here(
                "data name `Slice` is reserved: `Slice<T>` is the slice type (alias of `[T]`)",
            ));
        }
        let (generic_parameters, next) =
            parse_generic_parameters(syntax_trees, input, GenericParameterSyntax::DataDeclaration)?;
        input = next;
        let type_parameters = generic_parameters.type_parameters;
        let lifetime_parameters = generic_parameters.lifetime_parameters;
        let (properties, next) = parse_property_brackets(input)?;
        input = next;
        let ((), next) = parse_machine_parameter_contracts(syntax_trees, type_parameters, input)?;
        validate_machine_parameter_contracts(syntax_trees, type_parameters, next)?;
        input = next;
        let (cleanup_selection, next) =
            super::domain::parse_cleanup_selection(syntax_trees, input)?;
        input = next;
        if input.at_punctuation(PunctuationKind::Equal) {
            input = input.take_punctuation(PunctuationKind::Equal, "=")?;
            let (carrier, next) = parse_type_reference_handle(syntax_trees, input)?;
            if next.at_punctuation(PunctuationKind::Semicolon) {
                if properties.multiplicity != language_core::Multiplicity::Affine
                    || properties.carry.is_some()
                    || cleanup_selection.is_some()
                {
                    return Err(next.error_here(
                    "a transparent type alias inherits its expansion's properties and cleanup; it cannot declare storage properties",
                ));
                }
                return Ok((
                    Item::TypeAlias(TypeAliasDefinition {
                        name,
                        is_public: false,
                        lifetime_parameters,
                        type_parameters,
                        conformance_bounds: generic_parameters.conformance_bounds,
                        authored_expansion: carrier,
                        expansion: carrier,
                    }),
                    next.take_punctuation(PunctuationKind::Semicolon, ";")?,
                ));
            }
            if properties.multiplicity == language_core::Multiplicity::Linear
                || properties.carry.is_some()
            {
                return Err(next.error_here(
                "a quotient data declaration cannot declare linear or carry properties; the initial quotient surface excludes custody",
            ));
            }
            input = next.take_punctuation(PunctuationKind::Percent, "%")?;
            let (relation, next) = parse_path_handle_span(input, |member| {
                items(syntax_trees).append_identifier_path_member(member)
            })?;
            input = next;
            let equivalence = if input.at_contextual("requires") {
                input = input.take_contextual("requires")?;
                let (selected_relation, next) = parse_path_handle_span(input, |member| {
                    items(syntax_trees).append_identifier_path_member(member)
                })?;
                input = next.take_contextual("satisfies")?;
                let ((trait_name, trait_lifetime_arguments, trait_arguments), next) =
                    parse_conformance_trait_application(syntax_trees, input)?;
                if !trait_lifetime_arguments.is_empty() {
                    return Err(next.error_here(
                    "quotient equivalence selection does not yet retain lifetime arguments; the application must remain lifetime-free",
                ));
                }
                input = next.take_contextual("as")?;
                let (conformance_name, next) = input.take_identifier()?;
                input = next;
                Some(QuotientEquivalenceSelection {
                    relation: selected_relation,
                    trait_name,
                    trait_arguments,
                    conformance_name,
                })
            } else {
                None
            };
            input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
            return Ok((
                Item::Data(DataDefinition {
                    name,
                    is_public: false,
                    supply_mode: language_core::DataSupplyMode::CheckedShape,
                    lifetime_parameters,
                    type_parameters,
                    conformance_bounds: generic_parameters.conformance_bounds,
                    generic_instance: None,
                    properties,
                    cleanup_selection,
                    quotient: Some(QuotientDefinition {
                        carrier,
                        relation,
                        equivalence,
                    }),
                    where_facts: HandleSpan::empty(),
                    members: HandleSpan::empty(),
                }),
                input,
            ));
        }
        let mut default_domain_facts = crate::storage::handle_spans::HandleSpanBuilder::new();
        while input.at_contextual("requires") {
            input = input.take_contextual("requires")?;
            let ((facts, _token_count), rest) =
                parse_required_logical_proof_facts_until(syntax_trees, input, |input| {
                    input.at_contextual("requires")
                        || input.at_punctuation(PunctuationKind::LeftBrace)
                        || input.tokens.is_empty()
                })?;
            default_domain_facts.push_span(facts);
            input = rest;
        }
        let where_facts = default_domain_facts.finish();
        input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;

        if input.at_integer() {
            return Err(input.error_here(
            "the legacy numbered-field spelling `N: name: Type;` is retired; write `#N name: Type;`",
        ));
        }

        let (members, input) = parse_data_members(syntax_trees, input)?;
        let input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;

        Ok((
            Item::Data(DataDefinition {
                name,
                is_public: false,
                supply_mode: language_core::DataSupplyMode::CheckedShape,
                lifetime_parameters,
                type_parameters,
                conformance_bounds: generic_parameters.conformance_bounds,
                generic_instance: None,
                properties,
                cleanup_selection,
                quotient: None,
                where_facts,
                members,
            }),
            input,
        ))
    }

    pub(crate) fn parse_boundary_data_definition<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        input: Input<'tokens, 'source>,
    ) -> ParseResult<'tokens, 'source, DataDefinition> {
        let (name, input) = input.take_identifier()?;
        if name.as_str(syntax_trees) == "Slice" {
            return Err(input.error_here(
                "data name `Slice` is reserved: `Slice<T>` is the slice type (alias of `[T]`)",
            ));
        }
        let (generic_parameters, input) =
            parse_generic_parameters(syntax_trees, input, GenericParameterSyntax::DataDeclaration)?;
        let type_parameters = generic_parameters.type_parameters;
        let lifetime_parameters = generic_parameters.lifetime_parameters;
        let (properties, input) = parse_property_brackets(input)?;
        let ((), input) = parse_machine_parameter_contracts(syntax_trees, type_parameters, input)?;
        validate_machine_parameter_contracts(syntax_trees, type_parameters, input)?;
        if input.at_contextual("requires") {
            return Err(input.error_here(
                "opaque `boundary data` has no visible fields for default-domain predicates",
            ));
        }
        let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
        Ok((
            DataDefinition {
                name,
                is_public: false,
                cleanup_selection: None,
                supply_mode: language_core::DataSupplyMode::BoundaryOpaque,
                lifetime_parameters,
                type_parameters,
                conformance_bounds: generic_parameters.conformance_bounds,
                generic_instance: None,
                properties,
                quotient: None,
                where_facts: HandleSpan::empty(),
                members: HandleSpan::empty(),
            },
            input,
        ))
    }

    fn parse_data_members<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        mut input: Input<'tokens, 'source>,
    ) -> ParseResult<'tokens, 'source, HandleSpan<DataMember>> {
        let mut members = crate::storage::handle_spans::HandleSpanBuilder::new();

        while !input.at_punctuation(PunctuationKind::RightBrace) {
            let (member, next) = parse_data_member(syntax_trees, input)?;
            input = next;
            members.push(items(syntax_trees).append_data_member(member));
        }

        let members = members.finish();
        validate_data_identity_modes(items_of(syntax_trees).data_members(members), input)?;
        Ok((members, input))
    }

    fn validate_data_identity_modes(
        members: &[DataMember],
        input: Input<'_, '_>,
    ) -> Result<(), crate::diagnostics::parse_error::ParseError> {
        let fields: Vec<Option<u64>> = members
            .iter()
            .filter_map(|member| match member {
                DataMember::Field(field) => Some(field.identity),
                _ => None,
            })
            .collect();
        let cases: Vec<Option<u64>> = members
            .iter()
            .filter_map(|member| match member {
                DataMember::Variant(case) => Some(case.identity),
                _ => None,
            })
            .collect();
        let retired: Vec<u64> = members
            .iter()
            .filter_map(|member| match member {
                DataMember::Retired(identity) => Some(*identity),
                _ => None,
            })
            .collect();

        if !retired.is_empty() && !fields.is_empty() && !cases.is_empty() {
            return Err(input.error_here(
                "`retired #N;` is ambiguous in mixed field-and-case data; publish separate record \
             and sum shapes so each retired identity has one structural scope",
            ));
        }
        validate_identity_scope(
            "record fields",
            &fields,
            if cases.is_empty() { &retired } else { &[] },
            input,
        )?;
        validate_identity_scope(
            "sum cases",
            &cases,
            if fields.is_empty() { &retired } else { &[] },
            input,
        )
    }

    fn validate_identity_scope(
        scope: &str,
        identities: &[Option<u64>],
        retired: &[u64],
        input: Input<'_, '_>,
    ) -> Result<(), crate::diagnostics::parse_error::ParseError> {
        let numbered = identities
            .iter()
            .filter(|identity| identity.is_some())
            .count();
        if numbered > 0 && numbered != identities.len() {
            return Err(input.error_here(format!(
                "stable identities are all-or-nothing for {scope}: number every member with `#N` \
             or number none of them"
            )));
        }
        if !retired.is_empty() && identities.iter().any(Option::is_none) {
            return Err(input.error_here(format!(
                "`retired #N;` enters numbered mode for {scope}: every live member also needs `#N`"
            )));
        }

        let mut seen = HashSet::new();
        for identity in identities
            .iter()
            .flatten()
            .copied()
            .chain(retired.iter().copied())
        {
            if !seen.insert(identity) {
                return Err(input.error_here(format!(
                "stable identity #{identity} is declared more than once among {scope} and their retired identities"
            )));
            }
        }
        Ok(())
    }

    fn take_optional_identity<'tokens, 'source>(
        input: Input<'tokens, 'source>,
    ) -> ParseResult<'tokens, 'source, Option<u64>> {
        if !input.at_punctuation(PunctuationKind::Hash) {
            return Ok((None, input));
        }
        let after_hash = input.take_punctuation(PunctuationKind::Hash, "#")?;
        let (identity, rest) = after_hash.take_identity()?;
        Ok((Some(identity), rest))
    }

    fn parse_data_member<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        mut input: Input<'tokens, 'source>,
    ) -> ParseResult<'tokens, 'source, DataMember> {
        if input.at_contextual("retired") {
            input = input.take_contextual("retired")?;
            input = input.take_punctuation(PunctuationKind::Hash, "#")?;
            let (identity, input) = input.take_identity()?;
            let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
            return Ok((DataMember::Retired(identity), input));
        }

        if input.at_contextual("reserved") {
            let after_reserved = input.take_contextual("reserved")?;
            if after_reserved.at_integer() {
                return Err(input.error_here(
                    "`reserved` is retired: tombstone an identity number with `retired #N;`",
                ));
            }
        }

        if input.at_contextual("version") {
            let after_version = input.take_contextual("version")?;
            if after_version.at_name_like() {
                let (_, after_name) = after_version.take_identifier()?;
                if after_name.at_punctuation(PunctuationKind::LeftBrace) {
                    return Err(input.error_here(
                    "data `version` blocks are retired; declare immutable era data types and an ordinary sum envelope",
                ));
                }
            }
        }

        let (identity, mut input) = take_optional_identity(input)?;
        if input.at_contextual("case") {
            let after_case = input.take_contextual("case")?;
            if after_case.at_punctuation(PunctuationKind::Hash) {
                return Err(after_case
                    .error_here("numbered cases are spelled `#N case Name`, not `case #N Name`"));
            }
            if after_case.at_punctuation(PunctuationKind::LeftBrace) {
                return Err(after_case.error_here(
                "case declaration blocks are not supported; declare each `case Name` separately",
            ));
            }
            if after_case.at_name_like() {
                return parse_case_member(syntax_trees, after_case, identity);
            }
        }

        let (field_name, next) = input.take_identifier()?;
        let (relevance, next) = parse_binding_relevance_brackets(next, "data-field")?;
        input = next;

        if input.at_punctuation(PunctuationKind::Colon) {
            input = input.take_punctuation(PunctuationKind::Colon, ":")?;
            let (type_reference, next) =
                parse_type_reference_handle_allowing_borrow(syntax_trees, input)?;
            input = next;
            if input.at_punctuation(PunctuationKind::Equal) {
                return Err(input.error_here(format!(
                    "data field `{}` declares a default initializer -- field defaults \
                 are retired: every field is zero-initialized (ZII), and a \
                 constructed default belongs in an ordinary constructor machine \
                 that writes the field",
                    field_name.as_str(syntax_trees)
                )));
            }
            input = if input.at_punctuation(PunctuationKind::Semicolon) {
                input.take_punctuation(PunctuationKind::Semicolon, ";")?
            } else {
                input
            };
            return Ok((
                DataMember::Field(DataField {
                    identity,
                    name: field_name,
                    relevance,
                    type_reference,
                }),
                input,
            ));
        }

        Err(input.error_here(format!(
            "expected `:` after data field `{}` (alternatives are spelled `case {};`)",
            field_name.as_str(syntax_trees),
            field_name.as_str(syntax_trees)
        )))
    }

    fn parse_case_member<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        input: Input<'tokens, 'source>,
        identity: Option<u64>,
    ) -> ParseResult<'tokens, 'source, DataMember> {
        let (case_name, mut input) = input.take_identifier()?;

        let (payload, retired_payload_identities) =
            if input.at_punctuation(PunctuationKind::LeftParen) {
                let (payload, next) = parse_case_payload_fields(syntax_trees, input)?;
                input = next;
                payload
            } else {
                (HandleSpan::empty(), Vec::new())
            };

        if input.at_contextual("requires") {
            return Err(input.error_here(
            "case-local predicate clauses are retired; write the predicate in the data declaration's default-domain `requires`",
        ));
        }
        let where_facts = HandleSpan::empty();

        input = if input.at_punctuation(PunctuationKind::Semicolon) {
            input.take_punctuation(PunctuationKind::Semicolon, ";")?
        } else {
            input
        };
        Ok((
            DataMember::Variant(DataVariant {
                identity,
                name: case_name,
                payload,
                where_facts,
                retired_payload_identities,
            }),
            input,
        ))
    }

    fn parse_case_payload_fields<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        input: Input<'tokens, 'source>,
    ) -> ParseResult<'tokens, 'source, (HandleSpan<DataField>, Vec<u64>)> {
        let mut input = input.take_punctuation(PunctuationKind::LeftParen, "(")?;
        let mut payload = crate::storage::handle_spans::HandleSpanBuilder::new();

        let mut retired_identities = Vec::new();
        while !input.at_punctuation(PunctuationKind::RightParen) {
            if input.at_contextual("retired") {
                input = input.take_contextual("retired")?;
                input = input.take_punctuation(PunctuationKind::Hash, "#")?;
                let (identity, next) = input.take_identity()?;
                retired_identities.push(identity);
                input = next;
                if input.at_punctuation(PunctuationKind::Comma) {
                    input = input.take_punctuation(PunctuationKind::Comma, ",")?;
                    continue;
                }
                break;
            }
            let (identity, after_identity) = take_optional_identity(input)?;
            let (field_name, next) = after_identity.take_identifier()?;
            let (relevance, next) = parse_binding_relevance_brackets(next, "data-field")?;
            input = next.take_punctuation(PunctuationKind::Colon, ":")?;
            let (type_reference, next) =
                parse_type_reference_handle_allowing_borrow(syntax_trees, input)?;
            input = next;

            let handle = items(syntax_trees).append_data_payload_field(DataField {
                identity,
                name: field_name,
                relevance,
                type_reference,
            });
            payload.push(handle);

            if input.at_punctuation(PunctuationKind::Comma) {
                input = input.take_punctuation(PunctuationKind::Comma, ",")?;
            } else {
                break;
            }
        }

        let input = input.take_punctuation(PunctuationKind::RightParen, ")")?;
        let payload = payload.finish();
        let identities: Vec<Option<u64>> = syntax_trees
            .items
            .data_payload_fields(payload)
            .iter()
            .map(|field| field.identity)
            .collect();
        validate_identity_scope(
            "structured case-payload fields",
            &identities,
            &retired_identities,
            input,
        )?;
        Ok(((payload, retired_identities), input))
    }
}
pub(crate) mod domain {
    use crate::grammar::contracts::facts::parse_required_logical_proof_facts_until;
    use crate::grammar::expressions::parse_postfix::try_parse_static_symbol_application;
    use crate::grammar::parameters::parse_generic_parameters::GenericParameterSyntax;
    use crate::grammar::parameters::parse_generic_parameters::parse_generic_parameters;
    use crate::grammar::type_syntax::parse_type::parse_domain_argument_handles;
    use crate::grammar::type_syntax::parse_type::parse_named_type_reference;
    use crate::storage::tables::{exprs, items, type_refs_of};
    use crate::tokens::token_cursor::{Input, ParseResult, parse_path_handle_span};
    use arena::HandleSpan;
    use language_core::DomainPredicatePresence;
    use syntax_trees::SyntaxTrees;
    use syntax_trees::expression::ExpressionNode;
    use syntax_trees::identifier::Identifier;
    use syntax_trees::item::{
        DomainAliasDefinition, DomainDefinition, ProofFact, ProofMembershipFact,
    };
    use syntax_trees::types::TypeReferenceHandle;
    use syntax_trees::types::TypeReferenceNode;
    use tokens::{KeywordKind, PunctuationKind};

    pub(crate) fn parse_domain_definition<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        input: Input<'tokens, 'source>,
    ) -> ParseResult<'tokens, 'source, DomainDefinition> {
        let (generic_parameters, input) =
            parse_generic_parameters(syntax_trees, input, GenericParameterSyntax::TypeAndConst)?;
        if !generic_parameters.lifetime_parameters.is_empty() {
            return Err(input.error_here(
            "domain families take a carrier type and proof-static const parameters, not lifetime parameters",
        ));
        }
        let (target_type, target_label, target_identifier, input) =
            if input.at_punctuation(PunctuationKind::LeftBracket) {
                let (handle, input) =
                    crate::grammar::type_syntax::parse_type::parse_type_reference_handle(
                        syntax_trees,
                        input,
                    )?;
                let label = type_reference_target_label(syntax_trees, handle);
                (handle, label, None, input)
            } else {
                let (target_name, input) = input.take_identifier()?;
                let (handle, input) =
                    parse_named_type_reference(syntax_trees, target_name.clone(), input, false)?;
                (
                    handle,
                    target_name.as_str(syntax_trees).to_owned(),
                    Some(target_name),
                    input,
                )
            };
        let input = input.take_punctuation(PunctuationKind::ColonColon, "::")?;
        let (first_segment, mut input) = input.take_identifier()?;
        let (first_arguments, rest) = parse_domain_argument_handles(syntax_trees, input)?;
        let mut name_segments = vec![(first_segment, first_arguments)];
        input = rest;
        while input.at_punctuation(PunctuationKind::ColonColon) {
            let rest = input.take_punctuation(PunctuationKind::ColonColon, "::")?;
            let (segment, rest) = rest.take_identifier()?;
            let (arguments, rest) = parse_domain_argument_handles(syntax_trees, rest)?;
            name_segments.push((segment, arguments));
            input = rest;
        }
        let domain_name = name_segments
            .last()
            .expect("a domain chain always holds its first segment")
            .0
            .clone();
        let index_arguments = name_segments
            .last()
            .expect("a domain chain always holds its first segment")
            .1;
        if input.at_punctuation(PunctuationKind::LeftParen) {
            return Err(input.error_here(
            "explicit domain subject binders are retired; use `self` in the domain's `requires` predicates",
        ));
        }
        let ((classification, classification_token_count), input) =
            parse_domain_classification(input)?;
        let (cleanup_selection, input) = parse_cleanup_selection(syntax_trees, input)?;
        let refinement_parent = if name_segments.len() > 1 {
            let parent_span = name_segments[name_segments.len() - 2].0.source_span();
            let subject_path = HandleSpan::from_parts(
                exprs(syntax_trees).append_identifier_path_member(Identifier::generated("self")),
                1,
            );
            let parent_path = items(syntax_trees).insert_identifier_path_members(
                target_identifier.iter().cloned().chain(
                    name_segments[..name_segments.len() - 1]
                        .iter()
                        .map(|(segment, _)| segment.clone()),
                ),
            );
            Some((
                ProofMembershipFact {
                    value: syntax_trees
                        .expressions
                        .insert(ExpressionNode::Name(subject_path)),
                    domain: parent_path,
                    domain_arguments: name_segments[name_segments.len() - 2].1,
                },
                parent_span,
            ))
        } else {
            None
        };
        let name = if generic_parameters.type_parameters.is_empty() {
            Identifier::new(
                format!(
                    "{target_label}::{}",
                    name_segments
                        .iter()
                        .map(|(segment, _)| segment.as_str(syntax_trees))
                        .collect::<Vec<_>>()
                        .join("::")
                ),
                domain_name.source_span(),
            )
        } else {
            Identifier::new(
                name_segments
                    .iter()
                    .map(|(segment, _)| segment.as_str(syntax_trees))
                    .collect::<Vec<_>>()
                    .join("::"),
                domain_name.source_span(),
            )
        };
        let (
            alias,
            authored_routes,
            predicate_presence,
            facts,
            operators,
            semantic_clause_token_count,
            input,
        ) = if input.at_punctuation(PunctuationKind::Equal) {
            if refinement_parent.is_some() {
                return Err(input.error_here(
                "a domain refinement chain cannot be an alias; write the parent membership in the alias explicitly",
            ));
            }
            let (alias, input) = parse_domain_alias(syntax_trees, input)?;
            (
                Some(alias),
                Vec::new(),
                DomainPredicatePresence::Absent,
                HandleSpan::empty(),
                HandleSpan::empty(),
                0,
                input,
            )
        } else {
            let parent_fact = refinement_parent.map(|(membership, parent_span)| {
                let handle = syntax_trees
                    .items
                    .append_proof_fact(ProofFact::Membership(membership));
                items(syntax_trees).set_proof_fact_source_span(handle, parent_span);
                handle
            });
            let ((predicate_presence, authored_facts, requires_token_count), input) =
                parse_domain_requires(syntax_trees, input)?;
            let (predicate_presence, facts) = match parent_fact {
                Some(handle) => (
                    DomainPredicatePresence::Present,
                    HandleSpan::from_parts(handle, 1 + authored_facts.count()),
                ),
                None => (predicate_presence, authored_facts),
            };
            let ((authored_routes, semantic_clause_token_count), input) =
                parse_domain_establishment(input, predicate_presence.is_present())?;
            (
                None,
                authored_routes,
                predicate_presence,
                facts,
                HandleSpan::empty(),
                requires_token_count.saturating_add(semantic_clause_token_count),
                input,
            )
        };

        Ok((
            DomainDefinition {
                name,
                type_parameters: generic_parameters.type_parameters,
                target_type,
                index_arguments,
                is_public: false,
                alias,
                authored_routes,
                cleanup_selection,
                classification,
                predicate_presence,
                facts,
                operators,
                semantic_clause_token_count: classification_token_count
                    .saturating_add(semantic_clause_token_count),
            },
            input,
        ))
    }

    fn parse_domain_classification<'tokens, 'source>(
        input: Input<'tokens, 'source>,
    ) -> ParseResult<'tokens, 'source, (Option<language_core::DomainClassification>, usize)> {
        if !input.at_contextual("satisfies") {
            return Ok(((None, 0), input));
        }
        let start_tokens = input.tokens.len();
        let input = input.take_contextual("satisfies")?;
        let (name, input) = input.take_identifier()?;
        let classification = match name.as_str(&input) {
            "ProgressProfile" => language_core::DomainClassification::ProgressProfile,
            other => {
                return Err(input.error_here(format!(
                "unknown compiler-owned domain classification `{other}`; expected `ProgressProfile`"
            )));
            }
        };
        if input.at_contextual("satisfies") {
            return Err(
                input.error_here("a domain may select at most one compiler-owned classification")
            );
        }
        Ok((
            (
                Some(classification),
                start_tokens.saturating_sub(input.tokens.len()),
            ),
            input,
        ))
    }

    fn parse_domain_requires<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        input: Input<'tokens, 'source>,
    ) -> ParseResult<'tokens, 'source, (DomainPredicatePresence, HandleSpan<ProofFact>, usize)>
    {
        if !input.at_contextual("requires") {
            return Ok((
                (DomainPredicatePresence::Absent, HandleSpan::empty(), 0),
                input,
            ));
        }

        let input = input.take_contextual("requires")?;
        let ((facts, token_count), input) = parse_required_logical_proof_facts_until(
            syntax_trees,
            input,
            domain_requires_terminator,
        )?;
        Ok((
            (DomainPredicatePresence::Present, facts, token_count),
            input,
        ))
    }

    fn domain_requires_terminator(input: Input<'_, '_>) -> bool {
        super::parse_declaration::named_implementation_prefix(input).is_some()
            || input.at_contextual("established")
            || input.at_contextual("satisfies")
            || input.at_punctuation(PunctuationKind::LeftBrace)
            || input.tokens.is_empty()
            || input.at_keyword(KeywordKind::Public)
            || input.at_keyword(KeywordKind::Data)
            || input.at_keyword(KeywordKind::Machine)
            || super::parse_declaration::starts_machine_declaration(input)
            || input.at_keyword(KeywordKind::Use)
            || input.at_contextual("domain")
            || input.at_contextual("operator")
            || input.at_contextual("boundary")
            || input.at_contextual("trait")
            || input.at_contextual("const")
            || input.at_contextual("export")
            || input.at_contextual("module")
            || input.at_contextual("package")
    }

    fn parse_domain_alias<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        input: Input<'tokens, 'source>,
    ) -> ParseResult<'tokens, 'source, DomainAliasDefinition> {
        let mut input = input.take_punctuation(PunctuationKind::Equal, "=")?;
        let mut constituents = Vec::new();

        loop {
            let (domain, rest) = parse_path_handle_span(input, |member| {
                items(syntax_trees).append_identifier_path_member(member)
            })?;
            constituents.push(domain);
            if !rest.at_punctuation(PunctuationKind::Ampersand) {
                input = rest;
                break;
            }
            input = rest.take_punctuation(PunctuationKind::Ampersand, "&")?;
        }

        let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
        Ok((DomainAliasDefinition { constituents }, input))
    }

    fn type_reference_target_label(
        syntax_trees: &SyntaxTrees,
        handle: TypeReferenceHandle,
    ) -> String {
        match type_refs_of(syntax_trees).type_reference(handle) {
            TypeReferenceNode::Slice { element_type } => {
                format!(
                    "[{}]",
                    type_reference_target_label(syntax_trees, *element_type)
                )
            }
            TypeReferenceNode::FixedArray { element_type, .. } => {
                format!(
                    "[{}; N]",
                    type_reference_target_label(syntax_trees, *element_type)
                )
            }
            TypeReferenceNode::Named(name) => name.as_str(syntax_trees).to_owned(),
            _ => "?".to_owned(),
        }
    }

    fn parse_domain_establishment<'tokens, 'source>(
        input: Input<'tokens, 'source>,
        requires_consumed: bool,
    ) -> ParseResult<'tokens, 'source, (Vec<Vec<Identifier>>, usize)> {
        if input.at_punctuation(PunctuationKind::Semicolon) {
            let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
            return Ok(((Vec::new(), 0), input));
        }

        if input.at_contextual("satisfies") {
            return Err(input.error_here(
            "a domain classification must appear immediately after the domain head, before `requires`",
        ));
        }

        if requires_consumed
            && !input.at_contextual("established")
            && !input.at_punctuation(PunctuationKind::LeftBrace)
            && domain_requires_terminator(input)
        {
            return Ok(((Vec::new(), 0), input));
        }

        if input.at_punctuation(PunctuationKind::LeftBrace) {
            let body = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
            if body.at_punctuation(PunctuationKind::RightBrace) {
                return Err(input
                    .error_here("empty domain bodies are retired; end the declaration with `;`"));
            }
            if at_legacy_authored_route(body) {
                return Err(input.error_here(
                    "domain route bodies are retired; write \
                 `established by Trait::requirement, ...;` after the domain predicates",
                ));
            }
            if body.at_contextual("operator") || body.at_contextual("boundary") {
                return Err(body.error_here(
                    "domain operators must be ordinary top-level declarations; write \
                 `operator Type::Domain::operation ...` outside the domain declaration",
                ));
            }
            return Err(body.error_here(
                "domain predicates must be written in `requires`; establishment routes use \
             `established by Trait::requirement, ...;`",
            ));
        }

        if !input.at_contextual("established") {
            return Err(input.error_here(
                "expected `;` to end the domain declaration or an `established by` clause",
            ));
        }
        let mut input = input.take_contextual("established")?;
        input = input.take_contextual("by")?;
        let routes_start_tokens = input.tokens.len();
        let mut authored_routes = Vec::new();

        loop {
            let (route, rest) = parse_authored_route(input)?;
            authored_routes.push(route);
            input = rest;
            if !input.at_punctuation(PunctuationKind::Comma) {
                break;
            }
            input = input.take_punctuation(PunctuationKind::Comma, ",")?;
        }

        input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
        let token_count = routes_start_tokens.saturating_sub(input.tokens.len());
        Ok(((authored_routes, token_count), input))
    }

    fn at_legacy_authored_route(mut input: Input<'_, '_>) -> bool {
        let Ok((_, rest)) = input.take_identifier() else {
            return false;
        };
        input = rest;
        let mut members = 1usize;
        while input.at_punctuation(PunctuationKind::ColonColon) {
            let Ok(rest) = input.take_punctuation(PunctuationKind::ColonColon, "::") else {
                return false;
            };
            let Ok((_, rest)) = rest.take_identifier() else {
                return false;
            };
            input = rest;
            members += 1;
        }
        members >= 2 && input.at_punctuation(PunctuationKind::Semicolon)
    }

    fn parse_authored_route<'tokens, 'source>(
        mut input: Input<'tokens, 'source>,
    ) -> ParseResult<'tokens, 'source, Vec<Identifier>> {
        let mut route = Vec::new();
        loop {
            let (member, rest) = input.take_identifier()?;
            route.push(member);
            input = rest;
            if !input.at_punctuation(PunctuationKind::ColonColon) {
                break;
            }
            input = input.take_punctuation(PunctuationKind::ColonColon, "::")?;
        }
        Ok((route, input))
    }

    pub(crate) fn parse_cleanup_selection<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        input: Input<'tokens, 'source>,
    ) -> ParseResult<'tokens, 'source, Option<syntax_trees::expression::StaticMachineArgument>>
    {
        if !input.at_contextual("dropped") {
            return Ok((None, input));
        }
        let input = input.take_contextual("dropped")?.take_contextual("by")?;
        let (path, input) = parse_authored_route(input)?;
        let (application, input) = match try_parse_static_symbol_application(syntax_trees, input)? {
            Some((application, input)) => (Some(application), input),
            None if input.at_punctuation(PunctuationKind::Less) => {
                return Err(
                    input.error_here("dropped by requires a complete static machine application")
                );
            }
            None => (None, input),
        };
        Ok((
            Some(syntax_trees::expression::StaticMachineArgument {
                type_reference: Default::default(),
                path: path.into_boxed_slice(),
                application,
                const_literal: None,
                evidence_projection: None,
            }),
            input,
        ))
    }
}
pub(crate) mod let_definition {
    use crate::grammar::expressions::parse_expression::parse_expression_handle;
    use crate::grammar::parameters::binding_properties::parse_binding_relevance_brackets;
    use crate::grammar::parameters::parse_generic_parameters::{
        GenericParameterSyntax, parse_generic_parameters,
    };
    use crate::grammar::type_syntax::parse_type::parse_type_reference_handle;
    use crate::tokens::token_cursor::{Input, ParseResult};
    use syntax_trees::SyntaxTrees;
    use syntax_trees::identifier::Identifier;
    use syntax_trees::item::{
        MathematicalDefinition, MathematicalDefinitionBody, MathematicalParameterNode,
        MathematicalTypeHandle, MathematicalTypeNode,
    };
    use tokens::PunctuationKind;

    pub(crate) fn parse_let_definition<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        input: Input<'tokens, 'source>,
        boundary: bool,
    ) -> ParseResult<'tokens, 'source, MathematicalDefinition> {
        let (name, input) = input.take_identifier()?;
        let (generics, input) = parse_generic_parameters(
            syntax_trees,
            input,
            GenericParameterSyntax::MathematicalDefinition,
        )?;
        if !generics.lifetime_parameters.is_empty() {
            return Err(input.error_here(
                "a mathematical `let` takes no lifetime parameters; universe binders are \
             value-shaped `u: core::Level` entries in the same telescope",
            ));
        }
        if !generics.conformance_bounds.is_empty() {
            return Err(input.error_here(
                "a mathematical `let` takes no `satisfies` conformance binders; parameterize \
             over carriers with `requires` facts on the citing machine instead",
            ));
        }

        let input = input.take_punctuation(PunctuationKind::LeftParen, "(")?;
        let mut parameters = crate::storage::handle_spans::HandleSpanBuilder::new();
        let mut input = input;
        if !input.at_punctuation(PunctuationKind::RightParen) {
            loop {
                let (parameter, rest) = parse_mathematical_parameter(syntax_trees, input)?;
                let handle = syntax_trees.items.insert_mathematical_parameter(parameter);
                let handle = syntax_trees
                    .items
                    .append_mathematical_parameter_handle(handle);
                parameters.push(handle);
                input = rest;
                let Some(next) = input.take_comma()? else {
                    break;
                };
                input = next;
            }
        }
        let input = input.take_punctuation(PunctuationKind::RightParen, ")")?;
        let parameters = parameters.finish();

        let input = input.take_punctuation(PunctuationKind::Colon, ":")?;
        let (result, input) = parse_mathematical_type(syntax_trees, input)?;

        if boundary {
            let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
            return Ok((
                MathematicalDefinition {
                    name,
                    is_public: false,
                    type_parameters: generics.type_parameters,
                    parameters,
                    result,
                    body: MathematicalDefinitionBody::Assumption,
                },
                input,
            ));
        }

        if input.at_punctuation(PunctuationKind::Semicolon) {
            return Err(input.error_here(
                "a top-level `let` needs a body: `= term;`. The bodyless assumption form \
             is spelled `boundary let`",
            ));
        }
        let input = input.take_punctuation(PunctuationKind::Equal, "=")?;
        let (term, input) = parse_expression_handle(syntax_trees, input)?;
        let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
        Ok((
            MathematicalDefinition {
                name,
                is_public: false,
                type_parameters: generics.type_parameters,
                parameters,
                result,
                body: MathematicalDefinitionBody::Definition(term),
            },
            input,
        ))
    }

    fn parse_mathematical_parameter<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        input: Input<'tokens, 'source>,
    ) -> ParseResult<'tokens, 'source, MathematicalParameterNode> {
        let (name, input) = input.take_identifier()?;
        let (relevance, input) = parse_binding_relevance_brackets(input, "parameter")?;
        let input = input.take_punctuation(PunctuationKind::Colon, ":")?;
        let (ty, input) = parse_mathematical_type(syntax_trees, input)?;
        Ok((
            MathematicalParameterNode {
                name,
                relevance,
                ty,
            },
            input,
        ))
    }

    fn parse_mathematical_type<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        input: Input<'tokens, 'source>,
    ) -> ParseResult<'tokens, 'source, MathematicalTypeHandle> {
        let outer_depth = input.depth();
        let input = input.deepen()?;
        let (handle, rest) = parse_mathematical_type_inner(syntax_trees, input)?;
        Ok((handle, rest.with_depth(outer_depth)))
    }

    fn parse_mathematical_type_inner<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        input: Input<'tokens, 'source>,
    ) -> ParseResult<'tokens, 'source, MathematicalTypeHandle> {
        let ((binder, domain), input) = parse_arrow_domain(syntax_trees, input)?;
        if input.at_punctuation(PunctuationKind::Arrow) {
            let input = input.take_punctuation(PunctuationKind::Arrow, "->")?;
            let (codomain, rest) = parse_mathematical_type(syntax_trees, input)?;
            let handle = syntax_trees
                .items
                .insert_mathematical_type(MathematicalTypeNode::Arrow {
                    binder,
                    domain,
                    codomain,
                });
            return Ok((handle, rest));
        }
        if binder.is_some() {
            return Err(input.error_here("a named domain binder `(x: T)` must be followed by `->`"));
        }
        Ok((domain, input))
    }

    fn parse_arrow_domain<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        input: Input<'tokens, 'source>,
    ) -> ParseResult<'tokens, 'source, (Option<Identifier>, MathematicalTypeHandle)> {
        if !input.at_punctuation(PunctuationKind::LeftParen) {
            let (type_reference, rest) = parse_type_reference_handle(syntax_trees, input)?;
            let handle = syntax_trees
                .items
                .insert_mathematical_type(MathematicalTypeNode::Ordinary(type_reference));
            let (domain, rest) = fold_type_applications(syntax_trees, handle, rest)?;
            return Ok(((None, domain), rest));
        }

        let after_paren = input.take_punctuation(PunctuationKind::LeftParen, "(")?;
        if after_paren.at_punctuation(PunctuationKind::RightParen) {
            let (type_reference, rest) = parse_type_reference_handle(syntax_trees, input)?;
            let handle = syntax_trees
                .items
                .insert_mathematical_type(MathematicalTypeNode::Ordinary(type_reference));
            let (domain, rest) = fold_type_applications(syntax_trees, handle, rest)?;
            return Ok(((None, domain), rest));
        }

        if let Ok((binder, after_name)) = after_paren.take_identifier()
            && after_name.at_punctuation(PunctuationKind::Colon)
        {
            let after_colon = after_name.take_punctuation(PunctuationKind::Colon, ":")?;
            let (domain, after_domain) = parse_mathematical_type(syntax_trees, after_colon)?;
            if after_domain.at_punctuation(PunctuationKind::Comma) {
                return Err(after_domain.error_here(
                    "a dependent binder names one domain type; curry further parameters \
                 through nested `->` domains, not a comma list",
                ));
            }
            let rest = after_domain.take_punctuation(PunctuationKind::RightParen, ")")?;
            return Ok(((Some(binder), domain), rest));
        }

        let (inner, after_inner) = parse_mathematical_type(syntax_trees, after_paren)?;
        let rest = after_inner.take_punctuation(PunctuationKind::RightParen, ")")?;
        let (domain, rest) = fold_type_applications(syntax_trees, inner, rest)?;
        Ok(((None, domain), rest))
    }

    fn fold_type_applications<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        mut domain: MathematicalTypeHandle,
        mut input: Input<'tokens, 'source>,
    ) -> ParseResult<'tokens, 'source, MathematicalTypeHandle> {
        while input.at_punctuation(PunctuationKind::LeftParen) {
            let mut rest = input.take_punctuation(PunctuationKind::LeftParen, "(")?;
            let mut arguments = Vec::new();
            if !rest.at_punctuation(PunctuationKind::RightParen) {
                loop {
                    let (argument, after) = parse_expression_handle(syntax_trees, rest)?;
                    arguments.push(argument);
                    rest = after;
                    if rest.at_punctuation(PunctuationKind::Comma) {
                        rest = rest.take_punctuation(PunctuationKind::Comma, ",")?;
                        continue;
                    }
                    break;
                }
            }
            let rest = rest.take_punctuation(PunctuationKind::RightParen, ")")?;
            let arguments = syntax_trees
                .expressions
                .insert_expression_handles(arguments);
            domain =
                syntax_trees
                    .items
                    .insert_mathematical_type(MathematicalTypeNode::Application {
                        callee: domain,
                        arguments,
                    });
            input = rest;
        }
        Ok((domain, input))
    }
}
pub(crate) mod machines {
    use crate::grammar::bodies as body;
    use crate::grammar::declarations::operator::parse_operator_spelling;
    use crate::grammar::parameters::parse_generic_parameters::GenericParameterSyntax;
    use crate::grammar::parameters::parse_generic_parameters::parse_generic_parameters;
    use crate::grammar::parameters::parse_parameters::{
        parse_optional_parameters, parse_optional_return_type,
    };
    use crate::storage::tables::{exprs, exprs_of};
    use crate::tokens::paths::join_path_identifier;
    use crate::tokens::token_cursor::{Input, ParseResult, parse_path_handle_span};
    use arena::HandleSpan;
    use syntax_trees::SyntaxTrees;
    use syntax_trees::identifier::Identifier;
    use syntax_trees::item::Machine;
    use tokens::PunctuationKind;

    use crate::grammar::contracts::conformance::parse_conformance::parse_satisfies_traits;
    use crate::grammar::contracts::parse_contract_clauses::parse_machine_clauses;
    use crate::grammar::parameters::contracts::parse_machine_parameter_contracts;
    use crate::grammar::parameters::contracts::validate_machine_parameter_contracts;

    pub(crate) fn parse_machine<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        input: Input<'tokens, 'source>,
    ) -> ParseResult<'tokens, 'source, Machine> {
        let (spelling, input) = if input.tokens.first().is_some_and(|token| {
            token.punctuation().is_some() && token.punctuation() != Some(PunctuationKind::Semicolon)
        }) {
            let (spelling, input) = parse_operator_spelling(input)?;
            (Some(spelling), input)
        } else {
            (None, input)
        };
        let (path, input) = parse_path_handle_span(input, |member| {
            exprs(syntax_trees).append_identifier_path_member(member)
        })?;
        let (generic_parameters, input) = parse_generic_parameters(
            syntax_trees,
            input,
            GenericParameterSyntax::MachineDeclaration,
        )?;
        let type_parameters = generic_parameters.type_parameters;
        let lifetime_parameters = generic_parameters.lifetime_parameters;
        let (machine_parameters, input) = parse_optional_parameters(syntax_trees, input)?;
        let (machine_return_type, input) = parse_optional_return_type(syntax_trees, input)?;
        let ((), mut input) =
            parse_machine_parameter_contracts(syntax_trees, type_parameters, input)?;
        let (satisfies, next) = parse_satisfies_traits(syntax_trees, input)?;
        if spelling.is_some() && !satisfies.is_empty() {
            return Err(next.error_here(
                "a machine with a `satisfies` clause is a realization and cannot declare an \
             operator token; the satisfied requirement owns the token binding",
            ));
        }
        if next.at_contextual("via") {
            return Err(next.error_here(
                "`via <Binding>` cannot supply a machine by itself; name the requirement first as \
             `satisfies Trait::machine via Binding::Case(...)`",
            ));
        }
        let (clauses, next) = parse_machine_clauses(syntax_trees, next, type_parameters)?;
        validate_machine_parameter_contracts(syntax_trees, type_parameters, next)?;
        let conformance_bounds = generic_parameters.conformance_bounds;
        input = next;
        let machine_return_type = if machine_return_type.is_valid() {
            machine_return_type
        } else {
            clauses.return_type
        };
        let MachinePath {
            name,
            attached_data,
            entry_name,
        } = split_machine_path(syntax_trees, path);

        let (
            body::parse_body::ParsedMachineBody {
                states,
                nested_machines,
                bodyless,
            },
            input,
        ) = body::parse_body::parse_body(
            syntax_trees,
            input,
            entry_name.clone(),
            machine_parameters,
            machine_return_type,
        )?;
        if clauses.admitted_same_stack_bytes != 0 {
            let span = clauses
                .same_stack_clause_span
                .expect("a positive stack demand records its clause span");
            if !bodyless {
                return Err(crate::diagnostics::parse_error::ParseError::at_source_span(
                    "`stack N bytes` admits demand only on a bodyless external realization",
                    span,
                ));
            }
            let has_external_binding = syntax_trees
                .items
                .satisfies_clauses(satisfies)
                .iter()
                .any(|clause| clause.via.is_some() || clause.via_expression.is_valid());
            if !has_external_binding {
                return Err(crate::diagnostics::parse_error::ParseError::at_source_span(
                    "`stack N bytes` requires a `satisfies ... via ...` binding",
                    span,
                ));
            }
        }
        Ok((
            Machine {
                target_family_interface: false,
                name,
                generic_data_template: Default::default(),
                default_body_trait: Default::default(),
                attached_data,
                spelling,
                is_public: false,
                target: None,
                authored_target: None,
                boundary: false,
                bodyless,
                lifetime_parameters,
                type_parameters,
                satisfies,
                conformance_bounds,
                terminates_guarantee: clauses.terminates_guarantee,
                ranking_subjects: clauses.ranking_subjects,
                ranking_view: clauses.ranking_view,
                ranking_view_arguments: clauses.ranking_view_arguments,
                ranking_range: clauses.ranking_range,
                service_reach_keyword_source_spans: clauses.service_reach_keyword_source_spans,
                service_reach_is_installation_bound: clauses.service_reach_is_installation_bound,
                service_reaches: clauses.service_reaches,
                invokes: clauses.invokes,
                suspends_keyword_source_spans: clauses.suspends_keyword_source_spans,
                blocks_keyword_source_spans: clauses.blocks_keyword_source_spans,
                suspends: clauses.suspends,
                blocks: clauses.blocks,
                admitted_same_stack_bytes: clauses.admitted_same_stack_bytes,
                contracts: clauses.contracts,
                states,
                nested_machines,
            },
            input,
        ))
    }

    struct MachinePath {
        name: Identifier,
        attached_data: Option<Identifier>,
        entry_name: Option<Identifier>,
    }

    fn split_machine_path(syntax_trees: &SyntaxTrees, path: HandleSpan<Identifier>) -> MachinePath {
        let members = exprs_of(syntax_trees).identifier_path_members(path);

        if members.len() <= 1 {
            return MachinePath {
                name: members
                    .first()
                    .cloned()
                    .expect("machine path should contain a name"),
                attached_data: None,
                entry_name: None,
            };
        }

        let name = join_path_identifier(syntax_trees, members);
        let attached_data = join_path_identifier(syntax_trees, &members[..members.len() - 1]);

        MachinePath {
            name,
            attached_data: Some(attached_data),
            entry_name: members.last().cloned(),
        }
    }
}
pub(crate) mod measure {
    use crate::grammar::expressions::parse_expression::parse_expression_handle;
    use crate::grammar::parameters::parse_parameters::{
        parse_optional_parameters, parse_optional_return_type,
    };
    use crate::tokens::token_cursor::{Input, ParseResult, parse_path_handle_span};
    use arena::HandleSpan;
    use syntax_trees::SyntaxTrees;
    use syntax_trees::expression::ExpressionHandle;
    use syntax_trees::item::{MeasureDefinition, StateParameterHandle};
    use tokens::PunctuationKind;

    pub(crate) fn parse_measure_definition<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        input: Input<'tokens, 'source>,
    ) -> ParseResult<'tokens, 'source, MeasureDefinition> {
        let body_start_tokens = input.tokens.len();
        let (name, input) = parse_path_handle_span(input, |member| {
            syntax_trees.items.append_identifier_path_member(member)
        })?;

        if input.at_contextual("lexicographic") {
            let input = input.take_contextual("lexicographic")?;
            let (components, input) = parse_lexicographic_components(syntax_trees, input)?;
            let token_count = body_start_tokens.saturating_sub(input.tokens.len());
            return Ok((
                MeasureDefinition {
                    name,
                    parameter: StateParameterHandle::invalid(),
                    return_type: syntax_trees::types::TypeReferenceHandle::invalid(),
                    lexicographic: true,
                    body: components,
                    token_count,
                },
                input,
            ));
        }

        let (parameters, input) = parse_optional_parameters(syntax_trees, input)?;
        let parameter_handles = syntax_trees.items.state_parameters(parameters);
        let parameter = if parameter_handles.len() == 1 {
            parameter_handles[0]
        } else {
            StateParameterHandle::invalid()
        };
        let (return_type, input) = parse_optional_return_type(syntax_trees, input)?;
        let (body, input) = parse_body_block(syntax_trees, input)?;
        let token_count = body_start_tokens.saturating_sub(input.tokens.len());

        Ok((
            MeasureDefinition {
                name,
                parameter,
                return_type,
                lexicographic: false,
                body,
                token_count,
            },
            input,
        ))
    }

    fn parse_body_block<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        input: Input<'tokens, 'source>,
    ) -> ParseResult<'tokens, 'source, HandleSpan<ExpressionHandle>> {
        let input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
        let (expression, input) = parse_expression_handle(syntax_trees, input)?;
        let input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
        let body = syntax_trees
            .expressions
            .insert_expression_handles([expression]);
        Ok((body, input))
    }

    fn parse_lexicographic_components<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        input: Input<'tokens, 'source>,
    ) -> ParseResult<'tokens, 'source, HandleSpan<ExpressionHandle>> {
        let mut input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
        let mut components = Vec::new();

        if !input.at_punctuation(PunctuationKind::RightBrace) {
            loop {
                let (expression, rest) = parse_expression_handle(syntax_trees, input)?;
                components.push(expression);
                input = rest;

                if input.at_punctuation(PunctuationKind::Comma) {
                    input = input.take_punctuation(PunctuationKind::Comma, ",")?;
                    if input.at_punctuation(PunctuationKind::RightBrace) {
                        break;
                    }
                    continue;
                }

                break;
            }
        }

        let input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
        let body = syntax_trees
            .expressions
            .insert_expression_handles(components);
        Ok((body, input))
    }
}
pub(crate) mod namespace {
    use crate::tokens::token_cursor::{Input, ParseResult};
    use syntax_trees::SyntaxTrees;
    use syntax_trees::identifier::Identifier;
    use syntax_trees::item::{ModuleDeclaration, PackageDeclaration};
    use tokens::PunctuationKind;

    pub(crate) fn parse_module_declaration<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        input: Input<'tokens, 'source>,
    ) -> ParseResult<'tokens, 'source, ModuleDeclaration> {
        let (path, input) = parse_dot_or_colon_path(input, |member| {
            syntax_trees.items.append_identifier_path_member(member)
        })?;
        let input = take_optional_semicolon(input)?;

        Ok((ModuleDeclaration { path }, input))
    }

    pub(crate) fn parse_package_declaration<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        input: Input<'tokens, 'source>,
    ) -> ParseResult<'tokens, 'source, PackageDeclaration> {
        let (path, input) = parse_dot_or_colon_path(input, |member| {
            syntax_trees.items.append_identifier_path_member(member)
        })?;
        let input = take_optional_semicolon(input)?;

        Ok((PackageDeclaration { path }, input))
    }

    fn parse_dot_or_colon_path<'tokens, 'source>(
        input: Input<'tokens, 'source>,
        mut append_member: impl FnMut(Identifier) -> arena::Handle<Identifier>,
    ) -> ParseResult<'tokens, 'source, arena::HandleSpan<Identifier>> {
        let (first, mut rest) = input.take_identifier()?;
        let mut path = crate::storage::handle_spans::HandleSpanBuilder::new();
        path.push(append_member(first));

        loop {
            if rest.at_punctuation(PunctuationKind::Dot) {
                rest = rest.take_punctuation(PunctuationKind::Dot, ".")?;
            } else if rest.at_punctuation(PunctuationKind::ColonColon) {
                rest = rest.take_punctuation(PunctuationKind::ColonColon, "::")?;
            } else {
                break;
            }

            let (member, next) = rest.take_identifier()?;
            path.push(append_member(member));
            rest = next;
        }

        Ok((path.finish(), rest))
    }

    pub(crate) fn take_optional_semicolon<'tokens, 'source>(
        input: Input<'tokens, 'source>,
    ) -> Result<Input<'tokens, 'source>, crate::diagnostics::parse_error::ParseError> {
        if input.at_punctuation(PunctuationKind::Semicolon) {
            input.take_punctuation(PunctuationKind::Semicolon, ";")
        } else {
            Ok(input)
        }
    }
}
pub(crate) mod operator {
    use crate::grammar::contracts::facts::parse_proof_facts_until;
    use crate::grammar::parameters::parse_generic_parameters::GenericParameterSyntax;
    use crate::grammar::parameters::parse_generic_parameters::parse_generic_parameters;
    use crate::grammar::parameters::parse_parameters::{
        parse_optional_parameters, parse_optional_return_type,
    };
    use crate::tokens::token_cursor::{Input, ParseResult, parse_path_handle_span};
    use syntax_trees::SyntaxTrees;
    use syntax_trees::item::{CapabilityContract, CapabilityContractKind, OperatorDefinition};
    use syntax_trees::operator_spelling::OperatorSpelling;
    use tokens::PunctuationKind;

    pub(crate) fn parse_operator_definition<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        input: Input<'tokens, 'source>,
        is_boundary: bool,
    ) -> ParseResult<'tokens, 'source, OperatorDefinition> {
        let body_start_tokens = input.tokens.len();
        let (spelling, input) = if input
            .tokens
            .first()
            .is_some_and(|token| token.punctuation().is_some())
        {
            let (spelling, input) = parse_operator_spelling(input)?;
            (Some(spelling), input)
        } else {
            (None, input)
        };
        let (name, input) = parse_path_handle_span(input, |member| {
            syntax_trees.items.append_identifier_path_member(member)
        })?;
        let (generic_parameters, input) =
            parse_generic_parameters(syntax_trees, input, GenericParameterSyntax::TypeAndConst)?;
        let type_parameters = generic_parameters.type_parameters;
        let (parameters, input) = parse_optional_parameters(syntax_trees, input)?;
        let (return_type, mut input) = parse_optional_return_type(syntax_trees, input)?;

        let mut contracts = crate::storage::handle_spans::HandleSpanBuilder::new();
        loop {
            if is_operator_contract_start(&input) {
                let contract = parse_operator_contract(syntax_trees, &mut input)?;
                contracts.push(syntax_trees.items.append_capability_contract(contract));
                continue;
            }
            break;
        }

        input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
        let token_count = body_start_tokens.saturating_sub(input.tokens.len());

        Ok((
            OperatorDefinition {
                is_public: false,
                is_boundary,
                name,
                lifetime_parameters: generic_parameters.lifetime_parameters,
                type_parameters,
                parameters,
                return_type,
                contracts: contracts.finish(),
                spelling,
                token_count,
            },
            input,
        ))
    }

    pub(crate) fn parse_operator_spelling<'tokens, 'source>(
        input: Input<'tokens, 'source>,
    ) -> ParseResult<'tokens, 'source, OperatorSpelling> {
        let mut symbol = String::new();
        let mut rest = input;
        while rest.tokens.first().is_some_and(|token| {
            token.punctuation().is_some() && token.punctuation() != Some(PunctuationKind::Semicolon)
        }) {
            let (token, next) = rest.expect_token()?;
            symbol.push_str(input.text(token).as_str());
            rest = next;
            if OperatorSpelling::from_symbol(&symbol).is_some() {
                break;
            }
        }

        // A bindable arithmetic token followed by a policy suffix lexes as one
        // suffixed operator; name the rule instead of listing spellings.
        let policy_form = symbol.len() > 1
            && symbol.ends_with(['%', '|', '!'])
            && OperatorSpelling::from_symbol(&symbol[..symbol.len() - 1])
                .is_some_and(|base| matches!(base.symbol(), "+" | "-" | "*" | "/" | "%"));
        if policy_form {
            return Err(input.error_here(
                "a suffixed policy operator such as `+%`, `+|` or `+!` is a compiler-owned integer primitive; a declaration cannot bind one",
            ));
        }
        match OperatorSpelling::from_symbol(&symbol) {
            Some(spelling) => Ok((spelling, rest)),
            None => Err(input.error_here(format!(
                "unknown operator spelling `{symbol}`; expected one of {}",
                OperatorSpelling::ALL
                    .iter()
                    .map(|spelling| spelling.symbol())
                    .collect::<Vec<_>>()
                    .join(" ")
            ))),
        }
    }

    fn parse_operator_contract<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        input: &mut Input<'tokens, 'source>,
    ) -> Result<CapabilityContract, crate::diagnostics::parse_error::ParseError> {
        if input.at_contextual("requires") {
            let keyword_source_span = Some(input.current_source_span());
            *input = input.take_contextual("requires")?;
            return parse_operator_fact_contract(
                syntax_trees,
                input,
                CapabilityContractKind::Requires,
                keyword_source_span,
            );
        }

        if input.at_contextual("ensures") {
            let keyword_source_span = Some(input.current_source_span());
            *input = input.take_contextual("ensures")?;
            return parse_operator_fact_contract(
                syntax_trees,
                input,
                CapabilityContractKind::Ensures,
                keyword_source_span,
            );
        }

        if input.at_contextual("crashes") {
            let keyword_source_span = Some(input.current_source_span());
            *input = input.take_contextual("crashes")?;
            if input.at_contextual("none") {
                return Err(input.error_here(
                    "`crashes none` is not an operator clause; an operator head without `crashes` declares no crash routes",
                ));
            }
            let (cause, after_cause) = input.take_identifier()?;
            let cause = crate::grammar::contracts::parse_crash_cause(&cause, after_cause)?;
            let after_header = after_cause;
            let header_token_count = 2usize;
            *input = after_header;
            let mut contract = parse_operator_fact_contract(
                syntax_trees,
                input,
                CapabilityContractKind::Crashes { cause },
                keyword_source_span,
            )?;
            contract.token_count = contract
                .token_count
                .checked_add(header_token_count)
                .expect("operator crash contract token count overflow");
            return Ok(contract);
        }

        Err(input.expected_one_of_here(&["`requires`", "`ensures`", "`crashes`"]))
    }

    fn parse_operator_fact_contract<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        input: &mut Input<'tokens, 'source>,
        kind: CapabilityContractKind,
        keyword_source_span: Option<source::SourceSpan>,
    ) -> Result<CapabilityContract, crate::diagnostics::parse_error::ParseError> {
        let ((facts, token_count), rest) =
            parse_proof_facts_until(syntax_trees, *input, operator_contract_terminator)?;
        *input = rest;

        Ok(CapabilityContract {
            kind,
            keyword_source_span,
            binding: None,
            facts,
            token_count,
        })
    }

    fn is_operator_contract_start(input: &Input<'_, '_>) -> bool {
        input.at_contextual("requires")
            || input.at_contextual("ensures")
            || input.at_contextual("crashes")
    }

    fn operator_contract_terminator(input: Input<'_, '_>) -> bool {
        input.at_punctuation(PunctuationKind::Semicolon)
            || input.at_punctuation(PunctuationKind::RightBrace)
            || is_operator_contract_start(&input)
            || input.at_contextual("operator")
            || input.at_contextual("boundary")
            || input.tokens.is_empty()
    }
}
pub(crate) mod parse_declaration {
    use super::namespace::{parse_module_declaration, parse_package_declaration};

    use super::capability::parse_capability_definition;
    use super::const_item::parse_const_definition;
    use super::domain::parse_domain_definition;
    use super::let_definition::parse_let_definition;
    use super::measure::parse_measure_definition;
    use super::operator::parse_operator_definition;
    use super::proposition::parse_proposition_definition;
    use super::trait_definition::parse_trait_definition;
    use super::use_item::parse_use_item;
    use crate::grammar::declarations::data::{
        parse_boundary_data_definition, parse_data_definition,
    };
    use crate::grammar::declarations::machines::parse_machine;
    use crate::grammar::expressions::parse_expression::parse_proof_fact_expression_handle;
    use crate::tokens::token_cursor::{Input, ParseResult};
    use syntax_trees::SyntaxTrees;
    use syntax_trees::identifier::Identifier;
    use syntax_trees::item::{Item, MathematicalDefinition};
    use tokens::{KeywordKind, PunctuationKind};

    pub(crate) enum ParsedDeclaration {
        Item(Item),
        Mathematical(MathematicalDefinition),
    }

    pub(crate) fn starts_following_item(
        syntax_trees: &mut SyntaxTrees,
        input: Input<'_, '_>,
    ) -> bool {
        if input.at_keyword(KeywordKind::Public) {
            let after = input.restarted(input.tokens.get(1..).unwrap_or_default());
            return starts_following_item(syntax_trees, after);
        }
        if input.at_keyword(KeywordKind::Machine)
            || input.at_keyword(KeywordKind::Data)
            || input.at_keyword(KeywordKind::Use)
            || input.at_keyword(KeywordKind::Let)
            || input.at_keyword(KeywordKind::Capability)
            || target_machine_prefix(input).is_some()
            || named_implementation_prefix(input).is_some()
        {
            return true;
        }
        if [
            "module",
            "package",
            "const",
            "proposition",
            "measure",
            "trait",
        ]
        .iter()
        .any(|keyword| input.at_contextual_then_name(keyword))
        {
            return true;
        }
        if input.at_contextual("operator") || input.at_contextual("domain") {
            let checkpoint = syntax_trees.append_checkpoint();
            let complete_fact = parse_proof_fact_expression_handle(syntax_trees, input)
                .is_ok_and(|(_, rest)| rest.at_punctuation(PunctuationKind::Semicolon));
            let following_item = !complete_fact && parse_declared_item(syntax_trees, input).is_ok();
            syntax_trees.rewind_appended(checkpoint);
            return following_item;
        }
        starts_boundary_item(input)
    }

    pub(crate) fn starts_boundary_item(input: Input<'_, '_>) -> bool {
        if !input.at_contextual("boundary") {
            return false;
        }
        let after = input.restarted(input.tokens.get(1..).unwrap_or_default());
        after.at_keyword(KeywordKind::Machine)
            || after.at_keyword(KeywordKind::Data)
            || after.at_keyword(KeywordKind::Let)
            || after.at_contextual("requirement")
            || after.at_contextual("operator")
            || after.at_contextual("trait")
    }

    pub(crate) fn named_implementation_prefix<'tokens, 'source>(
        input: Input<'tokens, 'source>,
    ) -> Option<(Identifier, Input<'tokens, 'source>)> {
        let (name, rest) = input.take_identifier().ok()?;
        if rest.at_contextual("satisfies") {
            return Some((name, rest));
        }
        if !rest.at_punctuation(PunctuationKind::Less) {
            return None;
        }
        let mut angle_depth = 0usize;
        let mut operand_depth = 0usize;
        for (token_index, token) in rest.tokens.iter().enumerate() {
            match token.punctuation() {
                Some(PunctuationKind::LeftParen | PunctuationKind::LeftBracket) => {
                    operand_depth += 1;
                }
                Some(PunctuationKind::RightParen | PunctuationKind::RightBracket) => {
                    operand_depth = operand_depth.checked_sub(1)?;
                }
                _ if operand_depth != 0 => continue,
                Some(PunctuationKind::Less) => angle_depth += 1,
                Some(PunctuationKind::Greater) => angle_depth = angle_depth.checked_sub(1)?,
                Some(PunctuationKind::GreaterGreater) => {
                    angle_depth = angle_depth.checked_sub(2)?;
                }
                Some(
                    PunctuationKind::Semicolon
                    | PunctuationKind::LeftBrace
                    | PunctuationKind::RightBrace,
                ) => return None,
                _ => continue,
            }
            if angle_depth == 0 && operand_depth == 0 {
                let after = input.restarted(&rest.tokens[token_index + 1..]);
                return after.at_contextual("satisfies").then_some((name, rest));
            }
        }
        None
    }

    fn target_machine_prefix<'tokens, 'source>(
        input: Input<'tokens, 'source>,
    ) -> Option<(Identifier, bool, Input<'tokens, 'source>)> {
        if !input.at_identifier_then_contextual("machine")
            && !input.at_identifier_then_contextual("boundary")
        {
            return None;
        }
        let (target, mut rest) = input.take_identifier().ok()?;
        let boundary = rest.at_contextual("boundary");
        if boundary {
            rest = rest.take_contextual("boundary").ok()?;
        }
        rest.at_keyword(KeywordKind::Machine)
            .then_some((target, boundary, rest))
    }

    pub(crate) fn starts_machine_declaration(input: Input<'_, '_>) -> bool {
        if input.at_keyword(KeywordKind::Public) || input.at_contextual("boundary") {
            return starts_machine_declaration(
                input.restarted(input.tokens.get(1..).unwrap_or_default()),
            );
        }
        input.at_keyword(KeywordKind::Machine) || target_machine_prefix(input).is_some()
    }

    pub(crate) fn parse_item<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        input: Input<'tokens, 'source>,
    ) -> ParseResult<'tokens, 'source, ParsedDeclaration> {
        if input.at_keyword(KeywordKind::Public) {
            let input = input.take_keyword(KeywordKind::Public, "public")?;
            let (mut declaration, rest) = parse_item(syntax_trees, input)?;
            match &mut declaration {
                ParsedDeclaration::Item(item) => match item {
                    Item::Conformance(conformance) => conformance.is_public = true,
                    Item::Const(constant) => constant.is_public = true,
                    Item::Data(data) => data.is_public = true,
                    Item::TypeAlias(alias) => alias.is_public = true,
                    Item::Domain(domain) => domain.is_public = true,
                    Item::Machine(machine) => machine.is_public = true,
                    Item::Operator(operator) => operator.is_public = true,
                    Item::Proposition(proposition) => proposition.is_public = true,
                    Item::Trait(trait_definition) => trait_definition.is_public = true,
                    _ => {
                        return Err(rest.error_here(
                        "`public` is not yet retained for this declaration kind; refusing to compile a silently private API",
                    ));
                    }
                },
                ParsedDeclaration::Mathematical(definition) => definition.is_public = true,
            }
            return Ok((declaration, rest));
        }

        if input.at_keyword(KeywordKind::Let) {
            let input = input.take_keyword(KeywordKind::Let, "let")?;
            let (definition, rest) = parse_let_definition(syntax_trees, input, false)?;
            return Ok((ParsedDeclaration::Mathematical(definition), rest));
        }

        if input.at_contextual("boundary") {
            let input = input.take_contextual("boundary")?;
            return parse_boundary_item(syntax_trees, input);
        }

        let (item, rest) = parse_declared_item(syntax_trees, input)?;
        Ok((ParsedDeclaration::Item(item), rest))
    }

    fn parse_boundary_item<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        input: Input<'tokens, 'source>,
    ) -> ParseResult<'tokens, 'source, ParsedDeclaration> {
        if input.at_keyword(KeywordKind::Let) {
            let input = input.take_keyword(KeywordKind::Let, "let")?;
            let (definition, rest) = parse_let_definition(syntax_trees, input, true)?;
            return Ok((ParsedDeclaration::Mathematical(definition), rest));
        }
        if input.at_contextual("requirement") {
            return Err(input.error_here(
            "standalone `boundary requirement Owner::operation(...);` declarations are retired; \
             declare the operation as a member of a boundary trait \
             (`boundary trait Owner { machine operation(...) reaches ...; }`), \
             using a one-member trait when it needs independent provider selection",
        ));
        }
        if input.at_keyword(KeywordKind::Data) {
            let input = input.take_keyword(KeywordKind::Data, "data")?;
            let (item, rest) = parse_boundary_data_definition(syntax_trees, input)?;
            return Ok((ParsedDeclaration::Item(Item::Data(item)), rest));
        }
        if input.at_keyword(KeywordKind::Machine) {
            let input = input.take_keyword(KeywordKind::Machine, "machine")?;
            let (mut item, rest) = parse_machine(syntax_trees, input)?;
            item.boundary = true;
            if item.service_reach_is_installation_bound
                && (!item.bodyless || !item.satisfies.is_empty())
            {
                return Err(rest.error_here(
                "`reaches <= Bound` is legal only on a top-level bodyless `boundary machine` requirement, not on a checked body or realization",
            ));
            }
            return Ok((ParsedDeclaration::Item(Item::Machine(item)), rest));
        }
        if input.at_contextual("operator") {
            let input = input.take_contextual("operator")?;
            let (item, rest) = parse_operator_definition(syntax_trees, input, true)?;
            return Ok((ParsedDeclaration::Item(Item::Operator(item)), rest));
        }
        let (item, rest) = parse_trait_definition(syntax_trees, input, true)?;
        Ok((ParsedDeclaration::Item(Item::Trait(item)), rest))
    }

    fn parse_declared_item<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        input: Input<'tokens, 'source>,
    ) -> ParseResult<'tokens, 'source, Item> {
        if input.at_contextual("module") {
            let input = input.take_contextual("module")?;
            let (item, rest) = parse_module_declaration(syntax_trees, input)?;
            return Ok((Item::Module(item), rest));
        }

        if input.at_contextual("package") {
            let input = input.take_contextual("package")?;
            let (item, rest) = parse_package_declaration(syntax_trees, input)?;
            return Ok((Item::Package(item), rest));
        }

        if input.at_keyword(KeywordKind::Use) {
            let input = input.take_keyword(KeywordKind::Use, "use")?;
            let (item, rest) = parse_use_item(syntax_trees, input)?;
            return Ok((Item::Use(item), rest));
        }

        if input.at_keyword(KeywordKind::Data) {
            let input = input.take_keyword(KeywordKind::Data, "data")?;
            return parse_data_definition(syntax_trees, input);
        }

        if input.at_contextual("domain") {
            let input = input.take_contextual("domain")?;
            let (item, rest) = parse_domain_definition(syntax_trees, input)?;
            return Ok((Item::Domain(item), rest));
        }

        if input.at_contextual("const") {
            let input = input.take_contextual("const")?;
            let (item, rest) = parse_const_definition(syntax_trees, input)?;
            return Ok((Item::Const(item), rest));
        }

        if input.at_contextual("proposition") {
            let input = input.take_contextual("proposition")?;
            let (item, rest) = parse_proposition_definition(syntax_trees, input)?;
            return Ok((Item::Proposition(item), rest));
        }

        if input.at_contextual("abi") {
            return Err(input.error_here(
            "`abi \"...\"` is retired: declare the exported callable as `boundary machine ...`; its satisfied requirement selects the calling policy (see wiki/spec/build/calling_plans.md)",
        ));
        }

        if input.at_keyword(KeywordKind::Machine) {
            let input = input.take_keyword(KeywordKind::Machine, "machine")?;
            let (item, rest) = parse_machine(syntax_trees, input)?;
            if item.service_reach_is_installation_bound {
                return Err(rest.error_here(
                "`reaches <= Bound` is legal only on a top-level bodyless `boundary machine` requirement",
            ));
            }
            let has_via = syntax_trees
                .items
                .satisfies_clauses(item.satisfies)
                .iter()
                .any(|clause| clause.via.is_some() || clause.via_expression.is_valid());
            if item.bodyless && !has_via && !item.satisfies.is_empty() {
                return Err(rest.error_here(
                    "a machine without a body is the ACCEPTED boundary form -- spell it \
                 `boundary machine ...;` (chapter 10: bodyless contracts are trust \
                 rows, not ordinary machines) -- or an EXTERNAL LEAF \
                 (`satisfies Requirement via <Binding>;`)",
                ));
            }
            return Ok((Item::Machine(item), rest));
        }

        if input.at_keyword(KeywordKind::Capability) {
            let input = input.take_keyword(KeywordKind::Capability, "capability")?;
            let (item, rest) = parse_capability_definition(syntax_trees, input)?;
            return Ok((Item::Capability(item), rest));
        }

        if input.at_keyword(KeywordKind::Target) {
            return Err(input.error_here(
                "`target` declarations are retired: select one exact target through the compiler \
             invocation; target host and boundary policy come from immutable compiler/package \
             inputs",
            ));
        }

        if input.at_contextual("invariant") {
            return Err(input.error_here(
                "the `invariant` declaration is retired: put value-wide facts in a data \
             default domain (`requires` or field constraints) and behavioral facts in \
             explicit contracts",
            ));
        }

        if input.at_keyword(KeywordKind::Library) {
            return Err(input.error_here(
            "the legacy `library \"...\" calling_convention ... { entry ... }` block is retired; declare an exact boundary-trait requirement and realize it with `satisfies ... via` one producer machine returning `ForeignBinding::DllImport { ... }`",
        ));
        }

        if input.at_contextual("measure") {
            let input = input.take_contextual("measure")?;
            let (item, rest) = parse_measure_definition(syntax_trees, input)?;
            return Ok((Item::Measure(item), rest));
        }

        if input.at_contextual("operator") {
            let input = input.take_contextual("operator")?;
            let (item, rest) = parse_operator_definition(syntax_trees, input, false)?;
            return Ok((Item::Operator(item), rest));
        }

        if input.at_keyword(KeywordKind::Host) {
            return Err(input.error_here(
                "authored `provides` declarations are retired (including `host ... provides`): \
             implement a boundary-trait \
             requirement with a checked `satisfies` adapter or a bodyless external leaf using \
             `satisfies Trait::method via Binding::Case(...)`",
            ));
        }

        if input.at_identifier_then_contextual("provides") {
            return Err(input.error_here(
                "authored `provides` declarations are retired: implement a boundary-trait \
             requirement with a checked `satisfies` adapter or a bodyless external leaf using \
             `satisfies Trait::method via Binding::Case(...)`",
            ));
        }

        if input.at_keyword(KeywordKind::Platform) {
            return Err(input.error_here(
                "`platform` blocks are retired: declare the host surface as a \
             `boundary trait` with per-method `reaches` rows (std's Console \
             is the model) -- same signatures, same requires/ensures, and \
             the purity checker sees the truth",
            ));
        }

        if input.at_contextual("trait") {
            let (item, rest) = parse_trait_definition(syntax_trees, input, false)?;
            return Ok((Item::Trait(item), rest));
        }

        if let Some((target, boundary, input)) = target_machine_prefix(input) {
            let input = input.take_keyword(KeywordKind::Machine, "machine")?;
            let (mut machine, rest) = parse_machine(syntax_trees, input)?;
            machine.boundary = boundary;
            machine.authored_target = Some(target.clone());
            machine.target = Some(target);
            if machine.service_reach_is_installation_bound
                && (!boundary || !machine.bodyless || !machine.satisfies.is_empty())
            {
                return Err(rest.error_here(
                "`reaches <= Bound` is legal only on a top-level bodyless `boundary machine` requirement, not on a checked body or realization",
            ));
            }
            return Ok((Item::Machine(machine), rest));
        }

        if let Some((alias, rest)) = named_implementation_prefix(input) {
            return super::conformance::parse_conformance(syntax_trees, alias, rest);
        }
        if let Ok((alias, rest)) = input.take_identifier()
            && (rest.at_punctuation(PunctuationKind::Colon)
                || rest.at_punctuation(PunctuationKind::Less))
        {
            return super::conformance::parse_conformance(syntax_trees, alias, rest);
        }

        if input.at_contextual("satisfies") {
            return Err(input.error_here(
            "the subjectless `satisfies Trait as Name { ... }` conformance header is retired; write `Name satisfies Trait { ... }`",
        ));
        }

        Err(input.expected_one_of_here(&[
            "`use`",
            "`data`",
            "`domain`",
            "`abi`",
            "`machine`",
            "`capability`",
            "`let`",
            "`library`",
            "`measure`",
            "`host`",
            "`module`",
            "`operator`",
            "`package`",
            "`platform`",
            "`public`",
            "`trait`",
            "`boundary let`",
            "`boundary operator`",
            "`boundary requirement`",
            "`boundary data`",
            "`boundary trait`",
        ]))
    }
}
pub(crate) mod proposition {
    use crate::grammar::expressions::parse_expression::parse_expression_handle_without_struct_literals;
    use crate::grammar::parameters::parse_generic_parameters::GenericParameterSyntax;
    use crate::grammar::parameters::parse_generic_parameters::parse_generic_parameters;
    use crate::grammar::parameters::parse_parameters::parse_optional_parameters;
    use crate::grammar::type_syntax::parse_type::parse_type_reference_handle;
    use crate::tokens::token_cursor::{Input, ParseResult};
    use syntax_trees::SyntaxTrees;
    use syntax_trees::item::{PropositionBody, PropositionDefinition};
    use tokens::PunctuationKind;

    pub(crate) fn parse_proposition_definition<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        input: Input<'tokens, 'source>,
    ) -> ParseResult<'tokens, 'source, PropositionDefinition> {
        let (name, input) = input.take_identifier()?;
        let (generic_parameters, input) =
            parse_generic_parameters(syntax_trees, input, GenericParameterSyntax::StaticBinders)?;
        if !generic_parameters.lifetime_parameters.is_empty() {
            return Err(input.error_here(
                "proposition binders are proof-static and cannot declare lifetime parameters",
            ));
        }
        let (parameters, input) = parse_optional_parameters(syntax_trees, input)?;

        let (body, transparent_formula_source_span, input) = if input
            .at_punctuation(PunctuationKind::Semicolon)
        {
            (
                PropositionBody::Primitive,
                None,
                input.take_punctuation(PunctuationKind::Semicolon, ";")?,
            )
        } else if input.at_contextual("evidence") {
            let input = input.take_contextual("evidence")?;
            let (evidence, input) = parse_type_reference_handle(syntax_trees, input)?;
            let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
            (PropositionBody::Witness { evidence }, None, input)
        } else if input.at_punctuation(PunctuationKind::LeftBrace) {
            return Err(input.error_here(
            "`{ Evidence; }` proposition evidence is retired; write `evidence Evidence;` after the proposition signature",
        ));
        } else if input.at_punctuation(PunctuationKind::Equal) {
            let input = input.take_punctuation(PunctuationKind::Equal, "=")?;
            let formula_start = input;
            let (proposition, formula_end) =
                parse_expression_handle_without_struct_literals(syntax_trees, input)?;
            let formula_source_span = formula_start.source_span_until(formula_end);
            let input = formula_end.take_punctuation(PunctuationKind::Semicolon, ";")?;
            (
                PropositionBody::Transparent { proposition },
                Some(formula_source_span),
                input,
            )
        } else {
            return Err(input.expected_one_of_here(&[
                "`;` for a primitive proposition",
                "`evidence Interface;` for a witness-bearing proposition",
                "`= fact;` for a transparent proposition",
            ]));
        };

        Ok((
            PropositionDefinition {
                name,
                is_public: false,
                type_parameters: generic_parameters.type_parameters,
                parameters,
                transparent_formula_source_span,
                body,
            },
            input,
        ))
    }
}
pub(crate) mod trait_definition {
    use crate::grammar::bodies::trait_default::parse_trait_default_machine_body;
    use crate::grammar::contracts::signature::parse_signature_clauses;
    use crate::grammar::declarations::operator::parse_operator_spelling;
    use crate::grammar::parameters::contracts::parse_machine_parameter_contracts;
    use crate::grammar::parameters::contracts::validate_machine_parameter_contracts;
    use crate::grammar::parameters::parse_generic_parameters::GenericParameterSyntax;
    use crate::grammar::parameters::parse_generic_parameters::parse_generic_parameters;
    use crate::grammar::parameters::parse_parameters::{
        parse_optional_parameters, parse_optional_return_type, parse_parameter,
    };
    use crate::grammar::type_syntax::parse_type::parse_type_reference_handle;
    use crate::storage::tables::{items, items_of, type_refs, type_refs_of};
    use crate::tokens::token_cursor::{Input, ParseResult};
    use arena::HandleSpan;
    use syntax_trees::SyntaxTrees;
    use syntax_trees::identifier::Identifier;
    use syntax_trees::item::{PropositionParameterSignature, StateSignature, TraitDefinition};
    use syntax_trees::types::TypeReferenceNode;
    use tokens::{KeywordKind, PunctuationKind};

    pub(crate) fn parse_trait_definition<'tokens, 'source>(
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
        if refines.is_some() && input.at_contextual("requires") {
            return Err(input.error_here(
            "a transparent refinement takes no `requires` clauses; bounds on its parameters belong to the base trait",
        ));
        }
        let ((), next) =
            parse_proposition_parameter_contracts(syntax_trees, type_parameters, input)?;
        input = next;
        let ((), next) = parse_machine_parameter_contracts(syntax_trees, type_parameters, input)?;
        validate_machine_parameter_contracts(syntax_trees, type_parameters, next)?;
        input = next;
        let conformance_bounds = generic_parameters.conformance_bounds;
        input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
        let mut requires = crate::storage::handle_spans::HandleSpanBuilder::new();
        let mut machines = crate::storage::handle_spans::HandleSpanBuilder::new();
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
                requires.push(handle);
                input = rest;
                continue;
            }

            if input.at_contextual("default") {
                return Err(input.error_here(
                    "the `default` keyword is retired: a trait machine with a body IS \
                 the default -- drop the keyword and keep the body",
                ));
            }
            if input.at_contextual("operator") {
                return Err(input.error_here(
                    "the `operator` introducer is retired: a trait requirement binds operator \
                 syntax as an ordinary machine -- write `machine + name(...)`, or \
                 `machine name(...)` without a token",
                ));
            }
            input = input.take_keyword(KeywordKind::Machine, "machine")?;
            let spelling = if input.tokens.first().is_some_and(|token| {
                token.punctuation().is_some()
                    && token.punctuation() != Some(PunctuationKind::Semicolon)
            }) {
                let (spelling, rest) = parse_operator_spelling(input)?;
                input = rest;
                Some(spelling)
            } else {
                None
            };
            let (mut signature, rest) = parse_trait_machine_signature(syntax_trees, input)?;
            signature.spelling = spelling;
            let (clauses, rest) =
                parse_signature_clauses(syntax_trees, rest, signature.type_parameters)?;
            validate_machine_parameter_contracts(syntax_trees, signature.type_parameters, rest)?;
            let is_default = rest.at_punctuation(PunctuationKind::LeftBrace);
            if !signature.native_callback_parameters.is_empty() && (!is_boundary || is_default) {
                return Err(rest.error_here(
                "`native callback ... from ...` is permitted only on a bodyless boundary-trait requirement",
            ));
            }
            signature.is_default = is_default;
            if is_boundary && clauses.service_reach_keyword_source_spans.is_empty() {
                return Err(rest.error_here(format!(
                "boundary-trait machine requirement `{}::{}` must declare an explicit `reaches` ceiling; write `reaches none` for empty reach",
                name.as_str(syntax_trees),
                signature.name.as_str(syntax_trees),
            )));
            }
            if clauses.service_reach_is_installation_bound && (!is_boundary || is_default) {
                return Err(rest.error_here(
                    "`reaches <= Bound` is permitted only on a bodyless boundary-trait requirement",
                ));
            }
            signature.service_reach_is_installation_bound =
                clauses.service_reach_is_installation_bound;
            signature.service_reach_keyword_source_spans =
                clauses.service_reach_keyword_source_spans;
            signature.service_reaches = clauses.service_reaches;
            signature.invokes = clauses.invokes;
            signature.suspends_keyword_source_spans = clauses.suspends_keyword_source_spans;
            signature.blocks_keyword_source_spans = clauses.blocks_keyword_source_spans;
            signature.suspends = clauses.suspends;
            signature.blocks = clauses.blocks;
            signature.contracts = clauses.contracts;
            signature.terminates_guarantee = clauses.terminates_guarantee;
            let (default_body, next) = if is_default {
                parse_trait_default_machine_body(syntax_trees, rest)?
            } else {
                (
                    HandleSpan::empty(),
                    rest.take_punctuation(PunctuationKind::Semicolon, ";")?,
                )
            };
            signature.default_body = default_body;
            let handle = items(syntax_trees).insert_state_signature(&signature);
            let handle = items(syntax_trees).append_state_signature_handle(handle);
            machines.push(handle);
            input = next;
        }

        input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
        Ok((
            TraitDefinition {
                is_boundary,
                is_public: false,
                name,
                lifetime_parameters: generic_parameters.lifetime_parameters,
                type_parameters,
                conformance_bounds,
                parents,
                requires: requires.finish(),
                machines: machines.finish(),
                refines,
                refinement_clauses,
            },
            input,
        ))
    }

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
                let base_name = refines.and_then(|base| {
                    match type_refs_of(syntax_trees).type_reference(base) {
                        TypeReferenceNode::Named(name) => Some(name.clone()),
                        TypeReferenceNode::Generic { base_name, .. } => Some(base_name.clone()),
                        _ => None,
                    }
                });
                let qualifies_base = qualifier.iter().all(|segment| {
                    base_name.as_ref().is_some_and(|name| {
                        segment.as_str(syntax_trees) == name.as_str(syntax_trees)
                    })
                });
                if !qualifies_base || qualifier.len() > 1 {
                    return Err(input.error_here(
                        "a refinement clause qualifies its requirement with the base trait name",
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
        let mut signature = crate::grammar::parameters::parse_parameters::blank_state_signature();
        signature.name = requirement
            .clone()
            .unwrap_or_else(|| Identifier::generated("*"));
        let mut authored_suspends = false;
        let mut authored_blocks = false;
        let mut authored_terminates = false;
        let mut authored_reaches = false;
        let mut contracts = Vec::new();
        loop {
            if input.at_contextual("ensures") {
                if requirement.is_none() {
                    return Err(input.error_here(
                        "a refinement postcondition names one exact base requirement",
                    ));
                }
                let keyword_source_span = Some(input.current_source_span());
                input = input.take_contextual("ensures")?;
                let (binding, fact_input) = input.take_optional_label()?;
                let ((facts, token_count), rest) =
                    crate::grammar::contracts::facts::parse_required_proof_facts_until(
                        syntax_trees,
                        fact_input,
                        refinement_reaches_clause_boundary,
                        false,
                    )?;
                contracts.push(syntax_trees::item::CapabilityContract {
                    kind: syntax_trees::item::CapabilityContractKind::Ensures,
                    keyword_source_span,
                    binding,
                    facts,
                    token_count,
                });
                input = rest;
                continue;
            }
            if input.at_contextual("reaches") {
                if authored_reaches {
                    return Err(
                        input.error_here("duplicate `reaches` clause on a refinement member")
                    );
                }
                authored_reaches = true;
                let after_keyword = input.take_contextual("reaches")?;
                if after_keyword.at_punctuation(PunctuationKind::LessEqual) {
                    return Err(after_keyword.error_here(
                    "`reaches <= Bound` is an installation row; a refinement narrows the authored reach set",
                ));
                }
                let mut reaches = Vec::new();
                let mut installation_bound = false;
                input = crate::grammar::contracts::take_operational_reaches_clause(
                    input,
                    &mut signature.service_reach_keyword_source_spans,
                    &mut reaches,
                    &mut installation_bound,
                    refinement_reaches_clause_boundary,
                )?;
                signature.service_reaches =
                    items(syntax_trees).insert_identifier_path_members(reaches);
                continue;
            }
            if input.at_contextual("suspends") {
                if authored_suspends {
                    return Err(
                        input.error_here("duplicate `suspends` clause on a refinement member")
                    );
                }
                authored_suspends = true;
                signature
                    .suspends_keyword_source_spans
                    .push(input.current_source_span());
                input = input.take_contextual("suspends")?;
                if input.at_keyword(KeywordKind::True) || input.at_keyword(KeywordKind::False) {
                    let (value, rest) = parse_refinement_axis_bool(input)?;
                    signature.suspends = value;
                    input = rest;
                } else {
                    signature.suspends = true;
                }
                continue;
            }
            if input.at_contextual("blocks") {
                if authored_blocks {
                    return Err(
                        input.error_here("duplicate `blocks` clause on a refinement member")
                    );
                }
                authored_blocks = true;
                signature
                    .blocks_keyword_source_spans
                    .push(input.current_source_span());
                input = input.take_contextual("blocks")?;
                if input.at_keyword(KeywordKind::True) || input.at_keyword(KeywordKind::False) {
                    let (value, rest) = parse_refinement_axis_bool(input)?;
                    signature.blocks = value;
                    input = rest;
                } else {
                    signature.blocks = true;
                }
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
                continue;
            }
            break;
        }
        let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
        signature.contracts =
            crate::grammar::contracts::publish_capability_contracts(syntax_trees, contracts);
        clauses.push(syntax_trees::item::TraitRefinementClause {
            requirement,
            signature,
        });
        Ok(((), input))
    }

    fn refinement_reaches_clause_boundary(input: Input<'_, '_>) -> bool {
        input.at_punctuation(PunctuationKind::Semicolon)
            || input.at_punctuation(PunctuationKind::RightBrace)
            || input.at_contract_continuation_keyword()
            || input.at_contextual("requires")
            || input.at_contextual("ensures")
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
            if !input.at_contextual("requires") {
                break;
            }
            let after_requires = input.take_contextual("requires")?;
            if !after_requires.at_contextual("proposition") {
                break;
            }
            let after_proposition = after_requires.take_contextual("proposition")?;
            let (name, after_name) = after_proposition.take_identifier()?;
            let Some(parameter_index) = syntax_trees
                .items
                .type_parameters(type_parameters)
                .iter()
                .position(|parameter| {
                    parameter.name.as_str(syntax_trees) == name.as_str(syntax_trees)
                })
            else {
                return Err(after_name.error_here(format!(
                    "`requires proposition {}` has no matching `<proposition {}>` parameter",
                    name.as_str(syntax_trees),
                    name.as_str(syntax_trees),
                )));
            };
            match &items_of(syntax_trees).type_parameters(type_parameters)[parameter_index].kind {
                syntax_trees::item::TypeParameterKind::Proposition { contract: None } => {}
                syntax_trees::item::TypeParameterKind::Proposition { contract: Some(_) } => {
                    return Err(after_name.error_here(format!(
                        "proposition parameter `{}` already has a declaration-site signature",
                        name.as_str(syntax_trees),
                    )));
                }
                _ => {
                    return Err(after_name.error_here(format!(
                    "`{}` is not a proposition parameter; declare it as `<proposition {}>` first",
                    name.as_str(syntax_trees),
                    name.as_str(syntax_trees),
                )));
                }
            }
            let (parameters, rest) = parse_optional_parameters(syntax_trees, after_name)?;
            let rest = rest.take_punctuation(PunctuationKind::Semicolon, ";")?;
            items(syntax_trees).type_parameters_mut(type_parameters)[parameter_index].kind =
                syntax_trees::item::TypeParameterKind::Proposition {
                    contract: Some(PropositionParameterSignature { name, parameters }),
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
            "proposition parameter `{}` requires an authored declaration-site signature: write `requires proposition {}(...);`",
            missing.name.as_str(syntax_trees),
            missing.name.as_str(syntax_trees),
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
            type_refs(syntax_trees).insert_type_reference_handles(parents),
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
        let ((), input) = parse_machine_parameter_contracts(
            syntax_trees,
            generic_parameters.type_parameters,
            input,
        )?;

        Ok((
            StateSignature {
                name,
                lifetime_parameters: generic_parameters.lifetime_parameters,
                type_parameters: generic_parameters.type_parameters,
                conformance_bounds: generic_parameters.conformance_bounds,
                parameters,
                native_callback_parameters,
                return_type,
                ..crate::grammar::parameters::parse_parameters::blank_state_signature()
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
        let mut parameters = crate::storage::handle_spans::HandleSpanBuilder::new();
        let mut native_callback_parameters = Vec::new();
        let mut native_ordinal = 0u32;

        while !input.at_punctuation(PunctuationKind::RightParen) {
            if input.at_contextual("native") {
                let input_after_native = input.take_contextual("native")?;
                let input_after_callback = input_after_native.take_contextual("callback")?;
                let (name, input_after_name) = input_after_callback.take_identifier()?;
                let input_after_from = input_after_name.take_contextual("from")?;
                let (binder, rest) = input_after_from.take_identifier()?;
                if native_callback_parameters.iter().any(
                    |prior: &syntax_trees::item::NativeCallbackParameterNode| {
                        prior.name.as_str(syntax_trees) == name.as_str(syntax_trees)
                    },
                ) {
                    return Err(rest.error_here(format!(
                        "native callback parameter `{}` is declared more than once",
                        name.as_str(syntax_trees),
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
                let handle = items(syntax_trees).append_state_parameter_handle(parameter);
                parameters.push(handle);
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
        let parameters = parameters.finish();
        Ok(((parameters, native_callback_parameters), input))
    }

    fn parse_trait_machine_name<'tokens, 'source>(
        input: Input<'tokens, 'source>,
    ) -> ParseResult<'tokens, 'source, Identifier> {
        let (name, input) = input.take_identifier()?;
        if input.at_punctuation(PunctuationKind::ColonColon) {
            return Err(input.error_here(
            "a trait requirement declares its bare name; a trait has no subject type to qualify it, so declare each data type it needs as an ordinary trait parameter",
        ));
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
}
pub(crate) mod use_item {
    use crate::tokens::token_cursor::{Input, ParseResult, parse_path_handle_span};
    use syntax_trees::SyntaxTrees;
    use syntax_trees::item::UseItem;
    use tokens::PunctuationKind;

    pub(crate) fn parse_use_item<'tokens, 'source>(
        syntax_trees: &mut SyntaxTrees,
        input: Input<'tokens, 'source>,
    ) -> ParseResult<'tokens, 'source, UseItem> {
        let (path, input) = parse_path_handle_span(input, |member| {
            syntax_trees.items.append_identifier_path_member(member)
        })?;
        let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
        Ok((UseItem { path }, input))
    }
}
