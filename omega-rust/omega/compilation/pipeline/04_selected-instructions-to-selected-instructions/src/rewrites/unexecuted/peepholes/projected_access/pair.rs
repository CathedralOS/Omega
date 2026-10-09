//! Declarative projected-access pair descriptors: producer, consumer, and
//! the rewritten form plus the admissibility axes each must satisfy.
//!
//! The [`literal_fold`](crate::rewrites::selected_lowering) pair grammar
//! describes a materialized literal folded into a consumer operand, and the
//! decided-branch descriptors in `rewrites::constant_branch::pair` and the
//! condition-materialization descriptors in
//! `rewrites::constant_boolean::pair` describe condition-state readers.
//! Neither vocabulary can name the pair
//! an address projection forms with the access that consumes it: the
//! `AddressOffset` producer's `Def` is an ordinary register — no literal to
//! substitute — and the consumer absorbs the projection's displacement into
//! the `byte_offset` its own kind already carries, so the rewritten form is
//! the consumer's *same* constraint row at a new displacement and pointer
//! register. This module declares that relationship. A
//! [`ProjectedAccessRule`] declares one (producer kind, consumer kind,
//! rewritten kind) triple and the axes the relationship must satisfy; the
//! producer consults its own descriptor when admitting a candidate, while
//! the independent replay re-derives the whole grammar — consumer kind and
//! operand shape, the pointer's last definition, the combined-displacement
//! bound, and the effect surface — from the instruction records alone and
//! never reads a rule.
//!
//! The landed family is the projected-access fold: a `Load8`, `Load16`,
//! `Load32`, `Load64`, or `Store` whose operand-0 pointer is the block's
//! last definition of an `AddressOffset` result becomes the same access
//! reading the projection's base register at the combined displacement —
//! the bespoke [`address_fold`](crate::rewrites::address_fold) transform,
//! restated as a declared pair with the machine-effect binding the bespoke
//! rewrite never consulted. The rewrite is the first descriptor whose
//! consumer and rewritten kind are identical, and it exercises the
//! machine-effect axes the board still names open: the *trap-preserving*
//! relationship [`ProjectedAccessEffects::AccessFaultRetained`] — the
//! dereference's `MayArchitecturalFaultV1` surface rides the fold verbatim
//! because the rewritten form performs the identical access — and, through
//! the `Store` consumer, the first declared pair whose consumer writes
//! memory (`WritePointerV1`). The producer stays: its projected register
//! remains published for every other reader the function still holds, so
//! the fold only changes which register the access reads and which
//! displacement it carries.
