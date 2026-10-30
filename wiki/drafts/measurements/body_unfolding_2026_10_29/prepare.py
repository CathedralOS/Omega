from pathlib import Path
out=Path('build/body-unfolding')
profile=Path('build/remaining-first-checking/diagnostic-profile.py').read_text().replace('build/remaining-first-checking','build/body-unfolding')
(out/'diagnostic-profile.py').write_text(profile)
chain=Path('build/remaining-first-checking/diagnostic-chain.py').read_text().replace('build/remaining-first-checking','build/body-unfolding')
chain=chain[:chain.index('profile=(out/')]+'''subprocess.run([sys.executable,str(out/'diagnostic-profile.py')],check=True)
assert subprocess.check_output(['git','diff','HEAD','--','omega-rust'])==patch
'''
(out/'diagnostic-chain.py').write_text(chain)
