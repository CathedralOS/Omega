The existing Product program and runtime oracles are unchanged. Its Build now executes the public `F64::negate(7.0)` operation and requires the actual result to be -7 before configuring the application and binding its four hosted entries. Build therefore needs the completed core defaults before application configuration returns.

`fail/build/float_default_wrong_value` copies the Product program and Build exactly except for changing the numeric input from 7 to 8. It must stop with `Build did not return normally: Crashed` and `cause: Trap`, before configuration. This is the surviving ordinary fixture for the Build-use counterexample; the retired float fact row is not restored.

At producer `13c6c6688a7a` on Linux, the positive Build completed configuration, while Product publication later refused a missing numeric declaration identity. The wrong-value input reached `Crashed` with `cause: Trap` before configuration. Those observations establish the Build operation at that producer; the Product program has no engine result from that failed publication.

The later declaration and storage repairs are awaiting fresh execution. All four hosted targets, final acceptance and parent-task completion remain open. The earlier `8dc` admission refusal retains its original source and inputs.
