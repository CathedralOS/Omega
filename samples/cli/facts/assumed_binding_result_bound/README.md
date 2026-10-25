# Assumed binding result bound

The root-assumed u32 result <= 1 survives each boundary and indexes `[7, 11]`
of length 2, observing 11; `Supplied::get` retains its own contract.

The original root reproduction is preserved in [main.omg](main.omg) and [build.omg](build.omg).
The first fetched-main failure was:

```
call `assume_binding_contract` supplies static machine arguments, but its generic callee did not resolve
```

## Places

The nine places carry the root-assumed bound through each language construct,
accompanied by `<place>/invalid/` reject twins and `<place>/controls/insufficient_bound/` controls:

| Place | Positive Program | Invalid Reject Twin | Insufficient Bound Control | Carrier / Transport |
| --- | --- | --- | --- | --- |
| root | [main.omg](main.omg) | — | — | root-assumed bound indexes array directly |
| local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) | [local/controls/insufficient_bound/main.omg](local/controls/insufficient_bound/main.omg) | root-assumed bound copied into local variable |
| field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) | [field/controls/insufficient_bound/main.omg](field/controls/insufficient_bound/main.omg) | root-assumed bound stored in and projected from record field |
| parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) | [parameter/controls/insufficient_bound/main.omg](parameter/controls/insufficient_bound/main.omg) | root-assumed bound passed as explicit machine parameter |
| call result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) | [call_result/controls/insufficient_bound/main.omg](call_result/controls/insufficient_bound/main.omg) | root-assumed bound returned across machine call boundary |
| match join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) | [match_join/controls/insufficient_bound/main.omg](match_join/controls/insufficient_bound/main.omg) | root-assumed bound preserved across match convergence join |
| const name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) | [const_name/controls/insufficient_bound/main.omg](const_name/controls/insufficient_bound/main.omg) | root-assumed bound named and constrained against constant |
| a nested machine | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) | [other_state/controls/insufficient_bound/main.omg](other_state/controls/insufficient_bound/main.omg) | root-assumed bound transitioned to successor machine argument |
| another machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | [other_machine/controls/insufficient_bound/main.omg](other_machine/controls/insufficient_bound/main.omg) | root-assumed bound forwarded through another machine |
| another target body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) | [target_body/controls/insufficient_bound/main.omg](target_body/controls/insufficient_bound/main.omg) | root-assumed bound consumed across target-scoped body implementations |

Run each place with `omega run --both <place>/main.omg`. Public Build
evaluation of that place's `assume_binding_contract` projects the exact root
Product risk rows for the activation. No `omega update` lock is required, and
a retained lock must still match those rows.

## Requirements and Evidence

Owning spec: [assumed binding contracts](../../../../wiki/spec/build/assumed_binding_contracts.md).
This row discharges the **explicitly accepted exit-side result contract**: the root's
`assume_binding_contract<Required::get, Supplied::get>` installs
`Required::get ensures result <= 1` on a supplier that guarantees nothing,
and the caller's downstream index proof consumes the assumed bound.

Full 8-layer specification and lowering evidence:

- **Establishment:** The boundary contract is introduced via root-declared
  `Build::assume_binding_contract<Required::get, Supplied::get>` accompanied by
  explicit `Extremely high risk - assumed binding contract` risk row acceptance
  in `omega.lock`. This establishes `Required::get ensures result <= 1` at the
  caller boundary without any post-hoc caller equality guard (`transition x == 100`).
  The supplier (`Supplied::get`) guarantees nothing, and the caller boundary
  contract is refutable and refutably accepted.
- **Identity and lifetime:** The assumed exit bound attaches strictly to the
  exact `Required::get()` call result identity. The lock receipt commits the
  exact contracts, parameter/result mapping, selected implementation, root,
  target, assumed chain, and the consumed semantic source closure. Modifying
  the source or supplier identity immediately invalidates the lock acceptance
  and requires re-review.
