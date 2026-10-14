use arena::{Arena, Handle, HandleSpan};
use std::fmt;
use symbols::SymbolHandle;

use crate::name::Identifier;
use crate::typed_trees::remapped;

mod copies;
mod display;
mod substitution;

pub type TypeReferenceHandle = Handle<TypeReferenceNode>;

/// Exact declaration use erased into a structural type, such as a named array
/// extent. This is initializer/selection custody, not runtime constant storage.
/// It is independent of public exposure: generated type copies need the same
/// dependency without inventing another authored visibility occurrence.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TypeConstantOrigin {
    pub declaration: SymbolHandle,
    pub reference: source::SourceSpan,
}

#[derive(Clone, PartialEq, Eq)]
pub struct TypeReferenceTable {
    type_references: Arena<TypeReferenceNode>,
    type_reference_handles: Arena<TypeReferenceHandle>,
    constraints: Arena<TypeConstraintNode>,
    constant_origin_spans: Arena<HandleSpan<TypeConstantOrigin>>,
    constant_origins: Arena<TypeConstantOrigin>,
    /// A substituted operand retains its original lifetime environment after
    /// copying into a private declaration. Zero means ordinary lexical lookup.
    operand_lifetime_owners: Arena<SymbolHandle>,
    named_rows: NamedRowIndex,
}

// Written out so the derived row index stays out of the table's rendering.
impl std::fmt::Debug for TypeReferenceTable {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TypeReferenceTable")
            .field("type_references", &self.type_references)
            .field("type_reference_handles", &self.type_reference_handles)
            .field("constraints", &self.constraints)
            .field("constant_origin_spans", &self.constant_origin_spans)
            .field("constant_origins", &self.constant_origins)
            .field("operand_lifetime_owners", &self.operand_lifetime_owners)
            .finish()
    }
}

/// The first `Named` row per symbol, built on the first lookup. Builtin
/// carriers are looked up by symbol throughout checking, and each lookup
/// scanned every row. `insert` extends a built index in place; the two
/// in-place rewrites that can change a row's symbol keep it exact. Derived
/// content, so it never takes part in equality.
#[derive(Clone, Default)]
struct NamedRowIndex(std::sync::OnceLock<symbols::SymbolKeyMap<SymbolHandle, TypeReferenceHandle>>);

impl PartialEq for NamedRowIndex {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl Eq for NamedRowIndex {}

impl TypeReferenceTable {
    pub(crate) fn retains_exact_prefix(&self, base: &Self) -> bool {
        let Self {
            type_references,
            type_reference_handles,
            constraints,
            constant_origin_spans,
            constant_origins,
            operand_lifetime_owners,
            named_rows: _,
        } = base;
        crate::typed_trees::retained_prefix::arena_is_exact_prefix(
            type_references,
            &self.type_references,
        ) && crate::typed_trees::retained_prefix::arena_is_exact_prefix(
            type_reference_handles,
            &self.type_reference_handles,
        ) && crate::typed_trees::retained_prefix::arena_is_exact_prefix(
            constraints,
            &self.constraints,
        ) && crate::typed_trees::retained_prefix::arena_is_exact_prefix(
            constant_origin_spans,
            &self.constant_origin_spans,
        ) && crate::typed_trees::retained_prefix::arena_is_exact_prefix(
            constant_origins,
            &self.constant_origins,
        ) && crate::typed_trees::retained_prefix::arena_is_exact_prefix(
            operand_lifetime_owners,
            &self.operand_lifetime_owners,
        )
    }

    pub fn new() -> Self {
        Self {
            type_references: Arena::new(),
            type_reference_handles: Arena::new(),
            constraints: Arena::new(),
            constant_origin_spans: Arena::new(),
            constant_origins: Arena::new(),
            operand_lifetime_owners: Arena::new(),
            named_rows: NamedRowIndex::default(),
        }
    }

    /// Summed revision of the table's arenas — the freshness key for every
    /// index or memo derived from type-reference rows and constraints.
    pub fn revision(&self) -> u64 {
        self.type_references
            .revision()
            .wrapping_add(self.type_reference_handles.revision())
            .wrapping_add(self.constraints.revision())
            .wrapping_add(self.constant_origins.revision())
            .wrapping_add(self.constant_origin_spans.revision())
            .wrapping_add(self.operand_lifetime_owners.revision())
    }

    pub fn insert(&mut self, type_reference: TypeReferenceNode) -> TypeReferenceHandle {
        let named = match &type_reference {
            TypeReferenceNode::Named { symbol, .. } => Some(*symbol),
            _ => None,
        };
        let handle = self.type_references.insert(type_reference);
        let origins = self.constant_origin_spans.insert(HandleSpan::empty());
        let lifetime_owner = self.operand_lifetime_owners.insert(SymbolHandle::invalid());
        debug_assert_eq!(handle.arena_index(), lifetime_owner.arena_index());
        debug_assert_eq!(handle.arena_index(), origins.arena_index());
        debug_assert_eq!(handle.generation(), origins.generation());
        if let Some(symbol) = named {
            self.note_named_row(symbol, handle);
        }
        handle
    }

    /// Record the supplying declaration before a simultaneous operand is
    /// copied into a callee. An already retained origin remains authoritative;
    /// moving the same operand again does not rebind its lifetime names.
    pub fn retain_operand_lifetime_owner(
        &mut self,
        reference: TypeReferenceHandle,
        owner: SymbolHandle,
    ) -> Result<(), &'static str> {
        if !self.contains_type_reference(reference) {
            return Err("substituted operand has no registered type row");
        }
        let row = Handle::from_parts(reference.arena_index(), reference.generation());
        let retained = self.operand_lifetime_owners.get_mut(row);
        if !retained.is_valid() {
            *retained = owner;
        }
        Ok(())
    }

    pub fn operand_lifetime_owner(&self, reference: TypeReferenceHandle) -> SymbolHandle {
        if !self.contains_type_reference(reference) {
            return SymbolHandle::invalid();
        }
        *self.operand_lifetime_owners.get(Handle::from_parts(
            reference.arena_index(),
            reference.generation(),
        ))
    }

