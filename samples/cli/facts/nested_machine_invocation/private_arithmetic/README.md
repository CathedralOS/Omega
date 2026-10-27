# Private arithmetic formation

Owner: `NESTED-MACHINE-UNIFICATION` in [TASKS.md](../../../../../TASKS.md).

Every private arrival carries a positive value, so the helper can form value - 1.

The helper has no handwritten duplicate bound. Guards or named positive
constants establish the actual argument's lower bound; every admissible arrival
must preserve it. The operation is the same exact u64 subtraction in all places.
Each twin passes zero on one arrival while retaining the guard and operation.
This challenges lexical-name substitution as well as missing incoming facts.

The runtime oracles distinguish 7 -> 6 and 9 -> 8, with separate tags. Guarded
places also execute the zero fallback. The constant place supplies two named
positive constants. All four hosted entry bindings are authored.

Baseline `5ab4e12334fba74babfc3ae8d9992572fba307e6`, optimized producer,
refuses all nine positive places at `apply`'s `value - 1` initializer with
`exact arithmetic ... local result may overflow u64`. All twins reach that same
formation obligation. No caller guard or helper annotation has been added to
make a place pass. The repaired producer executes every positive with native/Terminal agreement
on macOS ARM64; every zero twin still rejects at this arithmetic obligation.


| Place | Positive | Zero twin |
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

The identical programs and Build files are pinned in
`tests/omega/{pass,fail}/facts/nested_machine_private_arithmetic_<place>_{exit,rejected}`.
Supplementary [controls](controls/) cover multiple incoming sites, public entry
contracts, escaping selections and mutation between argument evaluations.
The snapshot control observes the cleared cell and its untouched neighbor as
well as the arithmetic result. Moving the read after the write is its reject twin.

Requirement evidence:

- **Establishment:** dominating guards or the two named positive constants
  justify each actual input; exact subtraction never establishes its own bound.
- **Identity and lifetime:** local copies, projections, completed-call snapshots
  and match joins retain the exact supplied value. The mutation controls
  distinguish a saved scalar from a later read of changed storage.
- **Composition:** the same operand-fit judgment runs against every closed
  private arrival, including a three-level route, an external relay and every
  checked target body. A single invalid predecessor prevents acceptance.
- **Soundness boundaries:** public bodies and escaping helpers still require
  independently established bounds. No enclosing lexical contract or recursive
  conclusion is donated to operand formation. Private cyclic inference remains
  conservative; this row establishes no new recursion eligibility.
- **Psi 05–07:** the retained scalar operation, branch tags and zero fallbacks
  execute through canonical Terminal Psi; the authored main traps on incorrect
  results. These programs introduce no proof-only carrier or cleanup action.
- **Omega 00–02:** authored Build companions bind all four hosted targets;
  the host execution exercises Note layout and helper argument/result placement.
- **Omega 03–06:** native execution exercises exact subtraction, conditional
  flow, helper calls and activation/register storage. This row does not claim a
  new WCSU, recursive-stack, aggregate-ABI or cleanup mechanism.
- **Omega 07–09 and emission:** the emitted macOS ARM64 image executes with
  the same zero exit as Terminal Psi. Other hosts have authored bindings and
  checked target bodies, but no runtime observation on this machine.

The parent task remains open: product parser/resolver migration, linked
applications, remaining invocation obligations and complete acceptance still
need their own evidence. This row closes only private arithmetic formation.
