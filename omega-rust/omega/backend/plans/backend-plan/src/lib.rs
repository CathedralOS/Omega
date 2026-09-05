//! One semantically admitted nominal callback use, carried across the Psi/Omega
//! firewall with its evaluated ABI plan attached, plus the derivations three
//! downstream consumers re-check instead of trusting.
//!
//! A compiler-private callback symbol is exactly 101 ASCII bytes, always
//! (spaces below are for reading and are not in the symbol):
//!
//! ```text
//!   __omega_callback_ s 0000001a g 00000001 _a 00000000
//!                     _m 0000000c g 00000001 _e 00000003 g 00000001
//!                     _f 3f1c9a04be775d20
//!   17 + 1 + 8 + 1 + 8 + 2+8 + 2+8+1+8 + 2+8+1+8 + 2+16 = 101
//! ```
//!
//! Every variable slot is a fixed-width hex field, `{:08x}` for a `u32` and
//! `{:016x}` for a `u64`, so no value can widen its slot and no two placements
//! can produce names that differ only in length. The six coordinates it encodes
//! are the use site (kind, arena index, generation), the static machine
//! ordinal, the selected machine and selected entry (each index and
//! generation), and the boundary calling plan's report fingerprint. Just as
//! important is what it does NOT encode: registration operation, satisfaction
//! trait, satisfaction requirement, canonical requirement overload, resource
//! receipt, and private materialization all differ between placements that get
//! the same symbol, and are separated by the fingerprint instead.
//!
//! That fingerprint is FNV-1a 64, seeded with the domain tag
//! `omega.callback-placement-identity.v2` before any payload, so a fingerprint
//! from another domain cannot collide with one from this one by construction.
//! Length-prefix discipline is uniform: every variable-length field is preceded
//! by a little-endian `u64` count, and every optional by a single 0 or 1 byte.
//!
//! One trap, because it looks like an ordering mistake and is not. The
//! `ValueClass` fingerprint tags are Integer 1, Float 2, BorrowedReference 5,
//! HomogeneousFloatAggregate 3, SystemVAggregate 4 - while the enum declares
//! BorrowedReference third, between Float and the aggregates.
//! `BorrowedReference` carries 5 because it was appended after the numbering was
//! frozen. Renumbering the match to follow declaration order changes every v2
//! fingerprint ever computed, silently, with no type error and no test that
//! names a literal.

//! `CallbackPlacementBindingIdentity` is a field-for-field copy of
//! `BoundNominalCallbackPlacement` - all twelve fields, same types, differing
//! only in where `resource_receipt` and `boundary_entry_plan` sit in the
//! declaration - and `callback_placement_binding_identity` is an identity
//! function up to the type name. Storing the placement itself, or a borrow of
//! it, is the obvious saving. It is rejected because the snapshot exists to be
//! compared against the live row: with one type,
//! `schedule.placement_identity != callback_placement_binding_identity(placement)`
//! collapses into `schedule.placement != *placement`, and a drift check becomes
//! a value compared with itself. The duplicated type is what keeps the two
//! roles distinguishable to the compiler.
//!
//! Collections that must hold exactly one element are kept as `Vec` and checked
//! by full linear scan rather than being modelled as a single field or found
//! through a map. Making the duplicate case unrepresentable sounds strictly
//! better and is not: a placement carrying two rows for one binder would then be
//! accepted by taking the first. These scans exist to prove uniqueness, not to
//! find a match, so "zero" and "two" both have to stay representable long enough
//! to be rejected.
//!
//! `plan_callback_root_schedule` validates the placement to build a schedule and
//! then runs its own freshly built output straight back through the public
//! `replay_callback_root_schedule`, validating the same placement a second time
//! and the boundary plan a third. Building and trusting would make the
//! constructor the only witness that its own output is canonical, and a later
//! change to it could not be caught by the replay path external consumers use.
//!
//! It also takes `entry_key`, `function_identity` and `private_symbol` as
//! parameters when all three are derivable inside this crate from the placement
//! and its index, and replay re-derives all three and rejects a mismatch. The
//! effect is that a schedule asserts the caller's separately stored identity
//! agrees with the canonical derivation, rather than restating the derivation
//! unconditionally. No comment says so - that reading is inferred from what
//! replay checks, and it is the one design claim in this header that the code
//! does not confirm on its own.

//! Consumed by `compilation-report` (which retains the placements
//! alongside its Terminal product) and by the machine-emission lane.
//!
//! @Note: `tests/architecture/layering.rs` reads this crate's source as TEXT and
//! asserts on literal strings, so several refactors here break a test that does
//! not break the build. It requires
//! `pub boundary_calling_plan_report_fingerprint: u64` to be present and
//! `pub boundary_calling_plan_fingerprint: u64` to be absent, which is what
//! keeps the compact coordinate from drifting back into a name that reads like
//! authority. More surprisingly it requires the literal line
//! `boundary_entry_plan: placement.boundary_entry_plan.clone()` - the
//! architecture test demands by name the clone that makes the binding identity a
//! real copy rather than a borrow, so rewriting that expression, even into
//! something equivalent, fails the gate.

mod callback_placements;
mod callback_root_schedule;
pub use callback_placements::{
    BoundCallbackPrivateMaterialization, BoundNominalCallbackPlacement,
    CallbackPlacementBindingIdentity, CallbackThunkPlan, callback_placement_binding_identity,
    callback_thunk_placement_identity_report_fingerprint, canonical_callback_private_symbol,
    canonical_callback_thunk_identity, validate_bound_nominal_callback_placement,
};
pub use callback_root_schedule::{
    CallbackRootActivationIdentity, CallbackRootSchedule, plan_callback_root_schedule,
    replay_callback_root_schedule,
};
