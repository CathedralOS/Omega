//! Join a closed domain application to its exact static telescope.
//!
//! Membership and interval readers share the same join: a partial tuple,
//! reordered head, open argument or mismatched canonical identity supplies no
//! binder value. This does not infer arguments from a predicate or runtime fact.

use language_semantics::SemanticDomainId;
use language_semantics::const_value::{CanonicalConstValue, DecodedCanonicalConstValue};
use numerics::bignum::BigInt;
use symbols::SymbolHandle;
use typed_trees::data::TypeParameterKind;
use typed_trees::domain::DomainDefinition;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

pub(crate) fn closed_integer_index_bindings(
    program: &dyn crate::CheckingInput,
    definition: &DomainDefinition,
    arguments: &[TypeReferenceHandle],
    semantic_domain: SemanticDomainId,
) -> Option<Vec<(SymbolHandle, BigInt)>> {
    let parameters = typed_trees::domain::index_parameters(program, definition);
    if parameters.len() != arguments.len()
        || definition.index_arguments.len() != parameters.len()
        || typed_trees::domain::has_generic_carrier(program, definition)
    {
        return None;
    }
    // The authored head names its index binders in telescope order. Solving
    // a computed or reordered head would substitute an unrelated subject.
    if !parameters
        .iter()
        .zip(&definition.index_arguments)
        .all(|(parameter, argument)| {
            matches!(program.type_reference_table.type_reference(*argument),
                TypeReferenceNode::Named { symbol, .. } if *symbol == parameter.symbol)
        })
    {
        return None;
    }
    let identity = typed_trees::domain::indexed_domain_instance_name(
        program, definition, parameters, arguments,
    )
    .ok()?;
    if !semantic_domain.is_valid()
        || program.semantic_domains.name(semantic_domain) != Some(identity.as_str())
    {
        return None;
    }
    let mut integer_bindings = Vec::new();
    for (parameter, argument) in parameters.iter().zip(arguments) {
        if !parameter.symbol.is_valid() {
            return None;
        }
        // A type index contributes to the exact application identity above,
        // but is not an integer variable in a runtime predicate. Validate its
        // complete closure before projecting only the integer-valued binders.
        // Predicate evaluation still declines any unsupported use of a type.
        if matches!(parameter.kind, TypeParameterKind::Type) {
            program
                .require_closed_application_operands(std::slice::from_ref(argument))
                .ok()?;
            continue;
        }
        let TypeParameterKind::Const { type_reference } = parameter.kind else {
            return None;
        };
        let carrier = program.primitive_type_reference(type_reference)?;
        let (minimum, maximum) =
            crate::proof_contracts::proof_embeddings::primitive_range(carrier)?;
        let value = match program.type_reference_table.type_reference(*argument) {
            TypeReferenceNode::Named { symbol, name } if !symbol.is_valid() => {
                if let Some(canonical) = CanonicalConstValue::from_atom(name.as_str()) {
                    if PrimitiveType::from_name(&canonical.type_name) != Some(carrier) {
                        return None;
                    }
                    let DecodedCanonicalConstValue::Integer { value, .. } =
                        canonical.decode_encoding()?
                    else {
                        return None;
                    };
                    BigInt::from_i128(value)
                } else {
                    BigInt::from_decimal_str(name.as_str())?
                }
            }
            TypeReferenceNode::ConstExpression(expression) => {
                crate::closed_integer_range_bound(program, *expression)?
            }
            _ => return None,
        };
        if value < minimum || value > maximum {
            return None;
        }
        integer_bindings.push((parameter.symbol, value));
    }
    Some(integer_bindings)
}
