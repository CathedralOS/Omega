# Design Brief: Proof Caching (Theoretical)

Scouted 2026-06-15. Status: THEORETICAL — idea + open questions, no mechanism chosen.

A speed mechanism for tractable exhaustive/heavy proof checking. Recorded as a
direction; the details are deliberately left open.

## The idea

Checking a proof is cheap (de Bruijn — re-run the small checker on the proof
object); *finding* a proof is expensive. So cache the expensive search, keyed by
a content hash, and never re-find an unchanged proof. A Merkle DAG of sub-proofs:
a proof's hash = `hash(statement + dependency-hashes + proof-term)`. Change a
sub-proof → its hash changes → its dependents re-check; everything else is a
cache hit. (Unison's content-addressing + incremental compilation, applied to
proofs.)

Motivation: Rust spends ~70-90% of compile time in LLVM; replacing LLVM with our
own verified backend reclaims that budget — though it *replaces* codegen, it does
not remove it (headroom, not free). Proof caching is the lever that keeps
heavy/brute-force proof affordable in the steady-state edit-compile loop.

## The few things that seem settled

- **Cache the witness, re-check it cheaply — never trust a cached "valid" bit.**
  Store the proof object; re-run the small checker on use. This keeps the cache
  OUTSIDE the TCB: a corrupt or poisoned entry just fails the re-check, caught.
  You cache the expensive *search*, never the *trust*.
- **In the content-addressed store, not side-files.** Omega/Cathedral is already
  content-addressed; a proof is an artifact in the same namespace as the code,
  not a desyncing `target/incremental`-style cache.
- **Hash normalized structure, not surface syntax** (likely). Renaming a variable
  should be a cache *hit* (alpha-invariant — hash a de-Bruijn-normalized
  structure, à la Unison); a structural / spec / checker-version change
  invalidates.

## The practical landing (decided direction, 2026-06-26)

Working the *transmissibility* question end-to-end (a published library shipping a proven fact a consumer must verify) lands hard on one rule: **only *small* certificates are practically useful, so proof-by-exhaustion is not a shippable verification.**

- **Small certs (inductive proofs; compact resolution/SMT *while small*) are the only transmissible currency.** They ship in the content-addressed closure (or a small `Proofs.lock` manifest — the storage-form bikeshed dissolves at small scale, both work) and a consumer **re-checks them locally and cheaply** (de Bruijn). This is the *only* configuration where trust-by-checking genuinely holds: you, locally, verified it, trusting nobody.
- **Large certs collapse into attestation-with-extra-steps.** A TB-scale proof (the Pythagorean-triples proof was ~200 TB) cannot be held, transferred, or linearly re-checked inside a build, so you offload to a checking *server* — and then you trust the server's verdict *and* that it bound the cert to your code-hash. Same trust as the prover, more infrastructure. **Second-class: an attestation, never sold as "checked."**
- **Brute-force witnesses are pure attestation** — you trust the witness ran the search; no cert at all.
- **So: no proof-by-exhaustion as a shippable verification.** Exhaustion (and TLC-style enumeration) stays a *local* bug-finder / confidence tool — its result does not transmit as a checkable artifact (Cathedral `testing_and_simulation.md`).
- **The escape hatch is the budget/bound measure** (`totality_and_bounded_computation.md`): re-express a would-be-exhaustion as a **bounded** computation — the bound *is* the decreasing measure, so totality/termination is **cheap and decidable by construction**, a small cert that ships. That is the preferred way to answer "does this halt / stay in bounds" without exhausting. *But cost scales with greed*: modest bounds verify cheaply and ship; crank the bound (or try to exhaust a large domain) and **build times suffer** — and a genuine large-domain *universal* still won't transmit anyway.

This resolves the open **storage-form** question (small certs → in-store content-addressed *or* a small `Proofs.lock`, both fine) and **trust-boundary** question (re-check-locally for small certs; anything bigger is an attestation, explicitly second-class) by **restricting the design to the regime where the cheap-re-check guarantee actually holds.** Contingent on Omega's proof-emitting + de-Bruijn-recheck integration being built.

## The constraint that actually matters

Caching pays off only if proofs are **modular** — local and composable.
Whole-program facts (no-aliasing, authority-flow across the call graph) resist
incremental caching: change one function and a global property may need global
re-checking. So "keep Omega's proof obligations modular/local" is a
**language-design pressure**, and it is the *same* pressure as separate
compilation (modular = cacheable = separately-compilable). See
`separate_compilation.md`.

## Open questions (deliberately unresolved)

- **Storage form:** a `Cargo.lock`-like lockfile? In-store content-addressed
  entries? Or *self-editing source* — the compiler rewrites the source to embed
  proof hashes the way a formatter rewrites layout? Each has a different
  failure/reproducibility story.
- **Granularity:** per-definition? per-proof-obligation? per-module? Finer = more
  reuse, more bookkeeping.
- **Invalidation rules:** exactly what changes the hash — rename (should be free),
  structural change (invalidates), spec change (invalidates), dependency change
  (propagates upward), checker-version bump (invalidates all). Needs a precise
  rule.
- **Structure serialization:** how the hashed normal form is serialized (and
  frozen for cross-version stability — the same concern as the wire/IR format).
- **Trust boundary:** confirm re-check-always stays cheap enough to keep the
  cache fully untrusted, vs. ever trusting a signed cache for speed.

## Cross-references
`verified_gated_ml_optimizer.md` (sibling compiler-speed direction);
`separate_compilation.md` (the modularity that makes caching work);
`cathedral_alignment.md`.
