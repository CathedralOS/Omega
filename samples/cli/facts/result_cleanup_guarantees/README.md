# Normal result cleanup guarantees

Parent task: `CML4`

## Governing requirement

A completed result's postcondition guarantees survive evaluation to satisfy normal automatic cleanup prerequisites (`Token::drop` requiring `token.ready`) for temporary values produced across all nine places.

**The shared Seed.ready == true observation survives each place into make_token(&seed), whose completed result is ready for normal shared-call cleanup.**

The root retains the original once-only effectful producer reproduction. The nine child projects carry the same copied `Seed` readiness through local, field, parameter, call result, match join, const name, a nested machine, another machine, and target body. Each place performs the same core operation: pass `&seed` into `make_token(&seed)`, borrow the returned temporary `Token` into `read(&make_token(&seed))`, and rely on `make_token`'s postcondition (`ensures result.ready == seed.ready`) to satisfy `Token::drop`'s cleanup prerequisite (`requires token.ready`) during automatic normal cleanup at the end of the statement. The positive oracle asserts `answer == 7` and exits 0 in both native and Terminal execution. Each invalid child changes only that carried readiness to `false`; the producer relation, cleanup prerequisite, and consuming operations stay identical.

## Places

| Place | Positive | Reject twin | Transport |
| --- | --- | --- | --- |
| Root | [`main.omg`](main.omg) | — | Direct effectful producer `make_token(&mut count)` ensuring `result.ready == true` |
| Local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | A local `Seed { ready: true }` variable passes into `make_token(&seed)` |
| Field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | A struct field `holder.seed` carries the ready seed into `make_token(&holder.seed)` |
| Parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | A machine parameter `seed` under `requires seed.ready` passes into `make_token(&seed)` |
| Call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | A helper call `produced() -> Seed` ensuring `result.ready` yields the seed |
| Match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | Both match arms of `choose(flag)` return a seed ensuring `result.ready` |
| Const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | A named constant `NAMED: Seed = Seed { ready: true }` passes into `make_token(&seed)` |
| A nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | A successor machine `onward(seed: Seed)` receives the ready seed under `requires seed.ready` |
| Another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | A forwarding machine `forward() -> Seed` returns the ready seed across machine boundaries |
| Another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | Target-scoped machine bodies across four hosted targets pass the ready seed |

### Verification commands

