use arena::HandleSpan;
use language_core::operator_spelling::OperatorSpelling;
use language_semantics::const_value::{CanonicalConstIdentity, CanonicalConstValue};
use symbols::SymbolHandle;

use crate::TypedTrees;
use crate::data::TypeParameter;
use crate::domain::DomainDefinition;
use crate::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

mod applications;
mod indexing;

pub use indexing::resolve_indexed_spelling_for_operands;

pub use applications::{
    ClosedOperatorApplicationArgument, ClosedOperatorRealizationApplication,
    SymbolicOperatorTypeApplicationArgument, closed_indexed_operator_application_for_operands,
    closed_operator_application_for_operands, closed_operator_realization_application,
    symbolic_operator_type_application_for_operands,
};

#[derive(Debug, Clone, PartialEq, Eq)]
struct OperatorConstBinding {
    symbol: SymbolHandle,
    value: CanonicalConstIdentity,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OperatorDefinition {
    pub is_public: bool,
    pub is_boundary: bool,
    pub symbol: SymbolHandle,
    pub name: HandleSpan<crate::name::Identifier>,
    pub lifetime_parameters: Vec<crate::name::Identifier>,
    pub type_parameters: HandleSpan<crate::data::TypeParameter>,
    pub parameters: HandleSpan<crate::signature::StateParameter>,
    pub return_type: crate::types::TypeReferenceHandle,
    pub contracts: HandleSpan<crate::signature::SignatureContract>,
    /// Optional `spelling` clause carried from syntax (Wave 0 decision #3).
    pub spelling: Option<OperatorSpelling>,
    pub token_count: usize,
}

/// A spelled operator meaning visible at a use site: a root operator, or a
/// domain operator together with its owning domain.
#[derive(Debug, Clone, Copy)]
pub struct SpelledOperator<'program> {
    pub operator: &'program OperatorDefinition,
    pub domain: Option<&'program DomainDefinition>,
}

/// Find one exact independently nameable operator declaration, whether it is
/// rooted directly or stored beneath a domain home. Visibility belongs to the
/// operator itself; callers must not infer it from the carrier/domain path.
pub fn declaration_by_symbol(
    program: &TypedTrees,
    symbol: SymbolHandle,
) -> Option<&OperatorDefinition> {
    program
        .operators()
        .iter()
        .chain(
            program
                .domain_definitions()
                .iter()
                .flat_map(|domain| program.domain_operators(domain)),
        )
        .find(|operator| operator.symbol == symbol)
}

/// Resolve an explicitly named operator call from its path and arity facts.
///
/// Named calls may reach this stage without an early `target_symbol`, so every
/// consumer must use this same ambiguity-checked fallback rather than accepting
/// a leaf spelling on its own. Return types never distinguish overloads.
pub fn resolve_named_call<'program>(
    program: &'program TypedTrees,
    target_symbol: SymbolHandle,
    static_receiver_segments: Option<&[&str]>,
    target_name: &str,
    argument_count: usize,
    has_value_receiver: bool,
) -> Option<&'program OperatorDefinition> {
    if target_symbol.is_valid() {
        return program
            .operators()
            .iter()
            .find(|operator| operator.symbol == target_symbol);
    }

    let mut candidates = program.operators().iter().filter(|operator| {
        named_operator_path_matches_call(program, operator, static_receiver_segments, target_name)
            && named_operator_arity_fits_call(program, operator, argument_count, has_value_receiver)
    });
    let first = candidates.next()?;
    candidates.next().is_none().then_some(first)
}

/// Resolve an expression-position named operator call using the receiver path
/// already retained in the typed expression table.
pub fn resolve_named_expression_call<'program>(
    program: &'program TypedTrees,
    call: &crate::expression::TableCallExpression,
) -> Option<&'program OperatorDefinition> {
    if call.target_symbol.is_valid() {
        return program
            .operators()
            .iter()
            .find(|operator| operator.symbol == call.target_symbol);
    }
    let candidates = named_expression_call_candidates(program, call);
    let [selected] = candidates.as_slice() else {
        return None;
    };
    Some(*selected)
}

/// Return every named operator matching an expression call's retained path and
/// arity. Result-overload normalization uses this before one exact result
/// dispatch has been selected, when ordinary single-candidate resolution must
/// deliberately report ambiguity.
pub fn named_expression_call_candidates<'program>(
    program: &'program TypedTrees,
    call: &crate::expression::TableCallExpression,
) -> Vec<&'program OperatorDefinition> {
    if call.target_symbol.is_valid() {
        return program
            .operators()
            .iter()
            .filter(|operator| operator.symbol == call.target_symbol)
            .collect();
    }
    let mut static_segments = Vec::new();
    let has_value_receiver = if !call.receiver.is_valid() {
        false
    } else if let crate::expression::ExpressionNode::Name(path) =
        program.expression_table.expression(call.receiver)
    {
        let receiver_members = program.expression_table.name_path_members(path.members);
        let is_known_static_namespace = path.symbol.is_valid()
            && (program
                .data_definitions()
                .iter()
                .any(|definition| definition.symbol == path.symbol)
                || program
                    .domain_definitions()
                    .iter()
                    .any(|definition| definition.symbol == path.symbol)
                || program
                    .machines()
                    .iter()
                    .any(|definition| definition.symbol == path.symbol)
                || program
                    .traits()
                    .iter()
                    .any(|definition| definition.symbol == path.symbol));
        // Named operator lowering intentionally may leave a static namespace
        // path symbol unresolved while retaining the exact call selection.
        // Reconstruct only the namespace/value classification from the closed
        // operator vocabulary and complete authored path; selection itself is
        // still finalized separately.
        let is_operator_namespace = !path.symbol.is_valid()
            && program.operators().iter().any(|operator| {
                let operator_path = program.operator_path_members(operator.name);
                operator_path
                    .split_last()
                    .is_some_and(|(member, namespace)| {
                        member.as_str() == call.target.as_str()
                            && namespace.len() == receiver_members.len()
                            && namespace
                                .iter()
                                .zip(receiver_members)
                                .all(|(expected, actual)| expected == actual)
                    })
            });
        let is_static_namespace = is_known_static_namespace || is_operator_namespace;
        if is_static_namespace {
            static_segments.extend(receiver_members.iter().map(|segment| segment.as_str()));
        }
        !is_static_namespace
    } else {
        true
    };
    let static_receiver_segments =
        (!static_segments.is_empty()).then_some(static_segments.as_slice());
    let argument_count = program
        .expression_table
        .expression_handles(call.arguments)
        .len();
    program
        .operators()
        .iter()
        .filter(|operator| {
            named_operator_path_matches_call(
                program,
                operator,
                static_receiver_segments,
                call.target.as_str(),
            ) && named_operator_arity_fits_call(
                program,
                operator,
                argument_count,
                has_value_receiver,
            )
        })
        .collect()
}

