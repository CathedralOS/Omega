//! Typed lowering of mathematical `let`/`boundary let` declarations
//! (PROOF-CONTRACT-MIGRATION).
//!
//! The resolved declaration-local grammar maps onto its typed mirror
//! structurally: ordinary references reuse the shared type lowering, arrows
//! and value-level applications recurse, and the transparent term or named
//! assumption carries through verbatim. Interpreting that grammar —
//! dependent telescopes, universe classification, kernel elaboration —
//! belongs to the checked-trees elaboration leg; this stage preserves shape
//! so no authored declaration is silently dropped.

use crate::expressions::expression::lower_expression_handle;
use crate::lowerer::Lowerer;
use crate::lowerer::name::lower_name;
use crate::signatures::type_parameters::lower_type_parameters;
use crate::type_reference::lower_type_reference_into_table;
use diagnostics::Diagnostic;
use symbol_resolved_trees as resolved;
use typed_trees as typed;

pub(crate) fn lower_mathematical_definition(
    lowerer: &mut Lowerer,
    definition: &resolved::mathematical::MathematicalDefinition,
) -> Result<typed::mathematical::MathematicalDefinition, Diagnostic> {
    let mut typed_definition = typed::mathematical::MathematicalDefinition {
        symbol: definition.symbol,
        name: lower_name(&definition.name),
        is_public: definition.is_public,
        binders: lower_type_parameters(lowerer, definition.binders)?,
        parameters: Default::default(),
        result: lower_mathematical_type(lowerer, definition.result)?,
        body: match &definition.body {
            resolved::mathematical::MathematicalBody::Assumption => {
                typed::mathematical::MathematicalBody::Assumption
            }
            resolved::mathematical::MathematicalBody::Definition(term) => {
                typed::mathematical::MathematicalBody::Definition(lower_expression_handle(
                    lowerer, *term,
                )?)
            }
        },
    };

    for parameter in lowerer
        .source_trees
        .tables
        .declarations
        .mathematical_parameters
        .span_or_empty(definition.parameters)
    {
        let ty = lower_mathematical_type(lowerer, parameter.ty)?;
        lowerer.typed_trees.push_mathematical_parameter(
            &mut typed_definition,
            typed::mathematical::MathematicalParameter {
                symbol: parameter.symbol,
                name: lower_name(&parameter.name),
                relevance: parameter.relevance,
                ty,
            },
        );
    }

    Ok(typed_definition)
}

/// Lower the declaration-local mathematical type grammar node-for-node.
/// Ordinary references share the machine-typed `TypeReference` table; arrows
/// and applications are proof-surface structure and stay in the dedicated
/// mathematical type arena.
fn lower_mathematical_type(
    lowerer: &mut Lowerer,
    handle: resolved::mathematical::MathematicalTypeHandle,
) -> Result<typed::mathematical::MathematicalTypeHandle, Diagnostic> {
    let ty = match lowerer
        .source_trees
        .tables
        .declarations
        .mathematical_types
        .get(handle)
    {
        resolved::mathematical::MathematicalType::Ordinary(type_reference) => {
            typed::mathematical::MathematicalType::Ordinary(lower_type_reference_into_table(
                lowerer,
                type_reference,
            )?)
        }
        resolved::mathematical::MathematicalType::Arrow {
            binder,
            domain,
            codomain,
        } => typed::mathematical::MathematicalType::Arrow {
            binder: binder.as_ref().map(lower_name),
            domain: lower_mathematical_type(lowerer, *domain)?,
            codomain: lower_mathematical_type(lowerer, *codomain)?,
        },
        resolved::mathematical::MathematicalType::Application { callee, arguments } => {
            let arguments = lowerer
                .source_trees
                .tables
                .bodies
                .expressions
                .expression_handles(*arguments)
                .iter()
                .copied()
                .map(|argument| lower_expression_handle(lowerer, argument))
                .collect::<Result<Vec<_>, Diagnostic>>()?;
            typed::mathematical::MathematicalType::Application {
                callee: lower_mathematical_type(lowerer, *callee)?,
                arguments: lowerer
                    .typed_trees
                    .expression_table
                    .insert_expression_handles(arguments),
            }
        }
    };
    Ok(lowerer.typed_trees.insert_mathematical_type(ty))
}
