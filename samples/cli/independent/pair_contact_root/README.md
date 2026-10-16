# Pair contact root

Small witness of the quadratic contact record used by `disk_collision_lab`.
Two copy disk records are passed by value. The contact machine returns a
copy `Contact` record through its states: a runtime discriminant goes through
`F64::square_root`, the smaller root is the time, and the Newton impulse is
the second field. This fixture is the head-on case with masses 1 and 3 whose
first contact is at time 2 with impulse 1.5.

Closed stdin is enough; the program reads nothing.

```sh
omega run samples/cli/independent/pair_contact_root/main.omg < /dev/null
omega --check samples/cli/independent/pair_contact_root/invalid/main.omg
```

Expected exit 0 and stdout containing `pair contact root 2 impulse 1.5`. The invalid
control indexes a two-slot ledger at 2 and must reject with `cannot prove index`.
