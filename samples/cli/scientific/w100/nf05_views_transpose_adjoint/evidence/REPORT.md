# NF05 verification handoff

Base `9d4451128dd05885e7b9c591796b2760189851db`, isolated branch `nf05-views`.
Starting checkout was `6e4f0c37ec`; fetched main was inspected before isolation.
The active ownership query timed out (exit 3); claiming NF05 failed because no
NF05 marker exists on TASKS.md (exit 1). No claim registration is asserted.

One Rust build: `source /workspace/cloud-setup/activate.sh && mbx build --release
-p omega`, exit 0, release optimized, 6m 29s. Warnings came from unchanged main.
No Rust changes were made. The binary's exact path is in `checkpoint.json` and
all exact executable/source argument vectors and cwd values are in `routes.json`.

The mathematical coordinate oracle passed 19 frozen comparisons. Numerical
oracles and tolerances were frozen before execution; SHA-256 values and original
source hashes are recorded. All values are exactly representable small integers.
No NF02 floating-point accuracy policy is introduced.

| Project | Check | Native | Terminal comparison |
|---|---|---|---|
| sensor_patch | 1, environment failure | 200, environment failure | 200, environment failure |
| hermitian_energy | 1, environment failure | 200, environment failure | 200, environment failure |
| invalid_readonly_write | 1, environment failure | not attempted | not attempted |

Every stderr contains:

```text
cannot open private source resolver storage: git cache entry `/` is invalid: cache custody ancestry is owned by an unrelated user
```

This is the wave's reported environment/custody blocker, before source validation.
No source check, native output, Terminal agreement, or intended readonly-write
rejection is claimed. The invalid source's exit 1 does not satisfy its intended
rejection oracle. System ownership, permissions and custody guards were unchanged.
Each invocation ran serially; no concurrent Rust builds or Omega execution.
Full stdout/stderr are retained separately. Full authored source and Build source
for failed positive invocations are archived as `.omg.txt` under `failures/` to
avoid creating unintended extra sample projects. Reproduction commands refer to
the original intact sample source, preserving its correct dependency paths.

Resources: 5 logical CPUs, cgroup quota 4 CPUs, memory limit 16 GiB. At initial
inspection available host memory was about 16 GiB and free disk about 30 GiB.
During build cgroup memory was 4,645,908,480 bytes; sampled host CPU utilization
was 79.96% over one second. After build free disk was about 28 GiB. The CPU sample
is host-wide, not an attribution of compiler CPU time; `resources.json` retains
raw cgroup accounting and `checkpoint.json` retains exact byte measurements.

## Remaining owned work and exact dependency frontier

`source/library/numerics/arrays/views_transpose_adjoint.api.txt` requests NF01's
canonical RankTwo, ArrayError/attempt cases, checked descriptor admission,
mutable-injectivity admission, try_index, try_element_count and contiguous-layout
predicate, including import paths and proof domains. NF03 must supply Complex64
construction/component access and conjugate with exported names and scalar
policy. Their interfaces are absent from the fetched base. NF01 retains package
metadata/layout ownership; NF02 retains accuracy policy. No competing shared
implementation or package metadata was invented. The production `.omg` file is
withheld rather than represented by a nonfunctional stub.

The complete fixed-coordinate samples are independent mathematical projects,
not NF05 descriptor acceptance. Parent/integration DRI should provide NF01/NF03
owner commits and legitimate environment configuration. Then implement the
specified descriptor-only transforms, wire these unchanged oracles to actual
shared API calls, execute the frozen admission cases, and requalify all routes.
No branch is landed and no board item is marked complete. Candidate work is
retained for independent review and normal landing through the parent DRI.
