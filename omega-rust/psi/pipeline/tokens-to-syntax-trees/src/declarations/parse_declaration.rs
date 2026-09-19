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
    if input.at_contextual("repr") {
        let input = input.take_contextual("repr")?;
        let input = input.take_contextual("native")?;
        let input = input.take_keyword(KeywordKind::Data, "data")?;
        // `repr native` explicitly requests the compiler's current native
        // field layout, so no extra representation marker is needed yet.
        let (item, rest) = parse_data_definition(syntax_trees, input)?;
        if syntax_trees
            .items
            .data_members(item.members)
            .iter()
            .any(|member| match member {
                syntax_trees::item::DataMember::Field(field) => field.identity.is_some(),
                syntax_trees::item::DataMember::Variant(variant) => variant.identity.is_some(),
                syntax_trees::item::DataMember::Retired(_) => true,
            })
        {
            return Err(input.error_here(
                "`repr native` data cannot carry identity numbers (identity is a schema fact \
                 for serialization grammars, not a layout request)",
            ));
        }
        return Ok((Item::Data(item), rest));
    }

    if input.at_contextual("wire") {
        let after = input.take_contextual("wire")?;
        let after = after.take_keyword(KeywordKind::Data, "data")?;
        let _ = after;
        // The `wire data` declaration form is RETIRED (ch20 rewrite): field
        // identity is optional syntax on plain `data`, consumed by
        // identity-keyed grammars at carriers.
        return Err(input.error_here(
            "`wire data` is retired: declare a plain `data` with identity numbers on its fields \
             (`data Save { #1 seed: u64; retired #2; }`) -- numbers are optional schema facts, \
             consumed by identity-keyed grammars (chapter 20)",
        ));
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
        return Err(input.error_here(
            "the `export` item is retired: mark package-owned declarations `pub`, use an \
             ordinary public wrapper, or declare the package whose declarations source selects",
        ));
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

    if input.at_keyword(KeywordKind::Enum) {
        return Err(input.error_here(
            "`enum` is retired; spell alternatives as `case` members of a `data` declaration",
        ));
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
            "the legacy `library \"...\" calling_convention ... { entry ... }` block is retired; declare an exact boundary-trait requirement and realize it with `satisfies ... via` one producer machine returning `Binding::DllImport { ... }`",
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

    // Identifier-led TARGET-SCOPED boundary machine --
    // `<target> boundary machine Path(..);`. Compiler-catalog leaves retain
    // both facts independently: `boundary` says that the bodyless declaration
    // is irreducible, while the target name participates in catalog lookup.
    if input.at_identifier_then_contextual("boundary") {
        let (target, input) = input.take_identifier()?;
        let input = input.take_contextual("boundary")?;
        let input = input.take_keyword(KeywordKind::Machine, "machine")?;
        let (mut machine, rest) = parse_machine(syntax_trees, input)?;
        machine.boundary = true;
        machine.target = Some(target);
        if machine.service_reach_is_installation_bound
            && (!machine.bodyless || !machine.satisfies.is_empty())
        {
            return Err(rest.error_here(
                "`reaches <= Bound` is legal only on a top-level bodyless `boundary machine` requirement, not on a checked body or realization",
            ));
        }
        return Ok((Item::Machine(machine), rest));
    }

    // Identifier-led TARGET-SCOPED machine -- `<target> machine Path(..) {..}`
    // (fs portable-contract settle 2026-07-18): a per-target implementation of
    // a portable contract signature. The machine parses
    // ordinarily and carries its target for the pre-resolution filter. Sits
    // BELOW the contextual-led items so `boundary machine ...` (the exported
    // callable) never reads `boundary` as a target name.
    if input.at_identifier_then_contextual("machine") {
        let (target, input) = input.take_identifier()?;
        let input = input.take_keyword(KeywordKind::Machine, "machine")?;
        let (mut machine, rest) = parse_machine(syntax_trees, input)?;
        if machine.service_reach_is_installation_bound {
            return Err(rest.error_here(
                "`reaches <= Bound` is legal only on a top-level bodyless `boundary machine` requirement",
            ));
        }
        machine.target = Some(target);
        return Ok((Item::Machine(machine), rest));
    }

    // The settled whole-conformance declaration names its evidence identity
    // first: `Primary: Circle satisfies Shape { ... }`, or
    // `ConcreteEvidence: satisfies Evidence { ... }` for carrierless proof
    // evidence. The telescope is owned by that declared name and is never
    // inferred from the subject or trait arguments.
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
            "the subjectless `satisfies Trait as Name { ... }` conformance header is retired; write `Name: satisfies Trait { ... }`",
        ));
    }

    // Retired unnamed carrier order. Bodyless conformances still use attached
    // exact-requirement machines, but their evidence identity is explicit in
    // the same name-first position as closed conformances.
    if let Ok((_type_name, rest)) = input.take_identifier()
        && rest.at_contextual("satisfies")
    {
        return Err(input.error_here(
            "the unnamed `Subject satisfies Trait` conformance header is retired; write `Name: Subject satisfies Trait`",
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
