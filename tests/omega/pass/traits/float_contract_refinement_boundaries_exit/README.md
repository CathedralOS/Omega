The existing Product program and runtime oracles are unchanged. Its Build now executes the public `F64::negate(7.0)` operation and requires the actual result to be -7 before configuring the application and binding its four hosted entries. Build therefore needs the completed core defaults before application configuration returns.

`fail/build/float_default_wrong_value` copies the Product program and Build exactly except for changing the numeric input from 7 to 8. It must stop with `Build did not return normally: Crashed` and `cause: Trap`, before configuration. This is the surviving ordinary fixture for the Build-use counterexample; the retired float fact row is not restored.

The previous 8dc producer refused during selected external-provider admission. Current-source success, the wrong-value cause and all four hosted targets remain unverified. Source history and ignored qualification records retain the original programs and actual observations.
