//! Snapshot demands for a fixed array of copyable scalar-payload cases.
//! The checker validates case selection, restoration, and ownership separately.

use std::cell::OnceCell;
use arena::IdSet;

use semantic_vocabulary::{
    CanonicalStructuralPathSegment, PlaceId, Proposition, ScalarTerm, StructuralTypeId,
};
use terminal_psi::{StructuralTypeDeclaration, StructuralTypeShape, TerminalMachine};

use super::Lowering;

impl Lowering<'_> {
    pub(super) fn indexed_case_array(
        &self,
        root: PlaceId,
        types: &[StructuralTypeDeclaration],
        machine: &TerminalMachine,
        exposed: &OnceCell<IdSet<StructuralTypeId>>,
    ) -> bool {
        let Some((_, carrier)) = self.roots.get(root) else {
            return false;
        };
        let Some(array) = terminal_psi::find_structural_type(types, *carrier) else {
            return false;
        };
        let StructuralTypeShape::FixedArray { element, .. } = array.shape else {
            return false;
        };
        let Some(element) = terminal_psi::find_structural_type(types, element) else {
            return false;
        };
        element.is_copyable()
            && matches!(element.shape, StructuralTypeShape::Sum { .. })
            && element
                .shape
                .fields()
                .all(|field| field.field_type.scalar_type().is_some())
            && !exposed
                .get_or_init(|| self.exposed_array_types(machine))
                .contains(*carrier)
    }

    /// A view or whole-value use can expose an index absent from this
    /// machine's place table. Keep all slots of that array type, including
    /// its other block formals. A jump that only carries the array to the
    /// next block preserves the indexing coordinate.
    fn exposed_array_types(
        &self,
        machine: &terminal_psi::TerminalMachine,
    ) -> IdSet<StructuralTypeId> {
        use terminal_psi::{StructuralPathSegment as Path, StructuralStorageUse as Use};

        // A proof-level storage expression can name elements without any
        // operation reading them. Preserve the full inventory when the
        // header or a state invariant observes storage by that route.
        if !self
            .signature
            .observing_clauses()
            .chain(self.invariants.iter().map(|row| &row.predicate))
            .all(scalar_clause)
            || !machine.block_entry_claims.is_empty()
            || !machine.content_identity_reshuffles.is_empty()
            || !machine.content_partition_compositions.is_empty()
            || !machine.content_entry_claims.is_empty()
            || !machine.loan_completions.is_empty()
            || self.signature.has_structural_guarantees()
            || machine.ranked_scc.is_some()
        {
            let mut all = IdSet::new();
            for (_, ty) in self.roots.values() {
                all.insert(*ty);
            }
            return all;
        }
        let mut exposed = IdSet::new();
        let mut expose = |root| {
            if let Some((_, ty)) = self.roots.get(root) {
                exposed.insert(*ty);
            }
        };
        let selected = |path: &[Path]| {
            matches!(
                path.first(),
                Some(Path::FixedIndex(_) | Path::RuntimeIndex { .. })
            )
        };
        for op in machine.operations() {
            // A direct scalar-returning call may borrow the whole array. Its
            // scalar-only contract observes no element. Private field inference
            // never crosses a case payload. The caller still validates every
            // write and handback and names its own later observations.
            let whole_borrow = op.module_invocation().is_some_and(|call| {
                call.required_contract.is_none()
                    && call.erased_arguments.is_empty()
                    && call.erased_proof_arguments.is_empty()
                    && call.claim_transfers.is_empty()
                    && call.completion_receipts.is_empty()
                    && call.crash_continuations.is_empty()
                    && self
                        .signatures
                        .machines
                        .get(call.callee)
                        .is_some_and(|callee| {
                            callee.result.is_none_or(|ty| ty.scalar().is_some())
                                && !callee.has_structural_guarantees()
                                && callee.observing_clauses().all(scalar_clause)
                        })
            });
            for (operand, _) in op.kind.storage_uses_with_arguments(|arg| {
                !(whole_borrow
                    && arg.path.is_empty()
                    && matches!(arg.access, terminal_psi::reading_borrow_pair_pattern!()))
            }) {
                match operand {
                    Use::Projection(p) if !selected(p.path) => expose(p.root),
                    Use::Canonical { root, path } | Use::ObservedField { root, path, .. }
                        if !matches!(
                            path.first(),
                            Some(CanonicalStructuralPathSegment::FixedIndex(_))
                        ) =>
                    {
                        expose(root)
                    }
                    Use::Place(root) => expose(root),
                    Use::Element { descriptor, .. } => expose(descriptor.root),
                    _ => {}
                }
            }
        }
        for block in &machine.blocks {
            let internal = !block.terminator.successor_edges().is_empty();
            for operand in block
                .terminator
                .storage_uses_with_arguments(|arg| !internal || !arg.path.is_empty())
            {
                if !selected(operand.path) {
                    expose(operand.root);
                }
            }
        }
        exposed
    }
}

fn scalar_clause(clause: &Proposition) -> bool {
    let mut scalar = clause.visit_value_ids(|_| {});
    clause.visit_scalar_terms(|term| {
        scalar &= matches!(
            term,
            ScalarTerm::Value { .. } | ScalarTerm::Integer { .. } | ScalarTerm::Boolean(_)
        );
    });
    scalar
}
