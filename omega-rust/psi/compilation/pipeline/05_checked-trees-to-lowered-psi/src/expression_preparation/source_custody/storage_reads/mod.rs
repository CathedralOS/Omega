//! Rejoin scalar reads with authored bindings, storage, and operand positions.

use super::super::{CheckedBooleanExpression, CheckedScalarExpression, Multiplicity};
use super::{
    CheckedScalarExpressionRole, CheckedTrees, ExpressionHandle, ExpressionNode, LoweringError,
    PrimitiveType, SourceRoot, StatementNode, authored_state, supported_mutable_parameter,
    unsupported,
};
pub(super) mod case_membership;

#[cfg(test)]
mod tests;

/// Reads retain their occurrence, operand position, and exact binding or place.
/// Unary and cast wrappers do not change the read's identity. Greater-than
/// comparisons use the same operand normalization as the checked producer.
pub(super) fn validate(
    checked: &CheckedTrees,
    binding: &checked_trees::CheckedScalarExpressionBindings,
    source: &SourceRoot,
) -> Result<(), LoweringError> {
    let (_, retained) = checked
        .facts
        .values
        .scalar_expressions
        .bound_expression_at(binding.state, binding.statement_ordinal, binding.role)
        .ok_or(LoweringError::Unsupported(
            "storage read has no exact scalar expression",
        ))?;
    let expression = if binding.role == CheckedScalarExpressionRole::Guard {
        guard_subject(checked, source.expression)
    } else {
        source.expression
    };
    validate_expression(
        checked,
        binding.state,
        binding.statement_ordinal,
        expression,
        retained,
    )
}

/// Rejoin fixed-integer and Boolean reads to their exact source scope.
/// Computation operands use this same check without inventing a pure-plan row.
pub(crate) fn validate_expression(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    statement: u32,
    expression: ExpressionHandle,
    retained: &CheckedScalarExpression,
) -> Result<(), LoweringError> {
    validate_reads(
        checked,
        state,
        ReadScope::Body(statement),
        expression,
        retained,
    )
}

/// Invocation requirements read parameter entry values, including mutable formals.
/// This checks read identity; the caller separately checks predicate meaning.
pub(crate) fn validate_entry_read_expression(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    expression: ExpressionHandle,
    retained: &CheckedScalarExpression,
) -> Result<(), LoweringError> {
    validate_reads(checked, state, ReadScope::Entry, expression, retained)
}

/// Normal guarantees read immutable scalar formals, structural post-state places,
/// and the exact contract-owned result when one exists. A Unit return has no
/// scalar result slot. Mutable scalar snapshots cannot impersonate post-state.
pub(crate) fn validate_normal_result_read_expression(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    expression: ExpressionHandle,
    retained: &CheckedScalarExpression,
) -> Result<(), LoweringError> {
    let (machine, source_state) = authored_state(checked, state)?;
    let primitive = checked
        .primitive_type_reference(source_state.return_type)
        .filter(|primitive| supported_mutable_parameter(*primitive));
    validate_reads(
        checked,
        state,
        ReadScope::NormalResult {
            machine: machine.symbol,
            return_type: source_state.return_type,
            primitive,
        },
        expression,
        retained,
    )
}

#[derive(Clone, Copy)]
enum ReadScope {
    Body(u32),
    Entry,
    NormalResult {
        machine: symbols::SymbolHandle,
        return_type: checked_trees::types::TypeReferenceHandle,
        primitive: Option<PrimitiveType>,
    },
}

impl ReadScope {
    fn preceding_statements(self) -> u32 {
        match self {
            Self::Body(statement) => statement,
            Self::Entry | Self::NormalResult { .. } => 0,
        }
    }
}

fn validate_reads(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    scope: ReadScope,
    expression: ExpressionHandle,
    retained: &CheckedScalarExpression,
) -> Result<(), LoweringError> {
    let (machine, state) = authored_state(checked, state)?;
    let mut authored_reads = Vec::new();
    let mut member_paths = Vec::new();
    let mut unmatched_member_paths = Vec::new();
    collect_authored_storage_reads(
        checked,
        state,
        scope,
        expression,
        &mut Vec::new(),
        &mut Vec::new(),
        &mut authored_reads,
        &mut member_paths,
        &mut unmatched_member_paths,
    )?;
    if matches!(scope, ReadScope::Entry) {
        // Both direct owned fields and projected borrowed fields must name
        // one runtime invocation root before their retained positions rejoin.
        for (_, root, _, kind) in &authored_reads {
            if !matches!(
                kind,
                ReadKind::OwnedField(_) | ReadKind::IndexedPrimitive(_)
            ) {
                continue;
            }
            let mut parameters = checked
                .state_parameters(state)
                .iter()
                .filter(|parameter| parameter.symbol == *root);
            let Some(parameter) = parameters.next() else {
                return unsupported("entry field read has no authored invocation parameter");
            };
            if !root.is_valid() || parameter.relevance.is_erased() || parameters.next().is_some() {
                return unsupported(
                    "entry field read requires one non-erased invocation parameter",
                );
            }
        }
    }
    let namespace = checked
        .state_parameters(state)
        .iter()
        .filter(|parameter| {
            // An `[erased]` binding occupies no position in this namespace.
            !parameter.relevance.is_erased()
                && checked
                    .primitive_type_reference(parameter.type_reference)
                    .is_some()
        })
        .map(|parameter| parameter.symbol)
        .chain(
            checked
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .take(scope.preceding_statements() as usize)
                .filter_map(|statement| match statement {
                    StatementNode::LocalData(local)
                        if !local.is_mutable
                            && local.initial_value.is_valid()
                            && checked
                                .primitive_type_reference(local.type_reference)
                                .is_some() =>
                    {
                        Some(local.symbol)
                    }
                    _ => None,
                }),
        )
        .collect::<Vec<_>>();
    let mut namespace = ReadNamespace {
        scope,
        checked,
        state,
        machine: machine.symbol,
        scalar: namespace,
        erased_scalar: checked
            .state_parameters(state)
            .iter()
            .filter(|parameter| {
                parameter.relevance.is_erased()
                    && checked
                        .primitive_type_reference(parameter.type_reference)
                        .is_some()
            })
            .map(|parameter| parameter.symbol)
            .collect(),
        structural: checked.state_parameters(state),
        owned_field_paths: member_paths,
        unmatched_member_paths,
        owned: checked
            .state_parameters(state)
            .iter()
            .map(|parameter| {
                owned_record(checked, parameter)
                    .map(|_| parameter.symbol)
                    .unwrap_or_default()
            })
            .collect(),
    };
    let mut retained_reads = Vec::new();
    let mut needs_value_replay = false;
    collect_scalar_storage_reads(
        retained,
        &mut namespace,
        &mut Vec::new(),
        &mut retained_reads,
        &mut needs_value_replay,
    );
    if retained_reads != authored_reads
        && !super::value_correspondence::matches_expanded_byte_comparison(
            checked,
            state.symbol,
            scope.preceding_statements(),
            expression,
            retained,
        )
    {
        return unsupported("scalar read differs from its authored binding or mutable place");
    }
    // Account for every authored value member, including receivers changed into
    // a different expression kind. A literal projection also needs the enclosing
    // scalar meaning replayed: leaf equality alone cannot justify replacing
    // addition with multiplication or collapsing the complete computation.
    if !namespace.unmatched_member_paths.is_empty() {
        return unsupported("scalar value member has no corresponding retained literal");
    }
    if needs_value_replay {
        let primitive = retained.primitive_type().ok_or(LoweringError::Unsupported(
            "projected scalar expression has no declared carrier",
        ))?;
        super::value_correspondence::validate(
            checked,
            state.symbol,
            scope.preceding_statements(),
            expression,
            primitive,
            &checked_trees::CheckedCallScalarArgument::Pure(retained.clone()),
        )?;
    }
    Ok(())
}

