use crate::parser::capability::parse_capability_definition;
use crate::parser::const_item::parse_const_definition;
use crate::parser::data::{parse_boundary_data_definition, parse_data_definition};
use crate::parser::domain::parse_domain_definition;
use crate::parser::export_item::parse_export_item;
use crate::parser::input::{Input, ParseResult, parse_path_handle_span};
use crate::parser::invariant::parse_invariant_definition;
use crate::parser::library::parse_library_definition;
use crate::parser::machine::parse_machine;
use crate::parser::measure::parse_measure_definition;
use crate::parser::operator::parse_operator_definition;
use crate::parser::target::parse_target_definition;
use crate::parser::trait_definition::parse_trait_definition;
use crate::parser::type_reference::{
    parse_type_reference_handle, parse_type_reference_handle_allowing_borrow,
};
use crate::parser::use_item::parse_use_item;
use omega_syntax_trees::SyntaxTrees;
use omega_syntax_trees::item::{
    ExternalBinding, Item, ModuleDeclaration, PackageDeclaration, ProviderDeclaration,
    WireDataDefinition, WireDataField, WireDataMember, WireDataReserved, WireDataVersion,
};
use omega_syntax_trees::operator_spelling::ProviderCategory;
use omega_tokens::{KeywordKind, PunctuationKind};

pub(super) fn parse_item<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, Item> {
    if input.at_keyword(KeywordKind::Pub) {
        let input = input.take_keyword(KeywordKind::Pub, "pub")?;
        // General semantic export rules still live in explicit `export` items
        // until module scoping grows up. Domains retain this bit now because a
        // public transparent alias may not publish a private constituent.
        let (mut item, rest) = parse_item(syntax_trees, input)?;
        if let Item::Domain(domain) = &mut item {
            domain.is_public = true;
        }
        return Ok((item, rest));
    }

    if input.at_contextual("repr") {
        let input = input.take_contextual("repr")?;
        let input = input.take_contextual("native")?;
        let input = input.take_keyword(KeywordKind::Data, "data")?;
        // `repr native` explicitly requests the compiler's current native
        // field layout, so no extra representation marker is needed yet.
        let (item, rest) = parse_data_definition(syntax_trees, input)?;
        return match item {
            crate::parser::data::ParsedDataDefinition::Plain(item) => Ok((Item::Data(item), rest)),
            crate::parser::data::ParsedDataDefinition::Numbered(_) => Err(input.error_here(
                "`repr native` data cannot carry identity numbers (identity is a schema fact \
                 for serialization grammars, not a layout request)",
            )),
        };
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
        let input = input.take_contextual("export")?;
        let (item, rest) = parse_export_item(syntax_trees, input)?;
        return Ok((Item::Export(item), rest));
    }

    if input.at_keyword(KeywordKind::Data) {
        let input = input.take_keyword(KeywordKind::Data, "data")?;
        let (item, rest) = parse_data_definition(syntax_trees, input)?;
        return Ok((
            match item {
                crate::parser::data::ParsedDataDefinition::Plain(item) => Item::Data(item),
                crate::parser::data::ParsedDataDefinition::Numbered(item) => Item::WireData(item),
            },
            rest,
        ));
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

    if input.at_keyword(KeywordKind::Enum) {
        return Err(input.error_here(
            "`enum` is retired; spell alternatives as `case` members of a `data` declaration",
        ));
    }

    if input.at_contextual("abi") {
        // RETIRED (calling_plans.md): a string names nothing checkable. The
        // exported-callable surface is `boundary machine ...`; its calling
        // plan is inferred from the image subsystem (an explicit plan arrives
        // as `boundary(<Plan>)` with the calling-plan vocabulary).
        return Err(input.error_here(
            "`abi \"...\"` is retired: declare the exported callable as `boundary machine ...` (calling plans are inferred from the image; see calling_plans.md)",
        ));
    }

    if input.at_keyword(KeywordKind::Machine) {
        let input = input.take_keyword(KeywordKind::Machine, "machine")?;
        let (item, rest) = parse_machine(syntax_trees, input)?;
        // PRV4 step 1: a bodyless machine is legal when it is an EXTERNAL
        // LEAF -- `satisfies Requirement via <Binding>;` -- whose realization
        // is the binding. Every other bodyless machine remains the accepted
        // boundary form.
        let has_via = syntax_trees
            .items
            .satisfies_clauses(item.satisfies)
            .iter()
            .any(|clause| clause.via.is_some());
        if item.bodyless && !has_via {
            return Err(rest.error_here(
                "a machine without a body is the ACCEPTED boundary form -- spell it \
                 `boundary machine ...;` (chapter 10: bodyless contracts are trust \
                 rows, not ordinary machines) -- or an EXTERNAL LEAF \
                 (`satisfies Requirement via <Binding>;`)",
            ));
        }
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
        // `boundary trait` (declared effects rows, ordinary requires/
        // ensures); Console's promotion proved the migration is a
        // spelling change.
        return Err(input.error_here(
            "`platform` blocks are retired: declare the host surface as a \
             `boundary trait` with per-method `effects` rows (std's Console \
             is the model) -- same signatures, same requires/ensures, and \
             the purity checker sees the truth",
        ));
    }

    if input.at_contextual("trait") {
        let (item, rest) = parse_trait_definition(syntax_trees, input, false)?;
        return Ok((Item::Trait(item), rest));
    }

    if input.at_contextual("boundary") {
        let input = input.take_contextual("boundary")?;
        if input.at_keyword(KeywordKind::Data) {
            let input = input.take_keyword(KeywordKind::Data, "data")?;
            let (item, rest) = parse_boundary_data_definition(syntax_trees, input)?;
            return Ok((Item::Data(item), rest));
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
            return Ok((Item::Machine(item), rest));
        }
        if input.at_contextual("operator") {
            let input = input.take_contextual("operator")?;
            let (item, rest) = parse_operator_definition(syntax_trees, input, true)?;
            return Ok((Item::Operator(item), rest));
        }
        let (item, rest) = parse_trait_definition(syntax_trees, input, true)?;
        return Ok((Item::Trait(item), rest));
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
        machine.target = Some(target);
        return Ok((Item::Machine(machine), rest));
    }

    // A standalone conformance item (frozen decision 8): `Point satisfies
    // Equatable;`. No leading keyword, so it is recognized by the
    // `satisfies` contextual after a type name.
    if let Ok((type_name, rest)) = input.take_identifier()
        && rest.at_contextual("satisfies")
    {
        let rest = rest.take_contextual("satisfies")?;
        let (trait_name, mut rest) = rest.take_identifier()?;
        let trait_arguments = if rest.at_punctuation(PunctuationKind::Less) {
            rest = rest.take_punctuation(PunctuationKind::Less, "<")?;
            let mut arguments = Vec::new();
            loop {
                let (argument, next) = parse_type_reference_handle(syntax_trees, rest)?;
                arguments.push(argument);
                rest = next;
                if rest.at_punctuation(PunctuationKind::Comma) {
                    rest = rest.take_punctuation(PunctuationKind::Comma, ",")?;
                    continue;
                }
                rest = rest.take_punctuation(PunctuationKind::Greater, ">")?;
                break;
            }
            syntax_trees
                .type_references
                .insert_type_reference_handles(arguments)
        } else {
            omega_core::arena::HandleSpan::empty()
        };
        let rest = take_optional_semicolon(rest)?;
        return Ok((
            Item::Conformance(omega_syntax_trees::item::ConformanceItem {
                type_name,
                trait_name,
                trait_arguments,
            }),
            rest,
        ));
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
        "`boundary operator`",
        "`boundary data`",
        "`boundary trait`",
    ]))
}

