//! Elaborated field views of data applications, not new nominal declarations.
//!
//! A caller's `Envelope<Selected<'storage>>` is closed for storage even though
//! syntax-level synthesis cannot select its lexical regions. Retain the exact
//! application and substitute its fields here, while typed arenas are owned.
//! Checking reads those fields without rebuilding types or borrowing one use's
//! region spelling from a layout-equivalent use. Source admission still checks
//! the original declaration and every authored application; this derivation
//! grants no bounds, loan, constructor or predicate authority.
//!
//! Open applications also need their own field telescope: `Buffer<M>::items`
//! has extent `M`, not the declaration's unrelated `N`. Retain that symbolic
//! view before freezing, separately from whether its storage is closed. A
//! checker reading the raw declaration would reject valid caller bounds and
//! accidentally accept bounds against the declaration's binder.
//!
//! The completed typed program owns the finite application roster, including
//! inferred projection types and selected machine instances. Substituting a
//! by-value field requires its own closed view: construction and disposal
//! consume that field's actual member types. Follow these storage edges, not
//! every newly generated type row. References stop expansion. A repeated
//! declaration can continue only by descending into an existing actual type
//! argument, so `Wrapper<Wrapper<T>>` closes without admitting a growing
//! recursive field family. References are expanded only for the finite member
//! projections actually present in source, child before parent. Reading
//! `node.next.value` prepares that reached referent, not every future
//! `Nest<[T; 2]>` in a growing recursive family.

use arena::HandleSpan;
use symbols::SymbolHandle;
use symbols::SymbolKind;
use typed_trees::TypedTrees;
use typed_trees::data::DataMember;
use typed_trees::name::Identifier;
use typed_trees::types::FixedArrayLength;
use typed_trees::types::TypeConstraintNode;
use typed_trees::types::TypeReferenceHandle;
use typed_trees::types::TypeReferenceNode;

pub(super) fn elaborate(program: &mut TypedTrees) -> Result<(), Vec<diagnostics::Diagnostic>> {
    let type_start = program.type_reference_table.type_reference_count();
    let applications = program
        .type_reference_table
        .type_reference_rows()
        .map(|(reference, _)| reference)
        .collect::<Vec<_>>();
    for reference in applications {
        elaborate_value_fields(program, reference, &mut Vec::new());
    }
    elaborate_projected_fields(program);
    finalize_produced_types(program, type_start)
}

/// Signature instantiation can append applications after source projections
/// have been resolved. Prepare those exact occurrences before freezing, so
/// lifetime and storage queries read one field telescope. A layout-equivalent
/// closed instance cannot supply another occurrence's lexical regions.
pub(super) fn elaborate_new_types(
    program: &mut TypedTrees,
    first_type: usize,
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    let type_start = program.type_reference_table.type_reference_count();
    let applications = program
        .type_reference_table
        .type_reference_rows()
        .filter(|(reference, _)| reference.arena_index() as usize > first_type)
        .map(|(reference, _)| reference)
        .collect::<Vec<_>>();
    for reference in applications {
        elaborate_value_fields(program, reference, &mut Vec::new());
    }
    finalize_produced_types(program, type_start)
}

/// Specialization reads member results while it derives call tuples. A
/// specialized clone's `items.tail` must already denote its receiver's
/// substituted field (`Seq<i32>`), not the declaration's unrelated `Seq<T>`
/// binder, or argument inference binds a callee's `T` to a foreign variable.
/// The same finite projection demand as the final elaboration, retained by
/// the producer as each fixed-point round appends cloned bodies.
pub(crate) fn elaborate_projected_receivers(
    program: &mut TypedTrees,
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    let type_start = program.type_reference_table.type_reference_count();
    elaborate_projected_fields(program);
    finalize_produced_types(program, type_start)
}

fn finalize_produced_types(
    program: &mut TypedTrees,
    type_start: usize,
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    let produced = program
        .type_reference_table
        .type_reference_rows()
        .filter(|(reference, _)| reference.arena_index() as usize > type_start)
        .map(|(reference, _)| reference)
        .collect::<Vec<_>>();
    crate::monomorphization::finalize_type_domain_instance_identities(program, produced)
        .map_err(|diagnostic| vec![diagnostic])
}