    /// Preserve the original selected declaration at its actual receiving type
    /// row. Distinct uses can have equal structural types and different origins.
    pub fn attach_constant_origins(
        &mut self,
        reference: TypeReferenceHandle,
        origins: impl IntoIterator<Item = TypeConstantOrigin>,
    ) -> Result<(), &'static str> {
        let mut origins = origins.into_iter().peekable();
        if origins.peek().is_none() {
            return Ok(());
        }
        let mut combined = self.constant_origins(reference)?.to_vec();
        for origin in origins {
            if !combined.contains(&origin) {
                combined.push(origin);
            }
        }
        *self.constant_origin_spans.get_mut(Handle::from_parts(
            reference.arena_index(),
            reference.generation(),
        )) = self.constant_origins.insert_many(combined);
        Ok(())
    }

    pub fn constant_origins(
        &self,
        reference: TypeReferenceHandle,
    ) -> Result<&[TypeConstantOrigin], &'static str> {
        let owner = Handle::from_parts(reference.arena_index(), reference.generation());
        if !self.type_references.is_valid(reference) || !self.constant_origin_spans.is_valid(owner)
        {
            return Err("constant type origin has a stale receiving type");
        }
        let span = *self.constant_origin_spans.get(owner);
        let origins = self.constant_origins.span_or_empty(span);
        if origins.len() != span.len() {
            return Err("constant type origin has a stale occurrence span");
        }
        Ok(origins)
    }

    /// Record `handle` as a `Named` row for `symbol` in a built index, where
    /// the lowest row wins as it does in a table scan.
    fn note_named_row(&mut self, symbol: SymbolHandle, handle: TypeReferenceHandle) {
        if let Some(rows) = self.named_rows.0.get_mut() {
            rows.entry(symbol)
                .and_modify(|first| {
                    if handle.arena_index() < first.arena_index() {
                        *first = handle;
                    }
                })
                .or_insert(handle);
        }
    }

    /// Recompute a built index's entry for `symbol` after the row holding
    /// its first occurrence changed.
    fn rescan_named_row(&mut self, symbol: SymbolHandle) {
        let Some(rows) = self.named_rows.0.get_mut() else {
            return;
        };
        let first = self
            .type_references
            .iter()
            .find_map(|(handle, node)| match node {
                TypeReferenceNode::Named {
                    symbol: candidate, ..
                } if *candidate == symbol => Some(handle),
                _ => None,
            });
        match first {
            Some(handle) => {
                rows.insert(symbol, handle);
            }
            None => {
                rows.remove(&symbol);
            }
        }
    }

    pub fn insert_type_reference_handles(
        &mut self,
        type_references: impl IntoIterator<Item = TypeReferenceHandle>,
    ) -> HandleSpan<TypeReferenceHandle> {
        self.type_reference_handles.insert_many(type_references)
    }

    pub fn reserve_type_reference_handles(
        &mut self,
        count: u32,
    ) -> HandleSpan<TypeReferenceHandle> {
        self.type_reference_handles.insert_many(
            std::iter::repeat_with(TypeReferenceHandle::invalid)
                .take(usize::try_from(count).expect("type reference handle span count overflow")),
        )
    }

    pub fn set_type_reference_handle_at_offset(
        &mut self,
        type_references: HandleSpan<TypeReferenceHandle>,
        offset: u32,
        type_reference: TypeReferenceHandle,
    ) {
        *self
            .type_reference_handles
            .get_mut(type_references.handle_at(offset)) = type_reference;
    }

    pub fn push_type_reference_handle(
        &mut self,
        span: &mut HandleSpan<TypeReferenceHandle>,
        type_reference: TypeReferenceHandle,
    ) {
        self.type_reference_handles
            .append_to_span(span, type_reference);
    }

    pub fn insert_constraints(
        &mut self,
        constraints: impl IntoIterator<Item = TypeConstraintNode>,
    ) -> HandleSpan<TypeConstraintNode> {
        self.constraints.insert_many(constraints)
    }

    pub fn reserve_constraints(&mut self, count: u32) -> HandleSpan<TypeConstraintNode> {
        self.constraints.insert_many(
            std::iter::repeat_with(TypeConstraintNode::default)
                .take(usize::try_from(count).expect("type constraint span count overflow")),
        )
    }

    pub fn set_constraint_at_offset(
        &mut self,
        constraints: HandleSpan<TypeConstraintNode>,
        offset: u32,
        constraint: TypeConstraintNode,
    ) {
        *self.constraints.get_mut(constraints.handle_at(offset)) = constraint;
    }

    pub fn push_constraint(
        &mut self,
        span: &mut HandleSpan<TypeConstraintNode>,
        constraint: TypeConstraintNode,
    ) {
        self.constraints.append_to_span(span, constraint);
    }

    #[inline]
    pub fn type_reference(&self, handle: TypeReferenceHandle) -> &TypeReferenceNode {
        self.type_references.get(handle)
    }

    /// `handle` followed through every `Reference` node — the referee the
    /// declared alias chain names.
    pub fn dereferenced_type_reference(
        &self,
        mut handle: TypeReferenceHandle,
    ) -> TypeReferenceHandle {
        while let TypeReferenceNode::Reference { referee, .. } = self.type_reference(handle) {
            handle = *referee;
        }
        handle
    }

    /// `handle` with every outer `Reference` and `Constrained` shell
    /// removed — the declared carrier underneath both wrappers.
    pub fn peeled_type_reference(&self, mut handle: TypeReferenceHandle) -> TypeReferenceHandle {
        while let TypeReferenceNode::Reference { referee, .. }
        | TypeReferenceNode::Constrained {
            base_type: referee, ..
        } = self.type_reference(handle)
        {
            handle = *referee;
        }
        handle
    }

    /// The decimal value of an anonymous numeric literal `Named`
    /// reference — an invalid symbol carrying an all-digits authored
    /// name — or `None` for any other node shape.
    pub fn anonymous_numeric_literal_value(&self, handle: TypeReferenceHandle) -> Option<usize> {
        let TypeReferenceNode::Named { symbol, name } = self.type_reference(handle) else {
            return None;
        };
        if symbol.is_valid() {
            return None;
        }
        name.as_str().parse::<usize>().ok()
    }

    /// The `Named` symbol, or the base symbol of a `Generic` whose argument
    /// span is empty — the nominal head a closed reference resolves to.
    /// `None` for every other node and for generics still carrying type
    /// arguments.
    pub fn closed_nominal_symbol(&self, handle: TypeReferenceHandle) -> Option<SymbolHandle> {
        match self.type_reference(handle) {
            TypeReferenceNode::Named { symbol, .. } => Some(*symbol),
            TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } if self.type_reference_handles(*arguments).is_empty() => Some(*base_symbol),
            _ => None,
        }
    }

    /// `handle` with every outer `Constrained` shell removed — the carrier
    /// type the constraint set annotates.
    pub fn unconstrained_type_reference(
        &self,
        mut handle: TypeReferenceHandle,
    ) -> TypeReferenceHandle {
        while let TypeReferenceNode::Constrained { base_type, .. } = self.type_reference(handle) {
            handle = *base_type;
        }
        handle
    }

    /// The declared symbol of the carrier behind every `Constrained` shell —
    /// `unconstrained_type_reference` followed by the `Named` node check.
    pub fn unconstrained_named_symbol(&self, handle: TypeReferenceHandle) -> Option<SymbolHandle> {
        match self.type_reference(self.unconstrained_type_reference(handle)) {
            TypeReferenceNode::Named { symbol, .. } => Some(*symbol),
            _ => None,
        }
    }

    /// The declared head symbol behind a `Named` or `Generic` reference —
    /// the base nominal, whether or not arguments are supplied.
    pub fn type_head_symbol(&self, handle: TypeReferenceHandle) -> Option<SymbolHandle> {
        match self.type_reference(handle) {
            TypeReferenceNode::Named { symbol, .. } => Some(*symbol),
            TypeReferenceNode::Generic { base_symbol, .. } => Some(*base_symbol),
            _ => None,
        }
    }

    /// The declared symbol behind a `Named` reference or a `Generic` with no
    /// supplied arguments — the head symbol when no instantiation happens.
    pub fn unparameterized_type_symbol(
        &self,
        handle: TypeReferenceHandle,
    ) -> Option<&SymbolHandle> {
        match self.type_reference(handle) {
            TypeReferenceNode::Named { symbol, .. } => Some(symbol),
            TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } if arguments.count() == 0 => Some(base_symbol),
            _ => None,
        }
    }

    /// Whether an exact generational handle still names a live type node.
    /// A stale reference resolving to dummy Unit is not retained type custody.
    pub fn contains_type_reference(&self, handle: TypeReferenceHandle) -> bool {
        self.type_references.is_valid(handle)
    }

    /// Every fixed-array type reference in the table, as `(handle, length)`
    /// pairs. The orchestration const-eval pass scans these for `ConstCall`
    /// lengths to substitute before checking/layout.
    pub fn fixed_array_lengths(
        &self,
    ) -> impl Iterator<Item = (TypeReferenceHandle, &FixedArrayLength)> {
        self.type_references
            .iter()
            .filter_map(|(handle, node)| match node {
                TypeReferenceNode::FixedArray { length, .. } => Some((handle, length)),
                _ => None,
            })
    }

    /// Live type nodes, retaining their exact generational handles.
    pub fn references(
        &self,
    ) -> impl Iterator<Item = (TypeReferenceHandle, &TypeReferenceNode)> + '_ {
        self.type_references.iter()
    }

    /// Every NAMED type reference in the table, as `(handle, symbol)` pairs.
    /// The machine-monomorphization pass scans these for generic type-parameter
    /// references to substitute before checking/layout (a type parameter's
    /// symbol is unique to its declaring machine, so symbol equality exactly
    /// identifies the occurrences to substitute).
    pub fn named_references(
        &self,
    ) -> impl Iterator<Item = (TypeReferenceHandle, SymbolHandle, &str)> + '_ {
        self.type_references
            .iter()
            .filter_map(|(handle, node)| match node {
                TypeReferenceNode::Named { symbol, name } => Some((handle, *symbol, name.as_str())),
                _ => None,
            })
    }

    /// Rewrite this occurrence's structure, preserving its original custody.
    /// When the replacement comes from another retained row, use
    /// `substitute_from` to preserve that row's origins as well.

    pub fn set_fixed_array_length(&mut self, handle: TypeReferenceHandle, value: usize) {
        let TypeReferenceNode::FixedArray { length, .. } = self.type_references.get_mut(handle)
        else {
            panic!("set_fixed_array_length called on a non-fixed-array type reference");
        };
        *length = FixedArrayLength::Literal(value);
    }

    pub fn type_reference_handles(
        &self,
        span: HandleSpan<TypeReferenceHandle>,
    ) -> &[TypeReferenceHandle] {
        self.type_reference_handles.span_or_empty(span)
    }

    pub fn constraints(&self, span: HandleSpan<TypeConstraintNode>) -> &[TypeConstraintNode] {
        self.constraints.span_or_empty(span)
    }

    /// Every constrained type's carrier and constraint span. Used by the
    /// post-lowering domain-normalization pass once all declarations exist.
    pub fn constrained_type_references(
        &self,
    ) -> Vec<(TypeReferenceHandle, HandleSpan<TypeConstraintNode>)> {
        self.type_references
            .iter()
            .filter_map(|(_, node)| match node {
                TypeReferenceNode::Constrained {
                    base_type,
                    constraints,
                } => Some((*base_type, *constraints)),
                _ => None,
            })
            .collect()
    }

    /// Every constrained node together with its carrier and constraint span.
    /// The node handle permits pre-normalization rewrites such as transparent
    /// alias expansion.
    pub fn constrained_type_reference_sites(
        &self,
    ) -> Vec<(
        TypeReferenceHandle,
        TypeReferenceHandle,
        HandleSpan<TypeConstraintNode>,
    )> {
        self.type_references
            .iter()
            .filter_map(|(handle, node)| match node {
                TypeReferenceNode::Constrained {
                    base_type,
                    constraints,
                } => Some((handle, *base_type, *constraints)),
                _ => None,
            })
            .collect()
    }

    /// Every proof-static open index expression retained in type position.
    /// Callers use the node handle to scope specialization rewrites to newly
    /// cloned type-reference regions without scanning diagnostic renderings.
    pub fn const_expression_sites(
        &self,
    ) -> Vec<(TypeReferenceHandle, crate::expression::ExpressionHandle)> {
        self.type_references
            .iter()
            .filter_map(|(handle, node)| match node {
                TypeReferenceNode::ConstExpression(expression) => Some((handle, *expression)),
                _ => None,
            })
            .collect()
    }

    pub fn set_constraint_span(
        &mut self,
        handle: TypeReferenceHandle,
        constraints: HandleSpan<TypeConstraintNode>,
    ) {
        let TypeReferenceNode::Constrained {
            constraints: current,
            ..
        } = self.type_references.get_mut(handle)
        else {
            panic!("set_constraint_span called on a non-constrained type reference");
        };
        *current = constraints;
    }

    pub fn constraints_mut(
        &mut self,
        span: HandleSpan<TypeConstraintNode>,
    ) -> &mut [TypeConstraintNode] {
        self.constraints.span_mut_or_empty(span)
    }

    pub fn constraint_span(
        &self,
        span: HandleSpan<TypeConstraintNode>,
    ) -> Option<&[TypeConstraintNode]> {
        self.constraints.span(span)
    }

    /// Existing plain view shells, preserving element and access identities.
    /// Result queries may select a declared carrier without manufacturing a
    /// type reference or copying the source collection's value predicates.
    pub fn slice_view_references(
        &self,
    ) -> impl Iterator<
        Item = (
            TypeReferenceHandle,
            language_core::ReferenceAccess,
            TypeReferenceHandle,
        ),
    > + '_ {
        self.type_references.iter().filter_map(|(handle, node)| {
            let TypeReferenceNode::Reference {
                referee, access, ..
            } = node
            else {
                return None;
            };
            let TypeReferenceNode::Slice { element_type } = self.type_reference(*referee) else {
                return None;
            };
            Some((handle, *access, *element_type))
        })
    }

    pub fn type_reference_count(&self) -> usize {
        self.type_references.len()
    }

    /// Every live type reference with its node, in arena order. Consumers
    /// that classify the whole table — not one declaration's types — walk
    /// this rather than rediscovering rows through the declaration roots,
    /// which never reach a type the table interned for an intermediate.
    pub fn type_reference_rows(
        &self,
    ) -> impl Iterator<Item = (TypeReferenceHandle, &TypeReferenceNode)> + '_ {
        self.type_references.iter()
    }

    pub fn find_named_type_reference(&self, symbol: SymbolHandle) -> Option<TypeReferenceHandle> {
        self.named_rows
            .0
            .get_or_init(|| {
                let mut rows = symbols::SymbolKeyMap::default();
                for (handle, node) in self.type_references.iter() {
                    if let Some(symbol) = node.as_named_symbol() {
                        rows.entry(symbol).or_insert(handle);
                    }
                }
                rows
            })
            .get(&symbol)
            .copied()
    }

    /// Find an exact carrier/policy result without borrowing input predicates.
    /// The caller establishes that `carrier` is its intended builtin symbol;
    /// this structural lookup neither resolves names nor strips qualifications.
    pub fn find_arithmetic_result_type_reference(
        &self,
        carrier: SymbolHandle,
        domain: numerics::arithmetic::ArithmeticDomain,
    ) -> Option<TypeReferenceHandle> {
        use numerics::arithmetic::ArithmeticDomain;
        if !carrier.is_valid() {
            return None;
        }
        if domain == ArithmeticDomain::Exact {
            return self.find_named_type_reference(carrier);
        }
        self.type_references.iter().find_map(|(handle, node)| {
            let TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } = node
            else {
                return None;
            };
            if !self.contains_type_reference(*base_type) {
                return None;
            }
            let Some(symbol) = self.type_reference(*base_type).as_named_symbol() else {
                return None;
            };
            (symbol == carrier
                && matches!(self.constraint_span(*constraints)?,
                    [TypeConstraintNode::ArithmeticDomain(retained)] if *retained == domain))
            .then_some(handle)
        })
    }

    pub fn constraint_count(&self) -> usize {
        self.constraints.len()
    }

    /// Find a retained policy shell over this exact target, preserving all of
    /// its predicates. Unlike an arithmetic-result projection, this lookup
    /// cannot substitute a same-carrier type with different range obligations.
    /// The producer owns eligibility and insertion; this query proves no facts.
    pub fn find_policy_qualified_type_reference(
        &self,
        target: TypeReferenceHandle,
        domain: numerics::arithmetic::ArithmeticDomain,
    ) -> Option<TypeReferenceHandle> {
        if !self.contains_type_reference(target) {
            return None;
        }
        self.type_references.iter().find_map(|(handle, node)| {
            let TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } = node
            else {
                return None;
            };
            (*base_type == target
                && matches!(self.constraint_span(*constraints)?,
                    [TypeConstraintNode::ArithmeticDomain(retained)] if *retained == domain))
            .then_some(handle)
        })
    }

    /// Collection arithmetic policy qualifies numeric element values and
    /// nested collection operations. A structural element keeps its declared
    /// type: the containing array's bounds policy grants no policy to a record,
    /// case value, Boolean or borrowed reference. Exact builtin identity owns
    /// numeric membership; a declaration's diagnostic spelling supplies none.
    pub fn element_inherits_collection_arithmetic_policy(
        &self,
        mut element: TypeReferenceHandle,
        symbols: &symbols::SymbolTable,
    ) -> bool {
        let mut visited = Vec::new();
        while self.contains_type_reference(element) && !visited.contains(&element) {
            visited.push(element);
            match self.type_reference(element) {
                TypeReferenceNode::Constrained { base_type, .. } => element = *base_type,
                TypeReferenceNode::FixedArray { .. } | TypeReferenceNode::Slice { .. } => {
                    return true;
                }
                TypeReferenceNode::Named { symbol, .. } => {
                    return matches!(
                        symbols.builtin_type_atom(*symbol),
                        Some(
                            symbols::BuiltinTypeAtom::I8
                                | symbols::BuiltinTypeAtom::I16
                                | symbols::BuiltinTypeAtom::I32
                                | symbols::BuiltinTypeAtom::I64
                                | symbols::BuiltinTypeAtom::U8
                                | symbols::BuiltinTypeAtom::U16
                                | symbols::BuiltinTypeAtom::U32
                                | symbols::BuiltinTypeAtom::U64
                                | symbols::BuiltinTypeAtom::Address
                                | symbols::BuiltinTypeAtom::F32
                                | symbols::BuiltinTypeAtom::F64
                        )
                    );
                }
                _ => return false,
            }
        }
        false
    }

    pub fn display_name(&self, handle: TypeReferenceHandle) -> String {
        self.type_reference(handle).display_name(self)
    }

    pub fn display_name_with_constraints(
        &self,
        handle: TypeReferenceHandle,
        expressions: &crate::expression::ExpressionTable,
    ) -> String {
        self.type_reference(handle)
            .display_name_with_constraints(self, expressions)
    }

    #[inline]
    pub fn primitive_type(&self, handle: TypeReferenceHandle) -> Option<PrimitiveType> {
        self.type_reference(handle).primitive_type(self)
    }

    /// The terminal nominal name of a reference, looking through reference and
    /// constraint shells. Compound structural types deliberately have no
    /// nominal name: callers that need their meaning must inspect the type
    /// reference rather than fall back to a diagnostic spelling.
    pub fn named_type(&self, handle: TypeReferenceHandle) -> Option<&Identifier> {
        if !handle.is_valid() {
            return None;
        }
        match self.type_reference(handle) {
            TypeReferenceNode::Reference { referee, .. } => self.named_type(*referee),
            TypeReferenceNode::Constrained { base_type, .. } => self.named_type(*base_type),
            TypeReferenceNode::Named { name, .. } => Some(name),
            TypeReferenceNode::FixedArray { .. }
            | TypeReferenceNode::Slice { .. }
            | TypeReferenceNode::Generic { .. }
            | TypeReferenceNode::ConstExpression(_)
            | TypeReferenceNode::DynamicTrait { .. }
            | TypeReferenceNode::Unit => None,
        }
    }

    /// True when the type bottoms out in a borrowed byte slice `&[u8]` (a
    /// `Slice` whose element is `u8`, looking through a leading reference /
    /// constraints). This is the honest zero-copy RAW-bytes/text view -- a
    /// length-prefixed window into a buffer -- as opposed to an owned `[u8; N]`
    /// (a `FixedArray`, which the wire layer packs as a repeated field). It is
    /// the spelling that replaces the retired `&string` for borrowed wire text.
    pub fn is_borrowed_byte_slice(&self, handle: TypeReferenceHandle) -> bool {
        let mut handle = handle;
        loop {
            if !handle.is_valid() {
                return false;
            }
            match self.type_reference(handle) {
                TypeReferenceNode::Reference { referee, .. } => handle = *referee,
                TypeReferenceNode::Constrained { base_type, .. } => handle = *base_type,
                TypeReferenceNode::Slice { element_type } => {
                    return self.primitive_type(*element_type) == Some(PrimitiveType::U8);
                }
                _ => return false,
            }
        }
    }

    /// The arithmetic domain (`T in Wrapping/Saturating/Trapping`, decision 17)
    /// declared on this type, looking through a leading reference / nested
    /// constraints. `Exact` when none is declared.
    pub fn arithmetic_domain(
        &self,
        handle: TypeReferenceHandle,
    ) -> numerics::arithmetic::ArithmeticDomain {
        self.type_reference(handle).arithmetic_domain(self)
    }

    pub fn type_symbol(&self, handle: TypeReferenceHandle) -> SymbolHandle {
        self.type_reference(handle).type_symbol(self)
    }
}

