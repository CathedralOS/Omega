# Owner Questions

Only unresolved owner-level language or architecture decisions belong here.
Settled decisions live in the language guide and design briefs; implementation
and deliberately deferred research live in `TASKS.md`. Questions are numbered
consecutively; pruning or adding one requires updating every repository
reference in the same change.

Last pruned: 2026-08-08.

## Q1 — Fixed-operator surface-binding syntax

Named `operator` declarations are the semantic identities behind fixed surface
tokens such as `+`, `[]`, and `[..]`. The language guide previously wrote this
association with a `spelling` clause, but that keyword and clause shape were
never approved and are not part of the settled language.

Choose the source form that binds a fixed operator token to a named declaration.
The decision must settle where the binding appears relative to the signature and
contract, how punctuation-shaped tokens such as `[]` and `[..]` are named, and
whether one declaration may bind more than one fixed token. It must preserve the
settled semantics: the named path remains canonical, resolution is static and
operand-directed, the public signature and proof contract remain visible, and a
`boundary operator` differs only in how its implementation is supplied.

## Q2 — UEFI physical handoff versus semantic program-storage entry

`ProgramStorageEntry::enter` canonically introduces image and initial-storage
roots, and the UEFI target slot currently requires its selected source machine
to expose exactly those two qualified `Extent` parameters. The Cathedral boot
contract separately records that firmware actually invokes the PE entry with
`ImageHandle` and `SystemTable`, that the semantic roots are not additional
firmware arguments, and that a generated bridge must preserve that real
invocation while installing the roots. The current target entry-shape carrier
cannot state both surfaces.

Choose how a target entry schema composes platform-private physical inputs with
portable semantic arrival requirements. In particular, decide whether the
selected source continuation receives both sets of values, receives only the
semantic roots while platform inputs install separately selected providers, or
binds a second target-owned slot for the platform handoff. The decision must
keep `ProgramStorageEntry::enter` as the sole root-introduction requirement,
avoid treating firmware handles as `Extent` values, and leave the generated
bridge with one exact auditable physical ABI and source-visible shape.

## Q3 — Sealed local-capacity declaration form

Compiler provisioning may originate a program-local content root only from an
owner-authored sealed declaration with declared capacity. The semantic model
settles that this is a compile-time root origin, not a runtime establishment or
provider issuance, but no approved source declaration form identifies the
owner, capacity algebra/value, qualification, and authorized establishment
route.

Choose that declaration form and where it may appear. The decision must keep
the declaration owner-unique and sealed, make its finite capacity explicit,
define whether it provisions one root or a declared family of roots, and bind
the resulting account to an exact qualification and establishment route.
Ordinary construction, proof terms, and firing a checked runtime route must not
be able to reproduce the provisioning evidence.

## Q4 — Write-only memory view

Foreign providers sometimes receive storage they may initialize or overwrite
but must not read. Omega's settled reference surface has only shared read and
exclusive read/write borrows, while placed-field accessors model individual
write-only operations rather than a contiguous memory view. Treating a
write-only foreign parameter as `&mut T` would therefore grant more authority
than the binding declares and may expose preexisting or uninitialized bytes.

Choose the core representation and source form for a write-only memory view.
The decision must settle whether it is a third borrow kind, a nominal core view,
or an operation-capability value; how it projects and subdivides; whether and
when it may cover uninitialized storage; and what evidence turns a complete or
partial provider write into readable initialized content. It must preserve
ordinary lifetime and nonaliasing checks without implying provider read
authority, and it must remain distinct from `Placed<P, T>` field accessors and
from durable custody transfer.
