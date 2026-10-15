//! Expression types: whether an expression's value may be delivered to a
//! declared type, and which type an expression produces.
//!
//! This file owns expression validation and the type queries it shares with callers.
//! `validate_expression_type_handle` is the check: the expression-statement
//! validator in `program_validation::statements` runs it on a state's
//! terminal expression. It reports a string literal that exceeds a bounded
//! text carrier's capacity, and otherwise a value whose type does not match.
//! `argument_matches_type_reference_handle` compares one argument with a
//! declared type for the call, machine-data, machine-parameter and proof
//! validators; the crate exports it as `checked_argument_matches_type_reference`
//! for package review. Runtime calls use `argument_matches_carrier`, which
//! shares that expression walk but leaves declared-domain proof to their flow
//! contract checker. `named_value_type_reference` finds the declared type
//! of a named binder, `bounded_byte_buffer_capacity` reads the capacity of a
//! domain-constrained `[u8; N]` text carrier, and `expression_type_name_handle`
//! names an expression's kind in diagnostics.
//!
//! The module list below groups the children by role:
//!
//! - The expression walk in `machine_calls::calls::expression_scanning` runs
//!   the per-expression checks: `match_dispatch` (`validate_match_dispatch`,
//!   which build-time evaluation also calls), `operator_validation`
//!   (`validate_binary_operand_types`, the unary logical-not and
//!   bitwise-not reports, and the non-integer index report) and
//!   `cast_validation` (`validate_cast_types`).
//! - The statement validators in `program_validation::statements`, call
//!   validation and struct-literal field obligations run the store checks on a
//!   value they store or pass: `shape_validation` (array, scalar, data and
//!   view element shape mismatches) and `value_classification` (cross-class
//!   stores and data type conflicts). `value_classification` also supplies
//!   the value classes the other checks compare.
//! - `result_type` (an expression's result type, independent of its
//!   destination), `reference_values` (reference type matching and mutable
//!   reference forwarding), and `match_dispatch` (`match_case_dispatch`) are
//!   read by other validators, checking and lowering. `scalar_observations`
//!   interprets a selected builtin scalar read without changing a reference's
//!   declared type or granting a referent value. Structural edge traversal
//!   and occurrence collection belong to `crate::expression_traversal`.

use diagnostics::Diagnostic;
use std::fmt;
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::type_identity::TypeIdentityRequest;
use typed_trees::types::{
    PrimitiveType, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};

// Per-expression checks the expression walk in
// `machine_calls::calls::expression_scanning` runs. Exact coercion validation
// shares that walk's live value facts; it never replays guards or initializers.
mod cast_validation;
mod float_coercions;
mod float_operations;
mod match_dispatch;
mod operator_validation;
pub(crate) use float_operations::validate_float_expression_formats;

// Store checks on a value delivered to a declared place, parameter, field or
// element, and the float format check `value_classification` runs for them.
mod float_destinations;
mod shape_validation;
mod value_classification;

// Shared vocabulary: result types, delivered types and reference type matching.
mod delivered_types;
mod reference_values;
mod result_type;
mod scalar_observations;

pub use match_dispatch::match_subject_primitive_type;
pub use match_dispatch::validate_match_dispatch;
pub use match_dispatch::{
    MatchCaseDispatch, MatchCaseDispatchArm, MatchCaseSubject, match_case_dispatch,
};
pub use result_type::{
    arithmetic_result_type_reference, expression_result_type_reference,
    join_result_type_references, parameter_expression_result_type_reference,
};

pub(crate) use delivered_types::report_delivered_type_mismatch;
pub(crate) use reference_values::{
    declared_reference_access, place_forwarded_exclusive_reference_access,
    place_permits_reference_borrow, place_permits_storage_write,
};
pub(crate) use result_type::declaration_expression_result_type_reference;
pub(crate) use result_type::domain_expression_result_type_reference;
pub(crate) use result_type::{
    cast_result_type_reference, operand_arithmetic_domain, selected_operator_result_type_reference,
};
pub(crate) use scalar_observations::{
    builtin_scalar_operand_references, scalar_observation_meaning,
};

pub(crate) use cast_validation::validate_cast_types;
pub(crate) use float_coercions::{
    float_coercion_preserves_denotation, validate_exact_float_coercion,
};

pub(crate) use operator_validation::{
    report_non_bool_logical_not, report_non_integer_bitwise_not, report_non_integer_index,
    validate_binary_operand_types,
};

pub(crate) use shape_validation::{
    report_array_scalar_shape_mismatch, report_scalar_data_shape_mismatch,
    report_view_element_argument_mismatch,
};

