#!/usr/bin/env python3
# io-verify.py PROGRAM.beta EXE — EXHAUSTIVE all-inputs check for a single-byte-reading program: for every
# input byte 0..255, compare the reference interpreter's (exit, stdout) against the compiled binary's. Prints
# nothing and exits 0 on complete agreement; on the first mismatch prints it and exits 1. This upgrades the
# correctness guarantee from "random fixed inputs" to "verified over the WHOLE bounded input domain".
import sys, os, subprocess
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from beta_parser import lex, Parser
from beta_interp import interpret

def main():
    prog, exe = sys.argv[1], sys.argv[2]
    with open(prog) as f:
        procs = Parser(lex(f.read())).parse()
    for b in range(256):
        stdin = bytes([b])
        iexit, iout = interpret(procs, stdin)
        r = subprocess.run([exe], input=stdin, capture_output=True)
        cexit, cout = r.returncode, r.stdout
        if iexit != cexit or iout != cout:
            print(f"  MISMATCH input byte {b}: interp=(exit={iexit} out={iout!r})  compiled=(exit={cexit} out={cout!r})")
            sys.exit(1)
    sys.exit(0)

main()
