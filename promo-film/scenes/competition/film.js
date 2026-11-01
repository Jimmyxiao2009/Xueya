'use strict';
const W=1920,H=1080,DUR=59,FPS=30;
const ink='#252d2b',muted='#66736d',green='#16735a',paper='#f4f2ed';
let scenes,images={};const ctx=document.getElementById('stage').getContext('2d');
const clamp=x=>Math.max(0,Math.min(1,x));
function text(t,x,y,size=30,color=ink,weight=400){ctx.fillStyle=color;ctx.font=`${weight} ${size}px "Microsoft YaHei","Segoe UI",sans-serif`;ctx.fillText(t,x,y)}
function wrap(t,x,y,width,size,color=ink,line=1.5){ctx.font=`400 ${size}px "Microsoft YaHei",sans-serif`;let row='',yy=y;for(const c of t){if(ctx.measureText(row+c).width>width){text(row,x,yy,size,color);row='';yy+=size*line;}row+=c;}if(row)text(row,x,yy,size,color);return yy;}
function rounded(x,y,w,h,r,color){ctx.fillStyle=color;ctx.beginPath();ctx.roundRect(x,y,w,h,r);ctx.fill()}
// Normalized crops remove empty margins; the underlying pixels are actual app captures.
const crops={roadmap:[.22,.04,.59,.90],'create-topic':[.28,.24,.45,.43],'create-level':[.27,.20,.47,.59],lesson:[.255,.08,.49,.66],opening:[.36,.07,.43,.43],probe:[.36,.10,.43,.36],confirm:[.36,.23,.43,.41],'quiz-feedback':[.365,.53,.415,.36]};
function product(name,t,x,y,w,h){let im=images[name],c=crops[name];let sw=im.width*c[2],sh=im.height*c[3];const scale=Math.min(w/sw,h/sh);let dw=sw*scale,dh=sh*scale;ctx.save();ctx.beginPath();ctx.roundRect(x,y,w,h,16);ctx.clip();ctx.fillStyle='#f8f7f4';ctx.fillRect(x,y,w,h);ctx.drawImage(im,im.width*c[0],im.height*c[1],sw,sh,x+(w-dw)/2,y+(h-dh)/2,dw,dh);ctx.restore();ctx.strokeStyle='#d8ded7';ctx.lineWidth=1;ctx.beginPath();ctx.roundRect(x,y,w,h,16);ctx.stroke();}
function render(t){ctx.fillStyle=paper;ctx.fillRect(0,0,W,H);let i=scenes.findIndex(s=>t<s.end);if(i<0)i=7;let s=scenes[i],u=t-s.start;
text('学涯',64,65,30,ink,700);text('参赛项目演示',160,65,22,muted);text('界面演示 · 预设案例',1585,65,20,muted);
ctx.fillStyle='#d8ded7';ctx.fillRect(64,92,1792,1);
text(i===0?'项目概览':s.tag,64,158,22,green,600);
const titleSize=i===0?86:42;wrap(s.title,64,246,500,titleSize,ink,1.3);
let yy=i===0?344:350;
if(i===0){wrap(s.tag,64,yy,435,31,green);yy+=118}
for(let n=0;n<s.lines.length;n++){let a=clamp((u-.2-n*.22)/.3);ctx.save();ctx.globalAlpha=a;rounded(64,yy-20,5,26,2,green);yy=wrap(s.lines[n],86,yy,420,i===7&&n===0?24:28,ink,1.5)+66;ctx.restore();}
if(i===7){rounded(64,680,446,124,16,'#e2ebe3');text('本地优先架构',88,725,27,green,600);text('学习数据默认保存在本机',88,772,23,ink);}
let name=s.alt&&u>(i===4?2.7:3.0)?s.alt:s.shot;product(name,t,574,124,1282,758);
// Keep the product crop unobstructed.
// A compact subtitle band: concrete narration, large enough for video judging.
rounded(64,914,1792,100,15,'#253a32');const parts=s.voice.match(/[^。！？]+[。！？]?/g)||[s.voice];let j=Math.min(parts.length-1,Math.floor(u/(s.end-s.start)*parts.length));let subtitle=(s.cues.find(c=>t>=c.start&&t<c.end)||s.cues.filter(c=>c.start<=t).at(-1)||s.cues[0]).text;ctx.textAlign='center';text(subtitle,960,973,31,'#ffffff');ctx.textAlign='left';
for(let k=0;k<8;k++){let x=64+k*225;rounded(x,1043,211,4,2,'#d6ddd6');if(k<i)rounded(x,1043,211,4,2,green);if(k===i)rounded(x,1043,211*clamp(u/(s.end-s.start)),4,2,green)}
text(`${String(i+1).padStart(2,'0')} / 08`,1770,1059,16,muted);
}
window.__film={ready:(async()=>{scenes=await(await fetch('script.json')).json();for(const s of scenes)for(const n of [s.shot,s.alt].filter(Boolean))if(!images[n]){let im=new Image();im.src=`../../shots/competition/${n}.png`;await im.decode();images[n]=im;}await document.fonts.ready;})(),render,DUR,FPS};
