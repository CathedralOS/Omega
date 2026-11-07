# Logic hazard timing (ADD08)

Offline synthetic electronics experiment in ordinary Omega. A private nine-node
DAG evaluates `a*b + !a*c`, alongside the consensus-protected
`a*b + !a*c + b*c`. An event calendar propagates declared gate delays, emits every
signal transition, detects completed low glitches, and exhaustively checks all
eight steady-state input combinations against direct Boolean evaluation.

This is distinct from `simulation/traffic_light` and `simulation/alarm_scheduler`:
those sources advance scripted phases/events; neither schedules gate propagation,
reconvergent paths, transport pulses, or cancellation of inertial pulses.
No active-worker code or in-progress numerics package is imported.

Read one mode byte from stdin (the remaining stream is unused):

| Byte | Output OR delay | Policy | Node 6 result |
| --- | --- | --- | --- |
| `0` | 1 tick | transport | low at 12, high at 15, width 3 |
| `1` | 4 ticks | transport | low at 15, high at 18, width 3 |
| `2` | 4 ticks | inertial | no output pulse |
| `d` | 0 ticks | invalid netlist control | rejection, exit 3 |

Unknown bytes and EOF reject with exit 2. Valid modes exit 0 only when all
truth checks, the entire waveform digest, output-change/glitch counts and final
outputs match the pre-execution oracle. Failure exits 1. Node 8 stays high for
all three modes; the consensus term bridges the reconvergent-path gap.

`model/netlist.omg` owns gate data, topology and Boolean evaluation. `main.omg`
owns input handling, validation, initialization, the private event calendar,
waveform emission and pulse observation. `data/*.input` are deterministic input
fixtures; `data/*.expected` are frozen exact stdout artifacts. `reference.py`
independently uses a heap and event generations, instead of the Omega calendar
and due-slot cancellation. Its truth oracle selects b or c directly by a.
Running it again compares to existing oracles and refuses any change.
The independent frozen wave digests are 58289, 52296 and 13462.

From the repository root, using a verified compiler binary:

```sh
OMEGA=/absolute/path/to/omega
$OMEGA update --project samples/cli/independent/logic_hazard_timing --target linux_x86_64
# Read and accept the generated local std/entry review, then:
$OMEGA update --resume --project samples/cli/independent/logic_hazard_timing
$OMEGA --check samples/cli/independent/logic_hazard_timing/main.omg
printf 0 | $OMEGA run samples/cli/independent/logic_hazard_timing/main.omg
printf 2 | $OMEGA run --both samples/cli/independent/logic_hazard_timing/main.omg
python3 samples/cli/independent/logic_hazard_timing/reference.py
python3 samples/cli/independent/logic_hazard_timing/validate.py --omega "$OMEGA"
```

Check `invalid/main.omg` directly; checking reports the obligation before
requiring an accepted deployable entry. Its complete separate entrypoint calls `positive_delay(0)` in violation of a
positive-delay precondition. Validation requires that intended diagnostic;
unrelated compilation refusal is not accepted as evidence.

The validator runs one Omega operation at a time, checks exact native stdout,
requires native/Terminal exit agreement and saves stdout, stderr, commands,
exit statuses, source/binary hashes, base, host and resource settings under
`build/validation/`. `--both` compares termination only in the current CLI;
the application checks its trace digest inside both runtimes. This digest is a
bounded regression check, not a collision-free equivalence proof.

Model limits: integer synthetic ticks, ideal two-valued gates, bounded DAG and
finite events, initial equilibrium, positive delays <=8, fixed falling-input
stimulus at t=10, ascending-node tie order, no analog threshold, metastability,
load/fanout physics or physical hardware claims. Transport preserves scheduled
edges; inertial replaces a gate's pending edge on each new desired value.
The calendar's 64 times x 10 nodes exceeds this fixture's horizon; the simulator
scans to t=31, after the last possible output event (t=19). Nodes are initialized
before injecting input, so startup settling is not counted as a glitch.
All numerical tolerances are exactly zero.

Assignment record: ADD08; namespace and isolated branch owned end to end;
acceptance is check/native/Terminal, frozen truth/trace outputs and independent
invalid controls; active-worker dependencies none. ADD08 was absent from the
board at assignment time, so no unrelated board item was claimed or modified.
The registry status request timed out with exit 3; ownership remains recorded
here while work proceeds in isolation.
