/*
    Each use of a constant names its selected declaration. An aggregate's
    constructors and fields are resolved in the declaration's source scope.
    Typing retains the declared type, and the source driver evaluates a
    computational initializer once for the deployments that request it.

    Module-owned scoped declarations
    additionally select an exact nongeneric carrier under their declaring
    source's ordinary name law: a module-local carrier outranks imported and
    unmoduled candidates, and an exact import exposes a foreign-module carrier.
    The authored scope token survives until complete symbol assignment, then joins
    the ordinary visibility/selection ledger; the structural value encoder alone
    cannot establish attachment ownership. Seeded declarations keep their existing
    selections rather than inventing a new scope occurrence from a display name.
    Numeric and Boolean initializers owe their declared landing even when private
    and unused. Check declarations before substitution, reusing the same validator
    during module normalization so public identity construction cannot bypass it.

    Nominal index values retain their receiving application while lowering.
    Ordinary symbol assignment publishes that application's selected declaration;
    its declaration supplies the exact binder span before finalization. Argument
    storage shared by distinct applications never identifies their receiving owner.
    Body materialization retains those same nominal declaration identities without
    turning canonical index encodings into constructor authority.

    Remaining boundaries, enforced loudly:
    - Complete resolution consumes literal materializations (scalars, payloadless
      cases, and struct/array literals). Call-free integer/Boolean computations
      reach that form through build-time evaluation, retaining declaration-owned
      original syntax and selection receipts. Initializer preparation is a
      separate non-executing mode: pending expressions acquire no value identity.
      Computed integer/Boolean leaves in arrays and selected nominal literals
      evaluate independently. Calls and floating computations still need evaluation.
    - A const may not collide with a case of its scope type: `Type::NAME` must
      stay unambiguous against case-constructor paths, which substitution
      would otherwise shadow.
    - Selected numeric values retain their declared landing recursively through
      arrays. Destination checking rejoins complete array declaration types from
      retained selections, so empty arrays cannot lose their element identity.

    This module decides which initializers need evaluation and finalizes
    constants. `initializer_leaves` collects pending initializer leaves
    and their selected destinations, `declaration_values` validates and
    retains declaration values, `uses` binds authored names to their declarations, `selections` finalizes const selections and arguments,
    `normalized_expressions` validates normalized expressions,
    `scalar_carriers` classifies selected builtin scalar atoms and
    `carrier`, `initializer_dependencies` and
    `initializer_normalization` carry the const carrier and initializer
    dependencies.
*/

mod receiving_applications;

mod carrier;
mod declaration_values;
mod literal_declarations;
pub(crate) mod initializer_dependencies;
mod initializer_leaves;
pub(crate) mod initializer_normalization;
pub(crate) use initializer_normalization::constant_initializer_owns_selection;
mod normalized_expressions;
mod scalar_carriers;
mod selections;
mod uses;

pub(crate) use declaration_values::semantic_const_name;
pub(crate) use declaration_values::{
    evaluated_declaration_value, retain_const_initializer, selected_expression_constant,
    validate_const_definition,
};
#[cfg(any(test, feature = "test-support"))]
pub use initializer_leaves::PendingConstInitializerLeaf;
pub(crate) use initializer_leaves::pending_const_initializer_leaves;
pub(crate) use normalized_expressions::{
    normalization_requires_expression, validate_normalized_const_argument,
    validate_normalized_expression,
};
pub(crate) use receiving_applications::bind_receiving_application;
pub(crate) use scalar_carriers::{is_builtin_scalar_carrier, is_structural_scalar_carrier};
pub(crate) use selections::{
    finalize_const_argument_selections, finalize_const_declarations, finalize_const_selections,
};
use uses::bind_constant_uses;

use crate::resolution::lowering::lowerer::Lowerer;
use crate::resolution::names::assign_constant_expression_symbols;
use crate::resolution::tables::syntax_expressions;
use diagnostics::Diagnostic;
use initializer_dependencies::bind_named_implementation_const_calls;
use syntax_trees::SyntaxTrees;
use syntax_trees::item::ConstDefinition;

/*
    Find unfinished values without claiming a carrier or constructor selection.
    Preparation separately admits exact scalar leaves in their declared owners.
*/
pub fn requires_const_initializer_evaluation(
    syntax: &SyntaxTrees,
    definition: &ConstDefinition,
) -> bool {
    use syntax_trees::expression::ExpressionNode;
    if definition.preparation().pending_target_observation {
        return false;
    }
    let mut pending = vec![definition.value()];
    while let Some(expression) = pending.pop() {
        match syntax.expressions.expression(expression) {
            ExpressionNode::Integer(_)
            | ExpressionNode::Boolean(_)
            | ExpressionNode::String(_)
            | ExpressionNode::UndeterminedNaN(_) => {}
            ExpressionNode::Float(text)
                if text
                    .literal(syntax)
                    .is_some_and(|literal| literal.landing().is_some()) => {}
            ExpressionNode::ArrayLiteral(elements) => {
                pending.extend(
                    syntax_expressions(syntax)
                        .expression_handles(*elements)
                        .iter()
                        .copied(),
                );
            }
            ExpressionNode::StructLiteral(literal) => {
                pending.extend(
                    syntax_expressions(syntax)
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

/*
    Finalize argument selections, initializer normalization, substitution, and
    use-site selections after resolution has joined constant declarations. The
    declaration join precedes receipt helper demand and must not repeat here:
    it also records the declaration's selected carrier and visibility custody.
    Constant-expression symbol and selection passes run inside because they
    consume only these pending values. Selections already name exact declaration
    handles, including retained constants; no cohort-relative ordinal is rebased.
*/
pub(crate) fn finalize(lowerer: &mut Lowerer) -> Result<(), Vec<Diagnostic>> {
    let program = &mut lowerer.symbol_resolved_trees;
    receiving_applications::bind_receiving_parameters(
        program,
        &lowerer.pending_const_argument_slots,
    );
    finalize_const_argument_selections(
        program,
        &lowerer.pending_const_argument_selections,
        &lowerer.pending_const_argument_slots,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    let const_expression_roots = lowerer
        .pending_const_values
        .iter()
        .copied()
        .chain(lowerer.pending_const_argument_expressions.iter().copied())
        .collect::<Vec<_>>();
    assign_constant_expression_symbols(program, const_expression_roots.iter().copied());
    bind_named_implementation_const_calls(program, &const_expression_roots)
        .map_err(|message| vec![Diagnostic::error(message)])?;
    if lowerer.const_resolution_mode
        == crate::resolution::lowering::lowerer::ConstResolutionMode::Complete
    {
        declaration_values::prepare_initializer_destinations(program)
            .map_err(|diagnostic| vec![diagnostic])?;
    }
    crate::resolution::selections::authored_selections::finalize_constant_expression_selections(
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
    bind_constant_uses(
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

/*
    Close duplicate declaration-side operator obligations once the authored
    selection ledger has recorded every ordinary selection.
*/
pub(crate) fn finalize_operator_obligations(lowerer: &mut Lowerer) -> Result<(), Vec<Diagnostic>> {
    initializer_normalization::finalize_operator_obligations(
        &mut lowerer.symbol_resolved_trees,
        &lowerer.pending_const_initializers,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    Ok(())
}
