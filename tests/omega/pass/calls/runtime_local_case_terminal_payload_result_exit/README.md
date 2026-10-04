# Terminal calls read the selected local payload

Run `omega run --both main.omg`; both engines must exit 0.

A helper produces a local sum. Each case forwards its saved scalar payload to
a record-returning terminal call. The caller checks distinct values 70 and 71.
This exercises the ordinary local field observation path under case selection.
