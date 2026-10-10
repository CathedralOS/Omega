# A skipped transfer retains cleanup prerequisites

The selected call prepares its consumed token for cleanup. On the skipped path,
the original token remains unprepared and still owes its nominal cleanup.
Checking must reject the missing `retained.ready == true` prerequisite instead
of treating a possible transfer as an unconditional disposal.
