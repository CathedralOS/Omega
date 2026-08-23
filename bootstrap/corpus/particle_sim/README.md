# Particle Simulation

A 5-step Euler-integration particle simulation using f64 arithmetic.
Stresses the full float codegen path: field-to-field arithmetic, float
literal operands, and f64 comparison guards.

Physics: `position += velocity; velocity += acceleration` each step.
Starting from `position=0.0, velocity=1.0, acceleration=0.5`, after 5
steps `position == 10.0`. The guard asserts `9.5 < position < 10.5` and
exits **70**.

```
omega --target windows_x64 --build-dir build samples/particle_sim/main.omg
./build/omega-program.exe   # exit 70
```

Exercises: f64 field arithmetic (`addsd`/`subsd`), float comparison guards
(`ucomisd`), `[copy]` aggregate property, dispatched sub-machine
calls that mutate `self` fields.
