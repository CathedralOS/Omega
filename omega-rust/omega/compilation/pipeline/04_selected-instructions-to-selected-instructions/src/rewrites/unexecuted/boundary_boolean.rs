//! Boundary-decided predicate materialization on the selected CFG.
//!
//! The constant-flag cluster decides a flag reader when the compare it
//! observes is compile-time constant on both operands
//! (`rewrites/constant_boolean` materializes the decided boolean,
//! `rewrites/constant_branch` jumps the decided edge), and the literal
//! selection folds (the selected-lowering compare pair rules) only change
//! the compare's form. Between those cases a compare whose
//! single known operand sits at a carrier-domain pole still reaches a
//! reader whose predicate the pole decides alone: `x < 0` unsigned and
//! `x < i64::MIN` signed can never hold, `0 <= x` unsigned and
//! `x <= i64::MAX` signed always hold, and `x <= 0` unsigned — like
//! `u64::MAX <= x`, `x <= i64::MIN`, or `i64::MAX <= x` — collapses to
//! equality at the pole. The surviving operand needs no producer at all.
//!
//! This rewrite is that compare/test selection at full selected-CFG
//! standing. A `MaterializeBooleanU64LessThan`,
//! `MaterializeBooleanI64LessThan`, `MaterializeBooleanU64LessOrEqual`, or
//! `MaterializeBooleanI64LessOrEqual` whose every implicit condition-state
//! use resolves to one compare — the last flag event on every path reaching
//! the materialization, through the shared least-fixpoint entry-event walk
//! — is admitted when the compare's operand audit yields a pole that fixes
//! the predicate. The compare is any of the three forms: `CompareI64`
//! resolves each side through its unique `MaterializeI64` producer where
//! one exists, `CompareI64Immediate` carries its encoded literal, and
//! `CompareI64Zero` reads as `left - 0`. A side nothing pins is simply
//! unknown; the pole must decide without it. The strict less-than forms
//! fold at the far pole only — `0 < x` unsigned is `x != 0` and `x < MAX`
//! is `x != MAX`, negations no boolean kind materializes — while the
//! less-or-equal forms at the near pole are exactly the equality condition
//! (`x <= 0` unsigned holds precisely when `x == 0`), so the reader's kind
//! rewrites to `MaterializeBooleanEqual` over the identical published
//! state: the flag-reading kinds share one constraint row, so operands,
//! implicit uses, and clobbers carry over verbatim. A compare reading the
//! same register twice is the identity case the constant family already
//! decides and stays refused here.
//!
//! A decided predicate rewrites the reader to the target's own
//! `MaterializeI64` of the outcome — the same emission the constant fold
//! uses — dropping the flag uses while the compare keeps publishing flag
//! state for every other reader. An equality collapse keeps the reader a
//! flag consumer: only its kind field changes. Either way the instruction
//! keeps its identity, position, result register, and provenance.
//!
//! Validation consumes the proposed program, requires the instruction at
//! the materialization's position to equal the independently computed
//! fold, and restores the complete source by content: every other
//! instruction, register, roster row, call, and settlement is retained
//! bit-identical. The validator re-derives the fold's preconditions on
//! its own audit — the flag-reader shape, the one compare every used
//! unit's reaching event must resolve to, the boundary operand poles the
//! predicate outcome is decided from, and the materialize row a constant
//! fold is built from — never consulting the producer's `admission`
//! routine; only the module's shared condition-state walk and operand
//! audit are common to both sides.

mod admission;
mod rewrite;
mod validation;
