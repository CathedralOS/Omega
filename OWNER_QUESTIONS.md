# Owner Questions

Only unresolved owner-level language or architecture decisions belong here.
Settled decisions live in the language guide and design briefs; implementation
and deliberately deferred research live in `TASKS.md`. Questions are numbered
consecutively; pruning or adding one requires updating every repository
reference in the same change.

Question numbers are mutable queue positions, not permanent decision identities.
Code, canaries, and settled documentation must cite a stable named decision or
the governing guide section rather than an owner-question number. A settled
decision's durable identity does not change when this queue is pruned.

Last pruned: 2026-08-22.

## Q1 — How does a target package declare a nominal foreign endpoint?

The foreign-binding model requires source to cite one namespace-owned
`DllImportId`, for example `Windows::Kernel32::WriteFile`, while raw library and
export bytes live only in sealed, fingerprinted target/link metadata. The
repository does not yet define the declaration that creates that nominal value
or the authored target input that maps it to those raw bytes. An ordinary
`const` cannot construct the opaque ID without reopening free pairing, and
deriving the ID from either strings or the realization machine would contradict
the settled identity rule.

Choose the target-package declaration and metadata-supply surface. It must:

- create one resolved nominal symbol usable as a `DllImportId` expression;
- bind that symbol inseparably to one library/export pair in sealed target/link
  metadata, with no raw strings in ordinary Omega source;
- make ownership, target applicability, duplicate/missing mapping rejection,
  fingerprinting, and package visibility explicit; and
- generalize coherently to `CallingPlanId`, firmware/table IDs, and other
  mechanism-specific nominal values without inventing a string-backed escape.

Recommended direction: a target-package-owned nominal-ID declaration plus a
separate sealed target metadata record keyed by that resolved declaration. Keep
`build.omg` limited to selecting target/provider declarations; it must neither
author linker spellings nor manufacture IDs.

## Q2 — How does a named guarantee declare its result-case guard?

Named `ensures proof: P` outputs and selective proof-output bindings are live
for unconditional guarantees. The settled caller surface also allows a proof
selector inside the matching result arm, but no declaration syntax currently
states that a named guarantee exists only for one result case. The checker must
not infer that association from `P`, visible case facts, or the producer's body.

Choose the source form and normalized identity for a result-case-guarded named
guarantee. It must:

- name one exact case of the machine's declared result sum, with no ambient or
  body-shape inference;
- remain public signature content, so moving a selector between cases is a
  breaking proof-interface change;
- require definite assignment exactly once on ordinary exits producing that
  case, and no assignment on other or crash-only exits;
- make the selector available only in that case's caller arm, after the `;`
  universe separator, while an omitted selector still contributes its fact
  only in that arm; and
- retain the normalized case identity through checked facts, Terminal Psi,
  codec identity, and independent verifier replay.

Recommended direction: extend the existing named `ensures` clause with an
explicit result-case selector, for example `ensures Success => proof: P`, and
normalize `Success` to the exact result-type case symbol. Keep unconditional
`ensures proof: P` unchanged. Do not admit an arbitrary Boolean guard here: the
customer is outcome-specific availability, and general guarded contracts would
introduce a larger proof and compatibility surface.

## Q3 — What source authority expresses variadic `Respects` evidence?

Quotient operations are selected explicitly as
`Quotient::lift<F, Respect>(...)` or `Quotient::define<F, Respect>(...)`. The
compiler already derives the representative operation's ordered runtime
telescope, the pointwise input relation `RA`, and result relation `RR`. What is
missing is a source/core declaration that lets the explicitly named `Respect`
conformance certify `Respects<F, RA, RR>` when `RA` has one entry per runtime
operand.

Choose the declaration and application model for this compiler-derived,
variadic proof interface. It must:

- retain one exact named conformance selected by the quotient owner, with no
  structural proof-machine discovery or visible-unique inference;
- derive operand positions from the normalized representative telescope,
  including attached `self` at position zero, rather than use an arity-specific
  `Respects1`/`Respects2` family;
- make the complete `F`/`RA`/`RR` application and proof rows available to
  checked and Terminal verification without exposing a runtime dictionary;
- support generic representative applications only after their static
  telescope is closed; and
- remain a reusable proof-interface mechanism rather than privileged syntax
  attached only to `Quotient::lift`.

Recommended direction: add a sealed proof-interface binder for a normalized
relation telescope, allowing core to declare one variadic `Respects` trait
whose applications the compiler constructs but whose named conformances remain
ordinary source declarations. Do not encode the telescope as an untyped list,
generate arity-indexed traits, or let the lift operation discover a proof by
shape.