pub(crate) use value_classification::ValueClass;
pub(crate) use value_classification::{report_cross_class_store, report_data_type_conflict};

#[derive(Debug, Clone, Copy)]
pub(crate) enum ExpressionTypeOwner<'program> {
    StateTerminalExpression {
        machine: &'program str,
        state: &'program str,
    },
}

impl fmt::Display for ExpressionTypeOwner<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StateTerminalExpression { machine, state } => {
                write!(
                    formatter,
                    "machine `{machine}` state `{state}` terminal expression"
                )
            }
        }
    }
}

/// Recheck an argument's declared type correspondence. This supplies neither
/// flow facts nor domain proof; post-check and logical-formation consumers use
/// the declaration-backed reference relation instead of runtime call evidence.
pub fn argument_matches_type_reference_handle(
    program: &dyn crate::CheckingInput,
    argument: ExpressionHandle,
    type_reference: TypeReferenceHandle,
) -> bool {
    argument_matches_type_reference_with_substitutions(program, argument, type_reference, &[])
}

/// Declaration-backed correspondence under an exact simultaneous telescope.
/// This does not establish proof facts, conversions, or borrowing authority.
pub(crate) fn argument_matches_type_reference_with_substitutions(
    program: &dyn crate::CheckingInput,
    argument: ExpressionHandle,
    type_reference: TypeReferenceHandle,
    substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
) -> bool {
    argument_matches_type_reference_using(
        program,
        SymbolHandle::invalid(),
        argument,
        type_reference,
        reference_values::declared_value_matches_reference,
        substitutions,
    )
}

pub(crate) fn argument_matches_carrier(
    program: &dyn crate::CheckingInput,
    scope: SymbolHandle,
    argument: ExpressionHandle,
    type_reference: TypeReferenceHandle,
    required_substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
) -> bool {
    let (type_reference, required_substitutions) = reference_values::substituted_required_reference(
        program,
        type_reference,
        required_substitutions,
    );
    // The opaque service shell does not erase its requirement identity. Normal
    // generic carrier matching may defer an open data argument, but a Binding
    // value must carry the same closed requirement identity.
    // This only compares denotation: borrowing and establishment remain with
    // their independent source custody checks.
    let required_service = match typed_trees::service::classify_exact_service_parameter_carrier(
        program,
        typed_trees::service::ServiceCarrierOccurrence {
            type_reference,
            lexical_owner: scope,
            enclosing_data_applications: &[],
        },
    ) {
        Ok(service) => service,
        Err(_) => return false,
    };
    if let Some(required) = required_service {
        let source = program
            .expression_table
            .borrow_expression(argument)
            .map_or(argument, |borrow| borrow.target);
        let mut substitutions = Vec::new();
        let Some(actual) =
            reference_values::declared_value_type(program, source, &mut substitutions)
                .and_then(|reference| {
                    reference_values::substituted_reference(program, reference, &substitutions)
                })
                .and_then(|reference| {
                    typed_trees::service::classify_exact_service_parameter_carrier(
                        program,
                        typed_trees::service::ServiceCarrierOccurrence {
                            type_reference: reference,
                            lexical_owner: scope,
                            enclosing_data_applications: &[],
                        },
                    )
                    .ok()
                    .flatten()
                })
        else {
            return false;
        };
        if actual.carrier != required.carrier {
            return false;
        }
    }
    // A call may attenuate an explicit mutable borrow to shared access. The
    // referent still passes through ordinary carrier matching; the caller
    // separately proves required domains and validates loan exclusivity.
    if let Some(borrow) = program.expression_table.borrow_expression(argument)
        && borrow.access == language_semantics::ReferenceAccess::Mutable
        && let TypeReferenceNode::Reference {
            referee,
            access: language_semantics::ReferenceAccess::Shared,
            ..
        } = program.type_reference_table.type_reference(type_reference)
    {
        return argument_matches_type_reference_using(
            program,
            scope,
            borrow.target,
            *referee,
            reference_values::value_carrier_matches_reference,
            required_substitutions,
        );
    }
    argument_matches_type_reference_using(
        program,
        scope,
        argument,
        type_reference,
        reference_values::value_carrier_matches_reference,
        required_substitutions,
    )
}

