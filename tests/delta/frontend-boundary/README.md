# Delta frontend-boundary gate

Run `sh tests/delta/frontend-boundary/run.sh` from the repository root. The gate
materializes the complete canonical Gamma-authored Delta compiler through its
role manifest and runs it with the selected Beta-authored Gamma evaluator.
Python frames requests, invokes those source-owned stages, and compares exact
observations. It neither parses Delta nor selects diagnostic reasons or source
coordinates.

The 37 exact 40-byte DCOUT controls cover the implemented frontend phases:

- Source-byte rejection uses code 3 and Delta-source coordinate space 1. Invalid
  bytes, including bytes inside comments and a Unicode BOM, precede syntax and
  global collection. The first invalid byte wins even when a duplicate appears
  earlier in source.
- Global collection rejects later duplicate types, constructors, and functions
  with codes 6, 7, and 8 at their exact declaration names. Unknown constructor
  and signature types and body defects do not preempt this earlier phase.
  Duplicate `main` is code 8, not a selected-profile schema failure.
- After frontend acceptance, missing `main` is code 19 in coordinate space 0
  at coordinate zero; `mai` and `main_suffix` do not supply that exact entry.
  A present but incompatible `main` is code 20 in
  Delta-source space at its declaration name, including after earlier
  declarations and comments between `def` and `main`.

Eight syntax, type, and body controls retain status 249 with empty stdout from
the implementing evaluator. In particular, an invalid frontend without `main`
does not become code 19, and an invalid body under an incompatible present
`main` does not become code 20. These unfinished paths are not canonical DCOUT
rejections or generated Delta application observations.

Six accepted programs exercise identity compilation, exact entry selection
after `main_suffix`, cross-namespace spelling reuse, forward and mutual data
visibility, forward and mutual function visibility, and the admitted ASCII
whitespace/comment boundaries. Each compiles
twice to identical bytes; its generated application preserves an exact binary
input including NUL and high bytes.

The expected coordinates are literal authored fixture facts. Whole-frame
comparison checks the reason, halt/tag agreement, coordinate space, reserved
zeros, little-endian coordinate, and zeroed unused resource fields. Compiler
source identity is pinned before any observation. The gate implements the
bounded checks above, not complete Delta frontend diagnostics, resource
conformance, or closure of the Delta bootstrap edge.

The phase order is fixed by [D20](../../../wiki/architecture/bootstrap_chain/decisions.md#d20--delta-names-resolve-through-four-namespaces-without-active-shadowing)
and [D33](../../../wiki/architecture/bootstrap_chain/decisions.md#d33--dcout-admission-and-schema-diagnosis-are-bounded-and-total).
See also the [Delta language](../../../bootstrap/delta/LANGUAGE.md) and the
adjacent [request-boundary gate](../request-boundary/README.md).