/// A reference is not a storage-expansion edge. A source member occurrence is
/// a finite demand for its receiver's fields, including symbolic applications.
/// Complete children first so the next receiver reads their applied types.
fn elaborate_projected_fields(program: &mut TypedTrees) {
    let mut projections = Vec::new();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            let mut expressions = Vec::new();
            for statement in program.statement_table.statements(state.statement_nodes) {
                validation::expression_traversal::collect_statement_expression_trees(
                    program,
                    statement,
                    &mut expressions,
                );
            }
            let mut visited = Vec::new();
            for expression in expressions {
                validation::expression_traversal::visit_post_order(
                    program,
                    expression,
                    &mut visited,
                    |_, node| {
                        if let typed_trees::expression::ExpressionNode::Member(member) = node {
                            projections.push((machine.symbol, state.symbol, member.receiver));
                        }
                    },
                );
            }
        }
    }
    for (machine_symbol, state_symbol, receiver) in projections {
        let Some(machine) = program.machine_by_symbol(machine_symbol) else {
            continue;
        };
        let Some(state) = program.state_by_symbol(state_symbol) else {
            continue;
        };
        let Some(reference) =
            validation::expression_result_type_reference(program, machine, state, receiver)
        else {
            continue;
        };
        let reference = program
            .type_reference_table
            .peeled_type_reference(reference);
        elaborate_value_fields(program, reference, &mut Vec::new());
    }
}

/// Elaborate the finite by-value member roster without following referent families.
/// This describes storage, not construction permission or layout admission;
/// recursive proof-only declarations still receive their source root views.
fn elaborate_value_fields(
    program: &mut TypedTrees,
    reference: TypeReferenceHandle,
    active: &mut Vec<(SymbolHandle, TypeReferenceHandle)>,
) {
    match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Reference { .. } | TypeReferenceNode::Slice { .. } => return,
        TypeReferenceNode::Constrained { base_type, .. } => {
            let base_type = *base_type;
            elaborate_value_fields(program, base_type, active);
            return;
        }
        TypeReferenceNode::FixedArray { element_type, .. } => {
            let element_type = *element_type;
            elaborate_value_fields(program, element_type, active);
            return;
        }
        _ => {}
    }
    let owner = match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Generic { base_symbol, .. } => *base_symbol,
        TypeReferenceNode::Named { symbol, .. } => *symbol,
        _ => return,
    };
    if let Some((_, parent)) = active.iter().rev().find(|(ancestor, _)| *ancestor == owner) {
        let TypeReferenceNode::Generic { arguments, .. } =
            program.type_reference_table.type_reference(*parent)
        else {
            return;
        };
        // A parameter field retains its actual argument handle. This is a
        // strict descent in the finite source type, unlike a declaration field
        // that manufactures another application of its own nominal owner.
        let arguments = program
            .type_reference_table
            .type_reference_handles(*arguments);
        if !contains_actual_type(program, arguments, reference) {
            return;
        }
    }
    elaborate_application(program, reference);
    let Some(data) = program
        .data_application_definition(reference)
        .or_else(|| program.closed_data_definition(reference))
    else {
        return;
    };
    let mut children = Vec::new();
    for member in program.data_members(data) {
        match member {
            DataMember::Field(field) => children.push(field.type_reference),
            DataMember::Variant(case) => children.extend(
                program
                    .data_payload_fields(case)
                    .iter()
                    .map(|field| field.type_reference),
            ),
        }
    }
    active.push((owner, reference));
    for child in children {
        elaborate_value_fields(program, child, active);
    }
    active.pop();
}

