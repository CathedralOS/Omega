//! Type inference for the synthetic `let __hoist_N = <indexed read>;` temps and
//! borrowed membership subjects (`&<indexed place>`) that the
//! operand-hoisting normalization synthesizes (see the syntax -> symbol-resolved
//! `statement::hoist_operand_indexed_reads`). Those temps carry a `Unit`
//! declared type (the inference sentinel) because the language has no local type
//! inference and the element type is not knowable at the hoist site. Here, with
//! the resolved data-field types available, the temp's element type is derived
//! from its initializer, retaining reference access for observations, so later
//! domain/range/layout checks and codegen see the matching concrete type.
//!
//! Member operands resolve through their declared receiver type, including
//! explicit state subjects and typed locals. A receiver-only shortcut leaves
//! hoisted compound indices as Unit and breaks otherwise supported reads and
//! stores after bounds checking; the shared member walk preserves the same
//! field type through normalization without inferring authored local types.
//!
//! Field/index inference lowers the declared root once and replays its complete
//! path through the typed projection owner. A generic application carries the
//! binder environment for later hops: restarting at a template field type
//! loses that environment, so an otherwise ordinary captured indexed subject
//! incorrectly keeps Unit. Projection retains declaration and selector handles;
//! it does not evaluate the subject or manufacture bounds or active-case proof.
//!
//! Hoisted calls retain the resolved declaration's result and instantiate its
//! type telescope from explicit operands or authored argument types. Machine
//! symbols and structural type operands share the same substitution walk;
//! selected operand origins survive. This determines a temporary's type only:
//! ordinary call admission still checks the full signature, bounds and custody.

use crate::expressions::call_results::declared_call_return;
use crate::lowerer::Lowerer;
use crate::tables::{resolved_expressions, type_constraints};
use crate::type_reference::lower_type_reference_into_table;
use diagnostics::Diagnostic;
use resolved::types::ConstrainedTypeReference;
use symbol_resolved_trees as resolved;
use symbol_resolved_trees::types::{TypeConstraint, TypeReference};
use typed_trees as typed;

use resolved::expression::{ExpressionHandle, ExpressionNode};

