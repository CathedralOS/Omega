//! Structural, declaration-derived lifetime frontiers shared by result and
//! input selection. Incomplete frontiers never establish a partial contract.
//! Fixed arrays share one element-type traversal; only actual reference leaves
//! are expanded into per-element paths. View-free storage therefore costs work
//! proportional to its type structure, not its physical element count.
use super::{SymbolHandle, TypeReferenceHandle, TypeReferenceNode};
use crate::borrow::BorrowOwnerSegment;
use crate::borrow::view_link::data_definition;

pub(super) struct CarriedLifetime {
    pub(super) owner_path: Vec<BorrowOwnerSegment>,
    pub(super) lifetime: Option<String>,
    pub(super) access: language_semantics::ReferenceAccess,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DeclarationLifetimeFrontier {
    Complete,
    TemplateDependent,
    Incomplete,
}

#[derive(Default)]
struct TemplateFrontier<'a> {
    parameters: &'a [SymbolHandle],
    dependent: bool,
}

/// Each generic telescope binds simultaneously. Its actual operands belong
/// to the outer scope, even when another application uses the same declaration.
#[derive(Clone, Copy, Default)]
struct TypeScope<'scope> {
    bindings: &'scope [(SymbolHandle, TypeReferenceHandle)],
    starts: &'scope [usize],
    outer: Option<&'scope TypeScope<'scope>>,
    argument_lifetimes: &'scope [(String, String)],
}

struct DataVisit {
    reference: TypeReferenceHandle,
    cyclic: bool,
}

/// A declaration may describe a type parameter without knowing its carried
/// references. This is not a complete frontier and never supplies a loan.
pub(crate) fn declaration_lifetime_frontier(
    program: &dyn ::validation::CheckingInput,
    reference: TypeReferenceHandle,
    parameters: &[SymbolHandle],
) -> DeclarationLifetimeFrontier {
    let mut template = TemplateFrontier {
        parameters,
        dependent: false,
    };
    let complete = collect_type(
        program,
        reference,
        &[],
        TypeScope::default(),
        &[],
        &mut Vec::new(),
        &mut Vec::new(),
        false,
        &mut template,
    );
    if !complete {
        DeclarationLifetimeFrontier::Incomplete
    } else if template.dependent {
        DeclarationLifetimeFrontier::TemplateDependent
    } else {
        DeclarationLifetimeFrontier::Complete
    }
}

pub(super) fn carried_lifetimes(
    program: &dyn ::validation::CheckingInput,
    reference: TypeReferenceHandle,
) -> Option<Vec<CarriedLifetime>> {
    let mut output = Vec::new();
    collect_type(
        program,
        reference,
        &[],
        TypeScope::default(),
        &[],
        &mut Vec::new(),
        &mut output,
        false,
        &mut TemplateFrontier::default(),
    )
    .then_some(output)
}

/// Every substitution maps to a valid, registered type reference.
fn substitutions_are_registered(
    program: &dyn ::validation::CheckingInput,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
) -> bool {
    !substitutions.iter().any(|(_, reference)| {
        reference.is_valid()
            && !program
                .type_reference_table
                .contains_type_reference(*reference)
    })
}

/// Exact call substitutions may prove that a formerly dependent result has
/// no carried views. A closed nonempty frontier still requires per-call loan
/// attribution and cannot use this deliberately narrower result.
pub(crate) fn substituted_result_is_view_free(
    program: &dyn ::validation::CheckingInput,
    reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
) -> bool {
    if !substitutions_are_registered(program, substitutions) {
        return false;
    }
    let mut output = Vec::new();
    collect_type(
        program,
        reference,
        &[],
        TypeScope {
            bindings: substitutions,
            starts: &[0],
            ..TypeScope::default()
        },
        &[],
        &mut Vec::new(),
        &mut output,
        false,
        &mut TemplateFrontier::default(),
    ) && output.is_empty()
}

/// Exact call substitutions may instead close a formerly dependent result
/// frontier completely. A closed view-bearing frontier is not view-free:
/// every instantiated leaf still borrows an exact caller input, which is the
/// caller-side loan relation `resolve_substituted_view_return_source`
/// carries. An unresolved structure is never a complete frontier.
pub(crate) fn substituted_result_frontier_is_complete(
    program: &dyn ::validation::CheckingInput,
    reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    scope_starts: &[usize],
) -> bool {
    if !substitutions_are_registered(program, substitutions) {
        return false;
    }
    collect_type(
        program,
        reference,
        &[],
        TypeScope {
            bindings: substitutions,
            starts: scope_starts,
            ..TypeScope::default()
        },
        &[],
        &mut Vec::new(),
        &mut Vec::new(),
        false,
        &mut TemplateFrontier::default(),
    )
}