Root `main.omg` and all nine place projects bind `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and declare `Expected exit: 0`. Run any positive directly:

```sh
omega run --both samples/cli/facts/result_cleanup_guarantees/main.omg
omega run --both samples/cli/facts/result_cleanup_guarantees/local/main.omg
omega run --both samples/cli/facts/result_cleanup_guarantees/field/main.omg
omega run --both samples/cli/facts/result_cleanup_guarantees/parameter/main.omg
omega run --both samples/cli/facts/result_cleanup_guarantees/call_result/main.omg
omega run --both samples/cli/facts/result_cleanup_guarantees/match_join/main.omg
omega run --both samples/cli/facts/result_cleanup_guarantees/const_name/main.omg
omega run --both samples/cli/facts/result_cleanup_guarantees/other_state/main.omg
omega run --both samples/cli/facts/result_cleanup_guarantees/other_machine/main.omg
omega run --both samples/cli/facts/result_cleanup_guarantees/target_body/main.omg
```

Each place has an `invalid/` twin whose transported `Seed` has `ready: false`, breaking the prerequisite required by `Token::drop`. The control must reject with `cannot prove automatic cleanup requires`. Check each twin with:

```sh
omega --check samples/cli/facts/result_cleanup_guarantees/<place>/invalid/main.omg
```

## Requirements and Evidence

Owning task: **CML4** (`TASKS.md`).
This row establishes that completed result guarantees survive evaluation contexts to satisfy automatic normal cleanup prerequisites on temporary values across all nine places.

- **Establishment:** The copyable record `Seed` establishes `seed.ready == true` through literal initialization, const binding, match join branches, machine call postcondition `ensures result.ready`, or transition entry requirement `requires seed.ready`. Machine `make_token(seed: &Seed) -> Token` requires a borrowed seed and guarantees postcondition `ensures result.ready == seed.ready` (or root `ensures result.ready == true`). The postcondition of `make_token` establishes `token.ready == true` on the returned temporary `Token` upon completion of the nested call in `read(&make_token(&seed))`, discharging the cleanup prerequisite `requires token.ready` on `Token::drop(&mut token)` when the temporary is dropped at the end of the statement without relying on post-hoc caller equality guards (`transition x == 100`).
- **Identity and lifetime:** The `Seed` record is a copyable value whose readiness observation (`ready == true`) has definite identity and value-level immutability across evaluation. The returned `Token` is an affine record (`ready: bool; value: u64;`) with exclusive temporary custody in the evaluating statement. Completed call value provenance tracks the exact returned value through evaluation, ensuring that guarantees attached to the completed `make_token` result belong directly to the temporary `Token` rather than unrelated storage or pre-store scratch buffers. Overlapping writes or earlier hook executions do not invalidate or confuse the surviving result fact.
- **Composition:** Completed result postconditions compose across expression evaluation and statement boundaries: `make_token(&seed)` substitutes actual arguments into the callee postcondition, joining the established guarantee `result.ready == true` into the evaluated expression context. In `read(&make_token(&seed))`, the temporary borrow `&token` is passed to `read` and retired before normal statement cleanup is invoked. At statement completion, normal cleanup joins the surviving result guarantee with the drop target's prerequisite. Across all nine places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`), `<place>/main.omg` and `<place>/invalid/main.omg` differ only in setting `ready: false` on the carried `Seed`, proving that the cleanup prerequisite discharge depends strictly on the transported fact.
- **Soundness boundaries:** All nine `<place>/invalid/main.omg` reject twins fail closed at compile time with diagnostic `cannot prove automatic cleanup requires`. When `seed.ready` is `false`, `make_token`'s guarantee yields `result.ready == false`, preventing the compiler from discharging `Token::drop`'s precondition `requires token.ready`. Soundness boundaries ensure that unproved cleanup prerequisites cannot execute or be bypassed, and that normal statement cleanup enforces nominal drop obligations prior to native realization.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Source checking lowers expressions and calls into canonical target-neutral Terminal Psi (`PSITERM\0`), materializing explicit affine cleanup actions (`InvokeNominal` for `Token::drop`, `RetireHome`, `DiscardResidual`) at the end of the containing statement while erasing proof-only obligation tags (`proof-term-tags`). The temporary borrow `&make_token(...)` enforces proper loan retirement windows before the nominal drop action `Token::drop(&mut token)` executes. The Terminal Psi interpreter executes normal cleanup actions and matches native execution (`omega run --both`, exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Build companion rows join `ProgramEntry` declarations across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) to the Terminal catalog without reopening source. Layout plans validate `Seed` (`size_of = 1`, align 1) and `Token` (`ready: bool` at byte offset 0, padding byte 1-7, `value: u64` at byte offset 8, `size_of = 16`, align 8). Target `CallPlan`/`StatePlan` lower argument passing (`&Seed` reference pointer in register `rcx`/`rdi`/`x0`) and aggregate return (`Token` in indirect `sret` home or packed register pairs `rax`/`rdx` or `x0`/`x1`) and allocate disjoint stack slots for temporary variables.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Target backend lowering selects target ISA instructions (`x86_64` / `arm64`), generates calls to `make_token`, `read`, and `Token::drop`, and manages register homes and temporary spill storage. Activation frame realization computes worst-case stack usage (WCSU), manages temporary lifetimes across nested calls, and ensures proper prologue and epilogue generation around nominal drop calls in compliance with the `StatePlan` ceiling.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Displacements for stack temporary allocations and basic blocks are encoded with short/near branch relaxation. Function call relocations to `make_token`, `read`, and `Token::drop` resolve cleanly. Transitive machine-state footprint evidence is validated, and direct PE/ELF/Mach-O executable binaries are emitted, achieving native execution agreement with Terminal Psi (`omega run --both`, exit 0) across all hosted targets.

The row is pinned by `tests/omega/{pass,fail}/facts/result_cleanup_guarantees_through_*` fixtures; every positive records native exit 0.
This row does not establish owned-input post-state transport or common Boolean result facts imported from a portable target-family head; those remain separate transport frontiers under `CML4` and `STATE-LOCAL-VALUE-FRONTIER`.
