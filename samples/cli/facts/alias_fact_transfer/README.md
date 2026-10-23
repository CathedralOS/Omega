# Alias Fact Transfer

An entry `requires` on `main.player` transfers through a bare `Player` alias,
and that alias carries `Alive` into `inspect`'s `requires`.
No runtime domain checks or narrowing assertions are inserted. The root witness and nine place
witnesses use canonical `requires`, `domain`, `where`, and established flow facts.

Every positive place (and the root `main.omg`) binds `ProgramEntry` for all four hosted targets
(`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`), declares `Expected exit: 0`,
and carries an authored `expected_native_exit.txt` containing `0` beside its entry. Both engines
(`omega run --both`) agree: Terminal Psi and native execution report exit 0.

## Places

| Place | Positive Program | Fact-sensitive Reject Twin | Carrier / Transport Mechanism |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) | — | root witness transferring entry fact on `main.player` through bare `Player` alias |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | `let mut copy: Player = main.player` carries `Alive` into `inspect(copy)` |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the alias stored in `Roster::member` still carries `Alive` into `inspect` |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | parameter alias `player: Player` carries `Alive` into callee body `inspect(player)` |
| call_result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | call result alias `pick(main.player)` carries `Alive` onto bare local `copy` |
| match_join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms return an alias carrying `Alive`, so the joined alias does |
| const_name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | const bound `main.player == BASE_PLAYER` carries `Alive` into `inspect(copy)` |
| other_state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | state transition parameter `arrived(copy)` receives the alias and its `Alive` fact |
| other_machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | helper machine `produce(main.player)` returns alias carrying `Alive` to `consume` |
| target_body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | every target-scoped body uses the transferred `Alive` fact across all hosted targets |

## Execution

Every positive place has an authored native exit oracle of zero and exercises an interior
case (`health: 42`) and the minimum boundary case (`health: 1`). A wrong result crashes `Trap`.
Run with `omega run --both`:

```sh
omega run --both samples/cli/facts/alias_fact_transfer/main.omg
omega run --both samples/cli/facts/alias_fact_transfer/local/main.omg
omega run --both samples/cli/facts/alias_fact_transfer/field/main.omg
omega run --both samples/cli/facts/alias_fact_transfer/parameter/main.omg
omega run --both samples/cli/facts/alias_fact_transfer/call_result/main.omg
omega run --both samples/cli/facts/alias_fact_transfer/match_join/main.omg
omega run --both samples/cli/facts/alias_fact_transfer/const_name/main.omg
omega run --both samples/cli/facts/alias_fact_transfer/other_state/main.omg
omega run --both samples/cli/facts/alias_fact_transfer/other_machine/main.omg
omega run --both samples/cli/facts/alias_fact_transfer/target_body/main.omg
```

All 10 samples report `native exit 0` and `interp exit: 0`.

Each place carries a fact-sensitive reject twin at `<place>/invalid/` (mirrored in `<place>/controls/invalid_alias/`)
that omits the `main.player in Player::Alive` entry contract, so the unproved alias cannot satisfy
`Main::inspect`'s `requires player in Player::Alive`. All 9 reject twins fail closed at proof checking
before native execution:

```sh
omega --check samples/cli/facts/alias_fact_transfer/<place>/invalid/main.omg
```

Every twin rejects with `cannot prove requires contract for call inspect` (or `call arrived` / `call consume`).

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` / `CANONICALIZE-SCALAR-RANGE-CONTRACTS` (`TASKS.md`).
- **Requirement established:** End-to-end nine-place fact preservation and domain contract alias transfer. An established domain fact on record storage (`main.player in Player::Alive`) transfers to a copied scalar/record value alias and satisfies downstream call contracts naming that alias across all nine program places, with exact agreement between Terminal Psi and native execution across hosted targets.
- **Pinned fixtures:** positive places and reject twins under `tests/omega/pass/facts/alias_fact_transfer_through_*_exit` and `tests/omega/fail/facts/alias_fact_transfer_through_*_rejected`.
- **Authority:** [Domains and specifications](../../../../wiki/spec/language/domains.md) (domain membership as proposition), [state contracts](../../../../wiki/spec/language/state_contracts.md#bindings-and-arrivals) (all-predecessor joins and contract substitution), and [ownership and loans](../../../../wiki/spec/terminal-psi/ownership.md) (copyable value transfer and alias fact lifetime).

Full 8-layer specification and lowering evidence:

- **Establishment:** The entry contract `requires main.player in Player::Alive` (where `domain Player::Alive(player) requires player.health > 0`) establishes domain membership directly on the exact subject without any post-hoc caller equality guard (`transition x == 100`). Both interior cases (`health: 42`) and boundary cases (`health: 1`) satisfy the entry precondition on valid callers.
- **Identity and lifetime:** The domain fact `Player::Alive` remains attached to the exact value version across alias creation and storage transport: local copy (`local`), record field projection (`field`), explicit parameter passing (`parameter`), completed call result (`call_result`), match convergence join (`match_join`), named const bound equality (`const_name`), state parameter transition (`other_state`), forwarding machine call (`other_machine`), and target-scoped body candidates (`target_body`). No mutation, alias clobbering, or invalidating write intervenes between fact establishment and consumption.
- **Composition:** The aliased `Player` copy preserves `player in Player::Alive` and satisfies `Main::inspect(copy)`'s precondition across all nine places. Calls substitute exact actuals, match joins retain the domain fact justified on all predecessor paths, and state transitions prove arrival obligations on incoming edges. Each `<place>/main.omg` vs. `<place>/invalid/main.omg` differs only in omitting the entry contract `requires main.player in Player::Alive`, so the alias receives an unproved player and fails closed.
- **Soundness boundaries:** All nine `<place>/invalid/main.omg` reject twins fail closed with `cannot prove requires contract for call inspect` (or `call arrived` / `call consume`) naming that place's unproved alias operand. The checker strictly refuses to transfer unproved domain facts to aliases, preventing invalid programs from reaching Terminal Psi or native code emission.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Checked domain predicates (`Player::Alive`) and contracts erase cleanly after verification while the underlying `Player` data structures, alias copies, and machine calls lower without loss into canonical target-neutral Terminal Psi (`PSITERM\0`). The resulting programs execute identically in the Terminal Psi interpreter (`omega run --both`), observing exact health values 42 and 1.
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Companion build rows bind `ProgramEntry` across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Concrete layout geometry for `Player` (4 bytes, 4-byte alignment) and `Main` (8 bytes, 4-byte alignment) records resolves offset and size plans. `CallPlan` and `StatePlan` place scalar `health: i32` and `Player` copy arguments in target ABI registers (`rcx`/`rdx` on Windows, `rdi`/`rsi` on Linux, `x0`/`x1` on ARM64) and manage indirect mutator references (`&mut Main`).
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA instructions for 32-bit integer moves and comparisons, assigns register homes and disjoint stack-frame activation storage, and inserts entry prologues and multi-return epilogues.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic stack-frame displacements and entry call targets resolve, basic blocks and branch offsets relax with offset reflow, machine-state footprint evidence validates, and direct PE/ELF/Mach-O emission agrees with Terminal Psi (`omega run --both` exit 0) across `windows_x86_64` and `macos_arm64`.

## What remains open

Mutable borrow alias fact invalidation under exclusive write projections (`&mut`), and cross-package domain contract re-export proofs remain governed under the wider `CONSERVATION-CONTRACT` roadmap.
