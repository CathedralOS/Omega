# PR744 execution evidence

Source head bd57535230243699e595f565e84d75fa19be881d; exact optimized producer e3c13ae2455c62f31486b028dab0fd6705ab6c4e; fixed base9a4ec4a40c352ea4eb51f49f2d23ee03e3e3a78e.

The archive contains qualification state/immutable receipts and raw streams, baseline comparisons, canonical excerpts, build provenance, exact validators, measured fixture outcomes and a Git source bundle. Failed setup/harness attempts remain distinct from accepted observations. Seven inherited defects remain red. Binaries are retained in the workbench; the source bundle and build receipts permit rebuilding without trusting them.

Apply source.bundle to a clone containing the fixed base. Build with the pinned toolchain and optimized profile (`cargo build --locked -p omega --bin omega`; use mbx if available). Generate ordinary fresh package acceptance rather than copying this machine's approval state. The core witnesses are `samples/cli/facts/nested_machine_invocation/joint_rank/*/structural_view/main.omg`, each run with `omega run --both`. The retained scope.json names the focused43-source/25-realization corpus checks. Only macOS ARM64 native execution is observed.