/// Parse the body of an IDENTITY-NUMBERED data declaration (ch20): the caller
/// (`parse_data_definition`) has consumed `data Name ... {` and peeked a
/// `#N`/`retired #N` first member. Numbers are optional schema facts on plain
/// `data` -- any values, any order, sparse -- but within one declaration they
/// are all-or-nothing today (a numbered schema with an unnumbered field is a
/// guided error; the tagged grammar consumes numbers only). The legacy
/// `encoding <name>` clause died with the `wire data` form: grammar selection
/// belongs at carriers, not declarations.
pub(super) fn parse_identity_data_body<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    name: omega_syntax_trees::identifier::Identifier,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, WireDataDefinition> {
    let (members, input) = parse_wire_data_members(syntax_trees, input)?;
    let input = input.take_punctuation(PunctuationKind::RightBrace, "}")?;

    Ok((
        WireDataDefinition {
            name,
            encoding: None,
            members,
        },
        input,
    ))
}

fn parse_wire_data_members<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    mut input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, omega_core::arena::HandleSpan<WireDataMember>> {
    // Parse the whole member list before appending any of it: a nested
    // `version` block appends its own members mid-list, so appending parent
    // members as they parse would interleave the two lists and break the
    // parent span's contiguity.
    let mut parsed_members = Vec::new();

    while !input.at_punctuation(PunctuationKind::RightBrace) {
        let (member, rest) = parse_wire_data_member(syntax_trees, input)?;
        parsed_members.push(member);
        input = rest;
    }

    let mut member_start = omega_core::arena::Handle::invalid();
    let mut member_count = 0u32;
    for member in parsed_members {
        let handle = syntax_trees.items.append_wire_data_member(member);
        if member_count == 0 {
            member_start = handle;
        }
        member_count = member_count
            .checked_add(1)
            .expect("wire data member span count overflow");
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
    if input.at_contextual("retired") {
        let input = input.take_contextual("retired")?;
        let input = input.take_punctuation(PunctuationKind::Hash, "#")?;
        let (number, input) = input.take_identity()?;
        let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
        return Ok((WireDataMember::Reserved(WireDataReserved { number }), input));
    }

    if input.at_contextual("reserved") {
        // The `reserved N;` spelling died with the `wire data` form: a retired
        // identity number is a DECLARATION, not a tombstone field.
        return Err(input.error_here(
            "`reserved` is retired: tombstone an identity number with `retired #N;` (chapter 21)",
        ));
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

    if !input.at_punctuation(PunctuationKind::Hash) {
        return Err(input.error_here(
            "identity numbers are all-or-nothing within one declaration: every field of a \
             numbered data needs its `#N` prefix (`#N name: Type;`)",
        ));
    }
    let input = input.take_punctuation(PunctuationKind::Hash, "#")?;
    let (number, input) = input.take_identity()?;
    let (name, input) = input.take_identifier()?;
    let input = input.take_punctuation(PunctuationKind::Colon, ":")?;
    // A wire field may be a borrowed view (`&[u8]`): the zero-copy raw-bytes
    // field that decodes as a window into the buffer rather than owning a copy.
    let (type_reference, input) = parse_type_reference_handle_allowing_borrow(syntax_trees, input)?;
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

fn parse_provider_binding_case<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, ExternalBinding> {
    let (case, input) = input.take_identifier()?;
    match case.as_str() {
        "Syscall" => {
            let input = input.take_punctuation(PunctuationKind::LeftParen, "(")?;
            let (number, input) = input.take_integer()?;
            let input = input.take_punctuation(PunctuationKind::RightParen, ")")?;
            Ok((ExternalBinding::Syscall { number }, input))
        }
        "VtableSlot" => {
            let input = input.take_punctuation(PunctuationKind::LeftParen, "(")?;
            let (index, input) = input.take_integer()?;
            let input = input.take_punctuation(PunctuationKind::RightParen, ")")?;
            Ok((ExternalBinding::VtableSlot { index }, input))
        }
        "DllImport" => {
            let input = input.take_punctuation(PunctuationKind::LeftParen, "(")?;
            let (module, input) = input.take_string()?;
            let input = input.take_punctuation(PunctuationKind::Comma, ",")?;
            let (symbol, input) = input.take_string()?;
            let input = input.take_punctuation(PunctuationKind::RightParen, ")")?;
            Ok((ExternalBinding::DllImport { module, symbol }, input))
        }
        // A service-table function: dispatch through the `over` struct's
        // fn-ptr FIELD like a bare-field arm, but the table pointer is
        // dispatch-only -- never a wire argument (EFI table services take
        // no This; protocol/COM methods do).
        "TableFunction" => {
            let input = input.take_punctuation(PunctuationKind::LeftParen, "(")?;
            let (field, input) = input.take_identifier()?;
            let input = input.take_punctuation(PunctuationKind::RightParen, ")")?;
            Ok((ExternalBinding::TableFunction { field }, input))
        }
        // The qualified external-leaf spelling cannot use the legacy bare
        // field shorthand because `Binding::field` would look like an open
        // sum. Keep the normalized binding case explicit.
        "VtableField" => {
            let input = input.take_punctuation(PunctuationKind::LeftParen, "(")?;
            let (field, input) = input.take_identifier()?;
            let input = input.take_punctuation(PunctuationKind::RightParen, ")")?;
            Ok((ExternalBinding::VtableField { field }, input))
        }
        other => Err(input.error_here(format!(
            "unknown Binding case `{other}`: external leaves require one of \
             `Binding::Syscall(n)`, `Binding::DllImport(\"module\", \"symbol\")`, \
             `Binding::VtableSlot(n)`, `Binding::VtableField(field)`, or \
             `Binding::TableFunction(field)`"
        ))),
    }
}

/// Parse the external-realization spelling used after `via`.
///
/// The leaf names the closed compiler-known sum explicitly as
/// `Binding::Case(...)`, so a package-local declaration cannot masquerade as
/// compiler binding data.
pub(crate) fn parse_external_provider_binding<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, ExternalBinding> {
    let start = input;
    let (root, input) = input.take_identifier()?;
    if root.as_str() != "Binding" {
        return Err(start.error_here(
            "an external realization must construct the compiler-known Binding sum; \
             write `via Binding::DllImport(...)` or another qualified `Binding::Case`",
        ));
    }
    let input = input.take_punctuation(PunctuationKind::ColonColon, "::")?;
    parse_provider_binding_case(input)
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