impl Default for TypeReferenceTable {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum TypeReferenceNode {
    Reference {
        referee: TypeReferenceHandle,
        access: language_core::ReferenceAccess,
        /// Explicit lifetime name (`&'buf T`), frozen decision 15 stage 2. A
        /// borrow-region tag only — no symbol, ignored by layout/codegen and by
        /// structural type equality; consulted solely by the borrow checker
        /// (see `checks/borrows/elision.rs`).
        lifetime: Option<Identifier>,
    },
    Constrained {
        base_type: TypeReferenceHandle,
        constraints: HandleSpan<TypeConstraintNode>,
    },
    FixedArray {
        element_type: TypeReferenceHandle,
        length: FixedArrayLength,
    },
    Slice {
        element_type: TypeReferenceHandle,
    },
    Generic {
        base_symbol: SymbolHandle,
        base_name: Identifier,
        /// Erased borrow-region arguments. They are retained for lifetime
        /// checking but ignored by runtime generic identity and layout.
        lifetime_arguments: Vec<Identifier>,
        arguments: HandleSpan<TypeReferenceHandle>,
    },
    /// A PDI3 proof-static expression over const binders. The expression is
    /// erased with its enclosing domain constraint but retains resolved names
    /// and selected-operation evidence for semantic identity and checking.
    ConstExpression(crate::expression::ExpressionHandle),
    /// `dyn Trait<Args>`: a runtime selector of one named implementation of
    /// this exact trait application. It packages no data instance; calls
    /// supply every data value as an ordinary argument.
    DynamicTrait {
        symbol: SymbolHandle,
        name: Identifier,
        arguments: HandleSpan<TypeReferenceHandle>,
    },
    Named {
        symbol: SymbolHandle,
        name: Identifier,
    },
    #[default]
    Unit,
}

/// The `(Constrained, Constrained)` operand pair — same-variant
/// comparisons destructure `base_type` on both sides and recur.
#[macro_export]
macro_rules! constrained_type_reference_pair {
    ($actual:ident, $required:ident) => {
        (
            $crate::types::TypeReferenceNode::Constrained {
                base_type: $actual,
                ..
            },
            $crate::types::TypeReferenceNode::Constrained {
                base_type: $required,
                ..
            },
        )
    };
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixedArrayLength {
    Literal(usize),
    /// Exact symbolic constant-position use, pending owned evaluation.
    Expression(TypeReferenceHandle),
    ConstParameter {
        symbol: SymbolHandle,
        name: Identifier,
    },
    /// A zero-argument machine call in length position (`[u8; table_size()]`).
    /// The orchestration const-eval pass replaces this with `Literal` before
    /// checking/layout; downstream consumers never see it in a well-formed
    /// pipeline (comptime stage 1).
    ConstCall {
        /// Exact declaration selected in the extent's original source scope.
        machine: SymbolHandle,
        name: Identifier,
        source_span: source::SourceSpan,
    },
}

impl Default for FixedArrayLength {
    fn default() -> Self {
        Self::Literal(0)
    }
}

impl From<usize> for FixedArrayLength {
    fn from(value: usize) -> Self {
        Self::Literal(value)
    }
}

impl fmt::Display for FixedArrayLength {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Literal(value) => write!(formatter, "{value}"),
            Self::Expression(_) => formatter.write_str("<constant expression>"),
            Self::ConstParameter { name, .. } => write!(formatter, "{name}"),
            Self::ConstCall { name, .. } => write!(formatter, "{name}()"),
        }
    }
}

