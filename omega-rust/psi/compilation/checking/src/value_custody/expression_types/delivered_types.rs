//! A value delivered with a complete type keeps that type at its destination.
//!
//! A call occurrence's applied result, a named binding's declared type and a
//! stored field or element read under its receiver's application are
//! complete: the program fixed them before the value reached the store. A
//! destination annotation, a call formal or a returned-value context never
//! becomes the value's type merely by being written. One comparison judges
//! every delivery: a reference keeps the carrier correspondence a call
//! argument owes its formal, and a by-value carrier keeps the identity of
//! every type parameter that is rigid where the store happens and of every
//! applied data argument.
//!
//! A type parameter is rigid inside the body of the machine that declares it:
//! there it names one unknown type, equal only to itself. Another
//! declaration's parameter seen unsubstituted is not a type at the store: in
//! a destination it is an open slot (a constructor field, a trait
//! requirement, a callee formal without an application) judged by the
//! application that closes it; in a delivered type it is an application its
//! producer left incomplete. Neither is judged here.
//!
//! Applied data arguments compare position by position, through nested
//! applications: a `Holder<Bar>` is not a `Holder<Foo>`, whether either side
//! is spelled as an application or as the concrete instance generated for
//! it. Two distinct declared concrete carriers at the store itself are judged
//! elsewhere: data declarations by the nominal store guard, primitives by the
//! class and narrowing checks; a directly spelled application there has no
//! declaration for that guard to name and is judged here. A reference formed
//! at the store (a borrow, a recast, a window, a literal) takes its carrier
//! from that formation and stays with the access, recast and domain-weakening
//! checks.

use super::{
    ExpressionHandle, ExpressionNode, TypeReferenceHandle, TypeReferenceNode,
    named_value_type_reference, reference_values,
};
use crate::frozen_program::expression_table;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::types::PrimitiveType;

/// The complete type a value arrives with, when the program already fixed
/// it, and the receiver applications its declared type is read under.
pub(super) struct DeliveredType {
    pub(super) reference: TypeReferenceHandle,
    /// A field's declared type names its data declaration's parameters; a
    /// read through an applied receiver supplies them. Empty for a call
    /// result or a named binding, whose types are already the caller's.
    pub(super) substitutions: Vec<(SymbolHandle, TypeReferenceHandle)>,
}

/// The complete type a value arrives with: a call occurrence's applied
/// result, a named binding's declared type, or a stored field or element
/// read under its receiver's application. A call with no applied result
/// keeps only its declaration's own binders, which cannot be compared with
/// the caller's.
pub(super) fn delivered_value_type(
    program: &dyn crate::CheckingInput,
    value: ExpressionHandle,
) -> Option<DeliveredType> {
    // Constant substitution changes the payload syntax, not the complete type
    // selected by this authored value use. Preserve its application operands
    // before contextual literal formation can choose another destination.
    if let Some(declaration) = program.expression_const_declaration(value) {
        return declaration
            .declared_type
            .is_valid()
            .then_some(DeliveredType {
                reference: declaration.declared_type,
                substitutions: Vec::new(),
            });
    }
    let mut substitutions = Vec::new();
    let reference = match expression_table(program).expression(value) {
        ExpressionNode::Call(_) => {
            expression_table(program).constructed_value_type_reference(value)
        }
        ExpressionNode::Name(path) => named_value_type_reference(program, path)?,
        ExpressionNode::Member(_) | ExpressionNode::Indexed(_) => {
            reference_values::declared_value_type(program, value, &mut substitutions)?
        }
        ExpressionNode::Atomic(_) => {
            let load = crate::atomic_load_carrier(program, value)?;
            let cell =
                reference_values::declared_value_type(program, load.place, &mut substitutions)?;
            let cell = reference_values::substituted_reference(program, cell, &substitutions)?;
            // An observation delivers the resident scalar, not the cell's
            // declaration or its receiver's generic substitutions.
            substitutions.clear();
            crate::atomic_resident_type_reference(program, cell)?
        }
        _ => return None,
    };
    reference.is_valid().then_some(DeliveredType {
        reference,
        substitutions,
    })
}

