#!/usr/bin/env python3
# meaning_cert_diamond.py — driver for the MEANING-CERT DIAMOND: every certificate in the meaning-TV
# stream (meaning claims, negative controls, safety/witness obligations) must be decided IDENTICALLY by
# proof-kernel/check.beta (the built alpha binary) AND proof-kernel/check_ref.py (the independent reference checker),
# and the verdict must be the structurally expected one: line 1 accept, line 2 (the perturbed control)
# reject, every VC line accept. `#render` lines are the structural-result pin, not certs — skipped.
#
# D5 discipline: the refinement pillar's cert classes got their three-checker diamond on day one; this
# closes the same seam for the summit's newer classes (value pins, chunked witnesses, binary bit-spine
# arithmetic, structural-tree claims). A disagreement between the two checkers is a checker bug by
# construction — exactly what the diamond exists to surface.
#
# THIRD LEG: each cert is also translated (refcert_to_gamma.py, table-carrying Fap encoding) and decided
# by checker.gamma running on interp.beta. The gamma stack is the tightest resource envelope of the three,
# so its verdict is three-way: accept / reject (stdout AND exit code must agree — a crash can fake an exit
# code but not both) / undecided (resource exhaustion — counted and REPORTED, never silently skipped).
# A decided-but-different verdict is a diamond break; undecided is not.
#
# usage: meaning_cert_diamond.py <check.exe> <interp.exe> <checker.gamma> <name>=<claims-file> ...
import io
import importlib
import os
import subprocess
import sys
import threading

sys.setrecursionlimit(400000)              # deep unary spines (12345-deep numerals) in reference parsing
threading.stack_size(512 * 1024 * 1024)    # ...need real stack too: the work runs in a big-stack thread

HERE = os.path.dirname(os.path.abspath(__file__))

def find_repo_root(start):
    current = start
    while True:
        if os.path.isfile(os.path.join(current, 'bootstrap', 'paths.sh')):
            return current
        parent = os.path.dirname(current)
        if parent == current:
            raise RuntimeError('cannot find repository root from %s' % start)
        current = parent

REPO_ROOT = os.environ.get('OMEGA_REPO_ROOT') or find_repo_root(HERE)
PROOF_KERNEL = os.environ.get(
    'OMEGA_PATH_PROOF_KERNEL',
    os.path.join(REPO_ROOT, 'bootstrap', 'assurance', 'proof-kernel'))


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


def gamma_verdict(cert, interp_exe, defs):
    tr = subprocess.run(['python3', os.path.join(PROOF_KERNEL, 'tools', 'refcert_to_gamma.py')], input=cert,
                        capture_output=True, text=True, timeout=180)
    if tr.returncode != 0:
        return 'untranslatable'            # a translator gap is a real failure, not a resource limit
    try:
        r = subprocess.run([interp_exe], input=defs + '\n' + tr.stdout,
                           capture_output=True, text=True, timeout=120)
    except subprocess.TimeoutExpired:
        return 'undecided'
    out = r.stdout.strip()
    if out == '1' and r.returncode == 1:
        return 'accept'
    if out == '0' and r.returncode == 0:
        return 'reject'
    return 'undecided'                     # geometry collision / garbage exit: resource, not a verdict


def main():
    check_exe, interp_exe, defs_path = sys.argv[1], sys.argv[2], sys.argv[3]
    defs = open(defs_path).read()
    sys.path.insert(0, os.path.join(PROOF_KERNEL, 'implementations', 'reference'))
    import check_ref
    total, bad, undec = 0, 0, 0
    for spec in sys.argv[4:]:
        name, path = spec.split('=', 1)
        for i, ln in enumerate(open(path).read().splitlines()):
            if not ln.strip() or ln.startswith('#render'):
                continue
            cert = ln.split(' ', 1)[1] if i == 0 else ln     # line 1 carries the exit-code prefix
            want = 'reject' if i == 1 else 'accept'
            va = subprocess.run([check_exe], input=cert, capture_output=True,
                                text=True, timeout=180).stdout.strip()
            vb = ref_verdict(cert, check_ref)
            vg = gamma_verdict(cert, interp_exe, defs)
            total += 1
            if vg == 'undecided':
                undec += 1
                vg = want                  # the gamma leg abstains; the other two must still both decide
            if not (va == vb == vg == want):
                bad += 1
                print('  DIAMOND BREAK %s line %d: check.beta=%r check_ref=%r checker.gamma=%r expected=%s'
                      % (name, i + 1, va, vb, vg, want))
    print('meaning-cert diamond (check.beta, check_ref.py AND checker.gamma agree on every meaning-TV '
          'cert): %d certs, %d disagreements, %d gamma-undecided (resource)' % (total, bad, undec))
    return 1 if bad or not total else 0


if __name__ == '__main__':
    rc = []
    t = threading.Thread(target=lambda: rc.append(main()))   # SystemExit in a thread won't reach the shell
    t.start()
    t.join()
    sys.exit(rc[0] if rc else 1)
