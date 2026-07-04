#!/usr/bin/env python3
# meaning_cert_diamond.py — driver for the MEANING-CERT DIAMOND: every certificate in the meaning-TV
# stream (meaning claims, negative controls, safety/witness obligations) must be decided IDENTICALLY by
# delta/check.beta (the built alpha binary) AND delta/check_ref.py (the independent reference checker),
# and the verdict must be the structurally expected one: line 1 accept, line 2 (the perturbed control)
# reject, every VC line accept. `#render` lines are the structural-result pin, not certs — skipped.
#
# D5 discipline: the refinement pillar's cert classes got their three-checker diamond on day one; this
# closes the same seam for the summit's newer classes (value pins, chunked witnesses, binary bit-spine
# arithmetic, structural-tree claims). A disagreement between the two checkers is a checker bug by
# construction — exactly what the diamond exists to surface.
#
# usage: meaning_cert_diamond.py <check.exe> <name>=<claims-file> ...
import io
import importlib
import subprocess
import sys
import threading

sys.setrecursionlimit(400000)              # deep unary spines (12345-deep numerals) in reference parsing
threading.stack_size(512 * 1024 * 1024)    # ...need real stack too: the work runs in a big-stack thread


def ref_verdict(cert, check_ref):
    importlib.reload(check_ref)            # module-level DATA/LEMMAS state: fresh per certificate
    old_in, old_out = sys.stdin, sys.stdout
    sys.stdin, sys.stdout = io.StringIO(cert), io.StringIO()
    try:
        check_ref.main()
        out = sys.stdout.getvalue().strip()
    except Exception:
        out = 'error'
    finally:
        sys.stdin, sys.stdout = old_in, old_out
    return out


def main():
    check_exe = sys.argv[1]
    sys.path.insert(0, '../delta')
    import check_ref
    total, bad = 0, 0
    for spec in sys.argv[2:]:
        name, path = spec.split('=', 1)
        for i, ln in enumerate(open(path).read().splitlines()):
            if not ln.strip() or ln.startswith('#render'):
                continue
            cert = ln.split(' ', 1)[1] if i == 0 else ln     # line 1 carries the exit-code prefix
            want = 'reject' if i == 1 else 'accept'
            va = subprocess.run([check_exe], input=cert, capture_output=True,
                                text=True, timeout=180).stdout.strip()
            vb = ref_verdict(cert, check_ref)
            total += 1
            if not (va == vb == want):
                bad += 1
                print('  DIAMOND BREAK %s line %d: check.beta=%r check_ref=%r expected=%s'
                      % (name, i + 1, va, vb, want))
    print('meaning-cert diamond (check.beta AND check_ref.py agree on every meaning-TV cert): '
          '%d certs, %d disagreements' % (total, bad))
    return 1 if bad or not total else 0


if __name__ == '__main__':
    rc = []
    t = threading.Thread(target=lambda: rc.append(main()))   # SystemExit in a thread won't reach the shell
    t.start()
    t.join()
    sys.exit(rc[0] if rc else 1)
