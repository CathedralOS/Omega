//! Declaration-owned borrow relations shared by callable admission and flow.
//! Opaque non-observation also queries reachable write-only authority through
//! this same scoped declaration traversal. That presence judgment cuts shared
//! access and consumes copy bounds; it publishes no lifetime or loan map.
//! Template dependence stays explicit until a call's ordinary substitutions
//! close it, rather than becoming proof that a generic input is view-free.
//! Applying a callback must preserve the same result/input relation that loan
//! attribution records. Keeping this query in one checking owner prevents
//! formation from accepting a region-erased shape that flow cannot implement.
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
//! Returned-view source rules:
//!
//! - an EXPLICIT output lifetime (`-> &'buf T` or `-> Message<'buf>`) links the
//!   output to every input carrying the same lifetime name (stage 2); the
//!   returned view's sources form the union of those inputs' matching leaves;
//! - otherwise (an ELIDED output) exactly one ref input is the source (elision
//!   rule 1); zero ref inputs leaves the output unlinked (historical behavior);
//!   two or more are ambiguous and must be disambiguated with a lifetime.
//!
//! For aggregate results or owned inputs carrying references, input references
//! include structurally carried leaves even when the output is a direct reference.
//! Explicit lifetimes select every leaf carrying that lifetime across all input
//! parameters; each candidate must supply the result's access, and the tracked
//! loan follows the candidate union. Elision requires one contained source, not
//! one parameter containing several unnamed sources.

use symbols::SymbolHandle;
use typed_trees::signature::StateParameter;
use typed_trees::state::State;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

use checked_trees::BorrowLoanOwnerSegment as BorrowOwnerSegment;

/// Where a returned view's borrow comes from.
#[derive(Debug, Clone)]
pub enum ViewReturnSource {
    /// The return type is not a view, or there is nothing to link (an elided
    /// output with zero ref inputs). No loan, no diagnostic.
    NotApplicable,
    /// The output borrows exactly one reference parameter at its positional
    /// argument index at a call site.
    Parameter { parameter_index: usize },
    /// Each result reference maps to the input named by its instantiated
    /// lifetime. A direct reference result has an empty output owner path.
    Fields { fields: Vec<ViewReturnFieldSource> },
    /// The signature is ambiguous or ill-formed; the declaration check turns
    /// this into a diagnostic and the loan attributor tracks no loan.
    Ambiguous(ViewReturnAmbiguity),
}

#[derive(Debug, Clone)]
pub struct ViewReturnFieldSource {
    pub owner_path: Vec<BorrowOwnerSegment>,
    pub source_path: Vec<BorrowOwnerSegment>,
    pub source: ViewReturnInputSource,
    pub source_type: TypeReferenceHandle,
    pub parameter_index: usize,
    pub kind: checked_trees::BorrowAccessKind,
}

/// Borrowing the parameter's referent and transferring a descriptor stored in
/// it have different lifetimes, even when both have an empty structural path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewReturnInputSource {
    Referent,
    Stored,
}

/// Why a view-returning signature could not be resolved to a single input.
#[derive(Debug, Clone)]
pub enum ViewReturnAmbiguity {
    /// A declaration cannot supply a complete structural lifetime frontier.
    IncompleteStructure { subject: String },
    /// Source access cannot supply the result's access without escalation.
    IncompatibleSourceAccess { input: String },
    /// Elided output with two or more candidate ref inputs (their names).
    ElidedMultipleInputs { candidates: Vec<String> },
    /// An explicit output lifetime that matches no input.
    LifetimeMatchesNoInput { lifetime: String },
}

/// Resolve the source of a state's returned view from its signature alone.
pub fn resolve_view_return_source(
    program: &dyn crate::CheckingInput,
    state: &State,
) -> ViewReturnSource {
    resolve_signature_view_return_source(
        program,
        program.state_parameters(state),
        state.return_type,
    )
}

/// Resolve the same lifetime/source relation for a bodyless callable
/// signature. Boundary-trait requirements and compile-time machine parameters
/// have no `State`, but their returned views create the same caller-side loans.
pub fn resolve_signature_view_return_source(
    program: &dyn crate::CheckingInput,
    parameters: &[StateParameter],
    return_type: TypeReferenceHandle,
) -> ViewReturnSource {
    if !returns_borrow(program, return_type) {
        return ViewReturnSource::NotApplicable;
    }

    // Direct and aggregate results select the same input sources. Looking only
    // at outer reference lifetimes would erase descriptors inside a borrow.
    structural_view_return_source(program, parameters, return_type, &[], &[], &[])
}

