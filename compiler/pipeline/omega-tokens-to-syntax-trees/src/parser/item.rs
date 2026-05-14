use crate::parser::capability::parse_capability_definition;
use crate::parser::data::{parse_data_definition, parse_enum_definition};
use crate::parser::input::{Input, ParseResult};
use crate::parser::invariant::parse_invariant_definition;
use crate::parser::library::parse_library_definition;
use crate::parser::machine::parse_machine;
use crate::parser::platform::parse_platform;
use crate::parser::target::parse_target_definition;
use crate::parser::trust::parse_trust_definition;
use crate::parser::use_item::parse_use_item;
use omega_syntax_trees::SyntaxTrees;
use omega_syntax_trees::item::Item;
use omega_tokens::KeywordKind;

pub(super) fn parse_item<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, Item> {
    if input.at_keyword(KeywordKind::Use) {
        let input = input.take_keyword(KeywordKind::Use, "use")?;
        let (item, rest) = parse_use_item(syntax_trees, input)?;
        return Ok((Item::Use(item), rest));
    }

    if input.at_keyword(KeywordKind::Data) {
        let input = input.take_keyword(KeywordKind::Data, "data")?;
        let (item, rest) = parse_data_definition(syntax_trees, input)?;
        return Ok((Item::Data(item), rest));
    }

    if input.at_keyword(KeywordKind::Enum) {
        let input = input.take_keyword(KeywordKind::Enum, "enum")?;
        let (item, rest) = parse_enum_definition(syntax_trees, input)?;
        return Ok((Item::Data(item), rest));
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

    if input.at_keyword(KeywordKind::Trust) {
        let input = input.take_keyword(KeywordKind::Trust, "trust")?;
        let (item, rest) = parse_trust_definition(input)?;
        return Ok((Item::TrustDefinition(item), rest));
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

    if input.at_keyword(KeywordKind::Platform) {
        let input = input.take_keyword(KeywordKind::Platform, "platform")?;
        let (item, rest) = parse_platform(syntax_trees, input)?;
        return Ok((Item::Platform(item), rest));
    }

    Err(input.expected_one_of_here(&[
        "`use`",
        "`data`",
        "`enum`",
        "`machine`",
        "`target`",
        "`trust`",
        "`capability`",
        "`invariant`",
        "`library`",
        "`platform`",
    ]))
}
