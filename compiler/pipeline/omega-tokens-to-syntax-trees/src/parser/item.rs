use crate::parser::capability::parse_capability_definition;
use crate::parser::data::parse_data_definition;
use crate::parser::domain::parse_domain_definition;
use crate::parser::export_item::parse_export_item;
use crate::parser::input::{Input, ParseResult, parse_path_handle_span};
use crate::parser::invariant::parse_invariant_definition;
use crate::parser::library::parse_library_definition;
use crate::parser::machine::parse_machine;
use crate::parser::measure::parse_measure_definition;
use crate::parser::operator::parse_operator_definition;
use crate::parser::platform::parse_platform;
use crate::parser::target::parse_target_definition;
use crate::parser::trait_definition::parse_trait_definition;
use crate::parser::type_reference::parse_type_reference_handle;
use crate::parser::use_item::parse_use_item;
use omega_syntax_trees::SyntaxTrees;
use omega_syntax_trees::item::{
    HostProviderDefinition, HostProviderMapping, HostProviderMappingKind, Item, ModuleDeclaration,
    PackageDeclaration, ProviderDeclaration, WireDataDefinition, WireDataField, WireDataMember,
    WireDataReserved, WireDataVersion,
};
use omega_syntax_trees::operator_spelling::ProviderCategory;
use omega_tokens::{KeywordKind, PunctuationKind};

