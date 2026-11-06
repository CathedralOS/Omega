# Borrowed captured dynamic values

Ratified 2026-11-06. A container may hold different borrowed state types paired
with their matching named implementations. Authors bind arguments explicitly;
the compiler derives the finite sum and dispatch. No receiver, privacy facility,
new keyword, implicit allocation or owned erased state is introduced.

The governing rules are [dynamic dispatch](../../spec/terminal-psi/dynamic_dispatch.md#borrowed-captures),
with [guide examples](../../language_guide/chapter_14_traits.md#capturing-borrowed-state)
and [replacement accounting](../../spec/build/component_publication.md#replacement-granularity).
This document tracks implementation, not evidence that the compiler accepts the
new form. `RESTORE-DYNAMIC-DESCRIPTOR-AND-TABLE-CUSTODY` on the
[execution board](../../../TASKS.md) owns this work alongside selector-only
dispatch, known-selection envelope refinement and its existing host coverage.

## Implementation work

1. Elaborate `Impl as dyn Trait<_> { parameter: borrow }` into an exact named
   conformance and resolved requirement-parameter bindings. Reject ambiguous
   names, incomplete maps, wrong types and escaping hidden result types. Preserve
   full contracts and parameter identity across inherited and overloaded rows.
2. Carry captured places, storage versions, lifetime/access constraints and
   multiplicity through all ordinary value placements. Dynamic calls supply
   the captures and remaining explicit arguments, reborrow mutable captures,
   prove current preconditions and preserve postconditions and returned loans.
3. Collect and canonicalize the finite artifact roster after concrete generic
   applications are known, before representation and dispatch. The same captured
   interface has one roster across helpers and containers. Preserve its identity
   in source checking, Terminal Psi, interpreter and native artifacts.
4. Lower borrowed environments as a closed sum with matching dispatch. Retain
   exact row, capture, result, effect, custody and resource evidence. Keep scalar
   selector-only lowering distinct; its existing 8-byte table coverage does not
   impose that width on captured values. Verify all hosted targets.
5. Publish code, roster/layout and custody dependencies into replacement closure.
   Reject Independent crossings, unknown roster tags and old/new era mixing.
   Local proxies retain the existing component boundary plans. Owned capture
   and cross-artifact erased packages remain outside this work.

## Acceptance

Keep existing `runtime_dyn_*_exit` and known-selection narrowing tests passing
on Windows, macOS and Linux (`python3 tools/corpus_gate.py --both --filter traits/`).
Carry captured values through the repository's nine ordinary value placements
with focused positive/negative controls, without generating a new facts matrix.
Include the following behavior in compiler, Terminal and native coverage:

- Queue and cache entries share a container and dispatch to their own state.
  A runtime branch and a generic helper append different admitted entries through
  a mutable container reference. Reusing an entry does not enlarge its roster.
- Missing second arguments, ambiguous inherited/overloaded names, incorrect
  state types and an erased `S` or `&S` result reject. Failed current preconditions
  reject even if they held when the capture was constructed.
- Local-state escape, mutable capture duplication and overlapping access reject.
  Moves and permitted shared copies preserve loans. Removing an entry transfers
  its loans; consuming the last holder releases access when no result loan lives.
- Capture/implementation mismatches and corrupted roster identities/tags reject.
  A new admitted variant updates every dependent dispatch/representation site,
  even if physical size is unchanged; traversal order does not change identity.
- Live captures retain their replacement dependencies. Pausing alone does not
  authorize unloading their code or reinterpreting their state. Coherent drain
  or migration/reconstruction works; local proxies preserve Independent cuts.

Source syntax examples in the guide are ratified acceptance targets, not newly
compiled fixtures in this documentation change.
