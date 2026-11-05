The existing Product program and runtime oracles are unchanged. Its Build now executes the public `F64::negate(7.0)` operation and requires the actual result to be -7 before configuring the application and binding its four hosted entries. Build therefore needs the completed core defaults before application configuration returns.

`fail/build/float_default_wrong_value` copies the Product program and Build exactly except for changing the numeric input from 7 to 8. It must stop with `Build did not return normally: Crashed` and `cause: Trap`, before configuration. This is the surviving ordinary fixture for the Build-use counterexample; the retired float fact row is not restored.

At producer `13c6c6688a7a` on Linux, the positive Build completed configuration, while Product publication later refused a missing numeric declaration identity. The wrong-value input reached `Crashed` with `cause: Trap` before configuration. Those observations establish the Build operation at that producer; the Product program has no engine result from that failed publication.

The `69ec9f80599b` compiler on Linux and `3b8603164e96` on Windows, Mac and Linux ARM each recorded native/Terminal exit 0 for this program in both the corpus and ordinary CLI. On each host, the wrong-value control reached the intended Build trap. All five public SDK workflows also passed at `3b8603164e96` on Linux, including artifact reload and repeated-request accounting. These results retain their actual producers; they do not qualify later compiler changes or close the broader target-selection task. The earlier `8dc` admission refusal retains its original source and inputs.

At integrated source `6b1c338126fa`, the same positive again checks and produces
native/Terminal exit 0 on Linux. The unchanged wrong-value control reaches the
intended Build trap. All five SDK scopes also pass at that source, including
artifact reload, exact per-purpose Build counts and the original fuel, consent,
byte-storage and output assertions. These are actual Linux observations in
[run 42026360839](https://github.com/CathedralOS/Omega/actions/runs/42026360839)
and its [corpus replay](https://github.com/CathedralOS/Omega/actions/runs/42033929209).
The newer storage repair has separate matching-host qualification; earlier float
host observations remain attributed to their original compilers.
