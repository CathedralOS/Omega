# Offline laboratory protocol simulator (IND22)

This application interprets stdin as a bounded command stream. It simulates one
vessel shared by actors 1 and 2. It performs console input/output only; it has
no equipment, network, filesystem or real-clock boundary.

Each line contains exactly an uppercase opcode and one ASCII digit. LF is the
separator; CR is an ordinary invalid byte. EOF commits a final nonempty line.
Blank, overlong, non-digit and unknown commands are rejected without changing
laboratory state. Overlong input is drained to LF with constant memory. The
65th command produces `BUDGET_EXHAUSTED` and exit 65; other streams exit 0.

| Command | Meaning |
| --- | --- |
| A1 / A2 | Acquire the unowned vessel and reset its elapsed ticks |
| L1 / L2 | Load the owned reserved vessel |
| S1 / S2 | Start the loaded vessel only with a closed lid |
| T1 .. T9 | Advance simulated time while running |
| F1 / F2 | Finish the owned running vessel and release its lease |
| C1 / C2 | Abort an owned reserved, loaded or running protocol |
| R1 / R2 | Recover an owned aborted vessel and release its lease |
| O0 / O1 | Open / close lid; opening while running aborts immediately |

A lease survives abort and timeout. Another actor cannot acquire, cancel,
finish or recover it. Recovery is explicit. Repeating acquisition is a resource
conflict; repeating load/start/cancel in the wrong phase is a phase rejection.
Time reaches the timeout at tick **5**, including a command that overshoots it;
the trace clips time to 5. Finish at tick 4 is accepted. Rejected commands are
atomic. Closing the lid after abort does not recover the protocol.

The initial owner and phase are zero, lid is open and counters are zero.
Phases are 00 idle, 01 reserved, 02 loaded, 03 running, 04 aborted, 05 completed.
Each trace row names the decision, interlock outcome where applicable, lease,
phase, lid, elapsed ticks, successful completions and aborts. Two-digit counters
are sufficient for the 64-command application bound. `??` is a visible report
invariant violation and can never match a frozen oracle.

The entrypoint and input framing live in `main.omg`; `protocol/model.omg` owns
the full protocol graph; `protocol/report.omg` owns trace rendering. The only
dependency is the current bundled std Console and ProgramEntry provider. The
application uses no private compiler hooks or another worker's numerics code.

Semantic comparison: `simulation/traffic_light` only cycles phases;
`simulation/elevator` gates movement on its door; `games/vending_machine` folds
coin events; `systems/account_ledger` dispatches transactions and frozen account
gates; `systems/wire_protocol` encodes packets. This sample adds a command
interpreter with retained abort leases, competing actors, recovery, simulated
deadlines, immediate lid-open aborts and exact per-command reporting.

`fixtures/*.txt` are immutable input bytes; `.stdout` and `.exit` are the
independently frozen expected results. `reference.py` uses a transition table
rather than the Omega state graph. It verifies its calculations against those
files by default; `--freeze` is an explicit authoring action, never validation.
The manifest was frozen before the first Omega execution. All arithmetic is
integer and all comparisons are exact: tolerance **0**. Nine fixtures cover
nominal, cancellation, repeated commands, timeout, conflict/interlocks, invalid
runtime input, final unterminated input, hostile framing and budget exhaustion.
The separate `invalid/` directory is a complete intended-invalid application
that attempts actor 2's release of actor 1's lease without proving the declared
precondition. Runtime input rejection is distinct from this source rejection.

From the repository root, with a compiler built from this base and its matching
library selected:

```sh
export OMEGA_LIBRARY_ROOT="$PWD/source/library"
OMEGA=./target/release/omega
$OMEGA update --project samples/cli/independent/laboratory_protocol --target linux_x86_64
# Inspect the generated review, accept the local std entry binding, then:
$OMEGA update --resume --project samples/cli/independent/laboratory_protocol
$OMEGA --check --target linux_x86_64 samples/cli/independent/laboratory_protocol/main.omg
$OMEGA run --both --keep samples/cli/independent/laboratory_protocol/main.omg < samples/cli/independent/laboratory_protocol/fixtures/nominal.txt
python3 samples/cli/independent/laboratory_protocol/reference.py
python3 samples/cli/independent/laboratory_protocol/validate.py --omega "$OMEGA"
```

The public `run --both` route replays the same finite stdin for Terminal and
native execution. Its comparison checks exit status; the forwarded stdout is
native. Validation checks all native trace bytes against the frozen oracle and
requires explicit Terminal exit agreement. It does not claim that this CLI
verifies Terminal stdout equality.

Validation records and generated artifacts belong in ignored `build/`.
The candidate's validation record identifies exact binary hash, base, host,
resource settings, commands and observed results. Run only one build and one
Omega execution at a time on this executor. Other targets are bound using
existing std providers, but only actually executed target evidence is claimed.
