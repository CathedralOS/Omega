# Clearance route planner (ADD11)

A complete bounded hoist-clearance route planner in Omega. Inputs and the
scientific model are synthetic: a hoist carries a load through four consecutive
spans, each admitting an integer operating height (decimeters) inside
`[floor, ceiling]`. The rigging adjusts at most four height units between
adjacent spans, and the final span must deliver the required end height. This
is a small exact planning demonstration, not a lifting-safety recommendation.

The private search enumerates all 10,000 four-span height words and admits each
obeying the per-span clearance bounds, the four-unit step limit, and the end
height. The objective minimizes total vertical travel; every profile tied on
that objective is counted, and the lexicographically smallest optimal profile
is emitted. The output includes the feasible count, optimal ties, minimum
travel, and the chosen span heights.

## Input and output

Provide one ten-digit ASCII record, optionally followed by LF:
`H0 HE F1 C1 F2 C2 F3 C3 F4 C4` with no spaces. `H0`/`HE` are start/required-end
heights 0..9; each span's floor `F` and ceiling `C` are 0..9 and must satisfy
floor <= ceiling. Invalid syntax or bounds exit 64; a valid but infeasible
model exits 2; an optimum exits 0. The bounded reader consumes the first
record; it accepts EOF after ten digits, preserves LF, and rejects CRLF, short
lines and overlong first lines.

For `3309090909`, start/end 3 with all spans 0..9 gives 373 feasible profiles,
one optimum at travel 000: profile 3,3,3,3. For `0209090909`, start 0 end 2
over open spans gives 210 feasible profiles and 10 optimal ties at travel 002;
the lexicographic optimum is 0,0,0,2. Every numeric output field is fixed-width
decimal; [data/frozen_oracles.json](data/frozen_oracles.json) records exact
expected bytes.

## Structure and independence

`main.omg` owns input and the entrypoint; `build.omg` binds exact target entry
slots. `input/parse.omg`, `clearance/{model,search}.omg`, and
`reporting/certificate.omg` own the whole workflow. The only dependency is the
stable repository std package and existing Console providers. There are no
numerics-library or worker-candidate imports and no shared exports.

Existing samples were searched for clearance, route, profile and hoist
semantics. `sample_collection_routing` optimizes timed pickup/drop tours on a
line; `transport_planner` solves a transportation network; neither plans
height profiles under per-span clearance and step limits. This application
adds exhaustive constrained profile planning and runtime input, rather than
renamed constants or a constant-expression witness.

## Provenance

This is a reconstruction, not recovered source. The original ADD11
candidate, its 22 oracles and its review archive are absent from every
reachable Git ref and the Library download route returns HTTP 401; only the
name, the oracle count, and the completion obligations survived in the
handoff. The model above is authored fresh against repository contracts in
the established independent-sample shape, marked here as new provenance; the
22 fixtures below are a new frozen suite, not recovered bytes.

## Frozen validation

The independent Python oracle searches the per-span allowed ranges by forward
product scan, then takes the minimum travel cost, counts tied profiles, and
selects their lexicographic minimum. This differs from Omega's 10,000-word
filter. The oracle and 22 fixtures were frozen before any Omega
run. Absolute/relative tolerances are both zero. Inputs cover flat corridors,
forced dips and rises, step-limited ascents and descents, tied optima, narrow
bounds, exact/infeasible ends, EOF, and malformed records.
`validate.py` checks fixture hashes, exact stdout, stderr and exit codes and writes
actual stdout/stderr under ignored `build/validation-{native,both,terminal}`.
It never accepts packages or regenerates expected data. `--route terminal`
uses the public Terminal runtime through the linked harness in `validation/`
and compares real stdout/stderr/exit, not only termination.
Program stderr must be empty. When executing through `omega run`, validation
also requires the CLI's exact native exit report, plus its interpreter exit
report for `--both`; unexpected stderr fails validation.

The separate [intended-invalid source](controls/intended_invalid/main.omg)
is a complete dependency-free package source that writes index four of a
four-slot carrier. The documented rule is
[collection bounds](../../../../wiki/spec/language/numeric_values.md#collection-bounds).
Its expected rejection is `cannot prove index `4` is within length 4`.
A package-review, parser, or other rejection is not an invalid-control pass.

## Reproduce through ordinary facilities

From the repository root, set `OMEGA_LIBRARY_ROOT` to that checkout's
`source/library`, and use the compiler binary built from that revision. Build
one Rust compiler and execute one Omega command at a time. Substitute the
verified absolute binary path for `$omega_binary` below.

```sh
sample=samples/cli/independent/clearance_route_planner
"$omega_binary" update --project "$sample" --target linux_x86_64
# Inspect the generated review and its findings; edit each pending decision.
# Resume only an accepted review, and never retry a denied acceptance.
"$omega_binary" update --resume --project "$sample"
"$omega_binary" --check --target linux_x86_64 --build-dir "$sample/build/check" "$sample/main.omg"
python3 "$sample/validate.py" --omega "$omega_binary" --route native
# Or compile once and pass the exact artifact path from the compiler report:
# python3 "$sample/validate.py" --executable "$reported_artifact" --route native
python3 "$sample/validate.py" --omega "$omega_binary" --route both
python3 "$sample/validation/build_terminal.py" --omega "$omega_binary" --output "$sample/build/terminal-harness"
python3 "$sample/validate.py" --route terminal --terminal-harness "$sample/build/terminal-harness"
"$omega_binary" --check --target linux_x86_64 --build-dir "$sample/build/invalid" "$sample/controls/intended_invalid/main.omg"
```

The control has no external package review; a lock from another worktree is not
acceptance for the valid application. macOS ARM64 and Windows x86-64 have
declared entry bindings; their execution is not implied by Linux evidence.
Generated output and local lock acceptance are excluded from the candidate;
review on another executor must establish its own package acceptance.
