# Arm Rest Guards

Depend-free compile-scope probe: transition-arm record patterns — the
first sample exercising `..` (the arm-only explicit opt-out from the
exhaustiveness law), `as` binding renames, and `if` guards on case arms.

- `Signal::Hop { dx as step, .. } if step == 30` renames the payload
  binding, waives `dy`, and guards on the renamed value — a partial
  pattern that refuses without `..` compiles with it.
- The same case admits a second, fuller arm below it: arms are tried in
  order, so `Hop { dx, dy }` catches every Hop the guarded arm declined.
- Payloads ride on a scalar domain (`i32 in Step`, 0..=60): domain
  members add through the ordinary `+` and produce plain i32.

Compile-checked only (no std dependency, no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/arm_rest_guards/main.omg
mbx run -p omega -- update --project samples/cli/probes/arm_rest_guards
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.
