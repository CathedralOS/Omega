# Project build

[`build`](build.rs) starts with the core plan. Each plan either asks for an artifact or describes a program to compile and run for its next plan.

The host admits each plan against the current grants and binds its reflected declarations in the next stage. The driver compiles that stage to Psi and gives it to the interpreter with the admitted services. The next plan can include newly discovered or generated source. When a plan requests an artifact, the driver returns the compiler's result directly.

Build records the [resource grant and usage](evaluation/usage/usage.rs) for each evaluated program. A [native binding receipt](evaluation/receipts/native_binding.rs) ties that usage and the observed inputs to the identity of the Psi program that ran.

Before publishing into a caller-visible directory, the host [acquires it for the input program](output_directory.rs). The lease excludes overlapping publishers. The persistent owner record prevents a different input from overwriting the same directory on a later invocation.

The loop and host interface are a prototype of the [V2 build decision](../../../../omega-rust-v2/PLAN.md#v2s-build-model-a-chain-of-stages). Implementing plan admission and interpreter integration is still open.
