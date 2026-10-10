A machine entry requires value == 1, but passes 2 into a recurrent named
state with a distinct value formal. The state's call to accept cannot use the
machine entry fact. Its fuel countdown is valid independently, so rejection
must identify accept's unsatisfied Requires rather than a ranking failure.

This exact source has SHA256
cec20132c4387b2c204dbef30d7f02c9b6b9b02fe011fe5dec75ade1352483fb.
Frozen revision 7e8e991e06620bc899061e98fd769c9b1629d14b rejected it at
that Requires contract. Check only; this invalid program is never executed.
