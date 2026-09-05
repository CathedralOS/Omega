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
//! - an EXPLICIT output lifetime (`-> &'buf T` or `-> Message<'buf>`) links the
//!   output to the input carrying the same lifetime name (stage 2); it must
//!   match exactly one input;
//! - otherwise (an ELIDED output) exactly one ref input is the source (elision
//!   rule 1); zero ref inputs leaves the output unlinked (historical behavior);
//!   two or more are ambiguous and must be disambiguated with a lifetime.
//!
//! For aggregate results, input references include structurally carried leaves.
//! Explicit lifetimes select one input parameter and every leaf carrying that
//! lifetime within it; each must supply the result's access. Elision requires
//! one contained source, not one parameter containing several unnamed sources.

use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::signature::StateParameter;
use psi_typed_trees::state::State;
use psi_typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

use super::tracker::BorrowOwnerSegment;

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
    /// A borrow-carrying aggregate maps each carried field to the one input
    /// named by that field's instantiated lifetime.
    Fields { fields: Vec<ViewReturnFieldSource> },
    /// The signature is ambiguous or ill-formed; the declaration check turns
    /// this into a diagnostic and the loan attributor tracks no loan.
    Ambiguous(ViewReturnAmbiguity),
}

#[derive(Debug, Clone)]
pub(crate) struct ViewReturnFieldSource {
    pub(crate) owner_path: Vec<BorrowOwnerSegment>,
    pub(crate) source_path: Vec<BorrowOwnerSegment>,
    pub(crate) source_type: TypeReferenceHandle,
    pub(crate) non_self_index: usize,
    pub(crate) kind: psi_checked_trees::BorrowAccessKind,
}

/// Why a view-returning signature could not be resolved to a single input.
#[derive(Debug, Clone)]
pub(crate) enum ViewReturnAmbiguity {
    /// A declaration cannot supply a complete structural lifetime frontier.
    IncompleteStructure { subject: String },
    /// Source access cannot supply the result's access without escalation.
    IncompatibleSourceAccess { input: String },
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
    resolve_signature_view_return_source(
        program,
        program.state_parameters(state),
        state.return_type,
    )
}

