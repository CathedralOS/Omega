# Owner Questions

Only the owner answers, closes, removes or rewrites an entry here, including
one that an agent believes the specification already answers. Agents add
questions and may research existing ones, but they report their findings and
citations to the user and leave the entry, the specification and the boards
unchanged; they never edit the specification or a task to settle a question.

Only unresolved owner-level language or architecture decisions belong here.
Settled decisions live in the specification and language guide; implementation
and deliberately deferred research live in `TASKS.md`. Questions are numbered
consecutively; pruning or adding one requires updating every repository
reference in the same change.

Question numbers are mutable queue positions, not permanent decision identities.
Code, canaries, and settled documentation must cite a stable named decision or
the governing guide section rather than an owner-question number. A settled
decision's durable identity does not change when this queue is pruned.

Psi representation and encoding design is delegated; human review is deferred
under [the current policy](omega-rust/pipeline.md#psi-implementation-and-deferred-human-audit).
Missing IR operations or a schema change needed for accepted behavior are
implementation work, not owner questions.

Before a proposed surface becomes an owner question, audit whether it is
implemented, whether any authored source uses it, and whether ordinary Omega
already expresses the customer. An unimplemented, unused spelling that adds no
capability beyond existing checked machines is retired rather than redesigned.
Hypothetical future utility does not by itself preserve syntax; a concrete
customer requiring a distinct capability may propose a new surface later.

Every `OWNER-BLOCKED` escalation must name an independently motivated product
requirement or credible external use case. Existing corpus use is not required.
A test, experiment, benchmark, or implementation task cannot be the sole
motivation, and machinery introduced only to support such work is removed or
kept non-authoritative rather than promoted into an owner decision.

Application ports, including Squalr, are implementation customers, not a separate
class of owner questions. Port shape, API adaptation, missing library operations,
and compiler rejections stay with the implementer under the existing contracts.
Escalate only a specific unresolved language, architecture, or trust rule, citing
the contracts that leave it open. The application may motivate that question;
its name or a workaround does not establish one. Do not ask the owner to approve
ordinary port choices or treat a workaround as permission to reduce acceptance.

Apply the same test to security machinery. Omega owns only claims it can
enforce at its actual compiler, package, and artifact boundaries. A proposal
that merely restates host operating-system, credential, transport, or operator
trust must be deleted or delegated to that owner rather than dressed as an
Omega guarantee. If the boundary or enforceable claim is genuinely ambiguous,
promote that narrow ambiguity here before adding machinery.

Bootstrap design exploration is delegated engineering work, not an owner
question merely because it compares a different implementation or language
facility. Apply [whole-chain minimization](bootstrap/MINIMIZATION.md): identify
the next compiler/checker customer and compare complete audit cost. Experiments
remain non-authoritative; changes to the trust boundary or required assurances
must be surfaced before relying on them.

Name the contracts checked and found silent. A question that cannot cite them
has not shown its decision is open. Specifications are organized by mechanism
while questions arrive by symptom, so the answering clause is routinely in a
document whose subject is not the question's: a containment rule sits with
activation inputs rather than with the vocabulary it confines, and an
instantiation rule sits with contract import rather than with the value class
it instantiates. A reviewer verifies those citations before the framing.

## Open questions

Proposed solutions below are recommendations, not owner rulings. The owner
removes an entry once it is answered; unfinished implementation stays on its
owning task.

### Q1 - Is a domain-qualified fixed byte array a bounded carrier?

Named decision: `qualified-fixed-byte-array-carrier`.

**Customer:** bounded text in fixed storage. Most of the 71
`tests/omega/pass/text` fixtures and Dungeon Crawler's byte snapshots build,
concatenate and measure text inside `[u8; N] in Utf8`-shaped values, for example
`pass/text/runtime_bounded_carrier_length_exit` expects `.len` of a
`[u8; 32] in Utf8` built by `"ALERT " + label` to be 10. The probes
`samples/cli/probes/write_only_domain_store` and
`samples/cli/probes/ascii_indexed_write` copy such a value into a plain
`[u8; 8]` and cannot be realized until this is answered.

**Existing rules, which point both ways:**

- Fixed reading. [Domains](wiki/spec/language/domains.md): "A domain attaches
  a static theory to a carrier without adding a runtime tag, wrapper, hidden
  storage, or second object model", "Representation may change only through a
  unique exact compiler-derived carrier transformation", and "Predicate-only
  atoms may disappear implicitly". [Counts and addresses](wiki/spec/language/counts_and_addresses.md):
  "A fixed array `[T; N]` has length exactly `N`". The guide's chapter 8 lists
  `[u8; 64] in Utf8 // fixed-length encoded byte array`.
- Bounded reading. Domains, encoded text: "Declarations over different bounded
  carriers may share that name ... Bounded-carrier construction proves the live
  byte length fits its capacity ... This does not change a raw fixed array's
  exact-length construction rule", which contrasts qualified carriers with raw
  fixed arrays. [Bounded growth](wiki/spec/resources/bounded_growth.md#carrier-invariants)
  defines "an owned variable-fill bounded carrier" with exact capacity and live
  length, and Terminal Psi's structural access defines `BoundedOwned` store,
  concatenation and length.
- Silent: no contract names the source type that spells an owned
  variable-fill bounded carrier. The Rust compiler gives `[u8; N] in D` the
  `BoundedOwned` carrier (`value_custody/reference_shells.rs`), which is
  implementation, not a ruling.

**Decision needed:** does `[u8; N] in <predicate domain>` denote

1. **the owned variable-fill bounded byte carrier** (capacity `N`, live length
   `L <= N`, `.len` is `L`). Then dropping the domain to a raw `[u8; N]` is not a
   predicate-only weakening: it needs `L == N` proved or an explicit operation
   naming what happens to the unused capacity, and the opening sentence of
   domains.md needs an exception for capacity-bearing carriers; or
2. **a raw fixed array of exactly `N` bytes with a static predicate**. Then a
   different source spelling must carry bounded text, and the text corpus and
   Dungeon Crawler migrate to it.

Recommendation: option 1, since it matches every existing text program and the
bounded-carrier paragraph, with the weakening rule stated for capacity-bearing
carriers.
