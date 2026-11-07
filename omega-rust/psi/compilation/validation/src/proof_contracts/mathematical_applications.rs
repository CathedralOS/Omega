//! Source custody for mathematical applications, shared by signature
//! elaboration, specification formation and fact substitution. A mathematical
//! definition owns a checked dependent telescope and transparent term; it is
//! never an executable machine, a runtime result occurrence or a purity claim.

mod fixed_core_vocabulary;
mod identity;
pub use fixed_core_vocabulary::{FixedCore, FixedCoreVocabulary};
pub use identity::{
    IdentityApplication, identity_application, identity_application_at, is_identity_call,
};
use typed_trees::type_identity::TypeIdentityRequest;
mod static_arguments;
pub use static_arguments::{
    MathematicalBinderKind, MathematicalCarrierHead, mathematical_binder_kind,
    mathematical_carrier_head, source_generic_argument_plan,
};
mod expressions;
pub use expressions::{
    definition_roots, erased_initializer_nodes, expression_nodes, scoped_expression_nodes,
};
mod logical_bodies;
pub use logical_bodies::{indexed_contract_is_proved, is_indexed_logical_machine};
mod float_predicates;
mod prepared;
mod scalar_predicates;
pub use float_predicates::{MathematicalFloatPredicate, float_predicates};
pub(crate) use prepared::application_spine;
pub use prepared::{
    PreparedApplication, PreparedCapture, PreparedConstant, PreparedConstantMeaning,
    PreparedIdentityValue, PreparedIndexedTheorem, PreparedMathematicalApplications,
    PreparedMathematicalView, PreparedSourceCarrier, PreparedSourceConstructor,
    PreparedSourceEvidence, integer_embedding_meaning,
};
pub use scalar_predicates::scalar_predicate;

use symbols::SymbolHandle;
use typed_trees::expression::{
    ExpressionHandle, ExpressionNode, StaticMachineArgument, TableCallExpression,
};
use typed_trees::mathematical::{MathematicalDefinition, MathematicalType};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// Mathematical declarations own dependent formation and have no runtime call.
pub fn is_mathematical_call(
    program: &dyn crate::CheckingInput,
    call: &TableCallExpression,
) -> bool {
    is_identity_call(program, call) || selected_definition(program, call).is_some()
}

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
    operand_type_reference_in_declaration(program, owner, expression)
}

/// Read an operand under its retained declaration telescope. Sibling trait
/// operations and applied signatures can return types that belong to that
/// exact owner; a context-free call query cannot export those local binders.
pub fn operand_type_reference_in_declaration(
    program: &dyn crate::CheckingInput,
    owner: SymbolHandle,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    if !program.expression_table.expression_is_valid(expression) {
        return None;
    }
    crate::value_custody::expression_types::declaration_expression_result_type_reference(
        program,
        owner,
        None,
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
        MathematicalType::Ordinary(reference) => {
            applied_result_reference(program, definition, call, *reference)
        }
        _ => None,
    }
}

/// Complete ordinary receiving types of a mathematical call's value slots.
/// These are formation inputs, not evidence that an actual inhabits its type.
pub fn argument_type_references(
    program: &dyn crate::CheckingInput,
    call: &TableCallExpression,
) -> Option<Vec<Option<TypeReferenceHandle>>> {
    if is_identity_call(program, call) {
        let application = identity_application(program, call).ok()?;
        return Some(vec![Some(application.carrier); 2]);
    }
    let definition = selected_definition(program, call)?;
    let parameters = program.mathematical_parameters(definition.parameters);
    (parameters.len() == call.arguments.len()).then_some(())?;
    Some(
        parameters
            .iter()
            .map(|parameter| match program.mathematical_type(parameter.ty) {
                MathematicalType::Ordinary(reference) => {
                    applied_result_reference(program, definition, call, *reference)
                }
                _ => None,
            })
            .collect(),
    )
}

