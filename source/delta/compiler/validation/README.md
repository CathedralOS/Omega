# Delta compiler validation

This directory owns the evidence and checks for publishing the canonical Delta
compiler. It is adjacent to the compiler it admits; it is not another language
rung or a generic assurance layer.

- `DELTA_SOURCE_CLOSURE_SNAPSHOT_V1.md` defines the exact compiler-source
  closure recorded under `source-closures/`.
- `DELTA_LOWER_ROOTED_ASSEMBLY_PUBLICATION_V1.md` defines repeated lower-rung
  execution and deterministic assembly publication.
- `lower_rooted_assembly_publication_v1_driver.py` prepares the attempt and
  keeps marker custody around four generated scripts. Those scripts expose the
  translator, packing transport, and interpreter commands literally; the
  driver does not privately select or execute a compiler command.
- `lower-rooted-assembly-publication-v1.sh` only verifies supplied evidence.
- `DELTA_LOWER_ROOTED_ARTIFACT_CUSTODY_V1.md` defines bounded native-artifact
  realization and custody after assembly publication.
- The colocated `*-test.sh` entrypoints exercise each boundary directly.

These tools may coordinate or reconstruct evidence, but they do not parse or
compile Delta and they do not add a compiler stage to the lattice.
