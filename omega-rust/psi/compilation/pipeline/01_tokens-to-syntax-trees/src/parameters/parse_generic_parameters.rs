use crate::input::token_cursor::{Input, ParseResult};
use crate::type_syntax::parse_type::parse_type_reference_handle;
use crate::type_syntax::properties::parse_property_brackets;
use arena::HandleSpan;
use syntax_trees::SyntaxTrees;
use syntax_trees::identifier::Identifier;
use syntax_trees::item::{
    DataProperties, MachineParameterContract, TypeParameter, TypeParameterKind,
};
use tokens::PunctuationKind;

#[derive(Default)]
pub(crate) struct ParsedGenericParameters {
    pub(crate) lifetime_parameters: Vec<Identifier>,
    pub(crate) type_parameters: HandleSpan<TypeParameter>,
    pub(crate) conformance_bounds: Vec<syntax_trees::item::GenericConformanceBound>,
}

#[derive(Clone, Copy)]
pub(crate) enum GenericParameterSyntax {
    TypeAndConst,
    StaticBinders,
    TraitRequirements,
    /// A requirement signature inside a trait body. Runtime-capable `Value`
    /// binders are admitted so a finite `where Binder == literal || ...`
    /// family can enumerate them; conformance binders stay on the trait or
    /// provider declaration where a dynamic envelope can still refuse them.
    RequirementSignature,
    MachineDeclaration,
    /// A `data` declaration header. Like `StaticBinders` it admits static
    /// machine symbols (N7 proof-data families), and it additionally admits
    /// runtime-capable `Value` binders so a field can qualify on a stored or
    /// erased input value. Downstream stages still own the construction-time
    /// obligation and erased-versus-executable index decision; the binder
    /// only gains a parse surface here.
    DataDeclaration,
    /// A top-level `let`/`boundary let` mathematical declaration
    /// (PROOF-CONTRACT-MIGRATION). `Value` binders are admitted because the
    /// settled binder spellings `u: core::Level` and `A: core::Type<u>` are
    /// value-shaped carriers — the universe/level reading comes from the
    /// carrier's resolved declaration, not a binder keyword. Machine,
    /// proposition and `satisfies` conformance binders stay excluded.
    MathematicalDefinition,
}

/// The `>` terminator shared by every exit from the parameter-list loop:
/// take the close punctuation, then wrap the accumulated span.
fn close_generic_parameter_list<'tokens, 'source>(
    input: Input<'tokens, 'source>,
    lifetime_parameters: Vec<Identifier>,
    type_parameters: crate::handle_spans::HandleSpanBuilder<TypeParameter>,
    conformance_bounds: Vec<syntax_trees::item::GenericConformanceBound>,
) -> ParseResult<'tokens, 'source, ParsedGenericParameters> {
    let input = input.take_punctuation(PunctuationKind::Greater, ">")?;
    Ok((
        ParsedGenericParameters {
            lifetime_parameters,
            type_parameters: type_parameters.finish(),
            conformance_bounds,
        },
        input,
    ))
}

