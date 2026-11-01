'use strict';
const W=1920,H=1080,FPS=30,DUR=59,ctx=document.getElementById('stage').getContext('2d');
const C={bg:'#f3f1ea',ink:'#23342c',green:'#218463',pale:'#dbe9df',dark:'#102720',white:'#f7f5ec',muted:'#64766b'};
const clamp=x=>Math.max(0,Math.min(1,x)),mix=(a,b,p)=>a+(b-a)*p,ease=x=>1-(1-clamp(x))**3;
let v,marks;let videoLast=-1;
const edits=[
{a:4,b:12,key:'create',end:'createEnd',n:'01',title:'从“想学什么”，到适合自己的路线',hint:'输入主题 → 选择基础 → 设定学习深度',label:'创建路线'},
{a:12,b:16,key:'route',end:'lesson',n:'02',title:'把知识点组织成一条学习路线',hint:'已掌握、正在学、接下来学什么，一眼看清',label:'学习路线'},
{a:16,b:20,key:'lesson',end:'probe',n:'03',title:'每一课，都有明确的小目标',hint:'明确目标 · 阅读资料 · 形成解释 · 检查理解',label:'进入课程'},
{a:20,b:32,key:'probe',end:'confirm',n:'04',title:'答错时，导师会继续追问',hint:'先表达自己的想法，再沿着提示重新思考',label:'对话引导'},
{a:32,b:41,key:'confirm',end:'quiz',n:'05',title:'让学习者亲自说出理解',hint:'修正回答 → 确认概念 → 查看资料依据',label:'形成理解'},
{a:41,b:50,key:'quiz',end:'end',n:'06',title:'用练习发现误解，安排针对性复习',hint:'提交答案后，看到具体反馈与复习提示',label:'阶段检查'}];
function tx(t,x,y,size=30,color=C.ink,weight=400,align='left'){ctx.font=`${weight} ${size}px "Microsoft YaHei","Segoe UI",sans-serif`;ctx.fillStyle=color;ctx.textAlign=align;ctx.fillText(t,x,y);ctx.textAlign='left'}
function rr(x,y,w,h,r,color){ctx.beginPath();ctx.roundRect(x,y,w,h,r);ctx.fillStyle=color;ctx.fill()}
function line(x0,y0,x1,y1,p,color=C.green,w=3){ctx.strokeStyle=color;ctx.lineWidth=w;ctx.beginPath();ctx.moveTo(x0,y0);ctx.lineTo(mix(x0,x1,p),mix(y0,y1,p));ctx.stroke()}
function dot(x,y,r=7,color='#35d0a5'){ctx.fillStyle=color;ctx.beginPath();ctx.arc(x,y,r,0,Math.PI*2);ctx.fill()}
function wrap(t,x,y,max,size,color){let l='';for(const c of t){ctx.font=`400 ${size}px "Microsoft YaHei",sans-serif`;if(ctx.measureText(l+c).width>max){tx(l,x,y,size,color);l='';y+=size*1.5}l+=c;}tx(l,x,y,size,color)}
function dark(t){ctx.fillStyle=C.dark;ctx.fillRect(0,0,W,H);ctx.strokeStyle='#294539';ctx.lineWidth=1;for(let x=60;x<W;x+=120){ctx.beginPath();ctx.moveTo(x,0);ctx.lineTo(x,H);ctx.stroke()}ctx.fillStyle='rgba(16,39,32,.7)';ctx.fillRect(0,0,W,H)}
function routeAnim(t,baseY=735){const xs=[350,760,1170,1570],ys=[baseY,baseY-55,baseY+5,baseY-35],labels=['学习目标','知识路线','对话引导','练习反馈'];for(let i=0;i<4;i++){let p=ease((t-i*.45)/.5);if(i<3)line(xs[i],ys[i],xs[i+1],ys[i+1],ease((t-i*.45-.2)/.55),'#35d0a5',3);ctx.globalAlpha=p;dot(xs[i],ys[i],9);tx(labels[i],xs[i],ys[i]+52,27,C.white,400,'center');ctx.globalAlpha=1;}let u=clamp(t/2.1)*3,i=Math.min(2,Math.floor(u)),p=u-i;dot(mix(xs[i],xs[i+1],p),mix(ys[i],ys[i+1],p),5,'#fff');}
function intro(t){dark(t);tx('学涯',112,142,48,C.white,600);tx('AI 学习应用',112,192,24,'#98b7a6');let p=ease(t/.65);ctx.globalAlpha=p;tx('自学，不只需要一个答案。',112,390,72,C.white,600);ctx.globalAlpha=ease((t-.65)/.65);tx('更需要：知道怎么学，确认真正懂了。',112,475,36,'#b3d0c0');ctx.globalAlpha=1;routeAnim(t-.9);if(t>3.55){let q=ease((t-3.55)/.45);ctx.fillStyle=C.bg;ctx.fillRect(W*(1-q),0,W*q,H)}}
function cropFor(s,u){if(s.key==='create')return [300,135,860,580];if(s.key==='route'){let p=ease((u-.55)/2.5);return [mix(375,465,p),mix(0,100,p),mix(785,690,p),mix(820,630,p)]}if(s.key==='lesson'){let p=ease((u-1.5)/1.2);return [250,mix(55,0,p),950,mix(640,900,p)]}if(s.key==='probe'){let p=ease((u-7.9)/1);return [mix(395,440,p),mix(45,100,p),mix(900,830,p),mix(825,430,p)]}if(s.key==='confirm'){let p=ease((u-4.2)/1);return [mix(395,440,p),mix(45,350,p),mix(900,830,p),mix(825,380,p)]}let p=ease((u-2.5)/1.1);return [mix(395,445,p),mix(45,300,p),mix(900,825,p),mix(825,505,p)]}
function drawVideo(s,u){let [sx,sy,sw,sh]=cropFor(s,u),x=88,y=196,w=1744,h=700;rr(x,y,w,h,18,'#f8f7f4');ctx.save();ctx.beginPath();ctx.roundRect(x,y,w,h,18);ctx.clip();let k=Math.min(w/sw,h/sh),dw=sw*k,dh=sh*k;ctx.drawImage(v,sx,sy,sw,sh,x+(w-dw)/2,y+(h-dh)/2,dw,dh);ctx.restore();ctx.strokeStyle='#d3ddd2';ctx.lineWidth=1;ctx.beginPath();ctx.roundRect(x,y,w,h,18);ctx.stroke();
// Corner brackets emphasize existing UI without covering its words.
let target=s.key==='probe'&&u>8?[465,365,790,62]:s.key==='confirm'&&u>5.5?[465,615,205,42]:s.key==='quiz'&&u>6.4?[465,702,790,80]:null;
if(target){let [ax,ay,aw,ah]=target,k=Math.min(w/sw,h/sh),ox=x+(w-sw*k)/2,oy=y+(h-sh*k)/2;let px=ox+(ax-sx)*k,py=oy+(ay-sy)*k,pw=aw*k,ph=ah*k;ctx.save();ctx.beginPath();ctx.rect(x,y,w,h);ctx.clip();ctx.strokeStyle=C.green;ctx.lineWidth=3;const len=18;for(const [xx,yy,dx,dy] of [[px,py,1,1],[px+pw,py,-1,1],[px,py+ph,1,-1],[px+pw,py+ph,-1,-1]]){ctx.beginPath();ctx.moveTo(xx+dx*len,yy);ctx.lineTo(xx,yy);ctx.lineTo(xx,yy+dy*len);ctx.stroke()}ctx.restore();}}

