import argparse, subprocess, sys
p=argparse.ArgumentParser();p.add_argument('--cli',required=True);p.add_argument('--source',required=True);p.add_argument('--reject',action='store_true');a=p.parse_args()
command=[a.cli,'--check',a.source] if a.reject else [a.cli,'run','--both',a.source]
r=subprocess.run(command,stdin=subprocess.DEVNULL,capture_output=True,text=True,timeout=65)
print(r.stdout,end='');print(r.stderr,end='',file=sys.stderr)
if a.reject:
    text=r.stdout+r.stderr
    valid=r.returncode!=0 and 'cannot prove requires contract' in text and 'consume' in text
    print('fact-sensitive reject verified' if valid else 'expected consume code-7 requirement refusal was not observed',file=sys.stderr)
    sys.exit(0 if valid else 3)
if r.returncode:sys.exit(r.returncode)
if 'native exit 0' not in r.stdout or 'interp exit: 0' not in r.stdout:
    print('missing required actual native and Terminal exit-zero observations',file=sys.stderr);sys.exit(4)
