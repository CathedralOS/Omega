# Hidden Markov inference

A two-state hidden Markov model with binary emissions and 1–16 observations.
The application computes the observation log-likelihood, scaled forward/backward
posterior probabilities, and an exact Viterbi path. Equal-weight paths select the
lexicographically smallest sequence (0 precedes 1). Viterbi compares bounded
base-million integer numerators, so roundoff cannot create or break a tie.

Input consists of ten LF-terminated lines of seven decimal digits, then an
LF-terminated string of 1–16 binary digits and EOF. Probabilities are integer
millionths from `0000000` to `1000000`. Lines give initial probabilities p0,p1,
transition entries t00,t01,t10,t11, and emission entries e00,e01,e10,e11. Each
pair must sum exactly to 1000000. This protocol deliberately refuses arbitrary
precision decimals, signs, whitespace, CRLF, partial lines and trailing data.
A 64-byte line buffer bounds every read.

The printed path is filled through a match of two runtime-length windows, kept in
a record field, and written by a call. Output begins `OK trillionths`. `LOG value` and each `POSTindex p0 p1` use signed
integer trillionths (truncate toward zero); `PATH bits` gives the hidden states.
All output is numeric and independently checked to absolute error at most 1e-9.
An impossible observation prints `IMPOSSIBLE` and exits 65, with no fabricated
posterior or path. Malformed input prints `INVALID` and exits 64. Success exits 0.

From the repository root, prepare packages with ordinary review first:

```
omega update --project samples/cli/independent/hidden_markov_inference --target linux_x86_64
omega run samples/cli/independent/hidden_markov_inference/main.omg < samples/cli/independent/hidden_markov_inference/data/standard.stdin
```

`reference.py` exhaustively enumerates every hidden path with 90-digit Decimal
arithmetic. `expected.json` and the input digests were frozen before executing
the implementation. Validation must consume that file, never regenerate it.
The 16-step cases enumerate all 65536 paths. State permutation, exact ambiguity,
determinism, zero transitions, tiny likelihood and impossible observations are
covered, alongside runtime and source-invalid controls.

For full qualification, build the private observation consumer from the root
workspace, using the repository's pinned toolchain:

```
CARGO_PROFILE_DEV_OPT_LEVEL=1 mbx build -p omega -p hmm-terminal-validation
target/debug/hmm-terminal-validation --review samples/cli/independent/hidden_markov_inference
```

Inspect any ordinary package review and resolve only the selected program's
actual authority choices. Source-risk disclosures alone grant no host access.
Resume a reviewed candidate with `--resume-review` using the same directory.
After preparation, capture both products and validate their numerical output:

```
target/debug/hmm-terminal-validation samples/cli/independent/hidden_markov_inference samples/cli/independent/hidden_markov_inference/build/terminal
python3 samples/cli/independent/hidden_markov_inference/validate.py --native /absolute/path/from/native-path.txt --terminal samples/cli/independent/hidden_markov_inference/build/terminal --producer target/debug/hmm-terminal-validation
```

The consumer uses the ordinary accepted package/native driver, retains its exact
Terminal artifact and runtime provisions, and observes each bounded input with
an isolated runtime. The receipt binds source, input, output, Terminal artifact,
producer and native-executable digests. Validation refuses Python optimization
that disables assertions and verifies the supplied producer executable digest. No expected-result execution callback,
source replay or compiler identity override supplies an oracle result.

The private Rust consumer is a root-workspace member so normal builds reuse the
compiler dependency closure; building it as an independent workspace obscures
cache paths and recompiles the same libraries. It is not a shipped CLI command.
Check the source-invalid control separately with `omega --check
samples/cli/independent/hidden_markov_inference/controls/invalid_transition/main.omg`;
its unguarded increment cannot prove the destination's bound of sixteen.