/// Guard lowering erases the builtin `subject == true` wrapper, not its subject.
pub(super) fn guard_subject(
    checked: &CheckedTrees,
    expression: ExpressionHandle,
) -> ExpressionHandle {
    let ExpressionNode::Binary(binary) = checked.expression_table.expression(expression) else {
        return expression;
    };
    if binary.operator != checked_trees::expression::BinaryOperator::Equal
        || checked
            .facts
            .operators
            .expression_use(expression)
            .is_some_and(|operator| {
                operator.status != checked_trees::CheckedOperatorResolutionStatus::BuiltinFallback
            })
    {
        return expression;
    }
    match (
        checked.expression_table.expression(binary.left),
        checked.expression_table.expression(binary.right),
    ) {
        (ExpressionNode::Boolean(true), _) => binary.right,
        (_, ExpressionNode::Boolean(true)) => binary.left,
        _ => expression,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ReadKind {
    Storage,
    Parameter,
    Local,
    Result,
    OwnedField(Vec<checked_trees::CheckedStructuralPredicatePathSegment>),
    IndexedPrimitive(Vec<checked_trees::CheckedStructuralPredicatePathSegment>),
    CaseMembership {
        path: Vec<checked_trees::CheckedStructuralPredicatePathSegment>,
        case: String,
    },
}

type StorageReadOccurrence = (Vec<usize>, symbols::SymbolHandle, PrimitiveType, ReadKind);

struct ReadNamespace<'checked> {
    scope: ReadScope,
    checked: &'checked CheckedTrees,
    state: &'checked checked_trees::state::State,
    machine: symbols::SymbolHandle,
    scalar: Vec<symbols::SymbolHandle>,
    /// Proof-only erased formals in their dense roster order.
    erased_scalar: Vec<symbols::SymbolHandle>,
    structural: &'checked [checked_trees::signature::StateParameter],
    owned_field_paths: Vec<Vec<usize>>,
    unmatched_member_paths: Vec<Vec<usize>>,
    // Structural positions are authored positions, including scalar formals.
    owned: Vec<symbols::SymbolHandle>,
}

impl ReadNamespace<'_> {
    /// The place a view local's establishment published, in the predicate
    /// path spelling the authored read walk records. A prefix carrying a
    /// runtime selector spells no fixed path identity.
    fn view_local_place(
        &self,
        symbol: symbols::SymbolHandle,
    ) -> Option<(
        symbols::SymbolHandle,
        Vec<checked_trees::CheckedStructuralPredicatePathSegment>,
    )> {
        let (root, path) =
            crate::emission::structural_scalar_store_source::local_store_destination(
                self.checked,
                self.machine,
                self.state,
                self.state.symbol,
                self.scope.preceding_statements(),
                symbol,
            )
            .ok()?;
        // Only a local whose initializer loans another place rejoins that
        // place; a marker local keeps its own root and no prefix, where the
        // authored side likewise records no projected leaf.
        if root == symbol && path.is_empty() {
            return None;
        }
        let path = path
            .iter()
            .map(|segment| match segment {
                checked_trees::CheckedUnitStructuralPathSegment::Field(identity) => Some(
                    checked_trees::CheckedStructuralPredicatePathSegment::Field(identity.clone()),
                ),
                checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(index) => {
                    Some(checked_trees::CheckedStructuralPredicatePathSegment::FixedIndex(*index))
                }
                _ => None,
            })
            .collect::<Option<Vec<_>>>()?;
        Some((root, path))
    }

    fn scalar_read(
        &self,
        position: usize,
        primitive: PrimitiveType,
        is_parameter: bool,
    ) -> (symbols::SymbolHandle, ReadKind) {
        if let ReadScope::NormalResult {
            machine,
            primitive: result_primitive,
            ..
        } = self.scope
            && is_parameter
            && position == self.scalar.len()
            && Some(primitive) == result_primitive
        {
            return (machine, ReadKind::Result);
        }
        (
            self.scalar.get(position).copied().unwrap_or_default(),
            if is_parameter {
                ReadKind::Parameter
            } else {
                ReadKind::Local
            },
        )
    }
}

fn owned_record<'checked>(
    checked: &'checked CheckedTrees,
    parameter: &checked_trees::signature::StateParameter,
) -> Option<&'checked checked_trees::data::DataDefinition> {
    if parameter.is_self
        || parameter.is_const
        || parameter.is_mutable
        || !parameter.symbol.is_valid()
        || !matches!(
            checked.type_multiplicity(parameter.type_reference),
            Multiplicity::Affine | Multiplicity::Unrestricted
        )
    {
        return None;
    }
    let checked_trees::types::TypeReferenceNode::Named { symbol, .. } = checked
        .type_reference_table
        .type_reference(parameter.type_reference)
    else {
        return None;
    };
    if !symbol.is_valid() || checked.symbols.get(*symbol).kind != symbols::SymbolKind::Data {
        return None;
    }
    let mut definitions = checked
        .data_definitions()
        .iter()
        .filter(|data| data.symbol == *symbol);
    let definition = definitions.next()?;
    if definitions.next().is_some()
        || checked
            .data_members(definition)
            .iter()
            .any(|member| matches!(member, checked_trees::data::DataMember::Variant(_)))
    {
        return None;
    }
    Some(definition)
}

fn authored_owned_field(
    checked: &CheckedTrees,
    state: &checked_trees::state::State,
    expression: ExpressionHandle,
) -> Result<Option<(symbols::SymbolHandle, PrimitiveType, ReadKind)>, LoweringError> {
    let ExpressionNode::Member(member) = checked.expression_table.expression(expression) else {
        return Ok(None);
    };
    if member.case_variant.is_some() {
        return Ok(None);
    }
    let ExpressionNode::Name(name) = checked.expression_table.expression(member.receiver) else {
        return Ok(None);
    };
    let mut parameters = checked
        .state_parameters(state)
        .iter()
        .filter(|parameter| parameter.symbol == name.symbol);
    let Some(parameter) = parameters.next() else {
        return Ok(None);
    };
    let Some(owner) = owned_record(checked, parameter) else {
        return Ok(None);
    };
    if parameters.next().is_some()
        || name.head_symbol != parameter.symbol
        || !matches!(checked.expression_table.name_path_members(name.members),
            [spelling] if spelling.as_str() == parameter.name.as_str())
        || !member.member_symbol.is_valid()
    {
        return unsupported("owned scalar field has no exact authored parameter or member");
    }
    let mut fields = checked
        .data_members(owner)
        .iter()
        .filter_map(|candidate| match candidate {
            checked_trees::data::DataMember::Field(field)
                if field.symbol == member.member_symbol =>
            {
                Some(field)
            }
            _ => None,
        });
    let field = fields.next().ok_or(LoweringError::Unsupported(
        "owned scalar field belongs to another record",
    ))?;
    if fields.next().is_some()
        || field.name != member.member
        || checked.symbols.get(field.symbol).kind != symbols::SymbolKind::Field
        || checked.symbols.get(field.symbol).parent != owner.symbol
    {
        return unsupported("owned scalar field differs from its resolved declaration");
    }
    let Some(primitive) = checked
        .primitive_type_reference(field.type_reference)
        .filter(|primitive| supported_mutable_parameter(*primitive))
    else {
        return Ok(None);
    };
    if field.relevance.is_erased() || checked.data_members(owner).iter().filter(|candidate| {
        matches!(candidate, checked_trees::data::DataMember::Field(candidate)
            if candidate.name == field.name || field.identity.is_some() && candidate.identity == field.identity)
    }).count() != 1 {
        return unsupported("owned scalar field has no unique relevant identity");
    }
    let (machine, _) = authored_state(checked, state.symbol)?;
    if validation::declared_place_type_raw(&checked.typed, machine, Some(state), expression)
        .and_then(|reference| checked.primitive_type_reference(reference))
        != Some(primitive)
    {
        return unsupported("owned scalar field differs from its declared place type");
    }
    let identity = field.path_identity();
    Ok(Some((
        parameter.symbol,
        primitive,
        ReadKind::OwnedField(vec![
            checked_trees::CheckedStructuralPredicatePathSegment::Field(identity),
        ]),
    )))
}

