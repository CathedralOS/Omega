# Index algebra laws

One selected algebra's checked laws -- left distributivity of `*` over `+`,
`add(a, 0) == a`, and `mul(a, 1) == a` -- make `Indexed<A * (B + C + 0) * 1>`
the same instance as `Indexed<A * B + A * C>`, so a value carried at the first
index is accepted where the second is required.

Owning task: **OPERATOR-MACHINE-SUPPLY** (`TASKS.md`). This row closed that
item's former bullet "extend `OpenIndexOperationSelection`
(`omega-rust/psi/compilation/checking/src/value_custody/type_references/open_index_expressions.rs`)
with distributivity and zero/one law evidence surviving
`monomorphization::finalize_domain_instance_identities`"; the board keeps only
the nullary-identity bridging named below. The rule is
[licensed normalization](../../../../wiki/spec/proofs/contracts.md#licensed-normalization):
"Associativity, commutativity, distributivity, and zero/one identities require
their respective laws; one does not imply another."

Every place copies that sentence and the same operation. The trait
`IndexRing<Operand>` declares `operator + add`, `operator * mul`, the two
identity laws and `mul_distributes`; `Projection` conforms with both operations
keeping their left operand, so every law is proved by its satisfier. A machine
`carry<A, B, C, Algebra satisfies IndexRing<u64>>` binds the index operations to
its explicit bound. `main` runs `carry<2, 3, 4, Projection>` on seeds 5 and 9 and
reaches `done()` (exit 0) only when both values come back unchanged; any other
value takes `bad()` and traps. Index types are erased at runtime, so this is the
runtime falsifier for the transport, not for the algebra.

Each reject twin removes only the `mul_distributes` law from the trait and its
conformance. Nothing then licenses the expansion, and the same transfer refuses
with `index compatibility condition ... ==Indexed<A * B + A * C> is not
established`.

| Place | Positive | Reject twin | Transport |
| --- | --- | --- | --- |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the value moves between two locals of the two spellings |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the value is stored in and read back from a `Holder<i64 in Indexed<...>>` field |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | the value is the actual of a parameter declared at the second index |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | a call result at the first index lands in a local at the second |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both arms yield first-index values (5 and 9); the join lands at the second |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named constants `ZERO`/`ONE` are the identity elements and `TWO`/`THREE`/`FOUR` instantiate the binders |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a nested successor machine receives the value at the second index |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a forwarding machine declares the first index in and the second out |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each of five target-scoped `carry` bodies performs the transfer |

The places are pinned as
`tests/omega/{pass,fail}/facts/index_algebra_laws_through_<place>_{exit,rejected}`.
Every positive binds `ProgramEntry` for `windows_x86_64`, `linux_x86_64`,
`macos_arm64`, `linux_arm64` and `macos_x86_64`.

Two controls separate the laws:
[`identity_absent`](identity_absent/main.omg) keeps distributivity but drops both
identity laws and must reject with the same diagnostic, and
[`right_distribution`](right_distribution/main.omg) declares only the right law
`mul(add(a, b), c) == add(mul(a, c), mul(b, c))` and identifies
`Indexed<(A + B) * C>` with `Indexed<A * C + B * C>` (exit 0). They are pinned
as `tests/omega/fail/facts/index_algebra_laws_identity_absent_rejected` and
`tests/omega/pass/facts/index_algebra_laws_right_distribution_exit`. Two check
fixtures bound the laws: `tests/omega/fail/generics/index_mutual_distribution_terminates`
(each operation distributes over the other, so neither expansion runs and
checking terminates) and `tests/omega/fail/generics/index_identity_law_under_requires`
(a law under its own `requires` licenses nothing).

## Baseline

On the fixed base `4f87c9e08b1a7217960f7f69c440a9d2f09d278d` (shared generation
`gen-4f87c9e08b1`, omega sha256 `0a78edae3c9f54eaf0e365c3e9b42758ad641ba008619914ab64180f0235adae`,
host `macos_x86_64`) every positive place refuses before execution with
`index compatibility condition ... Indexed<A * B + C + 0 * 1>==Indexed<A * B + A * C> is not established`,
once for the generic body and once for the `carry$specialized` copy. (The
diagnostic display drops parentheses: `A * B + C + 0 * 1` is the authored
`A * (B + C + 0) * 1`.)

## Requirements

- **Establishment:** the laws are trait law slots; the bound
  `Algebra satisfies IndexRing<u64>` supplies their checked evidence through the
  ordinary conformance, and `Projection`'s satisfiers prove each one. A law slot
  is detected by shape only: `R(a, S(b, c)) == S(R(a, b), R(a, c))` (and the
  right-hand form) with distinct parameters, and `R(a, e) == a` / `R(e, a) == a`
  with `e` a closed integer literal. The reject twins and `identity_absent` show
  that an absent law licenses nothing.
- **Identity and lifetime:** the selection records the law evidence beside the
  existing commutativity/associativity flags, and body copies carry it. The
  specialized copy's check (`carry$specialized...`) normalizes the same way with
  the `Projection` application and literal leaves, so the identity survives
  specialization and domain-instance finalization.
- **Composition:** distribution only expands a product over the named sum of the
  same algebra instance (same provider application, trait and carrier), never
  when that sum also distributes back over the product, and at most 64
  expansions per normal form.
  Identity elimination runs before distribution, so `A * (B + C + 0) * 1`
  reduces bottom-up to `A * B + A * C`.
- **Soundness boundaries:** a nullary identity requirement such as the core
  `CommutativeSemiring::zero()` is not a closed element; connecting it to a
  literal needs ordinary unfolding of a concrete provider and is not licensed
  here. Unequal normal forms establish nothing.
- **Psi 05-07 and Omega 00-09:** the index is erased; the value is an ordinary
  `i64` through calls, a record field, a match join and nested machines. No new
  layout, calling-plan or encoding path is exercised; the row claims no physical
  boundary.

## Execution

```sh
omega run --both samples/cli/facts/index_algebra_laws/<place>/main.omg
```

Measured on `macos_x86_64` only. The other four hosted targets' native parity is
not observable on this host.