impl TypeReferenceNode {
    pub fn primitive_type(&self, table: &TypeReferenceTable) -> Option<PrimitiveType> {
        match self {
            TypeReferenceNode::Reference { .. } => None,
            TypeReferenceNode::Constrained { base_type, .. } => table.primitive_type(*base_type),
            TypeReferenceNode::Named { name, .. } => PrimitiveType::from_name(name),
            TypeReferenceNode::DynamicTrait { .. } => Some(PrimitiveType::Selector),
            TypeReferenceNode::FixedArray { .. }
            | TypeReferenceNode::Slice { .. }
            | TypeReferenceNode::Generic { .. }
            | TypeReferenceNode::ConstExpression(_)
            | TypeReferenceNode::Unit => None,
        }
    }

    pub fn arithmetic_domain(
        &self,
        table: &TypeReferenceTable,
    ) -> numerics::arithmetic::ArithmeticDomain {
        match self {
            TypeReferenceNode::Reference { referee, .. } => table.arithmetic_domain(*referee),
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => table
                .constraints(*constraints)
                .iter()
                .find_map(|constraint| match constraint {
                    TypeConstraintNode::ArithmeticDomain(domain) => Some(*domain),
                    _ => None,
                })
                .unwrap_or_else(|| table.arithmetic_domain(*base_type)),
            _ => numerics::arithmetic::ArithmeticDomain::Exact,
        }
    }

