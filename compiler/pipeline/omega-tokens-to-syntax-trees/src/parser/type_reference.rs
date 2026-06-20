use crate::parse_error::ParseError;
use crate::parser::expression::parse_expression_handle_without_struct_literals;
use crate::parser::input::{Input, ParseResult};
use omega_core::arena::{Handle, HandleSpan};
use omega_syntax_trees::SyntaxTrees;
use omega_syntax_trees::expression::ExpressionNode;
use omega_syntax_trees::types::{
    FixedArrayLength, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};
use omega_tokens::{KeywordKind, PunctuationKind};

pub(super) fn parse_type_reference_handle<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, TypeReferenceHandle> {
    if input.at_punctuation(PunctuationKind::LeftParen) {
        let input = input.take_punctuation(PunctuationKind::LeftParen, "(")?;
        let input = input.take_punctuation(PunctuationKind::RightParen, ")")?;
        return Ok((syntax_trees.type_references.insert_unit(), input));
    }

    if input.at_punctuation(PunctuationKind::LeftBracket) {
        let input = input.take_punctuation(PunctuationKind::LeftBracket, "[")?;
        let (element_type, mut input) = parse_type_reference_handle(syntax_trees, input)?;
        if input.at_punctuation(PunctuationKind::Semicolon) {
            let input = input.take_punctuation(PunctuationKind::Semicolon, ";")?;
            let (length, input) = if input
                .tokens
                .first()
                .is_some_and(crate::parser::input::is_identifier_token_for_parser)
            {
                let (name, input) = input.take_identifier()?;
                // `[T; table_size()]`: a zero-argument machine call in length
                // position is CONST-EVALUATED at compile time (comptime stage 1).
                if input.at_punctuation(PunctuationKind::LeftParen) {
                    let input = input.take_punctuation(PunctuationKind::LeftParen, "(")?;
                    if !input.at_punctuation(PunctuationKind::RightParen) {
                        return Err(input.error_here(
                            "const arguments in array-length calls are not supported yet; \
                             the const-evaluated machine must take zero parameters",
                        ));
                    }
                    let input = input.take_punctuation(PunctuationKind::RightParen, ")")?;
                    (FixedArrayLength::ConstCall(name), input)
                } else {
                    (FixedArrayLength::ConstParameter(name), input)
                }
            } else {
                let (length, input) = input.take_integer()?;
                let length = usize::try_from(length)
                    .map_err(|_| input.error_here("expected non-negative array length"))?;
                (FixedArrayLength::Literal(length), input)
            };
            let input = input.take_punctuation(PunctuationKind::RightBracket, "]")?;
            let array = syntax_trees
                .type_references
                .insert(TypeReferenceNode::FixedArray {
                    element_type,
                    length,
                });
            // A fixed array can carry an encoding domain: `[u8; 32] in Utf8`.
            return apply_in_domain_suffix(syntax_trees, array, input);
        }

        let mut type_reference = syntax_trees
            .type_references
            .insert(TypeReferenceNode::Slice { element_type });

        if input.at_punctuation(PunctuationKind::Comma) {
            let input_after_comma = input.take_punctuation(PunctuationKind::Comma, ",")?;
            let (constraints, rest) =
                parse_type_constraint_handles(syntax_trees, input_after_comma)?;
            input = rest;
            type_reference = syntax_trees
                .type_references
                .insert(TypeReferenceNode::Constrained {
                    base_type: type_reference,
                    constraints,
                });
        }

        let input = input.take_punctuation(PunctuationKind::RightBracket, "]")?;
        // A slice can carry an encoding domain: `[u8] in Utf8` (ch8).
        return apply_in_domain_suffix(syntax_trees, type_reference, input);
    }

    if input.at_keyword(KeywordKind::SelfType) {
        let input = input.take_keyword(KeywordKind::SelfType, "Self")?;
        return Ok((syntax_trees.type_references.insert_self_type(), input));
    }

    if input.at_contextual("dyn") {
        let input = input.take_contextual("dyn")?;
        let (trait_name, input) = input.take_identifier()?;
        return Ok((
            syntax_trees
                .type_references
                .insert(TypeReferenceNode::DynamicTrait(trait_name)),
            input,
        ));
    }

    let (mut base_name, mut input) = input.take_identifier()?;

    // A historical-version shape (`Counter::v1`) is a nameable type: consume a
    // single `::vN` selector segment and fold it into the type name, matching
    // the names that `data ... { version vN { ... } }` blocks introduce. Other
    // `::` segments are left untouched (and keep failing to parse here).
    if input.at_punctuation(PunctuationKind::ColonColon) {
        let after_separator = input.take_punctuation(PunctuationKind::ColonColon, "::")?;
        if after_separator
            .tokens
            .first()
            .is_some_and(crate::parser::input::is_identifier_token_for_parser)
        {
            let (segment, rest) = after_separator.take_identifier()?;
            if omega_syntax_trees::item::is_version_selector(segment.as_str()) {
                base_name = omega_syntax_trees::identifier::Identifier::generated(format!(
                    "{}::{}",
                    base_name.as_str(),
                    segment.as_str()
                ));
                input = rest;
            }
        }
    }

    let mut type_reference = if input.at_punctuation(PunctuationKind::Less) {
        input = input.take_punctuation(PunctuationKind::Less, "<")?;
        let mut argument_start = Handle::invalid();
        let mut argument_count = 0u32;
        let mut first_argument = TypeReferenceHandle::invalid();

        loop {
            let (argument, rest) = parse_type_reference_handle(syntax_trees, input)?;
            if argument_count == 0 {
                first_argument = argument;
            }
            let handle = syntax_trees
                .type_references
                .append_type_reference_handle(argument);
            if argument_count == 0 {
                argument_start = handle;
            }
            argument_count = argument_count
                .checked_add(1)
                .expect("type reference argument span count overflow");
            input = rest;

            if input.at_punctuation(PunctuationKind::Comma) {
                input = input.take_punctuation(PunctuationKind::Comma, ",")?;
                continue;
            }

            break;
        }

        input = input.take_punctuation(PunctuationKind::Greater, ">")?;
        let arguments = if argument_count == 0 {
            HandleSpan::empty()
        } else {
            HandleSpan::from_parts(argument_start, argument_count)
        };
        // CONCURRENCY STAGE 1: `Join<T>` ERASES TO `T` here in the parser,
        // mirroring the synchronous-spawn desugar (`expression/spawn.rs`):
        // the spawned call completes at the spawn site, so the handle is
        // structurally the completed result. `Join` is a reserved data-type
        // name (rejected at `data` definitions), so the fold is never
        // ambiguous with user generics. When a real scheduler lands, this
        // fold is replaced by a synthesized container definition (the
        // `Versioned<T>` precedent in omega-core/src/versioning.rs).
        if base_name.as_str() == "Join" && argument_count == 1 {
            first_argument
        } else if base_name.as_str() == "Slice" && argument_count == 1 {
            // `Slice<T>` is the canonical slice type; `[T]` is its alias. Both fold
            // to the same `Slice` node, so the spellings are interchangeable
            // (`Slice<u8> in Utf8` == `[u8] in Utf8`). Like `Join`, `Slice` is a
            // reserved type name so the fold is unambiguous with user generics.
            syntax_trees
                .type_references
                .insert(TypeReferenceNode::Slice {
                    element_type: first_argument,
                })
        } else {
            syntax_trees
                .type_references
                .insert(TypeReferenceNode::Generic {
                    base_name,
                    arguments,
                })
        }
    } else {
        syntax_trees.type_references.insert_named(base_name)
    };

    if input.at_punctuation(PunctuationKind::LeftBracket) {
        let (constraints, rest) = parse_type_constraint_handles(syntax_trees, input)?;
        input = rest;
        type_reference = syntax_trees
            .type_references
            .insert(TypeReferenceNode::Constrained {
                base_type: type_reference,
                constraints,
            });
    }

    let (type_reference, input) = apply_in_domain_suffix(syntax_trees, type_reference, input)?;
    Ok((type_reference, input))
}

