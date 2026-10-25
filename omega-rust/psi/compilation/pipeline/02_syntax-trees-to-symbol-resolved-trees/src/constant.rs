//! Constant substitution and exact const-index declaration custody; value contract: wiki/spec/language/constants.md.
//!
//! Const VALUE semantics exist only until symbol resolution:
//! A constant declares a named pure value; each selected expression path gets
//! a fresh copy of its initializer before resolution publishes its result.
//! Typed trees, validation, proofs, backends, and the interpreter never grow a
//! const-value concept -- each use IS the literal, which is exactly the copied-at-each-use
//! semantics the contract specifies (and why interior mutability can never hide in
//! one). Resolved declarations keep a detached initializer handle so later
//! source extensions can use the same substitution without reparsing or
//! decoding review encodings. Only newly authored roots resolve names: retained
//! roots keep their declaration-side selections. The symbol table preserves
//! declaration provenance for authored-selection and package-authority checks.
//!
//! Scalar and closed aggregate literals substitute
//! only after the shared resolver has selected their namespace and lexical
//! binding. Detached initializer roots resolve constructors and fields in their
//! declaring source. Each use deep-copies aggregate children and carries both
//! that declaration-side selection custody and its own constant-use occurrence.
//! Root and module declarations use this same path; no pre-resolution spelling
//! substitution or whole-forest local-shadowing restriction is needed. Module-owned scoped declarations
//! additionally select an exact nongeneric carrier under their declaring
//! source's ordinary name law: a module-local carrier outranks imported and
//! unmoduled candidates, and an exact import exposes a foreign-module carrier.
//! The authored scope token survives until complete symbol assignment, then joins
//! the ordinary visibility/selection ledger; the structural value encoder alone
//! cannot establish attachment ownership. Seeded declarations keep their existing
//! selections rather than inventing a new scope occurrence from a display name.
//! Numeric and Boolean initializers owe their declared landing even when private
//! and unused. Check declarations before substitution, reusing the same validator
//! during module normalization so public identity construction cannot bypass it.
//!
//! Nominal index values retain a private link to the actual parent-owned
//! argument span while lowering. Finalization rejoins the resolved parameter
//! and selected declaration before publishing occurrence custody. The ordinary
//! generic application and indexed-domain owners provide that relationship;
//! Body materialization retains those same nominal declaration identities without
//! turning canonical index encodings into constructor authority.
//!
//! Remaining boundaries, enforced loudly:
//! - Complete resolution consumes literal materializations (scalars, payloadless
//!   cases, and struct/array literals). Call-free integer/Boolean computations
//!   reach that form through build-time evaluation, retaining declaration-owned
//!   original syntax and selection receipts. Initializer preparation is a
//!   separate non-executing mode: pending expressions acquire no value identity.
//!   Computed integer/Boolean leaves in arrays and selected nominal literals
//!   evaluate independently. Calls and floating computations still need evaluation.
//! - A const may not collide with a case of its scope type: `Type::NAME` must
//!   stay unambiguous against case-constructor paths, which substitution
//!   would otherwise shadow.
//! - Selected numeric values retain their declared landing recursively through
//!   arrays. Destination checking rejoins complete array declaration types from
//!   retained selections, so empty arrays cannot lose their element identity.
//!
//! This file decides which initializers need evaluation and finalizes
//! constants. `initializer_leaves.rs` collects pending initializer leaves
//! and their selected destinations, `declaration_values.rs` validates and
//! retains declaration values, `substitution.rs` substitutes resolved
//! constants, `selections.rs` finalizes const selections and arguments,
//! `normalized_expressions.rs` validates normalized expressions,
//! `scalar_carriers.rs` recognizes builtin scalar carrier spellings and
//! `carrier.rs`, `initializer_dependencies.rs` and
//! `initializer_normalization.rs` carry the const carrier and initializer
//! dependencies.

mod carrier;
mod declaration_values;
pub(crate) mod initializer_dependencies;
mod initializer_leaves;
pub(crate) mod initializer_normalization;
mod normalized_expressions;
mod scalar_carriers;
mod selections;
mod substitution;

pub(crate) use declaration_values::{
    evaluated_declaration_value, retain_const_initializer, selected_expression_constant,
    validate_const_definition, validate_scalar_initializer,
};
pub use initializer_leaves::PendingConstInitializerLeaf;
pub(crate) use initializer_leaves::pending_const_initializer_leaves;
pub(crate) use normalized_expressions::{
    normalization_requires_expression, validate_normalized_const_argument,
    validate_normalized_expression,
};
pub(crate) use scalar_carriers::{is_builtin_scalar_carrier, is_structural_scalar_carrier};
pub(crate) use selections::{
    finalize_const_argument_selections, finalize_const_declarations, finalize_const_selections,
};
pub(crate) use substitution::{semantic_const_name, substitute_resolved_constants};

use diagnostics::Diagnostic;
use syntax_trees::SyntaxTrees;
use syntax_trees::item::ConstDefinition;