/// Where a compared carrier sits: the stored value itself, or an applied
/// data argument (or an element reached through one) inside it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Position {
    Stored,
    Applied,
}

/// A generated concrete instance names its application: `Holder<Foo>`'s
/// nominal symbol stands for `Holder` applied to `Foo`. Comparing through
/// that origin lets an instance meet the same application spelled directly.
/// The instance may carry lifetime arguments of its own; they supply no type
/// argument.
pub(super) fn application_form(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> TypeReferenceHandle {
    let symbol = match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Named { symbol, .. } => symbol,
        TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } if arguments.count() == 0 => base_symbol,
        _ => return reference,
    };
    if let Some(origin) = program
        .data_definition_by_symbol(*symbol)
        .and_then(|definition| definition.generic_instance)
        && origin.is_valid()
    {
        program
            .type_reference_table
            .unconstrained_type_reference(origin)
    } else {
        reference
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Carrier {
    Parameter(SymbolHandle),
    Data(SymbolHandle),
    Implementation(SymbolHandle),
    Primitive(PrimitiveType),
}

fn carrier(program: &dyn crate::CheckingInput, reference: TypeReferenceHandle) -> Option<Carrier> {
    if let Some(primitive) = program.primitive_type_reference(reference) {
        return Some(Carrier::Primitive(primitive));
    }
    let symbol = program.type_reference_table.type_head_symbol(reference)?;
    if !symbol.is_valid() {
        return None;
    }
    match program.symbols.get(symbol).kind {
        symbols::SymbolKind::TypeParameter | symbols::SymbolKind::ConformanceParameter => {
            Some(Carrier::Parameter(symbol))
        }
        symbols::SymbolKind::Data => Some(Carrier::Data(symbol)),
        symbols::SymbolKind::Conformance => Some(Carrier::Implementation(symbol)),
        _ => None,
    }
}

/// Whether a by-value `actual` contradicts `required` under the binders
/// that are rigid in `scope`. Declared-domain shells qualify a carrier
/// without changing it; applied data arguments and array elements compare
/// position by position. `actual_substitutions` supplies the receiver
/// applications a delivered field type is read under;
/// `required_substitutions` is the call's simultaneous formal-to-caller
/// environment, empty at a plain store.
pub(super) fn delivered_carrier_conflict(
    program: &dyn crate::CheckingInput,
    scope: SymbolHandle,
    context: ExpressionHandle,
    actual: TypeReferenceHandle,
    actual_substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    required: TypeReferenceHandle,
    required_substitutions: &[(SymbolHandle, TypeReferenceHandle)],
) -> bool {
    scope.is_valid()
        && carrier_conflict(
            program,
            scope,
            context,
            actual,
            actual_substitutions,
            required,
            required_substitutions,
            Position::Stored,
        )
}

fn carrier_conflict(
    program: &dyn crate::CheckingInput,
    scope: SymbolHandle,
    context: ExpressionHandle,
    actual: TypeReferenceHandle,
    actual_substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    required: TypeReferenceHandle,
    required_substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    position: Position,
) -> bool {
    let (required, required_substitutions) =
        reference_values::substituted_required_reference(program, required, required_substitutions);
    let unconstrained = |reference| {
        program
            .type_reference_table
            .unconstrained_type_reference(reference)
    };
    // A receiver application that cannot be read without re-entering its own
    // telescope is not a complete delivered type.
    let Some(actual) = reference_values::substituted_reference(
        program,
        unconstrained(actual),
        actual_substitutions,
    ) else {
        return false;
    };
    // A type binder is rigid except where an already established static
    // equality refines it. Read that occurrence's context after application
    // substitution, including at every nested carrier comparison. Neither
    // the destination nor the value being formed supplies a new assumption.
    let actual = program.statically_refined_type_reference(context, unconstrained(actual));
    let required = program.statically_refined_type_reference(context, unconstrained(required));
    let rigid = |carrier: Carrier| {
        matches!(carrier, Carrier::Parameter(parameter)
            if program.symbols.get(parameter).parent == scope)
    };
    match (
        program.type_reference_table.type_reference(actual),
        program.type_reference_table.type_reference(required),
    ) {
        (
            TypeReferenceNode::FixedArray {
                element_type: actual,
                ..
            },
            TypeReferenceNode::FixedArray {
                element_type: required,
                ..
            },
        ) => {
            return carrier_conflict(
                program,
                scope,
                context,
                *actual,
                actual_substitutions,
                *required,
                required_substitutions,
                position,
            );
        }
        (TypeReferenceNode::Reference { .. }, _) | (_, TypeReferenceNode::Reference { .. }) => {
            return false;
        }
        _ => {}
    }
    let (Some(actual_carrier), Some(required_carrier)) =
        (carrier(program, actual), carrier(program, required))
    else {
        return false;
    };
    // A parameter another declaration owns is not a type at this store: in
    // the destination it is an open slot, in a delivered type an application
    // the producer left incomplete. Neither is judged here.
    let foreign = |carrier: Carrier| matches!(carrier, Carrier::Parameter(_)) && !rigid(carrier);
    if foreign(actual_carrier) || foreign(required_carrier) {
        return false;
    }
    if actual_carrier != required_carrier && (rigid(actual_carrier) || rigid(required_carrier)) {
        return true;
    }
    // Two declared concrete carriers at the store itself belong to the
    // nominal guard, which names both declarations. An application spelled
    // directly (a call's applied result, a receiver-applied field) is judged
    // here.
    let declared = |reference| {
        matches!(
            program.type_reference_table.type_reference(reference),
            TypeReferenceNode::Named { .. }
        )
    };
    if position == Position::Stored && declared(actual) && declared(required) {
        return false;
    }
    let actual = application_form(program, actual);
    let required = application_form(program, required);
    let (Some(actual_carrier), Some(required_carrier)) =
        (carrier(program, actual), carrier(program, required))
    else {
        return false;
    };
    if actual_carrier != required_carrier {
        // Inside an application every argument is part of the type, and an
        // application at the store has no declared carrier for the nominal
        // guard to name: two distinct concrete carriers are distinct types.
        return true;
    }
    let (
        TypeReferenceNode::Generic {
            arguments: actual_arguments,
            ..
        },
        TypeReferenceNode::Generic {
            arguments: required_arguments,
            ..
        },
    ) = (
        program.type_reference_table.type_reference(actual),
        program.type_reference_table.type_reference(required),
    )
    else {
        return false;
    };
    let actual_arguments = program
        .type_reference_table
        .type_reference_handles(*actual_arguments);
    let required_arguments = program
        .type_reference_table
        .type_reference_handles(*required_arguments);
    let implementation_parameters = match actual_carrier {
        Carrier::Implementation(symbol) => program
            .conformance_by_symbol(symbol)
            .map(|declaration| program.conformance_type_parameters(declaration)),
        _ => None,
    };
    actual_arguments.len() == required_arguments.len()
        && actual_arguments
            .iter()
            .zip(required_arguments.iter())
            .enumerate()
            .any(|(ordinal, (actual, required))| {
                // A selected implementation's own static operands are part of
                // its identity, not runtime carriers whose absence can waive
                // comparison. Type operands retain the contextual/rigid
                // correspondence below. Normalize supplied value bindings in
                // the same chained/receiving-simultaneous scopes as references.
                if let Some(parameter) = implementation_parameters
                    .and_then(|parameters| parameters.get(ordinal))
                    .filter(|parameter| {
                        !matches!(
                            parameter.kind,
                            typed_trees::data::TypeParameterKind::Type
                                | typed_trees::data::TypeParameterKind::Implementation
                        )
                    })
                {
                    return static_implementation_operand_conflict(
                        program,
                        scope,
                        &parameter.kind,
                        (*actual, actual_substitutions),
                        (*required, required_substitutions),
                    );
                }
                carrier_conflict(
                    program,
                    scope,
                    context,
                    *actual,
                    actual_substitutions,
                    *required,
                    required_substitutions,
                    Position::Applied,
                )
            })
}

fn static_implementation_operand_conflict(
    program: &dyn crate::CheckingInput,
    scope: SymbolHandle,
    kind: &typed_trees::data::TypeParameterKind,
    actual: (TypeReferenceHandle, &[(SymbolHandle, TypeReferenceHandle)]),
    required: (TypeReferenceHandle, &[(SymbolHandle, TypeReferenceHandle)]),
) -> bool {
    use typed_trees::data::TypeParameterKind;
    use typed_trees::type_identity::{IndexIdentityArgument, StaticOperandIdentityRequest};
    let compare = |actual_operand, required_operand| {
        let actual_identity = program.static_operand_identity(StaticOperandIdentityRequest {
            operand: actual_operand,
            lexical_owner: scope,
            substitutions: actual.1,
        });
        let required_identity = program.static_operand_identity_with_simultaneous_substitutions(
            StaticOperandIdentityRequest {
                operand: required_operand,
                lexical_owner: scope,
                substitutions: required.1,
            },
        );
        !matches!((actual_identity, required_identity), (Some(actual), Some(required)) if actual == required)
    };
    match kind {
        // Different static value identities can still be equal under a checked
        // caller premise. The application-index checker owns that comparison
        // together with capture time, effects and current scalar relations.
        TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. } => false,
        TypeParameterKind::Machine { .. } | TypeParameterKind::Proposition { .. } => {
            let Ok(actual_operand) = program.static_operand_from_type_reference(actual.0) else {
                return true;
            };
            let Ok(required_operand) = program.static_operand_from_type_reference(required.0)
            else {
                return true;
            };
            compare(
                IndexIdentityArgument::StaticArgument(&actual_operand),
                IndexIdentityArgument::StaticArgument(&required_operand),
            )
        }
        TypeParameterKind::Type | TypeParameterKind::Implementation => false,
    }
}