/// Read the selected source telescope, independently of the caller's value
/// operands. The mathematical application checker owns argument validity and
/// universe inference; this projection only finds an existing ordinary result
/// reference. Inserted actual references keep their supplying scope.
fn applied_result_reference(
    program: &dyn crate::CheckingInput,
    definition: &MathematicalDefinition,
    call: &TableCallExpression,
    reference: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    program
        .type_reference_table
        .contains_type_reference(reference)
        .then_some(())?;
    let binders = program.data_type_parameters.span(definition.binders)?;
    let vocabulary = FixedCoreVocabulary::resolve(&program.symbols);
    let plan =
        source_generic_argument_plan(program, binders, &call.machine_arguments, &vocabulary)?;
    let dependencies =
        typed_trees::references::collect_type_symbol_dependencies(program, reference);
    let mut substitutions = Vec::new();
    for slot in plan {
        match slot.kind {
            MathematicalBinderKind::Level => continue,
            MathematicalBinderKind::Term => {
                // A value index is already a type-level operand when this
                // program retains its const or named reference. Substituting
                // that reference instantiates `Vector<Length, Total>` to the
                // same application the caller wrote. A runtime term with no
                // such reference stays unresolved; it is not reclassified as
                // a Type, and the kernel still checks the term itself.
                if !dependencies.contains(&slot.binder.symbol) {
                    continue;
                }
                let argument = slot.argument?;
                let actual = value_index_reference(program, argument)?;
                substitutions.push((slot.binder.symbol, actual));
                continue;
            }
            MathematicalBinderKind::Type => {}
        }
        if !dependencies.contains(&slot.binder.symbol) {
            continue;
        }
        let argument = slot.argument?;
        if argument.application.is_some()
            || argument.evidence_projection.is_some()
            || argument.const_literal.is_some()
        {
            return None;
        }
        let actual = if program
            .type_reference_table
            .contains_type_reference(argument.type_reference)
        {
            argument.type_reference
        } else {
            argument.symbol.is_valid().then_some(())?;
            program
                .type_reference_table
                .find_named_type_reference(argument.symbol)?
        };
        substitutions.push((slot.binder.symbol, actual));
    }
    // A direct formal result is already represented by its actual operand;
    // consuming it once avoids reinterpreting recursive caller binders.
    let projected =
        typed_trees::static_applications::project_type_operand(program, reference, &substitutions);
    if projected != reference || substitutions.is_empty() {
        return Some(projected);
    }
    let identity = program.type_identity_with_simultaneous_substitutions(TypeIdentityRequest {
        substitutions: &substitutions,
        ..TypeIdentityRequest::ordinary(reference)
    });
    identity.is_valid().then_some(())?;
    if program.normalized_type_identity(reference) == identity {
        return Some(reference);
    }
    // Compound results may already have a source occurrence in this program.
    // Match its full identity, including nested policy/qualification, instead
    // of copying the source graph or borrowing a comparison peer's carrier.
    program
        .type_reference_table
        .type_reference_rows()
        .find_map(|(candidate, _)| {
            (program.normalized_type_identity(candidate) == identity).then_some(candidate)
        })
}

/// The type-level reference a value-index argument already has. Integer
/// literals on a call (`f<1, 7>`) keep the number and no reference of their
/// own; the same literal's const expression, written on the instantiated
/// carrier, is the reference identity substitution already understands.
fn value_index_reference(
    program: &dyn crate::CheckingInput,
    argument: &StaticMachineArgument,
) -> Option<TypeReferenceHandle> {
    if argument.application.is_some() || argument.evidence_projection.is_some() {
        return None;
    }
    if program
        .type_reference_table
        .contains_type_reference(argument.type_reference)
    {
        return Some(argument.type_reference);
    }
    if argument.symbol.is_valid() {
        return program
            .type_reference_table
            .find_named_type_reference(argument.symbol);
    }
    let literal = argument.const_literal.as_ref()?;
    program
        .type_reference_table
        .type_reference_rows()
        .find_map(|(handle, node)| {
            let TypeReferenceNode::ConstExpression(expression) = node else {
                return None;
            };
            matches!(
                program.expression_table.expression(*expression),
                ExpressionNode::Integer(value) if value == literal
            )
            .then_some(handle)
        })
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

/// The mathematical producer retains formation of source indexed carriers
/// under their exact value telescope. Erasure may carry those checked terms
/// even when no closed runtime layout exists. This does not classify the data
/// or its containing machine as proof-only.
pub fn is_source_evidence_carrier(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> bool {
    program.prepared_mathematics().is_some_and(|view| {
        view.source_carrier(reference).is_some_and(|carrier| {
            carrier.type_identity.is_valid()
                && carrier.type_identity == program.normalized_type_identity(reference)
        })
    })
}