/// Walk authored type structure, never a nominal declaration's member graph.
/// Array and constraint shells still make strict argument descent finite.
fn contains_actual_type(
    program: &TypedTrees,
    arguments: &[TypeReferenceHandle],
    selected: TypeReferenceHandle,
) -> bool {
    let mut pending = arguments.to_vec();
    let mut visited = Vec::new();
    while let Some(reference) = pending.pop() {
        if reference == selected {
            return true;
        }
        if visited.contains(&reference) {
            continue;
        }
        visited.push(reference);
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Generic { arguments, .. } => pending.extend(
                program
                    .type_reference_table
                    .type_reference_handles(*arguments)
                    .iter()
                    .copied(),
            ),
            TypeReferenceNode::Constrained { base_type, .. } => pending.push(*base_type),
            TypeReferenceNode::FixedArray { element_type, .. } => pending.push(*element_type),
            _ => {}
        }
    }
    false
}

fn elaborate_application(program: &mut TypedTrees, reference: TypeReferenceHandle) {
    if program.data_application_definition(reference).is_some() {
        return;
    }
    let TypeReferenceNode::Generic {
        base_symbol,
        lifetime_arguments,
        arguments,
        ..
    } = program
        .type_reference_table
        .type_reference(reference)
        .clone()
    else {
        return;
    };
    let Some(template) = program
        .unique_data_definition_by_symbol(base_symbol)
        .cloned()
    else {
        return;
    };
    let parameters = program.data_type_parameters(&template).to_vec();
    let arguments = program
        .type_reference_table
        .type_reference_handles(arguments)
        .to_vec();
    if parameters.len() != arguments.len()
        || template.lifetime_parameters.len() != lifetime_arguments.len()
    {
        return;
    }
    // A runtime-capable index need not be static to have fixed storage.
    // Its substituted fields still prove representation closure below; this
    // never admits a dynamic inline extent or erases application identity.
    let mut is_closed = parameters
        .iter()
        .zip(&arguments)
        .all(|(parameter, argument)| {
            matches!(
                parameter.kind,
                typed_trees::data::TypeParameterKind::Value { .. }
            ) || closed_type(program, *argument)
        });
    // Const evaluation can already have retained a declaration with exact
    // extents and constructor facts. Runtime carrier equality alone is not
    // enough to reuse its fields: it erases regions inside type arguments.
    // Compare application identities without inventing a lexical region owner.
    // If that custody is unavailable, elaborate this occurrence's own telescope
    // below rather than borrowing a layout-equivalent use's lifetime relation.
    if let Some(instance) = program.closed_data_definition(reference).cloned()
        && let Some(origin) = instance.generic_instance
        && let Some(identity) = typed_trees::type_identity::applied_type_identity(
            program,
            reference,
            SymbolHandle::invalid(),
            &[],
        )
        && typed_trees::type_identity::applied_type_identity(
            program,
            origin,
            SymbolHandle::invalid(),
            &[],
        )
        .is_some_and(|retained| retained == identity)
    {
        program.retain_data_application_definition(reference, instance, true);
        return;
    }
    let substitutions = parameters
        .iter()
        .zip(arguments)
        .map(|(parameter, argument)| (parameter.symbol, argument))
        .collect::<Vec<_>>();
    let lifetimes = template
        .lifetime_parameters
        .iter()
        .cloned()
        .zip(lifetime_arguments)
        .collect::<Vec<_>>();
    let members = program.data_members(&template).to_vec();
    let mut instantiated = Vec::with_capacity(members.len());
    for member in members {
        let member = match member {
            DataMember::Field(mut field) => {
                field.type_reference =
                    instantiate_type(program, field.type_reference, &substitutions, &lifetimes);
                is_closed &= closed_type(program, field.type_reference);
                DataMember::Field(field)
            }
            DataMember::Variant(mut variant) => {
                let mut fields = program.data_payload_fields(&variant).to_vec();
                for field in &mut fields {
                    field.type_reference =
                        instantiate_type(program, field.type_reference, &substitutions, &lifetimes);
                    is_closed &= closed_type(program, field.type_reference);
                }
                variant.payload = program.data_payload_fields.insert_many(fields);
                DataMember::Variant(variant)
            }
        };
        instantiated.push(member);
    }
    let mut application = template;
    application.members = program.data_members.insert_many(instantiated);
    application.type_parameters = HandleSpan::empty();
    application.lifetime_parameters.clear();
    application.generic_instance = Some(reference);
    program.retain_data_application_definition(reference, application, is_closed);
}

