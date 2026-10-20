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

use crate::lowerer::Lowerer;
use crate::type_reference::lower_type_reference_into_table;
use diagnostics::Diagnostic;
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
        // the machine-state arena. An inferred-return callee cannot be
        // typed at this phase -- reject it with the actionable fix
        // instead of the confusing Unit frame-slot layout error.
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
            let target_name = call.target.as_str().to_string();
            let mut machine_substitutions = Vec::new();
            let declared_return = crate::expressions::call_results::declared_call_return(
                lowerer.source_trees,
                target_symbol,
            );
            let declared_return = declared_return
                    .cloned()
                    .or_else(|| {
                        static_conformance_requirement_return(lowerer, state, call)
                    })
                    .or_else(|| {
                        // MP3/MP4: a call through a static machine parameter
                        // has the authored `where machine F(..) -> R` return
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
                // An unresolved call spelling a sibling internal state is not a
                // missing return annotation: states are transition
                // destinations, never callables (state_contracts.md).
                let program = lowerer.source_trees;
                let internal_state = program.roots.machines.iter().any(|machine| {
                    let states = program.machine_state_handles(machine.states);
                    states
                        .iter()
                        .any(|handle| program.machine_state(*handle).symbol == state.symbol)
                        && states.iter().skip(1).any(|handle| {
                            program.machine_state(*handle).name.as_str() == target_name
                        })
                });
                if internal_state {
                    return Err(Diagnostic::error(format!(
                        "`{target_name}` is an internal state of this machine and cannot be \
                         called; ordinary calls invoke machines, not states: enter it with \
                         `transition {{ _ -> {target_name}(..) }}`, or make it a machine if \
                         its result is used as a value"
                    )));
                }
                return Err(Diagnostic::error(format!(
                    "a value-machine call in a guard comparison needs the callee's \
                         return type declared: annotate `{target_name}` with an explicit \
                         `-> Type`, or bind the call to a typed `let` first and guard \
                         the local"
                )));
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

    // A COMPUTED-INDEX temp (`let __hoist = self.k + 1`, hoisted by the
    // syntax->resolved index hoist so `arr[k + 1]` indexes a slotted plain
    // place): the temp's type is the BASE SCALAR of the first typeable place
    // operand -- the operand's OWN range does not hold for the computed value
    // (`k in [0..=3]` but `k + 1` reaches 4) -- carrying a SYNTHESIZED range
    // computed by interval arithmetic over the operands' Exact declared
    // ranges. The range is what lets the index prover discharge the bound
    // (`expression_enforced_declared_range`), and it is SOUND because a
    // declared range is store-enforced: the temp's one store (the
    // initializer) is range-proved, which accepts `k + 1` outright and
    // rejects an unguarded `k - 1` (underflow at k=0) unless a dominating
    // guard narrows k -- exactly the manual let-temp idiom's behavior. An
    // operand with no Exact range yields a bare scalar temp (the index prover
    // then reports its clean cannot-prove error).
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
                let Some((low, high)) = computed_index_interval(
                    lowerer,
                    attached_data,
                    state,
                    binary.operator,
                    binary.left,
                    binary.right,
                ) else {
                    // An operand's carrier is not the operation's result:
                    // comparison and selected operators may return another
                    // type. Without a derived arithmetic range, preparation
                    // infers the result from the selected typed expression.
                    return Ok(None);
                };
                // A negative bound cannot ride an UNSIGNED scalar's range;
                // clamp the low end to 0 (the store proof still gates every
                // value, so clamping never widens what is accepted).
                let low = if scalar_is_unsigned(&base) {
                    if high < 0 {
                        return Ok(Some(base_handle));
                    }
                    low.max(0)
                } else {
                    low
                };
                let minimum = lowerer.typed_trees.expression_table.insert(
                    typed::expression::ExpressionNode::Integer(
                        numerics::literals::IntegerLiteral::from_value(low),
                    ),
                );
                let maximum = lowerer.typed_trees.expression_table.insert(
                    typed::expression::ExpressionNode::Integer(
                        numerics::literals::IntegerLiteral::from_value(high),
                    ),
                );
                let constraints = lowerer
                    .typed_trees
                    .type_reference_table
                    .insert_constraints([typed::types::TypeConstraintNode::Range {
                        minimum,
                        maximum,
                        end_inclusive: true,
                    }]);
                return Ok(Some(lowerer.typed_trees.type_reference_table.insert(
                    typed::types::TypeReferenceNode::Constrained {
                        base_type: base_handle,
                        constraints,
                    },
                )));
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
    let ExpressionNode::Name(receiver) = lowerer
        .source_trees
        .tables
        .bodies
        .expressions
        .expression(call.receiver)
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
    let arguments = lowerer
        .source_trees
        .tables
        .bodies
        .expressions
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
    source_trees
        .tables
        .types
        .constraints
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

fn scalar_is_unsigned(type_reference: &TypeReference) -> bool {
    matches!(
        type_reference,
        TypeReference::Named { name, .. }
            if matches!(name.as_str(), "u8" | "u16" | "u32" | "u64" | "usize")
    )
}

/// The inclusive interval of a hoisted computed index, by interval arithmetic
/// over the two operands' intervals. `None` when an operand has no usable
/// interval, the operator is not +/-/*, or the arithmetic overflows i64.
fn computed_index_interval(
    lowerer: &Lowerer,
    attached_data: Option<&resolved::name::DiagnosticName>,
    state: &resolved::state::State,
    operator: resolved::expression::BinaryOperator,
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> Option<(i64, i64)> {
    // R3b: the DIRECT product spelling `pixels[y * self.cols + x]` -- the
    // typed-side bounded-product rule mirrored at the hoist synthesis, so
    // the temp's range carries the coupling bound and the index prover
    // discharges without an explicit ranged temp.
    if let Some(interval) = dependent_product_index_interval(lowerer, state, operator, left, right)
    {
        return Some(interval);
    }
    let left = operand_declared_interval(lowerer, attached_data, state, left)?;
    let right = operand_declared_interval(lowerer, attached_data, state, right)?;
    match operator {
        resolved::expression::BinaryOperator::Add => {
            Some((left.0.checked_add(right.0)?, left.1.checked_add(right.1)?))
        }
        resolved::expression::BinaryOperator::Subtract => {
            Some((left.0.checked_sub(right.1)?, left.1.checked_sub(right.0)?))
        }
        resolved::expression::BinaryOperator::Multiply => {
            let products = [
                left.0.checked_mul(right.0)?,
                left.0.checked_mul(right.1)?,
                left.1.checked_mul(right.0)?,
                left.1.checked_mul(right.1)?,
            ];
            Some((
                *products.iter().min().expect("non-empty"),
                *products.iter().max().expect("non-empty"),
            ))
        }
        _ => None,
    }
}

/// An OPERAND's inclusive interval: an integer literal's exact value, or a
/// place's declared Exact range (a non-Exact domain's range is deliberately
/// permissive -- probed live: `[0..=4] in Wrapping` accepts a store of 100 --
/// so it must never seed an index-bound synthesis).
fn operand_declared_interval(
    lowerer: &Lowerer,
    attached_data: Option<&resolved::name::DiagnosticName>,
    state: &resolved::state::State,
    operand: ExpressionHandle,
) -> Option<(i64, i64)> {
    let expressions = &lowerer.source_trees.tables.bodies.expressions;
    if let Some(literal) = expressions.integer_node(operand) {
        let value = literal.value_i64()?;
        return Some((value, value));
    }
    // A nested binary operand (`y * 4` inside `y * 4 + x`) composes through
    // the same interval arithmetic -- depth-bounded by the hoist predicate,
    // which admits exactly one nested level (R0's row-major shape).
    if let Some(inner) = expressions.binary_node(operand) {
        return computed_index_interval(
            lowerer,
            attached_data,
            state,
            inner.operator,
            inner.left,
            inner.right,
        );
    }
    // The chain resolver, so an operand spelled through an explicit subject
    // (`main.k`) contributes its declared range exactly as `self.k` does.
    let place_type = member_chain_type(lowerer, attached_data, state, operand)?;
    declared_exact_range(lowerer.source_trees, &place_type).or_else(|| {
        dependent_exact_range_substituted(lowerer.source_trees, attached_data, &place_type)
    })
}

/// Constrained-shell constraint rows of `type_reference`, only when every
/// arithmetic domain in the shell is Exact — the shared gate both range
/// recognizers run.
fn exact_domain_constraints<'t>(
    source_trees: &'t resolved::SymbolResolvedTrees,
    type_reference: &'t TypeReference,
) -> Option<(
    &'t resolved::types::ConstrainedTypeReference,
    &'t [TypeConstraint],
)> {
    let TypeReference::Constrained(constrained) = type_reference else {
        return None;
    };
    let constraints = source_trees
        .tables
        .types
        .constraints
        .span_or_empty(constrained.constraints);
    if constraints.iter().any(|constraint| {
        matches!(
            constraint,
            TypeConstraint::ArithmeticDomain(domain)
                if *domain != numerics::arithmetic::ArithmeticDomain::Exact
        )
    }) {
        return None;
    }
    Some((constrained, constraints))
}

/// R1a dependent maximum as an INTERVAL operand: `y: u32 [0..=self.rows]`
/// contributes `(0, high(rows) + offset)` -- the named field's own
/// store-enforced literal range caps the dependent bound at every moment,
/// so the substituted interval is sound for the temp's synthesized range
/// (the same substitution the checker's index prover applies). The field
/// resolves on the ENCLOSING machine's attached data (`attached_data`).
fn dependent_exact_range_substituted(
    source_trees: &resolved::SymbolResolvedTrees,
    attached_data: Option<&resolved::name::DiagnosticName>,
    type_reference: &TypeReference,
) -> Option<(i64, i64)> {
    let (_constrained, constraints) = exact_domain_constraints(source_trees, type_reference)?;
    let data_name = attached_data?;
    constraints.iter().find_map(|constraint| {
        let TypeConstraint::Range {
            minimum,
            maximum,
            end_inclusive,
        } = constraint
        else {
            return None;
        };
        let expressions = &source_trees.tables.bodies.expressions;
        let low = match expressions.expression(*minimum) {
            ExpressionNode::Integer(literal) => literal.value_i64()?,
            _ => return None,
        };
        let (field, offset) = resolved_symbolic_max_bound(source_trees, *maximum)?;
        let field_type = attached_field_type(source_trees, data_name, &field)?;
        let (_, field_high) = declared_exact_range(source_trees, &field_type)?;
        let high = field_high.checked_add(offset)?;
        // This conservative inference hint does not validate the authored
        // endpoint. Semantic range normalization checks it after typing.
        let high = if *end_inclusive {
            high
        } else {
            high.checked_sub(1)?
        };
        (low <= high).then_some((low, high))
    })
}

/// Resolved-tree twin of `typed_trees::dependent_ranges`'s recognizer
/// (`<param>.<field>` on a subject parameter plus an optional literal
/// offset). Kept to the same admissible class -- the typed-level gate has
/// already validated it. Subject position is decided by the receiver's
/// resolved symbol, never by its spelling.
fn resolved_symbolic_max_bound(
    source_trees: &resolved::SymbolResolvedTrees,
    bound: ExpressionHandle,
) -> Option<(String, i64)> {
    let expressions = &source_trees.tables.bodies.expressions;
    let subject_field = |handle: ExpressionHandle| -> Option<String> {
        let member = expressions.member_node(handle)?;
        let path = expressions.name_path_node(member.receiver)?;
        let [_] = expressions.name_path_members(path.members) else {
            return None;
        };
        resolved_receiver_is_subject_parameter(source_trees, path.symbol)
            .then(|| member.member.as_str().to_owned())
    };
    match expressions.expression(bound) {
        ExpressionNode::Member(_) => Some((subject_field(bound)?, 0)),
        ExpressionNode::Binary(binary) => {
            let field = subject_field(binary.left)?;
            let literal = expressions.integer_node(binary.right)?;
            let magnitude = literal.value_i64()?;
            let offset = match binary.operator {
                resolved::expression::BinaryOperator::Add => magnitude,
                resolved::expression::BinaryOperator::Subtract => magnitude.checked_neg()?,
                _ => return None,
            };
            Some((field, offset))
        }
        _ => None,
    }
}

/// Whether `symbol` names a parameter holding a subject position: a
/// machine state parameter declaring the owning machine's attached data --
/// the resolved-stage mirror of `typed_trees::dependent_ranges`'s
/// `receiver_is_subject_parameter`. A resolved name's symbol identifies
/// one parameter program-wide, so the answer never depends on which
/// signature asks.
fn resolved_receiver_is_subject_parameter(
    source_trees: &resolved::SymbolResolvedTrees,
    receiver: symbols::SymbolHandle,
) -> bool {
    if !receiver.is_valid() {
        return false;
    }
    for machine in source_trees.machines.iter() {
        for state_handle in source_trees.machine_state_handles(machine.storage.states) {
            let state = source_trees.machine_state(*state_handle);
            for parameter in source_trees.state_parameters(state.parameters) {
                if parameter.symbol == receiver {
                    return resolved_type_names_attached_data(
                        source_trees,
                        &parameter.type_reference,
                        machine.attached_data_symbol,
                    );
                }
            }
        }
    }
    false
}

/// Whether `type_reference` names `attached_data_symbol` once references
/// and constraints are stripped -- the resolved twin of
/// `TypedTrees::type_reference_names_attached_data`.
fn resolved_type_names_attached_data(
    source_trees: &resolved::SymbolResolvedTrees,
    type_reference: &TypeReference,
    attached_data_symbol: symbols::SymbolHandle,
) -> bool {
    if !attached_data_symbol.is_valid() {
        return false;
    }
    match type_reference {
        TypeReference::Named { symbol, .. } => *symbol == attached_data_symbol,
        TypeReference::Reference(reference) => resolved_type_names_attached_data(
            source_trees,
            source_trees.child_type_reference(reference.storage.referee),
            attached_data_symbol,
        ),
        TypeReference::Constrained(constrained) => resolved_type_names_attached_data(
            source_trees,
            source_trees.child_type_reference(constrained.storage.base_type),
            attached_data_symbol,
        ),
        _ => false,
    }
}

/// The Range constraint of a resolved place type, ONLY when every arithmetic
/// domain in its Constrained shells is Exact (mirrors the checker's
/// `enforced_range_of_type_reference`).
fn declared_exact_range(
    source_trees: &resolved::SymbolResolvedTrees,
    type_reference: &TypeReference,
) -> Option<(i64, i64)> {
    let (constrained, constraints) = exact_domain_constraints(source_trees, type_reference)?;
    constraints
        .iter()
        .find_map(|constraint| match constraint {
            TypeConstraint::Range {
                minimum,
                maximum,
                end_inclusive,
            } => {
                let expressions = &source_trees.tables.bodies.expressions;
                let low = match expressions.expression(*minimum) {
                    ExpressionNode::Integer(literal) => literal.value_i64()?,
                    _ => return None,
                };
                let high = match expressions.expression(*maximum) {
                    ExpressionNode::Integer(literal) => literal.value_i64()?,
                    _ => return None,
                };
                // Unknown or out-of-carrier hints stay absent; the original
                // endpoint survives for semantic checking after typing.
                let high = if *end_inclusive {
                    high
                } else {
                    high.checked_sub(1)?
                };
                Some((low, high))
            }
            _ => None,
        })
        .or_else(|| {
            declared_exact_range(
                source_trees,
                source_trees.child_type_reference(constrained.base_type),
            )
        })
}

/// The declared resolved type of `field_name` in the data definition named
/// `data_name`.
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

/// The declared resolved type of state parameter `name`.
fn parameter_type(
    source_trees: &resolved::SymbolResolvedTrees,
    state: &resolved::state::State,
    name: &str,
) -> Option<TypeReference> {
    source_trees
        .state_parameters(state.parameters)
        .iter()
        .find(|parameter| parameter.name.as_str() == name)
        .map(|parameter| parameter.type_reference.clone())
}

/// The authored resolved type of a state local. Prefer symbol identity so a
/// shadowed name cannot retag a hoist; the name fallback is retained for the
/// generated-tree phase where a path may not yet carry its final symbol.
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

/// R3b: `a * self.Fb + c` with STRICT dependent params (`a < self.Fa`,
/// `c < self.Fb`, resolved-tree recognizers) and the owning machine's
/// `requires self.Fa * self.Fb <= K` coupling -> `(0, K - 1)`. Mirrors
/// validation's refine_dependent_product; kept in lockstep. The
/// preservation gate is the typed-side rule's job at USE time -- the
/// synthesized range is store-proved on the temp's own initializer, which
/// re-runs the typed-side rule (a machine that writes the dims fails THAT
/// proof, so a stale synthesis here never survives to an index).
fn dependent_product_index_interval(
    lowerer: &Lowerer,
    state: &resolved::state::State,
    operator: resolved::expression::BinaryOperator,
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> Option<(i64, i64)> {
    if operator != resolved::expression::BinaryOperator::Add {
        return None;
    }
    let expressions = &lowerer.source_trees.tables.bodies.expressions;
    let product = expressions.binary_node(left)?;
    if product.operator != resolved::expression::BinaryOperator::Multiply {
        return None;
    }
    let (a_expr, fb_expr) = {
        let left_is_field = resolved_symbolic_max_bound(lowerer.source_trees, product.left)
            .is_some_and(|(_, offset)| offset == 0);
        if left_is_field {
            (product.right, product.left)
        } else {
            (product.left, product.right)
        }
    };
    let (fb, fb_offset) = resolved_symbolic_max_bound(lowerer.source_trees, fb_expr)?;
    if fb_offset != 0 {
        return None;
    }
    let fa = strict_dependent_param_field(lowerer, state, a_expr)?;
    let c_field = strict_dependent_param_field(lowerer, state, right)?;
    if c_field != fb {
        return None;
    }
    let machine = lowerer.source_trees.machines.iter().find(|machine| {
        lowerer
            .source_trees
            .machine_state_handles(machine.storage.states)
            .iter()
            .any(|handle| lowerer.source_trees.machine_state(*handle).symbol == state.symbol)
    })?;
    let k = resolved_product_coupling(lowerer, machine, &fa, &fb)?;
    let high = k.checked_sub(1)?;
    (high >= 0).then_some((0, high))
}

/// The field a STRICT dependent PARAM names on the resolved trees: the
/// expression is a bare param name whose declared range maximum is
/// `self.<field> - 1`.
fn strict_dependent_param_field(
    lowerer: &Lowerer,
    state: &resolved::state::State,
    expression: ExpressionHandle,
) -> Option<String> {
    let expressions = &lowerer.source_trees.tables.bodies.expressions;
    let path = expressions.name_path_node(expression)?;
    let [only] = expressions.name_path_members(path.members) else {
        return None;
    };
    let parameter = parameter_type(lowerer.source_trees, state, only.as_str())?;
    let TypeReference::Constrained(constrained) = parameter else {
        return None;
    };
    let constraints = lowerer
        .source_trees
        .tables
        .types
        .constraints
        .span_or_empty(constrained.constraints);
    constraints.iter().find_map(|constraint| {
        let TypeConstraint::Range {
            maximum,
            end_inclusive,
            ..
        } = constraint
        else {
            return None;
        };
        let (field, offset) = resolved_symbolic_max_bound(lowerer.source_trees, *maximum)?;
        let offset = if *end_inclusive {
            offset
        } else {
            offset.checked_sub(1)?
        };
        (offset == -1).then_some(field)
    })
}

/// The owning machine's `requires self.Fa * self.Fb <= K` conjunct on the
/// RESOLVED trees (either multiply order; strict `<` gives K - 1).
fn resolved_product_coupling(
    lowerer: &Lowerer,
    machine: &resolved::machine::Machine,
    fa: &str,
    fb: &str,
) -> Option<i64> {
    for contract in lowerer.source_trees.machine_contracts(machine) {
        if contract.kind != resolved::signature::SignatureContractKind::Requires {
            continue;
        }
        for fact in lowerer.source_trees.proof_facts(contract.facts) {
            let resolved::domain::ProofFact::Expression(expression) = fact else {
                continue;
            };
            if let Some(k) = resolved_product_conjunct(lowerer, machine, *expression, fa, fb) {
                return Some(k);
            }
        }
    }
    None
}

fn resolved_product_conjunct(
    lowerer: &Lowerer,
    machine: &resolved::machine::Machine,
    guard: ExpressionHandle,
    fa: &str,
    fb: &str,
) -> Option<i64> {
    let expressions = &lowerer.source_trees.tables.bodies.expressions;
    let binary = expressions.binary_node(guard)?;
    use resolved::expression::BinaryOperator;
    match binary.operator {
        BinaryOperator::And => resolved_product_conjunct(lowerer, machine, binary.left, fa, fb)
            .or_else(|| resolved_product_conjunct(lowerer, machine, binary.right, fa, fb)),
        BinaryOperator::LessOrEqual | BinaryOperator::Less => {
            let product = expressions.binary_node(binary.left)?;
            if product.operator != BinaryOperator::Multiply {
                return None;
            }
            let lhs = resolved_product_coupling_operand(lowerer, machine, product.left)?;
            let rhs = resolved_product_coupling_operand(lowerer, machine, product.right)?;
            let matches = (lhs == fa && rhs == fb) || (lhs == fb && rhs == fa);
            if !matches {
                return None;
            }
            let literal = expressions.integer_node(binary.right)?;
            let k = literal.value_i64()?;
            if binary.operator == BinaryOperator::Less {
                k.checked_sub(1)
            } else {
                Some(k)
            }
        }
        _ => None,
    }
}

/// Resolved-tree twin of validation's bounded-product operand projection.
/// The direct field spelling remains canonical. An Exact widening cast is
/// transparent because validation independently proves it preserves the
/// source value and makes the specification product total.
fn resolved_product_coupling_operand(
    lowerer: &Lowerer,
    machine: &resolved::machine::Machine,
    expression: ExpressionHandle,
) -> Option<String> {
    let expressions = &lowerer.source_trees.tables.bodies.expressions;
    if let Some((field, 0)) = resolved_symbolic_max_bound(lowerer.source_trees, expression) {
        return Some(field);
    }
    let cast = expressions.cast_node(expression)?;
    if cast.form.is_recast()
        || !cast.semantic_domain.is_empty()
        || cast.domain != numerics::arithmetic::ArithmeticDomain::Exact
    {
        return None;
    }
    let (field, 0) = resolved_symbolic_max_bound(lowerer.source_trees, cast.value)? else {
        return None;
    };
    let target = lowerer
        .source_trees
        .child_type_reference(cast.target_type)
        .primitive_type()?;
    let source = resolved_primitive_type(
        lowerer.source_trees,
        &attached_field_type(
            lowerer.source_trees,
            machine.attached_data.as_ref()?,
            &field,
        )?,
    )?;
    let width = |primitive| match primitive {
        resolved::types::PrimitiveType::U8 => Some(8),
        resolved::types::PrimitiveType::U16 => Some(16),
        resolved::types::PrimitiveType::U32 => Some(32),
        resolved::types::PrimitiveType::U64 => Some(64),
        _ => None,
    };
    if width(source)? >= width(target)? {
        return None;
    }
    Some(field)
}

fn resolved_primitive_type(
    source_trees: &resolved::SymbolResolvedTrees,
    type_reference: &TypeReference,
) -> Option<resolved::types::PrimitiveType> {
    match type_reference {
        TypeReference::Constrained(constrained) => resolved_primitive_type(
            source_trees,
            source_trees.child_type_reference(constrained.base_type),
        ),
        other => other.primitive_type(),
    }
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
