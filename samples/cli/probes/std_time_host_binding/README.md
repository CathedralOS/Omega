# std_time_host_binding

`Time::now` and a 50ms `Time::sleep_for` run through the selected raw time-host boundary traits. The elapsed duration is at least 50ms on the interpreter's virtual clock and on the host clock.

Expected exit: 70.
