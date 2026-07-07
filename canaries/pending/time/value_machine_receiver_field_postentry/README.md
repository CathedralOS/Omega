# PARKED REPRO (2026-07-07): value-callee miscompiles, TWO native faces

Found authoring `std::time`'s Duration core (the first multi-state pure-value
machines in std). Interpreter is correct throughout; native diverges. `main.omg`
here is the full failing differential canary; `time_at_repro.omg.txt` is the
std/time.omg body at the time (an earlier probe iteration; the live
`omega/language/std/time.omg` carries the workarounds).

## Face 1 — domain-cast WIDTH inside an inlined value callee (SILENT, root)

`(x as u64 in Wrapping) + (y as u64 in Wrapping)` inside a value machine's
entry, inlined at its call sites, EMITS AS AN i8 CONVERT — backend_report
Target Operations show:

    write runtime storage binary ... bytes 8
        (omega_machine_Main::main_storage@0/8 as i8->i8) Add (3 as i8->i8)

The cast's target type resolves against a wrong/un-remapped type-reference
entry during the value-call splice (the per-call-site `LocalStorage` dumps for
the same let show DIFFERENT `type_reference` handles). Small values mask the
truncation (this is why `checked_add` "worked": every operand fit i8);
`1000000000 as u32 in Wrapping` truncates to byte 0x00 and the borrow path
collapses. Every existing corpus `as X in Domain` cast lives in a TOP-LEVEL
machine, which is why this never fired before.

## Face 2 — receiver-field reads in POST-ENTRY states (SILENT)

In the same inlined value callee, `self.field` / `other.field` reads in states
AFTER the entry resolve against the CALLER's frame (the report renders the
inlined guard as `self.seconds > self.b.seconds ...` — the ARGUMENT
substituted, the RECEIVER did not). The fenced guard-subject case
("runtime dispatch body needs guarded branch expansion") is this same
machinery refusing where it KNOWS; assignment-position multi-state callees
slip through silently.

## Workarounds (live in omega/language/std/time.omg)

- Entry-only field reads; post-entry states consume threaded params only.
- No `as T in Domain` casts in value machines (face 1) — but Wrapping LETS
  REQUIRE operand casts (decision 17 is operand-driven), so full-range u64
  checked arithmetic is currently INTERPRETER-ONLY; the native promotion of
  `runtime_duration_core_interpreter_oracle` waits on face 1.

## Next

Fix face 1 first (likely the value-call splice's Cast copy: remap/resolve the
target type from the callee's table); face 2 needs either the receiver
substitution extended past the entry or a loud fence. Then promote the oracle
test to a `_canary_runs` native test and delete this directory.