/// Statement-position counterpart to [`named_expression_call_candidates`].
/// Statement calls retain their receiver path in the statement table rather
/// than as an expression node, but static namespaces and value receivers obey
/// the same path/arity rule.
pub fn named_statement_call_candidates<'program>(
    program: &'program TypedTrees,
    call: &crate::statement::TableCall,
) -> Vec<&'program OperatorDefinition> {
    if call.target_symbol.is_valid() {
        return program
            .operators()
            .iter()
            .filter(|operator| operator.symbol == call.target_symbol)
            .collect();
    }
    let receiver = program.statement_table.name_path_members(call.receiver);
    let is_static_namespace = call.receiver_symbol.is_valid()
        && (program
            .data_definitions()
            .iter()
            .any(|definition| definition.symbol == call.receiver_symbol)
            || program
                .domain_definitions()
                .iter()
                .any(|definition| definition.symbol == call.receiver_symbol)
            || program
                .machines()
                .iter()
                .any(|definition| definition.symbol == call.receiver_symbol)
            || program
                .traits()
                .iter()
                .any(|definition| definition.symbol == call.receiver_symbol));
    let static_segments = is_static_namespace.then(|| {
        receiver
            .iter()
            .map(|segment| segment.as_str())
            .collect::<Vec<_>>()
    });
    let has_value_receiver = !receiver.is_empty() && !is_static_namespace;
    let argument_count = program
        .statement_table
        .expression_handles(call.arguments)
        .len();
    program
        .operators()
        .iter()
        .filter(|operator| {
            named_operator_path_matches_call(
                program,
                operator,
                static_segments.as_deref(),
                call.target.as_str(),
            ) && named_operator_arity_fits_call(
                program,
                operator,
                argument_count,
                has_value_receiver,
            )
        })
        .collect()
}

/// Resolve one machine's explicit `satisfies Namespace::requirement` edge to
/// an exact overloaded boundary operator. Trait conformances keep their own
/// resolver; this path is for target/provider machines realizing an operator
/// requirement such as the f32 or f64 overload of `Float::add`.
pub fn resolve_satisfied_boundary_operator<'program>(
    program: &'program TypedTrees,
    machine: &crate::machine::Machine,
    namespace: &str,
    requirement: &str,
) -> Option<&'program OperatorDefinition> {
    resolve_satisfied_operator(program, machine, namespace, requirement, true)
}

/// Resolve an ordinary checked machine's exact operator requirement. This is
/// the PDI3 counterpart to the boundary-provider route: signature and path are
/// identical, while `boundary_only` is deliberately false.
pub fn resolve_satisfied_checked_operator<'program>(
    program: &'program TypedTrees,
    machine: &crate::machine::Machine,
    namespace: &str,
    requirement: &str,
) -> Option<&'program OperatorDefinition> {
    resolve_satisfied_operator(program, machine, namespace, requirement, false)
}

/// Resolve a concrete generic checked-body specialization through the exact
/// closed operator application retained by authoritative monomorphization.
///
/// This is deliberately separate from [`resolve_satisfied_checked_operator`]:
/// the latter proves the authored generic declaration relation, while this
/// path proves one closed specialization and returns its retained application
/// for category/bounds/commitment replay.
pub fn resolve_specialized_checked_operator_application<'program>(
    program: &'program TypedTrees,
    machine: &'program crate::machine::Machine,
    namespace: &str,
    requirement: &str,
) -> Option<(
    &'program OperatorDefinition,
    &'program ClosedOperatorRealizationApplication,
)> {
    let mut specializations = program
        .machine_specializations
        .iter()
        .filter(|specialization| specialization.instance == machine.symbol);
    let specialization = specializations.next()?;
    if specializations.next().is_some() {
        return None;
    }
    let mut rows = specialization
        .operator_realizations
        .iter()
        .filter_map(|row| {
            let operator = declaration_by_symbol(program, row.requirement_symbol)?;
            operator_path_matches(operator, program, namespace, requirement)
                .then_some((operator, row))
        });
    let (operator, retained) = rows.next()?;
    if rows.next().is_some()
        || closed_operator_realization_application(program, machine, operator).as_ref()
            != Some(retained)
    {
        return None;
    }
    Some((operator, retained))
}

fn resolve_satisfied_operator<'program>(
    program: &'program TypedTrees,
    machine: &crate::machine::Machine,
    namespace: &str,
    requirement: &str,
    boundary_only: bool,
) -> Option<&'program OperatorDefinition> {
    let state = program.machine_states(machine).first()?;
    let actual_parameters = program.state_parameters(state);
    let mut candidates = program.operators().iter().filter(|operator| {
        if (boundary_only && !operator.is_boundary)
            || !operator_path_matches(operator, program, namespace, requirement)
        {
            return false;
        }
        let Some((machine_binders, operator_binders)) =
            operator_realization_static_binders(program, machine, operator)
        else {
            return false;
        };
        let required_parameters = program.operator_parameters(operator);
        actual_parameters.len() == required_parameters.len()
            && actual_parameters
                .iter()
                .zip(required_parameters.iter())
                .all(|(actual, required)| {
                    actual.is_self == required.is_self
                        && actual.is_const == required.is_const
                        && actual.is_mutable == required.is_mutable
                        && program.normalized_type_identity_with_binders(
                            actual.type_reference,
                            &machine_binders,
                        ) == program.normalized_type_identity_with_binders(
                            required.type_reference,
                            &operator_binders,
                        )
                })
            && state.return_type.is_valid() == operator.return_type.is_valid()
            && (!state.return_type.is_valid()
                || program
                    .normalized_type_identity_with_binders(state.return_type, &machine_binders)
                    == program.normalized_type_identity_with_binders(
                        operator.return_type,
                        &operator_binders,
                    ))
    });
    let selected = candidates.next()?;
    candidates.next().is_none().then_some(selected)
}