/// The same lifetime/source relation decided after one call's exact
/// substitution has closed a template-dependent signature. Declaration type
/// parameters resolve through `substitutions` inside every frontier walk, so
/// each instantiated output view leaf still borrows the exact input leaf its
/// lifetime names — or the one unambiguous input an elided leaf admits.
/// Carrier results resolve leaf by leaf through the shared complete-frontier
/// query. A relation that cannot close stays ambiguous rather than reading
/// as an empty frontier.
pub fn resolve_substituted_view_return_source(
    program: &dyn crate::CheckingInput,
    parameters: &[StateParameter],
    return_type: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    scope_starts: &[usize],
) -> ViewReturnSource {
    if !substituted_returns_borrow(program, return_type, substitutions, scope_starts) {
        return ViewReturnSource::NotApplicable;
    }
    structural_view_return_source(
        program,
        parameters,
        return_type,
        substitutions,
        scope_starts,
        &[],
    )
}

pub fn substituted_returns_borrow(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    scope_starts: &[usize],
) -> bool {
    substituted_carried_lifetimes(program, type_reference, substitutions, scope_starts)
        .is_none_or(|leaves| !leaves.is_empty())
}

mod callable_refinement;
mod callable_regions;
mod carried_lifetimes;
mod provider_outputs;
mod region_correspondence;
mod region_inference;

pub use provider_outputs::{
    ProviderLifetimeDependency, ProviderOutput, ProviderOutputDestination, resolve_provider_outputs,
};

pub(crate) use callable_refinement::lexical_region_owner;
pub use callable_refinement::{CallableBorrowContract, validate_callable_borrow_refinement};

/// Call admission and caller-side loans consume one applied requirement
/// relation. Invocation binders retain their declaration identity independently
/// of captured Type operands; ordinary requirements still need a full frontier.
pub fn resolve_applied_signature_view_return_source(
    program: &dyn crate::CheckingInput,
    signature: &typed_trees::signature::StateSignature,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    scope_starts: &[usize],
    argument_owner: SymbolHandle,
) -> ViewReturnSource {
    if signature.lifetime_parameters.is_empty() {
        return resolve_substituted_view_return_source(
            program,
            program.state_signature_parameters(signature),
            signature.return_type,
            substitutions,
            scope_starts,
        );
    }
    callable_refinement::resolve_invocation_view_return_source(
        program,
        CallableBorrowContract {
            lexical_owner: signature.symbol,
            lifetime_parameters: &signature.lifetime_parameters,
            parameters: program.state_signature_parameters(signature),
            return_type: signature.return_type,
            substitutions,
            substitution_scope_starts: scope_starts,
            application_scopes: &[],
            binders: &[],
            lifetime_bindings: &[],
            argument_owner,
            supplied_telescope: false,
        },
        scope_starts,
    )
}

pub use carried_lifetimes::{
    DeclarationLifetimeFrontier, declaration_lifetime_frontier,
    substituted_result_frontier_is_complete, substituted_result_is_view_free,
};
use carried_lifetimes::{carried_lifetimes, substituted_carried_lifetimes};

fn structural_view_return_source(
    program: &dyn crate::CheckingInput,
    parameters: &[StateParameter],
    return_type: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    scope_starts: &[usize],
    symbolic_parameters: &[SymbolHandle],
) -> ViewReturnSource {
    let Some(outputs) = carried_lifetimes::symbolic_carried_lifetimes(
        program,
        return_type,
        substitutions,
        scope_starts,
        symbolic_parameters,
    ) else {
        if substitutions.is_empty()
            && let Some(source) = whole_elided_result_source(program, parameters, return_type)
        {
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
    for (index, parameter) in parameters.iter().enumerate() {
        if !substituted_returns_borrow(
            program,
            parameter.type_reference,
            substitutions,
            scope_starts,
        ) {
            continue;
        }
        let Some(leaves) = carried_lifetimes::symbolic_input_lifetimes(
            program,
            parameter.type_reference,
            substitutions,
            scope_starts,
            symbolic_parameters,
        ) else {
            return ViewReturnSource::Ambiguous(ViewReturnAmbiguity::IncompleteStructure {
                subject: format!("input `{}`", parameter.name),
            });
        };
        for leaf in leaves {
            inputs.push((index, parameter, leaf));
        }
    }
    structural_frontier_return_source(outputs, &inputs)
}

fn structural_frontier_return_source(
    outputs: Vec<carried_lifetimes::CarriedLifetime>,
    inputs: &[(usize, &StateParameter, carried_lifetimes::CarriedLifetime)],
) -> ViewReturnSource {
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
                lifetime: match output.lifetime {
                    Some(carried_lifetimes::RegionIdentity::Declared(name)) => name,
                    Some(identity) => format!("{identity:?}"),
                    None => "elided".to_owned(),
                },
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
        // Every input parameter carrying the selected lifetime contributes
        // its matching leaves as candidate sources; each must still supply
        // the result's access.
        if matching.iter().any(|(_, _, input)| {
            use language_semantics::ReferenceAccess;
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
                source: input.source,
                source_type: parameter.type_reference,
                parameter_index: *index,
                kind: match output.access {
                    language_semantics::ReferenceAccess::Mutable => {
                        checked_trees::BorrowAccessKind::Mutable
                    }
                    language_semantics::ReferenceAccess::Shared => {
                        checked_trees::BorrowAccessKind::Read
                    }
                    language_semantics::ReferenceAccess::WriteOnly => {
                        checked_trees::BorrowAccessKind::WriteOnly
                    }
                },
            });
        }
    }
    ViewReturnSource::Fields { fields }
}

