# Delta feature ledger

Delta is justified by the smallest robust surface needed to author the
Delta-written full Omega compiler source closure `D` and compile it with the
Gamma-written Delta compiler.

[`LANGUAGE.md`](LANGUAGE.md) currently records the OWNER Q7 contract proposal; it
does not fix Delta v1 facilities before that ruling. This ledger is rationale
and decision input, not a second specification or a vote by historical
samples. After OWNER Q7, a future facility belongs to Delta only when at least one
of these holds and the normative language contract is revised explicitly:

1. the Delta-written Omega compiler closure `D` requires it;
2. implementing or compiling `D` coherently requires it;
3. it materially reduces total implementation or assurance cost without
   weakening deterministic failure, resource, or safety behavior.

## Retained baseline

- deterministic byte input and artifact/diagnostic output;
- explicit process termination and failure;
- finite nominal data, sums, records, arrays, and bounded views;
- checked integer arithmetic sufficient for compiler indexing and layout;
- deterministic control flow, calls, and recursion with explicit resource
  ceilings;
- enough source custody to consume the exact package-resolved closure `D`;
- conservative lowering to canonical Alpha tape.

## Not implied

Implementing an Omega operation does not automatically add the corresponding
surface to Delta. Private paired-word arithmetic, layout records, checked
indexes, or provider tables may remain implementation techniques. Delta does
not acquire Omega's proof language, dependent authoring surface, general
boundary-trait system, package model, optimizer, or runtime merely because the
compiler it produces implements those facilities for Omega programs. Delta's
closed `boundary trait Console` spelling denotes only the four sealed byte-I/O
operations fixed by its own contract.

## Decision method

For each candidate:

- cite the exact canonical compiler or `C` use;
- compare a Delta-language facility with a private implementation technique;
- state resource and rejection behavior;
- retain lower-rung meaning and mutation controls;
- remove the candidate when the motivating source disappears.

The former Delta-to-Gamma bridge and versioned native-publication matrix are
deleted. New progress is measured against the
Gamma-written Delta compiler, exact closure `D`, and one Alpha-tape edge—not a
growing sequence of private snapshot formats.
