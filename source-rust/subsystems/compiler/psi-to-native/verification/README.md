# Provider and installation admission

These packages check the evidence that lets a native program call a provider or
install a handler. Their callers need the checked records during compilation.

- [Provider correspondence](provider-correspondence/src/lib.rs) matches candidate
  bodies and plans to declarations. Provider selection and admission both use
  those matches.
- [Provider admission](provider-admission/src/lib.rs) checks the program's
  bindings and permissions and issues the corresponding admitted executions,
  installations and callbacks. Later stages consume those records.
- [Installation admission](installation-admission/src/lib.rs) checks handler
  requirements against the selected program. Instruction selection obtains
  the planned resources, frame binding checks the measured handlers, and
  publication checks the recorded physical demands.

[Machine-code checks](../pipeline/representations/machine-code/) compare a
frame, instruction encoding or layout with its contract. [Object replay](../replay/)
checks an emitted object against the original stage inputs kept for a validator request.
