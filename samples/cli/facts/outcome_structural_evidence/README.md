# Selected evidence through owned structural inputs

Selecting Success from pick(Gate{threshold:5}) transports exact
above(gate.threshold) evidence and preserves threshold 5.

The root `main.omg` is the original probe. Each row uses the same Gate value,
Success guarantee, `selected` selector and threshold comparison. Only the Gate's
transport changes. The abstract proposition is nominal witness evidence; its
name does not introduce an arithmetic theorem.

| Place | Project | Transport |
| --- | --- | --- |
| local | `local/` | a local Gate |
| field | `field/` | Holder.gate |
| parameter | `parameter/` | a Gate parameter |
| call result | `call_result/` | make_gate() |
| match join | `match_join/` | both Boolean arms construct Gate{threshold:5} |
| const name | `const_name/` | NAMED_GATE |
| another state | `other_state/` | inspect's Gate parameter |
| another machine | `other_machine/` | deliver() forwards the Gate |
| another target body | `target_body/` | four hosted target bodies |

Every positive binds ProgramEntry for windows_x86_64, linux_x86_64,
macos_arm64 and linux_arm64, and declares Expected exit: 0. Run a prepared
project with `omega run --both <place>/main.omg`; use `omega update --project
<place> --target <host>` first. Each row also calls pick_failure(), exercises
the nonmatching arm without manufacturing a Success witness, and traps if
that dispatch or the preserved threshold is wrong. Both match-join arms run.

Each `invalid/` twin removes only `selected = Fresh;` from pick. It must reject
at the producer's applicable ordinary exit with:

```
outcome-specific evidence `selected` is not definitely assigned on the matching ordinary exit
```

Selection is fact-sensitive: removing establishment invalidates the exact named
guarantee consumed at the Success operation, even though the runtime comparison
itself does not execute a proof. The nine pairs are pinned under
`tests/omega/{pass,fail}/facts/outcome_structural_evidence_<place>`.

## Obligations and implementation

Establishment belongs to every ordinary producer exit of the matching case.
Failure exits owe no Success conclusion. The caller imports a row only under
the exact result-case guard, retaining the selected Type witness identity.
Equal proposition applications do not identify distinct witness values.

Owned reference-free arguments are evaluated value snapshots. Their later source
homes do not become proof subjects again merely by retaining the same spelling.
Checking uses the existing normal-value path owner to compare completed scalar
and projected values, including copies, later argument writes and branch agreement.
Unknown captures and unsupported computed subjects cannot establish a binding.
These comparison keys stay internal to checking. References and records containing
borrowed storage do not acquire detached owned snapshot semantics.

The checked declaration classifies each subject by authored formal position and
resolved field declarations. Terminal lowering retains a declaration-owned
subject roster: scalar and structural parameter namespaces are distinct, and
structural rows carry the complete authenticated field path. Ordinary evaluated
call operands retain actual prefixes. External decoding rejoins the selected
callee obligation, case, field, evidence term and exact subject roster. The
canonical codec/fingerprint retains these rows; no runtime proof storage or
cleanup is manufactured. The native and Terminal engines consume ordinary
record copies and payloadless case dispatch.

Focused call fixtures additionally check nested projections, two same-shaped
record formals after a scalar formal, a closed constant proposition argument,
and preservation of an old copy while the source becomes threshold 6. The
checking-only snapshot forwarding controls use an erased requires lane; its
structural state-contract native lowering remains outside this row. Their twins
reject mutated record/scalar subjects, later-argument mutation and an unsupported
same-name computed subject. They are not counted as runtime row acceptance.

The governing contract is `wiki/spec/proofs/contracts.md`, blob
`0551b01c0d931150f979d6ea295d3bafd99629bf`: exact subjects/substitutions,
path- and result-sensitive guarantees, validity intersections, Type witness
identity, and erased evidence retaining semantics without executable storage.
The original baseline checked but `run --both` refused structural-input
selection at Terminal lowering. This row advances owned, unrestricted,
unqualified reference-free structural formals with exact primitive projections.

Remaining OUTCOME-SPECIFIC-CALL-EVIDENCE scope includes entry claims, other
contract rows, crash routes, borrowed/reference-bearing structural inputs,
computed dependent subjects and broader evidence scope transport. This row does
not claim those capabilities or other-host native execution.
