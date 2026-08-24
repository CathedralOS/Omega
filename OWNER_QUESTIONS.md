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

Last pruned: 2026-08-23.

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

## Q4 — How does a native layout declare a private callback demand?

Registered-callback lowering already maps one nominal static-machine binder to
one native parameter or nested layout place. A nested destination is valid only
when the independently validated native layout declares a typed private slot
for that exact callback requirement. That slot is absent from the semantic
schema and its layout, slot, and requirement identities are compiler-issued,
but no source/library input currently creates the demand.

Choose the target-package declaration and layout-policy input for one private
callback demand. It must:

- declare the native slot independently of the registrar's materialization row,
  so the supply cannot authorize its own destination;
- name one exact signature-free callback requirement and reject overload
  ambiguity without authored numeric identities;
- keep the slot absent from source projection, read, write, serialization, and
  runtime value topology while retaining it in normalized layout identity;
- allow the compiler to derive `LayoutPlanId`, `LayoutSlotId`, and
  `CallbackRequirementId` from resolved declarations and the validated plan;
  and
- support exact missing, duplicate, overlap, wrong-requirement, wrong-layout,
  and replay-drift rejection when the calling plan closes the demand.

Recommended direction: extend the ordinary `Schema`/`Plan` library vocabulary
with a bounded compiler-private slot source whose authored inputs are a stable
native slot declaration and exact callback-requirement path. The compiler
resolves those names into opaque identities during layout evaluation. Do not
put raw IDs or field offsets in source, infer the demand from the callback row,
or expose a callback-address-shaped semantic field.

## Q5 — What compiler build identity authorizes package evidence?

Context: package evidence must be inseparable from the compiler that derived
it. Source and Terminal Psi already expose useful commitments, but the compiler
currently has no exact self-identity suitable for an accepted lock baseline.

Problem statement: a version string, source revision, or caller-supplied digest
can identify the wrong executable or allow one compiler build to impersonate
another. The authoritative identity must work for library-driven compilation as
well as the `omega` executable and must not conflate compiler implementation
identity with the separately reported Terminal trust graph.

Proposed solution: make the digest of the exact distributed compiler artifact
the authoritative compiler-build identity. Record readable release/source
metadata only as diagnostics, and retain Terminal semantics, verifier, codec,
fuel schedule, and backend commitments as separate evidence axes.

Alternates: a reproducible-build attestation may later authorize a set of
byte-distinct artifacts, but should supplement rather than replace the exact
artifact digest. A version string, Git commit alone, or a digest supplied by the
invoking build is tempting but not an authority boundary.

## Q6 — Must accepted package evidence require complete Terminal coverage?

Context: the compiler can now form an in-memory package review projection from
checked trees, while final Terminal Psi commitments and verifier replay do not
yet cover every exported boundary/build callable.

Problem statement: persisting that projection early would turn an incomplete
compiler-stage summary into accepted evidence. Waiting for complete Terminal
coverage delays package admission, but avoids creating a second, weaker trust
meaning that later lock formats must preserve forever.

Proposed solution: require exact Terminal Psi and proof/verifier commitments for
every exported boundary callable and selected build callable before issuing or
persisting a `PackageInstance`. Until then, keep the compiler projection
explicitly review-only and non-persistable.

Alternates: allowing partial evidence with a completeness bit is useful for
diagnostics, but wrong for admission because users and tooling will eventually
treat the admitted state as sufficient. Exempting build callables is also
unsafe: root build orchestration may not run dependency build machines, but the
package still publishes that code and its claimed authority surface.
