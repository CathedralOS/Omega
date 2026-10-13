"""Review-driven addendum; original frozen oracle remains byte-identical.
Freeze a nontrivial unique equilibrium partition analytically, separately from
both production's event solution and the independent enthalpy integration.
"""
import hashlib,json
from fractions import Fraction
from pathlib import Path
from reference import solve
ROOT=Path(__file__).resolve().parents[1]
text='1 2 100 20 -20 2 3 60 40 110 2 8 0 0.1 1 3 5 10 30 1000\n'
# If body 2 is melting at 40, body 1 is liquid: h1=100+2*(40-20).
# The initial energy is -20+2*110=200, hence h2=(200-140)/2=30.
# 0<h2<60 proves this phase partition is self-consistent and unique.
energy=Fraction(-20)+2*Fraction(110)
h1=Fraction(100)+2*(Fraction(40)-20);h2=(energy-h1)/2
assert 0<h2<60
analytic=dict(temperature1=40,temperature2=40,enthalpy1=float(h1),enthalpy2=float(h2),fraction1=1,fraction2=float(h2/60),transferred_energy=float(Fraction(-20)-h1))
rows=solve(text,2e-13);coarse=solve(text,2e-12)
for a,b in zip(rows,coarse):
    for key in a:assert abs(a[key]-b[key])<2e-8*max(1,abs(a[key]))
for key,wanted in analytic.items():assert abs(rows[-1][key]-wanted)<1e-8,(key,rows[-1][key],wanted)
(ROOT/'data/plateau_partition.txt').write_text(text)
case=dict(input_sha256=hashlib.sha256(text.encode()).hexdigest(),rows=rows,analytic_equilibrium=analytic,equilibrium_temperature=40,energy_scale=240)
(ROOT/'validation/supplement_oracle.json').write_text(json.dumps(dict(cases=dict(plateau_partition=case)),indent=2)+'\n')
files=[Path(__file__),ROOT/'validation/supplement_oracle.json',ROOT/'data/plateau_partition.txt']
(ROOT/'validation/SUPPLEMENT_FREEZE.json').write_text(json.dumps({str(p.relative_to(ROOT)):hashlib.sha256(p.read_bytes()).hexdigest() for p in files},indent=2)+'\n')
