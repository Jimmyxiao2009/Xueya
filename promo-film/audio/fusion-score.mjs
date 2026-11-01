/** Original instrumental: warm keys, arpeggio, bass and light percussion. No voice or samples. */
import fs from 'node:fs';
const SR=48000,D=59,N=SR*D,L=new Float32Array(N),R=new Float32Array(N),beat=60/104;
let seed=29;const rng=()=>{seed=(Math.imul(seed,1664525)+1013904223)>>>0;return seed/4294967296*2-1};
const hz=n=>440*2**((n-69)/12);
function add(start,len,gain,pan,fn){let a=Math.round(start*SR),count=Math.min(Math.round(len*SR),N-a);for(let j=0;j<count;j++){let t=j/SR,v=fn(t,j)*gain;L[a+j]+=v*Math.sqrt((1-pan)/2);R[a+j]+=v*Math.sqrt((1+pan)/2)}}
function key(start,n,g=.12,pan=0,len=2.5){let f=hz(n);add(start,len,g,pan,t=>{let env=Math.min(1,t/.008)*Math.exp(-t*2.7)*(1-Math.max(0,(t-len+.15)/.15));return env*(Math.sin(2*Math.PI*f*t)+.28*Math.sin(2*Math.PI*f*2*t)*Math.exp(-t*2)+.12*Math.sin(2*Math.PI*f*3*t)*Math.exp(-t*4))});if(g>.05)key(start+.21,n,g*.22,-pan,.9)}
const chords=[[50,57,62,66],[47,54,59,62],[43,50,55,59],[45,52,57,61]];
for(let bar=0;bar*beat*4<56;bar++){let start=bar*beat*4,ch=chords[bar%4];let level=start>=20&&start<41?.70:1;
// Soft sustained harmony, with a deliberately slow attack.
for(let j=0;j<ch.length;j++){let f=hz(ch[j]);add(start,beat*4+.5,.032*level,(j-1.5)/3,t=>{let e=Math.min(1,t/.28)*Math.min(1,Math.max(0,(beat*4+.5-t)/.7));return e*(Math.sin(2*Math.PI*f*t)+.32*Math.sin(2*Math.PI*f*1.003*t))})}
for(let b=0;b<8;b++){let t=start+b*beat/2;if(t>55)continue;let notes=[ch[0]+12,ch[2]+12,ch[1]+12,ch[3]+12,ch[2]+12,ch[1]+12,ch[3]+12,ch[2]+12];key(t,notes[b],.085*level,b%2? .25:-.25,1.6)}
if(start>3){key(start,ch[0]-12,.14,0,1.7);key(start+2*beat,ch[0]-12,.09,0,1.2);}
for(let b=0;b<4;b++){let t=start+b*beat;if(t<4||t>54)continue;
if(b===0||b===2)add(t,.24,.2*level,0,x=>Math.sin(2*Math.PI*(48*x+3.2*(1-Math.exp(-x*30))))*Math.exp(-x*19));
if(b===1||b===3)add(t,.11,.055*level,.05,x=>rng()*Math.exp(-x*55));
add(t+beat/2,.055,.025*level,-.3,x=>rng()*Math.exp(-x*100));}}
// Clear resolution under the final Chinese title.
[50,57,62,66,69].forEach((n,i)=>key(54.4+i*.09,n,.12,(i-2)/4,4.4));
let peak=0;for(let i=0;i<N;i++){let t=i/SR,g=Math.min(1,t/.6)*Math.min(1,(59-t)/1.4);L[i]=Math.tanh(L[i]*1.1)*g;R[i]=Math.tanh(R[i]*1.1)*g;peak=Math.max(peak,Math.abs(L[i]),Math.abs(R[i]))}let norm=.8/peak;
let out=Buffer.alloc(44+N*4);out.write('RIFF');out.writeUInt32LE(36+N*4,4);out.write('WAVE',8);out.write('fmt ',12);out.writeUInt32LE(16,16);out.writeUInt16LE(1,20);out.writeUInt16LE(2,22);out.writeUInt32LE(SR,24);out.writeUInt32LE(SR*4,28);out.writeUInt16LE(4,32);out.writeUInt16LE(16,34);out.write('data',36);out.writeUInt32LE(N*4,40);for(let i=0;i<N;i++){out.writeInt16LE(Math.round(L[i]*norm*32767),44+i*4);out.writeInt16LE(Math.round(R[i]*norm*32767),46+i*4)}fs.writeFileSync('audio/fusion-score.wav',out);console.log('59-second instrumental written.');
