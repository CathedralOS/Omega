# Epsilon feature ledger

Epsilon is justified by the smallest robust surface needed to author the
Epsilon-written full Omega compiler source closure `D` and execute it with the
Delta-written Epsilon evaluator.

[`LANGUAGE.md`](LANGUAGE.md) is the normative Epsilon v1 contract fixed by D17.
This ledger is rationale and change control, not a second specification or a
vote by historical samples. A future facility belongs to Epsilon only when at
least one of these holds and the normative contract is revised explicitly:

1. the Epsilon-written Omega compiler closure `D` requires it;
2. implementing or compiling `D` coherently requires it;
3. it materially reduces total implementation or assurance cost without
   weakening deterministic failure, resource, or safety behavior.

## Retained baseline

- deterministic byte input and artifact/diagnostic output;
- explicit process termination and failure;
- finite nominal data, sums, records, arrays, and bounded views, including the
  allocation-free D38 adapter from a place-valued fixed array to its full
  immutable view;
- zero-field records and positive fixed-array lengths with target capacity kept
  outside language validity;
- profile-owned static-storage refusal with an exact-or-bounded canonical
  witness rather than target-dependent type validity or arbitrary-precision
  source arithmetic;
- checked integer arithmetic sufficient for compiler indexing and layout;
- deterministic control flow, calls, and recursion with five locally checked
  block-exit effects, no reachability-dependent validity, and explicit resource
  ceilings;
- free machines plus owner-qualified data methods whose mandatory first input
  is the owning mutable instance; reserved `self` is that ordinary receiver
  binding, not a second value model or an owner-qualified static facility;
- enough source custody to consume the exact package-resolved closure `D`;
- deterministic execution under the selected evaluator profile.

## Not implied

Implementing an Omega operation does not automatically add the corresponding
surface to Epsilon. Private paired-word arithmetic, layout records, checked
indexes, or provider tables may remain implementation techniques. Epsilon does
not acquire Omega's proof language, dependent authoring surface, general
boundary-trait system, package model, optimizer, or runtime merely because the
compiler it produces implements those facilities for Omega programs. Epsilon's
closed `boundary trait Console` spelling denotes only the four sealed byte-I/O
operations fixed by its own contract.

## Decision method

For each candidate:

- cite the exact canonical compiler or `C` use;
- compare a Epsilon-language facility with a private implementation technique;
- state resource and rejection behavior;
- retain lower-rung meaning and mutation controls;
- remove the candidate when the motivating source disappears.

The former Epsilon-to-Delta bridge and versioned native-publication matrix are
deleted. New progress is measured against the
Delta-written Epsilon evaluator, exact closure `D`, and Omega's
`alpha_bootstrap` product edge—not a growing sequence of private snapshot
formats.
