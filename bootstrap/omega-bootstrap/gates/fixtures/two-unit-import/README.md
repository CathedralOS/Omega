# Two-unit direct-import fixture

This fixture is the first call-free CKIR1 package/module join. Package `dep`
publishes `model::Pair`; package `root` has the requester-local alias `dep` and
imports exactly `dep::model::Pair`. The selected `Probe::run` stores and returns
the scalar `70` through the imported nominal record without a machine call.

Run `two_unit_compilation_fixture.py build OUT` to produce:

- `source.bundle`, the canonical two-entry custody bundle;
- `compilation-envelope.bin` and its SHA-256 receipt;
- `compilation-envelope.json`, the decoded structural expectation;
- `reference.bundle`, the one-unit resolved/module-erased CKIR reference input;
- `expected-resolution.json`, the semantic-order and selected-root expectation;
  and
- `expected-observation.txt`, containing `70`.

The reference bundle deliberately orders `Pair`, `Probe`, and `Probe::run` as
the compilation contract orders them after resolution: package/source order,
then authored declaration order. The existing one-unit custody producer may
compile it to expected CKIR1. A two-unit producer must independently parse and
resolve the envelope sources and emit byte-identical CKIR1. Passing only the
structural envelope verifier is insufficient.

`two_unit_compilation_fixture.py check-pair EXPECTED.ckir ACTUAL.ckir` validates
both modules, pins the imported nominal record order and shape, recomputes result
70, and requires exact canonical CKIR bytes. `EXPECTED.ckir` is the existing
one-unit producer's output for generated `reference.bundle`; `ACTUAL.ckir` is
the future two-unit producer's output for `compilation-envelope.bin`. The tool
also freezes the source-bundle, envelope, reference-bundle, and expected-CKIR
SHA-256 digests; its receipt is therefore a drift check rather than a digest
accepted merely because it accompanied untrusted envelope bytes.

## Required negative inventory

Each case must reject before CKIR or artifact publication:

1. **missing-direct-alias** — remove the root `dep` alias while retaining the
   exact `use dep::model::Pair` source.
2. **transitive-only-reach** — add a middle package with `root -> middle -> dep`,
   define `dep` only for the middle requester, and retain the root import.
3. **private-import** — remove `pub` from dependency `Pair`.
4. **module-mismatch** — retain envelope module `model` but author a different
   dependency `module` item (and the converse mutation on root `app`).
5. **alias-module-ambiguity** — add a same-package top-level module named `dep`
   while the requester-local dependency alias `dep` remains present.
6. **duplicate-identity** — contribute a second `Pair` declaration to dependency
   module `model`; source order must not select a winner.
7. **wrong-selected-root** — select a source other than `root/main.omg`, or name
   an owner/machine not uniquely authored in that selected source and module.

Envelope-only negatives should additionally distinguish structural package/
source ownership failure from the semantic selected-source/entry failure.

The lower-rooted checker decomposition and normalized-resolution carrier for
this fixture are specified in
[`OMGCOMP_REFINEMENT_WITNESS.md`](../../../../assurance/refinement/omega-bootstrap/OMGCOMP_REFINEMENT_WITNESS.md).
