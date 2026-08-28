# Conservative CKIR5 backend implementation note

[`CKIR5`](OMEGA_BOOTSTRAP_CHECKED_IR_V5.md) defines the normative byte and
meaning contract. This note owns only its conservative Delta artifact
implementation and focused evidence.

`omega-bootstrap-checked-ir-v5-to-elf.alp` is the versioned successor to the
frozen CKIR4 backend. It accepts schema majors 4 and 5. Its schema-4 branch
retains the 80-byte header, inherited table sequence, validations, frame
assignment, instruction templates, and ELF bytes. The focused gate compares
that output with both the frozen CKIR4 backend and the independent CKIR4 ELF
reconstructor.

For schema 5 the backend independently derives sum/case/payload partitions,
private tag/payload layout, recursive Copy and zero properties, constructor
objects, and selected-payload binding snapshots. Fixed compact arenas retain at
most 4,096 cases, 4,096 payload fields, 4,096 case arms, and the shared 94,208
ordinary-operand plus arm-argument budget. Constructor objects and structural
bindings consume the inherited per-machine frame and live-stack budgets.

`ConstructCase` stores the declaration-order tag and active semantic payload
before publishing its frame-owned address. Structural Call passes that address
unchanged. Copy and selected structural payload binding walk semantic leaves;
CaseDispatch checks the unsigned runtime tag before any payload read, selects
arms in declaration order, stages all selected arguments, and only then commits
target parameters. Impossible tags branch to the inherited trap.

Text-pass state, block/operation traversal, sum-case traversal, and structural
payload displacements are independent state. Nested sizing or copy walkers may
not reuse those cursors: the sizing pass must retain every block offset for the
later exact pass, and structural payload code must restore the displacement
selected before entering a recursive walker.

The focused evidence is:

- `../gates/delta-checked-ir-v5-backend-fixture.py`: a handcrafted product-
  shaped result-70 carrier plus isolated semantic and 4/5, 64/65, and 4,096/
  4,097 resource controls.

The former producer-backed wrapper joined that fixture to native/self CKIR5
artifact identity, template checks, independent meaning, and frozen CKIR4 byte
identity. That replay is suspended until canonical Delta publication.

This is private bootstrap evidence. It does not define a public Omega sum ABI,
add sum constants or structural returns, or widen the source profile beyond the
normative CKIR5 tranche.