/// A direct implementation selection is a symbolic literal, not an evaluated
/// table address. Ordinary resolution and dynamic selection checking retain
/// and authenticate its implementation and complete target application, even
/// when the declaration has no use. Public canonical identity remains separate.
pub(crate) fn direct_selector_literal(
    syntax: &SyntaxTrees,
    expression: syntax_trees::expression::ExpressionHandle,
) -> bool {
    let syntax_trees::expression::ExpressionNode::Cast(cast) =
        syntax.expressions.expression(expression)
    else {
        return false;
    };
    !cast.form.is_recast()
        && cast.domain == numerics::arithmetic::ArithmeticDomain::Exact
        && cast.semantic_domain.is_empty()
        && cast.semantic_domain_arguments.is_empty()
        && matches!(
            syntax.type_references.type_reference(cast.target_type),
            syntax_trees::types::TypeReferenceNode::DynamicTrait { .. }
        )
        && matches!(
            syntax.expressions.expression(cast.value),
            syntax_trees::expression::ExpressionNode::Name(_)
        )
}

/// Find unfinished values without claiming a carrier or constructor selection.
/// Preparation separately admits exact scalar leaves in their declared owners.
pub fn requires_const_initializer_evaluation(
    syntax: &SyntaxTrees,
    definition: &ConstDefinition,
) -> bool {
    use syntax_trees::expression::ExpressionNode;
    if definition.pending_target_observation {
        return false;
    }
    let mut pending = vec![definition.value];
    while let Some(expression) = pending.pop() {
        if direct_selector_literal(syntax, expression) {
            continue;
        }
        match syntax.expressions.expression(expression) {
            ExpressionNode::Integer(_)
            | ExpressionNode::Boolean(_)
            | ExpressionNode::String(_)
            | ExpressionNode::UndeterminedNaN(_) => {}
            // An explicit float format needs ordinary carrier validation, not
            // an integer probe. Unsuffixed decimals may still land as integers.
            ExpressionNode::Float(text)
                if numerics::literals::FloatLiteral::parse(text.as_str(syntax))
                    .is_some_and(|literal| literal.landing().is_some()) => {}
            ExpressionNode::ArrayLiteral(elements) => {
                pending.extend(
                    syntax
                        .expressions
                        .expression_handles(*elements)
                        .iter()
                        .copied(),
                );
            }
            ExpressionNode::StructLiteral(literal) => {
                pending.extend(
                    syntax
                        .expressions
                        .struct_fields(literal.fields)
                        .iter()
                        .map(|field| field.value),
                );
            }
            _ => return true,
        }
    }
    false
}

/// Finalize argument selections, initializer normalization, substitution, and
/// use-site selections after resolution has joined constant declarations. The
/// declaration join precedes receipt helper demand and must not repeat here:
/// it also records the declaration's selected carrier and visibility custody.
/// Constant-expression symbol and selection passes run inside because they
/// consume only these pending values. A seeded lowerer first renumbers its
/// pending selections after the base's declarations.
pub(crate) fn finalize(
    lowerer: &mut crate::resolution::lowerer::Lowerer,
) -> Result<(), Vec<Diagnostic>> {
    if let Some(base_declarations) = lowerer
        .seed
        .as_ref()
        .map(|seed| seed.roots.const_declarations)
    {
        for selection in &mut lowerer.pending_const_selections {
            selection.declaration_ordinal = selection
                .declaration_ordinal
                .checked_add(base_declarations)
                .expect("seeded const declaration ordinal overflow");
        }
    }
    let program = &mut lowerer.symbol_resolved_trees;
    finalize_const_argument_selections(
        program,
        &lowerer.pending_const_argument_selections,
        &lowerer.pending_const_argument_slots,
        &lowerer.resolved_lexical_inputs,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    let const_expression_roots = lowerer
        .pending_const_values
        .iter()
        .copied()
        .chain(lowerer.pending_const_argument_expressions.iter().copied())
        .collect::<Vec<_>>();
    crate::symbols::assign_constant_expression_symbols(
        program,
        const_expression_roots.iter().copied(),
    );
    // Selection recording reads the call target. A named-implementation call
    // in a constant initializer has no machine-body route yet, so bind the
    // owned realization entry before that recording. Later requirement
    // routing still publishes the public requirement on machine-body calls.
    initializer_dependencies::bind_named_implementation_const_calls(
        program,
        &const_expression_roots,
    )
    .map_err(|message| vec![Diagnostic::error(message)])?;
    if lowerer.const_resolution_mode == crate::resolution::lowerer::ConstResolutionMode::Complete {
        declaration_values::validate_nominal_destinations(program)
            .map_err(|diagnostic| vec![diagnostic])?;
    }
    crate::selection::authored_selections::finalize_constant_expression_selections(
        program,
        lowerer
            .pending_const_values
            .iter()
            .copied()
            .chain(lowerer.pending_const_argument_expressions.iter().copied()),
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    let initializer_custody =
        initializer_normalization::finalize(program, &lowerer.pending_const_initializers)
            .map_err(|diagnostic| vec![diagnostic])?;
    substitute_resolved_constants(
        program,
        &lowerer.pending_authored_expressions,
        &mut lowerer.pending_const_selections,
        lowerer.const_resolution_mode,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    finalize_const_selections(program, &lowerer.pending_const_selections)
        .map_err(|diagnostic| vec![diagnostic])?;
    initializer_normalization::rejoin_after_substitution(program, &initializer_custody)
        .map_err(|diagnostic| vec![diagnostic])?;
    Ok(())
}

/// Close duplicate declaration-side operator obligations once the authored
/// selection ledger has recorded every ordinary selection.
pub(crate) fn finalize_operator_obligations(
    lowerer: &mut crate::resolution::lowerer::Lowerer,
) -> Result<(), Vec<Diagnostic>> {
    initializer_normalization::finalize_operator_obligations(
        &mut lowerer.symbol_resolved_trees,
        &lowerer.pending_const_initializers,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    Ok(())
}
