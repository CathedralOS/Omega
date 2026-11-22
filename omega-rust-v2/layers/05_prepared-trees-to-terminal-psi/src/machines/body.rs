use super::branches::BlockFlow;
use super::expressions::Expressions;
use crate::lowering::applied_tree::LoweringTree;
use crate::{
    EntryDiagnostics, LoweringError, LoweringTypes, MachineBody, OperationResult, Parameter, Place, PlaceId, PlaceOrigin, TypeId, Value, ValueId, ValueOrigin,
};
use omega_prepared::PreparedTrees;
use omega_resolved::{IndexedSource, Symbol};
use omega_syntax::NodeKind;
use omega_typed::{NodeSemantics, Site, TypedSignature, ValueType};
use std::num::NonZeroUsize;

pub(crate) struct Declarations<'a> {
    pub machine: crate::MachineId,
    pub types: &'a LoweringTypes,
    pub data: &'a [crate::LoweredData],
    pub callees: &'a super::calls::Callees<'a>,
    /// The machine's published entry clauses, the same premises the checker reads.
    pub requires: &'a [crate::ContractClause],
}

pub(crate) fn lower_body(
    prepared: &PreparedTrees<'_>,
    tree: &LoweringTree<'_>,
    signature: &TypedSignature,
    parameter_types: &[TypeId],
    result_type: Option<TypeId>,
    declarations: &Declarations<'_>,
) -> Result<(MachineBody, super::ownership::OwnedCrossings), LoweringError> {
    let mut body = emit_body(prepared, tree, signature, parameter_types, result_type, declarations)?;
    let crossings = super::ownership::schedule(&mut body, declarations.types, declarations.data, declarations.callees)?;
    Ok((body, crossings))
}