/// This is closure, not admission: unknown/open operands retain no closed storage.
fn closed_type(program: &TypedTrees, reference: TypeReferenceHandle) -> bool {
    let mut pending = vec![reference];
    while let Some(reference) = pending.pop() {
        if !program
            .type_reference_table
            .contains_type_reference(reference)
        {
            return false;
        }
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Named { symbol, .. } => {
                if symbol.is_valid()
                    && matches!(
                        program.symbols.get(*symbol).kind,
                        SymbolKind::TypeParameter | SymbolKind::Parameter | SymbolKind::Local
                    )
                {
                    return false;
                }
                if let Some(data) = program.unique_data_definition_by_symbol(*symbol)
                    && !program.data_type_parameters(data).is_empty()
                {
                    return false;
                }
                if let Some(declaration) = program.trait_definition_by_symbol(*symbol)
                    && !program.trait_type_parameters(declaration).is_empty()
                {
                    return false;
                }
            }
            TypeReferenceNode::Reference { referee, .. } => pending.push(*referee),
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                pending.push(*base_type);
                for constraint in program.type_reference_table.constraints(*constraints) {
                    match constraint {
                        TypeConstraintNode::Domain(domain) => {
                            pending.extend(domain.arguments.iter().copied())
                        }
                        TypeConstraintNode::Range {
                            minimum, maximum, ..
                        } if !closed_expression(program, *minimum)
                            || !closed_expression(program, *maximum) =>
                        {
                            return false;
                        }
                        _ => {}
                    }
                }
            }
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => {
                if !matches!(length, FixedArrayLength::Literal(_)) {
                    return false;
                }
                pending.push(*element_type);
            }
            TypeReferenceNode::Slice { element_type } => pending.push(*element_type),
            TypeReferenceNode::Generic {
                base_symbol,
                lifetime_arguments,
                arguments,
                ..
            } => {
                // A nominal application's static operands can themselves be
                // applied traits. They describe its closed contract, not a
                // second stored data value. Closure must follow either exact
                // declaration telescope rather than treating every Generic
                // node as a data constructor.
                let telescope = program
                    .unique_data_definition_by_symbol(*base_symbol)
                    .map(|data| {
                        (
                            program.data_type_parameters(data).len(),
                            data.lifetime_parameters.len(),
                        )
                    })
                    .or_else(|| {
                        program
                            .trait_definition_by_symbol(*base_symbol)
                            .map(|declaration| {
                                (
                                    program.trait_type_parameters(declaration).len(),
                                    declaration.lifetime_parameters.len(),
                                )
                            })
                    });
                let Some((parameter_count, lifetime_count)) = telescope else {
                    return false;
                };
                let arguments = program
                    .type_reference_table
                    .type_reference_handles(*arguments);
                if parameter_count != arguments.len() || lifetime_count != lifetime_arguments.len()
                {
                    return false;
                }
                pending.extend(arguments.iter().copied());
            }
            TypeReferenceNode::ConstExpression(_) | TypeReferenceNode::DynamicTrait { .. } => {
                return false;
            }
            TypeReferenceNode::Unit => {}
        }
    }
    true
}

fn closed_expression(
    program: &TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
) -> bool {
    let mut expressions = Vec::new();
    validation::collect_expression_tree(program, expression, &mut expressions);
    !expressions.iter().any(|expression| {
        matches!(program.expression_table.expression(*expression),
            typed_trees::expression::ExpressionNode::Name(path)
                if path.symbol.is_valid() && matches!(program.symbols.get(path.symbol).kind,
                    SymbolKind::TypeParameter | SymbolKind::Parameter | SymbolKind::Local))
    })
}

