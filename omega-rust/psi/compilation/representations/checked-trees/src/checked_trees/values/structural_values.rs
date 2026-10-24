//! Fresh structural values and their selected scalar dispatch paths.

use crate::{CheckedOperatorUseFact, CheckedScalarComputationHandle, CheckedScalarDispatchPattern};
use arena::Arena;
use arena::{Handle, HandleSpan};
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, TableMatchArm};
use typed_trees::type_identity::TypeIdentityId;
use typed_trees::types::TypeReferenceHandle;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedStructuralValuePlans {
    pub roots: Arena<CheckedStructuralValueRoot>,
    pub nodes: Arena<CheckedStructuralValue>,
    pub dispatch_arms: Arena<CheckedStructuralDispatchArm>,
    pub record_fields: Arena<CheckedStructuralRecordField>,
    call_schedules: Arena<StructuralCallSchedule>,
    calls: Arena<crate::CheckedStructuralValueCall>,
    roots_by_state: arena::OwnedIndex<
        symbols::SymbolKeyMap<SymbolHandle, Vec<Handle<CheckedStructuralValueRoot>>>,
    >,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct StructuralCallSchedule {
    value: CheckedStructuralValueHandle,
    /// Exact predecessor namespace of this independently checked value root.
    preceding_bindings: u32,
    calls: HandleSpan<crate::CheckedStructuralValueCall>,
}

pub type CheckedStructuralValueHandle = Handle<CheckedStructuralValue>;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedStructuralValueRoot {
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    pub statement_ordinal: u32,
    pub expression: ExpressionHandle,
    pub type_reference: TypeReferenceHandle,
    pub root: CheckedStructuralValueHandle,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedStructuralValue {
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    pub statement_index: u32,
    pub expression: ExpressionHandle,
    /// Diagnostic location of the occurrence; lowering refusals cite it
    /// without reading the expression back.
    pub source_span: source::SourceSpan,
    /// Complete occurrence type, including its receiving obligations. The
    /// linked source judgments, not this annotation, prove membership.
    pub type_reference: TypeReferenceHandle,
    pub qualification_decisions: HandleSpan<crate::CheckedValueQualification>,
    pub kind: CheckedStructuralValueKind,
}

/// A view borrows either the collection's existing elements or an exactly
/// tiled byte region. The latter retains its scalar computation handle so the
/// offset is evaluated once at formation, before any later mutation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedElementViewRegion {
    WholeCollection,
    RepresentationBytes {
        start: CheckedScalarComputationHandle,
        end: u64,
    },
}

/// The exact semantic edge authorizing a whole-value quotient adaptation.
/// These are checked occurrence coordinates, never layout-based permissions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedQuotientAdaptationLicense {
    Mint,
    Input {
        request: ExpressionHandle,
        position: u32,
    },
    Result {
        request: ExpressionHandle,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedStructuralValueKind {
    /// Selected executable demand, not a runtime invocation or constant value.
    NativeCodeLayout {
        selected_state: SymbolHandle,
    },
    /// Exact sealed source request, distinct from an ordinary FlowCallFact.
    /// Publication must join the checked correspondence for this request.
    /// The ordinary selected body computes the result; its primitive result
    /// may be held temporarily in structural storage for expression composition.
    QuotientCall {
        request: ExpressionHandle,
        representative_state: SymbolHandle,
        arguments: Vec<CheckedStructuralValueHandle>,
    },
    /// Preserve the chosen representative's bytes at a checked nominal edge.
    /// The child is evaluated once before the copy; source and receiving type
    /// identities stay distinct through Terminal publication.
    QuotientAdaptation {
        source: CheckedStructuralValueHandle,
        quotient: TypeReferenceHandle,
        license: CheckedQuotientAdaptationLicense,
    },

    /// An evaluated scalar established in its declared structural storage.
    /// The scalar computation supplies the value; the receiving type supplies
    /// an independent initialization obligation, just as for a primitive local.
    Scalar {
        value: CheckedScalarComputationHandle,
    },
    /// Capture borrowed storage into an owned permission carrier.
    /// A record field consumes this carrier, never a scalar referent snapshot.
    Reference {
        source: crate::CheckedUnitStructuralArgumentPlan,
        /// Authored index evaluations survive fixed endpoint geometry. A call
        /// that proves one ordinal still executes before the reference forms.
        selectors: HandleSpan<CheckedScalarComputationHandle>,
        /// The checked source interpretation is independent of the logical
        /// referee. A view retains its admitted storage codec at formation.
        storage: crate::CheckedReferenceStorage,
        /// Exact supplying type of the selected storage endpoint. A view's
        /// logical referee cannot reconstruct this catalog dependency.
        backing_type: TypeReferenceHandle,
        /// The backing producer when the source has no existing storage home.
        /// Zero names existing storage. Evaluation lends this child; it never
        /// transfers the child into the descriptor or its containing record.
        source_value: CheckedStructuralValueHandle,
    },
    Case(crate::CheckedScalarCaseConstruction),
    /// An owned construction of a named case whose payload fields are not all
    /// scalar. Each field composes exactly as it does inside a `Record` — a
    /// scalar computation or a nested structural value — while the selected
    /// case contributes the discriminated tag a bare field set cannot spell.
    StructuralCase {
        data_symbol: SymbolHandle,
        case: SymbolHandle,
        fields: HandleSpan<CheckedStructuralRecordField>,
    },
    Call {
        source_call: Handle<crate::FlowCallFact>,
    },
    Record {
        data_symbol: SymbolHandle,
        fields: HandleSpan<CheckedStructuralRecordField>,
    },
    /// Parameter and local sources transfer existing storage; conditional
    /// transfer and residual ownership come from checked flow. A byte-literal
    /// source instead constructs fresh bounded owned storage at its declared
    /// carrier, never transferring ownership of immutable image bytes.
    Place(crate::CheckedUnitStructuralArgumentPlan),
    /// A shared descriptor over immutable image bytes. Type-directed value
    /// production retains the literal's admitted byte domains in any expression
    /// position. A containing record borrows the image-backed descriptor;
    /// named and anonymous root values copy it into their ordinary result home.
    ByteLiteralView {
        bytes: Vec<u8>,
    },
    /// A borrowed `&[T]` view of the contiguous elements an owned collection
    /// place holds. `source` is that lent place under its shared loan, taken
    /// from checked borrow admission rather than from the authored view
    /// spelling; the view carries its own stored length and copies no
    /// elements, so the lent storage keeps its owner and extent.
    BorrowedSliceView {
        source: crate::CheckedUnitStructuralArgumentPlan,
        region: CheckedElementViewRegion,
    },
    /// A mutably borrowed `&mut [T]` view of the contiguous elements an owned
    /// collection place holds. `source` is that lent place under its mutable
    /// loan, taken from checked borrow admission rather than from the authored
    /// view spelling; the view carries its own stored length and copies no
    /// elements, and its mutable custody lets element writes and element
    /// borrows reach the lent storage.
    MutableSliceView {
        source: crate::CheckedUnitStructuralArgumentPlan,
        region: CheckedElementViewRegion,
    },
    /// An unrestricted stored subtree observed and copied into a fresh owned
    /// place, without moving a child out of the readable source. Record,
    /// fixed-array and view projections retain the same source/path contract;
    /// view selectors carry the descriptor's live extent, not an array length.
    /// `source` lends that place under shared borrow, with `type_identity`
    /// naming the projected leaf's complete stored carrier.
    CopiedStructuralPlace {
        source: crate::CheckedUnitStructuralArgumentPlan,
        /// The projected leaf's type reference: lowering registers it with the
        /// unit's structural catalog so the leaf's `type_identity` resolves.
        leaf: TypeReferenceHandle,
    },
    /// A child selected from resident storage or an evaluated value along an
    /// exact field/fixed-index path. The checked use copies or transfers that
    /// child; a transfer settles its source's residual on the selected edge.
    /// `type_identity` is the projected carrier, not its containing owner's.
    Projection {
        source: CheckedStructuralValueHandle,
        path: Vec<crate::CheckedUnitStructuralPathSegment>,
        type_identity: TypeIdentityId,
        /// A selected copy leaves its source's independent disposal debt live.
        /// Whole-value forwarding has no selection and does not copy here.
        copies_selected_value: bool,
    },
    Dispatch {
        subject: crate::CheckedDispatchSubject,
        arms: HandleSpan<CheckedStructuralDispatchArm>,
    },
    /// An element-wise owned construction of a fixed array of structural
    /// values. Each element is its own structural node — a record literal,
    /// a place, a call product — so field scalars, ownership, and calls
    /// compose exactly as they do inside a record constructor; the array
    /// contributes only their ordered container.
    FixedArray {
        elements: Vec<CheckedStructuralValueHandle>,
    },
    /// A supplied closed primitive array (`[i32; 4]`, `[[u8; 2]; 2]`) inside a
    /// structural value. A Terminal primitive leaf has no arithmetic policy, so
    /// the whole array is one scalar establishment: `elements` is the complete
    /// leaf roster in row-major (outer-index-first) authored order, each leaf
    /// retained under `StructuralArrayElement` keyed by this node's expression.
    /// Owned structural elements keep the element-wise `FixedArray` route.
    ScalarArray {
        elements: Vec<CheckedScalarComputationHandle>,
    },
    /// An omitted record field's zero value: a `[scalar; N]` leaf
    /// established all-zero. Membership decisions use the same exact-value
    /// qualification span as other constructors. `element` is the planner's
    /// literal-zero computation in the declared element carrier; the single
    /// computation supplies every leaf, so `element_count` is the declared
    /// flattened leaf count of the enclosing array.
    ZeroedScalarArray {
        element: CheckedScalarComputationHandle,
        element_count: u64,
    },
    /// An omitted record field's zero value over a structural (non-scalar)
    /// member: `[Record; N]` elements or a nested record, all leaves zero.
    /// No element roster exists — `EstablishZeroedStructural` names the whole
    /// zero-constructible shape and owns its zero semantics.
    ZeroedStructural {
        projected_qualifications: Vec<crate::CheckedStructuralPathQualification>,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedStructuralRecordField {
    pub field: SymbolHandle,
    pub expression: ExpressionHandle,
    pub type_reference: TypeReferenceHandle,
    pub value: CheckedStructuralRecordFieldValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedStructuralRecordFieldValue {
    Scalar(CheckedScalarComputationHandle),
    Structural(CheckedStructuralValueHandle),
}

impl Default for CheckedStructuralRecordFieldValue {
    fn default() -> Self {
        Self::Scalar(Handle::invalid())
    }
}

impl Default for CheckedStructuralValueKind {
    fn default() -> Self {
        Self::Case(crate::CheckedScalarCaseConstruction::default())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedStructuralDispatchArm {
    pub source_arm: Handle<TableMatchArm>,
    pub equality_use: Handle<CheckedOperatorUseFact>,
    pub pattern: CheckedScalarDispatchPattern,
    pub value: CheckedStructuralValueHandle,
}

impl CheckedStructuralValuePlans {
    /// Structural values actually reached from a retained root, including
    /// computed backing storage. This is the value graph, not the referent
    /// type graph: a stored pointer alone never demands its pointee's storage.
    /// Catalog and evaluation consumers share these edges so an anonymous
    /// backing cannot depend on an unrelated expression registering its type.
    pub fn value_dependencies(
        &self,
        root: CheckedStructuralValueHandle,
    ) -> Option<Vec<CheckedStructuralValueHandle>> {
        let mut pending = vec![root];
        let mut values = Vec::new();
        while let Some(value) = pending.pop() {
            if !self.nodes.is_valid(value) {
                return None;
            }
            if values.contains(&value) {
                continue;
            }
            values.push(value);
            match &self.nodes.get(value).kind {
                CheckedStructuralValueKind::QuotientCall { arguments, .. } => {
                    pending.extend(arguments)
                }
                CheckedStructuralValueKind::Record { fields, .. }
                | CheckedStructuralValueKind::StructuralCase { fields, .. } => {
                    for field in self.record_fields.span(*fields)? {
                        if let CheckedStructuralRecordFieldValue::Structural(child) = field.value {
                            pending.push(child);
                        }
                    }
                }
                CheckedStructuralValueKind::Dispatch { subject, arms } => {
                    let captured = subject.structural_value();
                    if captured.is_valid() {
                        pending.push(captured);
                    }
                    pending.extend(self.dispatch_arms.span(*arms)?.iter().map(|arm| arm.value));
                }
                CheckedStructuralValueKind::Projection { source, .. }
                | CheckedStructuralValueKind::QuotientAdaptation { source, .. } => {
                    pending.push(*source)
                }
                CheckedStructuralValueKind::FixedArray { elements } => pending.extend(elements),
                CheckedStructuralValueKind::Reference { source_value, .. } => {
                    if source_value.is_valid() {
                        pending.push(*source_value);
                    }
                }
                _ => {}
            }
        }
        Some(values)
    }

    /// Executable calls belong to the exact value DAG, not its eventual local,
    /// argument or observation consumer. Catalog order is not evaluation order.
    pub fn call_schedule(
        &self,
        value: CheckedStructuralValueHandle,
    ) -> Option<(u32, &[crate::CheckedStructuralValueCall])> {
        let mut schedules = self
            .call_schedules
            .iter()
            .map(|(_, row)| row)
            .filter(|row| row.value == value);
        let schedule = schedules.next()?;
        if schedules.next().is_some() || !self.nodes.is_valid(value) {
            return None;
        }
        Some((
            schedule.preceding_bindings,
            self.calls.span_or_empty(schedule.calls),
        ))
    }

    pub fn retain_calls_for_root(
        &mut self,
        value: CheckedStructuralValueHandle,
        preceding_bindings: u32,
        calls: Vec<crate::CheckedStructuralValueCall>,
    ) {
        assert!(self.nodes.is_valid(value));
        assert!(
            !self
                .call_schedules
                .iter()
                .any(|(_, row)| row.value == value)
        );
        let calls = self.calls.insert_many(calls);
        self.call_schedules.insert(StructuralCallSchedule {
            value,
            preceding_bindings,
            calls,
        });
    }

    /// Retained invocation occurrences, including their normal-return schedules.
    /// Typed identities let final planning complete rows without rebuilding the
    /// value-to-call ownership index or cloning executable bodies.
    pub fn scheduled_calls(
        &self,
    ) -> impl Iterator<
        Item = (
            Handle<crate::CheckedStructuralValueCall>,
            &crate::CheckedStructuralValueCall,
        ),
    > {
        self.calls.iter()
    }

    pub fn scheduled_call_mut(
        &mut self,
        call: Handle<crate::CheckedStructuralValueCall>,
    ) -> Option<&mut crate::CheckedStructuralValueCall> {
        if !self.calls.is_valid(call) {
            return None;
        }
        Some(self.calls.get_mut(call))
    }

    fn with_state_roots<R>(
        &self,
        state: SymbolHandle,
        run: impl FnOnce(&[Handle<CheckedStructuralValueRoot>]) -> R,
    ) -> R {
        // Batch insertion and mutation have independent revision/length effects.
        // Only the arena's exact revision establishes freshness of every bucket.
        self.roots_by_state.with(
            self.roots.revision(),
            || {
                let mut by_state = symbols::SymbolKeyMap::<SymbolHandle, Vec<_>>::default();
                for (handle, root) in self.roots.iter() {
                    by_state.entry(root.state).or_default().push(handle);
                }
                by_state
            },
            |by_state| run(by_state.get(&state).map_or(&[], Vec::as_slice)),
        )
    }

    pub fn roots_for_state(&self, state: SymbolHandle) -> Vec<&CheckedStructuralValueRoot> {
        self.with_state_roots(state, |handles| {
            handles
                .iter()
                .map(|handle| self.roots.get(*handle))
                .filter(|root| root.state == state)
                .collect()
        })
    }

    pub fn roots_for_statement(
        &self,
        state: SymbolHandle,
        statement_ordinal: u32,
    ) -> Vec<&CheckedStructuralValueRoot> {
        self.with_state_roots(state, |handles| {
            handles
                .iter()
                .map(|handle| self.roots.get(*handle))
                .filter(|root| root.state == state && root.statement_ordinal == statement_ordinal)
                .collect()
        })
    }

    /// Continuation and primary returns may share a statement, but never an
    /// authored value occurrence. Rejoin both coordinates without ambiguity.
    pub fn root_for_expression(
        &self,
        state: SymbolHandle,
        statement_ordinal: u32,
        expression: ExpressionHandle,
    ) -> Option<&CheckedStructuralValueRoot> {
        self.with_state_roots(state, |handles| {
            let mut roots = handles
                .iter()
                .map(|handle| self.roots.get(*handle))
                .filter(|root| {
                    root.state == state
                        && root.statement_ordinal == statement_ordinal
                        && root.expression == expression
                });
            let root = roots.next()?;
            roots.next().is_none().then_some(root)
        })
    }

    /// Resolve a retained value dependency through authenticated saved aliases.
    /// A saved read depends on its capture's supplying subject, not an argument
    /// catalog root for that read. Prefer the exact supplying root: a nested
    /// observation may have produced a copy from another saved subject, and
    /// that completed copy must not collapse to the older capture. This lookup establishes neither
    /// execution nor dominance; scheduling must require an existing lexical
    /// completion before consuming a saved alias. Catalog-only readers may omit
    /// the statement, but still require a unique root within the state.
    pub fn completed_expression_dependency(
        &self,
        expressions: &typed_trees::expression::ExpressionTable,
        state: SymbolHandle,
        statement: Option<u32>,
        mut expression: ExpressionHandle,
    ) -> Option<&CheckedStructuralValueRoot> {
        let mut active_captures = Vec::new();
        if !expressions.expression_is_valid(expression) {
            return None;
        }
        if let typed_trees::expression::ExpressionNode::SavedSubject(saved) =
            expressions.expression(expression)
        {
            active_captures.push(saved.capture);
            expression = expressions.saved_subject_source(saved)?;
        }
        loop {
            if !expressions.expression_is_valid(expression) {
                return None;
            }
            if let typed_trees::expression::ExpressionNode::SavedSubject(saved) =
                expressions.expression(expression)
                && active_captures.contains(&saved.capture)
            {
                return None;
            }
            let root = self
                .with_state_roots(state, |handles| {
                    let mut matching =
                        handles
                            .iter()
                            .map(|handle| self.roots.get(*handle))
                            .filter(|root| {
                                root.state == state
                                    && root.expression == expression
                                    && statement
                                        .is_none_or(|statement| root.statement_ordinal == statement)
                            });
                    let root = matching.next();
                    if matching.next().is_some() {
                        Err(())
                    } else {
                        Ok(root)
                    }
                })
                .ok()?;
            if let Some(root) = root {
                return (self.nodes.is_valid(root.root)
                    && self.nodes.get(root.root).expression == expression)
                    .then_some(root);
            }
            let typed_trees::expression::ExpressionNode::SavedSubject(saved) =
                expressions.expression(expression)
            else {
                return None;
            };
            if active_captures.contains(&saved.capture) {
                return None;
            }
            active_captures.push(saved.capture);
            expression = expressions.saved_subject_source(saved)?;
        }
    }

    pub fn root_at(
        &self,
        state: SymbolHandle,
        statement_ordinal: u32,
    ) -> Option<&CheckedStructuralValueRoot> {
        self.with_state_roots(state, |handles| {
            let mut roots = handles
                .iter()
                .map(|handle| self.roots.get(*handle))
                .filter(|root| root.state == state && root.statement_ordinal == statement_ordinal);
            let root = roots.next()?;
            roots.next().is_none().then_some(root)
        })
    }
}

#[macro_export]
macro_rules! record_or_structural_case_pattern {
    ($($fields:tt)*) => {
        $crate::CheckedStructuralValueKind::Record { $($fields)* }
            | $crate::CheckedStructuralValueKind::StructuralCase { $($fields)* }
    };
}

#[cfg(test)]
mod index_tests {
    use super::{CheckedStructuralValuePlans, CheckedStructuralValueRoot, SymbolHandle};
    use typed_trees::expression::{
        ExpressionHandle, ExpressionNode, ExpressionTable, MatchPattern, TableDestructurePattern,
        TableMatchArm, TableMatchExpression, TableSavedSubject,
    };

    fn saved_subject(
        expressions: &mut ExpressionTable,
        source: ExpressionHandle,
    ) -> ExpressionHandle {
        let capture = expressions.insert_match_subject(source);
        let head = expressions.insert(ExpressionNode::Boolean(true));
        let alias = expressions.insert(ExpressionNode::SavedSubject(TableSavedSubject {
            capture,
            pattern_head: head,
        }));
        let arms = expressions.insert_match_arms([TableMatchArm {
            pattern: MatchPattern::Destructure(TableDestructurePattern {
                head,
                ..Default::default()
            }),
            value: alias,
            ..Default::default()
        }]);
        expressions.insert(ExpressionNode::Match(TableMatchExpression {
            subject: source,
            capture,
            arms,
        }));
        alias
    }

    fn retain_root(
        plans: &mut CheckedStructuralValuePlans,
        expression: ExpressionHandle,
    ) -> super::CheckedStructuralValueHandle {
        let state = SymbolHandle::from_parts(1, 1);
        let value = plans.nodes.insert(super::CheckedStructuralValue {
            machine: state,
            state,
            statement_index: 7,
            expression,
            source_span: Default::default(),
            type_reference: Default::default(),
            qualification_decisions: Default::default(),
            kind: super::CheckedStructuralValueKind::Scalar {
                value: Default::default(),
            },
        });
        plans.roots.insert(CheckedStructuralValueRoot {
            machine: state,
            state,
            statement_ordinal: 7,
            expression,
            root: value,
            ..Default::default()
        });
        value
    }

    #[test]
    fn saved_dependency_uses_defining_subject_and_preserves_a_nested_capture() {
        let mut expressions = ExpressionTable::default();
        let source = expressions.insert(ExpressionNode::Boolean(true));
        let first_alias = saved_subject(&mut expressions, source);
        let second_alias = saved_subject(&mut expressions, first_alias);
        let mut plans = CheckedStructuralValuePlans::default();
        let original = retain_root(&mut plans, source);
        let nested = retain_root(&mut plans, first_alias);
        retain_root(&mut plans, second_alias);
        let state = SymbolHandle::from_parts(1, 1);
        assert_eq!(
            plans
                .completed_expression_dependency(&expressions, state, Some(7), first_alias)
                .map(|root| root.root),
            Some(original)
        );
        assert_eq!(
            plans
                .completed_expression_dependency(&expressions, state, Some(7), second_alias)
                .map(|root| root.root),
            Some(nested)
        );
        assert!(
            plans
                .completed_expression_dependency(&expressions, state, Some(8), first_alias)
                .is_none()
        );
        assert!(
            plans
                .completed_expression_dependency(
                    &expressions,
                    SymbolHandle::from_parts(2, 1),
                    Some(7),
                    first_alias
                )
                .is_none()
        );
        retain_root(&mut plans, source);
        assert!(
            plans
                .completed_expression_dependency(&expressions, state, Some(7), first_alias)
                .is_none()
        );
    }

    #[test]
    fn saved_dependency_rejects_a_cycle_even_with_a_retained_source_root() {
        let mut expressions = ExpressionTable::default();
        let source = expressions.insert(ExpressionNode::Boolean(true));
        let alias = saved_subject(&mut expressions, source);
        *expressions.expression_mut(source) = expressions.expression(alias).clone();
        let mut plans = CheckedStructuralValuePlans::default();
        retain_root(&mut plans, source);
        assert!(
            plans
                .completed_expression_dependency(
                    &expressions,
                    SymbolHandle::from_parts(1, 1),
                    Some(7),
                    alias
                )
                .is_none()
        );
    }

    #[test]
    fn root_index_preserves_mutation_before_batch_append() {
        let mut plans = CheckedStructuralValuePlans::default();
        let original = SymbolHandle::from_parts(1, 1);
        let moved = SymbolHandle::from_parts(2, 1);
        let first = plans.roots.insert(CheckedStructuralValueRoot {
            state: original,
            ..CheckedStructuralValueRoot::default()
        });
        plans.with_state_roots(original, |handles| assert_eq!(handles, [first]));
        plans.roots.get_mut(first).state = moved;
        plans.roots.insert_many([
            CheckedStructuralValueRoot {
                state: original,
                ..CheckedStructuralValueRoot::default()
            },
            CheckedStructuralValueRoot {
                state: original,
                ..CheckedStructuralValueRoot::default()
            },
        ]);
        plans.with_state_roots(moved, |handles| assert_eq!(handles, [first]));
        assert!(plans.roots.free(first));
        plans.with_state_roots(moved, |handles| assert!(handles.is_empty()));
    }
}