fn emit_body(
    prepared: &PreparedTrees<'_>,
    tree: &LoweringTree<'_>,
    signature: &TypedSignature,
    parameter_types: &[TypeId],
    result_type: Option<TypeId>,
    declarations: &Declarations<'_>,
) -> Result<MachineBody, LoweringError> {
    let Declarations {
        types,
        data,
        callees,
        machine,
        requires,
    } = *declarations;
    let resolved = tree.resolved();
    let file = resolved.file;
    let declaration = signature.node.node;
    if resolved.declaration_at(declaration) != Some(&signature.machine) {
        return Err(LoweringError::new(&signature.site, "machine signature disagrees with its defining tree"));
    }

    let assembly = file.nodes[declaration].kind == NodeKind::AsmMachine;
    let body = if assembly {
        declaration
    } else {
        file.child(declaration, NodeKind::Body)
            .ok_or_else(|| LoweringError::new(&signature.site, "machine has no executable body"))?
    };
    let params = file.child(declaration, NodeKind::Params);
    if !assembly && params.is_none() {
        return Err(LoweringError::new(&signature.site, "machine has no parameter list"));
    }
    let binders = params.map_or(&[][..], |p| file.nodes[p].children.as_slice());
    if binders.len() != signature.parameters.len() || binders.len() != parameter_types.len() {
        return Err(LoweringError::new(
            &site_at(file, params.unwrap_or(declaration)),
            "parameter list disagrees with the typed signature",
        ));
    }

    let body_site = site_at(file, body);
    let mut block = MachineBody {
        lifetime_bindings: Vec::new(),
        call_regions: Vec::new(),
        call_acknowledgements: Vec::new(),
        suspension_sites: Vec::new(),
        callback_invocations: Vec::new(),
        blocks: Vec::new(),
        operations: Vec::new(),
        parameters: Vec::with_capacity(binders.len()),
        mutable_homes: Vec::new(),
        places: Vec::new(),
        values: Vec::new(),
        diagnostics: EntryDiagnostics {
            body: body_site.clone(),
            parameters: Vec::with_capacity(binders.len()),
            operations: Vec::new(),
            edges: Vec::new(),
            bounds: Vec::new(),
        },
    };
    for ((&binder, (span, carrier)), &ty) in binders.iter().zip(&signature.parameters).zip(parameter_types) {
        let node = &file.nodes[binder];
        let site = site_at(file, binder);
        if node.binder_name().is_none_or(|name| name.span != *span) || node.kind != NodeKind::Param || node.children.len() != 1 {
            return Err(LoweringError::new(&site, "parameter binder disagrees with the typed signature"));
        }
        let parameter = match crate::declarations::regions::slot_carrier(carrier, &signature.machine) {
            carrier if crate::lowering::types::uses_structural_place(carrier) => {
                let place = PlaceId(next_id(block.places.len()));
                block.places.push(Place {
                    ty,
                    origin: PlaceOrigin::Parameter,
                });
                Parameter::Structural(place)
            }
            _ => Parameter::Scalar(block.push_value(ty, ValueOrigin::Parameter)),
        };
        block.parameters.push(parameter);
        block.diagnostics.parameters.push(site);
    }

    if !assembly {
        check_container(tree, body)?;
    }
    let mut expressions = Expressions {
        machine,
        prepared,
        tree,
        signature,
        types,
        data,
        callees,
        bindings: binders
            .iter()
            .copied()
            .zip(block.parameters.iter().map(|parameter| match parameter {
                Parameter::Scalar(value) => OperationResult::Scalar(*value),
                Parameter::Structural(place) => OperationResult::Structural(*place),
            }))
            .collect(),
        selector_premises: super::view_loans::selector_premises(requires, &block.parameters),
    };
    let mut flow = BlockFlow::entry();
    let borrowed = if assembly {
        Vec::new()
    } else {
        directly_borrowed_binders(resolved, file, body)
    };
    for (&binder, slot) in binders.iter().zip(expressions.bindings.iter_mut()) {
        slot.1 = home_borrowed_scalar(file, types, &borrowed, binder, slot.1, &mut block);
    }
    if assembly {
        super::assembly::machine(&expressions, declaration, &mut block, &mut flow, result_type)?;
        flow.finish(&mut block);

        return Ok(block);
    }
    let statements = &file.nodes[body].children;
    // The grammar places nested machines after the enclosing machine's
    // executable statements. Their bodies have separate machine identities.
    let end = statements
        .iter()
        .position(|&n| file.nodes[n].kind == NodeKind::Machine)
        .unwrap_or(statements.len());
    let (statements, nested) = statements.split_at(end);
    for &node in nested {
        if file.nodes[node].kind != NodeKind::Machine
            || !resolved
                .declaration_at(node)
                .is_some_and(|id| callees.local.contains_key(id))
        {
            return Err(unsupported_node(file, node, "nested declaration has no Psi machine"));
        }
    }
    let mut statements_iter = statements.iter().copied().enumerate().peekable();
    while let Some((index, statement)) = statements_iter.next() {
        let node = &file.nodes[statement];
        if matches!(node.kind, NodeKind::Tail | NodeKind::Discard | NodeKind::ExpressionStatement)
            && let [expression] = node.children.as_slice()
            && let Some(completion) = super::external_exit::recognized(&expressions, *expression)
        {
            check_container(tree, statement)?;
            super::external_exit::lower(&expressions, *expression, completion, &mut block, &mut flow)?;
            flow.finish(&mut block);
            return Ok(block);
        }
        match (node.kind, node.children.as_slice()) {
            (NodeKind::Let, [binder, _])
                if tree
                    .semantics_at(statement)
                    .any(|r| matches!(r,NodeSemantics::Obligation(o) if matches!(o.kind,omega_typed::ObligationKind::ZeroValue(_)))) =>
            {
                super::references::provision_local(&mut expressions, statement, *binder, data, &mut block)?;
            }
            (NodeKind::Let, [binder, initializer] | [binder, _, initializer])
                if file.nodes[*binder].kind == NodeKind::BindingPattern && resolved.pattern_binding(*binder).is_some() =>
            {
                check_container(tree, statement)?;
                let carrier = binding_carrier(tree, *binder)?;
                let value = expressions.bind_local(*initializer, carrier, &mut block, &mut flow)?;
                let homed = home_borrowed_scalar(file, types, &borrowed, *binder, value, &mut block);
                // A record home, or a primitive home established just now for a borrow.
                if let OperationResult::Structural(home) = homed
                    && (crate::lowering::types::contains_nominal_value(carrier) || homed != value)
                    && tree
                        .forest()
                        .mutable_bindings
                        .contains(&file.node_id(*binder))
                {
                    block.mutable_homes.push(home);
                }
                expressions.bindings.push((*binder, homed));
            }
            (NodeKind::Let, [pattern, initializer]) if file.nodes[*pattern].kind == NodeKind::RecordPattern => {
                check_container(tree, statement)?;
                super::ownership::decompose(&mut expressions, *pattern, *initializer, &mut block, &mut flow)?;
            }
            (NodeKind::Assign, [place, initializer]) => {
                check_container(tree, statement)?;
                let place = file.ungroup(*place);
                if matches!(file.nodes[place].kind, NodeKind::FieldAccess | NodeKind::Dereference) {
                    super::references::store_place(&expressions, place, *initializer, data, &mut block, &mut flow)?;
                    continue;
                }
                if super::view_loans::selects_view_element(&expressions, place) {
                    let before_return = index + 1 == statements.len() && result_type.is_none();
                    let next = statements_iter.peek().map(|(position, statement)| (*statement, position + 1 == statements.len() && result_type.is_none()));
                    if super::view_loans::store_element(&expressions, place, *initializer, &mut block, &mut flow, before_return, next)? {
                        statements_iter.next();
                    }
                    continue;
                }
                let Some(Symbol::Binding {
                    binder_node,
                    kind: NodeKind::BindingPattern,
                    ..
                }) = resolved.symbol(place)
                else {
                    return Err(unsupported_node(file, place, "assignment needs a lowered storage path"));
                };
                let carrier = binding_carrier(tree, *binder_node)?;
                if crate::lowering::types::contains_nominal_value(carrier) {
                    let Some(OperationResult::Structural(root)) = expressions
                        .bindings
                        .iter()
                        .rev()
                        .find(|(binder, _)| binder == binder_node)
                        .map(|(_, value)| *value)
                    else {
                        return Err(unsupported_node(file, place, "assignment has no structural local"));
                    };
                    if !block.mutable_homes.contains(&root) {
                        return Err(unsupported_node(file, place, "assignment requires a writable local"));
                    }
                    super::references::store_intact(
                        &expressions,
                        crate::ProjectedPlace { root, path: vec![] },
                        *initializer,
                        carrier,
                        &mut block,
                        &mut flow,
                        site_at(file, place),
                        None,
                    )?;
                    continue;
                }
                let value = expressions.bind_local(*initializer, carrier, &mut block, &mut flow)?;
                let slot = expressions
                    .bindings
                    .iter_mut()
                    .rfind(|(binder, _)| binder == binder_node)
                    .ok_or_else(|| unsupported_node(file, place, "assignment has no local value"))?;
                // A borrowed mutable primitive keeps one home: the assignment
                // stores into it, so earlier loans and later reads see the same
                // storage. An unborrowed one stays an SSA value that is rebound.
                if let OperationResult::Structural(home) = slot.1
                    && matches!(block.places[home.index()].origin, crate::PlaceOrigin::Operation(id)
                        if matches!(block.operations[id.index()].kind, crate::OperationKind::EstablishPrimitiveLocal { .. }))
                {
                    let OperationResult::Scalar(value) = value else {
                        return Err(unsupported_node(file, place, "primitive home assignment needs a scalar value"));
                    };
                    if !block.mutable_homes.contains(&home) {
                        return Err(unsupported_node(file, place, "assignment requires a writable local"));
                    }
                    use super::expressions::{EmitOperation, OperationOutput};
                    block.push_operation(
                        OperationOutput::Unit,
                        crate::OperationKind::ProjectedScalarStore {
                            destination: crate::ProjectedPlace { root: home, path: vec![] },
                            value,
                        },
                        site_at(file, place),
                    );
                    continue;
                }
                slot.1 = value;
            }
            (NodeKind::AsmBlock, _) => super::assembly::inline(&expressions, statement, &mut block, &mut flow)?,
            (NodeKind::ExpressionStatement, [expression]) => {
                check_container(tree, statement)?;
                expressions.lower(*expression, &ValueType::NoResult, &mut block, &mut flow)?;
            }
            (NodeKind::Discard, [expression]) => {
                check_container(tree, statement)?;
                let carrier = discarded_carrier(tree, *expression)?;
                if *carrier != ValueType::NoResult && !types.can_forward_value(carrier) && !crate::lowering::types::contains_nominal_value(carrier) {
                    return Err(LoweringError::new(
                        &site_at(file, *expression),
                        "explicit discard requires a supported result carrier",
                    ));
                }
                let result = expressions.lower(*expression, carrier, &mut block, &mut flow)?;
                if let OperationResult::Structural(place) = result
                    && super::ownership::owned(block.places[place.index()].ty, types, data)
                {
                    super::ownership::dispose_temporary(&expressions, &mut block, &mut flow, place, site_at(file, *expression))?;
                }
            }
            (NodeKind::Tail, [expression]) if index + 1 == statements.len() => {
                check_container(tree, statement)?;
                if file.nodes[file.ungroup(*expression)].kind == NodeKind::Match {
                    crate::machines::branches::lower_tail_dispatch(&expressions, *expression, &mut block, &mut flow, result_type)?;
                    flow.finish(&mut block);

                    return Ok(block);
                }
                let value = expressions.lower(*expression, &signature.result, &mut block, &mut flow)?;
                crate::machines::branches::emit_return_block(&mut block, &flow, Some(value), result_type, site_at(file, *expression))?;
                flow.finish(&mut block);

                return Ok(block);
            }
            (NodeKind::Transition, _) if index + 1 == statements.len() => {
                crate::machines::branches::lower_tail_dispatch(&expressions, statement, &mut block, &mut flow, result_type)?;
                flow.finish(&mut block);

                return Ok(block);
            }
            _ => return Err(unsupported_node(file, statement, "unsupported statement")),
        }
    }
    if result_type.is_some() {
        return Err(LoweringError::new(&block.diagnostics.body, "machine body has no result"));
    }
    crate::machines::branches::emit_return_block(&mut block, &flow, None, None, body_site)?;
    flow.finish(&mut block);

    Ok(block)
}

