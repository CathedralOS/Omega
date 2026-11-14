# Match effectful subject custody

A match subject call installs a fresh token in the slot it borrows, the selected
arm's branch-local call takes custody of that installed token or of the other
token, and every token the arms leave behind is cleaned up exactly once.

Parent: **MATCH-SELECTIVE-LOWERING** in [TASKS.md](../../../../TASKS.md), the
affine-custody requirement: branch-local calls, effectful subjects and
heterogeneous residual custody. The arms' calls take custody, the subject call
has an effect on storage an arm consumes, and each path leaves a different
residual token. This row establishes those clauses only.

`Token` is affine: it holds `&mut Counter` and its drop hook adds its `mark`
to that counter. Each program owns `slot` (mark 11, counter `a`) and `other`
(mark 13, counter `b`). The subject `refill(&mut slot, c, flag)` replaces
`slot` with a token over counter `c` (mark 5), dropping the original. The
`true` arm passes `slot` to `relay`, which sets the mark to 40; the `false`
arm relays `other`. `main()` runs both flags. `true` expects `a == 11`,
`b == 13`, `c == 40`; `false` expects `a == 11`, `b == 40`, `c == 5`. A wrong
arm, a missed or doubled drop of the residual token, or a drop of the token
the arm took traps.

## Places

| Place | Positive | Reject twin | Transport |
| --- | --- | --- | --- |
| local | [main](local/main.omg) | [invalid](invalid_local/main.omg) | both tokens are locals of `exercise` |
| field | [main](field/main.omg) | [invalid](invalid_field/main.omg) | both tokens are fields of a `Pair` record |
| parameter | [main](parameter/main.omg) | [invalid](invalid_parameter/main.omg) | both tokens arrive as owned parameters of `apply` |
| call result | [main](call_result/main.omg) | [invalid](invalid_call_result/main.omg) | both tokens are produced by `make` calls |
| match join | [main](match_join/main.omg) | [invalid](invalid_match_join/main.omg) | both tokens are produced by `match true` joins |
| const name | [main](const_name/main.omg) | [invalid](invalid_const_name/main.omg) | the token marks are the constants `SLOT_MARK` and `OTHER_MARK` |
| other state | [main](other_state/main.omg) | [invalid](invalid_other_state/main.omg) | the nested machine `pick` performs the match over tokens its parent built |
| other machine | [main](other_machine/main.omg) | [invalid](invalid_other_machine/main.omg) | `forward` passes the tokens to the top-level `apply` |
| target body | [main](target_body/main.omg) | [invalid](invalid_target_body/main.omg) | Build selects `X86Apply` or `ArmApply` for `SelectedApply::exercise`; each checks its target tag (17 / 29) first |

Pinned fixtures: `tests/omega/pass/facts/match_effectful_subject_custody_through_<place>_exit`
and `tests/omega/fail/facts/match_effectful_subject_custody_through_<place>_rejected`.

Each twin reads `slot.mark` after the match. The `true` arm transferred `slot`,
so the read rejects with ``after `slot` was transferred or consumed``
(``pair.slot`` in the field place).

## Repair

On base `5445b5af611` every place was refused with ``creates local borrow
`slot` while local borrow `held` is still active``. The same program with the
subject hoisted into its own `let` passed. The statement-completion roster row
for the refilled slot (zero formation expression) was checked against every
loan formed in the statement, including `held`, which the arm formed later by
moving that same descriptor out of `slot`. Split across two statements, the
same pair is judged in evaluation order: `held` is derived from the slot's loan.

`05_typed-trees-to-checked-trees/src/checks_borrows.rs` now judges
a roster row against an operand loan of the same statement in that order: the
operand loan is the forming loan and the roster row its supplier, through the
existing `supplier_preserves_loan` rule.

The control
[`tests/omega/fail/borrows/match_effectful_subject_fresh_overlap_rejected`](../../../../tests/omega/fail/borrows/match_effectful_subject_fresh_overlap_rejected/main.omg)
forms a new token over `c` in an arm while the refilled slot still holds `c`.
It still rejects, inline and with the subject hoisted.

Later, native lowering refused every place with ``callee stored-reference
effects are unresolved``. Inside `refill`, the old token is moved out of `slot`
for its drop hook before the new token is stored, so for one step the slot's
reference field holds no known value. The descriptor replay that summarizes a
helper's reference writes treated that vacant leaf as an unknown ancestor and
refused the store, which marked `refill` as clobbering unknown memory.
In `terminal-psi/semantics/.../replay/replay.rs`, finding where a store goes
(`contents_address_invalidated`) now looks only at strict ancestors of the
leaf. Every read keeps the leaf-inclusive check (`contents_path_invalidated`),
so a read of the vacant leaf is still unknown.

## Requirements and evidence

| Requirement | How this row establishes it |
| --- | --- |
| Establishment | The subject call installs the token; the arm's custody comes from that completed call, not from a caller guard. |
| Identity and lifetime | The installed token, not the original, reaches the arm (`c` sees 40 or 5; `a` sees only the replaced original's 11). |
| Composition | Each arm consumes a different token; the residual set differs per path and each residual is dropped once. |
| Soundness boundaries | A read of the transferred slot rejects; an overlapping fresh borrow of `c` still rejects. |
| Psi 06–08 | The interpreter agrees with native execution on both flags. |
| Omega 00–09 | macOS AArch64 native images run the drop hooks on the residual tokens only. |

Evidence, 2026-11-01, host macOS AArch64: candidate compiler built from base
`5445b5af611` plus the change above. `omega run --both` exits 0 with native and
interpreter agreeing for all nine positives; `omega --check` rejects all nine
twins with the fragment above. Evidence, 2026-11-03, host Linux x86-64: base `3e22616789c` plus the replay
repair. `corpus_gate.py --both` builds all nine positives, native exit 0 with
the interpreter agreeing; all nine twins reject with the fragment above.
Windows native execution is not observed.

## Open

Branch-local receivers and claim-bearing body plans remain on the board.