    pub fn type_symbol(&self, table: &TypeReferenceTable) -> SymbolHandle {
        match self {
            TypeReferenceNode::Reference { referee, .. } => table.type_symbol(*referee),
            TypeReferenceNode::Constrained { base_type, .. } => table.type_symbol(*base_type),
            TypeReferenceNode::FixedArray { element_type, .. } => table.type_symbol(*element_type),
            TypeReferenceNode::Slice { element_type } => table.type_symbol(*element_type),
            TypeReferenceNode::Generic {
                base_symbol,
                base_name,
                ..
            } => {
                if PrimitiveType::from_name(base_name.as_str()).is_some() {
                    SymbolHandle::invalid()
                } else {
                    *base_symbol
                }
            }
            TypeReferenceNode::DynamicTrait { symbol, .. } => *symbol,
            TypeReferenceNode::ConstExpression(_) => SymbolHandle::invalid(),
            TypeReferenceNode::Named { symbol, name } => {
                if PrimitiveType::from_name(name.as_str()).is_some() {
                    SymbolHandle::invalid()
                } else {
                    *symbol
                }
            }
            TypeReferenceNode::Unit => SymbolHandle::invalid(),
        }
    }

    /// The declared symbol, if this reference is a plain named leaf —
    /// primitive spellings included; `type_symbol` is the primitive-aware
    /// judgment. `None` for any other node shape.
    pub fn as_named_symbol(&self) -> Option<SymbolHandle> {
        let TypeReferenceNode::Named { symbol, .. } = self else {
            return None;
        };
        Some(*symbol)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeConstraintNode {
    Named(Identifier),
    Range {
        minimum: crate::expression::ExpressionHandle,
        maximum: crate::expression::ExpressionHandle,
        /// Describes the authored endpoint, not an already-subtracted value.
        /// Normalize exclusive ends in proof integers after checking them.
        end_inclusive: bool,
    },
    ArithmeticDomain(numerics::arithmetic::ArithmeticDomain),
    /// A declared domain on a carrier (`[u8] in Utf8`); ch8.
    Domain(DomainConstraint),
}

/// One closed semantic subject for a domain constraint.
///
/// `DomainConstraint::name` is retained only for diagnostics. Compiler-owned
/// meaning must travel through this enum rather than an invalid symbol paired
/// with a significant string. A symbol-backed package declaration always
/// remains `Declared`, regardless of its diagnostic spelling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DomainConstraintSubject {
    #[default]
    Declared,
    Carry(language_semantics::CarryPermission),
    Value(language_semantics::value_domain::ValueDomain),
    OmegaLayout {
        grammar: OmegaLayoutGrammar,
    },
}

/// The closed grammar selected by a compiler-owned `OmegaLayout` constraint.
///
/// The canonical one-argument surface selects `Derived`. Explicit grammar
/// arguments remain unclassified until the language implements them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OmegaLayoutGrammar {
    Derived,
}