/// Build one alpha-normalized relation between a realizing machine's static
/// telescope and the operator requirement telescope it claims to satisfy.
///
/// Type and const parameters are matched by category and declaration order;
/// names and private symbols are not identity. Const carriers remain exact;
/// provider type-property demands may weaken the requirement but never
/// strengthen it. Static-machine and proposition parameters have no
/// operator-application replay rule yet and therefore fail closed here instead
/// of being flattened to a count.
fn operator_realization_static_binders(
    program: &TypedTrees,
    machine: &crate::machine::Machine,
    operator: &OperatorDefinition,
) -> Option<(Vec<(SymbolHandle, String)>, Vec<(SymbolHandle, String)>)> {
    let machine_parameters = program.machine_type_parameters(machine);
    let operator_parameters = program.operator_type_parameters(operator);
    if machine_parameters.len() != operator_parameters.len() {
        return None;
    }
    let machine_binders = machine_parameters
        .iter()
        .enumerate()
        .map(|(ordinal, parameter)| (parameter.symbol, format!("${ordinal}")))
        .collect::<Vec<_>>();
    let operator_binders = operator_parameters
        .iter()
        .enumerate()
        .map(|(ordinal, parameter)| (parameter.symbol, format!("${ordinal}")))
        .collect::<Vec<_>>();
    for (machine_parameter, operator_parameter) in
        machine_parameters.iter().zip(operator_parameters)
    {
        match (&machine_parameter.kind, &operator_parameter.kind) {
            (crate::data::TypeParameterKind::Type, crate::data::TypeParameterKind::Type)
                if !crate::data::type_parameter_demands_stronger_properties(
                    operator_parameter,
                    machine_parameter,
                ) => {}
            (
                crate::data::TypeParameterKind::Const {
                    type_reference: machine_carrier,
                },
                crate::data::TypeParameterKind::Const {
                    type_reference: operator_carrier,
                },
            ) if machine_parameter.bounds == operator_parameter.bounds
                && program
                    .normalized_type_identity_with_binders(*machine_carrier, &machine_binders)
                    == program.normalized_type_identity_with_binders(
                        *operator_carrier,
                        &operator_binders,
                    ) => {}
            _ => return None,
        }
    }
    Some((machine_binders, operator_binders))
}

fn operator_path_matches(
    operator: &OperatorDefinition,
    program: &TypedTrees,
    namespace: &str,
    requirement: &str,
) -> bool {
    let path = program.operator_path_members(operator.name);
    matches!(path, [owner, member]
        if owner.as_str() == namespace && member.as_str() == requirement)
}

/// Stable slot identity for one exact overloaded boundary-operator
/// requirement. Provider selection must distinguish f32 and f64 overloads even
/// though both are authored as `Float::add`.
pub fn boundary_operator_requirement_identity(
    program: &TypedTrees,
    operator: &OperatorDefinition,
) -> String {
    let path = program
        .operator_path_members(operator.name)
        .iter()
        .map(|member| member.as_str())
        .collect::<Vec<_>>()
        .join("::");
    let parameters = program
        .operator_parameters(operator)
        .iter()
        .map(|parameter| {
            program
                .normalized_type_identity(parameter.type_reference)
                .into_string()
        })
        .collect::<Vec<_>>()
        .join(",");
    let result = operator.return_type.is_valid().then(|| {
        program
            .normalized_type_identity(operator.return_type)
            .into_string()
    });
    format!(
        "operator::{path}({parameters})->{}",
        result.as_deref().unwrap_or("unit")
    )
}

fn named_operator_path_matches_call(
    program: &TypedTrees,
    operator: &OperatorDefinition,
    static_receiver_segments: Option<&[&str]>,
    target_name: &str,
) -> bool {
    let path = program.operator_path_members(operator.name);
    let Some((last, prefix)) = path.split_last() else {
        return false;
    };
    if last.as_str() != target_name {
        return false;
    }

    match static_receiver_segments {
        Some(segments) => {
            prefix.len() == segments.len()
                && prefix
                    .iter()
                    .zip(segments.iter())
                    .all(|(member, segment)| member.as_str() == *segment)
        }
        None => true,
    }
}

fn named_operator_arity_fits_call(
    program: &TypedTrees,
    operator: &OperatorDefinition,
    argument_count: usize,
    has_value_receiver: bool,
) -> bool {
    let parameters = program.operator_parameters(operator);
    let has_self = parameters.iter().any(|parameter| parameter.is_self);
    let positional = parameters
        .iter()
        .filter(|parameter| !parameter.is_self)
        .count();

    if has_self {
        return has_value_receiver && positional == argument_count;
    }
    if has_value_receiver {
        return positional == argument_count + 1;
    }
    positional == argument_count
}