pub(super) fn parse_item<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, Item> {
    if input.at_keyword(KeywordKind::Pub) {
        let input = input.take_keyword(KeywordKind::Pub, "pub")?;
        // Visibility is syntax-level metadata for now. Semantic export rules
        // still live in explicit `export` items until module scoping grows up.
        return parse_item(syntax_trees, input);
    }

    if input.at_contextual("repr") {
        let input = input.take_contextual("repr")?;
        let input = input.take_contextual("native")?;
        let input = input.take_keyword(KeywordKind::Data, "data")?;
        // `repr native` explicitly requests the compiler's current native
        // field layout, so no extra representation marker is needed yet.
        let (item, rest) = parse_data_definition(syntax_trees, input)?;
        return Ok((Item::Data(item), rest));
    }

    if input.at_contextual("wire") {
        let input = input.take_contextual("wire")?;
        let input = input.take_keyword(KeywordKind::Data, "data")?;
        let (item, rest) = parse_wire_data_definition(syntax_trees, input)?;
        return Ok((Item::WireData(item), rest));
    }

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

    if input.at_contextual("export") {
        let input = input.take_contextual("export")?;
        let (item, rest) = parse_export_item(syntax_trees, input)?;
        return Ok((Item::Export(item), rest));
    }

    if input.at_keyword(KeywordKind::Data) {
        let input = input.take_keyword(KeywordKind::Data, "data")?;
        let (item, rest) = parse_data_definition(syntax_trees, input)?;
        return Ok((Item::Data(item), rest));
    }

    if input.at_contextual("domain") {
        let input = input.take_contextual("domain")?;
        let (item, rest) = parse_domain_definition(syntax_trees, input)?;
        return Ok((Item::Domain(item), rest));
    }

    if input.at_keyword(KeywordKind::Enum) {
        return Err(input.error_here(
            "`enum` is retired; spell alternatives as `case` members of a `data` declaration",
        ));
    }

    if input.at_contextual("abi") {
        let input = input.take_contextual("abi")?;
        let (abi, input) = input.take_string()?;
        let input = input.take_keyword(KeywordKind::Machine, "machine")?;
        let (mut item, rest) = parse_machine(syntax_trees, input)?;
        item.abi = Some(abi);
        return Ok((Item::Machine(item), rest));
    }

    if input.at_keyword(KeywordKind::Machine) {
        let input = input.take_keyword(KeywordKind::Machine, "machine")?;
        let (item, rest) = parse_machine(syntax_trees, input)?;
        return Ok((Item::Machine(item), rest));
    }

    if input.at_keyword(KeywordKind::Target) {
        let input = input.take_keyword(KeywordKind::Target, "target")?;
        let (item, rest) = parse_target_definition(syntax_trees, input)?;
        return Ok((Item::Target(item), rest));
    }

    if input.at_keyword(KeywordKind::Capability) {
        let input = input.take_keyword(KeywordKind::Capability, "capability")?;
        let (item, rest) = parse_capability_definition(syntax_trees, input)?;
        return Ok((Item::Capability(item), rest));
    }

    if input.at_keyword(KeywordKind::Invariant) {
        let input = input.take_keyword(KeywordKind::Invariant, "invariant")?;
        let (item, rest) = parse_invariant_definition(syntax_trees, input)?;
        return Ok((Item::Invariant(item), rest));
    }

    if input.at_keyword(KeywordKind::Library) {
        let input = input.take_keyword(KeywordKind::Library, "library")?;
        let (item, rest) = parse_library_definition(syntax_trees, input)?;
        return Ok((Item::Library(item), rest));
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

    if input.at_contextual("provider") {
        let input = input.take_contextual("provider")?;
        let (item, rest) = parse_provider_declaration(syntax_trees, input)?;
        return Ok((Item::Provider(item), rest));
    }

    if input.at_keyword(KeywordKind::Host) {
        let input = input.take_keyword(KeywordKind::Host, "host")?;
        let (item, rest) = parse_host_provider_definition(syntax_trees, input)?;
        return Ok((Item::HostProvider(item), rest));
    }

    if input.at_keyword(KeywordKind::Platform) {
        let input = input.take_keyword(KeywordKind::Platform, "platform")?;
        let (item, rest) = parse_platform(syntax_trees, input)?;
        return Ok((Item::Platform(item), rest));
    }

    if input.at_contextual("trait") {
        let (item, rest) = parse_trait_definition(syntax_trees, input, false)?;
        return Ok((Item::Trait(item), rest));
    }

    if input.at_contextual("boundary") {
        let input = input.take_contextual("boundary")?;
        if input.at_contextual("operator") {
            let input = input.take_contextual("operator")?;
            let (item, rest) = parse_operator_definition(syntax_trees, input, true)?;
            return Ok((Item::Operator(item), rest));
        }
        let (item, rest) = parse_trait_definition(syntax_trees, input, true)?;
        return Ok((Item::Trait(item), rest));
    }

    Err(input.expected_one_of_here(&[
        "`use`",
        "`export`",
        "`data`",
        "`domain`",
        "`abi`",
        "`machine`",
        "`target`",
        "`capability`",
        "`invariant`",
        "`library`",
        "`measure`",
        "`host`",
        "`module`",
        "`operator`",
        "`package`",
        "`platform`",
        "`pub`",
        "`provider`",
        "`trait`",
        "`wire data`",
        "`boundary operator`",
        "`boundary trait`",
    ]))
}

fn parse_wire_data_definition<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, WireDataDefinition> {
    let (name, mut input) = input.take_identifier()?;
    let encoding = if input.at_contextual("encoding") {
        input = input.take_contextual("encoding")?;
        let (encoding, rest) = input.take_identifier()?;
        input = rest;
        Some(encoding)
    } else {
        None
    };
    input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
    let (members, input) = parse_wire_data_members(syntax_trees, input)?;
    let input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;

    Ok((
        WireDataDefinition {
            name,
            encoding,
            members,
        },
        input,
    ))
}

fn parse_wire_data_members<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    mut input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, omega_core::arena::HandleSpan<WireDataMember>> {
    let mut member_start = omega_core::arena::Handle::invalid();
    let mut member_count = 0u32;

    while !input.at_punctuation(PunctuationKind::RightBrace) {
        let (member, rest) = parse_wire_data_member(syntax_trees, input)?;
        let handle = syntax_trees.items.append_wire_data_member(member);
        if member_count == 0 {
            member_start = handle;
        }
        member_count = member_count
            .checked_add(1)
            .expect("wire data member span count overflow");
        input = rest;
    }

    let members = if member_count == 0 {
        omega_core::arena::HandleSpan::empty()
    } else {
        omega_core::arena::HandleSpan::from_parts(member_start, member_count)
    };
    Ok((members, input))
}

fn parse_wire_data_member<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, WireDataMember> {
    if input.at_contextual("reserved") {
        let input = input.take_contextual("reserved")?;
        let (number, input) = input.take_integer()?;
        let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
        return Ok((WireDataMember::Reserved(WireDataReserved { number }), input));
    }

    if input.at_contextual("version") {
        let input = input.take_contextual("version")?;
        let (name, input) = input.take_identifier()?;
        let input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;
        let (members, input) = parse_wire_data_members(syntax_trees, input)?;
        let input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
        return Ok((
            WireDataMember::Version(WireDataVersion { name, members }),
            input,
        ));
    }

    let (number, input) = input.take_integer()?;
    let input = input.take_punctuation(PunctuationKind::Colon, ":")?;
    let (name, input) = input.take_identifier()?;
    let input = input.take_punctuation(PunctuationKind::Colon, ":")?;
    let (type_reference, input) = parse_type_reference_handle(syntax_trees, input)?;
    let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;

    Ok((
        WireDataMember::Field(WireDataField {
            number,
            name,
            type_reference,
        }),
        input,
    ))
}

fn parse_host_provider_definition<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, HostProviderDefinition> {
    let (target, input) = input.take_identifier()?;
    let input = input.take_contextual("provides")?;
    let (boundary_trait, mut input) = parse_path_handle_span(input, |member| {
        syntax_trees.items.append_identifier_path_member(member)
    })?;
    input = input.take_punctuation(PunctuationKind::LeftBrace, "{")?;

    let mut mapping_start = omega_core::arena::Handle::invalid();
    let mut mapping_count = 0u32;
    while !input.at_punctuation(PunctuationKind::RightBrace) {
        let (machine, rest) = input.take_identifier()?;
        let rest = rest.take_punctuation(PunctuationKind::Arrow, "->")?;
        let rest = rest.take_contextual("syscall")?;
        let (value, rest) = rest.take_integer()?;
        let rest = take_optional_semicolon(rest)?;
        let handle = syntax_trees
            .items
            .append_host_provider_mapping(HostProviderMapping {
                machine,
                kind: HostProviderMappingKind::Syscall,
                value,
            });
        if mapping_count == 0 {
            mapping_start = handle;
        }
        mapping_count = mapping_count
            .checked_add(1)
            .expect("host provider mapping span count overflow");
        input = rest;
    }

    let input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;
    let mappings = if mapping_count == 0 {
        omega_core::arena::HandleSpan::empty()
    } else {
        omega_core::arena::HandleSpan::from_parts(mapping_start, mapping_count)
    };
    Ok((
        HostProviderDefinition {
            target,
            boundary_trait,
            mappings,
        },
        input,
    ))
}

fn parse_module_declaration<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, ModuleDeclaration> {
    let (path, input) = parse_dot_or_colon_path(input, |member| {
        syntax_trees.items.append_identifier_path_member(member)
    })?;
    let input = take_optional_semicolon(input)?;

    Ok((ModuleDeclaration { path }, input))
}

fn parse_package_declaration<'tokens, 'source>(
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
    mut append_member: impl FnMut(
        omega_syntax_trees::identifier::Identifier,
    ) -> omega_core::arena::Handle<
        omega_syntax_trees::identifier::Identifier,
    >,
) -> ParseResult<
    'tokens,
    'source,
    omega_core::arena::HandleSpan<omega_syntax_trees::identifier::Identifier>,
> {
    let (first, mut rest) = input.take_identifier()?;
    let start = append_member(first);
    let mut count = 1u32;

    loop {
        if rest.at_punctuation(PunctuationKind::Dot) {
            rest = rest.take_punctuation(PunctuationKind::Dot, ".")?;
        } else if rest.at_punctuation(PunctuationKind::ColonColon) {
            rest = rest.take_punctuation(PunctuationKind::ColonColon, "::")?;
        } else {
            break;
        }

        let (member, next) = rest.take_identifier()?;
        append_member(member);
        count = count
            .checked_add(1)
            .expect("module/package path member span count overflow");
        rest = next;
    }

    Ok((
        omega_core::arena::HandleSpan::from_parts(start, count),
        rest,
    ))
}

fn take_optional_semicolon<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> Result<Input<'tokens, 'source>, crate::parse_error::ParseError> {
    if input.at_punctuation(PunctuationKind::Semicolon) {
        input.take_punctuation(PunctuationKind::Semicolon, ";")
    } else {
        Ok(input)
    }
}

/// Parses `provider <QualifiedName> : <Category>;` (frozen Wave 0 decision #4).
fn parse_provider_declaration<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, ProviderDeclaration> {
    let (name, input) = parse_path_handle_span(input, |member| {
        syntax_trees.items.append_identifier_path_member(member)
    })?;
    let input = input.take_punctuation(PunctuationKind::Colon, ":")?;
    let (category_name, input) = input.take_identifier()?;
    let category = ProviderCategory::from_name(category_name.as_str()).ok_or_else(|| {
        input.error_here(format!(
            "unknown provider category `{}`; expected one of {}",
            category_name.as_str(),
            ProviderCategory::ALL
                .iter()
                .map(|category| category.name())
                .collect::<Vec<_>>()
                .join(", ")
        ))
    })?;
    let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;

    Ok((ProviderDeclaration { name, category }, input))
}