fn argument_matches_type_reference_using(
    program: &dyn crate::CheckingInput,
    scope: SymbolHandle,
    argument: ExpressionHandle,
    type_reference: TypeReferenceHandle,
    correspondence: reference_values::ReferenceCorrespondence,
    required_substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
) -> bool {
    let (type_reference, required_substitutions) = reference_values::substituted_required_reference(
        program,
        type_reference,
        required_substitutions,
    );
    if let Some(dispatch) = program.expression_table.match_node(argument) {
        // Selection changes which value arrives, not its required carrier.
        // Reference arms obey the same exact referent/access correspondence;
        // independent access and loan checks still own their authority.
        return !program
            .expression_table
            .match_arms(dispatch.arms)
            .is_empty()
            && program
                .expression_table
                .match_arms_all(dispatch.arms, |_, value| {
                    argument_matches_type_reference_using(
                        program,
                        scope,
                        value,
                        type_reference,
                        correspondence,
                        required_substitutions,
                    )
                });
    }
    if let Some(inner_expression) = program.expression_table.borrow_expression(argument) {
        let TypeReferenceNode::Reference {
            referee, access, ..
        } = program.type_reference_table.type_reference(type_reference)
        else {
            return false;
        };
        if inner_expression.access != *access {
            return false;
        }
        // Reborrowing an existing reference lends its referent, not its
        // descriptor as though that were an owned value of the referee type.
        // Keep the same declared/carrier correspondence the consumer selected;
        // this type relation supplies neither access authority nor a loan.
        let mut substitutions = Vec::new();
        if let Some(actual) = reference_values::declared_value_type(
            program,
            inner_expression.target,
            &mut substitutions,
        )
        .and_then(|actual| reference_values::substituted_reference(program, actual, &substitutions))
            && matches!(
                program.type_reference_table.type_reference(actual),
                TypeReferenceNode::Reference { .. }
            )
        {
            // Explicit attenuation changes access, not the selected referent.
            // Bare forwarding still uses reference correspondence below and
            // cannot silently acquire write-only authority. Formation and loan
            // checking independently require a writable, non-observing path.
            if *access == language_semantics::ReferenceAccess::WriteOnly
                && let TypeReferenceNode::Reference {
                    referee: actual_referee,
                    access: language_semantics::ReferenceAccess::Mutable,
                    ..
                } = program.type_reference_table.type_reference(actual)
            {
                return program.type_identity(TypeIdentityRequest {
                    substitutions: &substitutions,
                    ..TypeIdentityRequest::ordinary(*actual_referee)
                }) == program.type_identity_with_simultaneous_substitutions(
                    TypeIdentityRequest {
                        substitutions: required_substitutions,
                        ..TypeIdentityRequest::ordinary(*referee)
                    },
                );
            }
            return correspondence(
                program,
                actual,
                type_reference,
                &substitutions,
                required_substitutions,
            );
        }
        return argument_matches_type_reference_using(
            program,
            scope,
            inner_expression.target,
            *referee,
            correspondence,
            required_substitutions,
        );
    }

    // A recast states its viewed carrier and access explicitly, whether or not
    // the source includes an outer Borrow. Representation and loan validation
    // independently judge whether the source can supply this view. Comparing
    // the stated type here must neither infer a conversion from the destination
    // nor erase qualifications. Required bindings are simultaneous: a selected
    // caller type must not be rebound by another callee formal.
    if let Some(cast) = program.expression_table.cast_node(argument)
        && cast.form.is_recast()
    {
        let required_carrier = if let TypeReferenceNode::Reference {
            referee, access, ..
        } = program.type_reference_table.type_reference(type_reference)
        {
            let Some(formation) = crate::reference_formation_source(program, argument) else {
                return false;
            };
            if formation.access != *access
                && !(formation.access == language_semantics::ReferenceAccess::Mutable
                    && *access == language_semantics::ReferenceAccess::Shared)
            {
                return false;
            }
            *referee
        } else {
            type_reference
        };
        return program.type_identity(TypeIdentityRequest::ordinary(cast.target_type))
            == program.type_identity_with_simultaneous_substitutions(TypeIdentityRequest {
                substitutions: required_substitutions,
                ..TypeIdentityRequest::ordinary(required_carrier)
            });
    }

    // A resolved reference result has a declared referee carrier. Do not let the
    // permissive scalar-call or implicit-shared syntax fallbacks erase it.
    if let Some(actual) = crate::machine_calls::calls::call_result_type(program, argument)
        && matches!(
            program.type_reference_table.type_reference(actual),
            TypeReferenceNode::Reference { .. }
        )
    {
        return correspondence(program, actual, type_reference, &[], required_substitutions);
    }

    // An element projection selects a stored value; a range constructs a view
    // and retains the existing slice-matching path below.
    let selects_stored_value = match program.expression_table.expression(argument) {
        ExpressionNode::Member(_) => true,
        ExpressionNode::Indexed(indexed) => !matches!(
            program.expression_table.expression(indexed.index),
            ExpressionNode::Range(_)
        ),
        _ => false,
    };
    if selects_stored_value
        && matches!(
            program.type_reference_table.type_reference(type_reference),
            TypeReferenceNode::Reference { .. }
        )
    {
        return reference_values::projected_matches_reference(
            program,
            argument,
            type_reference,
            correspondence,
            required_substitutions,
        );
    }

    // A reference already stored in a named parameter/local is a value of its
    // declared reference type. Forwarding that value does not form a new loan,
    // so it has no `Borrow` syntax node to inspect. Use the same exact reference
    // and array-view correspondence as projected and call-produced references.
    // Falling through to permissive name matching could change element types;
    // requiring whole reference identity would reject ordinary array views.
    if matches!(
        program.type_reference_table.type_reference(type_reference),
        TypeReferenceNode::Reference { .. }
    ) && let ExpressionNode::Name(path) = program.expression_table.expression(argument)
        && let Some(actual) = named_value_type_reference(program, path)
        && matches!(
            program.type_reference_table.type_reference(actual),
            TypeReferenceNode::Reference { .. }
        )
    {
        return correspondence(program, actual, type_reference, &[], required_substitutions);
    }

    // A value delivered with a complete type keeps it. A type parameter of
    // the machine making the call is rigid there and equal only to itself,
    // and applied data arguments are part of the type; the formal has
    // already taken the call's simultaneous application.
    if let Some(actual) = delivered_types::delivered_value_type(program, argument)
        && delivered_types::delivered_carrier_conflict(
            program,
            scope,
            actual.reference,
            &actual.substitutions,
            type_reference,
            required_substitutions,
        )
    {
        return false;
    }

    let argument_node = program.expression_table.expression(argument);

    // A subslice `collection[a..b]` produces a fresh view over the
    // collection's own backing, so its reference correspondence is the
    // collection's resolved reference type rather than an implicit shared
    // borrow: only a mutable source answers a mutable result.
    if let ExpressionNode::Indexed(indexed) = argument_node
        && matches!(
            program.expression_table.expression(indexed.index),
            ExpressionNode::Range(_)
        )
        && reference_values::subslice_view_matches_reference(
            program,
            indexed,
            type_reference,
            correspondence,
            required_substitutions,
        )
    {
        return true;
    }

    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference {
            referee, access, ..
        } => {
            // Shared lending observes a completed value, including an anonymous
            // constructor. Its carrier must correspond; loan formation and
            // temporary lifetime belong to the independent custody checks, not
            // a syntax whitelist that excludes valid construction operands.
            *access == language_semantics::ReferenceAccess::Shared
                && argument_matches_type_reference_using(
                    program,
                    scope,
                    argument,
                    *referee,
                    correspondence,
                    required_substitutions,
                )
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            // A literal may directly establish an owned bounded text carrier
            // (`[u8; N] in Utf8`) in argument and terminal-result positions,
            // just as it already can in field/local writes. Do this before
            // erasing the value-domain constraint: the unconstrained base is
            // an always-full fixed array, which correctly does NOT accept a
            // text literal by itself.
            (matches!(argument_node, ExpressionNode::String(literal)
                if bounded_byte_buffer_capacity(program, type_reference)
                    .is_some_and(|capacity| literal.len() <= capacity))
                || argument_matches_type_reference_using(
                    program,
                    scope,
                    argument,
                    *base_type,
                    correspondence,
                    required_substitutions,
                ))
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => {
            // Array syntax is not type correspondence. The same walk already
            // follows selected arms and explicit borrows; recurse into literal
            // children here, and compare the complete declared type of an
            // already produced/stored value, including nested extents.
            if let ExpressionNode::String(bytes) = argument_node {
                // The empty byte sequence exactly forms a zero-length byte
                // array; nonempty text still requires its bounded text carrier.
                return bytes.is_empty()
                    && *length == typed_trees::types::FixedArrayLength::Literal(0)
                    && program.primitive_type_reference(*element_type) == Some(PrimitiveType::U8);
            }
            if let ExpressionNode::ArrayLiteral(elements) = argument_node {
                let elements = program.expression_table.expression_handles(*elements);
                return !matches!(length, typed_trees::types::FixedArrayLength::Literal(length)
                    if *length != elements.len())
                    && elements.iter().all(|element| {
                        argument_matches_type_reference_using(
                            program,
                            scope,
                            *element,
                            *element_type,
                            correspondence,
                            required_substitutions,
                        )
                    });
            }
            let mut substitutions = Vec::new();
            let Some(actual) =
                reference_values::declared_value_type(program, argument, &mut substitutions)
            else {
                return false;
            };
            // The enclosing constraint arm delegates only carrier matching;
            // qualification admission still consumes the actual value's facts.
            // Strip the corresponding actual root shell, never child evidence.
            let actual = program
                .type_reference_table
                .unconstrained_type_reference(actual);
            program.type_identity(TypeIdentityRequest {
                substitutions: &substitutions,
                ..TypeIdentityRequest::ordinary(actual)
            }) == program.type_identity_with_simultaneous_substitutions(TypeIdentityRequest {
                substitutions: required_substitutions,
                ..TypeIdentityRequest::ordinary(type_reference)
            }) || reference_values::fixed_array_binds_open_length(
                program,
                actual,
                type_reference,
                &substitutions,
                required_substitutions,
            )
        }
        TypeReferenceNode::Slice { element_type } => {
            // A string literal is a byte sequence, so it satisfies a `[u8]` slice
            // target (`&[u8] in Utf8 = "..."`) -- the basis for migrating string
            // literals to the `[u8] in Utf8` view. Other element types keep the
            // reference/place forms only.
            let element_is_u8 = matches!(
                program.type_reference_table.primitive_type(*element_type),
                Some(PrimitiveType::U8)
            );
            matches!(
                argument_node,
                ExpressionNode::Call(_)
                    | ExpressionNode::Indexed(_)
                    | ExpressionNode::Member(_)
                    | ExpressionNode::Name(_)
            ) || (element_is_u8 && matches!(argument_node, ExpressionNode::String(_)))
        }
        TypeReferenceNode::Generic { .. } => matches!(
            argument_node,
            ExpressionNode::Binary(_)
                | ExpressionNode::Call(_)
                | ExpressionNode::Cast(_)
                | ExpressionNode::Indexed(_)
                | ExpressionNode::Integer(_)
                | ExpressionNode::Member(_)
                | ExpressionNode::Name(_)
                | ExpressionNode::StructLiteral(_)
                | ExpressionNode::Unary(_)
        ),
        TypeReferenceNode::DynamicTrait { .. } => matches!(
            argument_node,
            ExpressionNode::Call(_)
                | ExpressionNode::Cast(_)
                | ExpressionNode::Indexed(_)
                | ExpressionNode::Member(_)
                | ExpressionNode::Name(_)
        ),
        TypeReferenceNode::Named {
            symbol,
            name: type_name,
        } => {
            if let Some(primitive_type) = PrimitiveType::from_name(type_name) {
                if crate::value_custody::literals::land_anonymous_integer_expression(
                    program,
                    argument,
                    primitive_type,
                    &[],
                    |expression| {
                        crate::value_custody::literals::has_anonymous_operator_meaning(
                            program, expression,
                        )
                    },
                )
                .is_some()
                {
                    return true;
                }
                if let ExpressionNode::UndeterminedNaN(format) = argument_node {
                    return primitive_type
                        == match format {
                            numerics::literals::FloatFormat::F32 => PrimitiveType::F32,
                            numerics::literals::FloatFormat::F64 => PrimitiveType::F64,
                        };
                }
                return matches!(argument_node, ExpressionNode::Boolean(_))
                    && primitive_type == PrimitiveType::Bool
                    || matches!(argument_node, ExpressionNode::Float(_))
                        && primitive_type.accepts_float_literal()
                    || matches!(argument_node, ExpressionNode::Integer(_))
                        && primitive_type.accepts_integer_literal()
                    || matches!(argument_node, ExpressionNode::Unary(unary)
                    if match unary.operator {
                        typed_trees::expression::UnaryOperator::BitwiseNot => {
                            primitive_type.accepts_integer_literal()
                        }
                        typed_trees::expression::UnaryOperator::LogicalNot => {
                            primitive_type == PrimitiveType::Bool
                        }
                    })
                    || matches!(
                        argument_node,
                        ExpressionNode::Binary(_)
                            | ExpressionNode::Call(_)
                            | ExpressionNode::Cast(_)
                            | ExpressionNode::Indexed(_)
                            | ExpressionNode::Member(_)
                            | ExpressionNode::Name(_)
                            | ExpressionNode::StructLiteral(_)
                    );
            }

            // A closed destination may name a synthesized instance while its
            // constructor retains the authored owner. Join through that exact
            // retained application, never spelling or layout coincidence.
            if symbol.is_valid() && program.symbols.get(*symbol).kind == symbols::SymbolKind::Data {
                match argument_node {
                    ExpressionNode::StructLiteral(literal) => {
                        return program
                            .closed_data_constructor_definition(type_reference, literal.type_symbol)
                            .is_some();
                    }
                    ExpressionNode::Name(path)
                        if program.symbols.get(path.symbol).kind
                            == symbols::SymbolKind::Variant =>
                    {
                        return program.symbols.get(path.symbol).parent == *symbol;
                    }
                    _ => {}
                }
            }

            // A named type parameter is an open slot, not a nominal data
            // declaration. Exact generic-call/operator application checking
            // separately closes that slot from the operand tuple and rejects
            // disagreement. A width-landed integer is therefore a valid
            // inference source even while this declaration still spells `T`.
            if symbol.is_valid()
                && program.symbols.get(*symbol).kind == symbols::SymbolKind::TypeParameter
                && matches!(argument_node, ExpressionNode::Integer(_))
            {
                return true;
            }

            matches!(
                argument_node,
                ExpressionNode::Binary(_)
                    | ExpressionNode::Call(_)
                    | ExpressionNode::Cast(_)
                    | ExpressionNode::Indexed(_)
                    | ExpressionNode::Member(_)
                    | ExpressionNode::Name(_)
                    | ExpressionNode::StructLiteral(_)
                    | ExpressionNode::Unary(_)
            )
        }
        TypeReferenceNode::ConstExpression(_) | TypeReferenceNode::Unit => false,
    }
}

