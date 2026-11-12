# Consuming replacement of a borrowed sum

```sh
omega run --both tests/omega/pass/ownership/runtime_borrowed_sum_retag_exit/main.omg
```

Both engines exit 0. `choice = transform(choice, increment)` extracts the
owned actual before invoking the transformer and restores its returned value
to the captured caller storage. The two calls retain a failure payload and
then retag it as success, changing 41 to 42 without changing the holder's marker.
The moved payload is consumed once; the vacant destination owes no second drop.
