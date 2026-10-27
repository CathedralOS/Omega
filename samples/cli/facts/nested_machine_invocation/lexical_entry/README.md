# Lexical entry selection

Owner: `NESTED-MACHINE-UNIFICATION` in [TASKS.md](../../../../../TASKS.md).

Every call to entry names the visible machine and supplies all three declared parameters.

The visible entry helper has three parameters; its sibling next has two.
The compiler-generated body coordinate is not an authored declaration.
The twin removes only the third actual argument. Runtime probes distinguish
7/11 and 9/13, and observe the explicitly supplied nonce 17.

Optimized exact baseline `70d5cda38eda5688b811050f86a3938b50e3c553`
rejects all nine positives with `entry expects 2 argument(s), got 3`, and
wrongly accepts all nine missing-argument twins. This is an invocation-identity
error: the current helper has two parameters and its generated body entry must
not shadow the visible three-parameter source helper. The repaired producer runs all nine positives with native/Terminal exit 0 on
macOS ARM64, and all nine twins reject for the declared three-argument signature.


| Place | Positive | Missing-argument twin |
| --- | --- | --- |
| local | [program](local/main.omg) | [reject](local/invalid/main.omg) |
| field | [program](field/main.omg) | [reject](field/invalid/main.omg) |
| parameter | [program](parameter/main.omg) | [reject](parameter/invalid/main.omg) |
| call_result | [program](call_result/main.omg) | [reject](call_result/invalid/main.omg) |
| match_join | [program](match_join/main.omg) | [reject](match_join/invalid/main.omg) |
| const_name | [program](const_name/main.omg) | [reject](const_name/invalid/main.omg) |
| other_state | [program](other_state/main.omg) | [reject](other_state/invalid/main.omg) |
| other_machine | [program](other_machine/main.omg) | [reject](other_machine/invalid/main.omg) |
| target_body | [program](target_body/main.omg) | [reject](target_body/invalid/main.omg) |

The identical sources and Build files are pinned under
`tests/omega/{pass,fail}/facts/nested_machine_lexical_entry_<place>_{exit,rejected}`.

Ten [controls](controls/) cover ordinary calls, parenthesized terminal calls,
qualified module calls and their required input, missing module declarations,
unpublished body coordinates, and a runtime parameter shadowing a module.
The qualified source explicitly declares `module routines;`: source filenames
alone do not declare logical modules. The original omitted-module setup was
invalid and remains a reject control; it is not counted as a valid baseline
regression. Three positive controls execute in both engines; seven reject.

Requirement evidence:

- **Establishment:** the source declaration introduces the three-parameter
  signature; every call supplies it. A generated body coordinate contributes
  no source declaration or alternative signature.
- **Identity and lifetime:** exact declaration identity survives copies,
  field projection, completed-call snapshots, joins, named constants and deeper
  lexical nesting. No enclosing runtime binding is captured.
- **Composition:** normal and terminal calls share callable selection. Qualified
  calls share the existing source-certified namespace elaboration. The actual
  selected signature owns argument and precondition checking.
- **Soundness boundaries:** the omitted parameter, nonexistent module, hidden
  body coordinate and runtime namespace shadow all reject. Static qualification
  grants no runtime receiver or new declaration-selection authority.
- **Psi 05–07:** explicit call inputs, selected branch tags and returned nonce
  survive Terminal Psi execution. These programs add no proof-only carrier,
  mutable loan, recursive eligibility or cleanup mechanism.
- **Omega 00–02:** Build companions bind all four hosted targets. Host execution
  exercises argument placement and the three-field Note result layout.
- **Omega 03–06:** native execution observes the selected helper's independent
  arguments, both branch tags and aggregate result across activation storage.
  This row makes no new stack-bound or recursive-cycle claim.
- **Omega 07–09 and emission:** emitted macOS ARM64 images agree with Terminal
  Psi at exit 0. Other hosted bindings and target bodies are authored and
  checked; other-host runtime execution is not established on this machine.

The parent remains open for bundled-library admission, the product
parser/resolver declaration model, linked applications, and its remaining full
invocation, cleanup and stack acceptance.