/// The authored place a literal-indexed scalar read rejoins. `bytes[3]` and
/// `maps[1].value` resolve through the same projected receiver walk: a member
/// leaf above a fixed index carries the same `IndexedPrimitive` read the
/// retained `StructuralParameterField` reports. Paths without a fixed index
/// keep their existing member/storage owners, except invocation requirements,
/// whose complete parameter field paths are rejoined here as entry reads.
fn authored_indexed_primitive_read(
    checked: &CheckedTrees,
    state: &checked_trees::state::State,
    scope: ReadScope,
    machine: &checked_trees::machine::Machine,
    expression: ExpressionHandle,
) -> Result<Option<(symbols::SymbolHandle, PrimitiveType, ReadKind)>, LoweringError> {
    let Some(primitive) =
        validation::declared_place_type_raw(&checked.typed, machine, Some(state), expression)
            .and_then(|reference| {
                // The retained leaf carries the base primitive whether or not
                // its declared type keeps a range or policy qualification, so
                // the authored side reads the same primitive.
                validation::unrestricted_builtin_primitive(&checked.typed, reference)
                    .or_else(|| checked.typed.primitive_type_reference(reference))
            })
            // `collect_owned_field` reports an indexed or entry leaf for every
            // primitive the retained `StructuralParameterField` carries,
            // IEEE floats included (`self.nums[0]` over `[f64; 6]`), so the
            // authored side must admit the same carriers or the two disagree.
            .filter(|primitive| {
                supported_mutable_parameter(*primitive)
                    || matches!(primitive, PrimitiveType::F32 | PrimitiveType::F64)
            })
    else {
        return Ok(None);
    };
    // Bounded fields and borrowed owners retain selector-only reads. A whole
    // owned local instead retains its exact binding and literal index, just
    // like other owned collections; its encoding qualification does not erase
    // that source identity. Bounds still use the separately proved live length.
    if let ExpressionNode::Indexed(indexed) = checked.expression_table.expression(expression)
        && validation::declared_place_type_raw(
            &checked.typed,
            machine,
            Some(state),
            indexed.collection,
        )
        .and_then(|collection| {
            validation::bounded_byte_buffer_capacity(&checked.typed, collection).or_else(|| {
                validation::loaned_bounded_byte_buffer_capacity(&checked.typed, collection)
            })
        })
        .is_some()
        && !matches!(
            checked.expression_table.expression(indexed.collection),
            ExpressionNode::Name(name)
                if fixed_carrier_local(checked, state, scope, name.symbol).is_some()
        )
    {
        return Ok(None);
    }
    let Some((root, field_path)) =
        authored_read_path(checked, state, scope, machine, expression, 0)?
    else {
        return Ok(None);
    };
    let indexed = field_path.iter().any(|segment| {
        matches!(
            segment,
            checked_trees::CheckedStructuralPredicatePathSegment::FixedIndex(_)
        )
    });
    if !indexed && !matches!(scope, ReadScope::Entry) {
        return Ok(None);
    }
    Ok(Some((
        root,
        primitive,
        if indexed {
            ReadKind::IndexedPrimitive(field_path)
        } else {
            ReadKind::OwnedField(field_path)
        },
    )))
}

/// Read observations preserve the shared place replayer's exact root and
/// projections. A retained case selects the existing case-payload observation
/// and copy operations, whose emission separately requires live membership;
/// this source-path join does not admit Case in primitive read or store paths.
fn authored_read_path(
    checked: &CheckedTrees,
    state: &checked_trees::state::State,
    scope: ReadScope,
    machine: &checked_trees::machine::Machine,
    expression: ExpressionHandle,
    depth: usize,
) -> Result<
    Option<(
        symbols::SymbolHandle,
        Vec<checked_trees::CheckedStructuralPredicatePathSegment>,
    )>,
    LoweringError,