/// Enumerate an operator `spelling`, optionally narrowed by the first operand.
/// This receiver-only query remains useful to validation that merely asks
/// whether a spelling exists for a carrier. Complete use-site resolution goes
/// through [`resolve_spelling_for_operands`]. Return types never distinguish.
pub fn resolve_spelling<'program>(
    program: &'program TypedTrees,
    spelling: OperatorSpelling,
    receiver_type: Option<TypeReferenceHandle>,
) -> Vec<SpelledOperator<'program>> {
    let root_candidates = program
        .operators()
        .iter()
        .filter(|operator| operator.spelling == Some(spelling))
        .map(|operator| SpelledOperator {
            operator,
            domain: None,
        });
    let domain_candidates = program.domain_definitions().iter().flat_map(|domain| {
        program
            .domain_operators(domain)
            .iter()
            .filter(move |operator| operator.spelling == Some(spelling))
            .map(move |operator| SpelledOperator {
                operator,
                domain: Some(domain),
            })
    });

    root_candidates
        .chain(domain_candidates)
        .filter(|candidate| match receiver_type {
            Some(receiver_type) => {
                operator_matches_receiver(program, candidate.operator, receiver_type)
            }
            None => true,
        })
        .collect()
}

/// Resolve a spelling against the complete operand tuple. `None` retains a
/// candidate for an operand whose type is not recoverable at this stage;
/// every known position must match, and generic parameter bindings are shared
/// across the tuple. This is the single use-site resolution authority. The
/// checked stage records its outcome as durable evidence
/// (`CheckedOperatorFacts`) for diagnostics and proof lowering rather than
/// re-resolving.
pub fn resolve_spelling_for_operands<'program>(
    program: &'program TypedTrees,
    spelling: OperatorSpelling,
    operand_types: &[Option<TypeReferenceHandle>],
) -> Vec<SpelledOperator<'program>> {
    resolve_spelling(program, spelling, None)
        .into_iter()
        .filter(|candidate| operator_matches_operands(program, candidate.operator, operand_types))
        .collect()
}

/// Admit builtin expression meaning only when neither declared nor selected
/// trait meanings apply and every authored occurrence retains builtin custody.
/// Unknown operand types stay wildcard candidates; absence of a later checked
/// operator row is not independently evidence of builtin meaning.
pub fn has_builtin_spelled_expression_meaning(
    program: &TypedTrees,
    machine_symbol: SymbolHandle,
    expression: crate::expression::ExpressionHandle,
    spelling: OperatorSpelling,
    operand_types: &[Option<TypeReferenceHandle>],
) -> bool {
    use language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionIntrinsic as Intrinsic,
        AuthoredDeclarationSelectionLateBinding as LateBinding,
        AuthoredDeclarationSelectionTarget as Target,
    };
    resolve_spelling_for_operands(program, spelling, operand_types).is_empty()
        && selected_trait_operator_meanings(program, machine_symbol, spelling, operand_types)
            .is_empty()
        && program
            .expression_table
            .authored_selection_occurrences(expression)
            .all(|occurrence| {
                program
                    .authored_declaration_selections()
                    .get(occurrence)
                    .is_some_and(|selection| {
                        matches!(
                            selection.target(),
                            Target::Intrinsic(Intrinsic::BuiltinOperator)
                                | Target::LateBound(LateBinding::CheckedOperator)
                        )
                    })
            })
}

fn operator_matches_operands(
    program: &TypedTrees,
    operator: &OperatorDefinition,
    operand_types: &[Option<TypeReferenceHandle>],
) -> bool {
    operator_matches_operands_with_indexed_collection(program, operator, operand_types, false)
}

fn operator_matches_operands_with_indexed_collection(
    program: &TypedTrees,
    operator: &OperatorDefinition,
    operand_types: &[Option<TypeReferenceHandle>],
    indexed_collection: bool,
) -> bool {
    let parameters = program.operator_parameters(operator);
    if parameters.len() != operand_types.len() {
        return false;
    }
    let type_parameters = program.operator_type_parameters(operator);
    let mut bindings = Vec::new();
    let mut const_bindings = Vec::new();
    operand_types
        .iter()
        .zip(normalized_operand_parameters(parameters))
        .enumerate()
        .all(|(position, (actual, expected))| {
            actual.is_none_or(|actual| {
                let (matched_actual, matched_expected) = if indexed_collection && position == 0 {
                    indexing::shared_collection_elements(program, actual, expected.type_reference)
                        .unwrap_or((actual, expected.type_reference))
                } else {
                    (actual, expected.type_reference)
                };
                type_reference_matches(
                    program,
                    matched_actual,
                    matched_expected,
                    None,
                    type_parameters,
                    &mut bindings,
                    &mut const_bindings,
                ) && declared_domain_constraints_match(program, actual, expected.type_reference)
                    && declared_domain_constraints_match(program, matched_actual, matched_expected)
            })
        })
}

/// Operator operand matching is structurally permissive about refinements, but
/// a declared semantic domain named by the operator parameter is part of that
/// meaning's dispatch key. In particular, `Quantity<METER>` and
/// `Quantity<KILOMETER>` share one family symbol while carrying different
/// normalized instance identities; stripping both constrained shells would
/// make ordinary per-unit overloads ambiguous.
fn declared_domain_constraints_match(
    program: &TypedTrees,
    actual: TypeReferenceHandle,
    expected: TypeReferenceHandle,
) -> bool {
    let expected_domains = declared_domain_constraints(program, expected);
    if expected_domains.is_empty() {
        return true;
    }
    let actual_domains = declared_domain_constraints(program, actual);
    expected_domains.iter().all(|expected| {
        actual_domains
            .iter()
            .any(|actual| declared_domain_constraint_matches(program, actual, expected))
    })
}

fn declared_domain_constraints(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Vec<&crate::types::DomainConstraint> {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            declared_domain_constraints(program, *referee)
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            let mut domains = declared_domain_constraints(program, *base_type);
            domains.extend(
                program
                    .type_reference_table
                    .constraints(*constraints)
                    .iter()
                    .filter_map(|constraint| match constraint {
                        crate::types::TypeConstraintNode::Domain(domain) => Some(domain),
                        _ => None,
                    }),
            );
            domains
        }
        _ => Vec::new(),
    }
}

