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
use crate::declarations::data::{parse_boundary_data_definition, parse_data_definition};
use crate::declarations::machines::parse_machine;
use crate::input::token_cursor::{Input, ParseResult};
use syntax_trees::SyntaxTrees;
use syntax_trees::identifier::Identifier;
use syntax_trees::item::{Item, MathematicalDefinition};
use tokens::{KeywordKind, PunctuationKind};

/// One parsed top-level declaration. `let`/`boundary let` mathematical
/// declarations are not [`Item`] variants yet — the symbol-resolution
/// lowering that admits declarations is a separately owned leg — so they ride
/// their own channel into `SyntaxTreeRoots::mathematical_definitions`.
pub(crate) enum ParsedDeclaration {
    Item(Item),
    Mathematical(MathematicalDefinition),
}

/// Contract semicolons also terminate bodyless declarations. Identify the next
/// declaration by its head, not by a name alone: contextual keywords and
/// target-named values remain ordinary expressions in proof facts. A named
/// implementation needs its complete telescope followed by `satisfies`; `<`
/// alone is also a comparison and cannot terminate the current contract.
pub(crate) fn starts_following_item(input: Input<'_, '_>) -> bool {
    if input.at_keyword(KeywordKind::Pub) {
        let after = Input::new(input.source_id, input.tokens.get(1..).unwrap_or_default());
        return starts_following_item(after);
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
        // These heads overlap expressions: `operator + offset > 0` and
        // `domain[0] > 0` are facts, while fixed-token operators and bracketed
        // domain carriers are declarations. Use their actual bodyless grammar
        // in a disposable arena instead of maintaining another type/expression
        // parser. This probe never scans a following executable machine body,
        // and never publishes speculative nodes into the caller's syntax.
        let mut probe = SyntaxTrees::new(input.source_id);
        // A token-only operator signature can also be a whole fact:
        // `operator < limit;`. Preserve the existing fact interpretation
        // whenever the expression grammar consumes that complete fact. A
        // declaration with typed parameters cannot be such an expression.
        if crate::expressions::parse_expression::parse_proof_fact_expression_handle(
            &mut probe, input,
        )
        .is_ok_and(|(_, rest)| rest.at_punctuation(PunctuationKind::Semicolon))
        {
            return false;
        }
        return parse_declared_item(&mut probe, input).is_ok();
    }
    if !input.at_contextual("boundary") {
        return false;
    }
    let after = Input::new(input.source_id, input.tokens.get(1..).unwrap_or_default());
    after.at_keyword(KeywordKind::Machine)
        || after.at_keyword(KeywordKind::Data)
        || after.at_keyword(KeywordKind::Let)
        || after.at_contextual("requirement")
        || after.at_contextual("operator")
        || after.at_contextual("trait")
}

/// Probe only a named implementation's unambiguous head. The real parser owns
/// binder validity and diagnostics. Parenthesized/array type operands may
/// contain comparisons or shifts, so their punctuation does not close the
/// outer telescope. No syntax nodes are allocated during contract lookahead.
fn named_implementation_prefix<'tokens, 'source>(
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
            let after = Input::new(input.source_id, &rest.tokens[token_index + 1..]);
            return after.at_contextual("satisfies").then_some((name, rest));
        }
    }
    None
}

/// Shared by declaration dispatch and contract lookahead. Consume only the
/// target/modifier prefix; the machine parser still owns the complete head.
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

