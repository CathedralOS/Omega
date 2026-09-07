//! Pre-resolution synthesis of closed generic data inside the name-resolution owner.
//!
//! These routines consume syntax and return syntax. Discovered templates,
//! substitutions and pending instances are private working state. Durable
//! instance declarations remain in SyntaxTrees, not a second program schema.

use arena::{Handle, HandleSpan};
use diagnostics::Diagnostic;
use language_semantics::const_value::CanonicalConstValue;
use numerics::literals::{IntegerLiteral, IntegerRadix};
use std::collections::{HashMap, HashSet};
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use syntax_trees::identifier::Identifier;
use syntax_trees::item::{
    ConstDefinition, DataDefinition, DataMember, Item, ProofFact, TypeParameterKind,
};
use syntax_trees::statement::StatementNode;
use syntax_trees::types::{
    FixedArrayLength, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};

mod arguments;
mod const_evaluation;
mod discovery;
mod eligibility;
mod substitution;
mod synthesis;
mod uses;

#[cfg(test)]
mod division_tests;
#[cfg(test)]
mod remainder_tests;

use arguments::*;
use const_evaluation::*;
use discovery::*;
use eligibility::*;
use substitution::*;
use synthesis::desugar_generic_data_instances;
use uses::*;

/// Canonicalize one source const declaration against its own declared type.
///
/// This is the narrow handoff used by declaration/API retention. The returned
/// value's structural encoding is semantic material; its display text remains
/// diagnostic-only. Constrained public constants stay unsupported until their
/// declaration-site proof obligations are checked rather than erased here.
pub(crate) fn canonicalize_declared_const_definition(
    syntax: &SyntaxTrees,
    definition: &ConstDefinition,
) -> Result<CanonicalConstValue, String> {
    if matches!(
        syntax
            .tables
            .type_references
            .type_reference(definition.type_reference),
        TypeReferenceNode::Constrained { .. }
    ) {
        return Err(
            "constrained const declarations require declaration-site proof checking before they can publish compatibility identity"
                .to_owned(),
        );
    }
    canonicalize_const_definition(syntax, definition, definition.type_reference)
}

/// Find `Base<Args..>` spellings in FIELD type position where `Base` is a
/// generic data definition, synthesize one concrete instance record per
/// distinct spelling (the parameter substituted for the argument), and rewrite
/// the field spellings to the instances' plain names.
/// Run Psi's target-neutral pre-resolution generic-data normalization and
/// return the only syntax tree downstream stages may consume.
///
/// Taking ownership prevents orchestration code from retaining an unnormalized
/// sibling or reaching into the elaborator as an in-place syntax mutator.
pub fn normalize_generic_data(syntax: SyntaxTrees) -> Result<SyntaxTrees, Vec<Diagnostic>> {
    let (syntax, warnings) = normalize_generic_data_with_warnings(syntax)?;
    for warning in warnings {
        eprintln!("{warning}");
    }
    Ok(syntax)
}

fn normalize_generic_data_with_warnings(
    mut syntax: SyntaxTrees,
) -> Result<(SyntaxTrees, Vec<Diagnostic>), Vec<Diagnostic>> {
    let mut warnings = Vec::new();
    desugar_generic_data_instances(&mut syntax, &mut warnings)?;
    deduplicate_generic_warnings(&mut warnings);
    Ok((syntax, warnings))
}

fn deduplicate_generic_warnings(warnings: &mut Vec<Diagnostic>) {
    let mut origins = Vec::new();
    warnings.retain(|warning| {
        let Some(origin) = warning.source_span else {
            return true;
        };
        if warning.is_error()
            || origin.source_id.0 == usize::MAX
            || origin.span.start >= origin.span.end
        {
            return true;
        }
        // Generic clones retain the same authored origin. Missing or invalid
        // spans do not establish that two diagnostics describe the same site.
        if origins.contains(&origin) {
            return false;
        }
        origins.push(origin);
        true
    });
}