pub(crate) trait MachineBodyValues {
    fn push_value(
        &mut self,
        ty: TypeId,
        origin: ValueOrigin,
    ) -> ValueId;
}
impl MachineBodyValues for MachineBody {
    fn push_value(
        &mut self,
        ty: TypeId,
        origin: ValueOrigin,
    ) -> ValueId {
        let value = ValueId(next_id(self.values.len()));
        self.values.push(Value { ty, origin });
        value
    }
}

/// Binders that some `@name` in this body borrows directly, sorted for lookup.
/// Nested machines have their own bodies and are skipped.
fn directly_borrowed_binders(
    resolved: &omega_resolved::SymbolResolvedTree<'_>,
    file: &IndexedSource<'_>,
    body: usize,
) -> Vec<usize> {
    let mut pending = file.nodes[body].children.clone();
    let mut binders = Vec::new();
    while let Some(node) = pending.pop() {
        if file.nodes[node].kind == NodeKind::Machine {
            continue;
        }
        pending.extend(&file.nodes[node].children);
        if file.nodes[node].kind != NodeKind::Borrow {
            continue;
        }
        if let [operand] = *file.nodes[node].children.as_slice()
            && let Some(Symbol::Binding { binder_node, .. }) = resolved.symbol(file.ungroup(operand))
        {
            binders.push(*binder_node);
        }
    }
    binders.sort_unstable();
    binders.dedup();
    binders
}

