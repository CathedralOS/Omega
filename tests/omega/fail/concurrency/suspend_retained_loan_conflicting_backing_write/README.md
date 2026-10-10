# Suspension retains access exclusion

Run `omega --check tests/omega/fail/concurrency/suspend_retained_loan_conflicting_backing_write/main.omg`.
The retained shared loan must continue excluding writes to its original backing
after the possibly suspending call. Suspension neither releases the loan nor
restores parent mutation authority before its final observation.
