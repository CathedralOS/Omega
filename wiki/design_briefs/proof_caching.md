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