> {
    use checked_trees::CheckedStructuralPredicatePathSegment as Segment;
    if depth >= 128 || !checked.expression_table.expression_is_valid(expression) {
        return unsupported("scalar read has a stale or cyclic projection");
    }
    // A store resolves `entries[0].value` through a borrowed view, but a read
    // of a view element retains only its selector, as `fixed_carrier_local`
    // records for view locals: it mints no projected storage leaf.
    if indexes_borrowed_view(checked, state, machine, expression) {
        return Ok(None);
    }
    if let Ok(source) = crate::emission::call_source_custody::projected_receivers::store_destination(
        checked,
        machine.symbol,
        state.symbol,
        Some(scope.preceding_statements() as usize),
        expression,
    ) {
        // Runtime selectors are expression reads, not fixed path identity.
        // Leave them to the indexed-expression walk below, which compares
        // their exact binding/operand position against the retained selector.
        // The ordinary indexed-read lowering still proves carrier and bounds.
        if source.path.iter().any(|segment| {
            matches!(
                segment,
                checked_trees::CheckedUnitStructuralPathSegment::RuntimeIndex(_)
            )
        }) {
            return Ok(None);
        }
        let path = source
            .path
            .iter()
            .map(|segment| match segment {
                checked_trees::CheckedUnitStructuralPathSegment::Field(identity) => {
                    Ok(Segment::Field(identity.clone()))
                }
                checked_trees::CheckedUnitStructuralPathSegment::Case(identity) => {
                    Ok(Segment::Case(identity.clone()))
                }
                checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(index) => {
                    Ok(Segment::FixedIndex(*index))
                }
                _ => unsupported("indexed primitive read has an unsupported projection"),
            })
            .collect::<Result<Vec<_>, _>>()?;
        // Keep root-to-leaf traversal order for every storage owner. Moving
        // a local's first index behind its fields or later indices changes
        // which element is read (`grid[1][0]` is not `grid[0][1]`).
        // A view-typed local roots its element reads at the view its
        // establishment published (`CheckedStorageRoot::ViewLocal`), whose
        // retained form is selector-only on either access family: a view is
        // a runtime-extent re-view, not an owned carrier, so no
        // `IndexedPrimitive` leaf rejoins even though the mutable family's
        // store-destination custody resolves here.
        if view_typed_local(checked, state, scope, source.root) {
            return Ok(None);
        }
        return Ok(Some((source.root, path)));
    }
    match checked.expression_table.expression(expression) {
        // A literal element of a local fixed carrier is a fixed projected
        // read rooted at the local binding itself (`rooms[0].exit_count`):
        // the checker's `StructuralParameterIndexedRead` retains exactly this
        // occurrence on its `OwnedLocal` root, so the authored path must
        // produce the same root and segments.
        ExpressionNode::Indexed(indexed) => {
            let Some((root, mut path)) = authored_read_path(
                checked,
                state,
                scope,
                machine,
                indexed.collection,
                depth + 1,
            )?
            else {
                return Ok(None);
            };
            // A projected aggregate's element retains the whole storage path,
            // including any case/field coordinates already established above.
            // Only a bare parameter collection uses selector-only custody;
            // local fixed carriers retain their binding even with no prefix.
            if path.is_empty() && fixed_carrier_local(checked, state, scope, root).is_none() {
                return Ok(None);
            }
            let Some(index) = constant_index_u64(checked, indexed.index) else {
                return Ok(None);
            };
            path.push(Segment::FixedIndex(index));
            return Ok(Some((root, path)));
        }
        ExpressionNode::Name(name) => {
            if !name.symbol.is_valid()
                || name.symbol != name.head_symbol
                || checked
                    .expression_table
                    .name_path_members(name.members)
                    .len()
                    != 1
            {
                return Ok(None);
            }
            let Some(symbol) = fixed_carrier_local(checked, state, scope, name.symbol) else {
                return Ok(None);
            };
            return Ok(Some((symbol, Vec::new())));
        }
        _ => {}
    }
    let ExpressionNode::Member(member) = checked.expression_table.expression(expression) else {
        return Ok(None);
    };
    let Some((root, mut path)) =
        authored_read_path(checked, state, scope, machine, member.receiver, depth + 1)?
    else {
        return Ok(None);
    };
    let Some(reference) =
        validation::declared_place_type_raw(&checked.typed, machine, Some(state), member.receiver)
            .and_then(|reference| validation::unwrapped_type_reference(&checked.typed, reference))
    else {
        return Ok(None);
    };
    let symbol = match checked.type_reference_table.type_reference(reference) {
        checked_trees::types::TypeReferenceNode::Named { symbol, .. } => *symbol,
        checked_trees::types::TypeReferenceNode::Generic { base_symbol, .. } => *base_symbol,
        _ => return Ok(None),
    };
    let Some(owner) = checked
        .data_definitions()
        .iter()
        .find(|owner| symbol.is_valid() && owner.symbol == symbol)
    else {
        return Ok(None);
    };
    let Some(field) = validation::exact_data_member_field(
        &checked.typed,
        owner,
        member.member_symbol,
        member.member.as_str(),
        member.case_variant.as_ref().map(|case| case.as_str()),
    )
    .filter(|field| !field.relevance.is_erased()) else {
        return Ok(None);
    };
    // A local element's selector retains after the member projections through
    // it (`local[i].member` keeps `[member..., FixedIndex(i)]`), so a member
    // read reaching into a fixed element inserts before the selector tail.
    let mut insert_at = path.len();
    if matches!(path.last(), Some(Segment::FixedIndex(_)))
        && fixed_carrier_local(checked, state, scope, root).is_some()
    {
        insert_at -= 1;
    }
    if let Some(case_name) = &member.case_variant {
        let Some(case) = checked
            .data_members(owner)
            .iter()
            .find_map(|member| match member {
                checked_trees::data::DataMember::Variant(case) if case.name == *case_name => {
                    Some(case)
                }
                _ => None,
            })
        else {
            return Ok(None);
        };
        path.insert(insert_at, Segment::Case(case.path_identity()));
        insert_at += 1;
    }
    path.insert(insert_at, Segment::Field(field.path_identity()));
    Ok(Some((root, path)))
}

/// The constant ordinal a literal (or constant-folded) element read denotes,
/// the same fold the checker applies to authored indices.
fn constant_index_u64(checked: &CheckedTrees, index: ExpressionHandle) -> Option<u64> {
    match checked.expression_table.expression(index) {
        ExpressionNode::Integer(literal) => literal.value_bignum().and_then(|value| value.to_u64()),
        _ => checked
            .expression_table
            .constant_integer_value(index)
            .and_then(|value| u64::try_from(value).ok()),
    }
}

/// Whether a member/index place selects an element of a borrowed `&[T]`
/// view anywhere along its chain, as the projected receiver walk types each
/// indexed collection.
fn indexes_borrowed_view(
    checked: &CheckedTrees,
    state: &checked_trees::state::State,
    machine: &checked_trees::machine::Machine,
    mut expression: ExpressionHandle,
) -> bool {
    loop {
        match checked.expression_table.expression(expression) {
            ExpressionNode::Member(member) => expression = member.receiver,
            ExpressionNode::Indexed(indexed) => {
                if validation::declared_place_type_raw(
                    &checked.typed,
                    machine,
                    Some(state),
                    indexed.collection,
                )
                .and_then(|reference| {
                    validation::unwrapped_type_reference(&checked.typed, reference)
                })
                .is_some_and(|reference| {
                    matches!(
                        checked.type_reference_table.type_reference(reference),
                        checked_trees::types::TypeReferenceNode::Slice { .. }
                    )
                }) {
                    return true;
                }
                expression = indexed.collection;
            }
            _ => return false,
        }
    }
}

/// A local binding declared as an owned fixed array: its own symbol roots
/// element reads, unlike scalar locals which rejoin through
/// `authored_storage_read`. This mirrors the checker's owned-local root, the
/// only root whose literal element reads retain an `IndexedPrimitive` leaf. A
/// borrowed `&[T]` view local roots its reads at the view its establishment
/// published (`CheckedStorageRoot::ViewLocal`), which retains only the
/// selector, so it must not mint an owned leaf here.
fn fixed_carrier_local(
    checked: &CheckedTrees,
    state: &checked_trees::state::State,
    scope: ReadScope,
    symbol: symbols::SymbolHandle,
) -> Option<symbols::SymbolHandle> {
    let mut locals = checked
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .take(scope.preceding_statements() as usize)
        .filter_map(|statement| match statement {
            StatementNode::LocalData(local) if local.symbol == symbol => Some(local),
            _ => None,
        });
    let local = locals.next()?;
    if locals.next().is_some() {
        return None;
    }
    // Qualifications retain the owned carrier. A reference does not: borrowed
    // collections must keep the selector-only read custody above.
    let mut reference = local.type_reference;
    loop {
        match checked.type_reference_table.type_reference(reference) {
            checked_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
                reference = *base_type;
            }
            checked_trees::types::TypeReferenceNode::FixedArray { .. } => {
                return Some(local.symbol);
            }
            _ => return None,
        }
    }
}

/// A local binding declared as a borrowed slice view of either access
/// family. Element reads through one root at the view's published place, so
/// the projected-receiver walk's store-destination root never mints an
/// owned-carrier leaf for them. Mirrors the checker's view-local root, which
/// admits the shared and mutable families alike.
fn view_typed_local(
    checked: &CheckedTrees,
    state: &checked_trees::state::State,
    scope: ReadScope,
    symbol: symbols::SymbolHandle,
) -> bool {
    let mut locals = checked
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .take(scope.preceding_statements() as usize)
        .filter_map(|statement| match statement {
            StatementNode::LocalData(local) if local.symbol == symbol => Some(local),
            _ => None,
        });
    let Some(local) = locals.next() else {
        return false;
    };
    if locals.next().is_some() {
        return false;
    }
    let mut type_reference = local.type_reference;
    loop {
        match checked.type_reference_table.type_reference(type_reference) {
            checked_trees::types::TypeReferenceNode::Constrained { base_type, .. }
            | checked_trees::types::TypeReferenceNode::Reference {
                referee: base_type, ..
            } => type_reference = *base_type,
            checked_trees::types::TypeReferenceNode::Slice { .. } => return true,
            _ => return false,
        }
    }
}