impl OmegaLayoutGrammar {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Derived => "derived",
        }
    }
}

/// DOM1's normalized binding-site carrier. The authored short name remains
/// diagnostics-only; `subject` retains compiler-owned meaning, while a valid
/// symbol marks a declared, carrier-compatible domain resolved after all typed
/// roots exist.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DomainConstraint {
    pub name: Identifier,
    /// Closed canonical const atoms, or a direct const-binder leaf while a
    /// generic template remains structural. These handles live in the owning
    /// typed type-reference table and erase with the domain constraint.
    pub arguments: Vec<TypeReferenceHandle>,
    pub subject: DomainConstraintSubject,
    pub symbol: SymbolHandle,
    pub semantic_id: language_semantics::SemanticDomainId,
    pub classification: Option<language_semantics::DomainClassification>,
    pub predicate_body: language_semantics::DomainPredicateBody,
    pub semantic_roles: language_semantics::DomainSemanticRoles,
    pub establishment_routes: Vec<language_semantics::DomainEstablishmentRoute>,
    /// Temporary source custody retained only until carrier-aware domain
    /// normalization can bind the authored spelling to an exact symbol and
    /// move the occurrence into the declaration-selection ledger.
    pub authored_selection: Option<AuthoredDomainConstraintSelection>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthoredDomainConstraintSelection {
    pub source_span: source::SourceSpan,
    pub exposure: language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure,
}

impl DomainConstraint {
    pub fn as_str(&self) -> &str {
        self.name.as_str()
    }
}

impl fmt::Display for DomainConstraint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.name.fmt(formatter)
    }
}

impl TypeConstraintNode {
    fn copy_from(
        &self,
        source_expressions: &crate::expression::ExpressionTable,
        target_expressions: &mut crate::expression::ExpressionTable,
    ) -> Self {
        match self {
            TypeConstraintNode::Named(name) => Self::Named(name.clone()),
            TypeConstraintNode::Domain(domain) => Self::Domain(domain.clone()),
            TypeConstraintNode::Range {
                minimum,
                maximum,
                end_inclusive,
            } => Self::Range {
                minimum: target_expressions.copy_from(source_expressions, *minimum),
                maximum: target_expressions.copy_from(source_expressions, *maximum),
                end_inclusive: *end_inclusive,
            },
            TypeConstraintNode::ArithmeticDomain(domain) => Self::ArithmeticDomain(*domain),
        }
    }
}