- **Composition:** The assumed `result <= 1` bound travels through all nine
  places (`local`, `field`, `parameter`, `call_result`, `match_join`,
  `const_name`, `other_state`, `other_machine`, `target_body`) to prove safe
  indexing into `values: [u32; 2] = [7, 11]` of length `2` (`index < 2`), observing
  `11`. Each `<place>/main.omg` vs. `<place>/invalid/main.omg` (and
  `<place>/controls/insufficient_bound/main.omg`) differs only in weakening the
  ensured bound from `<= 1` to `<= 2`, where indexing length 2 is no longer proved.
- **Soundness boundaries:** All nine `<place>/invalid/` reject twins (and
  `<place>/controls/insufficient_bound/`) reject at `--check` with
  `cannot prove index ... is within length 2`. The additional controls under
  `controls/` enforce noninheritance across packages, runtime carrier/order/passing-mode
  agreement, and rejection when no provider is selected (`missing_link`).
  Any edit to the accepted source graph invalidates the host-local lock
  acceptance before execution.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Source constructs lower
  without loss into canonical target-neutral Terminal Psi (`PSITERM\0`). Boundary-application
  demands (`Required::get`) and assumed exit certificates lower into canonical
  representations without erasing the boundary call demand, and execute in the
  Terminal Psi interpreter (`omega run --both`) after provider settlement.
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):**
  Target realization joins Build companion rows (`ProgramEntry`, `ProviderPlan`,
  and `assume_binding_contract` risk rows) to connect `Supplied::get` to `Required::get`
  across the four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`,
  `linux_arm64`). Target layout plans resolve `[u32; 2]` contiguous geometry (8 bytes,
  alignment 4), and `CallPlan`/`StatePlan` lower the boundary call and `u32` scalar
  return carrier across register conventions.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):**
  Backend lowering selects target ISA instructions (`x86_64` / `arm64`) for the
  settled provider invocation, 32-bit zero-extended index computation, and direct array
  element load (`values[index] == 11`). Backend assigns register homes and disjoint
  stack-frame activation storage, inserting target-appropriate entry prologues and
  multi-return epilogues.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):**
  Symbolic stack-frame displacements and settled provider call targets resolve, basic
  blocks and branch offsets relax with offset reflow, machine-state footprint
  evidence validates, and direct PE/ELF/Mach-O images execute natively with
  Terminal Psi agreement (`omega run --both` reports `native exit 0` and `interp exit: 0`)
  on host platforms (`windows_x86_64` and `macos_arm64`).

Remaining open on the ASSUMED-BINDING-CONTRACTS board: input-side authority
issuance, invocation correspondence and operational/lifetime relationships
fail closed; unsupported authority establishment is not demonstrated here;
project-level acceptance across real dependency packages is exercised only
through the repository fixtures.

## Additional controls

`controls/` retains direct-supplier noninheritance, missing/competing/cyclic
supply, bad checked supplier guarantees, runtime carrier/order/passing-mode
mismatches, blocking incompatibility and build-time calls — all rejecting at
`--check` with their authored fragment. Two controls differ:

- `controls/missing_link` has no assume link, so `Required::get` has no
  provider. `--check` passes (provider admission is a realization
  obligation); `omega run` rejects `no provider is selected for boundary
  requirement` for the entry target.
- `controls/renamed_lifetime_mapping` is a positive: same lifetime arity,
  alpha-renamed (`'a,'b` vs `'x,'y`) — checks clean, proving the link accepts
  alpha-equivalent lifetime correspondence.

The bounded result link does not close input-side authority issuance or the
full ASSUMED-BINDING-CONTRACTS board outcome. Unsupported authority
establishment, invocation correspondence and operational/lifetime
relationships fail closed.

## Fixture consent and evaluated risk rows

Each place's root Build is the Product decision for an activation with no
retained lock. Evaluation records `assume_binding_contract`, and product
admission projects the **Extremely high risk - assumed binding contract**
rows from that checked compilation. The nine corpus fixtures still opt into
the same projection with `accepted_assumed_bindings.txt` when a test supplies
historical rows. A retained lock that differs from the fresh rows rejects.
Changing the accepted relationship or its source closure invalidates a
retained lock before execution.
