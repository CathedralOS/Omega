//! Checked mathematical declarations: the proof-surface records a top-level
//! `let`/`boundary let` elaborates into (PROOF-CONTRACT-MIGRATION).
//!
//! The typed mirror (`typed_trees::mathematical`) preserves the authored
//! declaration-local grammar; this module interprets it into
//! [`CheckedMathematicalDeclaration`], the checked shape mirroring the proof
//! kernel's `Declaration` (`mathematical_core::signature`):
//!
//! - Generic binders classify by authored carrier: `u: core::Level` becomes a
//!   universe `Level` binder, `A: core::Type<u>` a `Type` binder carrying its
//!   level argument, `const` stays a compile-time value binder, and every
//!   other carrier stays a `Value` subject binder. The fixed `core::*`
//!   declarations do not exist yet, so carriers classify by resolved authored
//!   name; symbol identity replaces name matching once they land.
//! - Telescope parameter and result types render to their nested-Pi
//!   identity: `(x: A) -> B(x)` or `A -> B` for arrows, `callee(arg, ...)`
//!   for applications, and the ordinary display spelling for ordinary
//!   references.
//! - The body renders the transparent term, or records the `Assumption`
//!   absence a `boundary let` declares.
//!
//! Grammar the mirror can carry but this surface cannot faithfully record
//! refuses loudly rather than silently dropping it: property bounds on
//! binders, machine or proposition binder kinds (a producer defect — the
//! mathematical grammar does not admit them), malformed `core::Type`/
//! `core::Level` carriers, and borrow, constrained or dynamic-trait type
//! references. Kernel-term elaboration and checking a definition body
//! against its declared result remain later legs.

use checked_trees::{
    CheckedMathematicalBinder, CheckedMathematicalBinderKind, CheckedMathematicalBody,
    CheckedMathematicalDeclaration, CheckedMathematicalParameter,
};
use typed_trees::TypedTrees;
use typed_trees::data::{DataProperties, TypeParameter, TypeParameterKind};
use typed_trees::mathematical::{
    MathematicalBody, MathematicalDefinition, MathematicalType, MathematicalTypeHandle,
};
use typed_trees::proposition::ProofSubstitutions;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// Build the checked mathematical-declaration surface for a typed program.
///
/// One [`CheckedMathematicalDeclaration`] per authored declaration, in
/// declaration order: universe binders, the ordered ordinary telescope, the
/// result identity, and either a transparent definition term or the
/// named-assumption absence a `boundary let` declares. A declaration the
/// surface cannot record faithfully fails with diagnostics rather than
/// elaborating partially.
pub(crate) fn build_checked_mathematical_declarations(
    program: &TypedTrees,
) -> Result<Vec<CheckedMathematicalDeclaration>, Vec<diagnostics::Diagnostic>> {
    program
        .mathematical_definitions()
        .iter()
        .map(|definition| elaborate_declaration(program, definition))
        .collect()
}

fn elaborate_declaration(
    program: &TypedTrees,
    definition: &MathematicalDefinition,
) -> Result<CheckedMathematicalDeclaration, Vec<diagnostics::Diagnostic>> {
    let binders = program
        .data_type_parameters
        .span_or_empty(definition.binders)
        .iter()
        .map(|binder| elaborate_binder(program, definition, binder))
        .collect::<Result<_, _>>()?;
    let parameters = program
        .mathematical_parameters(definition.parameters)
        .iter()
        .map(|parameter| {
            Ok(CheckedMathematicalParameter {
                name: parameter.name.to_string(),
                relevance: parameter.relevance,
                type_identity: mathematical_type_identity(program, definition, parameter.ty)?,
            })
        })
        .collect::<Result<_, Vec<diagnostics::Diagnostic>>>()?;
    let result = mathematical_type_identity(program, definition, definition.result)?;
    let body = match definition.body {
        MathematicalBody::Assumption => CheckedMathematicalBody::Assumption,
        MathematicalBody::Definition(term) => CheckedMathematicalBody::Definition {
            term_identity: program.render_proof_expression(term, ProofSubstitutions::None),
        },
    };
    Ok(CheckedMathematicalDeclaration {
        symbol: definition.symbol,
        name: definition.name.to_string(),
        is_public: definition.is_public,
        binders,
        parameters,
        result,
        body,
    })
}

fn elaborate_binder(
    program: &TypedTrees,
    definition: &MathematicalDefinition,
    binder: &TypeParameter,
) -> Result<CheckedMathematicalBinder, Vec<diagnostics::Diagnostic>> {
    if binder.bounds != DataProperties::default() {
        return Err(refuse(
            program,
            definition,
            format!(
                "mathematical binder `{}` cannot carry property bounds",
                binder.name
            ),
        ));
    }
    let kind = match &binder.kind {
        TypeParameterKind::Type => CheckedMathematicalBinderKind::Type { universe: None },
        TypeParameterKind::Const { type_reference } => CheckedMathematicalBinderKind::Const {
            type_identity: ordinary_type_identity(program, definition, *type_reference)?,
        },
        TypeParameterKind::Value { type_reference } => {
            classify_subject_carrier(program, definition, binder, *type_reference)?
        }
        TypeParameterKind::Machine { .. } | TypeParameterKind::Proposition { .. } => {
            return Err(refuse(
                program,
                definition,
                format!(
                    "mathematical binder `{}` cannot carry a machine or proposition contract",
                    binder.name
                ),
            ));
        }
    };
    Ok(CheckedMathematicalBinder {
        name: binder.name.to_string(),
        kind,
    })
}

