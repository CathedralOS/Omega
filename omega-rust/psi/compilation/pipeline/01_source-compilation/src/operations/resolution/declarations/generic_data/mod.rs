/*
    Structural arguments retain the declarations and values chosen by resolution.

    Shared type/constant utilities serve declaration validation and structural
    machine equations. The private specializer consumes only the closed type roots
    introduced by equation completion, after provisional resolution has selected
    their value names. Its templates, substitutions and pending instances are
    private working state; synthesized declarations remain in SyntaxTrees.
*/

use crate::resolution::declarations::declaration_selection;
use crate::resolution::tables::syntax_type_references;
use application_identity::const_value::CanonicalConstValue;
use arena::HandleSpan;
use diagnostics::Diagnostic;
use numeric_literals::IntegerLiteral;
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
pub(crate) mod domain_heads;
mod eligibility;
pub(crate) use crate::resolution::declarations::type_equations as equations;
pub(crate) use equations::{template_type_equation_offsets, validate_materialized_type_equations};
pub(crate) mod source_modules;
mod substitution;
mod synthesis;
mod uses;

mod work_domain;
#[cfg(any(test, feature = "test-support"))]
pub(crate) use uses::statement_call_inputs;
pub(crate) use uses::statement_type_inputs;
pub(super) use uses::{
    collect_expression_handles, collect_statement_expression_handles, visit_expression_type_inputs,
    visit_static_argument_type_inputs,
};

pub(crate) use arguments::{
    DomainCarrier, DomainHead, closed_argument_identity, closed_parameter_argument_identity,
    domain_application_head, domain_index_telescope, monomorphizable_argument_slugs,
    normalize_domain_head_positions, selected_domain_family, type_reference_slug,
};
pub(in crate::resolution::declarations::generic_data) use arguments::{
    canonicalize_monomorphizable_argument_handles, complete_argument_tuple,
};
pub(crate) use const_evaluation::{
    ConstFactValue, ConstScalarSpelling, ConstScalarValue, closed_fixed_array_length,
    evaluate_const_fact_binary, evaluate_const_fact_expression, evaluate_const_membership_fact,
    fold_closed_scalar_initializers,
};
pub(in crate::resolution::declarations::generic_data) use const_evaluation::{
    ConstPredicateAdmission, EvaluatedConst, canonicalize_closed_domain_indices,
    canonicalize_selected_declaration_value,
    canonicalize_selected_declaration_value_with_admission, canonicalize_selected_index_expression,
    capture_machine_runtime_template_names, collect_data_type_reference_positions,
    const_integer_type, const_integer_within_budget, domain_index_parameters,
    evaluate_const_argument_expression, generic_const_integer_types, integer_literal_value,
    normalize_generic_template_const_expressions, prepare_closed_application,
    prove_const_leaf_domain_constraints, qualified_const_name, replace_const_expression_names_from,
    replace_machine_const_expression_names_from, syntax_type_identity,
    validate_syntax_integer_range,
};
pub(in crate::resolution::declarations) use const_evaluation::{
    canonicalize_selected_const_definition, collect_type_positions,
};
use declaration_selection::ConstantSelection;
pub(crate) use discovery::{ClosedArgumentIdentity, ClosedConstraintIdentity, selected_data_item};
pub(in crate::resolution::declarations::generic_data) use discovery::{
    GenericData, GenericDataShape, Instantiation, PendingRewrite, selected_generic_data,
};
pub(crate) use substitution::{
    closed_name_identity, substitute_member, type_reference_mentions_parameter,
};

pub(crate) use synthesis::desugar_generic_data_instances_in;
pub(crate) use work_domain::GenericDataWorkDomain;

/*
    Whether this exact selected destination carries declaration predicates.
    This schedules the ordinary checked result judgment; it proves nothing.
*/
pub(crate) fn const_destination_requires_proof(
    syntax: &SyntaxTrees,
    definition: &ConstDefinition,
    selection: Option<&ConstantSelection>,
) -> bool {
    const_evaluation::const_type_requires_declaration_proof(
        syntax,
        definition.type_reference,
        &mut Vec::new(),
        selection,
    )
}

/*
    Whether the definition's retained qualification names its own destination
    and initializer: the qualification the definition itself already satisfies.
*/
pub(super) fn qualification_names_itself(definition: &ConstDefinition) -> bool {
    definition
        .preparation()
        .qualification
        .as_ref()
        .is_some_and(|qualification| {
            qualification.destination == definition.type_reference
                && qualification.initializer == definition.value()
        })
}

