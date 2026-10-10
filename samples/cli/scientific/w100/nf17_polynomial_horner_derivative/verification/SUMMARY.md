# NF17 candidate qualification

Base: `26c92c17b8f4b21f39e14a5be57c85cfdec601de`. Worktree: `.codex/worktrees/nf17`;
branch: `nf17-polynomial`. No Rust compiler or shared NF01/NF02 implementation changed.

- Fresh CLI build: `source /workspace/cloud-setup/activate.sh; CARGO_TARGET_DIR=/workspace/Omega/target mbx build -p omega --release`, exit 0, 11m 37s. Existing Rust warnings remain.
- `python .../verify_oracles.py`: 10 independent exact rational fixtures verified.
- `git diff --check`: exit 0.
- Scalar and batch check routes: exit 1 before source validation.
- Scalar and batch native and Terminal comparison routes: exit 200 before source validation; no numerical output and no Terminal result.
- Invalid empty-source control: exit 1 before source validation, **not the intended rejection**.

All seven authored project routes fail private package-cache custody:
`git cache entry '/' is invalid: cache custody ancestry is owned by an unrelated user`.
An unchanged existing float corpus control reproduces this on the base revision.
Read-only host evidence: effective UID 1000; `/` UID 65534; `/workspace` and the
configured cache UID 1000. No ownership, permissions, or custody guards changed.

A package-free direct library check reaches source validation and rejects the
current kernel. `kernel-final-check.txt` and `kernel-final-source.txt` preserve
its exact command and complete source. Remaining frontier: generic returned
record structural lifetime handling; preservation of nonempty/length facts and
subslice bounds; slice-rank termination proof. A separate package-free concrete
binary64 scalar diagnostic also rejects (`bare-scalar-*`), confirming this is
not just an unused open-template diagnostic. That diagnostic preceded the final
batch-loop adjustment; it is not qualification of either authored CLI project.
Earlier source failures are archived with their complete source; intermediate
binding and tail-transition corrections resolved syntax and nested-call errors.

This is an authored candidate, **not accepted or ready to land**. After legitimate
cache configuration is available, fix/resolve the direct kernel frontier and
rerun all authored routes. Read `.api.txt` for exact NF01/NF02 contract requests.
NF01 metadata and descriptor adapters and NF02 policy adapters remain pending;
there is no competing descriptor or accuracy policy here.
