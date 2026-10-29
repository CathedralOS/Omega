# Source-free task activation admission

Owns the task-activation portion of `PIPELINE-ARCHITECTURE-ENFORCEMENT` /
`PROVIDER-SELECTION-AFTER-TERMINAL`. Checked `start` and `try_start` applications
publish provider-independent demands; native admission receives those demands
from Terminal Psi and joins the exact selected runtime and target geometry.

`Main::probe` supplies the static task applications. `Main::main` is an empty
executable entry: native and Terminal execution both exit zero. This proves
publication/admission, **not execution of a task runtime**.

The inspection oracle compares source-only checking with published activation
plans, then transports the artifact through its public codec and compares those
plans again. A second ordinary named-inspection request exercises the product
completion route; the inert probe's obligations must still be admitted from its
published bundle. Both plans retain a 64-byte stack bound:
the root's resume word, Token parameter/local and predicate occupy 25 bytes;
the largest staged call makes 48 aligned bytes; the nested finish activation
adds 16. Summing the two staged calls would give 72, and summing the two mutually
exclusive finish edges would give 80. Pending boundary contributions remain
explicitly unresolved, not invented as zero-cost completed stack evidence.

Controls refuse missing providers, wrong package ownership, changed authored
requirements and out-of-scope requirements. External artifact ingress refuses
unknown storage, absent roots/callees, unresolved entry identities, absent
specialization commitments and truncation. The sibling rejection fixtures
retain provider-contract narrowing, machine-effect mismatch and missing
selection checks; `ordinary_named_task_runtime_compile` prevents recognition
of an unrelated user trait by spelling alone.
