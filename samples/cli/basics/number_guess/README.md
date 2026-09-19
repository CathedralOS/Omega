# number_guess

Scripted binary-search guessing game. The target is hardcoded; stdin is used
only for the final pause, which also completes on EOF.

## What it demonstrates

- Integer division (`/`) in a `let`-binding: `mid = (lo + hi) / 2`
- `[copy]` data type holding the search state (`lo`, `hi`, `mid`, `guesses`)
- Sequential calls to the same sub-machine (`step`) that each mutate `lo` or `hi`
- Reading fields updated by the PREVIOUS sub-machine call at the top of the CURRENT call
- Nested Boolean transitions inside `step`: first `== target`, then `< target`
- Guard ladder in `main` verifying `found == 1` and `guesses == 7`

## Binary search trace

Secret number: **42**, range: **[1, 100]**

| Guess | mid | Result     | lo | hi  |
|-------|-----|------------|----|-----|
| 1     | 50  | > 42, hi-- | 1  | 49  |
| 2     | 25  | < 42, lo++ | 26 | 49  |
| 3     | 37  | < 42, lo++ | 38 | 49  |
| 4     | 43  | > 42, hi-- | 38 | 42  |
| 5     | 40  | < 42, lo++ | 41 | 42  |
| 6     | 41  | < 42, lo++ | 42 | 42  |
| 7     | 42  | == 42, ✓   | —  | —   |

7 guesses to find 42.

## Expected exit code

**70** — `found == 1` (target found) and `guesses == 7` both verify correct.

Expected stdout (each line ends in a newline):

```text
number_guess: binary-searching [1,100] for the secret number 42
PASS: found 42 in exactly 7 guesses (exit 70)
[press Enter to close]
```

## Review the project

From the repository root, request review for the host where you will run it:

```text
cargo run -p omega -- update --project samples/cli/basics/number_guess --target macos_arm64
```

Use `windows_x86_64`, `linux_x86_64`, or `linux_arm64` for those hosts.
Exit 3 means decisions are pending, not that acceptance was published. Inspect
the reported `build/package-manager/review-<target>.txt`, change each decision
from `pending` to `accept` or `reject`, then resume:

```text
cargo run -p omega -- update --resume --project samples/cli/basics/number_guess
```

Resume must succeed before running. Local-path source identities bind the
checkout; another worktree's lock is not reusable approval. Review the generated
findings rather than copying decisions from this example. Package acceptance
does not bypass proof checking or supply a receiving policy; see
[package acceptance](../../../../wiki/spec/packages/acceptance.md).
Use `mbx` in place of `cargo` when available.

## Execute on the host

On macOS ARM64 or Linux:

```sh
guess_exit=0
cargo run -p omega -- run samples/cli/basics/number_guess/main.omg || guess_exit=$?
test "$guess_exit" -eq 70
```

On Windows PowerShell:

```powershell
cargo run -p omega -- run samples/cli/basics/number_guess/main.omg
if ($LASTEXITCODE -ne 70) { throw 'Expected exit 70' }
```

`run` compiles, publishes, and executes the receipt-bound artifact; do not guess
an executable filename. It captures the program's output and closes its stdin,
so the final pause completes on EOF. Expect the stdout above and
`native exit: 70` on stderr. Do not add `--target` to this execution command:
an explicit target makes `run` compile-only.

The ordinary review/resume/run route passed on macOS ARM64 at `ba57b10d5e`.
Windows x86-64 and both Linux hosts remain unverified; their required runtime
coverage stays in [SAMPLE-CORPUS](../../../../TASKS.md).
