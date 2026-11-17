import sys, re, html

import markdown
import os
HERE=os.path.dirname(os.path.abspath(__file__))
PLAN=os.path.join(HERE,'..','..','plans','foundation-and-backend.md')
md=open(PLAN).read()
# drop the H1 and intro paragraph and source line; we render our own header
body=md.split('## 0. Method',1)[1]
body='## 0. Method'+body
body=body.replace('\n# Part A','\n## Part A').replace('\n# Part B','\n## Part B').replace('\n# Owner questions','\n## Owner questions')
h=markdown.markdown(body, extensions=['tables','sane_lists'])
# section ids
def sec(m):
    text=re.sub('<[^>]+>','',m.group(1))
    slug=re.sub(r'[^a-z0-9]+','-',text.lower()).strip('-')
    return f'<h2 id="{slug}">{m.group(1)}</h2>'
h=re.sub(r'<h2>(.*?)</h2>',sec,h)
h=re.sub(r'<table>',r'<div class="scroll"><table>',h)
h=h.replace('</table>','</table></div>')
chips={'primitive':'infra','split':'split','stage output':'out','delete':'split'}
def chip(m):
    t=m.group(1); low=re.sub('<[^>]+>','',t).lower()
    for k,c in chips.items():
        if low.startswith(k): return f'<span class="chip {c}">{t}</span>'
    if low.startswith('move') or low.startswith('stage output'): return f'<span class="chip move">{t}</span>'
    return f'<strong>{t}</strong>'
h=re.sub(r'<strong>(.*?)</strong>', chip, h)
h=h.replace('<em>judgment</em>','<span class="flag">judgment</span>').replace('<em>unverified</em>','<span class="flag warn">unverified</span>')
toc=re.findall(r'<h2 id="([^"]+)">(.*?)</h2>',h)
tochtml='\n'.join(f'<li><a href="#{i}">{re.sub("<[^>]+>","",t)}</a></li>' for i,t in toc)
page=open(os.path.join(HERE,'fplan_template.html')).read().replace('%%TOC%%',tochtml).replace('%%BODY%%',h)
os.makedirs(os.path.join(HERE,'out'),exist_ok=True)
open(os.path.join(HERE,'out','index.html'),'w').write(page)
print(len(page), [t for _,t in toc])
