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
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct StructuralCallSchedule {
    value: CheckedStructuralValueHandle,
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

    /// Capture borrowed storage into an owned permission carrier.
    /// A record field consumes this carrier, never a scalar referent snapshot.
    Reference {
        source: crate::CheckedUnitStructuralArgumentPlan,
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
    /// An owned child projected out of `source` (a `Place` or `Call` node)
    /// along an exact field/fixed-index path. The untouched residual siblings
    /// die on the selected edge; `type_identity` is the normalized projected
    /// (leaf) type, matching the enclosing result expectation.
    Projection {
        source: CheckedStructuralValueHandle,
        path: Vec<crate::CheckedUnitStructuralPathSegment>,
        type_identity: TypeIdentityId,
    },
    Dispatch {
        subject: CheckedScalarComputationHandle,
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
    /// established all-zero — the only omitted-runtime-field carrier the
    /// zero-defaulting rule admits today. `element` is the planner's
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
    ZeroedStructural {},
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
                CheckedStructuralValueKind::Dispatch { arms, .. } => {
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
    pub fn calls_for_root(
        &self,
        value: CheckedStructuralValueHandle,
    ) -> Option<&[crate::CheckedStructuralValueCall]> {
        let mut schedules = self
            .call_schedules
            .iter()
            .map(|(_, row)| row)
            .filter(|row| row.value == value);
        let schedule = schedules.next()?;
        if schedules.next().is_some() || !self.nodes.is_valid(value) {
            return None;
        }
        Some(self.calls.span_or_empty(schedule.calls))
    }

    pub fn retain_calls_for_root(
        &mut self,
        value: CheckedStructuralValueHandle,
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
        self.call_schedules
            .insert(StructuralCallSchedule { value, calls });
    }

    /// Continuation and primary returns may share a statement, but never an
    /// authored value occurrence. Rejoin both coordinates without ambiguity.
    pub fn root_for_expression(
        &self,
        state: SymbolHandle,
        statement_ordinal: u32,
        expression: ExpressionHandle,
    ) -> Option<&CheckedStructuralValueRoot> {
        let mut roots = self.roots.iter().map(|(_, root)| root).filter(|root| {
            root.state == state
                && root.statement_ordinal == statement_ordinal
                && root.expression == expression
        });
        let root = roots.next()?;
        roots.next().is_none().then_some(root)
    }

    pub fn root_at(
        &self,
        state: SymbolHandle,
        statement_ordinal: u32,
    ) -> Option<&CheckedStructuralValueRoot> {
        let mut roots = self
            .roots
            .iter()
            .map(|(_, root)| root)
            .filter(|root| root.state == state && root.statement_ordinal == statement_ordinal);
        let root = roots.next()?;
        roots.next().is_none().then_some(root)
    }
}

#[macro_export]
macro_rules! record_or_structural_case_pattern {
    ($($fields:tt)*) => {
        $crate::CheckedStructuralValueKind::Record { $($fields)* }
            | $crate::CheckedStructuralValueKind::StructuralCase { $($fields)* }
    };
}
