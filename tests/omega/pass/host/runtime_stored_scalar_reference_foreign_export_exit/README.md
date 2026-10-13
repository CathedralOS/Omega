# Stored scalar reference export

Run `omega run main.omg` from this directory on macOS or Linux. The program
writes `AB` followed by a newline and exits 0.

One reference lives after padding in a record; another is selected through a
runtime-indexed record array. Helpers forward both to a bodyless byte-output
boundary. Export must load the selected descriptor's pointer, not pass its stack
slot, and preserve the original slot offset and array selector in the retained
memory footprint. The source bytes and neighboring padding remain unchanged.
