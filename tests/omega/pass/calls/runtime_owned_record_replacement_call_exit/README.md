# Calls consume the current owned record

```sh
mbx run -p omega -- run --both tests/omega/pass/calls/runtime_owned_record_replacement_call_exit/main.omg
```

Three calls move values from one mutable record binding. Each replacement
establishes a new value in that home. Exit 0 requires observing 17, 42 and 85
in order, without treating a consumed initializer as the next call's operand.
This checks ordinary owned storage independently of encoded reference geometry.
The `ownership/read_through_consumed_owner_rejected` and
`ownership/linear_second_transfer` controls still reject unrepaired reuse.
