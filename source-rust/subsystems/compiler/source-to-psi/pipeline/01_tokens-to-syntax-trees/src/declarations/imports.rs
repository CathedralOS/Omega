use crate::errors::parse_error::ParseError;
use crate::token_input::cursor::{Input, ParseResult, parse_path_handle_span};
use arena::HandleSpan;
use syntax_trees::SyntaxTrees;
use syntax_trees::identifier::Identifier;
use syntax_trees::item::UseItem;
use tokens::{PunctuationKind as P, TokenKind};

pub(crate) fn parse_use_item<'tokens, 'source>(
    trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, UseItem> {
    // Existing library sources still use namespace imports during migration.
    if let Ok((_, next)) = input.take_identifier()
        && (next.at_path_member() || next.at_punctuation(P::Semicolon))
    {
        let (path, rest) = parse_path_handle_span(input, |name| {
            trees.items.append_identifier_path_member(name)
        })?;
        return Ok((
            UseItem::Namespace { path },
            rest.take_punctuation(P::Semicolon, ";")?,
        ));
    }
    let (root, mut rest) = if input.at_punctuation(P::LeftBrace) {
        let (root, rest) = input
            .take_punctuation(P::LeftBrace, "{")?
            .take_identifier()?;
        let rest = rest
            .take_punctuation(P::RightBrace, "}")?
            .take_punctuation(P::Slash, "/")?;
        (Some(root), rest)
    } else {
        (None, input)
    };
    let mut components = HandleSpan::empty();
    let stem = loop {
        let beginning = rest;
        let (component, next) = file_component(rest)?;
        let implicit_alias = beginning.tokens.first().is_some_and(|token| {
            token.kind == TokenKind::Identifier && token.span == component.source_span().span
        });
        let is_last = next.at_punctuation(P::Dot);
        let stem = (is_last && implicit_alias).then(|| component.clone());
        components.push_contiguous(trees.items.append_identifier_path_member(component));
        if is_last {
            rest = next.take_punctuation(P::Dot, ".")?.take_contextual("omg")?;
            break stem;
        }
        rest = next.take_punctuation(P::Slash, "/ or the .omg filename extension")?;
    };
    let alias = if rest.at_contextual("as") {
        let at_alias = rest.take_contextual("as")?;
        if !at_alias
            .tokens
            .first()
            .is_some_and(|token| token.kind == TokenKind::Identifier)
        {
            return Err(at_alias.error_here("expected a file import alias"));
        }
        let (alias, next) = at_alias.take_identifier()?;
        rest = next;
        alias
    } else {
        stem.ok_or_else(|| rest.error_here("this filename needs an explicit file alias (as name)"))?
    };
    if symbols::BuiltinTypeAtom::is_builtin_type_spelling(alias.as_str(trees)) {
        return Err(ParseError::at_source_span(
            format!("file alias `{}` names a builtin type", alias.as_str(trees)),
            alias.source_span(),
        ));
    }
    Ok((
        UseItem::File {
            only_product_operands: false,
            root,
            components,
            alias,
            span: input.source_span_until(rest),
        },
        rest.take_punctuation(P::Semicolon, ";")?,
    ))
}

fn file_component<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, Identifier> {
    let mut rest = input;
    let mut end = None;
    while let Some(token) = rest.tokens.first() {
        let spelling = rest.spelling(token);
        if end.is_some_and(|end| end != token.span.start)
            || !language_core::logical_filename::is_component(spelling)
        {
            break;
        }
        end = Some(token.span.end);
        rest = rest.expect_token()?.1;
    }
    if end.is_none() {
        return Err(input.error_here("expected a logical filename component"));
    }
    Ok((input.name_until(rest), rest))
}

/// Core's packed-integer instruction contracts are reached only through `asm`.
/// A written import of that file would turn the instructions back into
/// ordinary callable core API.
pub(crate) fn reject_packed_contract_imports(
    trees: &SyntaxTrees,
    roots: &syntax_trees::ParsedRootItems,
) -> Result<(), ParseError> {
    use language_core::inline_assembly::{
        PACKED_INTEGER_CONTRACTS_COMPONENTS, PACKED_INTEGER_CONTRACTS_ROOT,
    };
    for handle in &roots.items[roots.import_range.clone()] {
        let (root, components, span) = match trees.root_item(*handle) {
            syntax_trees::item::Item::Use(UseItem::File {
                root,
                components,
                span,
                ..
            }) => (root.as_ref().map(|root| root.as_str(trees)), *components, *span),
            syntax_trees::item::Item::Use(UseItem::Namespace { path }) => {
                let Some(head) = trees.items.identifier_path_members(*path).first() else {
                    continue;
                };
                (None, *path, head.source_span())
            }
            _ => continue,
        };
        let written: Vec<&str> = trees
            .items
            .identifier_path_members(components)
            .iter()
            .map(|component| component.as_str(trees))
            .collect();
        let names_contracts = match root {
            Some(root) => {
                root == PACKED_INTEGER_CONTRACTS_ROOT && written == PACKED_INTEGER_CONTRACTS_COMPONENTS
            }
            None => {
                written.first() == Some(&PACKED_INTEGER_CONTRACTS_ROOT)
                    && written[1..] == PACKED_INTEGER_CONTRACTS_COMPONENTS
            }
        };
        if names_contracts {
            return Err(ParseError::at_source_span(
                "core `assembly_packed_integer.omg` holds the checked asm instruction contracts; \
                 write the instruction in an `asm { ... }` region instead of importing the file",
                span,
            ));
        }
    }
    Ok(())
}

/// Import the packed-integer instruction contracts for this file's `asm`
/// regions under the alias the assembly desugar names.
pub(crate) fn import_packed_contracts(
    trees: &mut SyntaxTrees,
    roots: &mut syntax_trees::ParsedRootItems,
    site: source_text::SourceSpan,
) {
    use language_core::inline_assembly::{
        PACKED_INTEGER_CONTRACTS_ALIAS, PACKED_INTEGER_CONTRACTS_COMPONENTS,
        PACKED_INTEGER_CONTRACTS_ROOT,
    };
    let mut components = HandleSpan::empty();
    for component in PACKED_INTEGER_CONTRACTS_COMPONENTS {
        components.push_contiguous(
            trees
                .items
                .append_identifier_path_member(Identifier::new(component, site)),
        );
    }
    let item = syntax_trees::item::Item::Use(UseItem::File {
        root: Some(Identifier::new(PACKED_INTEGER_CONTRACTS_ROOT, site)),
        components,
        alias: Identifier::new(PACKED_INTEGER_CONTRACTS_ALIAS, site),
        only_product_operands: false,
        span: site,
    });
    let handle = trees.push_root_item(item);
    let position = if roots.import_range.is_empty() {
        0
    } else {
        roots.import_range.end
    };
    roots.items.insert(position, handle);
    if roots.import_range.is_empty() {
        roots.import_range = position..position + 1;
    } else {
        roots.import_range.end += 1;
    }
}

pub(crate) fn check_file_aliases(
    trees: &SyntaxTrees,
    roots: &syntax_trees::ParsedRootItems,
    source: &str,
) -> Result<(), ParseError> {
    let mut aliases = roots.items[roots.import_range.clone()]
        .iter()
        .filter_map(|root| {
            if let syntax_trees::item::Item::Use(UseItem::File { alias, .. }) =
                trees.root_item(*root)
            {
                Some(alias)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    aliases.sort_by_key(|alias| (alias.as_str(trees), alias.source_span().span.start));
    for pair in aliases.windows(2) {
        let name = pair[0].as_str(trees);
        if name == pair[1].as_str(trees) {
            let first = source_text::SourcePosition::of(source, pair[0].source_span().span.start);
            return Err(ParseError::at_source_span(
                format!(
                    "duplicate file import alias `{name}`, first imported at line {}, column {}",
                    first.line, first.column
                ),
                pair[1].source_span(),
            ));
        }
    }
    Ok(())
}

/// A root binding names a Product declaration without asking Build to check that
/// file as host code. Keep the ordinary import if the alias also appears in a
/// type, call, contract or any other use outside those operands.
pub(crate) fn mark_product_operand_imports(
    trees: &mut SyntaxTrees,
    roots: &syntax_trees::ParsedRootItems,
    qualifiers: &crate::token_input::file_qualifiers::FileQualifiers<'_>,
) {
    use syntax_trees::item::Item;

    let operands = qualifiers.only_product_operands(trees, roots);
    if operands.is_empty() {
        return;
    }
    for handle in &roots.items[roots.import_range.clone()] {
        let Item::Use(UseItem::File { alias, .. }) = trees.root_item(*handle) else { continue };
        if operands.contains(alias.as_str(trees)) {
            let mut item = trees.root_item(*handle).clone();
            if let Item::Use(UseItem::File { only_product_operands, .. }) = &mut item {
                *only_product_operands = true;
            }
            trees.items.replace_item(*handle, item);
        }
    }
}
