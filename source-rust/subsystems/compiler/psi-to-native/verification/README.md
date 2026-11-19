# Provider and installation admission

These packages check the evidence that lets a native program call a provider or
install a handler. Their callers need the checked records during compilation.

- [Provider correspondence](provider-correspondence/src/lib.rs) matches candidate
  bodies and plans to declarations. Provider selection and admission both use
  those matches.
  [Direct bindings](provider-correspondence/direct-boundaries/bindings.rs) verify the published requirement’s service schema, selected native operation and ABI.
  [Callback placements](provider-correspondence/src/callbacks.rs) match the private function,
  its inbound ABI and the registrar operation to the same published Psi program.
- [Provider admission](provider-admission/src/lib.rs) checks the program's
  bindings and permissions and issues the corresponding admitted executions,
  installations and callbacks. Its [same-stack evidence](provider-admission/same-stack/src/lib.rs)
  binds an opaque call's stack use to the exact provider plan, requirement and receipt.
  Its [authority review receipt](provider-admission/authority/receipt.rs) checks the
  reviewed leaves against the physical classification and receiving permission,
  and commits each role-tagged mechanism to the selected provider closure.
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
- [Component descriptions](component-description/description.rs) reconstruct the published
  entries, exports, authority and obligations from embedded Psi. Receivers compare
  those records with the native component description before admitting a provider.
- [Runtime publication](runtime-publication/publication.rs) compares the admitted
  component call, authority and exact ABI to the emitted image's export slot and
  [measured stack](../representations/machine-code/stack-demand/physical_stack.rs).
  The interpreter receives the checked component roster and host provisions.

[Machine-code checks](../representations/machine-code/) compare a
frame, instruction encoding or layout with its contract. [Stage validators](stage_validators.rs) call [published-object verification](published-objects/) with the emitted object and the originals retained for that request. The normal emission path hands off the finished image directly.