/// Resolve the same lifetime/source relation for a bodyless callable
/// signature. Boundary-trait requirements and compile-time machine parameters
/// have no `State`, but their returned views create the same caller-side loans.
pub(crate) fn resolve_signature_view_return_source(
    program: &TypedTrees,
    parameters: &[StateParameter],
    return_type: TypeReferenceHandle,
) -> ViewReturnSource {
    if !returns_borrow(program, return_type) {
        return ViewReturnSource::NotApplicable;
    }

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

    if !is_reference_type(program, return_type) {
        return aggregate_view_return_source(program, parameters, return_type);
    }

    match reference_lifetime(program, return_type) {
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

mod carried_lifetimes;

use carried_lifetimes::carried_lifetimes;
pub(crate) use carried_lifetimes::{DeclarationLifetimeFrontier, declaration_lifetime_frontier};

fn aggregate_view_return_source(
    program: &TypedTrees,
    parameters: &[StateParameter],
    return_type: TypeReferenceHandle,
) -> ViewReturnSource {
    let Some(outputs) = carried_lifetimes(program, return_type) else {
        if let Some(source) = whole_elided_result_source(program, parameters, return_type) {
            return source;
        }
        return ViewReturnSource::Ambiguous(ViewReturnAmbiguity::IncompleteStructure {
            subject: "result".to_owned(),
        });
    };
    if outputs.is_empty() {
        return ViewReturnSource::NotApplicable;
    }
    let mut inputs = Vec::new();
    for (index, parameter) in parameters
        .iter()
        .filter(|parameter| !parameter.is_self)
        .enumerate()
    {
        if !returns_borrow(program, parameter.type_reference) {
            continue;
        }
        let Some(leaves) = carried_lifetimes(program, parameter.type_reference) else {
            return ViewReturnSource::Ambiguous(ViewReturnAmbiguity::IncompleteStructure {
                subject: format!("input `{}`", parameter.name),
            });
        };
        for leaf in leaves {
            inputs.push((index, parameter, leaf));
        }
    }
    // Retain historical source-free output handling. A carrier input containing
    // one reference is now the same single-source elision case as a bare ref.
    if inputs.is_empty() && outputs.iter().all(|output| output.lifetime.is_none()) {
        return ViewReturnSource::NotApplicable;
    }
    let mut fields = Vec::new();
    for output in outputs {
        let matching = inputs
            .iter()
            .filter(|(_, _, input)| {
                output
                    .lifetime
                    .as_ref()
                    .is_none_or(|lifetime| input.lifetime.as_ref() == Some(lifetime))
            })
            .collect::<Vec<_>>();
        let Some(first) = matching.first() else {
            return ViewReturnSource::Ambiguous(ViewReturnAmbiguity::LifetimeMatchesNoInput {
                lifetime: output.lifetime.unwrap_or_else(|| "elided".to_owned()),
            });
        };
        if output.lifetime.is_none() && matching.len() != 1 {
            return ViewReturnSource::Ambiguous(ViewReturnAmbiguity::ElidedMultipleInputs {
                candidates: matching
                    .iter()
                    .map(|(_, parameter, input)| {
                        format!("{} carried source {:?}", parameter.name, input.owner_path)
                    })
                    .collect(),
            });
        }
        if matching.iter().any(|(index, _, _)| *index != first.0) {
            let mut candidates = Vec::new();
            for (_, parameter, _) in &matching {
                let name = parameter.name.as_str().to_owned();
                if !candidates.contains(&name) {
                    candidates.push(name);
                }
            }
            return ViewReturnSource::Ambiguous(
                ViewReturnAmbiguity::LifetimeMatchesMultipleInputs {
                    lifetime: output.lifetime.unwrap_or_else(|| "elided".to_owned()),
                    candidates,
                },
            );
        }
        if matching.iter().any(|(_, _, input)| {
            use psi_language_semantics::ReferenceAccess;
            input.access != output.access && input.access != ReferenceAccess::Mutable
        }) {
            return ViewReturnSource::Ambiguous(ViewReturnAmbiguity::IncompatibleSourceAccess {
                input: first.1.name.as_str().to_owned(),
            });
        }
        for (index, parameter, input) in matching {
            fields.push(ViewReturnFieldSource {
                owner_path: output.owner_path.clone(),
                source_path: input.owner_path.clone(),
                source_type: parameter.type_reference,
                non_self_index: *index,
                kind: match output.access {
                    psi_language_semantics::ReferenceAccess::Mutable => {
                        psi_checked_trees::BorrowAccessKind::Mutable
                    }
                    psi_language_semantics::ReferenceAccess::Shared => {
                        psi_checked_trees::BorrowAccessKind::Read
                    }
                    psi_language_semantics::ReferenceAccess::WriteOnly => {
                        psi_checked_trees::BorrowAccessKind::WriteOnly
                    }
                },
            });
        }
    }
    ViewReturnSource::Fields { fields }
}

fn whole_elided_result_source(
    program: &TypedTrees,
    parameters: &[StateParameter],
    return_type: TypeReferenceHandle,
) -> Option<ViewReturnSource> {
    let output_accesses = carried_lifetimes::whole_elided_result_accesses(program, return_type)?;
    // The existing whole-aggregate fallback carries read/mutable polarity;
    // it cannot represent a write-only recursive result without escalation.
    if output_accesses.contains(&psi_language_semantics::ReferenceAccess::WriteOnly) {
        return None;
    }
    let mut candidates = parameters
        .iter()
        .filter(|parameter| !parameter.is_self)
        .enumerate()
        .filter(|(_, parameter)| returns_borrow(program, parameter.type_reference));
    let (index, parameter) = candidates.next()?;
    if candidates.next().is_some() || !is_reference_type(program, parameter.type_reference) {
        return None;
    }
    let inputs = carried_lifetimes(program, parameter.type_reference)?;
    let [input] = inputs.as_slice() else {
        return None;
    };
    if !output_accesses.iter().all(|access| {
        *access == input.access || input.access == psi_language_semantics::ReferenceAccess::Mutable
    }) {
        return Some(ViewReturnSource::Ambiguous(
            ViewReturnAmbiguity::IncompatibleSourceAccess {
                input: parameter.name.as_str().to_owned(),
            },
        ));
    }
    Some(ViewReturnSource::Parameter {
        non_self_index: index,
    })
}

/// Whether a type reference is a view (a reference, possibly under a
/// `Constrained` wrapper).
pub(crate) fn is_reference_type(program: &TypedTrees, type_reference: TypeReferenceHandle) -> bool {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { .. } => true,
        TypeReferenceNode::Constrained { base_type, .. } => is_reference_type(program, *base_type),
        _ => false,
    }
}

/// Whether a return TYPE carries a borrow that must be linked to an input: a
/// bare reference, OR a borrow-carrying `data` value (a `data` type with a
/// reference-typed field — `data Msg<'buf> { body: &'buf string }`). Both make a
/// machine's result outlive-bounded by one of its inputs (decision 15 stage 2).
pub(crate) fn returns_borrow(program: &TypedTrees, type_reference: TypeReferenceHandle) -> bool {
    is_reference_type(program, type_reference) || is_borrow_carrying_data(program, type_reference)
}

/// Every structural projection within `type_reference` that carries a
/// reference. Persistent-storage checking uses this frontier to prove that a
/// place copied as a whole is backed only by static storage even when its
/// borrowed leaves were established by separate field writes.
///
/// `None` means the frontier cannot be enumerated completely (currently an
/// unresolved fixed-array length or a recursive by-value data cycle), so callers
/// must fail closed unless the whole source place already has static provenance.
pub(crate) fn borrow_carrying_owner_paths(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<Vec<Vec<BorrowOwnerSegment>>> {
    let mut output = Vec::new();
    collect_borrow_carrying_owner_paths(
        program,
        type_reference,
        &[],
        &[],
        &mut Vec::new(),
        &mut output,
    )
    .then_some(output)
}

fn collect_borrow_carrying_owner_paths(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    owner_path: &[BorrowOwnerSegment],
    visiting: &mut Vec<SymbolHandle>,
    output: &mut Vec<Vec<BorrowOwnerSegment>>,
) -> bool {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { .. } => {
            output.push(owner_path.to_vec());
            true
        }
        TypeReferenceNode::Constrained { base_type, .. } => collect_borrow_carrying_owner_paths(
            program,
            *base_type,
            substitutions,
            owner_path,
            visiting,
            output,
        ),
        TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => {
            let psi_typed_trees::types::FixedArrayLength::Literal(length) = length else {
                return false;
            };
            for index in 0..*length {
                let mut element_path = owner_path.to_vec();
                element_path.push(BorrowOwnerSegment::FixedIndex(index));
                if !collect_borrow_carrying_owner_paths(
                    program,
                    *element_type,
                    substitutions,
                    &element_path,
                    visiting,
                    output,
                ) {
                    return false;
                }
            }
            true
        }
        TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } => {
            let Some(definition) = data_definition(program, *base_symbol) else {
                return true;
            };
            let arguments = program
                .type_reference_table
                .type_reference_handles(*arguments);
            let mut nested_substitutions = substitutions.to_vec();
            nested_substitutions.extend(
                program
                    .data_type_parameters(definition)
                    .iter()
                    .zip(arguments.iter())
                    .map(|(parameter, argument)| (parameter.symbol, *argument)),
            );
            collect_data_borrow_carrying_owner_paths(
                program,
                definition,
                &nested_substitutions,
                owner_path,
                visiting,
                output,
            )
        }
        TypeReferenceNode::Named { symbol, .. } => {
            if let Some((_, concrete)) = substitutions
                .iter()
                .rev()
                .find(|(parameter, _)| parameter == symbol)
            {
                return collect_borrow_carrying_owner_paths(
                    program,
                    *concrete,
                    substitutions,
                    owner_path,
                    visiting,
                    output,
                );
            }
            let Some(definition) = data_definition(program, *symbol) else {
                return true;
            };
            collect_data_borrow_carrying_owner_paths(
                program,
                definition,
                substitutions,
                owner_path,
                visiting,
                output,
            )
        }
        TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Unit => true,
    }
}

