# Owner Questions

Only unresolved language or architecture decisions belong here. Settled
decisions live in the language guide and design briefs; implementation work
lives in `TASKS.md`. When a question is answered, remove it from this file after
recording the ruling in those authoritative homes.

Last pruned: 2026-07-18.

## Machine and linear surfaces

1. **Accepted proof supply spelling.** Decision 20 distinguishes checked
   bodies, requirements, external providers, and accepted declarations in the
   semantic artifact.

   Needed ruling: whether an accepted theorem remains a bodyless `boundary
   machine`, becomes `boundary fact`, or uses another spelling. Whatever the
   surface, trust expenditure must remain explicit and reportable.

2. **Linear terminal-consumer spelling.** `[linear]` and
   create/transfer/consume conservation are settled.

   Needed ruling: whether `move self` plus result contracts are enough for the
   checker to infer which outcomes discharge an obligation, or terminal
   consumers/outcomes require an explicit declaration. Also settle how an
   authorized `detach` visibly transfers a `Join<T>` obligation out of
   structured scope.

## Surface semantics

5. **Aggregate field defaults: support or reject.** Parked repro
   `canaries/pending/arithmetic/array_field_default_silent` (2026-07-05):
   an inline aggregate-literal FIELD DEFAULT (`xs: [i32;3] = [1,2,3]`, and
   presumably `Foo {..}`) is silently DROPPED at emission (reads see ZII),
   and its length/element class are UNVALIDATED (`[i32;2] = [1,2,3,4]`
   compiles). Scalar literal defaults and nested-record defaults both work.

   Needed ruling: (a) SUPPORT aggregate field defaults — emit inline
   aggregate literals + wire `validate_array_literal_elements` into
   data.rs's field loop; or (b) REJECT non-scalar field defaults with a
   clear diagnostic ("aggregate field defaults are not emitted; initialize
   in a machine body"). Per "no silent anything", the current
   silently-dropped-and-unvalidated state is the one indefensible option;
   engineering is ready to build either ruling.

## Resources and components

3. **Resource algebra first customer and proof surface.** Owned splitting and
   merging (`LinBuf<T, n>`), quantitative resources, and attenuation require a
   conservation algebra beyond core multiplicity.

   Needed ruling: choose the first customer and proof surface before promising
   dependent owned-buffer splits or quantitative effect-row members. Whole
   ownership and borrowed views do not wait on this.

4. **Component versioning.** The leading design uses bounded multi-version
   coexistence, per-version activation pools and liveness pins, and import slots
   pinned to normalized machine-contract identities with deterministic
   refinement admission.

   Still needed: outbound-call semantics for old continuations, version
   budgets and eviction, linking mechanics, and the boundary between v1
   coexistence and later continuation migration.

## 2026-07-16: Sealed progress profiles (decision 23 / TPR4's grant half) — DESIGN GAP

The termination brief settles progress-profile SEMANTICS (named opaque
commitments on boundary traits/providers/slots; sealed by default; no
self-grant; inert until granted; receipts; deterministic admission) and the
REFERENCE spelling (`requires scheduler in WeakFair` through the normal
requirement surface). It does NOT settle:

1. **The profile DECLARATION spelling** — how does a boundary trait author
   declare/mint `WeakFair`? (A trait-body item? `profile WeakFair;`? A
   boundary-clause form?) The brief only says profiles are "named, opaque
   semantic commitments on boundary traits, providers, and slots".
2. **The grant/receipt carrier** — the brief leans on "the existing boundary
   grant machinery", but the omega-rs line's grant machinery today is
   BoundaryLevel/BoundaryPolicy clauses + host-call authorization checks;
   there is no package-grant/receipt/trust-report subsystem to ride. Building
   one is a subsystem-scale arc that deserves its own design pass (it also
   gates acceptance tests 8 and 9, and TPR6's export-omission half needs the
   artifact-serialization story).

Blocked pending: the declaration spelling ruling + a scoping decision on
whether the grant/receipt subsystem lands as part of TPR4 or as its own
front-loaded arc. Everything else in TPR1–TPR5 is landed; TPR6's
firewall/acceptance work continues where it doesn't depend on profiles.

## 2026-07-16: N2(d) arithmetic bridge (n > 0 => n == Succ(n - 1)) — DESIGN GAP

The N-ladder's remaining N2 item wants INTEGER-measured induction to consume
Nat lemmas: from an integer hypothesis `n > 0`, expose the structural reading
`n == Succ(n - 1)` so integer-typed subjects can meet structural machinery
(and dually, Nat lemma ensures could extract to polynomial facts).

What is NOT settled — the bridge's sanctioned surface:

1. **The homomorphism direction and spelling.** Does a Nat lemma's ensures
   EXTRACT to integer facts (add(a,b) => a + b under a Nat->integer measure
   map, with which trust story?), or do integer facts REFLECT into Nat
   structure (an integer-typed variable gaining constructor readings under
   range hypotheses)? Both are sound in principle; they lead to different
   engine plumbing (polynomial engine consuming structural ensures vs the
   structural judge consuming range facts), different fence surfaces, and
   different soundness audits.
2. **Which types participate.** u64-only? All unsigned? Signed with what
   floor discipline? (The Int introduction rule says order has no floor for
   IntPair — measures stay Nat-valued — so the bridge presumably speaks
   unsigned integers only, but that is my inference, not a ruling.)
3. **Where the bridge fact lives.** A judge rule (structural side), an
   entailment tier (polynomial side), or an explicit citable core lemma with
   a kernel-recognized discharge?

The quotient/conformance arc (IntPair rungs 1-6, landed 2026-07-16) did not
need the bridge; nothing else in the current queue is blocked on it. Parking
until the direction is ruled; both engines are healthy and the bridge can
land as a bounded slice once the surface is chosen.

## 2026-07-16: Permission plans (decision 19's authority half) — SHARES THE #81 GRANT-CARRIER GAP

The mint's three v1 discharge routes are live (literals, declared ranges,
guard-carried requires). The remaining decision-19 half is INTRODUCTION
AUTHORITY: sealed-by-default qualification, `introduction open;` opt-ins,
and `MintAuthority<D>` as an exported, attenuable, contract-visible
capability. All of it bites only at PACKAGE boundaries — in-program, the
owning package qualifies its own domains, which v1 grants uniformly.