/// The instantiated structural frontier of one substituted type: the same
/// leaf walk as [`carried_lifetimes`], with each declaration type parameter
/// resolved through the call's own closed bindings. `None` means the
/// instantiated frontier cannot be enumerated completely.
pub(super) fn substituted_carried_lifetimes(
    program: &dyn ::validation::CheckingInput,
    reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    scope_starts: &[usize],
) -> Option<Vec<CarriedLifetime>> {
    let mut output = Vec::new();
    collect_type(
        program,
        reference,
        &[],
        TypeScope {
            bindings: substitutions,
            starts: scope_starts,
            ..TypeScope::default()
        },
        &[],
        &mut Vec::new(),
        &mut output,
        false,
        &mut TemplateFrontier::default(),
    )
    .then_some(output)
}

/// A nongeneric recursive result can retain one whole input loan under ordinary
/// elision. Repeated declarations need no repeated path expansion here, because
/// no projected frontier is published. Explicit lifetimes or substitutions
/// require the ordinary complete contract and cannot use this fallback.
pub(super) fn whole_elided_result_accesses(
    program: &dyn ::validation::CheckingInput,
    reference: TypeReferenceHandle,
) -> Option<Vec<language_semantics::ReferenceAccess>> {
    let mut output = Vec::new();
    collect_type(
        program,
        reference,
        &[],
        TypeScope::default(),
        &[],
        &mut Vec::new(),
        &mut output,
        true,
        &mut TemplateFrontier::default(),
    )
    .then(|| output.into_iter().map(|leaf| leaf.access).collect())
}

fn collect_type(
    program: &dyn ::validation::CheckingInput,
    reference: TypeReferenceHandle,
    lifetimes: &[(String, String)],
    types: TypeScope<'_>,
    owner_path: &[BorrowOwnerSegment],
    visiting: &mut Vec<DataVisit>,
    output: &mut Vec<CarriedLifetime>,
    whole_elided_contract: bool,
    template: &mut TemplateFrontier<'_>,
) -> bool {
    // Zero remains the ordinary absence/Unit convention. A stale nonzero
    // generational handle resolving to the same dummy node is not a type.
    if reference.is_valid()
        && !program
            .type_reference_table
            .contains_type_reference(reference)
    {
        return false;
    }
    if program.primitive_type_reference(reference).is_some() {
        return true;
    }
    match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Reference {
            access, lifetime, ..
        } => {
            if whole_elided_contract && lifetime.is_some() {
                return false;
            }
            output.push(CarriedLifetime {
                owner_path: owner_path.to_vec(),
                lifetime: lifetime.as_ref().map(|lifetime| {
                    lifetimes
                        .iter()
                        .rev()
                        .find_map(|(parameter, argument)| {
                            (parameter == lifetime.as_str()).then_some(argument.clone())
                        })
                        .unwrap_or_else(|| lifetime.as_str().to_owned())
                }),
                access: *access,
            });
            true
        }
        TypeReferenceNode::Constrained { base_type, .. } => collect_type(
            program,
            *base_type,
            lifetimes,
            types,
            owner_path,
            visiting,
            output,
            whole_elided_contract,
            template,
        ),
        TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => {
            if matches!(length, typed_trees::types::FixedArrayLength::Literal(0)) {
                return true;
            }
            // The path does not participate in frontier admission. Check the
            // element once in the same substitution/recursion context, then
            // prefix its complete relative leaves. Incompleteness must not
            // become an empty (view-free) frontier.
            let mut element_frontier = Vec::new();
            if !collect_type(
                program,
                *element_type,
                lifetimes,
                types,
                &[],
                visiting,
                &mut element_frontier,
                whole_elided_contract,
                template,
            ) {
                return false;
            }
            if element_frontier.is_empty() {
                // A symbolic extent does not hide references when the element
                // frontier is empty. Only view-bearing arrays require an exact
                // count to publish their per-element owner paths.
                return true;
            }
            let typed_trees::types::FixedArrayLength::Literal(length) = length else {
                return false;
            };
            for element_index in 0..*length {
                for leaf in &element_frontier {
                    let mut path = owner_path.to_vec();
                    path.push(BorrowOwnerSegment::FixedIndex(element_index));
                    path.extend_from_slice(&leaf.owner_path);
                    output.push(CarriedLifetime {
                        owner_path: path,
                        lifetime: leaf.lifetime.clone(),
                        access: leaf.access,
                    });
                }
            }
            true
        }
        TypeReferenceNode::Generic {
            base_symbol,
            lifetime_arguments,
            arguments,
            ..
        } => {
            if whole_elided_contract {
                return false;
            }
            // Preparation owns closed field substitution. Read its exact view
            // without replaying the declaration telescope or rebinding actual
            // operands under another application of the same nominal head.
            if let Some(definition) = program.data_application_definition(reference) {
                return collect_data(
                    program,
                    reference,
                    definition,
                    lifetimes,
                    TypeScope::default(),
                    owner_path,
                    visiting,
                    output,
                    whole_elided_contract,
                    template,
                );
            }
            let Some(definition) = data_definition(program, *base_symbol) else {
                return false;
            };
            let arguments = program
                .type_reference_table
                .type_reference_handles(*arguments);
            let parameters = program.data_type_parameters(definition);
            if definition.lifetime_parameters.len() != lifetime_arguments.len()
                || parameters.len() != arguments.len()
            {
                return false;
            }
            let nested_lifetimes = definition
                .lifetime_parameters
                .iter()
                .zip(lifetime_arguments)
                .map(|(parameter, argument)| {
                    let argument = lifetimes
                        .iter()
                        .rev()
                        .find_map(|(outer, concrete)| {
                            (outer == argument.as_str()).then_some(concrete.clone())
                        })
                        .unwrap_or_else(|| argument.as_str().to_owned());
                    (parameter.as_str().to_owned(), argument)
                })
                .collect::<Vec<_>>();
            let nested_types = parameters
                .iter()
                .zip(arguments)
                .map(|(parameter, argument)| (parameter.symbol, *argument))
                .collect::<Vec<_>>();
            collect_data(
                program,
                reference,
                definition,
                &nested_lifetimes,
                TypeScope {
                    bindings: &nested_types,
                    starts: &[0],
                    outer: Some(&types),
                    argument_lifetimes: lifetimes,
                },
                owner_path,
                visiting,
                output,
                whole_elided_contract,
                template,
            )
        }
        TypeReferenceNode::Named { symbol, .. } => {
            let mut scope = Some(&types);
            while let Some(frame) = scope {
                if let Some((binding_ordinal, (_, concrete))) = frame
                    .bindings
                    .iter()
                    .enumerate()
                    .rev()
                    .find(|(_, (parameter, _))| parameter == symbol)
                {
                    // An unresolved self-substitution is not a finite concrete type.
                    if *concrete == reference && template.parameters.contains(symbol) {
                        template.dependent = true;
                        return true;
                    }
                    if *concrete == reference {
                        return false;
                    }
                    let Some((scope_ordinal, start)) = frame
                        .starts
                        .iter()
                        .enumerate()
                        .rev()
                        .find(|(_, start)| **start <= binding_ordinal)
                    else {
                        return false;
                    };
                    let outer = if *start == 0 {
                        frame.outer.copied().unwrap_or_default()
                    } else {
                        TypeScope {
                            bindings: &frame.bindings[..*start],
                            starts: &frame.starts[..scope_ordinal],
                            ..*frame
                        }
                    };
                    return collect_type(
                        program,
                        *concrete,
                        frame.argument_lifetimes,
                        outer,
                        owner_path,
                        visiting,
                        output,
                        whole_elided_contract,
                        template,
                    );
                }
                scope = frame.outer;
            }
            let Some(definition) = data_definition(program, *symbol) else {
                if template.parameters.contains(symbol) {
                    template.dependent = true;
                    return true;
                }
                return false;
            };
            if !definition.type_parameters.is_empty()
                || (whole_elided_contract && !definition.lifetime_parameters.is_empty())
            {
                return false;
            }
            collect_data(
                program,
                reference,
                definition,
                lifetimes,
                types,
                owner_path,
                visiting,
                output,
                whole_elided_contract,
                template,
            )
        }
        TypeReferenceNode::Unit => true,
        TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::DynamicTrait { .. } => false,
    }
}