/// Infers the typed declared type for a hoist temp whose `initial_value` is a
/// runtime-indexed read. Returns `None` (the caller then keeps `Unit`) for any
/// shape it cannot resolve -- that preserves the pre-existing behavior for those
/// shapes rather than guessing.
pub(super) fn infer_hoist_temp_type(
    lowerer: &mut Lowerer,
    attached_data: Option<&resolved::name::DiagnosticName>,
    state: &resolved::state::State,
    initial_value: ExpressionHandle,
) -> Result<Option<typed::types::TypeReferenceHandle>, Diagnostic> {
    let expressions = &lowerer.source_trees.tables.bodies.expressions;

    // Membership normalization captures indexed storage through a loan, not
    // an owned element snapshot. Keep that access mode on the inferred local
    // so downstream observation and ownership checks see the original place.
    if let ExpressionNode::Borrow(borrow) = expressions.expression(initial_value) {
        let (target, access) = (borrow.target, borrow.access);
        let Some(referee) = infer_hoist_temp_type(lowerer, attached_data, state, target)? else {
            return Ok(None);
        };
        return Ok(Some(lowerer.typed_trees.type_reference_table.insert(
            typed::types::TypeReferenceNode::Reference {
                referee,
                access,
                lifetime: None,
            },
        )));
    }

    match expressions.expression(initial_value) {
        // A `size_of<T>()`/`align_of<T>()` query hoisted out of a guard
        // comparison (`let __hoist = size_of<Row>()`): the compiler-owned
        // query has no callee signature to read a return type from, but its
        // result is always the builtin `u64`.
        ExpressionNode::Call(call)
            if language_core::data_layout_queries::is_data_layout_query_spelling(
                call.target.as_str(),
                call.target_symbol.is_valid(),
                call.receiver.is_valid(),
            ) =>
        {
            return builtin_primitive_type(lowerer, symbols::BuiltinTypeAtom::U64, "u64");
        }
        // A primitive byte predicate has a fixed Boolean result, independent
        // of its operand's carrier. Resolved declarations and receiver calls
        // retain their own result signatures even when their names coincide.
        ExpressionNode::Call(call)
            if !call.target_symbol.is_valid()
                && !call.receiver.is_valid()
                && expressions.expression_handles(call.arguments).len() == 1
                && language_semantics::byte_predicates::ByteSequencePredicate::from_name(
                    call.target.as_str(),
                )
                .is_some() =>
        {
            return builtin_primitive_type(lowerer, symbols::BuiltinTypeAtom::Bool, "bool");
        }
        // A USER value-machine call hoisted out of a guard comparison
        // (`let __hoist = self.dbl(5)`, syntax->symbol-resolved
        // `hoist_scalar_value_call_comparison`): the temp's type is the
        // callee's DECLARED return type, resolved by target symbol over
        // the machine-state arena. Calls awaiting typed overload selection
        // retain the inference sentinel until preparation settles their result.
        ExpressionNode::Call(call) => {
            let target_symbol = if call.target_symbol.is_valid() {
                call.target_symbol
            } else {
                crate::expressions::expression::declared_receiver_method_target(
                    lowerer.source_trees,
                    expressions,
                    call.receiver,
                    &call.target,
                )
            };
            let mut machine_substitutions = Vec::new();
            let declared_return = declared_call_return(lowerer.source_trees, target_symbol);
            let declared_return = declared_return
                    .cloned()
                    .or_else(|| {
                        static_conformance_requirement_return(lowerer, state, call)
                    })
                    .or_else(|| {
                        // MP3/MP4: a call through a static machine parameter
                        // has the authored `requires machine F(..) -> R` return
                        // type even before a concrete selection is known.
                        lowerer
                            .source_trees
                            .roots
                            .machines
                            .iter()
                            .find_map(|machine| {
                                lowerer
                                    .source_trees
                                    .machine_type_parameters(machine)
                                    .iter()
                                    .find_map(|parameter| match &parameter.kind {
                                        resolved::data::TypeParameterKind::Machine { contract }
                                            if parameter.symbol == target_symbol =>
                                        {
                                            let contract = lowerer
                                                .source_trees
                                                .machine_parameter_contract_view(contract)?
                                                .signature();
                                            machine_substitutions.extend(
                                                lowerer
                                                    .source_trees
                                                    .data_type_parameters(contract.type_parameters)
                                                    .iter()
                                                    .filter(|parameter| {
                                                        matches!(
                                                            parameter.kind,
                                                            resolved::data::TypeParameterKind::Machine { .. }
                                                        )
                                                    })
                                                    .zip(call.machine_arguments.iter())
                                                    .filter_map(|(parameter, argument)| {
                                                        argument.path.last().map(|name| {
                                                            (
                                                                parameter.symbol,
                                                                argument.symbol,
                                                                crate::lowerer::name::lower_name(name),
                                                            )
                                                        })
                                                    }),
                                            );
                                            contract.return_type.clone()
                                        }
                                        _ => None,
                                    })
                            })
                    });
            let Some(declared_return) = declared_return else {
                // Overload selection belongs to typed preparation. An unresolved
                // call does not imply an unannotated result: retain the inferred
                // binding until that owner selects its declaration and refreshes
                // the result through the shared expression-type query.
                return Ok(None);
            };
            let declared_return = lower_type_reference_into_table(lowerer, &declared_return)?;
            let machine_substitutions = machine_substitutions
                .into_iter()
                .map(|(parameter, symbol, name)| {
                    let reference = lowerer
                        .typed_trees
                        .type_reference_table
                        .insert(typed::types::TypeReferenceNode::Named { symbol, name });
                    (parameter, reference)
                })
                .collect::<Vec<_>>();
            substitute_call_type_arguments(
                &mut lowerer.typed_trees.type_reference_table,
                declared_return,
                &machine_substitutions,
            )?;
            instantiate_declared_call_result(
                lowerer,
                attached_data,
                state,
                call,
                target_symbol,
                declared_return,
            )?;
            return Ok(Some(declared_return));
        }
        _ => {}
    }

    // Scalar-place snapshots, fields and indexed elements follow the same
    // declared projection owner.
    // Retain the reaching application through every hop rather than selecting
    // a field from a receiver's diagnostic name and losing its arguments.
    if matches!(
        expressions.expression(initial_value),
        ExpressionNode::Name(_) | ExpressionNode::Member(_) | ExpressionNode::Indexed(_)
    ) {
        return declared_projection_type(lowerer, attached_data, state, initial_value);
    }

    // A computed index keeps its scalar carrier and arithmetic policy. The
    // initializer's ordinary arithmetic and index checks prove its bounds;
    // copying the operand's qualification would constrain the wrong value.
    if let Some(binary) = expressions.binary_node(initial_value) {
        // A Boolean-valued operator's result carrier is the primitive Boolean
        // whatever its operands' carriers are: the operator decides the result
        // type, so such a temp does not take the operand's base scalar the way a
        // computed index does, and no arithmetic interval describes it. The
        // caller keeps the `Unit` inference sentinel when this arm declines, and
        // a local whose declared type is neither primitive nor a real structural
        // type has no plan: the checked statement sequence abandons the whole
        // state, so a capture as ordinary as `transition seen == 30` — or a
        // conjunction over a slice, fixed-array or reference carrier — could not
        // plan its body either. The same comparison bound to an explicit `bool`
        // local always planned, which is the behavior restored here.
        if binary.operator.result_is_boolean() {
            return builtin_primitive_type(lowerer, symbols::BuiltinTypeAtom::Bool, "bool");
        }
        for operand in [binary.left, binary.right] {
            // `member_chain_type`, not `collection_type_reference`: the latter
            // resolves a member only when its receiver is spelled `self`, so
            // an explicit subject (`main.k + 1`) found no place operand and the
            // temp kept no carrier at all -- losing its unsignedness, which is
            // what discharges an index's lower bound by type alone. The chain
            // resolver reaches the same field through the subject parameter's
            // declared type.
            if let Some(place_type) = member_chain_type(lowerer, attached_data, state, operand) {
                let base = base_scalar_type(lowerer.source_trees, &place_type).clone();
                let scalar_handle = lower_type_reference_into_table(lowerer, &base)?;
                // The operand's arithmetic policy is part of the computed
                // value's type (`qh + qn` over `u64 in Wrapping` fields is a
                // `u64 in Wrapping`), unlike its range or declared domain.
                let base_handle = match arithmetic_domain(lowerer.source_trees, &place_type) {
                    Some(domain) => {
                        let constraints = lowerer
                            .typed_trees
                            .type_reference_table
                            .insert_constraints([
                                typed::types::TypeConstraintNode::ArithmeticDomain(domain),
                            ]);
                        lowerer.typed_trees.type_reference_table.insert(
                            typed::types::TypeReferenceNode::Constrained {
                                base_type: scalar_handle,
                                constraints,
                            },
                        )
                    }
                    None => scalar_handle,
                };
                return Ok(Some(base_handle));
            }
        }
        return Ok(None);
    }

    Ok(None)
}