fn whole_elided_result_source(
    program: &dyn crate::CheckingInput,
    parameters: &[StateParameter],
    return_type: TypeReferenceHandle,
) -> Option<ViewReturnSource> {
    let output_accesses = carried_lifetimes::whole_elided_result_accesses(program, return_type)?;
    // The existing whole-aggregate fallback carries read/mutable polarity;
    // it cannot represent a write-only recursive result without escalation.
    if output_accesses.contains(&language_semantics::ReferenceAccess::WriteOnly) {
        return None;
    }
    let mut candidates = parameters
        .iter()
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
        *access == input.access || input.access == language_semantics::ReferenceAccess::Mutable
    }) {
        return Some(ViewReturnSource::Ambiguous(
            ViewReturnAmbiguity::IncompatibleSourceAccess {
                input: parameter.name.as_str().to_owned(),
            },
        ));
    }
    Some(ViewReturnSource::Parameter {
        parameter_index: index,
    })
}

/// Whether a type reference is a view (a reference, possibly under a
/// `Constrained` wrapper).
pub fn is_reference_type(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> bool {
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
pub fn returns_borrow(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> bool {
    is_reference_type(program, type_reference) || is_borrow_carrying_data(program, type_reference)
}

/// Non-observation follows accessible permission, not mere type occurrence.
/// A template-dependent input remains an obligation for its call application;
/// it is never classified as authority-free.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum InputWriteOnlyAccess {
    Absent,
    Present,
    TemplateDependent,
    Incomplete,
}

pub(crate) fn input_write_only_access(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
    parameters: &[typed_trees::data::TypeParameter],
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
) -> InputWriteOnlyAccess {
    carried_lifetimes::input_write_only_access(program, type_reference, parameters, substitutions)
}

/// Every structural projection within `type_reference` that carries a
/// reference. Persistent-storage checking uses this frontier to prove that a
/// place copied as a whole is backed only by static storage even when its
/// borrowed leaves were established by separate field writes.
///
/// `None` means the frontier cannot be enumerated completely (currently an
/// unresolved fixed-array length or a view-bearing recursive cycle), so callers
/// must fail closed unless the whole source place already has static provenance.
pub fn borrow_carrying_owner_paths(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> Option<Vec<Vec<BorrowOwnerSegment>>> {
    carried_lifetimes(program, type_reference)
        .map(|leaves| leaves.into_iter().map(|leaf| leaf.owner_path).collect())
}

/// Borrow classification and projected paths share the complete frontier.
/// An unresolved structure is not evidence of absent loans. Consumers must
/// retain the obligation until admission supplies its complete instantiated
/// frontier; a second symbol-only walk previously lost finite nested heads.
pub fn is_borrow_carrying_data(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> bool {
    carried_lifetimes(program, type_reference).is_none_or(|leaves| !leaves.is_empty())
}

/// Whether a data value carries an exclusive leaf, conservatively retaining
/// that obligation when its complete structural frontier is not available.
pub fn is_mutably_borrow_carrying_data(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> bool {
    carried_lifetimes(program, type_reference)
        .is_none_or(|leaves| leaves.iter().any(|leaf| leaf.access.is_exclusive()))
}

fn data_definition(
    program: &dyn crate::CheckingInput,
    symbol: SymbolHandle,
) -> Option<&typed_trees::data::DataDefinition> {
    program.data_definition_by_symbol(symbol)
}