fn collect_data_borrow_carrying_owner_paths(
    program: &TypedTrees,
    definition: &psi_typed_trees::data::DataDefinition,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    owner_path: &[BorrowOwnerSegment],
    visiting: &mut Vec<SymbolHandle>,
    output: &mut Vec<Vec<BorrowOwnerSegment>>,
) -> bool {
    if visiting.contains(&definition.symbol) {
        return false;
    }
    visiting.push(definition.symbol);
    for member in program.data_members(definition) {
        match member {
            psi_typed_trees::data::DataMember::Field(field) => {
                let mut field_path = owner_path.to_vec();
                field_path.push(BorrowOwnerSegment::Field(field.symbol));
                if !collect_borrow_carrying_owner_paths(
                    program,
                    field.type_reference,
                    substitutions,
                    &field_path,
                    visiting,
                    output,
                ) {
                    visiting.pop();
                    return false;
                }
            }
            psi_typed_trees::data::DataMember::Variant(variant) => {
                for field in program.data_payload_fields(variant) {
                    let mut field_path = owner_path.to_vec();
                    field_path.push(BorrowOwnerSegment::Case(variant.symbol));
                    field_path.push(BorrowOwnerSegment::Field(field.symbol));
                    if !collect_borrow_carrying_owner_paths(
                        program,
                        field.type_reference,
                        substitutions,
                        &field_path,
                        visiting,
                        output,
                    ) {
                        visiting.pop();
                        return false;
                    }
                }
            }
        }
    }
    visiting.pop();
    true
}