/// Recover a projected place's type from its declared root. Replaying the
/// complete path keeps the generic binding environment across field/index
/// hops; restarting at a field's template type would strand its type binders.
/// Selector expressions are lowered to retain their handles, never evaluated.
fn declared_projection_type(
    lowerer: &mut Lowerer,
    attached_data: Option<&resolved::name::DiagnosticName>,
    state: &resolved::state::State,
    place: ExpressionHandle,
) -> Result<Option<typed::types::TypeReferenceHandle>, Diagnostic> {
    enum ProjectionStep<'source> {
        Element(ExpressionHandle),
        Field(&'source resolved::expression::TableMemberExpression),
    }
    let expressions = &lowerer.source_trees.tables.bodies.expressions;
    let mut steps = Vec::new();
    let mut root = place;
    loop {
        match expressions.expression(root) {
            ExpressionNode::Indexed(indexed) => {
                if matches!(
                    expressions.expression(indexed.index),
                    ExpressionNode::Range(_)
                ) {
                    return Ok(None);
                }
                steps.push(ProjectionStep::Element(indexed.index));
                root = indexed.collection;
            }
            ExpressionNode::Member(member) => {
                // The declared attached field is the root for the legacy
                // receiver spelling; ordinary receiver fields are path hops.
                if matches!(
                    expressions.expression(member.receiver),
                    ExpressionNode::Name(path) if expressions.name_path_members(path.members)
                        .first().is_some_and(|name| name.is_self_receiver())
                ) {
                    break;
                }
                steps.push(ProjectionStep::Field(member));
                root = member.receiver;
            }
            ExpressionNode::Borrow(borrow) => root = borrow.target,
            _ => break,
        }
    }
    let Some(declared_root) = collection_type_reference(lowerer, attached_data, state, root) else {
        return Ok(None);
    };
    let root_type = lower_type_reference_into_table(lowerer, &declared_root)?;
    let mut current = root_type;
    let mut path = Vec::with_capacity(steps.len());
    for step in steps.into_iter().rev() {
        match step {
            ProjectionStep::Element(selector) => {
                let selector =
                    crate::expressions::expression::lower_expression_handle(lowerer, selector)?;
                path.push(facts::PlaceSegment::Index {
                    expression: selector,
                });
            }
            ProjectionStep::Field(member) => {
                let Some(symbol) = declared_projection_field(&lowerer.typed_trees, current, member)
                else {
                    return Ok(None);
                };
                path.push(facts::PlaceSegment::Field { symbol });
            }
        }
        let Some(projected) = validation::project_type_reference_from_segments(
            &lowerer.typed_trees,
            root_type,
            &path,
        ) else {
            return Ok(None);
        };
        current = projected;
    }
    // Projection owns inherited arithmetic policy and retains the element's
    // complete declaration. Container predicates are not element predicates;
    // reappending the container constraints changes the captured value's type.
    Ok(Some(current))
}