pub(crate) fn parse_generic_parameters<'tokens, 'source>(
    syntax_trees: &mut SyntaxTrees,
    mut input: Input<'tokens, 'source>,
    syntax: GenericParameterSyntax,
) -> ParseResult<'tokens, 'source, ParsedGenericParameters> {
    let allow_machine_parameters = !matches!(
        syntax,
        GenericParameterSyntax::TypeAndConst | GenericParameterSyntax::MathematicalDefinition
    );
    let allow_proposition_parameters = matches!(syntax, GenericParameterSyntax::TraitRequirements);
    let allow_conformance_binders = matches!(
        syntax,
        GenericParameterSyntax::TraitRequirements | GenericParameterSyntax::MachineDeclaration
    );
    let trait_requirement_parameters = matches!(syntax, GenericParameterSyntax::TraitRequirements);
    let allow_value_parameters = matches!(
        syntax,
        GenericParameterSyntax::RequirementSignature
            | GenericParameterSyntax::MachineDeclaration
            | GenericParameterSyntax::MathematicalDefinition
            | GenericParameterSyntax::DataDeclaration
    );
    if !input.at_punctuation(PunctuationKind::Less) {
        return Ok((ParsedGenericParameters::default(), input));
    }

    input = input.take_punctuation(PunctuationKind::Less, "<")?;
    let mut type_parameters = crate::handle_spans::HandleSpanBuilder::new();
    let mut lifetime_parameters = Vec::new();
    let mut conformance_bounds = Vec::new();
    let mut declared_names = Vec::<String>::new();
    let mut saw_runtime_parameter = false;

    loop {
        // A leading bracket is the attribute-prefix spelling, which decision
        // 13 rejects: brackets attach to what they FOLLOW.
        if input.at_punctuation(PunctuationKind::LeftBracket) {
            return Err(input.error_here(
                "property brackets attach to the name they follow: write the bounds after the type parameter, like `T [copy]`",
            ));
        }

        // A lifetime parameter (`<'buf>`); frozen decision 15 stage 2. It is
        // an erased borrow-region binder, stored separately from ordinary
        // type/const/machine parameters so runtime generic arity and
        // monomorphization never count it.
        if input.at_punctuation(PunctuationKind::Apostrophe) {
            if saw_runtime_parameter {
                return Err(input.error_here(
                    "lifetime parameters precede type, const, and machine parameters",
                ));
            }
            let after_tick = input.take_punctuation(PunctuationKind::Apostrophe, "'")?;
            let (lifetime_name, next) = after_tick.take_identifier()?;
            if declared_names
                .iter()
                .any(|declared| declared == lifetime_name.as_str(syntax_trees))
            {
                return Err(next.error_here(format!(
                    "duplicate generic parameter `{}`",
                    lifetime_name.as_str(syntax_trees)
                )));
            }
            declared_names.push(lifetime_name.as_str(syntax_trees).to_owned());
            lifetime_parameters.push(lifetime_name);
            input = next;

            if input.at_punctuation(PunctuationKind::Comma) {
                input = input.take_punctuation(PunctuationKind::Comma, ",")?;
                continue;
            }

            return close_generic_parameter_list(
                input,
                lifetime_parameters,
                type_parameters,
                conformance_bounds,
            );
        }

        let (name, mut kind, next) = if input.at_contextual("const") {
            let input = input.take_contextual("const")?;
            let (name, input) = input.take_identifier()?;
            let input = input.take_punctuation(PunctuationKind::Colon, ":")?;
            let (type_reference, input) = parse_type_reference_handle(syntax_trees, input)?;
            (name, TypeParameterKind::Const { type_reference }, input)
        } else if input.at_keyword(tokens::KeywordKind::Machine) {
            if !allow_machine_parameters {
                return Err(input.error_here(
                    "`<machine M>` is a static machine parameter and is only legal on a machine or conformance declaration",
                ));
            }
            let input = input.take_keyword(tokens::KeywordKind::Machine, "machine")?;
            let (name, input) = input.take_identifier()?;
            let contract = trait_requirement_parameters
                .then_some(MachineParameterContract::RequirementIdentity);
            (name, TypeParameterKind::Machine { contract }, input)
        } else if input.at_contextual("proposition") {
            if !allow_proposition_parameters {
                return Err(input.error_here(
                    "`<proposition Relation>` is currently legal only on a trait declaration",
                ));
            }
            let input = input.take_contextual("proposition")?;
            let (name, input) = input.take_identifier()?;
            (
                name,
                TypeParameterKind::Proposition { contract: None },
                input,
            )
        } else {
            let (name, input) = input.take_identifier()?;
            (name, TypeParameterKind::Type, input)
        };
        saw_runtime_parameter = true;
        input = next;
        if declared_names
            .iter()
            .any(|declared| declared == name.as_str(syntax_trees))
        {
            return Err(input.error_here(format!(
                "duplicate generic parameter `{}`",
                name.as_str(syntax_trees)
            )));
        }
        declared_names.push(name.as_str(syntax_trees).to_owned());

        // `Order satisfies Ranked<Element>` is an evidence binder: an
        // implementation parameter, not a type. Its trait application carries
        // every data type the interface names; no parameter is its subject.
        if matches!(kind, TypeParameterKind::Type) && input.at_contextual("satisfies") {
            if !allow_conformance_binders {
                return Err(input.error_here(format!(
                    "an evidence binder `{} satisfies Trait<...>` is not admitted in this parameter list",
                    name.as_str(syntax_trees)
                )));
            }
            let rest = input.take_contextual("satisfies")?;
            let (trait_name, rest) = rest.take_identifier()?;
            let (arguments, rest) =
                crate::contracts::conformance::parse_conformance::parse_optional_satisfies_type_arguments(
                    syntax_trees,
                    rest,
                )?;
            conformance_bounds.push(syntax_trees::item::GenericConformanceBound {
                binder: name.clone(),
                trait_name,
                arguments,
            });
            // Trait applications supply every static slot positionally.
            // Machines retain their existing separate evidence telescope;
            // traits publish the implementation slot alongside type, const,
            // machine and proposition slots, with the bound carrying its
            // contract rather than pretending an implementation is data.
            if trait_requirement_parameters {
                let handle = syntax_trees.items.append_type_parameter(TypeParameter {
                    name,
                    kind: TypeParameterKind::Implementation,
                    bounds: DataProperties::default(),
                });
                type_parameters.push(handle);
            }
            input = rest;

            if input.at_punctuation(PunctuationKind::Comma) {
                input = input.take_punctuation(PunctuationKind::Comma, ",")?;
                continue;
            }
            return close_generic_parameter_list(
                input,
                lifetime_parameters,
                type_parameters,
                conformance_bounds,
            );
        }
        if matches!(kind, TypeParameterKind::Type)
            && input.at_punctuation(PunctuationKind::Colon)
            && input
                .take_punctuation(PunctuationKind::Colon, ":")
                .and_then(|rest| rest.take_identifier().map(|(_, rest)| rest))
                .is_ok_and(|rest| rest.at_contextual("satisfies"))
        {
            return Err(input.error_here(format!(
                "a conformance has no data subject; write the evidence binder as `{} satisfies Trait<Data>`",
                name.as_str(syntax_trees)
            )));
        }

        // `Name: TypeRef` — a runtime-capable value binder, admitted on
        // machine signature generics and trait requirement signatures. Its
        // carrier type parses exactly like a const binder's; brackets after
        // it attach to the type, not the name.
        if matches!(kind, TypeParameterKind::Type)
            && allow_value_parameters
            && input.at_punctuation(PunctuationKind::Colon)
        {
            let rest = input.take_punctuation(PunctuationKind::Colon, ":")?;
            let (type_reference, rest) = parse_type_reference_handle(syntax_trees, rest)?;
            kind = TypeParameterKind::Value { type_reference };
            input = rest;
        }

        // Where `satisfies` binders are not admitted, `Name: Subject
        // satisfies Carrier` is a rejected conformance binder, not a value
        // binder with a stray tail; say so rather than failing on `satisfies`
        // as unexpected punctuation.
        if matches!(kind, TypeParameterKind::Value { .. })
            && !allow_conformance_binders
            && input.at_contextual("satisfies")
        {
            return Err(input.error_here(format!(
                "a `satisfies` conformance binder is not admitted in this parameter list; `{}: <type>` already declares a runtime value binder",
                name.as_str(syntax_trees)
            )));
        }

        // Rust-style `<T: copy>` is rejected with the bracket spelling
        // suggested: a colon bound would split the property spelling system.
        if matches!(kind, TypeParameterKind::Type) && input.at_punctuation(PunctuationKind::Colon) {
            let after_colon = input.take_punctuation(PunctuationKind::Colon, ":")?;
            let parameter = name.as_str(syntax_trees);
            let bound = after_colon
                .take_identifier()
                .map(|(bound, _)| bound.as_str(syntax_trees).to_owned())
                .unwrap_or_else(|_| "copy".to_owned());
            return Err(input.error_here(format!(
                "type parameter `{parameter}` takes property bounds in brackets after its name: write `{parameter} [{bound}]`, not `{parameter}: {bound}`"
            )));
        }

        // Brackets after a const parameter never reach here: they attach to
        // the const's TYPE as a constraint list (`const N: usize [range ...]`).
        if matches!(kind, TypeParameterKind::Machine { .. })
            && input.at_punctuation(PunctuationKind::LeftBracket)
        {
            return Err(input.error_here(
                "a machine parameter takes its callable contract in a mandatory `where machine M(...) -> Result` clause, not property brackets",
            ));
        }
        if matches!(kind, TypeParameterKind::Proposition { .. })
            && input.at_punctuation(PunctuationKind::LeftBracket)
        {
            return Err(input.error_here(
                "a proposition parameter takes its signature in a mandatory `where proposition Name(...)` clause, not property brackets",
            ));
        }
        let bounds = if input.at_punctuation(PunctuationKind::LeftBracket) {
            let (bounds, next) = parse_property_brackets(input)?;
            input = next;
            bounds
        } else {
            DataProperties::default()
        };

        let handle = syntax_trees
            .items
            .append_type_parameter(TypeParameter { name, kind, bounds });
        type_parameters.push(handle);

        if input.at_punctuation(PunctuationKind::Comma) {
            input = input.take_punctuation(PunctuationKind::Comma, ",")?;
            continue;
        }

        return close_generic_parameter_list(
            input,
            lifetime_parameters,
            type_parameters,
            conformance_bounds,
        );
    }
}