/// Apply an optional `in <Domain>` suffix to a just-parsed type reference.
///
/// The name is an arithmetic overflow domain (`Wrapping`/`Saturating`/`Trapping`;
/// frozen decision 17) when it matches one -- opting a primitive into defined
/// overflow behaviour while bare arithmetic stays exact (a proof obligation).
/// Otherwise it is a DECLARED encoding domain on the carrier (`[u8] in Utf8`;
/// ch8 "domains over carriers"). Either way the domain rides as a constraint on
/// a `Constrained` type-reference -- the same carrier as `a..=b` -- so
/// layout/codegen see through to the base type.
///
/// An unknown encoding-domain name is NOT a parse error: validation resolves it
/// against `domain ...::Name` declarations (rejecting typos with a clear
/// message). `in` is the contextual membership keyword; in TYPE position nothing
/// else consumes a trailing `in`, so this suffix is additive. Factored out so it
/// applies after slice/array returns too (`[u8] in Utf8`), not just named types.
fn apply_in_domain_suffix<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    type_reference: TypeReferenceHandle,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, TypeReferenceHandle> {
    if !input.at_contextual("in") {
        return Ok((type_reference, input));
    }
    let after_in = input.take_contextual("in")?;
    let (domain_name, rest) = after_in.take_identifier()?;
    let constraint =
        match omega_core::arithmetic::ArithmeticDomain::from_name(domain_name.as_str()) {
            Some(domain) => TypeConstraintNode::ArithmeticDomain(domain),
            None => TypeConstraintNode::Domain(domain_name),
        };
    let constraint = syntax_trees.type_references.append_constraint(constraint);
    let type_reference = syntax_trees
        .type_references
        .insert(TypeReferenceNode::Constrained {
            base_type: type_reference,
            constraints: HandleSpan::from_parts(constraint, 1),
        });
    Ok((type_reference, rest))
}