fn collect_owned_field(
    parameter_position: u32,
    field_path: &[checked_trees::CheckedStructuralPredicatePathSegment],
    primitive: PrimitiveType,
    namespace: &ReadNamespace,
    path: &[usize],
    reads: &mut Vec<StorageReadOccurrence>,
) {
    if field_path.iter().any(|segment| {
        matches!(
            segment,
            checked_trees::CheckedStructuralPredicatePathSegment::FixedIndex(_)
        )
    }) {
        reads.push((
            path.to_vec(),
            namespace
                .structural
                .get(parameter_position as usize)
                .map(|parameter| parameter.symbol)
                .unwrap_or_default(),
            primitive,
            ReadKind::IndexedPrimitive(field_path.to_vec()),
        ));
        return;
    }
    if matches!(namespace.scope, ReadScope::Entry) {
        reads.push((
            path.to_vec(),
            namespace
                .structural
                .get(parameter_position as usize)
                .map(|parameter| parameter.symbol)
                .unwrap_or_default(),
            primitive,
            ReadKind::OwnedField(field_path.to_vec()),
        ));
        return;
    }
    // Nested, indexed, receiver and borrowed projections retain their existing owners.
    if !supported_mutable_parameter(primitive)
        || !matches!(field_path, [checked_trees::CheckedStructuralPredicatePathSegment::Field(_)])
        // Match members present in typed source, including equality expanded
        // during typing. Later plan-only expansion has no source member here.
        || !namespace.owned_field_paths.iter().any(|authored| authored == path)
    {
        return;
    }
    if let Some(symbol) = namespace
        .owned
        .get(parameter_position as usize)
        .filter(|symbol| symbol.is_valid())
    {
        reads.push((
            path.to_vec(),
            *symbol,
            primitive,
            ReadKind::OwnedField(field_path.to_vec()),
        ));
    }
}

/// A `let b: &T = &place` local names its captured place at read
/// position, not its own carrier. Peel the (possibly recast)
/// `Borrow{Shared}` initializer to the place expression the checker's
/// scalar operands re-evaluate.
fn shared_loan_target(
    checked: &CheckedTrees,
    state: &checked_trees::state::State,
    scope: ReadScope,
    name: &checked_trees::expression::TableNamePath,
) -> Option<ExpressionHandle> {
    if !name.symbol.is_valid()
        || name.symbol != name.head_symbol
        || checked
            .expression_table
            .name_path_members(name.members)
            .len()
            != 1
    {
        return None;
    }
    let local = checked
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .take(scope.preceding_statements() as usize)
        .filter_map(|statement| match statement {
            StatementNode::LocalData(local) if local.symbol == name.symbol => Some(local),
            _ => None,
        })
        .next()?;
    let mut initializer = local.initial_value;
    loop {
        match checked.expression_table.expression(initializer) {
            ExpressionNode::Borrow(borrow)
                if borrow.access == language_core::ReferenceAccess::Shared =>
            {
                return Some(borrow.target);
            }
            ExpressionNode::Cast(cast) if cast.form.is_recast() => {
                initializer = cast.value;
            }
            _ => return None,
        }
    }
}

fn authored_storage_read(
    checked: &CheckedTrees,
    state: &checked_trees::state::State,
    scope: ReadScope,
    name: &checked_trees::expression::TableNamePath,
) -> Result<Option<(symbols::SymbolHandle, PrimitiveType, ReadKind)>, LoweringError> {
    if !name.symbol.is_valid()
        || name.symbol != name.head_symbol
        || checked
            .expression_table
            .name_path_members(name.members)
            .len()
            != 1
    {
        if matches!(scope, ReadScope::Entry | ReadScope::NormalResult { .. }) {
            return unsupported("entry predicate read has no exact parameter identity");
        }
        return Ok(None);
    }
    let mut locals = checked
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .take(scope.preceding_statements() as usize)
        .filter_map(|statement| match statement {
            StatementNode::LocalData(local) if local.symbol == name.symbol => Some(local),
            _ => None,
        });
    if let Some(local) = locals.next() {
        if locals.next().is_some() {
            return unsupported("scalar storage read has duplicate authored declarations");
        }
        return Ok(checked
            .primitive_type_reference(local.type_reference)
            .filter(|primitive| local.is_mutable || supported_mutable_parameter(*primitive))
            .map(|primitive| {
                (
                    local.symbol,
                    primitive,
                    if local.is_mutable {
                        ReadKind::Storage
                    } else {
                        ReadKind::Local
                    },
                )
            }));
    }
    let mut parameters = checked
        .state_parameters(state)
        .iter()
        .filter(|parameter| parameter.symbol == name.symbol);
    let Some(parameter) = parameters.next() else {
        if matches!(scope, ReadScope::Entry | ReadScope::NormalResult { .. }) {
            return unsupported("entry predicate read is not an invocation parameter");
        }
        return Ok(None);
    };
    if parameters.next().is_some() {
        return unsupported("scalar storage read has duplicate authored parameters");
    }
    if matches!(scope, ReadScope::NormalResult { .. }) && parameter.is_mutable {
        return unsupported(
            "normal predicate cannot replace mutable post-state with entry storage",
        );
    }
    if matches!(scope, ReadScope::Entry | ReadScope::NormalResult { .. }) {
        let primitive = checked
            .primitive_type_reference(parameter.type_reference)
            .filter(|primitive| supported_mutable_parameter(*primitive))
            .ok_or(LoweringError::Unsupported(
                "entry predicate read requires a fixed integer or Boolean parameter",
            ))?;
        return Ok(Some((parameter.symbol, primitive, ReadKind::Parameter)));
    }
    if let Some((primitive, _)) = super::primitive_references::parameter_type(checked, parameter) {
        return Ok(Some((parameter.symbol, primitive, ReadKind::Storage)));
    }
    Ok(checked
        .primitive_type_reference(parameter.type_reference)
        .filter(|primitive| parameter.is_mutable || supported_mutable_parameter(*primitive))
        .map(|primitive| {
            (
                parameter.symbol,
                primitive,
                if parameter.is_mutable {
                    ReadKind::Storage
                } else {
                    ReadKind::Parameter
                },
            )
        }))
}

