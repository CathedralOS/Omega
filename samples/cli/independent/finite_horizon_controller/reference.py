"""Independent product enumeration. Freeze once before executing Omega."""
import itertools, json, pathlib, hashlib
ROOT = pathlib.Path(__file__).resolve().parent

def valid(p):
    return (1 <= p['horizon'] <= 5 and -4 <= p['lower'] <= p['upper'] <= 4
            and p['lower'] <= p['initial'] <= p['upper'] and -4 <= p['target'] <= 4
            and -1 <= p['action_lower'] <= p['action_upper'] <= 1
            and -1 <= p['drift'] <= 1 and all(0 <= p[k] <= 9 for k in
            ['state_weight','action_weight','terminal_weight']))

def oracle(p):
    if not valid(p): return 'INVALID: problem outside supported bounds\n', 2
    boundary = terminal = 0
    candidates = []
    for actions in itertools.product((-1,0,1), repeat=p['horizon']):
        x = p['initial']; trajectory = [x]; cost = 0
        for u in actions:
            x += u + p['drift']
            if not (p['action_lower'] <= u <= p['action_upper'] and p['lower'] <= x <= p['upper']):
                boundary += 1; break
            trajectory.append(x)
            cost += p['state_weight']*(x-p['target'])**2 + p['action_weight']*u*u
        else:
            if p['exact_terminal'] and x != p['target']:
                terminal += 1; continue
            cost += p['terminal_weight']*(x-p['target'])**2
            candidates.append((cost, actions, trajectory))
    text = f"candidates={3**p['horizon']}\nfeasible={len(candidates)}\nboundary_rejected={boundary}\nterminal_rejected={terminal}\n"
    if not candidates:
        return text + 'INFEASIBLE: every sequence violates a step constraint or terminal equality\n', 3
    cost, actions, trajectory = min(candidates)
    text += f'cost={cost}\ninitial={trajectory[0]}\n'
    for t,u in enumerate(actions):
        text += f'step={t} action={u} state={trajectory[t+1]}\n'
    return text + 'OPTIMAL: lexicographically smallest minimum-cost sequence\n', 0

if __name__ == '__main__':
    cases_bytes = (ROOT/'data/cases.json').read_bytes()
    cases = json.loads(cases_bytes)
    manifest = {'arithmetic':'exact integers', 'absolute_tolerance':0, 'relative_tolerance':0,
                'fixture_sha256':hashlib.sha256(cases_bytes).hexdigest(), 'cases':[]}
    for i,p in enumerate(cases):
        text, status = oracle(p)
        (ROOT/f'data/expected/{i}.stdout').write_text('IND07 finite horizon controller: select 0..7\n'+text)
        (ROOT/f'data/{i}.stdin').write_text(f'{i}\n')
        manifest['cases'].append({'selector':i,'name':p['name'],'exit':status})
    for name,value in [('unknown','9\n'),('malformed','00\n'),('empty',''),('blank','\n')]:
        (ROOT/f'data/{name}.stdin').write_text(value)
        (ROOT/f'data/expected/{name}.stdout').write_text('IND07 finite horizon controller: select 0..7\nINVALID: expected one selector digit and LF\n')
        manifest['cases'].append({'selector':name,'exit':2})
    (ROOT/'data/oracle.json').write_text(json.dumps(manifest,indent=2)+'\n')