fn collect_data(
    program: &dyn ::validation::CheckingInput,
    reference: TypeReferenceHandle,
    definition: &typed_trees::data::DataDefinition,
    lifetimes: &[(String, String)],
    types: TypeScope<'_>,
    owner_path: &[BorrowOwnerSegment],
    visiting: &mut Vec<DataVisit>,
    output: &mut Vec<CarriedLifetime>,
    whole_elided_contract: bool,
    template: &mut TemplateFrontier<'_>,
) -> bool {
    if let Some(ancestor) = visiting
        .iter()
        .position(|visit| visit.reference == reference)
    {
        // A cycle with no reference leaves is still borrow-free (for example
        // Nat). A cycle carrying views has no finite projected frontier. Mark
        // the participating frames, then decide only after all members have
        // contributed their leaves; member order must not decide completeness.
        for visit in &mut visiting[ancestor..] {
            visit.cyclic = true;
        }
        return true;
    }
    let first_leaf = output.len();
    visiting.push(DataVisit {
        reference,
        cyclic: false,
    });
    let complete = program
        .data_member_fields(program.data_members(definition))
        .all(|field| {
            let mut path = owner_path.to_vec();
            if let Some(variant) = facts::payload_variant_for_field(program, field.symbol) {
                path.push(BorrowOwnerSegment::Case(variant));
            }
            path.push(BorrowOwnerSegment::Field(field.symbol));
            collect_type(
                program,
                field.type_reference,
                lifetimes,
                types,
                &path,
                visiting,
                output,
                whole_elided_contract,
                template,
            )
        });
    let cyclic = visiting.pop().is_some_and(|visit| visit.cyclic);
    complete && (whole_elided_contract || !cyclic || output.len() == first_leaf)
}
