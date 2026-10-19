# placed_stable_write — discrimination measurement

Producer: /tmp/tgt-slot1/debug/omega (dev, opt-level 1), revision fa145e0a (== origin/main).
Host: macos_arm64. Library root: <worktree>/source/library.

## Half (i) — the write is OBSERVABLE through the placed home

Probe A: green `local` place, changing ONLY the consumer's demanded value
(`transition value.number == 29` -> `== 30`), leaving the write at 29.

    native signal 5
    interp crash: Trap (host presentation agrees)

The corruption traps BOTH engines, so the oracle reads the stored value back
through the placed home. The transition is not vacuous and the write is not
elided, folded away, or written to a descriptor instead of its referent.

## Half (ii) — violating the construct's own licence refuses

Probe B: fail twin (`write: false`), `omega run --both`:

    error: placed write application: the placement plan does not permit this operation

Refused at the CONSTRUCTION/placement gate, before any runtime realization.

## Is a RUNTIME trap reachable?

Probe E: permission derived from a runtime-read schema value, not a constant —
`write: schema.fields[0].key != 0u64`:

    native exit 0
    interp exit: 0

The expression is true here, and the gate evaluates it from the plan; it does
not defer the decision to runtime.

Probe G: permission folded to false via `write: 1u64 == 0u64`:

    error: placed write application: the placement plan does not permit this operation

Probe D: seeding the permission from a runtime parameter is not admissible at
all — the plan machine signature rejects the extra argument:

    error: placed view `Placed<StablePlacement,Value>`: policy
    `StablePlacement::plan` needs one unique declared data context in addition
    to its 1 supplied argument(s)

Probe F: a `requires`-bounded main parameter feeding the permission refuses in
the plan state before submission (`permit` is not a declared local of
`StablePlacement::plan`).

## Class

CONSTRUCTION-GATE obligation. The write licence is decided when the placed view
is constructed from its plan; no admissible seeding defers it to runtime, so no
runtime trap exists for a licence violation. This matches the measured
`where`-clause/construction class. It is NOT a value/arithmetic bound class.

The runtime trap that DOES exist belongs to half (i): corruption of the
observed value, which traps both engines (native SIGTRAP, interpreted Trap).

## Oracle strength

The fail side is an accepted-publication refusal, not an execution fact
(`omega run --both` never produces an exit code for it). The twins are
therefore scored by their authored `expected.txt` fragments, which match the
observed diagnostic exactly on all nine places.
