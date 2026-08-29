# Delta compiler validation

This directory owns the evidence and checks for publishing the canonical Delta
compiler. It is adjacent to the compiler it admits; it is not another language
rung or a generic assurance layer.

- `DELTA_SOURCE_CLOSURE_SNAPSHOT_V1.md` defines the exact compiler-source
  closure recorded under `source-closures/`.
- `DELTA_LOWER_ROOTED_ASSEMBLY_PUBLICATION_V1.md` defines repeated lower-rung
  execution and deterministic assembly publication. That two-execution schema
  is now historical diagnostic compatibility only: it may verify old fixtures
  but cannot gate the replacement attempt. The replacement join must retain
  one canonical execution and no required heartbeat/process ceremony.
- `lower_rooted_assembly_publication_v1_driver.py` prepares the attempt and
  keeps marker custody around four generated scripts. Those scripts expose the
  translator, packing transport, and interpreter commands literally; the
  driver does not privately select or execute a compiler command.
- `lower-rooted-assembly-publication-v1.sh` only verifies supplied evidence.
- `DELTA_LOWER_ROOTED_ARTIFACT_CUSTODY_V1.md` defines bounded native-artifact
  realization and custody after assembly publication. Its terminal receipt
  surfaces the exact source, assembly, target, replayed-executable,
  reconstruction-obligation, and scoped host/target-admission bindings while
  leaving source-to-artifact refinement explicitly open.
- `realize-delta-artifact-v1.py` runs the exact V1 clang command from explicit
  absolute inputs, admits its result through the custody `observe` command,
  and atomically publishes only the candidate executable, empty process
  streams, and realization observation into a previously absent directory.
- `install-verified-artifact-v1.sh` reconstructs and verifies the complete
  supplied publication/custody evidence before atomically installing the exact
  six retained files. `verify-installed-artifact-v1.sh` rechecks both that
  inventory and the full reconstruction route; `installation.json` is only a
  content inventory and grants no authority of its own.
- `reconstruct-and-verify-installed-artifact-v1.py` rebuilds the disposable
  lower-rooted tapes, short elaboration, packed Gamma, decoded assembly, and
  ordinal observations from those six retained files plus the exact current
  repository, then calls the installed-artifact verifier. It does not rerun the
  long Gamma evaluator or recreate historical evidence that two processes ran.
- The colocated `*-test.sh` entrypoints exercise each boundary directly.

These tools may coordinate or reconstruct evidence, but they do not parse or
compile Delta and they do not add a compiler stage to the lattice.
