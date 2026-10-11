# Finite horizon controller (IND07)

This is an offline discrete simulation. The Omega entrypoint reads one selector
`0` through `7` followed by LF, loads a problem, validates its limits, enumerates
all action sequences, and emits the optimal actions, complete state trajectory,
integer cost and rejection counts. Exit 0 means optimal; 2 means invalid input
or configuration; 3 means infeasible. No external device is controlled.

The model is `x[t+1] = x[t] + u[t] + drift`. Every action and every state,
including the initial state, must satisfy their inclusive configured bounds.
The cost is the sum of `state_weight*(x[t+1]-target)^2 + action_weight*u[t]^2`
plus `terminal_weight*(x[H]-target)^2`. Optional terminal equality is a hard
constraint. Horizons are 1..5, states/target -4..4, action bounds within -1..1,
drift -1..1 and weights 0..9. At most 243 sequences and 1215 simulated steps
are needed. Accepted costs are at most 3501, so explicit Wrapping carriers
never overflow. Arithmetic and oracle tolerances are exactly zero.

The private search enumerates base-three words in lexicographic action order
(-1, 0, 1), retaining a candidate only on strict cost improvement. It owns all
optimization and storage; it imports only stable std console facilities.
`controller/model.omg` owns the model and limits, `controller/search.omg` owns
exhaustive optimization, `data/fixtures.omg` owns scenarios and `io/render.omg`
formats computed results. Build metadata binds `Main::main` explicitly.

Compared against existing samples: brightness_control validates direct commands,
elevator dispatches scripted commands, maze_flood performs BFS, and bubble_sort
sorts values. None performs horizon optimization with stage/terminal costs,
restricted actions, disturbances, state feasibility and deterministic ties.

| Selector | Problem | Frozen result |
| --- | --- | --- |
| 0 | tracking | cost 8, actions -1,-1,-1,0 |
| 1 | target beyond state ceiling | cost 13, actions 1,1,0,0 |
| 2 | all costs zero | cost 0, lexicographic actions -1,-1 |
| 3 | exact unreachable terminal | 9 terminal rejections, infeasible |
| 4 | drift with restricted actions | cost 12, actions 0,-1,-1,-1,-1 |
| 5 | no possible first transition | 27 boundary rejections, infeasible |
| 6 | horizon 6 | invalid configuration |
| 7 | reversed bounds | invalid configuration |

`data/cases.json` and `reference.py` independently enumerate Cartesian products
using Python integer arithmetic. Expected stdout and exits in `data/expected/`
and `data/oracle.json` were frozen before Omega execution. The reference is a
review oracle, not the application. Do not regenerate expected output to make a
compiler run pass. Blank input, EOF, malformed and unknown selectors are also
fixtures. `invalid/` is a separate complete program that must fail compilation
because it calls a 1..5 horizon contract with 6.

From the repository root, with a verified Omega binary and matching library:

```text
omega update --project samples/cli/independent/finite_horizon_controller/reproduction --target linux_x86_64
omega update --resume --project samples/cli/independent/finite_horizon_controller/reproduction
omega update --project samples/cli/independent/finite_horizon_controller --target linux_x86_64
omega update --resume --project samples/cli/independent/finite_horizon_controller
omega --check samples/cli/independent/finite_horizon_controller/main.omg
omega --check samples/cli/independent/finite_horizon_controller/invalid/main.omg
omega run --both samples/cli/independent/finite_horizon_controller/main.omg < samples/cli/independent/finite_horizon_controller/data/0.stdin
python3 samples/cli/independent/finite_horizon_controller/validate.py --omega /absolute/path/to/omega
```

Refresh the nested positive control first and the controller last: the nested
control's generated accepted lock is part of the parent source snapshot.
Inspect and accept the generated package review before each required resume. The invalid
control must return nonzero with a structurally false horizon requirement. Windows can
use `python` and `omega.exe`; the validation script supplies stdin directly and
needs no shell redirection. Other targets have build bindings but are untested
unless named in the evidence. Generated output belongs in ignored `build/`.
Validation builds once and executes the retained native artifact for all 12
inputs, recording its SHA-256. It runs one Omega process at a time: 16 Omega
invocations and 12 direct native executions make 28 sequential commands.
It captures every command, stdout,
stderr and exit under `build/validation/`, checks byte-exact native output and
recomputes every emitted transition and cost independently. `--both` interprets
the produced Terminal artifact; the current CLI compares termination only, so
Terminal agreement here is not a claim of Terminal stdout equivalence.

The complete positive `reproduction/` preserves the local fixed-byte input loan
that exposed the compiler gap. Its input `0` plus LF must exit 70 with empty
stdout in native execution and Terminal. The same control is retained as
`tests/omega/pass/collections/local_byte_array_input_state_loan_exit`.