pub(crate) fn named_value_type_reference(
    program: &dyn crate::CheckingInput,
    path: &typed_trees::expression::TableNamePath,
) -> Option<TypeReferenceHandle> {
    if program.symbols.builtin_type_member(path.symbol)
        == Some(symbols::BuiltinTypeMember::AddressBound)
    {
        return crate::proof_contracts::proof_embeddings::proof_int_type_reference(program);
    }
    let [_] = program.expression_table.name_path_members(path.members) else {
        return None;
    };
    if let Some(reference) =
        crate::proof_contracts::mathematical_applications::parameter_type_reference(
            program,
            path.symbol,
        )
    {
        return Some(reference);
    }
    // Static value binders live in the shared declaration arena, including
    // domain telescopes with no executable machine/state. Their exact symbol
    // supplies the declared carrier; a same-spelled binder supplies nothing.
    if path.symbol.is_valid()
        && path.head_symbol == path.symbol
        && let Some((_, parameter)) = program
            .data_type_parameters
            .iter()
            .find(|(_, parameter)| parameter.symbol == path.symbol)
        && let typed_trees::data::TypeParameterKind::Const { type_reference }
        | typed_trees::data::TypeParameterKind::Value { type_reference } = parameter.kind
    {
        return Some(type_reference);
    }
    let matches_symbol = |candidate: symbols::SymbolHandle| {
        candidate.is_valid()
            && ((path.symbol.is_valid() && candidate == path.symbol)
                || (path.head_symbol.is_valid() && candidate == path.head_symbol))
    };

    // The resolved binder's retained symbol parent names its owning
    // declaration: machine-owned data under the machine, parameters and
    // locals under the state, parameters under the proposition. Check that
    // one container before the whole-program scan; a miss still runs the
    // full scan below, so binders whose retained parent disagrees with
    // storage keep resolving exactly as before.
    if let Some(type_reference) = hinted_named_value_type_reference(program, path, matches_symbol) {
        return Some(type_reference);
    }

    // Bodyless operators and trait signatures have formals but no executable
    // state to visit below. Their formals share the parameter arena with state
    // formals, so the exact retained symbol supplies the same declared type.
    // This matters for anonymous contract endpoints: `input: f32` compared
    // with `128.0` must land that literal as f32, not guess a default format.
    if path.symbol.is_valid()
        && path.head_symbol == path.symbol
        && let Some((_, parameter)) = program
            .state_parameters
            .iter()
            .find(|(_, parameter)| parameter.symbol == path.symbol)
    {
        return Some(parameter.type_reference);
    }

    for machine in program.machines() {
        if let Some(owned) = program
            .machine_owned_data(machine)
            .iter()
            .find(|owned| matches_symbol(owned.symbol))
        {
            return Some(owned.type_reference);
        }
        for state in program.machine_states(machine) {
            if let Some(type_reference) = state_symbol_declared_type(program, state, matches_symbol)
            {
                return Some(type_reference);
            }
        }
    }
    for proposition in program.propositions() {
        if let Some(parameter) = program
            .proposition_parameters(proposition)
            .iter()
            .find(|parameter| matches_symbol(parameter.symbol))
        {
            return Some(parameter.type_reference);
        }
    }
    None
}