fn declared_domain_constraint_matches(
    program: &TypedTrees,
    actual: &crate::types::DomainConstraint,
    expected: &crate::types::DomainConstraint,
) -> bool {
    if actual.semantic_id.is_valid() && expected.semantic_id.is_valid() {
        return actual.semantic_id == expected.semantic_id;
    }
    if !actual.symbol.is_valid() || actual.symbol != expected.symbol {
        return false;
    }
    actual.arguments.len() == expected.arguments.len()
        && actual
            .arguments
            .iter()
            .zip(&expected.arguments)
            .all(|(actual, expected)| {
                program.normalized_type_identity(*actual)
                    == program.normalized_type_identity(*expected)
            })
}

/// Whether the operator's first parameter (its receiver) accepts a value of
/// `receiver_type`, binding the operator's own type parameters structurally.
fn operator_matches_receiver(
    program: &TypedTrees,
    operator: &OperatorDefinition,
    receiver_type: TypeReferenceHandle,
) -> bool {
    let Some(receiver_parameter) = program.operator_parameters(operator).first() else {
        return false;
    };
    type_reference_matches(
        program,
        receiver_type,
        receiver_parameter.type_reference,
        None,
        program.operator_type_parameters(operator),
        &mut Vec::new(),
        &mut Vec::new(),
    )
}

/// Whether one exact selected conformance application supplies this
/// trait-owned fixed-token requirement for the complete operand tuple.
///
/// The application is already the proof-static binder selected by generic
/// specialization. Matching therefore consults no visible conformance set: it
/// binds the trait's `Self` owner and declared type parameters from the
/// requirement telescope, then checks those bindings against the closed
/// application's retained subject and trait arguments.
pub fn trait_operator_matches_application(
    program: &TypedTrees,
    trait_definition: &crate::trait_definition::TraitDefinition,
    requirement: &crate::signature::StateSignature,
    application: &crate::typed_trees::ClosedConformanceApplication,
    operand_types: &[Option<TypeReferenceHandle>],
) -> bool {
    let parameters = program.state_signature_parameters(requirement);
    if parameters.len() != operand_types.len() || operand_types.iter().all(Option::is_none) {
        return false;
    }

    let type_parameters = program
        .trait_type_parameters(trait_definition)
        .iter()
        .chain(program.state_signature_type_parameters(requirement))
        .cloned()
        .collect::<Vec<_>>();
    let mut bindings = Vec::new();
    let mut const_bindings = Vec::new();
    if !operand_types
        .iter()
        .zip(normalized_operand_parameters(parameters))
        .all(|(actual, expected)| {
            actual.is_none_or(|actual| {
                type_reference_matches(
                    program,
                    actual,
                    expected.type_reference,
                    Some(trait_definition.symbol),
                    &type_parameters,
                    &mut bindings,
                    &mut const_bindings,
                )
            })
        })
    {
        return false;
    }

    let binding_identity = |symbol| {
        bindings.iter().find_map(|(bound, actual)| {
            (*bound == symbol).then(|| program.display_type_reference(*actual))
        })
    };
    if let Some(subject) = binding_identity(trait_definition.symbol)
        && application.subject_identity.as_deref() != Some(subject.as_str())
    {
        return false;
    }
    application.trait_definition != trait_definition.symbol
        || program
            .trait_type_parameters(trait_definition)
            .iter()
            .zip(&application.trait_arguments)
            .all(|(parameter, expected)| {
                binding_identity(parameter.symbol).is_none_or(|actual| actual == *expected)
            })
}

#[derive(Debug, Clone, Copy)]
pub struct SelectedTraitOperatorMeaning<'program> {
    pub trait_definition: &'program crate::trait_definition::TraitDefinition,
    pub requirement: &'program crate::signature::StateSignature,
    pub application: &'program crate::typed_trees::ClosedConformanceApplication,
    pub row: &'program crate::typed_trees::ClosedConformanceRowIdentity,
}

/// Fixed-token meanings supplied by the proof-static conformance applications
/// already selected on one specialized machine. This is the sole lookup: it
/// walks no package-visible conformance declarations and cannot manufacture an
/// application from operand types.
pub fn selected_trait_operator_meanings<'program>(
    program: &'program TypedTrees,
    machine_symbol: SymbolHandle,
    spelling: OperatorSpelling,
    operand_types: &[Option<TypeReferenceHandle>],
) -> Vec<SelectedTraitOperatorMeaning<'program>> {
    let Some(specialization) = program
        .machine_specializations
        .iter()
        .find(|specialization| specialization.instance == machine_symbol)
    else {
        return Vec::new();
    };

    specialization
        .conformance_applications
        .iter()
        .flat_map(|application| {
            application.rows.iter().filter_map(move |row| {
                let trait_definition = program
                    .traits()
                    .iter()
                    .find(|candidate| candidate.symbol == row.declaring_trait)?;
                let requirement = program
                    .trait_machine_signatures(trait_definition)
                    .iter()
                    .find(|candidate| candidate.symbol == row.requirement)?;
                (requirement.spelling == Some(spelling)
                    && trait_operator_matches_application(
                        program,
                        trait_definition,
                        requirement,
                        application,
                        operand_types,
                    ))
                .then_some(SelectedTraitOperatorMeaning {
                    trait_definition,
                    requirement,
                    application,
                    row,
                })
            })
        })
        .collect()
}

fn type_reference_matches(
    program: &TypedTrees,
    actual: TypeReferenceHandle,
    expected: TypeReferenceHandle,
    bindable_owner: Option<SymbolHandle>,
    type_parameters: &[TypeParameter],
    bindings: &mut Vec<(SymbolHandle, TypeReferenceHandle)>,
    const_bindings: &mut Vec<OperatorConstBinding>,
) -> bool {
    type_reference_matches_with_policy(
        program,
        actual,
        expected,
        bindable_owner,
        type_parameters,
        bindings,
        const_bindings,
        true,
    )
}