fn declared_projection_field(
    program: &typed::TypedTrees,
    reference: typed::types::TypeReferenceHandle,
    member: &resolved::expression::TableMemberExpression,
) -> Option<symbols::SymbolHandle> {
    let reference = program
        .type_reference_table
        .peeled_type_reference(reference);
    let owner = program.closed_data_definition(reference).or_else(|| {
        let symbol = program.type_reference_table.type_head_symbol(reference)?;
        validation::unique_data_definition_by_symbol(program, symbol)
    })?;
    // Closed applications translate template selections only through their
    // retained instance correspondence, never through a same-spelled sibling.
    if owner.generic_instance.is_some() {
        return program
            .closed_data_member_field(reference, member.member_symbol, member.member.as_str())
            .map(|field| field.symbol);
    }
    let mut fields = program
        .data_members(owner)
        .iter()
        .filter_map(|row| match row {
            typed::data::DataMember::Field(field) if member.case_variant.is_none() => Some(field),
            _ => None,
        })
        .filter(|field| field.name.as_str() == member.member.as_str());
    let field = fields.next()?;
    if fields.next().is_some()
        || !field.symbol.is_valid()
        || (member.member_symbol.is_valid() && member.member_symbol != field.symbol)
    {
        return None;
    }
    Some(field.symbol)
}

fn static_conformance_requirement_return(
    lowerer: &Lowerer,
    state: &resolved::state::State,
    call: &resolved::expression::TableCallExpression,
) -> Option<TypeReference> {
    let ExpressionNode::Name(receiver) =
        resolved_expressions(lowerer.source_trees).expression(call.receiver)
    else {
        return None;
    };
    let owner = lowerer.source_trees.roots.machines.iter().find(|machine| {
        lowerer
            .source_trees
            .machine_state_handles(machine.states)
            .iter()
            .any(|handle| lowerer.source_trees.machine_state(*handle).symbol == state.symbol)
    })?;
    let mut bounds = owner
        .conformance_bounds
        .iter()
        .filter(|bound| bound.binder == receiver.symbol);
    let bound = bounds.next()?;
    if bounds.next().is_some() {
        return None;
    }
    let trait_definition = lowerer
        .source_trees
        .roots
        .trait_definition(bound.trait_symbol)?;
    // Requirements declared on a `requires` parent remain call requirements of
    // the bound trait, so the name lookup covers the transitive parent closure.
    let mut requirements = Vec::new();
    let mut pending = vec![trait_definition];
    let mut visited = Vec::new();
    while let Some(definition) = pending.pop() {
        if visited.contains(&definition.symbol) {
            continue;
        }
        visited.push(definition.symbol);
        requirements.extend(
            lowerer
                .source_trees
                .trait_machine_signatures(definition.machines)
                .iter()
                .filter(|requirement| requirement.name.as_str() == call.target.as_str()),
        );
        pending.extend(
            lowerer
                .source_trees
                .trait_requirements(definition.requires)
                .iter()
                .filter_map(|required| {
                    lowerer.source_trees.roots.trait_definition(required.symbol)
                }),
        );
    }
    let [requirement] = requirements.as_slice() else {
        return None;
    };
    requirement.storage.return_type.clone()
}