pub(super) fn parse_type_reference_handle_allowing_borrow<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, TypeReferenceHandle> {
    let (is_reference, lifetime, is_mutable, is_relaxed, input) =
        if input.at_punctuation(PunctuationKind::Ampersand) {
            let input = input.take_punctuation(PunctuationKind::Ampersand, "&")?;
            // Explicit lifetime (`&'buf T`, `&'buf mut T`); frozen decision 15
            // stage 2. The tick precedes `mut`/`relaxed`, mirroring Rust's
            // `&'a mut T`. A bare `&T` keeps the elided (`None`) form.
            let (lifetime, input) = parse_optional_lifetime(input)?;
            let (is_mutable, input) = if input.at_contextual("mut") {
                (true, input.take_contextual("mut")?)
            } else {
                (false, input)
            };
            let (is_relaxed, input) = if input.at_contextual("relaxed") {
                (true, input.take_contextual("relaxed")?)
            } else {
                (false, input)
            };
            (true, lifetime, is_mutable, is_relaxed, input)
        } else {
            (false, None, false, false, input)
        };

    let (type_reference, input) = parse_type_reference_handle(syntax_trees, input)?;
    let type_reference = if is_reference {
        syntax_trees
            .type_references
            .insert(TypeReferenceNode::Reference {
                referee: type_reference,
                is_mutable,
                is_relaxed,
                lifetime,
            })
    } else {
        type_reference
    };

    Ok((type_reference, input))
}

/// Parse a `'name` lifetime if present, returning its identifier. The lexer
/// emits `'` as `Apostrophe` punctuation immediately followed by the name
/// identifier (frozen decision 15 stage 2).
pub(super) fn parse_optional_lifetime<'tokens, 'source>(
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, Option<omega_syntax_trees::identifier::Identifier>> {
    if !input.at_punctuation(PunctuationKind::Apostrophe) {
        return Ok((None, input));
    }
    let input = input.take_punctuation(PunctuationKind::Apostrophe, "'")?;
    let (name, input) = input.take_identifier()?;
    Ok((Some(name), input))
}

pub(super) fn parse_type_constraint_handles<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    input: Input<'tokens, 'source>,
) -> ParseResult<'tokens, 'source, HandleSpan<TypeConstraintNode>> {
    let mut input = input.take_punctuation(PunctuationKind::LeftBracket, "[")?;
    let mut constraint_start = Handle::invalid();
    let mut constraint_count = 0u32;

    if !input.at_punctuation(PunctuationKind::RightBracket) {
        loop {
            let constraint = if input.at_contextual("range") {
                return Err(input.error_here(
                    "the `range<a, b>` constraint syntax is removed; use `a..=b` (inclusive) or `a..b` (exclusive)",
                ));
            } else if input.at_name_like() {
                let (name, rest) = input.take_identifier()?;
                input = rest;
                TypeConstraintNode::Named(name)
            } else {
                // Range refinement: `min..=max` (inclusive) or `min..max` (exclusive).
                // The node stores an INCLUSIVE maximum, so an exclusive bound is
                // normalised to `max - 1` at parse (its upper bound must therefore be
                // an integer literal); every downstream consumer keeps reading an
                // inclusive maximum unchanged.
                let (minimum, rest) =
                    parse_expression_handle_without_struct_literals(syntax_trees, input)?;
                if rest.at_punctuation(PunctuationKind::DotDotEqual) {
                    let rest = rest.take_punctuation(PunctuationKind::DotDotEqual, "..=")?;
                    let (maximum, rest) =
                        parse_expression_handle_without_struct_literals(syntax_trees, rest)?;
                    input = rest;
                    TypeConstraintNode::Range { minimum, maximum }
                } else if rest.at_punctuation(PunctuationKind::DotDot) {
                    let rest = rest.take_punctuation(PunctuationKind::DotDot, "..")?;
                    let (end_exclusive, rest) = rest.take_integer()?;
                    input = rest;
                    let maximum = syntax_trees
                        .expressions
                        .insert(ExpressionNode::Integer(end_exclusive - 1));
                    TypeConstraintNode::Range { minimum, maximum }
                } else {
                    return Err(
                        rest.error_here("range constraint requires `..` or `..=` between bounds")
                    );
                }
            };

            let handle = syntax_trees.type_references.append_constraint(constraint);
            if constraint_count == 0 {
                constraint_start = handle;
            }
            constraint_count = constraint_count
                .checked_add(1)
                .expect("type constraint span count overflow");

            if input.at_punctuation(PunctuationKind::Comma) {
                input = input.take_punctuation(PunctuationKind::Comma, ",")?;
                continue;
            }

            break;
        }
    }

    input = input.take_punctuation(PunctuationKind::RightBracket, "]")?;
    let constraints = if constraint_count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(constraint_start, constraint_count)
    };
    Ok((constraints, input))
}
