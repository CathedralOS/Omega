"""Independent reference for the explicitly quantized acoustic sample model."""
# Independent integer oracle: floor each layer duration before accumulation.
def reference(text):
    tokens=text.split(); pos=0
    def get():
        nonlocal pos
        if pos>=len(tokens): raise ValueError
        s=tokens[pos];pos+=1
        if not s.isascii() or not s.isdecimal() or len(s)>9:raise ValueError
        return int(s)
    def error(kind):return (2,'error '+kind+'\n')
    try:
        layers,echoes,window,present,offset=[get() for _ in range(5)]
        if not(1<=layers<=4 and 1<=echoes<=8 and window<=1000 and offset<=1000000 and present<=1):return error('invalid_header')
        if present!=1:return error('missing_reference')
        model=[]
        for _ in range(layers):
            speed,thickness=get(),get()
            if not(1000<=speed<=10000 and 1<=thickness<=1000000):return error('invalid_layer')
            model.append((speed,thickness))
        arrivals=[]
        for _ in range(echoes):
            arrival=get()
            if not(offset<=arrival<=9000000):return error('invalid_echo')
            arrivals.append(arrival)
        if pos!=len(tokens):return error('malformed_input')
    except ValueError:return error('malformed_input')
    cumulative=0; paths=[]
    for layer,(speed,thickness) in enumerate(model,1):
        previous=cumulative;cumulative+=thickness*2000//speed
        for order in range(1,4):paths.append((layer,order,cumulative*order,previous,speed))
    lines=['acoustic thickness echoes: ns / um / m_per_s']
    for i,arrival in enumerate(arrivals,1):
        relative=arrival-offset
        hits=[path for path in paths if abs(relative-path[2])<=window]
        if not hits: result='unmatched'
        elif len(hits)>1:result=f'ambiguous paths={len(hits)}'
        else:
            layer,order,_,previous,speed=hits[0]
            result=(f'primary layer={layer} thickness_um={(relative-previous)*speed//2000}' if order==1 else f'multiple layer={layer} order={order}')
        lines.append(f'echo {i} {result}')
    return 0,'\n'.join(lines+['complete'])+'\n'
if __name__ == '__main__':
    import sys
    code, text = reference(sys.stdin.read())
    sys.stdout.write(text)
    sys.exit(code)