/// Instantiate a synthetic call result from the same declaration telescope as
/// ordinary calls. Explicit structural operands retain their supplying origins;
/// omitted type arguments are inferred from the authored argument types. This
/// query supplies a type, never call admission, property bounds or loan evidence.
fn instantiate_declared_call_result(
    lowerer: &mut Lowerer,
    attached_data: Option<&resolved::name::DiagnosticName>,
    state: &resolved::state::State,
    call: &resolved::expression::TableCallExpression,
    target: symbols::SymbolHandle,
    result: typed::types::TypeReferenceHandle,
) -> Result<(), Diagnostic> {
    use crate::expressions::call_results::{
        declared_call_parameters, declared_call_type_parameters,
    };
    let parameters = declared_call_type_parameters(lowerer.source_trees, target).to_vec();
    if parameters.is_empty() {
        return Ok(());
    }
    let type_parameters: Vec<_> = parameters
        .iter()
        .filter(|parameter| matches!(parameter.kind, resolved::data::TypeParameterKind::Type))
        .map(|parameter| parameter.symbol)
        .collect();
    let mut substitutions = Vec::new();
    for (parameter, argument) in parameters.iter().zip(call.machine_arguments.iter()) {
        if !matches!(parameter.kind, resolved::data::TypeParameterKind::Type) {
            continue;
        }
        let selected = crate::expressions::expression::lower_static_machine_argument(
            Some(lowerer.source_trees),
            &mut lowerer.typed_trees,
            lowerer.type_reference_exposure,
            argument,
        )?;
        let concrete = lowerer
            .typed_trees
            .static_operand_type_reference(&selected)
            .map_err(Diagnostic::error)?;
        substitutions.push((parameter.symbol, concrete));
    }
    let formal_parameters = declared_call_parameters(lowerer.source_trees, target).to_vec();
    let arguments = resolved_expressions(lowerer.source_trees)
        .expression_handles(call.arguments)
        .to_vec();
    for (formal, argument) in formal_parameters.iter().zip(arguments) {
        let Some(concrete) = infer_hoist_temp_type(lowerer, attached_data, state, argument)? else {
            continue;
        };
        let formal = lower_type_reference_into_table(lowerer, &formal.type_reference)?;
        infer_declared_type_arguments(
            &lowerer.typed_trees.type_reference_table,
            formal,
            concrete,
            &type_parameters,
            &mut substitutions,
        );
    }
    substitute_call_type_arguments(
        &mut lowerer.typed_trees.type_reference_table,
        result,
        &substitutions,
    )
}

fn infer_declared_type_arguments(
    table: &typed::types::TypeReferenceTable,
    formal: typed::types::TypeReferenceHandle,
    concrete: typed::types::TypeReferenceHandle,
    parameters: &[symbols::SymbolHandle],
    substitutions: &mut Vec<(symbols::SymbolHandle, typed::types::TypeReferenceHandle)>,
) {
    use typed::types::TypeReferenceNode as Node;
    match (table.type_reference(formal), table.type_reference(concrete)) {
        (Node::Named { symbol, .. }, _) if parameters.contains(symbol) => {
            if !substitutions
                .iter()
                .any(|(parameter, _)| parameter == symbol)
            {
                substitutions.push((*symbol, concrete));
            }
        }
        (Node::Constrained { base_type, .. }, _) => {
            infer_declared_type_arguments(table, *base_type, concrete, parameters, substitutions)
        }
        (_, Node::Constrained { base_type, .. }) => {
            infer_declared_type_arguments(table, formal, *base_type, parameters, substitutions)
        }
        (
            Node::Reference {
                referee: formal, ..
            },
            Node::Reference {
                referee: concrete, ..
            },
        )
        | (
            Node::Slice {
                element_type: formal,
            },
            Node::Slice {
                element_type: concrete,
            },
        )
        | (
            Node::FixedArray {
                element_type: formal,
                ..
            },
            Node::FixedArray {
                element_type: concrete,
                ..
            },
        ) => infer_declared_type_arguments(table, *formal, *concrete, parameters, substitutions),
        (
            Node::Generic {
                base_symbol: formal_symbol,
                arguments: formal,
                ..
            },
            Node::Generic {
                base_symbol: concrete_symbol,
                arguments: concrete,
                ..
            },
        ) if formal_symbol == concrete_symbol => {
            for (formal, concrete) in table
                .type_reference_handles(*formal)
                .iter()
                .zip(table.type_reference_handles(*concrete))
            {
                infer_declared_type_arguments(table, *formal, *concrete, parameters, substitutions);
            }
        }
        _ => {}
    }
}