/// Classify a `name: Carrier` binder by the fixed core declaration its
/// carrier names: `core::Level` makes a universe binder and `core::Type` a
/// type binder carrying its level argument; any other carrier stays an
/// ordinary subject binder. Until the fixed `core::*` declarations exist the
/// match is on the resolved authored name.
fn classify_subject_carrier(
    program: &TypedTrees,
    definition: &MathematicalDefinition,
    binder: &TypeParameter,
    carrier: TypeReferenceHandle,
) -> Result<CheckedMathematicalBinderKind, Vec<diagnostics::Diagnostic>> {
    if !program
        .type_reference_table
        .contains_type_reference(carrier)
    {
        return Err(refuse(
            program,
            definition,
            format!("mathematical binder `{}` has no carrier type", binder.name),
        ));
    }
    match program.type_reference_table.type_reference(carrier) {
        TypeReferenceNode::Named { name, .. } => match name.as_str() {
            "core::Level" => Ok(CheckedMathematicalBinderKind::Level),
            "core::Type" => Ok(CheckedMathematicalBinderKind::Type { universe: None }),
            _ => Ok(CheckedMathematicalBinderKind::Value {
                type_identity: ordinary_type_identity(program, definition, carrier)?,
            }),
        },
        TypeReferenceNode::Generic {
            base_name,
            arguments,
            ..
        } => match base_name.as_str() {
            "core::Type" => {
                let arguments = program
                    .type_reference_table
                    .type_reference_handles(*arguments);
                match arguments {
                    [universe] => Ok(CheckedMathematicalBinderKind::Type {
                        universe: Some(ordinary_type_identity(program, definition, *universe)?),
                    }),
                    _ => Err(refuse(
                        program,
                        definition,
                        format!(
                            "mathematical binder `{}` applies `core::Type` to {} arguments; it takes one level",
                            binder.name,
                            arguments.len()
                        ),
                    )),
                }
            }
            "core::Level" => Err(refuse(
                program,
                definition,
                format!(
                    "mathematical binder `{}` applies arguments to `core::Level`; it takes none",
                    binder.name
                ),
            )),
            _ => Ok(CheckedMathematicalBinderKind::Value {
                type_identity: ordinary_type_identity(program, definition, carrier)?,
            }),
        },
        _ => Ok(CheckedMathematicalBinderKind::Value {
            type_identity: ordinary_type_identity(program, definition, carrier)?,
        }),
    }
}

/// Render one mathematical type to its nested-Pi identity. Arrows associate
/// right and a named domain binder scopes over the codomain; an arrow or
/// application in domain/callee position parenthesizes so the spelling keeps
/// its authored grouping.
fn mathematical_type_identity(
    program: &TypedTrees,
    definition: &MathematicalDefinition,
    handle: MathematicalTypeHandle,
) -> Result<String, Vec<diagnostics::Diagnostic>> {
    match program.mathematical_type(handle) {
        MathematicalType::Ordinary(reference) => {
            ordinary_type_identity(program, definition, *reference)
        }
        MathematicalType::Arrow {
            binder,
            domain,
            codomain,
        } => {
            let domain_is_arrow = matches!(
                program.mathematical_type(*domain),
                MathematicalType::Arrow { .. }
            );
            let domain = mathematical_type_identity(program, definition, *domain)?;
            let domain = if domain_is_arrow {
                format!("({domain})")
            } else {
                domain
            };
            let codomain = mathematical_type_identity(program, definition, *codomain)?;
            Ok(match binder {
                Some(binder) => format!("({binder}: {domain}) -> {codomain}"),
                None => format!("{domain} -> {codomain}"),
            })
        }
        MathematicalType::Application { callee, arguments } => {
            let callee_identity = mathematical_type_identity(program, definition, *callee)?;
            let callee_identity = match program.mathematical_type(*callee) {
                MathematicalType::Arrow { .. } => format!("({callee_identity})"),
                _ => callee_identity,
            };
            let arguments = program
                .expression_table
                .expression_handles(*arguments)
                .iter()
                .map(|argument| {
                    program.render_proof_expression(*argument, ProofSubstitutions::None)
                })
                .collect::<Vec<_>>()
                .join(", ");
            Ok(format!("{callee_identity}({arguments})"))
        }
    }
}

/// Render an ordinary type reference carried by the mathematical surface.
/// Borrows, constrained references and dynamic traits are runtime-custody or
/// property surfaces the checked declaration cannot record, so they refuse.
fn ordinary_type_identity(
    program: &TypedTrees,
    definition: &MathematicalDefinition,
    reference: TypeReferenceHandle,
) -> Result<String, Vec<diagnostics::Diagnostic>> {
    if !program
        .type_reference_table
        .contains_type_reference(reference)
    {
        return Err(refuse(
            program,
            definition,
            format!(
                "mathematical declaration `{}` references a type that does not exist",
                definition.name
            ),
        ));
    }
    match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Reference { .. }
        | TypeReferenceNode::Constrained { .. }
        | TypeReferenceNode::DynamicTrait { .. } => Err(refuse(
            program,
            definition,
            format!(
                "mathematical type `{}` cannot be a borrow, constraint or dynamic trait",
                program.display_type_reference(reference)
            ),
        )),
        _ => Ok(program.display_type_reference(reference)),
    }
}

fn refuse(
    program: &TypedTrees,
    definition: &MathematicalDefinition,
    message: String,
) -> Vec<diagnostics::Diagnostic> {
    let mut diagnostic = diagnostics::Diagnostic::error(message);
    if let Some(span) = program.symbols.symbol_source_span(definition.symbol) {
        diagnostic = diagnostic.with_source_span(span);
    }
    vec![diagnostic]
}

#[cfg(test)]
mod tests;
