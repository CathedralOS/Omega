//! Optimizer module role: executable entrance. Proven field-value specialization boundary.
//!
//! One bounded specialization family: a `BooleanStructuralField` or
//! `IntegerStructuralField` observation whose `source` place carries a stored
//! field value the unit itself proves. Five proofs qualify. Record
//! establishment: the place is declared `StructuralPlaceKind::OperationResult`
//! and its producer node in the same function is an `EstablishRecord` — the
//! operation-result place is assigned exactly once by its producer and no
//! operation can rewrite it, so the record field initializer fixes the field's
//! stored value permanently. Variant establishment: at a lone `Case` path the
//! producer is an `EstablishScalarCase` whose `result_case` matches the
//! observed case, so its scalar case-field initializer fixes the payload
//! field's value the same way. Nested establishment: a `Field` path segment
//! descends the same two proofs into the child place the record stores whole
//! — the field's initializer must be an owned, complete structural argument
//! whose declared type is exactly the field's declared carrier, and the
//! child's own producer then establishes the deeper position, so
//! `Record { inner: child }` proves `source.inner` reads from the child's
//! establishment, and a scalar-case child proves `source.inner.Case` reads.
//! Bound establishment: a `StructuralPlaceKind::BlockParameter` place holds
//! no producer of its own, but when every incoming edge binds it to the
//! same place through a whole `Owned`/`SharedBorrow` argument and nothing
//! in the function rewrites or mutably re-lends it, the parameter's
//! contents are exactly the bound place's — the bound place's own
//! establishment proves the parameter's observations, transitively through
//! further parameters. Declared bound: the observed field's declared type
//! is a `BoundedInteger` whose inclusive bound closes over exactly one
//! value, so every inhabitant of the position holds that value independently
//! of how the place arrived —
//! parameters, block parameters, results, and non-establishing producers all
//! qualify, at any resolvable path depth.
//!
//! What the proven initializer admits depends on its shape. A
//! `BooleanConstant`/`IntegerConstant` in the same function — or a bound
//! singleton — is a `Constant` resolution: the observation folds in place,
//! the result keeps its value identity, and the node keeps its
//! `PsiProvenance::Operation` custody, fuel settlement, successors,
//! definitions, uses, and ownership events. A nonconstant scalar initializer
//! is a `Forward` resolution when the substitution can retire the read
//! exactly: the initializer must be defined in the same function at the
//! read's own scalar type, dominate every use site, and every scalar-operand
//! use of the read's result must sit in a position the substitution lane
//! rewrites — establishments, scalar stores, ordinary calls, arithmetic,
//! comparisons, shifts, jump and conditional bindings, returns, and atomic
//! operands — while the observation node itself must be cleanly removable.
//! Forwarding then rebinds those uses, retires the observation node, and
//! fuses the read's provenance and fuel settlement into the node inheriting
//! the vacated index; the function's derived metadata restamps from
//! operation shape.
//!
//! Reads whose field the unit cannot prove — an unestablished field on a
//! multi-valued bound, a `Field` descent into a borrowed, pathed, or
//! unestablished child, a `Case` path on a place established under a
//! different case, a block parameter whose incoming bindings diverge, bind
//! a pathed place, lend write authority, or leave it rewritable, an
//! `Erased` or structural field position, or a path that fails to resolve —
//! are not covered, and neither is a nonconstant initializer whose uses
//! escape the substitution lane or which fails to dominate a use.
//! Only machines absent from the authenticated Terminal-cycle component
//! roster are eligible: a machine containing a verified cyclic component is
//! frozen byte-exact under `validate_frozen_component_blocks`.
//!
//! This family owns admission and the plan only. `propose::plan` derives one
//! place's complete `FieldValueSpecializationRewrite` — every proven
//! observation with its site, custody identity, path, field, proof basis, and
//! resolution — and `accounting::plan_accounting` derives the block region
//! and node custody that plan carries. `rules::field_value_specialization`
//! publishes the plan as a `PsiRewriteCandidate`, and
//! `optimization_unit_semantics::validate_field_value_specialization_candidate`
//! re-admits every row, rebuilds the output, and reconstructs the custody
//! independently, so a forged or stale row fails by recomputation rather
//! than trust. There is no second proposal or application route.

use optimization_unit::{
    FieldValueResolution, FieldValueRow, FieldValueSpecializationRewrite, FoldedFieldValue,
    NodeLocation, ProvenanceDisposition, ProvenanceRewrite, PsiOptimizationFunction,
    PsiOptimizationUnit, PsiRealizationSite,
};
use semantic_vocabulary::{OperationId, PlaceId, ScalarType, StructuralPlaceKind};

use abstract_operations::AbstractOperation as O;

pub(crate) mod accounting;
mod admission;
pub(crate) mod propose;

#[cfg(test)]
mod tests;
