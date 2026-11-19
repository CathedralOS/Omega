#!/usr/bin/env python3
"""Run a stamped Alpha seed and print the POSIX-observable exit surface.

MSYS shells (Git Bash) normalize the raw process exit DWORD: the audited Trap's
STATUS_ILLEGAL_INSTRUCTION (0xC000001D) arrives as SIGILL (128+4 = 132), but
every other sign-bit-set code — including a halt whose register sign-extends
into the DWORD — observes as 127, hiding the authored byte. A subprocess run by
the Windows-native python3 sees the DWORD itself, so this route restores the
byte-level expectations on MSYS hosts (seed_env.sh's ALPHA_SEED_EXIT_COLLAPSED).

Reduction to the POSIX surface:

  0xC000001D            -> 132      SIGILL, the audited Trap mechanism
  other 0xC0......      -> raw DWORD, unsigned decimal; a foreign exception
                            (e.g. a leaked #DE as 0xC0000094) cannot alias a
                            legal expectation and stays visible in the diff
  anything else         -> code & 0xFF   the authored low byte, as wait()
                            would report it on a POSIX host

Usage: native_exit.py EXE STDIN_BIN STDOUT_BIN — feed EXE the bytes of
STDIN_BIN, write its stdout to STDOUT_BIN, print the reduced exit surface.
A case exceeding the same 300-second watchdog bounds.py uses prints
'watchdog-timeout', which cannot equal a numeric expectation.
"""

import subprocess
import sys

WATCHDOG_SECONDS = 300
STATUS_ILLEGAL_INSTRUCTION = 0xC000001D


def main(argv):
    if len(argv) != 4:
        print(__doc__, file=sys.stderr)
        return 2
    exe, stdin_path, stdout_path = argv[1:4]
    with open(stdin_path, 'rb') as handle:
        data = handle.read()
    try:
        result = subprocess.run([exe], input=data,
                                stdout=subprocess.PIPE,
                                stderr=subprocess.DEVNULL,
                                timeout=WATCHDOG_SECONDS)
    except subprocess.TimeoutExpired:
        print('watchdog-timeout')
        return 0
    with open(stdout_path, 'wb') as handle:
        handle.write(result.stdout)
    code = result.returncode & 0xFFFFFFFF
    if code == STATUS_ILLEGAL_INSTRUCTION:
        print(132)
    elif (code >> 24) == 0xC0:
        print(code)
    else:
        print(code & 0xFF)
    return 0


if __name__ == '__main__':
    sys.exit(main(sys.argv))
