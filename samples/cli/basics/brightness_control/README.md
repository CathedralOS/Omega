# Brightness control

The controller applies a command's bounded payload through one state, stores
the resulting brightness, and reports it as the process exit. Its authored
SetLevel command produces 65 and the two console lines in `main.omg`.

```sh
omega run --both samples/cli/basics/brightness_control/main.omg
python3 tools/sample_native_gate.py --filter basics/brightness_control
python3 tools/corpus_gate.py --both --filter dependent/zero_case_predicate_entry_exit,dependent/where_field_shadows_module_const_exit
python3 tools/corpus_gate.py --filter arithmetic/construction_payload_out_of_range,dependent/case_where_zero_payload_requires_establishment,dependent/case_where_zero_wide_arithmetic_not_saturated
```

The public command uses ordinary package and entry review. No acceptance
policy is committed. Matching-host qualification here is macOS ARM64;
the other authored target bindings still need their hosted runs.

## Historical declaration inventory

The four scalar declarations in this sample at `c8eaaebde25` keep their
ordinary `i32` carriers and all their original bounds:

| Position | Original suffix | Canonical obligation |
|---|---|---|
| `Command.SetLevel.level` | `[0..=100]` | case `where level >= 0 && level <= 100` |
| `Command.Dim.amount` | `[0..=10]` | case `where amount >= 0 && amount <= 10` |
| `Main.brightness` | `[0..=100]` | data `where brightness >= 0 && brightness <= 100` |
| `apply.target` | `[0..=100]` | state `requires target >= 0 && target <= 100` |

The three dispatch arms, multiplication by ten, field write, and output
are unchanged. A match publishes only its selected case's facts. No nominal
interval domain replaces a contextual predicate.

The related invalid constructor at `97ebeb57d89`, then
`canaries/fail/arithmetic/construction_payload_out_of_range/main.omg`, has
one `Slot.Found.index: i32 [0..=15]` declaration. Its case now requires
`index >= 0 && index <= 15`, retaining the deliberately invalid literal 9999.
The diagnostic names the case constraint that construction violates.

Entry storage selects the first case, SetLevel, with level zero. The complete
case predicates must hold at that actual valuation; clause presence alone
neither refuses storage nor grants establishment. The maintained regression
carries the command through a parameter, call result, and local beside a
same-named module constant. Its generic command retains the applied maximum
100, so field valuation must use the prepared declaration's identities.
The negative zero control excludes zero from the first case while retaining
a later empty case; that alternative does not establish zero storage.
The wide-arithmetic control refuses treating the mathematical value 2^63 as
2^63 - 1 merely because the checker's finite interval endpoint overflowed.
