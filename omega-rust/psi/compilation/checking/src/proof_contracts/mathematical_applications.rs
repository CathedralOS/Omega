//! Source custody for mathematical applications, shared by signature
//! elaboration, specification formation and fact substitution. A mathematical
//! definition owns a checked dependent telescope and transparent term; it is
//! never an executable machine, a runtime result occurrence or a purity claim.

mod expressions;
pub use expressions::{definition_roots, expression_nodes};
mod prepared;
pub(crate) use prepared::application_spine;
pub use prepared::{
    PreparedApplication, PreparedCapture, PreparedConstant, PreparedConstantMeaning,
    PreparedMathematicalApplications, PreparedMathematicalView, integer_embedding_meaning,
};

use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use typed_trees::mathematical::{MathematicalDefinition, MathematicalType};
use typed_trees::types::TypeReferenceHandle;

pub fn selected_definition<'program>(
    program: &'program dyn crate::CheckingInput,
    call: &TableCallExpression,
) -> Option<&'program MathematicalDefinition> {
    call.target_symbol.is_valid().then_some(())?;
    program
        .mathematical_definitions()
        .iter()
        .find(|definition| definition.symbol == call.target_symbol)
}

pub fn parameter_type_reference(
    program: &dyn crate::CheckingInput,
    symbol: SymbolHandle,
) -> Option<TypeReferenceHandle> {
    symbol.is_valid().then_some(())?;
    program
        .mathematical_definitions()
        .iter()
        .find_map(|definition| {
            let parameter = program
                .mathematical_parameters(definition.parameters)
                .iter()
                .find(|parameter| parameter.symbol == symbol)?;
            match program.mathematical_type(parameter.ty) {
                MathematicalType::Ordinary(reference) => Some(*reference),
                _ => None,
            }
        })
}

/// Ordinary leaf type custody for the calculus elaborator's free source
/// operands. Computation uses the same selected-result walk as other source
/// declarations; a destination or spelling supplies no type.
pub fn operand_type_reference(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    if !program.expression_table.expression_is_valid(expression) {
        return None;
    }
    let owner = crate::reserved_result_place(program, expression)
        .map(|place| place.machine_symbol)
        .or_else(|| match program.expression_table.expression(expression) {
            ExpressionNode::Name(path) if path.symbol.is_valid() => {
                Some(program.symbols.get(path.symbol).parent)
            }
            _ => None,
        })
        .unwrap_or_else(|| program.symbols.root());
    crate::value_custody::expression_types::declaration_expression_result_type_reference(
        program,
        owner,
        &|expression| match program.expression_table.expression(expression) {
            ExpressionNode::Name(_) | ExpressionNode::Member(_) => {
                crate::proof_contracts::proof_embeddings::expression_type_reference(
                    program, expression,
                )
            }
            _ => None,
        },
        expression,
    )
}

/// Complete application result references are reusable by scalar consumers.
/// Partial applications remain mathematical function terms, not guessed
/// ordinary value types; their dependent formation is checked by the calculus.
pub fn result_type_reference(
    program: &dyn crate::CheckingInput,
    call: &TableCallExpression,
) -> Option<TypeReferenceHandle> {
    let definition = selected_definition(program, call)?;
    (call.arguments.len() == definition.parameters.len()).then_some(())?;
    match program.mathematical_type(definition.result) {
        MathematicalType::Ordinary(reference) => Some(*reference),
        _ => None,
    }
}

/// Adapt the ordinary portion of a mathematical telescope for shared source
/// checking. These are source binder views, never generated machine states.
/// Dependent functions/types retain their full MathematicalType at the kernel.
pub fn ordinary_parameters(
    program: &dyn crate::CheckingInput,
    definition: &MathematicalDefinition,
) -> Vec<typed_trees::signature::StateParameter> {
    program
        .mathematical_parameters(definition.parameters)
        .iter()
        .filter_map(|parameter| {
            let MathematicalType::Ordinary(reference) = program.mathematical_type(parameter.ty)
            else {
                return None;
            };
            Some(typed_trees::signature::StateParameter {
                symbol: parameter.symbol,
                name: parameter.name.clone(),
                type_reference: *reference,
                ..Default::default()
            })
        })
        .collect()
}

/// Selected primitive value congruence for opaque mathematical source terms.
/// The ordinary structural owner retains carrier/policy and exact bound values;
/// unsupported terms remain distinct rather than borrowing source shape keys.
pub fn source_operations_have_same_meaning(
    program: &dyn crate::CheckingInput,
    owner: SymbolHandle,
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> bool {
    let parameters = if let Some(definition) = program
        .mathematical_definitions()
        .iter()
        .find(|definition| definition.symbol == owner)
    {
        ordinary_parameters(program, definition)
    } else if let Some(machine) = program.machine_by_symbol(owner) {
        let Some(entry) = program.machine_states(machine).first() else {
            return false;
        };
        program.state_parameters(entry).to_vec()
    } else {
        return false;
    };
    crate::proof_contracts::contract_entailment::source_operations_have_same_meaning(
        program,
        owner,
        &parameters,
        left,
        right,
    )
}
