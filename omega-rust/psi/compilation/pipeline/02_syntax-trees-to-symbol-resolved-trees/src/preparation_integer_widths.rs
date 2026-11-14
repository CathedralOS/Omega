/*
    `u<N>` and `i<N>` are the fixed-width integers written with their width as
    an argument, and `u32` is only a shorter way to write `u<32>`
    (`wiki/spec/language/numeric_values.md#width-indexed-integers`). A program
    that writes `u<32>` therefore has to end up holding the very type `u32`
    names, not a generic application that happens to compare equal to it.

    This pass makes that true before any symbol exists: an application whose
    width is a closed positive integer is replaced by the decimal shorthand,
    so every later stage sees one spelling of one carrier and none of them
    learns about the indexed form. The width is read from an integer literal
    or from the constant its name selects in the scope of the source that
    wrote it, once that constant's initializer has folded to a literal; a
    width that must be computed is left as written and refused by the first
    stage that needs its value.

    Three malformed applications are refused here, each for its own reason:
    a zero width, an omitted width (bare `u` and `i` are the unbounded
    integers; nothing is inferred), and a width that names neither a constant
    nor a const parameter, which is a runtime value.

    A width that names a const parameter is symbolic: the application is left
    as written for the specialization that substitutes it. This pass does not
    know which declaration a type sits in, so it accepts the name of any
    parameter in the forest; a runtime value that shares such a name is not
    resolved here and is refused by the stage that cannot find its meaning.
*/

use super::declaration_selection::ConstantSelection;
use diagnostics::Diagnostic;
use numerics::integer_carrier::{FixedInteger, IntegerSign};
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::ExpressionNode;
use syntax_trees::identifier::Identifier;
use syntax_trees::item::TypeParameterKind;
use syntax_trees::types::TypeReferenceNode;

/* The sign a family head names, or `None` for any other generic head. */
fn family_sign(head: &str) -> Option<IntegerSign> {
    match head {
        "u" => Some(IntegerSign::Unsigned),
        "i" => Some(IntegerSign::Signed),
        _ => None,
    }
}

enum Width {
    Closed(i128),
    /* Beyond `u64`, so not a width at all. */
    TooWide,
    Symbolic,
    Runtime,
    /* An expression or a declared constant this pass does not evaluate. */
    Unevaluated,
}

