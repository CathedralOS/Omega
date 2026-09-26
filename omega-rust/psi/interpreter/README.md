# Psi interpreters

These independent engines execute target-neutral representations:

- [Checked interpreter](checked-interpreter/src/interpreter.rs): checked-tree
  reference execution, also used by build-time evaluation.
- [Terminal interpreter](terminal-interpreter/README.md): execution of a verified
  Terminal Psi artifact.

Compilation precedes execution in the calling product workflow. Engines depend
on their input representations and semantic checks, not on the product compiler
coordinator. This keeps compiler use of evaluation from creating a dependency
cycle through an interpreter command workflow.

Native realization remains Omega-owned; interpretation is not a fallback for
unsupported native lowering. See the [pipeline contract](../../pipeline.md).