fn type_reference_matches_with_policy(
    program: &TypedTrees,
    actual: TypeReferenceHandle,
    expected: TypeReferenceHandle,
    bindable_owner: Option<SymbolHandle>,
    type_parameters: &[TypeParameter],
    bindings: &mut Vec<(SymbolHandle, TypeReferenceHandle)>,
    const_bindings: &mut Vec<OperatorConstBinding>,
    allow_name_fallback: bool,
) -> bool {
    if !actual.is_valid() || !expected.is_valid() {
        return false;
    }
    if let Some(parameter) =
        expected_type_parameter(program, expected, type_parameters, allow_name_fallback)
        && let crate::data::TypeParameterKind::Const { type_reference } = parameter.kind
    {
        if let Some(value) =
            closed_const_identity_from_type_reference(program, actual, type_reference)
        {
            return bind_operator_const(const_bindings, parameter.symbol, value);
        }
        // Ordinary unresolved generic matching historically permits forwarded
        // symbolic arguments. Exact D29 inference disables name fallback and
        // therefore fails closed instead of treating such a demand as closed.
        if !allow_name_fallback {
            return false;
        }
    }
    if let Some(bindable_symbol) = expected_bindable_symbol(
        program,
        expected,
        bindable_owner,
        type_parameters,
        allow_name_fallback,
    ) {
        if let Some((_, bound_actual)) = bindings
            .iter()
            .find(|(symbol, _)| *symbol == bindable_symbol)
        {
            return type_reference_matches_with_policy(
                program,
                actual,
                *bound_actual,
                None,
                &[],
                &mut Vec::new(),
                &mut Vec::new(),
                allow_name_fallback,
            );
        }
        bindings.push((bindable_symbol, actual));
        return true;
    }

    match (
        program.type_reference_table.type_reference(actual),
        program.type_reference_table.type_reference(expected),
    ) {
        (
            TypeReferenceNode::Reference {
                referee: actual_referee,
                access: actual_access,
                // Lifetimes do not affect operator/conformance type matching.
                lifetime: _,
            },
            TypeReferenceNode::Reference {
                referee: expected_referee,
                access: expected_access,
                lifetime: _,
            },
        ) => {
            actual_access == expected_access
                && type_reference_matches_with_policy(
                    program,
                    *actual_referee,
                    *expected_referee,
                    bindable_owner,
                    type_parameters,
                    bindings,
                    const_bindings,
                    allow_name_fallback,
                )
        }
        (
            TypeReferenceNode::Constrained {
                base_type: actual_base,
                ..
            },
            _,
        ) => type_reference_matches_with_policy(
            program,
            *actual_base,
            expected,
            bindable_owner,
            type_parameters,
            bindings,
            const_bindings,
            allow_name_fallback,
        ),
        (
            _,
            TypeReferenceNode::Constrained {
                base_type: expected_base,
                ..
            },
        ) => type_reference_matches_with_policy(
            program,
            actual,
            *expected_base,
            bindable_owner,
            type_parameters,
            bindings,
            const_bindings,
            allow_name_fallback,
        ),
        (
            TypeReferenceNode::FixedArray {
                element_type: actual_element,
                length: actual_length,
            },
            TypeReferenceNode::FixedArray {
                element_type: expected_element,
                length: expected_length,
            },
        ) => {
            fixed_array_lengths_match(
                program,
                actual_length,
                expected_length,
                type_parameters,
                const_bindings,
                allow_name_fallback,
            ) && type_reference_matches_with_policy(
                program,
                *actual_element,
                *expected_element,
                bindable_owner,
                type_parameters,
                bindings,
                const_bindings,
                allow_name_fallback,
            )
        }
        (
            TypeReferenceNode::Slice {
                element_type: actual_element,
            },
            TypeReferenceNode::Slice {
                element_type: expected_element,
            },
        ) => type_reference_matches_with_policy(
            program,
            *actual_element,
            *expected_element,
            bindable_owner,
            type_parameters,
            bindings,
            const_bindings,
            allow_name_fallback,
        ),
        (
            TypeReferenceNode::Named {
                symbol: actual_symbol,
                ..
            },
            TypeReferenceNode::Generic { .. },
        ) if actual_symbol.is_valid() => program
            .data_definitions()
            .iter()
            .find(|definition| definition.symbol == *actual_symbol)
            .and_then(|definition| definition.generic_instance)
            .is_some_and(|origin| {
                type_reference_matches_with_policy(
                    program,
                    origin,
                    expected,
                    bindable_owner,
                    type_parameters,
                    bindings,
                    const_bindings,
                    allow_name_fallback,
                )
            }),
        (
            TypeReferenceNode::Named {
                symbol: actual_symbol,
                name: actual_name,
            },
            TypeReferenceNode::Named {
                symbol: expected_symbol,
                name: expected_name,
            },
        ) => nominal_type_identity_matches(
            *actual_symbol,
            actual_name.as_str(),
            *expected_symbol,
            expected_name.as_str(),
            allow_name_fallback,
        ),
        (
            TypeReferenceNode::Generic {
                base_symbol: actual_symbol,
                base_name: actual_name,
                arguments: actual_arguments,
                ..
            },
            TypeReferenceNode::Generic {
                base_symbol: expected_symbol,
                base_name: expected_name,
                arguments: expected_arguments,
                ..
            },
        ) => {
            nominal_type_identity_matches(
                *actual_symbol,
                actual_name.as_str(),
                *expected_symbol,
                expected_name.as_str(),
                allow_name_fallback,
            ) && type_reference_spans_match(
                program,
                *actual_arguments,
                *expected_arguments,
                bindable_owner,
                type_parameters,
                bindings,
                const_bindings,
                allow_name_fallback,
            )
        }
        (TypeReferenceNode::Unit, TypeReferenceNode::Unit) => true,
        _ => false,
    }
}

fn type_reference_spans_match(
    program: &TypedTrees,
    actual: HandleSpan<TypeReferenceHandle>,
    expected: HandleSpan<TypeReferenceHandle>,
    bindable_owner: Option<SymbolHandle>,
    type_parameters: &[TypeParameter],
    bindings: &mut Vec<(SymbolHandle, TypeReferenceHandle)>,
    const_bindings: &mut Vec<OperatorConstBinding>,
    allow_name_fallback: bool,
) -> bool {
    let actual = program.type_reference_table.type_reference_handles(actual);
    let expected = program
        .type_reference_table
        .type_reference_handles(expected);
    actual.len() == expected.len()
        && actual.iter().zip(expected).all(|(actual, expected)| {
            type_reference_matches_with_policy(
                program,
                *actual,
                *expected,
                bindable_owner,
                type_parameters,
                bindings,
                const_bindings,
                allow_name_fallback,
            )
        })
}

