# Delta lower-rooted assembly publication receipt, version 1

This contract defines the exact evidence join for a candidate publication of
the canonical Delta compiler's deterministic Darwin ARM64 **assembly stream**.
It is not executable-Mach-O publication, compiler authority, a Delta language
freeze, or permission to use an ambient assembler, linker, or retired producer.

No receipt exists until the lower-rooted translator elaborates the canonical
source once and the canonical Gamma interpreter executes the exact closed
program twice. The generator has no default evidence paths and emits nothing
on rejection. No artifact extent or digest is frozen before those executions.

## Antecedents and transformations

The join reconstructs `delta.compiler.current.v1`'s exact canonical LF image.
It binds exact bytes for:

- the Alpha-written assembler source and supplied deterministic persisted
  assembler tape, under role `canonical_alpha_written_assembler_artifact`;
- `bc.beta` and the repository's persisted fixed-point `bc.tape`, under role
  `persisted_alpha_rooted_beta_compiler_fixed_point`;
- `omega2gamma.beta` and its Beta-built Alpha tape;
- `interp.beta` and its Beta-built Alpha tape; and
- the canonical packed-input encoder and structured-output decoder sources.

The two construction roles refer to the already-established Alpha/Beta
publication boundaries. This receipt rebinds their exact source/tape bytes; it
does not claim to re-prove those boundaries or infer tool correctness from a
hash.

One canonical elaboration observation records status zero, empty diagnostics,
the exact source image, toolchain, and emitted Gamma template. That observation
is evidence of the source-to-template execution relation, not a theorem that
the translator implements all Delta semantics. The join then independently
reruns the versioned packed-input transformation over the template and exact LF
image and requires byte equality with the supplied closed Gamma program. A
caller-supplied closed-Gamma hash alone is insufficient.

Each Gamma execution supplies both the interpreter's raw structured stdout and
the assembly bytes decoded from it. The join independently decodes the exact
`(Pair status stdout)` observation, requires semantic status zero, compares the
decoded bytes with the supplied assembly, and applies the strict Darwin ARM64
assembly validator. Thus neither a raw Gamma-output hash nor an assembly hash
alone establishes the result.

## Observations and resources

The canonical schemas are:

```text
omega.delta-gamma-elaboration-observation.v1
omega.delta-gamma-execution-observation.v1
```

Objects are key-sorted canonical JSON with two-space indentation and one final
LF. Elaboration and both execution observations recompute all file extents and
SHA-256 identities. Execution ordinals are exactly 0 and 1. Process status and
the decoded Delta status are both zero. Diagnostic stderr is exactly empty.
Elapsed milliseconds are retained as diagnostics and may differ between runs.

The execution observation binds the interpreter's concrete canonical profile:

```text
Gamma source bytes              4,194,304
evaluator argument scratch      512 values
evaluator fuel                  50,000,000
stable-address heap arena       41,943,040 bytes
heap mark/allocation map         5,242,880 bytes
Alpha return-stack reserve       1,048,576 bytes
```

These are checked against the exact `interp.beta` source identity and are not
summarized by a free resource-profile string. The observation states the actual
zero status and empty diagnostics. It makes no claim about elapsed time as
semantic identity and does not turn successful finite execution into a general
termination proof.

Documents are bounded at 65,536 bytes, the Gamma template at 1 MiB, the closed
Gamma program at 4 MiB, and each deterministic Alpha tape at 262,140 bytes.
The raw structured Gamma observation is bounded at 256 MiB because it renders
the byte list structurally. Assembly retains the separate 16 MiB, 500,000-line,
512-byte-line validator profile. Malformed/cross-paired evidence returns 251;
resource overflow returns 252. Rejection publishes no stdout bytes.

## Receipt

The receipt schema is
`omega.delta-lower-rooted-assembly-publication.v1`; its publication ID is
`delta.compiler.darwin-arm64-assembly.v1`, and its deliberately narrow claim is
`candidate_lower_rooted_assembly_only`. It binds source snapshot/image,
toolchain, template, closed Gamma, elaboration observation, two execution
observations, common assembly, target `darwin_arm64`, configuration
`conservative`, ABI `darwin-arm64-assembly-v1`, and validation profile
`delta.darwin-arm64-assembly.strict-v1`.

The executions must agree byte-for-byte on raw Gamma stdout and decoded
assembly, as well as every semantic identity. Only elapsed diagnostics may
differ. `receipt_sha256` is SHA-256 over the domain
`omega.delta-lower-rooted-assembly-publication.v1\0`, the u64 little-endian
compact-projection length, and canonical compact JSON with the receipt digest
field omitted.

`lower-rooted-assembly-publication-v1.sh` verifies an already-generated receipt
and requires every execution product explicitly. Missing evidence returns 2;
there is no placeholder that can pass. Even a green receipt closes only this
candidate assembly join. Separate lower-rooted assembly/Mach-O reconstruction
and direct refinement are required before the result is a runnable canonical
Delta compiler artifact or compiler authority.