fn collect_authored_storage_reads(
    checked: &CheckedTrees,
    state: &checked_trees::state::State,
    scope: ReadScope,
    expression: ExpressionHandle,
    path: &mut Vec<usize>,
    active: &mut Vec<ExpressionHandle>,
    reads: &mut Vec<StorageReadOccurrence>,
    member_paths: &mut Vec<Vec<usize>>,
    unmatched_member_paths: &mut Vec<Vec<usize>>,
) -> Result<(), LoweringError> {
    use checked_trees::expression::BinaryOperator;
    if !checked.expression_table.expression_is_valid(expression) || active.contains(&expression) {
        return unsupported("scalar storage source contains a stale or cyclic expression");
    }
    active.push(expression);
    if let Some((_, operand)) = validation::integer_embedding_argument(&checked.typed, expression) {
        // Proof embedding observes the finite operand; it neither calls an
        // executable machine nor changes that operand's storage namespace.
        collect_authored_storage_reads(
            checked,
            state,
            scope,
            operand,
            path,
            active,
            reads,
            member_paths,
            unmatched_member_paths,
        )?;
        active.pop();
        return Ok(());
    }
    if let Some((subject, subject_path, case)) =
        case_membership::authored(checked, state, expression)?
    {
        reads.push((
            path.clone(),
            subject,
            PrimitiveType::Bool,
            ReadKind::CaseMembership {
                path: subject_path,
                case,
            },
        ));
        active.pop();
        return Ok(());
    }
    match checked.expression_table.expression(expression) {
        ExpressionNode::Match(_) => {
            return unsupported("scalar dispatch requires selective computation source custody");
        }
        ExpressionNode::Name(name) => {
            if let ReadScope::NormalResult {
                machine,
                return_type,
                primitive,
            } = scope
                && let Some(owner) = validation::reserved_result_owner(&checked.typed, expression)
            {
                if owner != (machine, return_type) {
                    return unsupported("normal predicate result belongs to another contract");
                }
                let primitive = primitive.ok_or(LoweringError::Unsupported(
                    "normal predicate result requires a fixed integer or Boolean carrier",
                ))?;
                reads.push((path.clone(), machine, primitive, ReadKind::Result));
                active.pop();
                return Ok(());
            }
            if let Some((symbol, primitive, kind)) =
                authored_storage_read(checked, state, scope, name)?
            {
                reads.push((path.clone(), symbol, primitive, kind));
            } else if let Some(target) = shared_loan_target(checked, state, scope, name) {
                // A shared place-loan carrier rejoins the captured
                // place's authored read rather than its own — replay
                // the borrow target through this same collector.
                collect_authored_storage_reads(
                    checked,
                    state,
                    scope,
                    target,
                    path,
                    active,
                    reads,
                    member_paths,
                    unmatched_member_paths,
                )?;
            }
        }
        ExpressionNode::Member(member) => {
            let (machine, _) = authored_state(checked, state.symbol)?;
            // Only source-backed places and their exact collection metadata
            // retain existing structural-read owners. A retained field, length,
            // or predicate tag cannot turn a closed value (or a corrupted
            // receiver) into a storage read.
            if !validation::place_has_builtin_coordinates(
                &checked.typed,
                machine,
                Some(state),
                expression,
            ) || validation::declared_place_type_raw(
                &checked.typed,
                machine,
                Some(state),
                expression,
            )
            .is_none()
                && validation::collection_length_receiver(
                    &checked.typed,
                    machine,
                    Some(state),
                    expression,
                )
                .is_none()
            {
                unmatched_member_paths.push(path.clone());
            }
            if member.case_variant.is_none()
                && matches!(
                    checked.expression_table.expression(member.receiver),
                    ExpressionNode::Name(_)
                )
            {
                member_paths.push(path.clone());
            }
            // A member read against a closed record constructor is a
            // projection of one declared field's closed value: the complete
            // constructor, every sibling initializer, and the field identity
            // all replay together, not just the selected leaf.
            if member.case_variant.is_none()
                && let ExpressionNode::StructLiteral(literal) =
                    checked.expression_table.expression(member.receiver)
                && literal.case_name.is_none()
                && validation::closed_record_scalar_projection(&checked.typed, expression).is_none()
            {
                return unsupported("closed record member lost its complete authored projection");
            }
            if let Some((symbol, primitive, kind)) =
                authored_owned_field(checked, state, expression)?
            {
                reads.push((path.clone(), symbol, primitive, kind));
            } else if let Some((symbol, primitive, kind)) =
                authored_indexed_primitive_read(checked, state, scope, machine, expression)?
            {
                reads.push((path.clone(), symbol, primitive, kind));
            } else if let Some(selector) = element_field_selector(checked, expression) {
                // A field of one view element (`view[i].value`) retains the
                // element read, whose selector is its only scalar operand;
                // replay that selector where the retained read places it.
                path.push(0);
                collect_authored_storage_reads(
                    checked,
                    state,
                    scope,
                    selector,
                    path,
                    active,
                    reads,
                    member_paths,
                    unmatched_member_paths,
                )?;
                path.pop();
            }
        }
        ExpressionNode::Binary(binary) => {
            let (left, right) = if matches!(
                binary.operator,
                BinaryOperator::Greater | BinaryOperator::GreaterOrEqual
            ) {
                (binary.right, binary.left)
            } else {
                (binary.left, binary.right)
            };
            for (position, operand) in [(0, left), (1, right)] {
                path.push(position);
                collect_authored_storage_reads(
                    checked,
                    state,
                    scope,
                    operand,
                    path,
                    active,
                    reads,
                    member_paths,
                    unmatched_member_paths,
                )?;
                path.pop();
            }
        }
        ExpressionNode::Unary(unary) => {
            collect_authored_storage_reads(
                checked,
                state,
                scope,
                unary.operand,
                path,
                active,
                reads,
                member_paths,
                unmatched_member_paths,
            )?;
        }
        ExpressionNode::Cast(cast) => {
            collect_authored_storage_reads(
                checked,
                state,
                scope,
                cast.value,
                path,
                active,
                reads,
                member_paths,
                unmatched_member_paths,
            )?;
        }
        ExpressionNode::Indexed(indexed) => {
            let (machine, _) = authored_state(checked, state.symbol)?;
            if let Some((symbol, primitive, kind)) =
                authored_indexed_primitive_read(checked, state, scope, machine, expression)?
            {
                reads.push((path.clone(), symbol, primitive, kind));
                active.pop();
                return Ok(());
            }
            path.push(0);
            collect_authored_storage_reads(
                checked,
                state,
                scope,
                indexed.index,
                path,
                active,
                reads,
                member_paths,
                unmatched_member_paths,
            )?;
            path.pop();
            // A runtime selector beneath another (`self.a[i][k]`) is one
            // nested read retaining every selector. Each enclosing runtime
            // selector replays one level deeper -- `[1, 0]`, `[1, 1, 0]` --
            // exactly where the retained read places it, so every selector's
            // identity rejoins, not only the top one.
            let mut descended = 0;
            let mut collection = indexed.collection;
            let mut visited = Vec::new();
            loop {
                if !checked.expression_table.expression_is_valid(collection)
                    || visited.contains(&collection)
                {
                    return unsupported("indexed read has a stale or cyclic collection");
                }
                visited.push(collection);
                let enclosing = match checked.expression_table.expression(collection) {
                    ExpressionNode::Member(member) if member.case_variant.is_none() => {
                        collection = member.receiver;
                        continue;
                    }
                    ExpressionNode::Indexed(enclosing)
                        if !matches!(
                            checked.expression_table.expression(enclosing.index),
                            ExpressionNode::Range(_)
                        ) =>
                    {
                        enclosing
                    }
                    _ => break,
                };
                path.push(1);
                descended += 1;
                path.push(0);
                collect_authored_storage_reads(
                    checked,
                    state,
                    scope,
                    enclosing.index,
                    path,
                    active,
                    reads,
                    member_paths,
                    unmatched_member_paths,
                )?;
                path.pop();
                collection = enclosing.collection;
            }
            path.truncate(path.len() - descended);
        }
        // These source forms are owned by literal, structural, or computation
        // custody. Their roots are not primitive scalar content reads.
        ExpressionNode::ArrayLiteral(_)
        | ExpressionNode::Atomic(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Call(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::UndeterminedNaN(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Borrow(_)
        | ExpressionNode::Range(_)
        | ExpressionNode::StructLiteral(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
    active.pop();
    Ok(())
}

/// The runtime selector of the element a member chain projects into
/// (`view[i].a.b` selects with `i`), when the chain bottoms out at a
/// single-element index rather than a range or a name.
fn element_field_selector(
    checked: &CheckedTrees,
    mut expression: ExpressionHandle,
) -> Option<ExpressionHandle> {
    while let ExpressionNode::Member(member) = checked.expression_table.expression(expression) {
        if member.case_variant.is_some() {
            return None;
        }
        expression = member.receiver;
    }
    let ExpressionNode::Indexed(indexed) = checked.expression_table.expression(expression) else {
        return None;
    };
    (!matches!(
        checked.expression_table.expression(indexed.index),
        ExpressionNode::Range(_)
    ))
    .then_some(indexed.index)
}

fn collect_scalar_storage_reads(
    expression: &CheckedScalarExpression,
    namespace: &mut ReadNamespace,
    path: &mut Vec<usize>,
    reads: &mut Vec<StorageReadOccurrence>,
    needs_value_replay: &mut bool,
) {
    match expression {
        CheckedScalarExpression::StorageRead {
            symbol,
            primitive_type,
        } => {
            reads.push((path.clone(), *symbol, *primitive_type, ReadKind::Storage));
        }
        CheckedScalarExpression::IntegerBinary { left, right, .. } => {
            for (position, operand) in [(0, left), (1, right)] {
                path.push(position);
                collect_scalar_storage_reads(operand, namespace, path, reads, needs_value_replay);
                path.pop();
            }
        }
        CheckedScalarExpression::StructuralParameterIndexedRead {
            root,
            path: read_path,
            index,
            inner_indexes,
            element_path,
            primitive_type,
        } => {
            // Selector-only read inventory cannot authenticate a parameter
            // view's record field. Replay its exact collection and field even
            // when this expression contains no other view-length observation.
            if matches!(root, checked_trees::CheckedStorageRoot::Parameter { .. })
                && !element_path.is_empty()
            {
                *needs_value_replay = true;
            }
            // A nested read (`self.a[i][k]`) places its selectors where the
            // authored walk visits them: the top indexing's selector -- the
            // last one applied -- at `[0]`, each enclosing one a level deeper
            // at `[1, 0]`, `[1, 1, 0]`. The value replay then rejoins the
            // chain's shape.
            if !inner_indexes.is_empty()
                || element_path.iter().any(|segment| {
                    matches!(
                        segment,
                        checked_trees::CheckedStructuralPredicatePathSegment::RuntimeIndex(_)
                    )
                })
            {
                *needs_value_replay = true;
                let mut tail_depth = 0;
                for segment in element_path.iter().rev() {
                    match segment {
                        checked_trees::CheckedStructuralPredicatePathSegment::RuntimeIndex(
                            coordinate,
                        ) => {
                            path.extend(std::iter::repeat_n(1, tail_depth));
                            path.push(0);
                            if let Some(selector) = retained_index_selector(namespace, coordinate) {
                                collect_scalar_storage_reads(
                                    &selector,
                                    namespace,
                                    path,
                                    reads,
                                    needs_value_replay,
                                );
                            }
                            path.truncate(path.len() - tail_depth - 1);
                            tail_depth += 1;
                        }
                        checked_trees::CheckedStructuralPredicatePathSegment::FixedIndex(_) => {
                            tail_depth += 1
                        }
                        _ => {}
                    }
                }
                let selectors = std::iter::once(index.as_ref())
                    .chain(inner_indexes.iter())
                    .rev()
                    .enumerate();
                for (depth, selector) in selectors {
                    let depth = depth + tail_depth;
                    path.extend(std::iter::repeat_n(1, depth));
                    path.push(0);
                    collect_scalar_storage_reads(
                        selector,
                        namespace,
                        path,
                        reads,
                        needs_value_replay,
                    );
                    path.truncate(path.len() - depth - 1);
                }
                return;
            }
            // An owned local's literal element read retains the same
            // `IndexedPrimitive` leaf the authored `local[literal]` rejoin
            // produces; a dynamic index retains only its selector read.
            if let checked_trees::CheckedStorageRoot::OwnedLocal { symbol } = *root
                && let CheckedScalarExpression::IntegerLiteral { literal } = index.as_ref()
                && let Some(index) = literal.value_u64()
            {
                let mut projection = read_path.clone();
                projection
                    .push(checked_trees::CheckedStructuralPredicatePathSegment::FixedIndex(index));
                projection.extend(element_path.iter().cloned());
                reads.push((
                    path.clone(),
                    symbol,
                    *primitive_type,
                    ReadKind::IndexedPrimitive(projection),
                ));
                return;
            }
            // A view local's literal element read retains the same
            // `IndexedPrimitive` leaf the authored walk records: the view's
            // own establishment names the place its loan captured, and the
            // member path plus selectors project beneath it. A runtime
            // selector stays selector-only, as the authored side records.
            if let checked_trees::CheckedStorageRoot::ViewLocal { symbol } = *root
                && let CheckedScalarExpression::IntegerLiteral { literal } = index.as_ref()
                && let Some(index) = literal.value_u64()
                && let Some((view_root, prefix)) = namespace.view_local_place(symbol)
            {
                let mut projection = prefix;
                projection.extend(read_path.iter().cloned());
                projection
                    .push(checked_trees::CheckedStructuralPredicatePathSegment::FixedIndex(index));
                projection.extend(element_path.iter().cloned());
                reads.push((
                    path.clone(),
                    view_root,
                    *primitive_type,
                    ReadKind::IndexedPrimitive(projection),
                ));
                return;
            }
            path.push(0);
            collect_scalar_storage_reads(index, namespace, path, reads, needs_value_replay);
            path.pop();
        }
        CheckedScalarExpression::IntegerBitwiseNot { operand, .. }
        | CheckedScalarExpression::IntegerWiden { operand, .. }
        | CheckedScalarExpression::IntegerExactCast { operand, .. }
        | CheckedScalarExpression::IntegerTrappingCast { operand, .. }
        | CheckedScalarExpression::IntegerWrappingCast { operand, .. }
        | CheckedScalarExpression::RepresentationView { operand, .. }
        | CheckedScalarExpression::IntegerSaturatingCast { operand, .. } => {
            collect_scalar_storage_reads(operand, namespace, path, reads, needs_value_replay);
        }
        CheckedScalarExpression::Boolean(expression) => {
            collect_boolean_storage_reads(expression, namespace, path, reads, needs_value_replay);
        }
        CheckedScalarExpression::Parameter {
            position,
            primitive_type,
        }
        | CheckedScalarExpression::Local {
            position,
            primitive_type,
        } => {
            if supported_mutable_parameter(*primitive_type)
                || matches!(
                    namespace.scope,
                    ReadScope::Entry | ReadScope::NormalResult { .. }
                )
            {
                let (symbol, kind) = namespace.scalar_read(
                    *position,
                    *primitive_type,
                    matches!(expression, CheckedScalarExpression::Parameter { .. }),
                );
                reads.push((path.clone(), symbol, *primitive_type, kind));
            }
        }
        CheckedScalarExpression::StructuralParameterField {
            parameter_position,
            path: field_path,
            primitive_type,
        } => {
            collect_owned_field(
                *parameter_position,
                field_path,
                *primitive_type,
                namespace,
                path,
                reads,
            );
        }
        CheckedScalarExpression::IntegerLiteral { .. }
        | CheckedScalarExpression::IeeeFloatLiteral { .. } => {
            if namespace
                .unmatched_member_paths
                .iter()
                .any(|source_path| source_path == path)
            {
                *needs_value_replay = true;
                namespace
                    .unmatched_member_paths
                    .retain(|source_path| source_path != path);
            }
        }
        CheckedScalarExpression::StructuralParameterByteLength { .. } => {
            // A length has no scalar storage read, but its structural root and
            // path still have to match this exact authored observation.
            *needs_value_replay = true;
        }
        // Proof-only erased formals own no storage; the read still rejoins
        // its authored formal under the dense erased roster.
        CheckedScalarExpression::ErasedParameter {
            position,
            primitive_type,
        } => {
            if supported_mutable_parameter(*primitive_type)
                || matches!(
                    namespace.scope,
                    ReadScope::Entry | ReadScope::NormalResult { .. }
                )
            {
                reads.push((
                    path.clone(),
                    namespace
                        .erased_scalar
                        .get(*position)
                        .copied()
                        .unwrap_or_default(),
                    *primitive_type,
                    ReadKind::Parameter,
                ));
            }
        }
    }
}

/// Recover the declared scalar type of a retained path coordinate. The complete
/// path replay separately checks that the authored selector names this coordinate.
fn retained_index_selector(
    namespace: &ReadNamespace,
    coordinate: &checked_trees::CheckedRuntimeIndex,
) -> Option<CheckedScalarExpression> {
    match coordinate {
        checked_trees::CheckedRuntimeIndex::Parameter { position } => {
            let symbol = *namespace.scalar.get(*position as usize)?;
            let parameter = namespace
                .structural
                .iter()
                .find(|parameter| parameter.symbol == symbol)?;
            Some(CheckedScalarExpression::Parameter {
                position: *position as usize,
                primitive_type: namespace
                    .checked
                    .primitive_type_reference(parameter.type_reference)?,
            })
        }
        checked_trees::CheckedRuntimeIndex::StructuralField(field) => {
            let reference = super::structural_places::declared_parameter_path_type(
                namespace.checked,
                namespace.state,
                field,
            )?;
            let primitive_type = namespace.checked.primitive_type_reference(reference)?;
            Some(CheckedScalarExpression::StructuralParameterField {
                parameter_position: field.parameter_position,
                path: field.path.clone(),
                primitive_type,
            })
        }
        _ => None,
    }
}

fn collect_boolean_storage_reads(
    expression: &CheckedBooleanExpression,
    namespace: &mut ReadNamespace,
    path: &mut Vec<usize>,
    reads: &mut Vec<StorageReadOccurrence>,
    needs_value_replay: &mut bool,
) {
    match expression {
        CheckedBooleanExpression::StorageRead { symbol } => {
            reads.push((
                path.clone(),
                *symbol,
                PrimitiveType::Bool,
                ReadKind::Storage,
            ));
        }
        CheckedBooleanExpression::Not(operand) => {
            collect_boolean_storage_reads(operand, namespace, path, reads, needs_value_replay);
        }
        CheckedBooleanExpression::Equal { left, right }
        | CheckedBooleanExpression::And { left, right }
        | CheckedBooleanExpression::Or { left, right } => {
            for (position, operand) in [(0, left), (1, right)] {
                path.push(position);
                collect_boolean_storage_reads(operand, namespace, path, reads, needs_value_replay);
                path.pop();
            }
        }
        CheckedBooleanExpression::IntegerMathComparison { left, right, .. } => {
            // Also replay value meaning when there are no reads: a malformed
            // literal-only capsule cannot evade correspondence through an
            // empty storage inventory.
            *needs_value_replay = true;
            for (position, expression) in [(0, left), (1, right)] {
                let mut root_path = path.clone();
                root_path.push(position);
                let mut pending = vec![(expression.root, root_path)];
                let mut remaining = namespace
                    .checked
                    .expression_table
                    .expression_count()
                    .saturating_mul(4);
                while let Some((handle, mut term_path)) = pending.pop() {
                    if remaining == 0 || !expression.terms.is_valid(handle) {
                        break; // Value replay independently rejects this capsule.
                    }
                    remaining -= 1;
                    use checked_trees::CheckedIntegerMathTerm;
                    match expression.terms.get(handle) {
                        CheckedIntegerMathTerm::Embed { operand, .. } => {
                            collect_scalar_storage_reads(
                                operand,
                                namespace,
                                &mut term_path,
                                reads,
                                needs_value_replay,
                            );
                        }
                        CheckedIntegerMathTerm::Literal(_) => {}
                        CheckedIntegerMathTerm::Add { left, right }
                        | CheckedIntegerMathTerm::Subtract { left, right }
                        | CheckedIntegerMathTerm::Multiply { left, right } => {
                            let mut right_path = term_path.clone();
                            right_path.push(1);
                            term_path.push(0);
                            pending.push((*right, right_path));
                            pending.push((*left, term_path));
                        }
                    }
                }
            }
        }
        CheckedBooleanExpression::IntegerComparison { left, right, .. }
        | CheckedBooleanExpression::ScalarIeeeFloatComparison { left, right, .. } => {
            for (position, operand) in [(0, left), (1, right)] {
                path.push(position);
                collect_scalar_storage_reads(operand, namespace, path, reads, needs_value_replay);
                path.pop();
            }
        }
        CheckedBooleanExpression::Parameter { position }
        | CheckedBooleanExpression::Local { position } => {
            let (symbol, kind) = namespace.scalar_read(
                *position,
                PrimitiveType::Bool,
                matches!(expression, CheckedBooleanExpression::Parameter { .. }),
            );
            reads.push((path.clone(), symbol, PrimitiveType::Bool, kind));
        }
        CheckedBooleanExpression::StructuralParameterField {
            parameter_position,
            path: field_path,
        } => {
            collect_owned_field(
                *parameter_position,
                field_path,
                PrimitiveType::Bool,
                namespace,
                path,
                reads,
            );
        }
        CheckedBooleanExpression::StructuralCaseMembership { subject, case } => {
            reads.push((
                path.clone(),
                namespace
                    .structural
                    .get(subject.parameter_position as usize)
                    .map(|parameter| parameter.symbol)
                    .unwrap_or_default(),
                PrimitiveType::Bool,
                ReadKind::CaseMembership {
                    path: subject.path.clone(),
                    case: case.clone(),
                },
            ));
        }
        CheckedBooleanExpression::Constant(_) => {
            if namespace
                .unmatched_member_paths
                .iter()
                .any(|source_path| source_path == path)
            {
                *needs_value_replay = true;
                namespace
                    .unmatched_member_paths
                    .retain(|source_path| source_path != path);
            }
        }
        CheckedBooleanExpression::ErasedParameter { position } => {
            reads.push((
                path.clone(),
                namespace
                    .erased_scalar
                    .get(*position)
                    .copied()
                    .unwrap_or_default(),
                PrimitiveType::Bool,
                ReadKind::Parameter,
            ));
        }
        CheckedBooleanExpression::IeeeFloatComparison { .. }
        | CheckedBooleanExpression::ByteSequenceEqual { .. }
        | CheckedBooleanExpression::ByteSequencePredicate { .. }
        | CheckedBooleanExpression::PayloadlessSumEqual { .. } => {}
    }
}
