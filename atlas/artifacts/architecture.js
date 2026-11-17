(() => {
  const root = document.querySelector('.architecture');
  if (!root) return;
  const model = JSON.parse(root.querySelector('#architecture-model').textContent);
  const viewport = root.querySelector('.arch-viewport'), svg = root.querySelector('.arch-svg');
  const canvas = root.querySelector('.arch-canvas'), ctx = canvas.getContext('2d');
  const mini = root.querySelector('.arch-minimap canvas'), miniCtx = mini.getContext('2d');
  if (!ctx || !miniCtx) return;
  const detail = root.querySelector('.arch-detail'), jump = root.querySelector('.arch-jump select');
  const camera = {x:0, y:0, scale:1}, pointers = new Map();
  const byId = new Map(model.nodes.map(n => [n.id,n]));
  const paths = new Map([...svg.querySelectorAll('.arch-edge')].map(g => {
    const p=g.querySelector('path'), length=p.getTotalLength();
    return [g.dataset.edge, {path:new Path2D(p.getAttribute('d')), end:p.getPointAtLength(length), before:p.getPointAtLength(Math.max(0,length-2))}];
  }));
  const rows = [...svg.querySelectorAll('.arch-row-label')].map(t=>({x:+t.getAttribute('x'),y:+t.getAttribute('y'),text:t.textContent}));
  const lines = new Map([...svg.querySelectorAll('.arch-node')].map(g=>[g.dataset.node,[...g.querySelectorAll('text')].map(t=>({y:+t.getAttribute('y'),text:t.textContent}))]));
  const dashes = {data:[],invoke:[8,5],dependency:[2,5],construction:[12,4,2,4],validation:[2,7],normative:[12,7],bypass:[6,4]};
  let selected=null, moved=false, frame=0, size={w:0,h:0}, allRelations=false;
  const make = (tag,text) => {const e=document.createElement(tag);if(text)e.textContent=text;return e;};
  const dimensions = () => ({w:viewport.clientWidth,h:viewport.clientHeight});
  const color = name => getComputedStyle(root).getPropertyValue('--'+name).trim();
  const invalidate = () => {if(!frame)frame=requestAnimationFrame(draw);};
  const cameraChanged = () => {root.querySelector('.arch-scale').textContent=Math.round(camera.scale*100)+'%';invalidate();};
  const fit = () => {
    const {w,h}=dimensions();camera.scale=Math.min(w/model.width,h/model.height)*.97;
    camera.x=(model.width-w/camera.scale)/2;camera.y=(model.height-h/camera.scale)/2;cameraChanged();
  };
  const reset = () => {clear();camera.scale=1;camera.x=0;camera.y=0;cameraChanged();};
  const zoom = (scale,px,py) => {
    const {w,h}=dimensions();px??=w/2;py??=h/2;
    const x=camera.x+px/camera.scale,y=camera.y+py/camera.scale;
    camera.scale=Math.max(.15,Math.min(3,scale));camera.x=x-px/camera.scale;camera.y=y-py/camera.scale;cameraChanged();
  };
  function prepare(context,element,w,h) {
    const dpr=devicePixelRatio||1, width=Math.round(w*dpr),height=Math.round(h*dpr);
    if(element.width!==width||element.height!==height){element.width=width;element.height=height;}
    context.setTransform(dpr,0,0,dpr,0,0);context.clearRect(0,0,w,h);
  }
  function draw() {
    frame=0;const {w,h}=dimensions();if(!w||!h)return;
    prepare(ctx,canvas,w,h);
    const ink=color('ink'),muted=color('ink-muted'),ground=color('ground'),quiet=color('rule'),trap=color('trap');
    const font=getComputedStyle(root).getPropertyValue('--font-serif').trim();
    ctx.fillStyle=ground;ctx.fillRect(0,0,w,h);ctx.save();ctx.scale(camera.scale,camera.scale);ctx.translate(-camera.x,-camera.y);
    for(const e of model.edges) {
      const related=e.start===selected||e.end===selected;
      if(!selected&&!allRelations&&!e.main)continue;
      const shape=paths.get(e.id),tint=selected&&!related?quiet:e.kind==='bypass'?trap:muted;
      ctx.strokeStyle=tint;ctx.lineWidth=related?3.5:['validation','normative'].includes(e.kind)?2.5:1.5;
      ctx.setLineDash(dashes[e.kind]);ctx.stroke(shape.path);ctx.setLineDash([]);
      const angle=Math.atan2(shape.end.y-shape.before.y,shape.end.x-shape.before.x);
      ctx.save();ctx.translate(shape.end.x,shape.end.y);ctx.rotate(angle);ctx.fillStyle=tint;
      ctx.beginPath();ctx.moveTo(0,0);ctx.lineTo(-9,-4.5);ctx.lineTo(-9,4.5);ctx.closePath();ctx.fill();ctx.restore();
    }
    ctx.font='500 22px '+font;ctx.textAlign='left';ctx.textBaseline='alphabetic';
    for(const row of rows) {
      ctx.fillStyle=ground;ctx.fillRect(row.x-4,row.y-23,ctx.measureText(row.text).width+8,30);
      ctx.fillStyle=muted;ctx.fillText(row.text,row.x,row.y);
    }
    ctx.font='500 20px '+font;ctx.textAlign='center';
    for(const n of model.nodes) {
      ctx.fillStyle=color('family-'+n.family+'-surface');ctx.fillRect(n.x,n.y,n.width,n.height);
      ctx.strokeStyle=selected===n.id?ink:color('family-'+n.family);ctx.lineWidth=selected===n.id?4:2;
      ctx.strokeRect(n.x,n.y,n.width,n.height);ctx.fillStyle=color('family-'+n.family);ctx.fillRect(n.x,n.y,n.width,7);
      ctx.fillStyle=ink;for(const line of lines.get(n.id))ctx.fillText(line.text,n.x+n.width/2,n.y+line.y);
    }
    ctx.restore();
    const mw=mini.clientWidth,mh=mini.clientHeight,scale=Math.min(mw/model.width,mh/model.height);
    prepare(miniCtx,mini,mw,mh);miniCtx.fillStyle=color('surface');miniCtx.fillRect(0,0,mw,mh);
    for(const n of model.nodes){miniCtx.fillStyle=color('family-'+n.family);miniCtx.fillRect(n.x*scale,n.y*scale,n.width*scale,n.height*scale);}
    miniCtx.strokeStyle=ink;miniCtx.lineWidth=2;miniCtx.strokeRect(camera.x*scale,camera.y*scale,w/camera.scale*scale,h/camera.scale*scale);
    root.dataset.scale=String(camera.scale);root.dataset.camera=JSON.stringify(camera);
  }
  function clear(restore=false) {
    detail.hidden=true;jump.value='';root.classList.remove('has-selection');selected=null;
    if(restore)jump.focus({preventScroll:true});invalidate();
  }
  function select(id,focus=false) {
    const n=byId.get(id);if(!n)return;
    selected=id;jump.value=id;detail.hidden=false;root.classList.add('has-selection');detail.className='arch-detail family-'+n.family;
    const close=make('button','Close ×');close.type='button';close.className='arch-detail-close';close.addEventListener('click',()=>clear(true));
    const badge=make('p',model.families[n.family]);badge.className='relationship';
    const heading=make('h3',n.title.replaceAll('\n',' · '));heading.tabIndex=-1;
    detail.replaceChildren(close,badge,heading,make('p',n.description));
    const links=make('p'),page=make('a','Read subsystem'),source=make('a','Pinned source');
    const prefix=location.pathname.endsWith('/')||location.pathname.endsWith('/index.html')?'pages/':'';
    page.href=new URL(prefix+n.page+'.html',location.href).href;source.href=n.source;
    links.append(page,document.createTextNode(' · '),source);detail.append(links);
    for(const incoming of [true,false]) {
      const relations=model.edges.filter(e=>(incoming?e.end:e.start)===id);if(!relations.length)continue;
      detail.append(make('h4',incoming?'Incoming relationships':'Outgoing relationships'));const list=make('ul');
      for(const e of relations) {
        const target=byId.get(incoming?e.start:e.end),item=make('li'),button=make('button',target.title.replaceAll('\n',' · '));button.type='button';
        button.addEventListener('click',()=>select(target.id,true));
        const kind=make('span',model.relations[e.kind]);kind.className='relationship';
        const evidence=make('a','Relationship source');evidence.href=e.source;
        item.dataset.edge=e.id;item.append(kind,button,make('p',e.label),evidence);list.append(item);
      }
      detail.append(list);
    }
    const {w,h}=dimensions(),narrow=w<700;camera.scale=Math.max(camera.scale,1);
    camera.x=n.x+n.width/2-(narrow?w/2:(w-370)/2)/camera.scale;
    camera.y=n.y+n.height/2-(narrow?h*.23:h/2)/camera.scale;
    cameraChanged();if(focus)heading.focus({preventScroll:true});
  }
  root.classList.add('is-enhanced');root.querySelector('.arch-toolbar').hidden=false;
  canvas.hidden=false;svg.hidden=true;root.querySelector('.arch-minimap').hidden=false;
  // The DOM outline and grouped selector expose every node without relying on
  // canvas hit targets or a screen reader interpreting pixels.
  root.querySelectorAll('.arch-outline dt').forEach((dt,i)=>{
    const button=make('button','Focus on map');button.type='button';button.setAttribute('aria-label','Focus on '+model.nodes[i].title.replaceAll('\n',' '));
    button.addEventListener('click',()=>{root.scrollIntoView({block:'start'});select(model.nodes[i].id,true);});dt.append(document.createTextNode(' '),button);
  });
  root.querySelectorAll('[data-map]').forEach(button=>button.addEventListener('click',async()=>{
    const action=button.dataset.map;
    if(action==='fit')fit();if(action==='reset')reset();if(action==='in')zoom(camera.scale*1.3);if(action==='out')zoom(camera.scale/1.3);if(action==='natural')zoom(1);
    if(action==='fullscreen')try{if(document.fullscreenElement===root)await document.exitFullscreen();else await root.requestFullscreen();}catch{button.textContent='Fullscreen unavailable';}
  }));
  root.querySelector('.arch-all-relations').addEventListener('change',e=>{allRelations=e.target.checked;invalidate();});
  jump.addEventListener('change',()=>{if(jump.value)select(jump.value,true);});
  viewport.addEventListener('wheel',e=>{
    e.preventDefault();const box=viewport.getBoundingClientRect();
    if(e.ctrlKey||e.metaKey)zoom(camera.scale*Math.exp(-e.deltaY*.003),e.clientX-box.left,e.clientY-box.top);
    else{camera.x+=(e.shiftKey?e.deltaY:e.deltaX)/camera.scale;camera.y+=(e.shiftKey?0:e.deltaY)/camera.scale;cameraChanged();}
  },{passive:false});
  viewport.addEventListener('pointerdown',e=>{if(e.button>0)return;moved=false;pointers.set(e.pointerId,{x:e.clientX,y:e.clientY});viewport.setPointerCapture(e.pointerId);if(pointers.size>1)moved=true;});
  viewport.addEventListener('pointermove',e=>{
    if(!pointers.has(e.pointerId))return;const before=[...pointers.values()],previous=pointers.get(e.pointerId),dx=e.clientX-previous.x,dy=e.clientY-previous.y;
    if(Math.abs(dx)+Math.abs(dy)<2)return;moved=true;viewport.classList.add('is-dragging');pointers.set(e.pointerId,{x:e.clientX,y:e.clientY});
    if(pointers.size===2){const after=[...pointers.values()],distance=p=>Math.hypot(p[0].x-p[1].x,p[0].y-p[1].y),box=viewport.getBoundingClientRect(),old=distance(before);if(old)zoom(camera.scale*distance(after)/old,(after[0].x+after[1].x)/2-box.left,(after[0].y+after[1].y)/2-box.top);}
    else{camera.x-=dx/camera.scale;camera.y-=dy/camera.scale;cameraChanged();}
  });
  const release=e=>{
    if(!pointers.has(e.pointerId))return;
    pointers.delete(e.pointerId);if(pointers.size)return;viewport.classList.remove('is-dragging');
    if(e.type==='pointerup'&&!moved){const b=viewport.getBoundingClientRect(),x=camera.x+(e.clientX-b.left)/camera.scale,y=camera.y+(e.clientY-b.top)/camera.scale;const n=model.nodes.find(n=>x>=n.x&&x<=n.x+n.width&&y>=n.y&&y<=n.y+n.height);if(n)select(n.id);}
  };
  viewport.addEventListener('pointerup',release);viewport.addEventListener('pointercancel',release);
  mini.addEventListener('pointerdown',e=>{const b=mini.getBoundingClientRect(),s=Math.min(b.width/model.width,b.height/model.height),{w,h}=dimensions();camera.x=(e.clientX-b.left)/s-w/camera.scale/2;camera.y=(e.clientY-b.top)/s-h/camera.scale/2;cameraChanged();});
  viewport.addEventListener('keydown',e=>{
    const delta=80/camera.scale;
    if(e.key==='Home'){e.preventDefault();reset();return;}if(e.key.toLowerCase()==='f'){e.preventDefault();fit();return;}
    if(e.key==='+'||e.key==='='){e.preventDefault();zoom(camera.scale*1.3);return;}if(e.key==='-'){e.preventDefault();zoom(camera.scale/1.3);return;}
    const actions={ArrowLeft:[-delta,0],ArrowRight:[delta,0],ArrowUp:[0,-delta],ArrowDown:[0,delta]};
    if(actions[e.key]){e.preventDefault();camera.x+=actions[e.key][0];camera.y+=actions[e.key][1];cameraChanged();}
  });
  root.addEventListener('keydown',e=>{if(e.key==='Escape'&&!detail.hidden){e.preventDefault();clear(true);}});
  new ResizeObserver(()=>{const next=dimensions();if(size.w&&size.h){camera.x+=(size.w-next.w)/2/camera.scale;camera.y+=(size.h-next.h)/2/camera.scale;}size=next;invalidate();}).observe(viewport);
  new MutationObserver(invalidate).observe(document.documentElement,{attributes:true,attributeFilter:['data-theme']});
  matchMedia('(prefers-color-scheme: dark)').addEventListener('change',invalidate);
  window.addEventListener('resize',invalidate);
  const watchDpr=()=>matchMedia('(resolution: '+devicePixelRatio+'dppx)').addEventListener('change',()=>{invalidate();watchDpr();},{once:true});
  watchDpr();
  document.fonts.ready.then(invalidate);document.fonts.addEventListener('loadingdone',invalidate);
  document.addEventListener('fullscreenchange',()=>{root.querySelector('[data-map="fullscreen"]').textContent=document.fullscreenElement===root?'Exit fullscreen':'Fullscreen';invalidate();});
  reset();
})();
