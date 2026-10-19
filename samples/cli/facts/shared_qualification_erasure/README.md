# Shared qualification erasure

Explicit shared qualification erasure retains Issued and the same Ticket referent.

`issue` establishes `Issued & Label`; an explicit reference view forgets `Label`
and passes the still-issued referent to `observe`. The runtime oracle checks the
read value is 17, exiting 0 or trapping on a mismatch. That comparison observes
the result; it does not establish the qualification required by `observe`.

This isolates a Cathedral prerequisite: lend a retained `Granted & Vacant`
extent through a `Granted` reference without discarding the owner's `Vacant`
obligation. It does not demonstrate physical initialization or retirement.

| Place | Positive | Reject twin |
| --- | --- | --- |
| Local descriptor | [main](local/main.omg) | [invalid](local/invalid/main.omg) |
| Record field | [main](field/main.omg) | [invalid](field/invalid/main.omg) |
| Parameter | [main](parameter/main.omg) | [invalid](parameter/invalid/main.omg) |
| Call result | [main](call_result/main.omg) | [invalid](call_result/invalid/main.omg) |
| Match join | [main](match_join/main.omg) | [invalid](match_join/invalid/main.omg) |
| Const evaluation | [main](const_name/main.omg) | [invalid](const_name/invalid/main.omg) |
| Another state | [main](other_state/main.omg) | [invalid](other_state/invalid/main.omg) |
| Another machine | [main](other_machine/main.omg) | [invalid](other_machine/invalid/main.omg) |
| Target body | [main](target_body/main.omg) | [invalid](target_body/invalid/main.omg) |

Each twin changes only the `Observed` alias from `Issued` to the independently
routed `Other`. It must reject with `cannot mint routed provenance` at the view,
not because an issuer declaration, import or entry binding is malformed.
Fixtures are pinned under `tests/omega/{pass,fail}/facts/shared_qualification_erasure_*`.

Run each positive with `omega run --both <place>/main.omg` and each twin with
`omega --check <place>/invalid/main.omg`. All four hosted targets have entry
bindings; the recorded native/Terminal executions are on macOS arm64. Bindings
alone do not establish execution on the other hosts. Producer identities and
observations belong in the qualification/claim record.

## Requirements and evidence

1. Establishment: `issue` is the authorized route. `as` retains existing
   membership; it is not another issuer. See [domains](../../../../wiki/spec/language/domains.md).
2. Identity and lifetime: the descriptor denotes the same storage, and the owner
   survives its use. The returned-view escape control separately checks that
   a qualification cannot extend a local's lifetime.
3. Composition: the nine places deliver the qualified reference to `observe`.
   The const place performs this operation during compile-time evaluation and
   checks its result at runtime; it does not store a runtime reference in a const.
4. Soundness: reject twins cannot substitute a different establishment route.
   Neighbor fixtures retain mutable invariants, exact indexed domains, field
   bounds and atomic observation authority. See [recasts](../../../../wiki/spec/layouts/recasts.md).
5. Psi 05–07: checked descriptor formation and occurrence-owned qualification
   survive lowering; Terminal execution reads the same authored value.
6. Omega 00–02: the ordinary Build entry and reference/record layouts feed call
   and state plans. This row needs no foreign provider or resource grant.
7. Omega 03–06: reference arguments, local storage, field reads and control edges
   use ordinary instruction selection, register homes and activation frames.
8. Omega 07–09 and emission: the native image executes the authored oracle and
   agrees with Terminal execution. This is end-to-end evidence for the exercised
   path, not an independent audit of every encoding or relocation family.

Remaining customer work is authoritative profile supply, actual retained-memory
initialization, resident read/write and complete retirement. This row does not
close that acceptance or certify unrelated atomic operations.
