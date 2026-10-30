import sys
sys.path.insert(0,'build/proof-profile')
import terminal_breakdown_run as h
h.finish('term-cold-a','timing')
h.run('terminal-breakdown','term-warm-a','timing',warm_from='term-cold-a')
for letter in 'bc':
 h.run('terminal-breakdown','term-cold-'+letter,'timing',True)
 h.run('terminal-breakdown','term-warm-'+letter,'timing',True,warm_from='term-cold-'+letter)
 if letter=='b':h.run('baseline','term-original-b');h.run('terminal-breakdown','term-off-b')
h.run('terminal-breakdown','term-counts-cold','counts')
h.run('terminal-breakdown','term-counts-warm','counts',warm_from='term-counts-cold')
h.run('terminal-breakdown','term-verified',comparison=True)