/// A borrowed scalar needs addressable storage, so a primitive local or formal
/// that `@name` borrows gets one home for its original value. Every view of it
/// borrows that home, and later reads observe the home. A mutable local's
/// assignments store into the same home. A qualified formal's home owes its
/// declared predicate on that exact initializer.
fn home_borrowed_scalar(
    file: &IndexedSource<'_>,
    types: &LoweringTypes,
    borrowed: &[usize],
    binder: usize,
    value: OperationResult,
    body: &mut MachineBody,
) -> OperationResult {
    let OperationResult::Scalar(initializer) = value else {
        return value;
    };
    let ty = body.values[initializer.index()].ty;
    if borrowed.binary_search(&binder).is_err() {
        return value;
    }
    let receiving_domains = match types.get(ty) {
        Some(crate::TypeShape::PrimitiveScalar(_)) => None,
        Some(crate::TypeShape::QualifiedScalar { domains, .. }) if file.nodes[binder].kind == NodeKind::Param => Some(*domains),
        _ => return value,
    };
    use super::expressions::{EmitOperation, OperationOutput};
    body.push_operation(
        OperationOutput::Structural(ty),
        crate::OperationKind::EstablishPrimitiveLocal { value: initializer, receiving_domains },
        site_at(file, binder),
    )
}