fn fixed_array_lengths_match(
    program: &TypedTrees,
    actual: &FixedArrayLength,
    expected: &FixedArrayLength,
    type_parameters: &[TypeParameter],
    const_bindings: &mut Vec<OperatorConstBinding>,
    allow_name_fallback: bool,
) -> bool {
    let FixedArrayLength::ConstParameter { symbol, name } = expected else {
        return actual == expected;
    };
    let Some(parameter) = type_parameters.iter().find(|parameter| {
        (symbol.is_valid() && parameter.symbol == *symbol)
            || (allow_name_fallback && parameter.name == *name)
    }) else {
        return actual == expected;
    };
    let crate::data::TypeParameterKind::Const { type_reference } = parameter.kind else {
        return false;
    };
    let FixedArrayLength::Literal(value) = actual else {
        return allow_name_fallback && actual == expected;
    };
    let Ok(value) = i128::try_from(*value) else {
        return false;
    };
    let Some(primitive) = program.type_reference_table.primitive_type(type_reference) else {
        return false;
    };
    bind_operator_const(
        const_bindings,
        parameter.symbol,
        CanonicalConstIdentity::integer(primitive.name(), value),
    )
}

fn closed_const_identity_from_type_reference(
    program: &TypedTrees,
    actual: TypeReferenceHandle,
    declared_carrier: TypeReferenceHandle,
) -> Option<CanonicalConstIdentity> {
    let TypeReferenceNode::Named { name, .. } = program.type_reference_table.type_reference(actual)
    else {
        return None;
    };
    if let Some(value) = CanonicalConstValue::from_atom(name.as_str()) {
        return Some(value.identity());
    }
    let value = name.as_str().parse::<i128>().ok()?;
    let primitive = program
        .type_reference_table
        .primitive_type(declared_carrier)?;
    Some(CanonicalConstIdentity::integer(primitive.name(), value))
}

fn bind_operator_const(
    bindings: &mut Vec<OperatorConstBinding>,
    symbol: SymbolHandle,
    value: CanonicalConstIdentity,
) -> bool {
    if let Some(existing) = bindings.iter().find(|binding| binding.symbol == symbol) {
        return existing.value == value;
    }
    bindings.push(OperatorConstBinding { symbol, value });
    true
}

fn nominal_type_identity_matches(
    actual_symbol: SymbolHandle,
    actual_name: &str,
    expected_symbol: SymbolHandle,
    expected_name: &str,
    allow_name_fallback: bool,
) -> bool {
    if allow_name_fallback {
        return (actual_symbol.is_valid() && actual_symbol == expected_symbol)
            || actual_name == expected_name;
    }
    if actual_symbol.is_valid() || expected_symbol.is_valid() {
        return actual_symbol.is_valid()
            && expected_symbol.is_valid()
            && actual_symbol == expected_symbol;
    }
    matches!(
        (
            crate::types::PrimitiveType::from_name(actual_name),
            crate::types::PrimitiveType::from_name(expected_name),
        ),
        (Some(actual), Some(expected)) if actual == expected
    )
}

fn expected_bindable_symbol(
    program: &TypedTrees,
    expected: TypeReferenceHandle,
    bindable_owner: Option<SymbolHandle>,
    type_parameters: &[TypeParameter],
    allow_name_fallback: bool,
) -> Option<SymbolHandle> {
    let (symbol, _) = match program.type_reference_table.type_reference(expected) {
        TypeReferenceNode::Named { symbol, name }
        | TypeReferenceNode::Generic {
            base_symbol: symbol,
            base_name: name,
            ..
        } => (*symbol, name),
        _ => return None,
    };
    if symbol.is_valid() && Some(symbol) == bindable_owner {
        return Some(symbol);
    }
    expected_type_parameter(program, expected, type_parameters, allow_name_fallback)
        .map(|parameter| parameter.symbol)
}

fn expected_type_parameter<'a>(
    program: &TypedTrees,
    expected: TypeReferenceHandle,
    type_parameters: &'a [TypeParameter],
    allow_name_fallback: bool,
) -> Option<&'a TypeParameter> {
    match program.type_reference_table.type_reference(expected) {
        TypeReferenceNode::Named { symbol, name }
        | TypeReferenceNode::Generic {
            base_symbol: symbol,
            base_name: name,
            ..
        } => type_parameters.iter().find(|parameter| {
            (symbol.is_valid() && parameter.symbol == *symbol)
                || (allow_name_fallback && parameter.name == *name)
        }),
        TypeReferenceNode::Reference { .. }
        | TypeReferenceNode::Constrained { .. }
        | TypeReferenceNode::FixedArray { .. }
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::Unit => None,
    }
}

/// All candidate indices carrying `spelling`, regardless of operand type. Useful
/// when a site only knows its receiver shape (e.g. "this is a slice") rather
/// than a fully normalized operand key.
pub fn candidates_for_spelling(
    operators: &[OperatorDefinition],
    spelling: OperatorSpelling,
) -> Vec<usize> {
    operators
        .iter()
        .enumerate()
        .filter(|(_, operator)| operator.spelling == Some(spelling))
        .map(|(index, _)| index)
        .collect()
}