function product(s,t){let u=t-s.a;ctx.fillStyle=C.bg;ctx.fillRect(0,0,W,H);tx('学涯',88,65,32,C.ink,600);tx('产品操作实录 · 预设案例',1450,64,22,C.muted);tx(s.title,88,146,45,C.ink,600);rr(88,167,76,5,2,C.green);drawVideo(s,u);let info=s.hint;
if(s.key==='probe')info=u<4.3?'导师提问：经过投影，什么会保持不变？':u<8?'学习者输入：“长度保持不变？”':'导师追问：长度会变，还有什么关系不会改变？';
if(s.key==='confirm')info=u<4.2?'重新回答：“交比？四个点的交比不变。”':'导师确认关键概念，并给出资料依据与下一步挑战';
rr(88,930,1744,88,15,C.dark);tx(info,960,987,31,C.white,400,'center');
const step=Math.floor((t-4)/46*6);for(let i=0;i<6;i++){let x=88+i*292;line(x,1050,x+274,1050,1,'#d2dcd2',4);let ss=edits[i];if(t>ss.a)line(x,1050,x+274,1050,clamp((t-ss.a)/(ss.b-ss.a)),C.green,4);}tx(`${s.n}  ${s.label}`,1600,145,23,C.green,600);
// Physical lateral reveal on chapter entry; it ends quickly so the UI can be read.
if(u<.24){let p=ease(u/.24);ctx.fillStyle=C.bg;ctx.fillRect(88+1744*p,196,1744*(1-p),700);}}
function recap(t){dark(t);tx('把一次学习，连成完整过程。',960,278,62,C.white,600,'center');let labels=['路线','对话','检查','复习'],sub=['明确方向','形成理解','发现误解','补齐薄弱点'];for(let i=0;i<4;i++){let p=ease((t-50-i*.28)/.48),x=172+i*420;ctx.save();ctx.globalAlpha=p;rr(x,440+(1-p)*24,320,210,24,'#1d3e2d');tx(labels[i],x+160,530,48,C.white,500,'center');tx(sub[i],x+160,588,27,'#b3d0c0',400,'center');ctx.restore();if(i<3)line(x+330,545,x+407,545,ease((t-50-.45-i*.28)/.45),'#35d0a5',4);}tx('学习记录默认保存在本机',960,835,30,'#b3d0c0',400,'center');}
function ending(t){dark(t);let u=t-54,p=ease(u/1.35);ctx.strokeStyle='#35d0a5';ctx.lineWidth=2.5;ctx.beginPath();ctx.ellipse(960,460,425,190,-.12,-Math.PI/2,-Math.PI/2+Math.PI*2*p);ctx.stroke();let a=-Math.PI/2+Math.PI*2*p;dot(960+425*Math.cos(a)*Math.cos(-.12)-190*Math.sin(a)*Math.sin(-.12),460+425*Math.cos(a)*Math.sin(-.12)+190*Math.sin(a)*Math.cos(-.12),7);ctx.globalAlpha=ease((u-.2)/.65);tx('学涯',960,480,112,C.white,600,'center');tx('有路线 · 有引导 · 有反馈',960,560,31,'#b3d0c0',400,'center');ctx.globalAlpha=1;tx('让每一次学习，都有下一步。',960,780,43,C.white,500,'center');tx('React  /  Tauri  /  Rust  /  SQLite',960,968,22,'#91aa9c',400,'center');}
async function seek(t){if(Math.abs(videoLast-t)<.005)return;await new Promise((resolve,reject)=>{let timer=setTimeout(()=>reject(Error('video seek timeout '+t)),10000);v.addEventListener('seeked',()=>{clearTimeout(timer);resolve()},{once:true});v.currentTime=t});videoLast=t;}
async function render(t){ctx.globalAlpha=1;if(t<4){intro(t);return}let s=edits.find(s=>t>=s.a&&t<s.b);if(s){await seek(mix(marks[s.key],marks[s.end]-.08,clamp((t-s.a)/(s.b-s.a))));product(s,t);}else if(t<54)recap(t);else ending(t);}
window.__film={ready:(async()=>{marks=await(await fetch('../../recordings/fusion/marks.json')).json();v=document.createElement('video');v.muted=true;v.preload='auto';v.src=URL.createObjectURL(await(await fetch('../../recordings/fusion/take.mp4')).blob());await new Promise((r,j)=>{v.addEventListener('loadeddata',r,{once:true});v.addEventListener('error',j,{once:true})});await document.fonts.ready;})(),render,DUR,FPS};