pub(crate) fn next_id(current_count: usize) -> NonZeroUsize {
    NonZeroUsize::new(current_count + 1).expect("vector length leaves room for a one-based identity")
}

fn binding_carrier<'a>(
    tree: &'a LoweringTree<'_>,
    node: usize,
) -> Result<&'a ValueType, LoweringError> {
    let mut carrier = None;
    for record in tree.semantics_at(node) {
        match record {
            NodeSemantics::Expression(e) if e.static_argument_slot.is_none() && carrier.is_none_or(|previous| previous == &e.ty) => carrier = Some(&e.ty),
            _ => return Err(unsupported_node(tree.resolved().file, node, "unsupported binding record")),
        }
    }
    carrier.ok_or_else(|| unsupported_node(tree.resolved().file, node, "binding has no carrier"))
}

/// The discarded operand's carrier is its own typed expression record. Other
/// records on the operand (calls, obligations, landings) are checked by the
/// expression lowering that consumes them.
fn discarded_carrier<'a>(
    tree: &'a LoweringTree<'_>,
    node: usize,
) -> Result<&'a ValueType, LoweringError> {
    let file = tree.resolved().file;
    let mut carrier = None;
    for record in tree.semantics_at(node) {
        if let NodeSemantics::Expression(e) = record {
            if e.static_argument_slot.is_some() || carrier.is_some_and(|previous| previous != &e.ty) {
                return Err(unsupported_node(file, node, "discarded operand has conflicting typed carriers"));
            }
            carrier = Some(&e.ty);
        }
    }
    carrier.ok_or_else(|| unsupported_node(file, node, "discarded operand has no typed carrier"))
}

pub(crate) fn check_container(
    tree: &LoweringTree<'_>,
    node: usize,
) -> Result<(), LoweringError> {
    if tree.semantics_at(node).next().is_some() {
        return Err(unsupported_node(tree.resolved().file, node, "unsupported typed body record"));
    }
    Ok(())
}

#[cfg_attr(feature = "diagnostic-origins", track_caller)]
pub(crate) fn unsupported_node(
    file: &IndexedSource<'_>,
    node: usize,
    reason: &str,
) -> LoweringError {
    LoweringError::new(&site_at(file, node), format!("{reason}: {:?}", file.nodes[node].kind))
}

pub(crate) fn site_at(
    file: &IndexedSource<'_>,
    node: usize,
) -> Site {
    Site {
        package: file.package.to_owned(),
        file: file.path.to_owned(),
        span: file.span(node),
    }
}
