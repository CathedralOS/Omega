use crate::declarations::namespaces::{parse_module_declaration, parse_package_declaration};

use crate::declarations::capabilities::parse_capability_definition;
use crate::declarations::constants::parse_const_definition;
use crate::declarations::data::{parse_boundary_data_definition, parse_data_definition};
use crate::declarations::domains::parse_domain_definition;
use crate::declarations::imports::parse_use_item;
use crate::declarations::machines::parse_machine;
use crate::declarations::mathematical_bindings::parse_let_definition;
use crate::declarations::measures::parse_measure_definition;
use crate::declarations::operators::parse_operator_definition;
use crate::declarations::propositions::parse_proposition_definition;
use crate::declarations::traits::parse_trait_definition;
use crate::expressions::parse::parse_proof_fact_expression_handle;
use crate::token_input::cursor::{Input, ParseResult};
use syntax_trees::SyntaxTrees;
use syntax_trees::identifier::Identifier;
use syntax_trees::item::{Item, MathematicalDefinition};
use tokens::{KeywordKind, PunctuationKind};

pub(crate) enum ParsedDeclaration {
    Item(Item),
    Mathematical(MathematicalDefinition),
}

pub(crate) fn starts_following_item(syntax_trees: &mut SyntaxTrees, input: Input<'_, '_>) -> bool {
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
        return crate::declarations::conformances::parse_conformance(syntax_trees, alias, rest);
    }
    if let Ok((alias, rest)) = input.take_identifier()
        && (rest.at_punctuation(PunctuationKind::Colon)
            || rest.at_punctuation(PunctuationKind::Less))
    {
        return crate::declarations::conformances::parse_conformance(syntax_trees, alias, rest);
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
