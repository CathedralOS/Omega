# Inferred array equation call

Fact: an omitted Capacity is inferred as 2 from the actual [u8; 2] type and Length == [u8; Capacity], independently of element values.

The frozen root calls `capacity(items)` with a declared `[u8; 2]` actual. It
previously failed before ordinary argument inference could anchor `Length`.
Its SHA-256 is `a03068442c4021bdb12f1cf6e784f6b91d3167944159783d882cfc2dfb46c4b3`.
The compiler now combines closed declaration-backed actual/result types with
where equations in the existing structural solver. Explicit static arguments
remain fixed, and ordinary compatibility and specialization bounds still apply.

| Place | Positive Project | Reject Twin | Establishment and transport |
| --- | --- | --- | --- |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | declared array local |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | selected nominal record field |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | declared ordinary parameter |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | declared array return type |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both arms carry the same declared type; both branches execute |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | declared array constant |
| another state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | state arrival parameter |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | ordinary call forwarding |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | four hosted implementations perform the same call |

Each positive declares `Expected exit: 0` and binds ProgramEntry on
windows_x86_64, linux_x86_64, macos_arm64 and linux_arm64. Each `invalid/` twin
changes exactly one consuming call to `capacity<[u8; 2], 3>(items)` (using that
place's original actual expression). The required rejection is
`where equation binds Capacity to 2 but its explicit argument is 3`, with the
compiler's backticks. Package errors or unrelated typing errors do not count.
Mirrored fixtures live under `tests/omega/{pass,fail}/facts/` with prefix
`inferred_array_equation_call_through_`.

## Execution

Every positive place is an application that exits 0, and each carries an
authored `expected_native_exit.txt` containing `0` beside its entry.
`omega run --both` must report Terminal Psi and native execution agreeing:

```sh
omega run --both samples/cli/facts/inferred_array_equation_call/main.omg
omega run --both samples/cli/facts/inferred_array_equation_call/local/main.omg
omega run --both samples/cli/facts/inferred_array_equation_call/field/main.omg
omega run --both samples/cli/facts/inferred_array_equation_call/parameter/main.omg
omega run --both samples/cli/facts/inferred_array_equation_call/call_result/main.omg
omega run --both samples/cli/facts/inferred_array_equation_call/match_join/main.omg
omega run --both samples/cli/facts/inferred_array_equation_call/const_name/main.omg
omega run --both samples/cli/facts/inferred_array_equation_call/other_state/main.omg
omega run --both samples/cli/facts/inferred_array_equation_call/other_machine/main.omg
omega run --both samples/cli/facts/inferred_array_equation_call/target_body/main.omg
```

All ten places, root included, were observed with `native exit 0` and
`interp exit: 0`. `python3 tools/sample_native_gate.py --facts --filter
inferred_array_equation_call` reports 10/10 samples meeting their documented
oracle on macos_arm64 and 10/10 intended-invalid controls matching their
authored rejection. The nine pinned positive fixtures score
`pass/facts/inferred_array_equation_call_through_<place>_exit built ... exit:0`
in `python3 tools/corpus_gate.py --native --filter
facts/inferred_array_equation_call`.

### The oracle discriminates

The authored `0` is not assumed. Two probes on the `local` place change the
construct's own input and make the carried result differ, so the consumer's
`transition observed == 2` must take `bad()` and trap:

| Program | Native engine | Terminal Psi | wrapper exit |
| --- | --- | --- | --- |
| `local/main.omg` (authored) | `native exit 0` | `interp exit: 0` | 0 |
| same place, `capacity(items) + 1u64` | `native signal 5` | `interp crash: Trap` | 255 |
| same place, actual widened to `[u8; 3]` | `native signal 5` | `interp crash: Trap` | 255 |

Probe one corrupts the carried result; probe two corrupts the equation's own
input, so `Length == [u8; Capacity]` binds `Capacity` to 3 and the returned
value leaves 2. Both engines trap together in both probes, so the recorded
oracle separates a correct inferred `Capacity` from a wrong one.

The `invalid/` twins discriminate in the other direction: they are refused
during checking rather than run, so they cannot be mistaken for a passing
positive. Each twin was observed at checking exit 1 with the exact
`where equation binds \`Capacity\` to 2 but its explicit argument is 3`
diagnostic naming that place's own entry point:

```sh
omega --check samples/cli/facts/inferred_array_equation_call/<place>/invalid/main.omg
```

The authority is [structural equations and inference](../../../../wiki/spec/language/generics.md#structural-type-equations-and-inference),
canonical Git blob `e3d20c0c1f44d36ac6221811e46446975bea57e8`:

> For ordinary data and machine applications, omitted arguments can be recovered
> only when the combined structural equations and existing argument/result
> inference determine one substitution.

> All applicable occurrences must be consistent, not just the first one visited.

> Inference selects arguments before checking ordinary compatibility.

The implementation/review context fingerprint is
`9e16129bdad951e9ebf98419ff1ca28ef66c9f6ca656895fbc3ab28b8d299be8`.
Resolved declaration handles retain identity through deferred body completion;
receiver projections select fields only within the exact declared nominal owner.
Literals, element values, private flow facts and storage-size coincidences do
not select generic arguments. Focused controls cover differing element values,
same-named const/type declarations, partial explicit calls, repeated conflicts
in both orders, incompatible match arms/results, constructor identity, missing
anchors, occurs cycles, copy bounds, nested/generic owner fields and deferred
bodies. A typed result may anchor inference; its argument must still satisfy
the selected array extent.

Terminal Psi and native lowering consume the ordinary completed specialization
and its concrete array carrier. This repair changes no carrier, loan lifetime,
ABI, frame allocation, image emission or backend semantics. Native/Terminal
exit agreement exercises the existing call, state transport, branch layout,
frame and image path. The witnesses assert Capacity==2 rather than merely
compiling; the join executes both arms. Linux x86-64 supplies matching-host
execution evidence. Checks for the other hosted targets supply source evidence
only; they do not establish execution on absent hosts.

## Requirements and Evidence

- **Parent outcome:** `STRUCTURAL-GENERIC-MATCHING` (`TASKS.md`).
- **Ticket:** `a9cb6a50914145348064fec720014cd4` (owner `DeepSeek / w24 pump16`).
- **Requirement established:** declaration-backed closed array-call inference.
  An omitted `Capacity` is recovered as 2 from a declared `[u8; 2]` actual
  combined with the `where Length == [u8; Capacity]` equation, independently of
  element values, and that recovered argument survives all nine places into a
  consuming operation that observes the value.
- **Pinned fixtures:** the nine positive places are pinned as
  `tests/omega/pass/facts/inferred_array_equation_call_through_<place>_exit/`,
  each carrying `main.omg`, `expected_native_exit.txt` with `0` and
  `expected_interpreter_exit.txt` with `0`. The nine reject twins are pinned as
  `tests/omega/fail/facts/inferred_array_equation_call_through_<place>_rejected/`,
  each binding `ProgramEntry` for all four hosted targets and carrying
  `expected.txt` with the required diagnostic. Every `<place>/invalid/main.omg`
  differs from its positive by the construct's own explicit argument only.
- **Obligations accounted for:**
  - *Establishment:* the declared `[u8; 2]` actual and the `where` equation
    together determine `Capacity`; the call is not guarded by a post-hoc caller
    equality and the recovery happens before compatibility checking.
  - *Identity and lifetime:* the selected `Capacity` is the exact inferred
    argument for that call, retained through the ordinary completed
    specialization and its concrete array carrier.
  - *Composition:* the call substitutes its exact actual in every place, the
    match join retains the fact on both incoming arms, and the consuming
    transition is proved from the carried value.
  - *Soundness boundaries:* the reject twins show an explicit argument that
    contradicts the inferred one is refused, so inference never silently
    overrides an authored argument.
  - *Checked-to-Terminal Psi lowering (`Psi 05–07`):* every place lowers to
    Terminal Psi and executes in the interpreter, and the discriminating probes
    show both engines leave the authored outcome together.
  - *Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):*
    the `target_body` place checks the call in each target-scoped body, so the
    specialization is settled per target rather than once for the host.
  - *Instruction selection, register homes, and activation frame realization
    (`Omega 03–06`):* the call, its `u64` result and the transition guard are
    realized on the hosted target, with the frame and image path unchanged.
  - *Form encoding, branch relaxation, relocations, and native image emission
    (`Omega 07–09`):* the nine pinned fixtures emit and execute a Mach-O image
    that agrees with Terminal Psi.
- **What remains open:** open schemas, late receiver/extension selection,
  operator/conformance/machine/evidence/value binders, other const-index kinds,
  unspecialized alternatives and computed structured equations.

This closes only the declaration-backed closed array-call inference slice of
STRUCTURAL-GENERIC-MATCHING. The Std-backed exit-92 and exit-7 customers retain
their existing Console/entry dependencies. No parent capability closure is
claimed.