pub(crate) fn parse_item<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, ParsedDeclaration> {
    if input.at_keyword(KeywordKind::Pub) {
        let input = input.take_keyword(KeywordKind::Pub, "pub")?;
        // Data retains this bit because public structural declarations publish
        // their source shape, including numbered fields.
        // Domains and propositions retain it because a public transparent
        // alias may not publish a private constituent; machines retain it because public checked
        // bodies publish strict authority and operational ceilings. Traits
        // retain it as source-level API metadata independent of trait identity.
        // Mathematical declarations retain it because a `boundary let`
        // assumption's declaration identity is its trust surface, and a
        // transparent `let` may not publish a private constituent.
        let (mut declaration, rest) = parse_item(syntax_trees, input)?;
        match &mut declaration {
            ParsedDeclaration::Item(item) => match item {
                Item::Conformance(conformance) => conformance.is_public = true,
                Item::Const(constant) => constant.is_public = true,
                Item::Data(data) => data.is_public = true,
                Item::Domain(domain) => domain.is_public = true,
                Item::Machine(machine) => machine.is_public = true,
                Item::Operator(operator) => operator.is_public = true,
                Item::Proposition(proposition) => proposition.is_public = true,
                Item::Trait(trait_definition) => trait_definition.is_public = true,
                _ => {
                    return Err(rest.error_here(
                        "`pub` is not yet retained for this declaration kind; refusing to compile a silently private API",
                    ));
                }
            },
            ParsedDeclaration::Mathematical(definition) => definition.is_public = true,
        }
        return Ok((declaration, rest));
    }

    // Top-level mathematical `let` — the selected surface for named
    // mathematical definitions (mathematical_bindings.md). `boundary let`
    // rides the `boundary` arm below.
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

/// The `boundary` continuation after its contextual keyword: the exported
/// callable, trusted requirement and data/trait forms, plus `boundary let`
/// mathematical assumptions.
fn parse_boundary_item<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, ParsedDeclaration> {
    // `boundary let name<binders>(params): Type;` — a named mathematical
    // assumption with no body and no implementation-search slot. Its exact
    // declaration identity is the trust surface receivers admit.
    if input.at_keyword(KeywordKind::Let) {
        let input = input.take_keyword(KeywordKind::Let, "let")?;
        let (definition, rest) = parse_let_definition(syntax_trees, input, true)?;
        return Ok((ParsedDeclaration::Mathematical(definition), rest));
    }
    if input.at_contextual("requirement") {
        let input = input.take_contextual("requirement")?;
        let (mut item, rest) = parse_machine(syntax_trees, input)?;
        if item.spelling.is_some() {
            return Err(rest.error_here(
                "a `boundary requirement` does not take a fixed operator token; a \
                 token-bearing boundary requirement is spelled bodyless \
                 `boundary machine + Name(...);`",
            ));
        }
        if !item.satisfies.is_empty() {
            return Err(rest.error_here(
                "a top-level `boundary requirement` declares a required operation and cannot itself carry a `satisfies` clause",
            ));
        }
        if !item.bodyless {
            return Err(rest.error_here(
                "a top-level `boundary requirement` is bodyless and must end with `;`, not a checked `{ ... }` body",
            ));
        }
        item.is_top_level_boundary_requirement = true;
        return Ok((ParsedDeclaration::Item(Item::Machine(item)), rest));
    }
    if input.at_keyword(KeywordKind::Data) {
        let input = input.take_keyword(KeywordKind::Data, "data")?;
        let (item, rest) = parse_boundary_data_definition(syntax_trees, input)?;
        return Ok((ParsedDeclaration::Item(Item::Data(item)), rest));
    }
    // THE EXPORTED CALLABLE (settled 2026-07-04): `boundary machine ...`
    // declares "we export this as a callable surface" -- the entry, a
    // callback, an interrupt handler. Its parameter list is the
    // boundary-trusted shape over the arrival bytes; its calling plan is
    // inferred from the image subsystem.
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
        let (item, rest) = parse_data_definition(syntax_trees, input)?;
        return Ok((Item::Data(item), rest));
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
        // Boundary calling policy belongs to the satisfied requirement,
        // not an ABI string or an image-subsystem inference.
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
        // A bodyless machine is legal as an EXTERNAL LEAF -- `satisfies
        // Requirement via <Binding>;` -- whose realization is the binding, or
        // as a bare tokenless signature that symbol resolution admits only
        // when it is an exact compiler-catalog primitive declared by the
        // sealed toolchain source (executable supply: "Exact compiler-owned
        // bodyless machine ... its authorized closed-catalog realization").
        // Any other bodyless machine rejects there with the boundary-form
        // guidance; the grammar itself stays neutral about which source
        // owns a name.
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
             default domain (`where` or field constraints) and behavioral facts in \
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
        // RETIRED (PRV4/P4d, ruling 2026-07-17): platform blocks are the
        // pre-boundary-culture host surface. A host service is a
        // `boundary trait` (declared service-reach rows, ordinary requires/
        // ensures); Console's promotion proved the migration is a
        // spelling change.
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

    // Target-scoped ordinary and boundary machines share the same head grammar.
    // Dispatch uses the same prefix probe as a preceding contract's terminator.
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

    // A named implementation owns its complete requirement map and telescope.
    // Its name is code identity, never a data subject or implicit argument.
    if let Some((alias, rest)) = named_implementation_prefix(input) {
        return super::conformance::parse_conformance(syntax_trees, alias, rest);
    }
    // In declaration position these incomplete/retired heads still deserve
    // the conformance parser's diagnostic. They are not contract boundaries:
    // an ordinary comparison fact also starts with a name followed by `<`.
    if let Ok((alias, rest)) = input.take_identifier()
        && (rest.at_punctuation(PunctuationKind::Colon)
            || rest.at_punctuation(PunctuationKind::Less))
    {
        return super::conformance::parse_conformance(syntax_trees, alias, rest);
    }

    // Retired subjectless order. Keeping this branch gives a directed
    // migration instead of letting the generic item diagnostic obscure the
    // evidence identity that must move to the front.
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
        "`pub`",
        "`trait`",
        "`boundary let`",
        "`boundary operator`",
        "`boundary requirement`",
        "`boundary data`",
        "`boundary trait`",
    ]))
}