fn substitute_call_type_arguments(
    table: &mut typed::types::TypeReferenceTable,
    reference: typed::types::TypeReferenceHandle,
    substitutions: &[(symbols::SymbolHandle, typed::types::TypeReferenceHandle)],
) -> Result<(), Diagnostic> {
    use typed::types::TypeReferenceNode as Node;
    let applications = table
        .type_alias_applications(reference)
        .map(|application| application.application)
        .collect::<Vec<_>>();
    for application in applications {
        substitute_call_type_arguments(table, application, substitutions)?;
    }
    match table.type_reference(reference).clone() {
        Node::Named { symbol, .. } => {
            if let Some((_, concrete)) = substitutions
                .iter()
                .find(|(parameter, _)| *parameter == symbol)
            {
                table
                    .substitute_from(reference, *concrete)
                    .map_err(Diagnostic::error)?;
            }
        }
        Node::Reference { referee, .. } => {
            substitute_call_type_arguments(table, referee, substitutions)?
        }
        Node::Constrained { base_type, .. } => {
            substitute_call_type_arguments(table, base_type, substitutions)?
        }
        Node::FixedArray { element_type, .. } | Node::Slice { element_type } => {
            substitute_call_type_arguments(table, element_type, substitutions)?
        }
        Node::Generic { arguments, .. } | Node::DynamicTrait { arguments, .. } => {
            for argument in table.type_reference_handles(arguments).to_vec() {
                substitute_call_type_arguments(table, argument, substitutions)?;
            }
        }
        Node::ConstExpression(_) | Node::Unit => {}
    }
    Ok(())
}

/// Resolves the resolved `TypeReference` of an indexed read's COLLECTION.
///
/// Handles `self.<field>` (resolved against the machine's attached data) and a
/// bare state parameter or explicitly typed local `<name>` (the slice/array a
/// `[i]` indexes). Other shapes (nested fields, deeper chains) return `None`.
fn collection_type_reference(
    lowerer: &Lowerer,
    attached_data: Option<&resolved::name::DiagnosticName>,
    state: &resolved::state::State,
    collection: ExpressionHandle,
) -> Option<TypeReference> {
    let expressions = &lowerer.source_trees.tables.bodies.expressions;
    match expressions.expression(collection) {
        // Synthetic computations and builtin operands use the same declared
        // member projection as indexed element walks. An explicit parameter
        // or typed local supplies its receiver type; it does not need a
        // synthetic attachment or a receiver-only inference route.
        ExpressionNode::Member(_) => member_chain_type(lowerer, attached_data, state, collection),
        // A bare `<name>` path -- a state parameter or explicitly typed local
        // naming the indexed collection. Locals remain fully explicit in Psi:
        // this only recovers the authored type of the collection that the
        // normalization pass moved behind a synthetic Unit-typed hoist temp.
        ExpressionNode::Name(path) => {
            let members = expressions.name_path_members(path.members);
            let [only] = members else {
                return None;
            };
            parameter_type(lowerer.source_trees, state, only.as_str()).or_else(|| {
                local_type(
                    lowerer.source_trees,
                    state,
                    path.head_symbol,
                    path.symbol,
                    only.as_str(),
                )
            })
        }
        _ => None,
    }
}

/// The declared type of a place read through a chain of field members rooted
/// at `self`, a state parameter or a typed local. A reference on the way is
/// looked through (the member reads the referee's field); any other shape,
/// including indexed steps, yields `None` so the caller keeps its sentinel.
fn member_chain_type(
    lowerer: &Lowerer,
    attached_data: Option<&resolved::name::DiagnosticName>,
    state: &resolved::state::State,
    place: ExpressionHandle,
) -> Option<TypeReference> {
    let expressions = &lowerer.source_trees.tables.bodies.expressions;
    let Some(member) = expressions.member_node(place) else {
        return collection_type_reference(lowerer, attached_data, state, place);
    };
    if matches!(
        expressions.expression(member.receiver),
        ExpressionNode::Name(path)
            if expressions.name_path_members(path.members)
                .first().is_some_and(|name| name.is_self_receiver())
    ) {
        return attached_field_type(lowerer.source_trees, attached_data?, member.member.as_str());
    }
    let receiver_type = member_chain_type(lowerer, attached_data, state, member.receiver)?;
    let data_name = named_data_of(lowerer.source_trees, &receiver_type)?;
    attached_field_type(lowerer.source_trees, &data_name, member.member.as_str())
}

