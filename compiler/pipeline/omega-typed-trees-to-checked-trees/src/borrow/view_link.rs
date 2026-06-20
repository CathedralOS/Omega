//! Lifetimes stage 2: resolving which input a returned view borrows.
//!
//! A machine that returns a view (`-> &T`, `-> &mut T`, `-> &string`, a slice
//! view, …) must have an unambiguous source for the returned borrow. Two
//! checkers consume this decision and MUST agree, or the borrow checker would
//! accept a program whose loan it never tracks (unsound):
//!
//! - the declaration check (`checks::borrows::elision`) rejects ambiguous or
//!   ill-formed signatures up front;
//! - the loan attributor (`borrow::loans`) links the returned view's loan to
//!   the borrowed argument at each call site.
//!
//! The rules (Rust's, frozen decision 15):
//!
//! - a `&self`/`&mut self` parameter links the output to self (elision rule 3),
//!   regardless of other ref inputs;
//! - an EXPLICIT output lifetime (`-> &'buf T`) links the output to the input
//!   carrying the same lifetime name (stage 2); it must match exactly one input;
//! - otherwise (an ELIDED output) exactly one ref input is the source (elision
//!   rule 1); zero ref inputs leaves the output unlinked (historical behavior);
//!   two or more are ambiguous and must be disambiguated with a lifetime.

use omega_typed_trees::TypedTrees;
use omega_typed_trees::state::State;
use omega_typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// Where a returned view's borrow comes from.
#[derive(Debug, Clone)]
pub(crate) enum ViewReturnSource {
    /// The return type is not a view, or there is nothing to link (an elided
    /// output with zero ref inputs). No loan, no diagnostic.
    NotApplicable,
    /// The output borrows the `&self` receiver (elision rule 3).
    SelfReceiver,
    /// The output borrows exactly one non-self ref parameter. `non_self_index`
    /// is the ordinal among NON-SELF parameters, which equals the positional
    /// argument index at a call site.
    Parameter { non_self_index: usize },
    /// The signature is ambiguous or ill-formed; the declaration check turns
    /// this into a diagnostic and the loan attributor tracks no loan.
    Ambiguous(ViewReturnAmbiguity),
}

/// Why a view-returning signature could not be resolved to a single input.
#[derive(Debug, Clone)]
pub(crate) enum ViewReturnAmbiguity {
    /// Elided output with two or more candidate ref inputs (their names).
    ElidedMultipleInputs { candidates: Vec<String> },
    /// An explicit output lifetime that matches no input.
    LifetimeMatchesNoInput { lifetime: String },
    /// An explicit output lifetime shared by two or more inputs (their names);
    /// a single returned view borrowing several inputs is not modelled yet.
    LifetimeMatchesMultipleInputs {
        lifetime: String,
        candidates: Vec<String>,
    },
}

/// Resolve the source of a state's returned view from its signature alone.
pub(crate) fn resolve_view_return_source(program: &TypedTrees, state: &State) -> ViewReturnSource {
    if !returns_borrow(program, state.return_type) {
        return ViewReturnSource::NotApplicable;
    }

    let parameters = program.state_parameters(state);
    if parameters.iter().any(|parameter| parameter.is_self) {
        // Elision rule 3: the returned view borrows self.
        return ViewReturnSource::SelfReceiver;
    }

    // Non-self ref parameters paired with their argument ordinal and lifetime.
    let mut ref_parameters: Vec<(usize, &str, Option<&str>)> = Vec::new();
    let mut non_self_index = 0usize;
    for parameter in parameters {
        if parameter.is_self {
            continue;
        }
        let index = non_self_index;
        non_self_index = non_self_index.saturating_add(1);
        if is_reference_type(program, parameter.type_reference) {
            ref_parameters.push((
                index,
                parameter.name.as_str(),
                reference_lifetime(program, parameter.type_reference),
            ));
        }
    }

    match reference_lifetime(program, state.return_type) {
        Some(output_lifetime) => {
            let matching: Vec<&(usize, &str, Option<&str>)> = ref_parameters
                .iter()
                .filter(|(_, _, lifetime)| *lifetime == Some(output_lifetime))
                .collect();
            match matching.as_slice() {
                [] => ViewReturnSource::Ambiguous(ViewReturnAmbiguity::LifetimeMatchesNoInput {
                    lifetime: output_lifetime.to_owned(),
                }),
                [single] => ViewReturnSource::Parameter {
                    non_self_index: single.0,
                },
                _ => ViewReturnSource::Ambiguous(
                    ViewReturnAmbiguity::LifetimeMatchesMultipleInputs {
                        lifetime: output_lifetime.to_owned(),
                        candidates: matching
                            .iter()
                            .map(|(_, name, _)| (*name).to_owned())
                            .collect(),
                    },
                ),
            }
        }
        None => match ref_parameters.as_slice() {
            [] => ViewReturnSource::NotApplicable,
            [single] => ViewReturnSource::Parameter {
                non_self_index: single.0,
            },
            _ => ViewReturnSource::Ambiguous(ViewReturnAmbiguity::ElidedMultipleInputs {
                candidates: ref_parameters
                    .iter()
                    .map(|(_, name, _)| (*name).to_owned())
                    .collect(),
            }),
        },
    }
}

/// The explicit lifetime name on a reference type, seeing through a
/// `Constrained` wrapper (e.g. `&'buf string[a..=b]`). `None` for an elided
/// reference or a non-reference type.
fn reference_lifetime(program: &TypedTrees, type_reference: TypeReferenceHandle) -> Option<&str> {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { lifetime, .. } => {
            lifetime.as_ref().map(|name| name.as_str())
        }
        TypeReferenceNode::Constrained { base_type, .. } => reference_lifetime(program, *base_type),
        _ => None,
    }
}

/// Whether a type reference is a view (a reference, possibly under a
/// `Constrained` wrapper).
pub(crate) fn is_reference_type(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> bool {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { .. } => true,
        TypeReferenceNode::Constrained { base_type, .. } => {
            is_reference_type(program, *base_type)
        }
        _ => false,
    }
}

/// Whether a return TYPE carries a borrow that must be linked to an input: a
/// bare reference, OR a borrow-carrying `data` value (a `data` type with a
/// reference-typed field — `data Msg<'buf> { body: &'buf string }`). Both make a
/// machine's result outlive-bounded by one of its inputs (decision 15 stage 2).
pub(crate) fn returns_borrow(program: &TypedTrees, type_reference: TypeReferenceHandle) -> bool {
    is_reference_type(program, type_reference)
        || is_borrow_carrying_data(program, type_reference)
}

/// Whether a type reference names a `data` definition that has at least one
/// reference-typed field (directly). Nested borrow-carrying fields are NOT yet
/// counted — a first-cut limitation; no corpus relies on it.
pub(crate) fn is_borrow_carrying_data(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> bool {
    let symbol = program.type_reference_symbol(type_reference);
    if !symbol.is_valid() {
        return false;
    }
    let Some(definition) = program
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == symbol)
    else {
        return false;
    };
    program
        .data_members(definition)
        .iter()
        .any(|member| data_member_has_reference_field(program, member))
}

fn data_member_has_reference_field(
    program: &TypedTrees,
    member: &omega_typed_trees::data::DataMember,
) -> bool {
    match member {
        omega_typed_trees::data::DataMember::Field(field) => {
            is_reference_type(program, field.type_reference)
        }
        omega_typed_trees::data::DataMember::Variant(variant) => program
            .data_payload_fields(variant)
            .iter()
            .any(|field| is_reference_type(program, field.type_reference)),
    }
}