/// The delivered type that fails its destination, if any: a reference by
/// carrier correspondence, a by-value carrier by rigid parameter and applied
/// argument identity.
/// Selected arms each deliver their own value.
fn delivered_type_conflict(
    program: &dyn crate::CheckingInput,
    scope: SymbolHandle,
    value: ExpressionHandle,
    destination: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    if let Some(dispatch) = expression_table(program).match_node(value) {
        return expression_table(program)
            .match_arms(dispatch.arms)
            .iter()
            .find_map(|arm| delivered_type_conflict(program, scope, arm.value, destination));
    }
    let DeliveredType {
        reference: delivered,
        substitutions,
    } = delivered_value_type(program, value)?;
    let conflict = if matches!(
        program.type_reference_table.type_reference(destination),
        TypeReferenceNode::Reference { .. }
    ) {
        matches!(
            program.type_reference_table.type_reference(delivered),
            TypeReferenceNode::Reference { .. }
        ) && !reference_values::value_carrier_matches_reference(
            program,
            delivered,
            destination,
            &substitutions,
            &[],
        )
    } else {
        delivered_carrier_conflict(
            program,
            scope,
            value,
            delivered,
            &substitutions,
            destination,
            &[],
        )
    };
    // Name the delivered type the store saw: a field read's declared type
    // under its receiver's application.
    conflict.then(|| {
        reference_values::substituted_reference(program, delivered, &substitutions)
            .unwrap_or(delivered)
    })
}

/// A value received into storage the program names -- a local, an assigned
/// place, a transition's returned value -- keeps the type it was delivered
/// with. `scope` is the machine whose body performs the store.
pub(crate) fn report_delivered_type_mismatch(
    program: &dyn crate::CheckingInput,
    scope: SymbolHandle,
    value: ExpressionHandle,
    destination: TypeReferenceHandle,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(delivered) = delivered_type_conflict(program, scope, value, destination) {
        diagnostics.push(Diagnostic::error(format!(
            "{owner} expects `{}`, but the value is delivered as `{}`",
            program.display_type_reference_with_constraints(destination),
            program.display_type_reference_with_constraints(delivered),
        )));
    }
}
