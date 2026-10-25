# Retained shared foreign backing

Parent: **FOREIGN-RETAINED-ARGUMENT-BACKING** in `TASKS.md`.

A shared foreign registration retains the exact original `Buffer` backing until
completion consumes `Pending`; completion observes the original value 37.
Each place transports that same obligation. Its invalid twin writes 99 while
that registration remains live and must reject for the active borrow.

The provider fixture has two slots. Acquisition issues one generation-checked
linear capacity; registration consumes it, completion returns that same capacity,
and release retires it. A separate registration generation changes on every
registration, including reuse before capacity release. Completion clears the
pointer before returning capacity. Invalid transitions trap under the authored
`crashes Trap` contracts. The C self-test checks simultaneous values 37 and 91,
exact capacity return and both generation lifecycles; nine subprocess controls
check exhaustion, null backing/output, stale capacity/pending generations,
duplicate registration/completion/release and release while registered.

The retained pointer reads one `u64` at the original record home. Its 8-byte
extent comes from the declared layout; the counted content projection is a
semantic amount, not an inferred native pointer/count association. The lifetime
obligation forbids overlapping writes and early disposal. It creates no pointer
inside `Pending`, copying permission or implicit write-back. The completion's
observed output has its own explicit `&mut u64` contract.

## Reproduce on Linux

The fixture declares imports for all four hosted target slots. Only Linux x86-64 was exercised.
All positive builds bind the four hosted program-entry slots; the other hosts
have no execution evidence here.

```sh
python3 samples/cli/facts/foreign_retained_shared_borrow/fixture/prepare.py
export LD_LIBRARY_PATH="$PWD/build/foreign_retained_fixture${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
build/gate-cli/debug/omega run --both samples/cli/facts/foreign_retained_shared_borrow/local/main.omg
```

Native execution must report `native exit 0`. The default Terminal host currently
**declines this custom foreign binding** because it has no trusted implementation.
That is not interpreter execution or agreement. This row's Terminal acceptance
and the larger capability remain open. The same compiled native path is exercised
through all nine places; no host adapter is recognized by sample name in the CLI.

| Place | Positive | Invalid twin |
| --- | --- | --- |
| local | [source](local/main.omg) | [write](local/invalid/main.omg) |
| field | [source](field/main.omg) | [write](field/invalid/main.omg) |
| parameter | [source](parameter/main.omg) | [write](parameter/invalid/main.omg) |
| call result | [source](call_result/main.omg) | [write](call_result/invalid/main.omg) |
| match join | [source](match_join/main.omg) | [write](match_join/invalid/main.omg) |
| constant name | [source](const_name/main.omg) | [write](const_name/invalid/main.omg) |
| a nested machine | [source](other_state/main.omg) | [write](other_state/invalid/main.omg) |
| another machine | [source](other_machine/main.omg) | [write](other_machine/invalid/main.omg) |
| target body | [source](target_body/main.omg) | [write](target_body/invalid/main.omg) |

The matching nine positives and negatives are pinned under
`tests/omega/{pass,fail}/facts/foreign_retained_shared_borrow_*`.
The controls additionally require early disposal, capacity reuse, repeated
completion, mutable/call-scoped retention and unsupported persistent retention
to reject for their stated obligations. Two simultaneous registrations remain
independent; completing one leaves the other loan live. A post-completion write
and raw values of the same carrier (including a factory with an unused borrow)
remain valid. Returning local backing or redeeming borrowed content as independent
owned custody rejects. A separate phantom control needs no
foreign effects and executes in both engines.

## Requirement coverage and limits

- Establishment: the admitted boundary's owner-unique content projection and
  explicit lifetime establish a retained loan. Its scalar token is not a pointer.
- Identity/lifetime: exact caller places and loan occurrences survive erasure,
  nesting, calls, joins and state transfers. Mutable access and disposal conflict;
  completion ends only the consumed occurrence.
- Composition: the consuming completion receives the retained token through the
  nine places. No caller guard re-establishes the loan or authorizes a write.
- Authority: nonspatial dependencies cannot supply reference-result pointers or
  access exemptions. Phantom lifetime spelling supplies no loan. Unpropagated
  persistent stores remain rejected.
- Psi 05–07: checked boundary custody retains its exact source/result projection;
  ordinary loan flow carries the token's dependency without a fabricated
  reference field. Terminal execution of this provider remains unsupported.
- Omega 00–02: the existing borrowed-record path resolves the original home,
  checks access, declared shape, offset and full extent, and supplies the native
  pointer. Protocol tokens remain ordinary transparent one-word records.
- Omega 03–06: native calls use ordinary register/stack and activation homes;
  suspended callers retain their frame backing while helpers transport tokens.
- Omega 07–09/emission: Linux ELF imports execute the authored C lifecycle and
  native oracle. Windows, macOS and Linux ARM64 execution is unverified.

This covers the direct shared fixed-record rung. Moved backing, explicit
snapshots, mutable retention, wider/recursive graphs, more general retained
provider installations and complete Terminal host integration remain open.
