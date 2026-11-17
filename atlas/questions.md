# Open questions

Questions the evidence cannot settle and the rulings do not answer yet. Each
says what the code does today, why it matters, and the options. The full
evidence is in [plans/foundation-and-backend.md](plans/foundation-and-backend.md)
§7; the letters match it.

## Realization: what it is, and where it belongs (Q-G)

**What it does.** Terminal Psi says *what* a program needs from outside
itself: "call boundary requirement `Console::write`", "this program's entry
is `ProgramStorageEntry::enter`". It does not say *who* provides those on a
given target. Realization answers that for one selected target:

| Crate | Lines | Job |
| --- | --- | --- |
| `provider-planning` | 7,230 | Chooses a provider for each boundary requirement the program reaches, from the Build's bindings and selections (for example: on `linux_x86_64`, `Console` is `LinuxX86Console`, and `LinuxX86ConsoleIo::write` is system call 1), and runs calling policies to get each call's ABI plan. |
| `provider-admission` | 26,491 | Checks each chosen provider against what the requirement promises (signature, ABI, placement, installation evidence) and seals the plans. Only admission can construct the sealed records. |
| `provider-selection` | 1,874 | Binds selected instances, bodies and boundary applications to the Terminal declarations. |
| `trust-model` | 708 | Rebuilds the program's trust obligations from Terminal declarations, root grants and provider evidence. |
| `selected-dispatch` | 105 | `SelectedExecution::settle`: the only way to make "this program, for this target, with these settled providers". |

Line counts: all `.rs` under each crate, at the atlas evidence revision.

**Why it is a question.** Its input is Terminal Psi (pipeline output) plus the
target and the Build's bindings; its output (provider plans, installations,
task plans) is what Omega 00, 02 and 03 read. That is exactly the shape of a
stage, but today it runs beside the pipeline and Omega stages read its results
through backend crates (`plans/*`, `runtime/*`).

**Options.**
- (a) Realization stays a separate producer, like Build, and its output is an
  input representation (`provider-plans`) that Omega 00 receives. This is what
  the plan does for now.
- (b) Realization becomes a numbered step between Terminal Psi and Omega 00
  ("Terminal Psi + target → selected program"), and its output is Omega 00's
  input representation.

**Recommendation:** (b). It reads pipeline output and writes pipeline input,
so under ruling 1 it is a stage.

## Still open from the plan

- **Q-A. Where does the shared process runner go?** The native-provider
  runtime, package resolvers, the CLI and three dev tools all start child
  processes through `bounded-process`. The runtime's dependency check stops it
  from using a crate under `packages/`.
- **Q-B. May Terminal Psi and the native compiler read Build inputs and loaded
  source data directly**, or does Terminal Psi copy the few fields it needs?
- **Q-C. Is Build-time evaluation a second front-end driver?** It calls Psi
  stages 02–04 directly instead of going through `source-compiler`.
- **Q-D. May a representation depend on any earlier form, or only the one just
  before it?** Every native form uses Terminal Psi identities directly today.
- **Q-F. Who encodes branches after stage 07?** Stages 08, 09 and the proposed
  frame-insertion stage all re-encode branches.