/// The data type a field member reads from, looking through references and
/// domain/range constraints on the receiver's declared type.
fn named_data_of(
    source_trees: &resolved::SymbolResolvedTrees,
    type_reference: &TypeReference,
) -> Option<resolved::name::DiagnosticName> {
    match type_reference {
        TypeReference::Named { name, .. } => Some(name.clone()),
        TypeReference::Reference(reference) => named_data_of(
            source_trees,
            source_trees.child_type_reference(reference.referee),
        ),
        TypeReference::Constrained(constrained) => named_data_of(
            source_trees,
            source_trees.child_type_reference(constrained.base_type),
        ),
        _ => None,
    }
}

/// The BASE SCALAR of a place type, peeling `Constrained` wrappers: a declared
/// range or domain describes that PLACE's stored values, not a value computed
/// FROM it.
fn base_scalar_type<'trees>(
    source_trees: &'trees resolved::SymbolResolvedTrees,
    type_reference: &'trees TypeReference,
) -> &'trees TypeReference {
    match type_reference {
        TypeReference::Constrained(constrained) => base_scalar_type(
            source_trees,
            source_trees.child_type_reference(constrained.base_type),
        ),
        other => other,
    }
}

/// The arithmetic policy a place type declares, found through its
/// `Constrained` wrappers.
fn arithmetic_domain(
    source_trees: &resolved::SymbolResolvedTrees,
    type_reference: &TypeReference,
) -> Option<numerics::arithmetic::ArithmeticDomain> {
    let TypeReference::Constrained(constrained) = type_reference else {
        return None;
    };
    type_constraints(source_trees)
        .span_or_empty(constrained.constraints)
        .iter()
        .find_map(|constraint| match constraint {
            TypeConstraint::ArithmeticDomain(domain) => Some(*domain),
            _ => None,
        })
        .or_else(|| {
            arithmetic_domain(
                source_trees,
                source_trees.child_type_reference(constrained.base_type),
            )
        })
}

/// A compiler primitive's fixed result type, selected by its builtin identity
/// rather than a user declaration with the same diagnostic spelling.
fn builtin_primitive_type(
    lowerer: &mut Lowerer,
    atom: symbols::BuiltinTypeAtom,
    spelling: &'static str,
) -> Result<Option<typed::types::TypeReferenceHandle>, Diagnostic> {
    let symbols = &lowerer.source_trees.symbols;
    let Some(symbol) = symbols
        .child_handles(symbols.root())
        .and_then(|mut children| {
            children.find(|symbol| symbols.builtin_type_atom(*symbol) == Some(atom))
        })
    else {
        return Ok(None);
    };
    Ok(Some(lowerer.typed_trees.type_reference_table.insert(
        typed::types::TypeReferenceNode::Named {
            symbol,
            name: typed::name::Identifier::generated_static(spelling),
        },
    )))
}

fn attached_field_type(
    source_trees: &resolved::SymbolResolvedTrees,
    data_name: &resolved::name::DiagnosticName,
    field_name: &str,
) -> Option<TypeReference> {
    let data = source_trees
        .data_definitions
        .iter()
        .find(|data| data.name.as_str() == data_name.as_str())?;
    source_trees
        .data_members(data.members)
        .iter()
        .find_map(|member| match member {
            resolved::data::DataMember::Field(field) if field.name.as_str() == field_name => {
                Some(field.type_reference.clone())
            }
            _ => None,
        })
}

fn local_type(
    source_trees: &resolved::SymbolResolvedTrees,
    state: &resolved::state::State,
    head_symbol: symbols::SymbolHandle,
    symbol: symbols::SymbolHandle,
    name: &str,
) -> Option<TypeReference> {
    source_trees
        .state_statements(state.statements)
        .iter()
        .find_map(|statement| {
            let resolved::statement::Statement::LocalData(local) = statement else {
                return None;
            };
            let symbol_matches = (head_symbol.is_valid() && head_symbol == local.symbol)
                || (symbol.is_valid() && symbol == local.symbol);
            let symbols_are_absent = !head_symbol.is_valid() && !symbol.is_valid();
            (symbol_matches || (symbols_are_absent && local.name.as_str() == name))
                .then(|| local.type_reference.clone())
        })
}
