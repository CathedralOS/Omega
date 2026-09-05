//! Pre-resolution synthesis of closed generic data inside the name-resolution owner.
//!
//! These routines consume syntax and return syntax. Discovered templates,
//! substitutions and pending instances are private working state. Durable
//! instance declarations remain in SyntaxTrees, not a second program schema.

use psi_arena::{Handle, HandleSpan};
use psi_diagnostics::Diagnostic;
use psi_language_semantics::const_value::CanonicalConstValue;
use psi_numerics::literals::{IntegerLiteral, IntegerRadix};
use psi_syntax_trees::SyntaxTrees;
use psi_syntax_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use psi_syntax_trees::identifier::Identifier;
use psi_syntax_trees::item::{
    ConstDefinition, DataDefinition, DataMember, Item, ProofFact, TypeParameterKind,
};
use psi_syntax_trees::statement::StatementNode;
use psi_syntax_trees::types::{
    FixedArrayLength, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};
use std::collections::{HashMap, HashSet};

mod arguments;
mod const_evaluation;
mod discovery;
mod eligibility;
mod substitution;
mod synthesis;
mod uses;

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
pub fn normalize_generic_data(mut syntax: SyntaxTrees) -> Result<SyntaxTrees, Vec<Diagnostic>> {
    desugar_generic_data_instances(&mut syntax)?;
    Ok(syntax)
}