pub(crate) fn normalize(
    syntax: &mut SyntaxTrees,
    selection: &ConstantSelection<'_>,
) -> Result<(), Vec<Diagnostic>> {
    let applications = syntax
        .type_references
        .iter_type_references()
        .filter_map(|(handle, node)| {
            let TypeReferenceNode::Generic {
                base_name,
                arguments,
                ..
            } = node
            else {
                return None;
            };
            let sign = family_sign(base_name.as_str(syntax))?;
            Some((handle, sign, base_name.source_span(), *arguments))
        })
        .collect::<Vec<_>>();
    if applications.is_empty() {
        return Ok(());
    }
    let width_of = |name: &Identifier| -> Result<Width, Diagnostic> {
        let text = name.as_str(syntax);
        if !text.is_empty()
            && text
                .bytes()
                .all(|byte| byte.is_ascii_digit() || byte == b'_')
        {
            return Ok(match text.replace('_', "").parse::<u64>() {
                Ok(value) => Width::Closed(i128::from(value)),
                Err(_) => Width::TooWide,
            });
        }
        /* The constant this name selects in the scope of the source that wrote it. */
        if let Some(constant) = selection.select(syntax, name)? {
            return Ok(match syntax.expressions.expression(constant.value) {
                ExpressionNode::Integer(literal) => literal
                    .value_u64()
                    .map(i128::from)
                    .or_else(|| literal.value_i64().map(i128::from))
                    .map_or(Width::TooWide, Width::Closed),
                _ => Width::Unevaluated,
            });
        }
        if selection.selects_const(syntax, name) {
            return Ok(Width::Unevaluated);
        }
        let parameter = syntax.items.type_parameter_entries().any(|parameter| {
            matches!(
                parameter.kind,
                TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. }
            ) && parameter.name.as_str(syntax) == text
        });
        Ok(if parameter {
            Width::Symbolic
        } else {
            Width::Runtime
        })
    };

    let mut diagnostics = Vec::new();
    let mut rewrites = Vec::new();
    for (handle, sign, head, arguments) in applications {
        let letter = match sign {
            IntegerSign::Unsigned => 'u',
            IntegerSign::Signed => 'i',
        };
        let refuse = |message: String| Diagnostic::error(message).with_source_span(head);
        let arguments = syntax.type_references.type_reference_handles(arguments);
        let [argument] = arguments else {
            diagnostics.push(refuse(if arguments.is_empty() {
                format!(
                    "`{letter}<>` omits its width: write the width, as in `{letter}<32>`; \
                     a width is never inferred, and bare `{letter}` is the unbounded integer"
                )
            } else {
                format!(
                    "`{letter}<...>` takes exactly one width, found {} arguments",
                    arguments.len()
                )
            }));
            continue;
        };
        let width = match syntax.type_references.type_reference(*argument) {
            TypeReferenceNode::Named(name) => width_of(name),
            TypeReferenceNode::ConstExpression(expression) => {
                match syntax.expressions.expression(*expression) {
                    ExpressionNode::Integer(literal) => Ok(match literal.value_u64() {
                        Some(value) => Width::Closed(i128::from(value)),
                        None => Width::TooWide,
                    }),
                    ExpressionNode::Name { members, .. } => {
                        match syntax.expressions.identifier_path_members(*members) {
                            [name] => width_of(name),
                            _ => Ok(Width::Unevaluated),
                        }
                    }
                    _ => Ok(Width::Unevaluated),
                }
            }
            _ => Ok(Width::Runtime),
        };
        let width = match width {
            Ok(width) => width,
            Err(diagnostic) => {
                diagnostics.push(diagnostic);
                continue;
            }
        };
        match width {
            Width::Closed(0) => diagnostics.push(refuse(format!(
                "`{letter}<0>` has a zero width: an integer width is a positive `u64`"
            ))),
            Width::Closed(value) => {
                match u64::try_from(value)
                    .ok()
                    .and_then(|width| FixedInteger::new(sign, width))
                {
                    Some(carrier) => rewrites.push((handle, carrier, head)),
                    None => diagnostics.push(refuse(format!(
                        "the width of `{letter}<...>` is {value}: an integer width is a \
                         positive `u64`"
                    ))),
                }
            }
            Width::TooWide => diagnostics.push(refuse(format!(
                "the width of `{letter}<...>` exceeds `u64`: an integer width is a positive `u64`"
            ))),
            Width::Runtime => diagnostics.push(refuse(format!(
                "the width of `{letter}<...>` is a runtime value: an integer width is a \
                 constant or a const parameter, known before the program runs"
            ))),
            Width::Symbolic | Width::Unevaluated => {}
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    for (handle, carrier, head) in rewrites {
        syntax.type_references.replace_type_reference(
            handle,
            TypeReferenceNode::Named(Identifier::new(carrier.to_string(), head)),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /* Every named type and every surviving application head, as written after the pass. */
    fn spellings(text: &str) -> Result<(Vec<String>, Vec<String>), Vec<String>> {
        let tokens = source_files_to_tokens::Lexer::new(text).tokenize().unwrap();
        let mut sources = source::SourceMap::default();
        let id = sources
            .add("widths/main.omg".into(), text.to_owned())
            .source_id;
        let mut syntax = tokens_to_syntax_trees::parse_syntax_trees_with_id(id, &tokens).unwrap();
        let selection =
            ConstantSelection::new(&syntax, Some(std::sync::Arc::new(sources)), Vec::new())
                .unwrap();
        normalize(&mut syntax, &selection).map_err(|diagnostics| {
            diagnostics
                .iter()
                .map(|diagnostic| diagnostic.message.to_string())
                .collect::<Vec<_>>()
        })?;
        let mut named = Vec::new();
        let mut applied = Vec::new();
        for (_, node) in syntax.type_references.iter_type_references() {
            match node {
                TypeReferenceNode::Named(name) => named.push(name.as_str(&syntax).to_owned()),
                TypeReferenceNode::Generic { base_name, .. } => {
                    applied.push(base_name.as_str(&syntax).to_owned())
                }
                _ => {}
            }
        }
        Ok((named, applied))
    }

    #[test]
    fn a_closed_width_application_is_the_decimal_shorthand() {
        let (named, applied) = spellings(
            "const WIDTH: u64 = 13;
             machine next(word: u<32>, small: i<13>, odd: u<WIDTH>, bit: i<1>) -> u<257> { 0 }",
        )
        .unwrap();
        for carrier in ["u32", "i13", "u13", "i1", "u257"] {
            assert!(
                named.iter().any(|name| name == carrier),
                "{carrier}: {named:?}"
            );
        }
        assert!(applied.is_empty(), "{applied:?}");
    }

    #[test]
    fn a_const_parameter_width_stays_an_application() {
        let (_, applied) =
            spellings("machine next<const N: u64>(value: u<N>) -> i<N> { value }").unwrap();
        assert_eq!(applied, ["u", "i"]);
    }

    #[test]
    fn zero_runtime_and_oversized_widths_are_refused_for_their_own_reasons() {
        for (source, reason) in [
            ("machine next(value: u<0>) -> u32 { 1 }", "zero width"),
            ("machine next(value: i<0>) -> u32 { 1 }", "zero width"),
            (
                "machine next(count: u64, value: u<count>) -> u32 { 1 }",
                "runtime value",
            ),
            (
                "machine next(value: u<18446744073709551616>) -> u32 { 1 }",
                "exceeds `u64`",
            ),
            (
                "machine next(value: u<8, 16>) -> u32 { 1 }",
                "exactly one width",
            ),
        ] {
            let stopped = spellings(source).unwrap_err();
            assert!(
                stopped.len() == 1 && stopped[0].contains(reason),
                "{source}: {stopped:?}"
            );
        }
    }
}
