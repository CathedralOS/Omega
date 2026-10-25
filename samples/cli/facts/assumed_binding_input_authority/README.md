# Assumed binding input authority

The root-assumed input qualification `value <= 1` indexes `[7, 11]` of length 2,
observing 11; `Supplied::get` retains its own contract.

`Required::get` takes an unqualified `Index`. `Supplied::get` requires
`index in Index::Bounded`, and that domain requires `index.value <= 1`.
The supplier body uses the qualification to index. The caller does not
prove the domain. On current main, installation refused the link:

```
native artifact checked-provider installation failed: PsiAdmission(AssumedBindingRejected { required: BoundaryMachineId(3), supplied_requirement: BoundaryMachineId(4), reason: UnsupportedAssumedAuthority })
```

Owning task: ASSUMED-BINDING-CONTRACTS. This row is the input-side
qualification the installation gate dropped. The exit result-bound row
stays in `assumed_binding_result_bound`.

## Places

Each place transports `Index { value: 1 }` into `Required::get`. The
invalid twin changes only the domain bound from `<= 1` to `<= 2`.

| Place | Positive | Reject twin |
| --- | --- | --- |
| root | [main.omg](main.omg) | — |
| local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) |
| field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) |
| parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) |
| call result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) |
| match join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) |
| const name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) |
| another state | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) |
| another machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) |
| another target body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) |

`controls/direct_supplier` calls `Supplied::get` with an unqualified
parameter and rejects `cannot prove requires contract`.
`controls/passing_mode` keeps the borrowed supplier parameter rejected
as incompatible runtime carriers.

## Requirements

- **Establishment:** `Build::assume_binding_contract<Required::get, Supplied::get>`
  admits the supplier's input domain. The supplier body is checked against
  that domain. The caller is checked against `Required::get`, which has
  no domain.
- **Identity:** the qualification is `Index::Bounded` on the `get` argument.
  A direct supplier call does not receive it.
- **Composition:** the value `1` reaches `Required::get` through all nine
  places. The invalid twin weakens only `index.value <= 1` to `<= 2`, and
  the supplier body rejects `cannot prove index ... within length 2`.
- **Soundness:** result qualifications, content guarantees, and program-local
  root introductions still fail closed. Passing mode still rejects.
- **Remaining:** broader invocation and lifetime correspondence, and
  install/update/audit acceptance of the authority difference.