/// Parent-hinted lookup for [`named_value_type_reference`]. The retained
/// parent resolves the owning declaration in one pass over the machine and
/// proposition rosters instead of descending every state's parameters and
/// statements. Each hinted container is verified with the caller's exact
/// symbol predicate; `None` leaves the whole-program scan authoritative.
fn hinted_named_value_type_reference(
    program: &dyn crate::CheckingInput,
    path: &typed_trees::expression::TableNamePath,
    matches_symbol: impl Fn(symbols::SymbolHandle) -> bool,
) -> Option<TypeReferenceHandle> {
    let binder = if path.symbol.is_valid() {
        path.symbol
    } else {
        path.head_symbol
    };
    if !binder.is_valid() {
        return None;
    }
    let parent = program.symbols.get(binder).parent;
    if !parent.is_valid() {
        return None;
    }
    match program.symbols.get(parent).kind {
        // A declaration predicate has no machine or state. Its field binder
        // still supplies a real destination type: float endpoints must land
        // in that carrier's format before the predicate is evaluated. Follow
        // the retained owner and field symbols, never a same-spelled field in
        // another declaration or case.
        symbols::SymbolKind::Data => {
            let definition = program.data_definition_by_symbol(parent)?;
            program
                .data_members(definition)
                .iter()
                .find_map(|member| match member {
                    typed_trees::data::DataMember::Field(field) if matches_symbol(field.symbol) => {
                        Some(field.type_reference)
                    }
                    _ => None,
                })
        }
        symbols::SymbolKind::Variant => {
            let owner = program.symbols.get(parent).parent;
            let definition = program.data_definition_by_symbol(owner)?;
            let variant =
                program
                    .data_members(definition)
                    .iter()
                    .find_map(|member| match member {
                        typed_trees::data::DataMember::Variant(variant)
                            if variant.symbol == parent =>
                        {
                            Some(variant)
                        }
                        _ => None,
                    })?;
            program
                .data_payload_fields(variant)
                .iter()
                .find(|field| matches_symbol(field.symbol))
                .map(|field| field.type_reference)
        }
        // Machine-owned data: the binder's parent is the machine itself.
        symbols::SymbolKind::Machine => {
            if let Some(machine) = program.machine(parent) {
                return program
                    .machine_owned_data(machine)
                    .iter()
                    .find(|owned| matches_symbol(owned.symbol))
                    .map(|owned| owned.type_reference);
            }
            None
        }
        // Proposition parameters: the binder's parent is the proposition.
        symbols::SymbolKind::Proposition => program
            .proposition_definition_by_symbol(parent)
            .and_then(|proposition| {
                program
                    .proposition_parameters(proposition)
                    .iter()
                    .find(|parameter| matches_symbol(parameter.symbol))
                    .map(|parameter| parameter.type_reference)
            }),
        // Measures keep their formal inline rather than in the state
        // parameter arena. Its retained declaration and symbol still supply
        // the same declared carrier to selected computation queries.
        symbols::SymbolKind::Measure => program
            .measures()
            .iter()
            .find(|measure| measure.symbol == parent)
            .and_then(|measure| measure.parameter.as_ref())
            .filter(|parameter| {
                path.symbol.is_valid()
                    && path.head_symbol == path.symbol
                    && parameter.symbol == path.symbol
            })
            .map(|parameter| parameter.type_reference),
        // State binders: the binder's parent is a state whose own retained
        // parent names its machine.
        symbols::SymbolKind::State => {
            let grandparent = program.symbols.get(parent).parent;
            if program.symbols.get(grandparent).kind != symbols::SymbolKind::Machine {
                return None;
            }
            let (_, state) = program.machine_state(grandparent, parent)?;
            state_symbol_declared_type(program, state, matches_symbol)
        }
        _ => None,
    }
}