/// Whether a type reference names a `data` definition that structurally carries
/// at least one reference. The walk follows nested records, live sum payloads,
/// fixed arrays, constraints, and concrete generic arguments. Recursive data
/// definitions terminate through a symbol stack.
pub(crate) fn is_borrow_carrying_data(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> bool {
    type_structurally_carries_borrow(program, type_reference, &[], &mut Vec::new(), false)
}

/// Whether a data value structurally carries at least one mutable borrow.
///
/// A call-produced borrow-carrying aggregate has no literal initializer to
/// inspect field by field, so its returned loan polarity comes from the
/// declared aggregate shape instead.
pub(crate) fn is_mutably_borrow_carrying_data(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> bool {
    type_structurally_carries_borrow(program, type_reference, &[], &mut Vec::new(), true)
}

fn type_structurally_carries_borrow(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    visiting: &mut Vec<SymbolHandle>,
    require_mutable: bool,
) -> bool {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { access, .. } => !require_mutable || access.is_exclusive(),
        TypeReferenceNode::Constrained { base_type, .. } => type_structurally_carries_borrow(
            program,
            *base_type,
            substitutions,
            visiting,
            require_mutable,
        ),
        TypeReferenceNode::FixedArray { element_type, .. } => type_structurally_carries_borrow(
            program,
            *element_type,
            substitutions,
            visiting,
            require_mutable,
        ),
        TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } => {
            let Some(definition) = data_definition(program, *base_symbol) else {
                return false;
            };
            let arguments = program
                .type_reference_table
                .type_reference_handles(*arguments);
            let mut nested_substitutions = substitutions.to_vec();
            nested_substitutions.extend(
                program
                    .data_type_parameters(definition)
                    .iter()
                    .zip(arguments.iter())
                    .map(|(parameter, argument)| (parameter.symbol, *argument)),
            );
            data_definition_carries_borrow(
                program,
                definition,
                &nested_substitutions,
                visiting,
                require_mutable,
            )
        }
        TypeReferenceNode::Named { symbol, .. } => {
            if let Some((_, concrete)) = substitutions
                .iter()
                .rev()
                .find(|(parameter, _)| parameter == symbol)
            {
                return type_structurally_carries_borrow(
                    program,
                    *concrete,
                    substitutions,
                    visiting,
                    require_mutable,
                );
            }
            let Some(definition) = data_definition(program, *symbol) else {
                return false;
            };
            data_definition_carries_borrow(
                program,
                definition,
                substitutions,
                visiting,
                require_mutable,
            )
        }
        TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Unit => false,
    }
}

fn data_definition(
    program: &TypedTrees,
    symbol: SymbolHandle,
) -> Option<&psi_typed_trees::data::DataDefinition> {
    program
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == symbol)
}

fn data_definition_carries_borrow(
    program: &TypedTrees,
    definition: &psi_typed_trees::data::DataDefinition,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    visiting: &mut Vec<SymbolHandle>,
    require_mutable: bool,
) -> bool {
    if visiting.contains(&definition.symbol) {
        return false;
    }
    visiting.push(definition.symbol);
    let carries = program
        .data_members(definition)
        .iter()
        .any(|member| match member {
            psi_typed_trees::data::DataMember::Field(field) => type_structurally_carries_borrow(
                program,
                field.type_reference,
                substitutions,
                visiting,
                require_mutable,
            ),
            psi_typed_trees::data::DataMember::Variant(variant) => {
                program.data_payload_fields(variant).iter().any(|field| {
                    type_structurally_carries_borrow(
                        program,
                        field.type_reference,
                        substitutions,
                        visiting,
                        require_mutable,
                    )
                })
            }
        });
    visiting.pop();
    carries
}
