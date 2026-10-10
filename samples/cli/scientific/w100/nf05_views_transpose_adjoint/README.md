# NF05 views, transpose and adjoint

Base: `9d4451128d`. Independent fixture checkpoint; shared descriptor acceptance
is pending NF01 and NF03. See the exact signature requests in
`source/library/numerics/arrays/views_transpose_adjoint.api.txt`.

The two complete standalone projects deliberately use fixed-coordinate backing
accesses until the shared interfaces arrive. They establish mathematical and
compiler-route fixtures, **not qualification of the NF05 library capability**.
No local competing descriptor, shape validator, or complex library is introduced.
All copying is visible in caller-owned fixed arrays; future descriptor transforms
must return metadata without copying or allocating.

- `sensor_patch`: packed 2x3 and offset/padded 2x3 storage produce the same 3x2
  transpose; 3x2 reshape preserves packed order; a repeated-row readonly mapping
  reads aliases; a transposed patch mutation changes one backing cell and preserves
  padding. Exact expected exit 0 and `NF05 sensor patch oracle: PASS` stdout.
- `hermitian_energy`: split integer components of a complex 2x2 matrix produce
  conjugate transpose and the Hermitian Gram matrix. The off-diagonal +48i differs
  from ordinary transpose and catches lost conjugation. Exact expected exit 0 and
  `NF05 Hermitian energy oracle: PASS` stdout.
- `invalid_readonly_write`: separate complete project that stores through
  `&[i32]`. Intended rejection is immutable/shared slice write. A missing package,
  syntax rejection, execution failure, or decline does not satisfy that oracle.

`oracles.json` was authored before execution. Absolute and relative tolerance
are zero: small integers and complex components are exact. This does not set NF02
floating-point accuracy policy. `verify_oracles.py` is an independent coordinate
oracle covering empty 0x0/0xN/Nx0 views, double transforms, nonzero-stride overlap,
readonly aliasing and Hermitian symmetry. It is not a production descriptor API.

`admission_cases.json` separately freezes exact boundary inputs and intended
try-error outcomes for shared API integration; these are not yet executed.

## Frozen full shared-API sample designs

Sensor integration must call NF01 admission on the packed and padded backing,
NF05 transpose/submatrix/reshape with separately supplied lengths, and access
through the shared checked index API. Assert padded reshape -> NonContiguous,
unequal-count reshape -> ElementCountMismatch, overflow -> SizeOverflow,
undersized backing -> InsufficientBacking and out-of-parent submatrix ->
OutOfBounds. Empty corner (2,3) must retain the admitted offset 1 and perform no
read/write: its fictitious nonempty address would be 12, beyond length 10.
Use readonly zero row stride [0,1] to read [2,2,3,3,5,5]; mutable admission must
reject it. Packed, padded, transposed and submatrix mutable admission must pass
under NF01's actual restricted policy and mutate only the expected backing cell.

Complex integration must construct NF03 Complex64 values [1+2i,3-4i,5+6i,7+8i],
read NF05 lazy adjoint [1-2i,5-6i,3+4i,7-8i], verify double adjoint identity,
and use explicit caller-owned copy destination if copying is wanted. Confirm
source backing remains unchanged. Use NF02 policy for later signed-zero,
nonfinite, rounded or approximate tests; do not silently derive it here.

## Reproduction

From the isolated worktree, activate the executor toolchain and build once:

```sh
source /workspace/cloud-setup/activate.sh
mbx build --release -p omega
python samples/cli/scientific/w100/nf05_views_transpose_adjoint/verify_oracles.py
```

Use the binary path reported by mbx (record it in evidence). For each positive
project run these commands separately, replacing `<project>` with `sensor_patch`
or `hermitian_energy` and `<omega>` with that binary:

```sh
<omega> --check samples/cli/scientific/w100/nf05_views_transpose_adjoint/<project>/main.omg
<omega> run samples/cli/scientific/w100/nf05_views_transpose_adjoint/<project>/main.omg
<omega> run --both samples/cli/scientific/w100/nf05_views_transpose_adjoint/<project>/main.omg
```

`run --both` must explicitly report Terminal agreement; native exit 0 accompanied
by DECLINED is not Terminal acceptance. For the invalid control run `--check` on
its `main.omg`, require exit 1 and inspect the diagnostic's intended cause.
Keep one host Rust build and one Omega invocation at a time. `evidence/` contains
exact commands, exits, source hashes and full diagnostic/stdout logs.
