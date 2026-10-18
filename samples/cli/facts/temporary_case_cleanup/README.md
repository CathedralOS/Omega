# Scalar temporary cleanup with omitted common fields

**Bonus.value == 2 survives each place into an affine case common field with
omitted zero storage and normal shared-call cleanup.**

Every place supplies the same copyable `Bonus { value: 2 }` to `consume`, which
constructs an affine `Event::Treasure` with an omitted zero common field and
borrows it for a scalar-result call. Each `invalid/` twin changes one supplied
`Bonus.value` from 2 to 1 and retains the required bound and operation.

| Place | Transport |
| --- | --- |
| `local` | Local binding |
| `field` | Enclosing record field |
| `parameter` | Required parameter fact |
| `call_result` | Guaranteed result fact |
| `match_join` | Both selected result arms |
| `const_name` | Named copyable `Bonus` constant |
| `other_state` | Named state parameter |
| `other_machine` | Separate producer and consumer |
| `target_body` | Target-selected producer |

Run a positive with `omega run --both <place>/main.omg`; both native and Terminal
execution must exit 0. Check its `invalid/main.omg` and compare the authored
`expected.txt` fragment. The const place transports `Bonus`, not an affine
`Event`.

The unchanged root `main.omg` additionally checks once-only ordered child calls:
short-circuiting skips the first construction, then two child calls increment a
counter before the constructed case is borrowed and cleaned up. It must also
exit 0 in native and Terminal execution. The unchanged full regression fixture
`tests/omega/pass/data/runtime_mixed_case_nested_common_fields_exit` retains its
arrays, erased fields, active payload and short-circuit paths.

Cleanup uses exact checked dying-owner identities. A contained omitted zero
field may share its constructor's source expression; it is not another owner.
The focused drop fixtures also check nested backing, linear claims, projected
residuals, and observable reverse completed-owner hook order. This bounded repair
does not close the broader CML4 ownership acceptance.