/// The declared type a state's parameter or `data` local carries for a
/// matching symbol, or `None`.
fn state_symbol_declared_type(
    program: &dyn crate::CheckingInput,
    state: &typed_trees::state::State,
    matches_symbol: impl Fn(SymbolHandle) -> bool,
) -> Option<TypeReferenceHandle> {
    if let Some(parameter) = program
        .state_parameters(state)
        .iter()
        .find(|parameter| matches_symbol(parameter.symbol))
    {
        return Some(parameter.type_reference);
    }
    for statement in program.statement_table.statements(state.statement_nodes) {
        if let Some(local) = statement.as_local_data()
            && matches_symbol(local.symbol)
        {
            return Some(local.type_reference);
        }
    }
    None
}

/// Mirror the backend layout classifier for an owned variable-fill text
/// carrier. A named value domain changes `[u8; N]` from an always-full fixed
/// array into `{len, bytes[N]}`; layout-policy domains do not.
/// The returned capacity is not evidence of the value's live length.
pub fn bounded_byte_buffer_capacity(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> Option<usize> {
    if let TypeReferenceNode::Reference { referee, .. } =
        program.type_reference_table.type_reference(type_reference)
    {
        return bounded_byte_buffer_capacity(program, *referee);
    }
    let TypeReferenceNode::Constrained {
        base_type,
        constraints,
    } = program.type_reference_table.type_reference(type_reference)
    else {
        return None;
    };
    if !has_value_domain(program, *constraints) {
        return None;
    }
    fixed_byte_array_capacity(program, *base_type)
}

fn has_value_domain(
    program: &dyn crate::CheckingInput,
    constraints: arena::HandleSpan<TypeConstraintNode>,
) -> bool {
    program
        .type_reference_table
        .constraints(constraints)
        .iter()
        .any(|constraint| match constraint {
            TypeConstraintNode::Domain(name) => {
                !typed_trees::wire::is_layout_domain_constraint(name)
                    && language_semantics::CarryPermission::from_name(name.as_str()).is_none()
            }
            _ => false,
        })
}

fn fixed_byte_array_capacity(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> Option<usize> {
    let TypeReferenceNode::FixedArray {
        element_type,
        length,
    } = program.type_reference_table.type_reference(type_reference)
    else {
        return None;
    };
    if program.type_reference_table.primitive_type(*element_type) != Some(PrimitiveType::U8) {
        return None;
    }
    match length {
        typed_trees::types::FixedArrayLength::Literal(capacity) => Some(*capacity),
        typed_trees::types::FixedArrayLength::ConstParameter { .. }
        | typed_trees::types::FixedArrayLength::Expression(_)
        | typed_trees::types::FixedArrayLength::ConstCall { .. } => None,
    }
}

pub(crate) fn validate_expression_type_handle(
    program: &dyn crate::CheckingInput,
    scope: SymbolHandle,
    expression: ExpressionHandle,
    type_reference: TypeReferenceHandle,
    diagnostics: &mut Vec<Diagnostic>,
    owner: ExpressionTypeOwner<'_>,
) {
    if let ExpressionNode::String(literal) = program.expression_table.expression(expression)
        && let Some(capacity) = bounded_byte_buffer_capacity(program, type_reference)
        && literal.len() > capacity
    {
        diagnostics.push(Diagnostic::error(format!(
            "{owner} constructs {} byte(s), exceeding the {capacity}-byte capacity of `{}`",
            literal.len(),
            program.display_type_reference_with_constraints(type_reference),
        )));
        return;
    }
    if !argument_matches_type_reference_using(
        program,
        scope,
        expression,
        type_reference,
        reference_values::declared_value_matches_reference,
        &[],
    ) {
        diagnostics.push(Diagnostic::error(format!(
            "{owner} expects `{}`, got `{}`",
            program.display_type_reference_with_constraints(type_reference),
            expression_type_name_handle(program, expression)
        )));
    }
}

pub(crate) fn expression_type_name_handle(
    program: &dyn crate::CheckingInput,
    argument: ExpressionHandle,
) -> &'static str {
    match program.expression_table.expression(argument) {
        ExpressionNode::Match(_) => "match expression",
        ExpressionNode::Atomic(atomic) => expression_type_name_handle(program, atomic.value),
        ExpressionNode::ArrayLiteral(_) => "array literal",
        ExpressionNode::Binary(_) => "binary expression",
        ExpressionNode::Boolean(_) => "bool",
        ExpressionNode::Call(_) => "call expression",
        ExpressionNode::Cast(_) => "cast expression",
        ExpressionNode::Float(_) => "float literal",
        ExpressionNode::UndeterminedNaN(format) => format.name(),
        ExpressionNode::Indexed(_) => "indexed value",
        ExpressionNode::Integer(_) => "integer literal",
        ExpressionNode::Member(_) => "member access",
        ExpressionNode::Borrow(inner_expression) => {
            expression_type_name_handle(program, inner_expression.target)
        }
        ExpressionNode::Name(_) => "named value",
        ExpressionNode::Range(_) => "range expression",
        ExpressionNode::StructLiteral(_) => "struct literal",
        ExpressionNode::String(_) => "String",
        ExpressionNode::Unary(unary) => match unary.operator {
            typed_trees::expression::UnaryOperator::BitwiseNot => "integer",
            typed_trees::expression::UnaryOperator::LogicalNot => "bool",
        },
        ExpressionNode::ZeroValue(_) => "zero-value representation observation",
    }
}