impl Default for TypeConstraintNode {
    fn default() -> Self {
        Self::Named(Identifier::default())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimitiveType {
    Bool,
    F32,
    F64,
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    /// A pointer-width address, distinct from counts (see
    /// wiki/spec/language/counts_and_addresses.md). Naive
    /// pointer-width for now -- rides the 8-byte unsigned path.
    Addr,
    /// The runtime carrier of a `dyn Trait<Args>` selector: one opaque
    /// pointer-width word naming an implementation table. It is never
    /// spelled, takes no literals, ranges or operators, and only
    /// conversions from a named implementation produce it.
    Selector,
}

/// U8 through U64 — the unsigned integer widths.
#[macro_export]
macro_rules! unsigned_integer_primitive_pattern {
    () => {
        $crate::types::PrimitiveType::U8
            | $crate::types::PrimitiveType::U16
            | $crate::types::PrimitiveType::U32
            | $crate::types::PrimitiveType::U64
    };
}

impl PrimitiveType {
    /// The primitive a fixed-width integer [`BuiltinTypeAtom`] names; the
    /// arbitrary-width `Int`/`UInt`, `Address`, and every non-integer atom
    /// map to `None`.
    pub fn from_builtin_atom(atom: symbols::BuiltinTypeAtom) -> Option<Self> {
        Some(match atom {
            symbols::BuiltinTypeAtom::I8 => Self::I8,
            symbols::BuiltinTypeAtom::I16 => Self::I16,
            symbols::BuiltinTypeAtom::I32 => Self::I32,
            symbols::BuiltinTypeAtom::I64 => Self::I64,
            symbols::BuiltinTypeAtom::U8 => Self::U8,
            symbols::BuiltinTypeAtom::U16 => Self::U16,
            symbols::BuiltinTypeAtom::U32 => Self::U32,
            symbols::BuiltinTypeAtom::U64 => Self::U64,
            _ => return None,
        })
    }

    /// A signed or unsigned fixed-width integer — the `I*`/`U*` carriers,
    /// excluding `Addr` and `Selector`, which are pointer-width but never
    /// integer-typed.
    #[inline]
    pub fn is_integer(&self) -> bool {
        matches!(
            self,
            Self::I8
                | Self::I16
                | Self::I32
                | Self::I64
                | Self::U8
                | Self::U16
                | Self::U32
                | Self::U64
        )
    }

    #[inline]
    pub fn from_name(name: &str) -> Option<Self> {
        // Every primitive and atomic spelling starts with one of these bytes;
        // data, machine and trait names fail here without string compares.
        if !matches!(
            name.as_bytes().first(),
            Some(b'b' | b'f' | b'i' | b'u' | b'a' | b'A')
        ) {
            return None;
        }
        match name {
            "bool" => Some(Self::Bool),
            "f32" => Some(Self::F32),
            "f64" => Some(Self::F64),
            "i8" => Some(Self::I8),
            "i16" => Some(Self::I16),
            "i32" => Some(Self::I32),
            "i64" => Some(Self::I64),
            "u8" => Some(Self::U8),
            "u16" => Some(Self::U16),
            "u32" => Some(Self::U32),
            "u64" => Some(Self::U64),
            "addr" => Some(Self::Addr),
            // Atomic types currently share the size/alignment of their
            // underlying primitives. Ordering legality and target instruction
            // strength are separate semantic/lowering obligations.
            "AtomicBool" => Some(Self::Bool),
            "AtomicU32" => Some(Self::U32),
            "AtomicU64" => Some(Self::U64),
            name if name.starts_with("AtomicBool#PlacedField<") => Some(Self::Bool),
            name if name.starts_with("AtomicU32#PlacedField<") => Some(Self::U32),
            name if name.starts_with("AtomicU64#PlacedField<") => Some(Self::U64),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Bool => "bool",
            Self::F32 => "f32",
            Self::F64 => "f64",
            Self::I8 => "i8",
            Self::I16 => "i16",
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::U8 => "u8",
            Self::U16 => "u16",
            Self::U32 => "u32",
            Self::U64 => "u64",
            Self::Addr => "addr",
            Self::Selector => "selector",
        }
    }

    /// The representable `(minimum, maximum)` range of an integer carrier —
    /// `Addr` included — as `i128`. `None` for `Bool`, the floats and
    /// `Selector`, which have no integer range.
    pub fn integer_bounds(self) -> Option<(i128, i128)> {
        Some(match self {
            Self::I8 => (i128::from(i8::MIN), i128::from(i8::MAX)),
            Self::I16 => (i128::from(i16::MIN), i128::from(i16::MAX)),
            Self::I32 => (i128::from(i32::MIN), i128::from(i32::MAX)),
            Self::I64 => (i128::from(i64::MIN), i128::from(i64::MAX)),
            Self::U8 => (0, i128::from(u8::MAX)),
            Self::U16 => (0, i128::from(u16::MAX)),
            Self::U32 => (0, i128::from(u32::MAX)),
            Self::U64 | Self::Addr => (0, i128::from(u64::MAX)),
            Self::Bool | Self::F32 | Self::F64 | Self::Selector => return None,
        })
    }

    /// The fixed-range integer carriers: every range-constrained spelling
    /// except the two finite floats.
    pub fn accepts_integer_literal(self) -> bool {
        self.accepts_range_constraint() && !self.accepts_finite_constraint()
    }

    pub fn accepts_float_literal(self) -> bool {
        matches!(self, Self::F32 | Self::F64)
    }

    /// Whether the primitive is one of the eight fixed-width integers
    /// (`i8`–`u64`); the pointer-width `Addr` is not a member.
    pub fn is_fixed_integer(self) -> bool {
        self.is_integer()
    }

    /// Whether an integer primitive is signed. `usize` is treated as unsigned.
    /// Non-integer primitives report `true` (the signed/default form), which is
    /// the safe choice for codegen that only distinguishes signed vs unsigned
    /// integer machine operations.
    pub fn is_signed_integer(self) -> bool {
        !matches!(
            self,
            Self::U8 | Self::U16 | Self::U32 | Self::U64 | Self::Addr | Self::Selector
        )
    }

    /// Byte size of a scalar primitive. Single source of truth for the backend's
    /// scalar-width decisions (conversions, guard operands, and storage layout).
    pub fn scalar_byte_size(self) -> Option<usize> {
        match self {
            Self::Bool | Self::I8 | Self::U8 => Some(1),
            Self::I16 | Self::U16 => Some(2),
            Self::F32 | Self::I32 | Self::U32 => Some(4),
            Self::F64 | Self::I64 | Self::U64 | Self::Addr | Self::Selector => Some(8),
        }
    }

    pub fn accepts_range_constraint(self) -> bool {
        matches!(
            self,
            Self::F32
                | Self::F64
                | Self::I8
                | Self::I16
                | Self::I32
                | Self::I64
                | Self::U8
                | Self::U16
                | Self::U32
                | Self::U64
                | Self::Addr
        )
    }

    pub fn accepts_finite_constraint(self) -> bool {
        matches!(self, Self::F32 | Self::F64)
    }
    /// The primitive integer type a landed literal integer type denotes.
    pub fn from_landed_integer(kind: numerics::literals::LandedIntegerType) -> Self {
        use numerics::literals::LandedIntegerType;
        match kind {
            LandedIntegerType::I8 => Self::I8,
            LandedIntegerType::I16 => Self::I16,
            LandedIntegerType::I32 => Self::I32,
            LandedIntegerType::I64 => Self::I64,
            LandedIntegerType::U8 => Self::U8,
            LandedIntegerType::U16 => Self::U16,
            LandedIntegerType::U32 => Self::U32,
            LandedIntegerType::U64 => Self::U64,
            LandedIntegerType::Addr => Self::Addr,
        }
    }

    /// The landed literal integer type this primitive lands as, or `None`
    /// for `Selector`, `Bool` and the floats.
    pub fn landed_landing(self) -> Option<numerics::literals::LandedIntegerType> {
        use numerics::literals::LandedIntegerType;
        Some(match self {
            Self::I8 => LandedIntegerType::I8,
            Self::I16 => LandedIntegerType::I16,
            Self::I32 => LandedIntegerType::I32,
            Self::I64 => LandedIntegerType::I64,
            Self::U8 => LandedIntegerType::U8,
            Self::U16 => LandedIntegerType::U16,
            Self::U32 => LandedIntegerType::U32,
            Self::U64 => LandedIntegerType::U64,
            Self::Addr => LandedIntegerType::Addr,
            Self::Selector | Self::Bool | Self::F32 | Self::F64 => return None,
        })
    }

    /// The fixed-width integer landing when the destination is one of the eight
    /// sized integer carriers. `Addr` is excluded: its width and meaning need
    /// target authority, not a guess from today's native representation.
    pub fn landed_landing_fixed(self) -> Option<numerics::literals::LandedIntegerType> {
        match self.landed_landing() {
            Some(numerics::literals::LandedIntegerType::Addr) | None => None,
            fixed => fixed,
        }
    }

    /// The sign and bit width of the eight fixed-width integers
    /// (`i8`–`u64`); `Addr`, `Selector`, `Bool` and the floats return `None`.
    pub fn integer_sign_bits(self) -> Option<(semantic_vocabulary::IntegerSign, u16)> {
        use semantic_vocabulary::IntegerSign;
        Some(match self {
            Self::I8 => (IntegerSign::Signed, 8),
            Self::I16 => (IntegerSign::Signed, 16),
            Self::I32 => (IntegerSign::Signed, 32),
            Self::I64 => (IntegerSign::Signed, 64),
            Self::U8 => (IntegerSign::Unsigned, 8),
            Self::U16 => (IntegerSign::Unsigned, 16),
            Self::U32 => (IntegerSign::Unsigned, 32),
            Self::U64 => (IntegerSign::Unsigned, 64),
            Self::Addr | Self::Selector | Self::Bool | Self::F32 | Self::F64 => {
                return None;
            }
        })
    }

    /// [`Self::from_landed_integer`] minus `Addr`, for callers rejecting
    /// the address lane.
    pub fn non_address_landed_integer(kind: numerics::literals::LandedIntegerType) -> Option<Self> {
        match kind {
            numerics::literals::LandedIntegerType::Addr => None,
            kind => Some(Self::from_landed_integer(kind)),
        }
    }
}

/// The builtin type atom a landed literal integer type denotes.
pub fn builtin_type_atom(kind: numerics::literals::LandedIntegerType) -> symbols::BuiltinTypeAtom {
    use numerics::literals::LandedIntegerType;
    use symbols::BuiltinTypeAtom;
    match kind {
        LandedIntegerType::I8 => BuiltinTypeAtom::I8,
        LandedIntegerType::I16 => BuiltinTypeAtom::I16,
        LandedIntegerType::I32 => BuiltinTypeAtom::I32,
        LandedIntegerType::I64 => BuiltinTypeAtom::I64,
        LandedIntegerType::U8 => BuiltinTypeAtom::U8,
        LandedIntegerType::U16 => BuiltinTypeAtom::U16,
        LandedIntegerType::U32 => BuiltinTypeAtom::U32,
        LandedIntegerType::U64 => BuiltinTypeAtom::U64,
        LandedIntegerType::Addr => BuiltinTypeAtom::Address,
    }
}

#[cfg(test)]
mod tests {
    use super::{FixedArrayLength, TypeReferenceNode, TypeReferenceTable};
    use crate::name::Identifier;
    use language_core::ReferenceAccess;
    use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};

    #[test]
    fn instantiation_rebuilds_only_rows_that_name_a_binder() {
        let mut builder = SymbolTableBuilder::default();
        let binder = builder.insert_root(SymbolKind::TypeParameter, SymbolNameRef::Static("T"));
        let item = builder.insert_root(SymbolKind::Data, SymbolNameRef::Static("Item"));
        let other = builder.insert_root(SymbolKind::Data, SymbolNameRef::Static("Other"));
        let mut table = TypeReferenceTable::new();
        let formal = table.insert(TypeReferenceNode::Named {
            symbol: binder,
            name: Identifier::from("T"),
        });
        let authored = TypeReferenceNode::Reference {
            referee: formal,
            access: ReferenceAccess::Shared,
            lifetime: None,
        };
        let borrowed = table.insert(authored.clone());
        let unrelated = table.insert(TypeReferenceNode::Named {
            symbol: other,
            name: Identifier::from("Other"),
        });
        let actual = table.insert(TypeReferenceNode::Named {
            symbol: item,
            name: Identifier::from("Item"),
        });
        let substitutions = [(binder, actual)];

        let instantiated = table
            .instantiate(borrowed, &substitutions)
            .expect("a reference to the binder instantiates");
        assert_ne!(instantiated, borrowed);
        assert_eq!(
            table.type_reference(instantiated),
            &TypeReferenceNode::Reference {
                referee: actual,
                access: ReferenceAccess::Shared,
                lifetime: None,
            }
        );
        // The authored declaration keeps its generic row.
        assert_eq!(table.type_reference(borrowed), &authored);
        // A row naming no binder, and the bare binder itself, are shared.
        assert_eq!(table.instantiate(unrelated, &substitutions), Ok(unrelated));
        assert_eq!(table.instantiate(formal, &substitutions), Ok(actual));
    }

    #[test]
    fn instantiation_refuses_a_binder_spelling_an_array_length() {
        let mut builder = SymbolTableBuilder::default();
        let binder = builder.insert_root(SymbolKind::TypeParameter, SymbolNameRef::Static("N"));
        let element = builder.insert_root(SymbolKind::Data, SymbolNameRef::Static("Item"));
        let mut table = TypeReferenceTable::new();
        let element = table.insert(TypeReferenceNode::Named {
            symbol: element,
            name: Identifier::from("Item"),
        });
        let array = table.insert(TypeReferenceNode::FixedArray {
            element_type: element,
            length: FixedArrayLength::ConstParameter {
                symbol: binder,
                name: Identifier::from("N"),
            },
        });
        assert!(table.instantiate(array, &[(binder, element)]).is_err());
    }
}
