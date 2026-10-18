# Case payload transport

A multi-variant Outcome sum carries its active payload value <= 50 across the place so matching Outcome::Ready adds 10 within result <= 60.

## Parent Capability and Requirement

- **Parent Task**: `STATIC-MACHINE-MIGRATION`
- **Claim Ticket**: `f665f940b5224387926cac45f2b04b8d`
- **Requirement**: Sum case active payload bound transport and constraint preservation across static machine places without implicit receiver or data-subject conformances.

Every place declares copyable `data Outcome [copy] { case Empty; case Ready(value: u64) where value <= 50; }` and transports the sum across the designated place. Destructuring `Outcome::Ready { value }` evaluates `value + 10` under postcondition `ensures result <= 60`. The test validates `Outcome::Ready { value: 32 }` yields `42` and `Outcome::Empty {}` yields `0`, exiting with 0.

## Nine Places and Twins

| Place | Transport Description | Positive Witness | Reject Twin |
| --- | --- | --- | --- |
| `local` | Copied into a local binding | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) |
| `field` | Stored in and read from a record field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) |
| `parameter` | Passed as an explicit machine parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) |
| `call_result` | Returned from a helper machine call | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) |
| `match_join` | Joined from conditional branch arms | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) |
| `const_name` | Named copyable constant binding | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) |
| `other_state` | Forwarded across an explicit machine state transition | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) |
| `other_machine` | Forwarded through a helper machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) |
| `target_body` | Realized across four target-scoped machine bodies | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) |

## Verification Commands

```sh
omega run --both samples/cli/facts/case_payload_transport/<place>/main.omg
omega --check samples/cli/facts/case_payload_transport/<place>/invalid/main.omg
```

## Requirements and Evidence (AGENTS.md)

- **Establishment**: `Outcome::Ready { value: 32 }` satisfies the declared payload bound `value <= 50`.
- **Identity and Lifetime**: Sum instances retain variant discriminant and payload bound facts across value copies, field storage, parameter passing, and state hops.
- **Composition**: Destructuring `Outcome::Ready { value }` introduces `value <= 50` into the branch environment, which satisfies the `ensures result <= 60` contract on `value + 10`.
- **Soundness Boundaries**: In every reject twin, the payload bound is widened to `value <= 80`. The compiler rejects checking because `value + 10` cannot prove `ensures result <= 60`.
- **Remaining Open**: Broader static machine migration across remaining standard library and runtime subsystems under `STATIC-MACHINE-MIGRATION`.
