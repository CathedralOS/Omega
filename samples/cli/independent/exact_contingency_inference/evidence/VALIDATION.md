# NEW095 validation record

Assignment: complete bounded exact contingency inference, namespace
`samples/cli/independent/exact_contingency_inference`, no worker dependencies.
The original checkout and all prior work are preserved. Implementation has no
worker dependencies. One explicitly authorized independent reviewer performed
read-only source and Python-oracle review; all implementation and compiler runs
are owned by this assignment.

Base: `c24b141cb29b311e92de0a75b9609ec37e9bf28a` fetched from main.
Branch: `samples/new095`; worktree: `/workspace/Omega/.codex/worktrees/new095`.
Claims registration and subsequent status both timed out. Pending ticket:
`be31ac00f7b6420f9262e2f29a4f248f`. No successful registration is claimed.
Main-only landing guidance was read. Publication uses the documented queue;
there is no remote worker branch, PR or direct main push.

Executor: Linux x86-64; 5 logical CPUs; about 17 GiB RAM; no swap.
One Rust build and one Omega process at a time. Compiler build:

```sh
source /workspace/cloud-setup/activate.sh
mbx build --locked -p omega --release -j 4
```

Build target resolves to
`/workspace/cloud-setup/cache/mbx/targets/v1/23e6a0809bb0b1fc458702df34da46619009b29b90b9dc5833c1598a703313d6`.
The pre-existing setup compiler reported unrecorded source provenance, so it
was not used for application validation. The selected checkout's compiler is
built independently. `OMEGA_LIBRARY_ROOT` selects this checkout's library.

The reference was frozen before any Omega check, execution, or package update.
Initial 18-case snapshot was superseded, still before execution, by the 19-case
snapshot containing distinct tail rankings:
`data/oracle.json` SHA-256
`539a454e7f5f18b09e089a2bda3e1a6303553d9ba0ce65007d037467da6293d4`.
Numeric tolerance is zero, ties inclusive. Independent oracle verification:

```sh
python3 samples/cli/independent/exact_contingency_inference/reference.py
```

Observed: 19 cases verified, tolerance=0. The supplementary numerical control
was also frozen before execution, SHA-256
`d8b98c629ad27e6412268fbf94f2740c9ec84920291a25684ad0fd3d1a12599c`.
It checks all numerical fields on seven valid input cases under native/Terminal.
Expected control stdout: `PASS frozen_numerical_control` plus LF; status 0.

The original main compiler binary SHA-256 is
`357c91b99452051c91947f225a4b8551b233c6d4c0759a0af9d14d0c6367cfb8`.
It reproduces a missing structural scalar store for mutable nonlinear owned
parameters in the complete `controls/owned_parameter_store` source and the
dependency-free `controls/inspection` source. The bounded repair admits owned
destinations through checked Terminal construction, lowering, physical selection
and legalization. Exact declaration joins, nonlinear multiplicity, qualification
and claim restrictions remain. Store-only register entry parameters receive the
existing aggregate frame home. No shared export or numerical library changed.

Pre-rebase repair compiler SHA-256:
`f5a71806fe2230d7dca933f4858cc4c7358740ff8bbb54e0c47a72cc01e2c61d`.
Its affine entry and continuation regression checks direct and nested fields,
preserves siblings and exits 70. The immutable destination negative rejects for
the required mutable binding rule. Native and Terminal corpus observations,
commands and actual exits are retained under `build/new095` in the delivery.
The final delivery manifest records the incorporated-main candidate and compiler
hash used for final application execution separately from this repair witness.

The initial recursive decimal formatter correctly failed the documented runtime
tail-recursion rule. `controls/recursive_unit` retains that complete reproduction.
The formatter now uses a bounded iterative state loop. The recursion eligibility
guard is unchanged; that refusal is not claimed as a compiler defect.

Ordinary project-local std review was accepted after inspecting all 286 exact
decisions: 140 callables, 140 supplies, three Console terminal permissions and
three nominal service declarations. No restricted build, assumed binding or host
provider grants were added. No package acceptance or execution approval was
denied. Cache custody requires the supported per-command approved compiler
context with real UID 1000; guards and ownership were not modified.

Application commands, run sequentially after ordinary project update:

```sh
export OMEGA_LIBRARY_ROOT="$PWD/source/library"
python3 samples/cli/independent/exact_contingency_inference/validate.py --omega "$PWD/build/new095/omega-final" --mode check
python3 samples/cli/independent/exact_contingency_inference/validate.py --omega "$PWD/build/new095/omega-final" --mode invalid
python3 samples/cli/independent/exact_contingency_inference/validate.py --omega "$PWD/build/new095/omega-final" --mode native
python3 samples/cli/independent/exact_contingency_inference/validate.py --omega "$PWD/build/new095/omega-final" --mode both
```

Each validator saves commands, exits, stdout/stderr hashes and exact comparisons
under ignored `build/validation`. Native validation builds one product, then
executes that unchanged ELF for all remaining records. Terminal validation uses
`--both` and demands the authored exit, exact stdout and Terminal exit agreement;
the supplementary numerical control checks every analyzer field internally.
The final delivery includes these observations and artifact/source hashes.
