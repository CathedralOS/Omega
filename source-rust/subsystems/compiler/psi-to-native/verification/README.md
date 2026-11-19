# Provider and installation admission

These packages check the evidence that lets a native program call a provider or
install a handler. Their callers need the checked records during compilation.

- [Provider correspondence](provider-correspondence/src/lib.rs) matches candidate
  bodies and plans to declarations. Provider selection and admission both use
  those matches.
  [Callback placements](provider-correspondence/src/callbacks.rs) match the private function,
  its inbound ABI and the registrar operation to the same published Psi program.
- [Provider admission](provider-admission/src/lib.rs) checks the program's
  bindings and permissions and issues the corresponding admitted executions,
  installations and callbacks. Its [same-stack evidence](provider-admission/same-stack/src/lib.rs)
  binds an opaque call's stack use to the exact provider plan, requirement and receipt.
  Later stages retain these records through publication.
  [Task activation checks](provider-admission/src/task_activations/mod.rs) match the selected
  runtime, stack geometry and suspension obligations to the published demand.
- [Native Build bindings](native-bindings/bindings.rs) rejoin the evaluated
  receipt and service declaration to the canonical Psi boundary, target layout
  and exact calling application. Provider settlement, runtime capture and
  retained-image replay all use those checks.
- [Installation admission](installation-admission/src/lib.rs) checks handler
  requirements against the selected program. Instruction selection obtains
  the planned resources, frame binding checks the measured handlers, and
  publication checks the recorded physical demands.
  Its [installation conditions](installation-admission/conditions/src/lib.rs)
  retain the caller's platform assumptions and each handler's measured stack
  requirements. Build and publication can carry these records without running
  provider or installation admission.
  [Bound installations](installation-admission/bound-installations/src/lib.rs)
  carry the installed interrupt and callback entries, helper associations and
  measured stacks read by instruction selection and frame binding.
- [Target providers](target-providers/target_providers.rs) joins Build's
  [selected plans](../../build/providers/selections/selection.rs) to the
  canonical candidate and service declarations, checks synchronous call cycles
  and rechecks every retained folded-constant dependency before installation.
- [Runtime publication](runtime-publication/publication.rs) compares the admitted
  component call, authority and exact ABI to the emitted image's export slot and
  [measured stack](../pipeline/representations/machine-code/stack-demand/physical_stack.rs).
  The interpreter receives the checked component roster and host provisions.

[Machine-code checks](../pipeline/representations/machine-code/) compare a
frame, instruction encoding or layout with its contract. [Object replay](../replay/)
checks an emitted object against the original stage inputs kept for a validator request.