/// The browsable path of the boundary operator governing a spelling (e.g.
/// `Slice::range`), taken from the `requires` contract owner of the first
/// spelled candidate that carries one. Failed subslice/index bounds
/// diagnostics name this path together with the spelling so the user can
/// look up the operator declaration and read the contract that sourced the
/// obligation.
///
/// Returns `None` when no spelled candidate carries a `requires` contract or
/// the carrying operator has no path members to name.
pub fn operator_contract_path(
    program: &TypedTrees,
    operators: &[OperatorDefinition],
    spelling: OperatorSpelling,
) -> Option<String> {
    operators
        .iter()
        .filter(|operator| operator.spelling == Some(spelling))
        .find(|operator| {
            program
                .operator_contracts(operator)
                .iter()
                .any(|contract| contract.kind == crate::signature::SignatureContractKind::Requires)
        })
        .and_then(|operator| {
            let path = program
                .operator_path_members(operator.name)
                .iter()
                .map(|member| member.as_str().to_owned())
                .collect::<Vec<_>>()
                .join("::");
            (!path.is_empty()).then_some(path)
        })
}

/// The `requires` clauses for a spelling, rendered as readable bound
/// obligations. The clause text is keyed on the spelling so a failed bound
/// reports the precise obligation (e.g.
/// `requires start <= end && end <= items.len` for `[..]`). Returns an empty
/// vector when no spelled candidate carries a `requires` contract, signalling
/// the caller that the obligation is not operator-sourced.
pub fn operator_requires_clauses(
    program: &TypedTrees,
    operators: &[OperatorDefinition],
    spelling: OperatorSpelling,
) -> Vec<String> {
    let has_requires = operators
        .iter()
        .filter(|operator| operator.spelling == Some(spelling))
        .any(|operator| {
            program
                .operator_contracts(operator)
                .iter()
                .any(|contract| contract.kind == crate::signature::SignatureContractKind::Requires)
        });
    if !has_requires {
        return Vec::new();
    }

    match spelling {
        OperatorSpelling::Index => vec!["index < items.len".to_owned()],
        OperatorSpelling::Range => vec!["start <= end".to_owned(), "end <= items.len".to_owned()],
        _ => Vec::new(),
    }
}

/// The canonical operand-type signature for an operator: its parameter types
/// normalized over the operator's own type parameters. The operator name and
/// return type are deliberately excluded — only operand types discriminate
/// within a spelling. Shared so dispatch and ambiguity validation agree.
pub fn operator_operand_signature(program: &TypedTrees, operator: &OperatorDefinition) -> String {
    let mut normalizer = TypeParameterNormalizer::new(
        program
            .operator_type_parameters(operator)
            .iter()
            .map(|parameter| parameter.symbol)
            .collect(),
    );
    let parameters = program.operator_parameters(operator);
    for parameter in normalized_operand_parameters(parameters) {
        collect_type_parameter_occurrences(program, parameter.type_reference, &mut normalizer);
    }
    let binders = normalizer.bindings();
    normalized_operand_parameters(parameters)
        .map(|parameter| {
            program
                .normalized_type_identity_with_binders(parameter.type_reference, &binders)
                .into_string()
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Canonical operand-type signature for a trait-owned operator requirement.
/// Trait and requirement binders share one alpha-normalized telescope. An
/// attached receiver is always position zero; otherwise the first explicit
/// parameter is position zero, matching expression operand order.
pub fn trait_operator_operand_signature(
    program: &TypedTrees,
    trait_definition: &crate::trait_definition::TraitDefinition,
    requirement: &crate::signature::StateSignature,
) -> String {
    let mut normalizer = TypeParameterNormalizer::new(
        program
            .trait_type_parameters(trait_definition)
            .iter()
            .chain(program.state_signature_type_parameters(requirement))
            .map(|parameter| parameter.symbol)
            .collect(),
    );
    let parameters = program.state_signature_parameters(requirement);
    for parameter in normalized_operand_parameters(parameters) {
        collect_type_parameter_occurrences(program, parameter.type_reference, &mut normalizer);
    }
    let binders = normalizer.bindings();
    normalized_operand_parameters(parameters)
        .map(|parameter| {
            program
                .normalized_type_identity_with_binders(parameter.type_reference, &binders)
                .into_string()
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn normalized_operand_parameters(
    parameters: &[crate::signature::StateParameter],
) -> impl Iterator<Item = &crate::signature::StateParameter> {
    parameters
        .iter()
        .filter(|parameter| parameter.is_self)
        .chain(parameters.iter().filter(|parameter| !parameter.is_self))
}

fn collect_type_parameter_occurrences(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    normalizer: &mut TypeParameterNormalizer,
) {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            collect_type_parameter_occurrences(program, *referee, normalizer);
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            collect_type_parameter_occurrences(program, *base_type, normalizer);
        }
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => {
            collect_type_parameter_occurrences(program, *element_type, normalizer);
        }
        TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } => {
            normalizer.canonical_index(*base_symbol);
            for argument in program
                .type_reference_table
                .type_reference_handles(*arguments)
            {
                collect_type_parameter_occurrences(program, *argument, normalizer);
            }
        }
        TypeReferenceNode::Named { symbol, .. }
        | TypeReferenceNode::DynamicTrait { symbol, .. } => {
            normalizer.canonical_index(*symbol);
        }
        TypeReferenceNode::ConstExpression(_) => {}
        TypeReferenceNode::Unit => {}
    }
}

struct TypeParameterNormalizer {
    declared: Vec<SymbolHandle>,
    canonical: Vec<(SymbolHandle, usize)>,
}

impl TypeParameterNormalizer {
    fn new(declared: Vec<SymbolHandle>) -> Self {
        Self {
            declared,
            canonical: Vec::new(),
        }
    }

    fn canonical_index(&mut self, symbol: SymbolHandle) -> Option<usize> {
        if !self.declared.contains(&symbol) {
            return None;
        }
        if let Some((_, index)) = self
            .canonical
            .iter()
            .find(|(candidate, _)| *candidate == symbol)
        {
            return Some(*index);
        }
        let index = self.canonical.len();
        self.canonical.push((symbol, index));
        Some(index)
    }

    fn bindings(&self) -> Vec<(SymbolHandle, String)> {
        self.canonical
            .iter()
            .map(|(symbol, index)| (*symbol, format!("${index}")))
            .collect()
    }
}
