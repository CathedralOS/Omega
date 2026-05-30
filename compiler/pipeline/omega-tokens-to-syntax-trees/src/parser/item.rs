use crate::parser::capability::parse_capability_definition;
use crate::parser::data::{parse_data_definition, parse_enum_definition};
use crate::parser::domain::parse_domain_definition;
use crate::parser::export_item::parse_export_item;
use crate::parser::input::{Input, ParseResult};
use crate::parser::invariant::parse_invariant_definition;
use crate::parser::library::parse_library_definition;
use crate::parser::machine::parse_machine;
use crate::parser::measure::parse_measure_definition;
use crate::parser::operator::parse_operator_definition;
use crate::parser::platform::parse_platform;
use crate::parser::target::parse_target_definition;
use crate::parser::trait_definition::parse_trait_definition;
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
        "`enum`",
        "`machine`",
        "`target`",
        "`capability`",
        "`invariant`",
        "`library`",
        "`measure`",
        "`operator`",
        "`platform`",
        "`trait`",
        "`boundary operator`",
        "`boundary trait`",
    ]))
}
