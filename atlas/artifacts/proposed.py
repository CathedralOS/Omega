"""Proposed stages 00–10 carry inputs through one forward chain.

These are design contracts, not observed implementation boundaries. Compiler
work stays inside its stage; runtime execution consumes completed products.
"""
from .overview import validate
from .proposed_operations import unfold

PLAN_REVISION = 'd6ea87967106d47eb007b4780c7df2974de8fe37'
PLAN_PATH = 'atlas/plans/foundation-and-backend.md'


def proposed_model():
    # Each row is an owner followed by its complete result, on the same spine.
    contracts=[
        ('admission','Admit inputs','inputs','admitted','Admitted inputs',
         'Record acquired source/package custody, scoped roots, Build/configuration requests, target/component/provider/loader/output inputs, selections, budgets and staged receipts. Values requiring evaluation are produced in 01.',
         'Source inputs and the requests needed later, carried in one compilation record.',
         'Specify complete input fields and authority without activating unused source.'),
        ('source-settlement','Compile source + values','source','accepted','Accepted program + values',
         'Own lazy syntax, resolution, typing, preparation, lowering and hermetic evaluation. The lowered-form checker gives the sole semantic verdict. Preserve independent certificate admission and the specified optimization/checker order.',
         'Checked candidate bodies, computed values, neutral selection records, invocation receipts and provenance, with downstream requests.',
         'Prove activation/error order, invocation admission, value isolation and charging. Replace old APIs with private operations, not a wrapper.'),
        ('seal','Seal portable program','source','portable','Psi + carried inputs',
         'Publish canonical target-neutral Psi from accepted code. Forward remaining inputs in their declared compilation record; native configuration does not become portable language semantics.',
         'Canonical checked code and its catalog, evidence and summaries, with the records needed downstream. No source owner or lookup callback.',
         'Define acyclic type ownership: Psi sees only neutral projections and never imports Omega target/ABI types.'),
        ('native-planning','Plan native code','native','selected-program','Selected native program',
         'Join carried Build selections to the checked catalog. Compute and validate calling policies inside this stage. Close retained native demand, call/ABI plans and generated-body recipes without source lookup.',
         'Retained bodies, ABI plans, obligations and generated-body recipes; carry target, installation and output records forward.',
         'Prove body-demand closure and exact policy admission/results, preserving optimization-pruned unsupported targets.'),
        ('physical-planning','Build physical program','native','physical-program','Complete physical program',
         'Represent ordinary and generated bodies together, including handlers, entries and adapters. Own register choices, frames, effects and symbolic resource uses.',
         'All executable bodies, register/effect/frame records and resource dependencies. Handler analysis needs no object-emission detour.',
         'Prove complete generated-body effects, caller constraints and finite resource ordering.'),
        ('binding','Bind resources','native','bound-program','Bound physical program',
         'Use complete physical resource summaries and carried installation limits. Check transitive stack, bridge storage, call bias, alignment, fault premises and placement identities.',
         'Complete physical code with bound resources and installation identities; preserve encoding, format, loader and output requests.',
         'Binding must not change allocation, effects, frames or body membership.'),
        ('encoding','Encode all code','encoding','encoded-program','Encoded code + fixups',
         'Encode ordinary and generated bodies, including frames. Publish legal alternatives and complete fixups; imported encoded components remain admitted data.',
         'Body/frame fragments, alternatives, fixups and attribution, with placement/output facts. No earlier executable graph.',
         'Complete alternatives/fixups for both ISAs and all formats. No later executable-body producer.'),
        ('layout-stage','Lay out code','encoding','laid-out','Laid-out program',
         'Place fragments, choose carried alternatives and patch references. Preserve body/frame relaxation, ranges and alignment without an encoder callback.',
         'Resolved content, symbols and placement records with remaining object/image/output requests.',''),
        ('object-stage','Build objects','encoding','objects','Objects + image inputs',
         'Construct object containers from laid-out content and carried symbols/format requests. No source/ABI rediscovery or private helper generation.',
         'Objects, attribution, entry/evidence records and carried container, loader and binding inputs.',''),
        ('image-stage','Build image','encoding','image','Completed image',
         'Place objects and produce relocations, signatures and container/loader metadata from carried inputs. Executable code is already encoded.',
         'Final image, regions, symbols, relocation evidence and provision records, plus the publication request.',''),
        ('publish','Publish products','products','published','Published products',
         'Install completed artifacts using the carried output request and return receipts. Publication performs no compiler transformations.',
         'Finished executable/component/Psi products and receipts. Runtime execution consumes finished products after compilation; it is not a pipeline helper.',''),
    ]
    nodes=[]
    for number,(key,title,group,result,result_title,description,result_description,gate) in enumerate(contracts):
        source=f'https://github.com/CathedralOS/Omega/blob/{PLAN_REVISION}/{PLAN_PATH}#L{173+number}'
        for endpoint,label,role,detail in ((key,f'{number:02d} {title}','owner',description),(result,result_title,'artifact',result_description)):
            nodes.append(dict(id=endpoint,owner=endpoint,title=label,role=role,
                              kind='owner' if role=='owner' else ('native' if group in ('native','encoding','products') else 'data'),
                              group=group,description=detail,page='foundation-backend-plan',
                              anchor='3-recommended-destination-and-stage-contracts',source=source,
                              gate=gate if role=='owner' else '',number=number if role=='owner' else None,width=220,height=60))
    nodes,phases=unfold(nodes,PLAN_REVISION)
    spine=[n['id'] for n in nodes]
    by_id={n['id']:n for n in nodes}
    edges=[dict(id=f'proposed-{a}-{b}',start=a,end=b,label='',kind='data',main=True,source=by_id[b]['source'])
           for a,b in zip(spine,spine[1:])]
    model=dict(nodes=nodes,edges=edges,spine=spine,view='proposed',status='unverified',plan_revision=PLAN_REVISION,
               badge=f'Proposed 00–10 · plan {PLAN_REVISION[:12]}',
               intro='Proposed · unfolded. Stage frames show ownership; decimal labels show the operations inside. Source and native worklists may repeat operations. Exact scheduling remains unverified.',
               phaseGaps={n['id']:32 for n in nodes if n['role']=='owner'},
               phases=phases,placements=[])
    validate(model)
    return model