fn instantiate_type(
    program: &mut TypedTrees,
    reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    lifetimes: &[(Identifier, Identifier)],
) -> TypeReferenceHandle {
    let original = program
        .type_reference_table
        .type_reference(reference)
        .clone();
    let instantiated = match original.clone() {
        TypeReferenceNode::Named { symbol, .. } => {
            return substitutions
                .iter()
                .find_map(|(parameter, actual)| (*parameter == symbol).then_some(*actual))
                .unwrap_or(reference);
        }
        TypeReferenceNode::Reference {
            referee,
            access,
            lifetime,
        } => TypeReferenceNode::Reference {
            referee: instantiate_type(program, referee, substitutions, lifetimes),
            access,
            lifetime: lifetime.map(|lifetime| instantiate_lifetime(lifetime, lifetimes)),
        },
        TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            lifetime_arguments,
            arguments,
        } => {
            let arguments = program
                .type_reference_table
                .type_reference_handles(arguments)
                .to_vec();
            let arguments = arguments
                .into_iter()
                .map(|argument| instantiate_type(program, argument, substitutions, lifetimes))
                .collect::<Vec<_>>();
            let lifetime_arguments = lifetime_arguments
                .into_iter()
                .map(|lifetime| instantiate_lifetime(lifetime, lifetimes))
                .collect::<Vec<_>>();
            // Reuse exact child handles, not carrier identity. Besides avoiding
            // copies, this closes recursive data references without expanding
            // a fresh equivalent application at every field traversal.
            if let Some((existing, _)) =
                program
                    .type_reference_table
                    .type_reference_rows()
                    .find(|(_, node)| {
                        matches!(node, TypeReferenceNode::Generic {
                    base_symbol: candidate, lifetime_arguments: regions, arguments: children, ..
                } if *candidate == base_symbol && *regions == lifetime_arguments
                    && program.type_reference_table.type_reference_handles(*children) == arguments)
                    })
            {
                return existing;
            }
            TypeReferenceNode::Generic {
                base_symbol,
                base_name,
                lifetime_arguments,
                arguments: program
                    .type_reference_table
                    .insert_type_reference_handles(arguments),
            }
        }
        TypeReferenceNode::Slice { element_type } => TypeReferenceNode::Slice {
            element_type: instantiate_type(program, element_type, substitutions, lifetimes),
        },
        TypeReferenceNode::FixedArray {
            element_type,
            mut length,
        } => {
            if let FixedArrayLength::ConstParameter { symbol, .. } = &length
                && let Some((_, actual)) = substitutions
                    .iter()
                    .find(|(parameter, _)| parameter == symbol)
                && let TypeReferenceNode::Named { symbol, name } =
                    program.type_reference_table.type_reference(*actual)
            {
                if symbol.is_valid() {
                    length = FixedArrayLength::ConstParameter {
                        symbol: *symbol,
                        name: name.clone(),
                    };
                } else if let Ok(value) = name.as_str().parse::<usize>() {
                    length = FixedArrayLength::Literal(value);
                }
            }
            TypeReferenceNode::FixedArray {
                element_type: instantiate_type(program, element_type, substitutions, lifetimes),
                length,
            }
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            let base_type = instantiate_type(program, base_type, substitutions, lifetimes);
            let mut constraints = program
                .type_reference_table
                .constraints(constraints)
                .to_vec();
            for constraint in &mut constraints {
                if let TypeConstraintNode::Domain(domain) = constraint {
                    domain.arguments = domain
                        .arguments
                        .iter()
                        .map(|argument| {
                            instantiate_type(program, *argument, substitutions, lifetimes)
                        })
                        .collect();
                }
            }
            TypeReferenceNode::Constrained {
                base_type,
                constraints: program.type_reference_table.insert_constraints(constraints),
            }
        }
        TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Unit => return reference,
    };
    if instantiated == original {
        return reference;
    }
    let Ok(origins) = program
        .type_reference_table
        .constant_origins(reference)
        .map(<[_]>::to_vec)
    else {
        return TypeReferenceHandle::invalid();
    };
    let result = program.type_reference_table.insert(instantiated);
    if program
        .type_reference_table
        .attach_constant_origins(result, origins)
        .is_err()
    {
        return TypeReferenceHandle::invalid();
    }
    result
}

fn instantiate_lifetime(
    lifetime: Identifier,
    lifetimes: &[(Identifier, Identifier)],
) -> Identifier {
    lifetimes
        .iter()
        .find_map(|(parameter, actual)| (*parameter == lifetime).then(|| actual.clone()))
        .unwrap_or(lifetime)
}