The blocker is the same one recorded for #81 (sealed progress profiles):
there is no package-grant/receipt/trust-report subsystem to carry an
exported authority. Building it is a subsystem-scale arc that should be
designed once for BOTH consumers (progress-profile grants and mint
authorities — likely the same carrier). Parking the permission-plan work
behind that design; the checked-plans arc continues with the
normalized-machine-contract plans, which have no such dependency.


## Proof-machine measured termination through `sub` (the mod/gcd gate; task #134, 2026-07-20)

The Rat ladder's next prerequisites (`mod`, `gcd` -- Euclid) recurse on
`sub(a, b)`, which is NOT a case-payload subterm: `terminates by a` needs the
checker to accept a MEASURED descent (`sub(a, b) < a` given `b >= 1 && a >= b`).
Today validate_proof_machine_recursion accepts structural descent only, and
the question co-mingles with the PARKED N2(d) Nat<->integer bridge:

1. Where does the decrease obligation live -- a `terminates by a` whose
   witness cites a proven lemma (`sub_lt`), or the runtime MR family's
   measured-recursion machinery extended to proof machines?
2. What is the order surface? `sub(a, b) == Zero` spells `a <= b` today, but
   a STRICT `<` needs either a Nat-valued Bool-like or the N2(d) extraction
   into integer facts (parked).
3. If lemma-cited decreases are the answer, the checker gains a
   proof-obligation face (cite-at-call-site discharging the ranking) --
   design wanted before building.

Until ruled: mod/gcd stay unimplemented; the monus lemma zoo (landed) is
everything Rat's normalization needs EXCEPT the gcd computation itself.
