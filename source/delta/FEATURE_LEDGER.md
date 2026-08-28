# Delta feature ledger

Delta is justified by the smallest robust surface needed to implement and
publish the final lower-rung compiler that consumes the production Omega
compiler source closure `C`.

This ledger is not a vote by historical samples. A facility belongs to Delta
when at least one of these holds:

1. the canonical Delta compiler source requires it;
2. direct compilation of `C` requires it;
3. it materially reduces total implementation or assurance cost without
   weakening deterministic failure, resource, or safety behavior.

## Retained baseline

- deterministic byte input and artifact/diagnostic output;
- explicit process termination and failure;
- finite nominal data, sums, records, arrays, and bounded views;
- checked integer arithmetic sufficient for compiler indexing and layout;
- deterministic control flow, calls, and recursion with explicit resource
  ceilings;
- enough source custody to consume the exact package-resolved closure `C`;
- conservative target lowering and artifact emission.

## Not implied

Implementing an Omega operation does not automatically add the corresponding
surface to Delta. Private paired-word arithmetic, layout records, checked
indexes, or provider tables may remain implementation techniques. Delta does
not acquire Omega's proof language, dependent authoring surface, boundary
traits, package model, optimizer, or general runtime merely because the
compiler it produces implements those facilities for Omega programs.

## Decision method

For each candidate:

- cite the exact canonical compiler or `C` use;
- compare a Delta-language facility with a private implementation technique;
- state resource and rejection behavior;
- retain lower-rung meaning and mutation controls;
- remove the candidate when the motivating source disappears.

The former versioned bridge IR/checkpoint matrix is retired. New progress is
measured against the live direct input closure and one complete compiler edge,
not a growing sequence of private snapshot formats.