/*
    Canonicalize one source const declaration against its own declared type.

    This is the narrow handoff used by declaration/API retention. The returned
    value's structural encoding is semantic material; its display text remains
    diagnostic-only. A constrained carrier canonicalizes at its base and then
    owes the compile-time owner's checked declaration-site result judgment.
    The judgment retains the exact destination and completed initializer;
    encoding alone cannot publish an unproved qualification.
*/
pub(crate) fn canonicalize_selected_declared_const_definition(
    syntax: &SyntaxTrees,
    definition: &ConstDefinition,
    selection: Option<&ConstantSelection>,
) -> Result<CanonicalConstValue, String> {
    if definition.preparation().pending_qualification.is_some() {
        return Err("constant destination qualification remains pending".to_owned());
    }
    let mut payload = definition.clone();
    if let TypeReferenceNode::Constrained {
        base_type,
        constraints,
    } = syntax_type_references(syntax).type_reference(definition.type_reference)
    {
        let has_domain = syntax_type_references(syntax)
            .constraints(*constraints)
            .iter()
            .any(|constraint| matches!(constraint, TypeConstraintNode::Domain(_)));
        if has_domain && !qualification_names_itself(definition) {
            return Err(
                "constant qualification requires a checked declaration-site result judgment"
                    .to_owned(),
            );
        }
        if has_domain {
            payload.declaration.type_reference = *base_type;
        }
    }
    if qualification_names_itself(definition) {
        canonicalize_selected_declaration_value_with_admission(
            syntax,
            &payload,
            selection,
            ConstPredicateAdmission::CheckedDestination,
        )
    } else {
        canonicalize_selected_declaration_value(syntax, &payload, selection)
    }
}

/*
    Replay direct structural arguments at their actual generic owner. The
    encoded label is consistency data; the selected template's parameter owns
    the carrier used by the existing canonical encoder.
*/
pub(crate) fn validate_direct_const_arguments(
    syntax: &SyntaxTrees,
    base_name: &Identifier,
    arguments: HandleSpan<TypeReferenceHandle>,
    selection: Option<&ConstantSelection>,
) -> Result<(), Diagnostic> {
    let arguments = syntax.type_references.type_reference_handles(arguments);
    let has_direct = arguments.iter().any(|argument| {
        syntax_type_references(syntax)
            .const_argument_normalization(*argument)
            .is_some_and(crate::resolution::constants::normalization_requires_expression)
    });
    if !has_direct {
        return Ok(());
    }
    let selection = selection.ok_or_else(|| {
        Diagnostic::error("direct constant argument has no declaration selection context")
            .with_source_span(base_name.source_span())
    })?;
    let definition = selection
        .data(syntax, base_name)
        .map_err(|message| Diagnostic::error(message).with_source_span(base_name.source_span()))?;
    let parameters = syntax.items.type_parameters(definition.type_parameters);
    if parameters.len() != arguments.len() {
        return Err(
            Diagnostic::error("direct constant argument lost its exact template slot")
                .with_source_span(base_name.source_span()),
        );
    }
    for (argument, parameter) in arguments.iter().zip(parameters) {
        let Some(normalization) =
            syntax_type_references(syntax).const_argument_normalization(*argument)
        else {
            continue;
        };
        if !crate::resolution::constants::normalization_requires_expression(normalization) {
            continue;
        }
        crate::resolution::constants::validate_normalized_expression(syntax, normalization)?;
        let TypeParameterKind::Const { type_reference } = parameter.kind else {
            return Err(Diagnostic::error(
                "direct constant argument no longer selects a const parameter",
            )
            .with_source_span(normalization.reference));
        };
        let value = canonicalize_selected_index_expression(
            syntax,
            type_reference,
            type_reference,
            if normalization.materialized_expression.is_valid() {
                normalization.materialized_expression
            } else {
                normalization.authored_expression
            },
            Some(selection),
        )
        .map_err(|message| Diagnostic::error(message).with_source_span(normalization.reference))?;
        if value.encoding != normalization.canonical_result_encoding {
            return Err(Diagnostic::error(
                "direct constant expression differs from its normalized value",
            )
            .with_source_span(normalization.reference));
        }
    }
    Ok(())
}
