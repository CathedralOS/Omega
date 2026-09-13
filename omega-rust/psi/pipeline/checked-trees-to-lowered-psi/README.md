# Checked trees to lowered Psi

Start at [psi_lowering.rs](src/psi_lowering.rs). It selects the checked machine,
lowers its source closure, retains source custody and evidence, checks the
completed module, and attaches debug companions. Its result is unsealed Psi;
optimization and portable publication belong to the following stages.

[Machine selection](src/psi_lowering/machine_dispatch.rs) owns plan precedence.
Each selected producer supplies its source closure and the work still required
before returning: conformance publication, operand proof completion, and debug
eligibility. The coordinator does not classify the source shape again.

An included source is not necessarily an exact emitted-machine owner.
Catalog-producing paths retain explicit source-to-machine bindings; other paths
must not invent callee custody from table position. Reborrow publication keeps
rejecting when the required exact owner is unavailable.

The subordinate owners follow the work:

- [Unit closures](src/psi_lowering/attached_unit.rs) assemble calls and storage.
- [Scalar closures](src/psi_lowering/scalar_call_closure.rs) assemble scalar graphs.
- [Evidence lowering](src/psi_lowering/evidence_lowering.rs) retains proof subjects.
- [Source custody](src/psi_lowering/call_source_custody.rs) preserves call identities.
- [Tests](src/psi_lowering/tests.rs) exercise completed artifacts and invalid custody.
