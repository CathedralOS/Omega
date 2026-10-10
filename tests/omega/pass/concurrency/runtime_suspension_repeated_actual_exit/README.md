# Repeated actuals at a possibly suspending call

Run `omega run --both tests/omega/pass/concurrency/runtime_suspension_repeated_actual_exit/main.omg`.
Both executions must exit 0 after the selected provider returns 42.

Two scalar argument positions read the same scalar binding, and two structural
positions copy the same unrestricted marker. The marker remains usable after
the call. The suspension frontier retains distinct value occurrences even
when physical storage, type and carry policy agree. Canonical ordering must preserve each occurrence's demand,
not reject or merge the repeated actual. The selected provider completes inline;
this witnesses ordinary call composition, not park/resume or loan admission.
