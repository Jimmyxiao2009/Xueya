
const icons={orbit:'<ellipse cx="12" cy="12" rx="11" ry="5" transform="rotate(-35 12 12)"/><circle cx="12" cy="12" r="2"/>',arrow:'<path d="M5 12h14m-6-6 6 6-6 6"/>',back:'<path d="m14 5-7 7 7 7"/>',chevron:'<path d="m9 5 7 7-7 7"/>',check:'<path d="m5 12 4 4L19 6"/>',infinity:'<path d="M12 12c-3-6-10-6-10 0s7 6 10 0 10-6 10 0-7 6-10 0Z"/>',line:'<path d="M3 18 21 6M3 6l18 12M12 3v18"/><circle cx="12" cy="12" r="3"/>',review:'<path d="M3 10a9 9 0 1 1 2 8M3 4v6h6M12 7v5l3 2"/>',quiz:'<path d="m12 2 3 6 7 1-5 5 1 7-6-3-6 3 1-7-5-5 7-1Z"/>',grid:'<rect x="3" y="3" width="18" height="18" rx="3"/><path d="M9 3v18M15 3v18M3 9h18M3 15h18"/>',spark:'<path d="m12 3 2.3 6.7L21 12l-6.7 2.3L12 21l-2.3-6.7L3 12l6.7-2.3L12 3ZM20 2v4M18 4h4"/>',plus:'<path d="M12 5v14M5 12h14"/>',book:'<path d="M12 5v16M12 5C8 2 4 3 2 4v15c4-1 7-1 10 2 3-3 6-3 10-2V4c-3-1-6-2-10 1Z"/>',play:'<path d="m8 4 12 8-12 8Z"/>',camera:'<path d="m8 5 2-3h4l2 3h5v16H3V5Z"/><circle cx="12" cy="12" r="4"/>',send:'<path d="M12 19V5m-6 6 6-6 6 6"/>',close:'<path d="m6 6 12 12M6 18 18 6"/>',trash:'<path d="M3 6h18M9 6V3h6v3M5 6l1 15h12l1-15M10 10v7M14 10v7"/>',clock:'<circle cx="12" cy="12" r="9"/><path d="M12 6v6l4 2"/>',upload:'<path d="M12 16V3m-5 5 5-5 5 5M3 15v6h18v-6"/>',nodes:'<circle cx="5" cy="5" r="3"/><circle cx="19" cy="12" r="3"/><circle cx="5" cy="19" r="3"/><path d="m8 6 8 5M8 18l8-5"/>'};
const I=n=>`<svg class="icon" viewBox="0 0 24 24" aria-hidden="true">${icons[n]||icons.spark}</svg>`;
const esc=s=>String(s).replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
const maps=[{name:'射影几何',subtitle:'从平行线出发，重新认识空间。',lessons:['从欧氏到射影','无穷远点','方向与无穷远点','无穷远直线','第一阶段 · 小试身手','交比'],done:1,kind:'geometry'},{name:'线性代数',subtitle:'用向量和变换，理解空间的语言。',lessons:['向量与空间','线性组合','向量与线性组合','线性无关','第一阶段 · 小试身手','基与维数'],done:1,kind:'algebra'}];
let mapIndex=1,lessonIndex=1,view='home',tab='todo',quizType='single',selected=[],chatMessages=[],knowledgeDone=0,quizResult=null,photo=null,generationTimer=null;let questionStep=0,lessonComplete=false;let newRoute={topic:'',goal:'',files:[],urls:[],web:true,download:true,results:[],searched:false};const app=document.getElementById('app');const M=()=>maps[mapIndex];
const geometricTodos=[['平行线，真的不会相交吗？','从透视图像中发现欧氏几何的边界','3 min'],['给每个方向，一个无穷远点','理解平行线族与无穷远点的对应关系','5 min'],['用齐次坐标表示无穷远点','认识 [x : y : 0]，区分有限点与无穷远点','5 min'],['试着解释一次，再动手验证','通过小测与画图检查你的理解','4 min']];
const algebraTodos=[['向量是可以相加的箭头','理解向量加法的几何含义','3 min'],['系数如何改变向量','认识数乘与线性组合','5 min'],['两个向量能到达哪里？','观察张成空间与共线的关系','5 min'],['把你的想法写出来','用小测检查理解','4 min']];
const specificTodos={0:[['熟悉的平行公设','回顾欧氏平面中的平行线','3 min'],['把中心投影看作映射','认识点、线与投影中心','4 min'],['为什么引入射影平面','了解扩充平面的动机','5 min'],['整理你的几何直觉','用一句话解释学习目标','3 min']],2:[['回忆无穷远点','不看笔记描述方向与点的关系','2 min'],['识别相同方向','判断两条平行线对应的无穷远点','3 min'],['检查齐次坐标','判断 [1 : 2 : 0] 与 [2 : 4 : 0]','3 min'],['记录仍然困惑的地方','根据你的回答安排后续复习','2 min']],3:[['把无穷远点放在一起','从方向的集合理解无穷远直线','4 min'],['为什么它也是一条直线','理解齐次方程 z = 0','5 min'],['有限直线如何与它相交','连接平行方向与交点','5 min'],['画一张完整的关系图','整理点、直线与射影平面的关系','4 min']],4:[['概念辨析','辨别有限点、无穷远点与无穷远直线','3 min'],['坐标判断','判断齐次坐标所代表的点','4 min'],['图形作答','拍照提交你画的关系图','5 min'],['根据结果调整路径','演示插入针对性的复习节点','2 min']],5:[['从长度比到交比','认识直线上四点的关系','4 min'],['用有向坐标计算','计算四个共线点的交比','5 min'],['理解射影不变量','了解交比在射影变换下的性质','5 min'],['完成一道练习','通过计算检查掌握情况','4 min']]};
function todos(){if(M().kind==='algebra')return algebraTodos;if(M().kind==='custom')return [['明确本课学习目标','这是一份待接入 AI 的演示提纲','3 min'],['阅读与整理资料','真实资料将在接入检索后补充','5 min'],['形成自己的解释','用自己的话复述所学内容','5 min'],['通过练习检查掌握情况','接入模型后生成对应练习','4 min']];return specificTodos[lessonIndex]||geometricTodos;}
function softBlur(edge='top'){return `<div class="soft-blur ${edge==='bottom'?'soft-blur-bottom':''}" aria-hidden="true"><i></i><i></i><i></i></div>`;}
function renderHeader(compact=false){return compact?`<header class="compact-top">${softBlur()}<button class="brand" onclick="go('home')" aria-label="回到学习路线">${I('orbit')} orbit<span>.</span></button><span class="prototype">学习空间</span></header>`:`<header class="app-header"><div class="header-blur" aria-hidden="true"><i></i><i></i><i></i></div><button class="subject-switch" onclick="openRoadmapLayer()" aria-haspopup="dialog" aria-label="${esc(M().name)}，展开路线管理"><span><span class="eyebrow">我的学习</span><span class="subject-name">${esc(M().name)}</span></span>${I('chevron')}</button><span class="header-orbit">${I('orbit')}</span></header>`;}
function animateView(direction=1){if(window.matchMedia?.('(prefers-reduced-motion: reduce)').matches)return;const surface=document.querySelector('main');surface?.animate?.([{opacity:0,transform:`translateX(${direction*24}px)`},{opacity:1,transform:'translateX(-3px)',offset:.76},{opacity:1,transform:'translateX(0)'}],{duration:440,easing:'cubic-bezier(.22,.8,.3,1)'});}
function go(v){if(v==='home'&&view==='lesson'){returnToRoadmap();return;}if(v!=='chat')clearTimeout(planIntroTimer);if(v==='maps'){openRoadmapLayer();return;}document.querySelector('.roadmap-layer')?.close();const backwards=v==='home'||(view==='chat'&&v==='lesson');view=v;render();window.scrollTo({top:0,behavior:'instant'});animateView(backwards?-1:1);}
function render(){cancelLessonTransition?.();roadmapWaveObserver?.disconnect();roadmapWaveObserver=null;roadmapWaves.forEach(a=>a.cancel());roadmapWaves=[];app.dataset.view=view;if(view==='home')home();if(view==='maps')manager();if(view==='lesson')lesson();if(view==='chat')chat();if(view==='create')createRoutePage();if(view==='generating')generationPage();bindSwipe();if(view==='home')animateRoadmapWaves();}
// Slow, overlapping waves: position and tilt follow different phases.
// Precomputed compositor animations keep hit targets still and avoid frame-loop JS.
let roadmapWaves=[],roadmapWaveObserver=null;const roadmapWaveFrames=new Map();
function animateRoadmapWaves(){
 // The user explicitly wants ambient floating from first paint, including remote sessions.
 // Keep this gentle idle motion independent of reduced-motion interaction effects.
 const visibility=new Map();
 app.querySelectorAll('.stop').forEach((stop,index)=>{
  const tracks=[];
  const phase=index*.81,amplitude=.91+(index%3)*.055;
  ['.node-buoy','.label-buoy'].forEach((selector,part)=>{
   const el=stop.querySelector(selector);if(!el?.animate)return;
   const cacheKey=index+':'+part;
   let frames=roadmapWaveFrames.get(cacheKey);
   if(!frames){frames=Array.from({length:241},(_,frame)=>{
    const t=frame/240*Math.PI*2;
    const x=1.2*Math.sin(2*t+phase+.8)+.45*Math.sin(5*t-phase);
    const y=amplitude*(4.2*Math.sin(4*t+phase)+.85*Math.sin(7*t+phase*1.4+.9)+.3*Math.sin(11*t-phase));
    const roll=(1.05*Math.cos(4*t+phase+.55)+.25*Math.sin(7*t+phase+1.6))*(part ? .3 : 1);
    return {transform:`translate3d(${x.toFixed(3)}px,${y.toFixed(3)}px,0) rotate(${roll.toFixed(3)}deg)`};
   });
   roadmapWaveFrames.set(cacheKey,frames);}
   const track=el.animate(frames,{duration:36000,iterations:Infinity,easing:'linear',fill:'both'});
   tracks.push(track);roadmapWaves.push(track);
  });
  visibility.set(stop,tracks);
 });
 if(typeof IntersectionObserver==='function'){
  roadmapWaveObserver=new IntersectionObserver(entries=>{
   const covered=!!document.querySelector('.roadmap-layer[open]');
   entries.forEach(entry=>visibility.get(entry.target)?.forEach(track=>{track.orbitVisible=entry.isIntersecting;entry.isIntersecting&&!covered?track.play():track.pause();}));
  },{root:app.querySelector('.home-page'),rootMargin:'80px 0px'});
  visibility.forEach((_,stop)=>roadmapWaveObserver.observe(stop));
 }
}
function roadmapHTML(m){const names=m.lessons;return `<main class="home-page scroll-pane" id="swipe-zone" aria-label="学习路线"><section><div class="roadhead"><h2>${m.kind==='geometry'?'第一章 · 走向无穷远':'第一章 · 建立直觉'}</h2><span>6 个节点</span></div><div class="road"><svg class="path" viewBox="0 0 400 800" preserveAspectRatio="none" aria-hidden="true"><path d="M68 68C68 124 332 132 332 208S68 272 68 348S332 412 332 488S68 552 68 628S332 692 332 768"/><path class="completed-path" d="M68 68C68 124 332 132 332 208"/></svg>${names.map((name,i)=>{const done=i<m.done,current=i===Math.min(m.done,5),kind=done?'done':i===2?'review':i===4?'quiz':current?'current':'';return `<div class="stop ${i%2?'on-right':'on-left'} ${current?'active-stop':''}" style="top:${14+i*140}px;--delay:${.08+i*.13}s"><button class="node-hit" aria-label="打开${esc(name)}" onclick="openLesson(${i},this)"><span class="node-buoy"><span class="node ${kind}">${I(done?'check':i===1?'infinity':i===2?'review':i===4?'quiz':i===3?'line':i===5?'grid':'book')}${current?'<span class="node-orbit"></span>':''}</span></span></button><button class="stop-info" onclick="openLesson(${i},this)"><span class="label-buoy"><span class="info-face ${current?'now-card':''}"><span class="tag ${i===2?'purple':i===4?'orange':''}">${done?'已掌握':current?'从这里继续':i===2?'为你插入 · 复习':i===4?'检查理解 · 小测':'课程 '+String(i+1).padStart(2,'0')}</span><h3>${esc(name)}</h3><p>${i===2?'8 分钟 · 巩固关键概念':i===4?'12 分钟 · 多种题型':'4 个知识点 · '+(i===1?'17':'15')+' 分钟'}</p>${current?`<span class="next-arrow">${I('arrow')}</span>`:''}</span></span></button></div>`}).join('')}<div class="endnote">${I('spark')} 路线会随你的理解，继续生长</div></div></section><div class="home-footer"><span>${I('orbit')} orbit</span><span>左右滑动，切换学习路线</span></div></main>`;}
function home(){app.innerHTML=renderHeader()+roadmapHTML(M())+noticeHTML();}
function animateHeaderTitle(direction=1){
 const name=app.querySelector('.app-header .subject-name');if(!name||window.matchMedia?.('(prefers-reduced-motion: reduce)').matches||!name.animate)return;
 name.animate([{opacity:0,transform:`translateX(${direction*10}px)`},{opacity:1,transform:'translateX(0)'}],{duration:210,easing:'cubic-bezier(.2,.8,.2,1)'});
}
function replaceHeaderAnimated(direction=1){
 const old=app.querySelector('.app-header');if(!old)return;
 const template=document.createElement('template');template.innerHTML=renderHeader().trim();
 const next=template.content.firstElementChild;old.replaceWith(next);animateHeaderTitle(direction);
}
function switchMap(i,animate=true,prepared=null){
 document.querySelector('dialog')?.close();const direction=i<mapIndex?-1:1;
 const previous=app.querySelector('.home-page');if(previous)M().scrollPosition=previous.scrollTop;
 mapIndex=(i+maps.length)%maps.length;view='home';
 if(prepared){
  // Keep the same swipe-zone node alive. This lets a second touch take over
  // immediately, even while the previous inertial settle is still running.
  if(previous){
   const targetScroll=prepared.scrollTop||0;
   previous.innerHTML=prepared.innerHTML;
   previous.className=prepared.className;
   previous.id='swipe-zone';
   previous.scrollTop=targetScroll;
   prepared.remove();
  }else{
   app.append(prepared);
  }
  replaceHeaderAnimated(direction);
  app.querySelector('.bottom-notice')?.remove();app.insertAdjacentHTML('beforeend',noticeHTML());app.querySelector('.bottom-notice')?.classList.add('settled-notice');
  roadmapWaveObserver?.disconnect();roadmapWaveObserver=null;roadmapWaves.forEach(a=>a.cancel());roadmapWaves=[];
  animateRoadmapWaves();
 }else{render();requestAnimationFrame(()=>animateHeaderTitle(direction));}
 if(animate)animateView(direction);
}
function manager(){openRoadmapLayer();}
function noticeHTML(){return M().noticeClosed?'':`<aside class="bottom-notice acrylic" id="route-notice" aria-label="路线更新"><span class="notice-icon">${I(M().adapted?'review':'quiz')}</span><div><strong>${M().adapted?'已调整下一次复习':'已为你添加阶段小测'}</strong><p>${M().adapted?'针对刚才的薄弱点，再巩固一下。':'完成这一段后，检查一下理解。'}</p></div><button onclick="dismissNotice()" aria-label="关闭路线提醒">${I('close')}</button></aside>`;}
function dismissNotice(){M().noticeClosed=true;const n=document.getElementById('route-notice');n?.classList.add('notice-out');setTimeout(()=>n?.remove(),200);}
function openRoadmapLayer(){const d=dialog(`<div class="layer-heading"><button class="subject-switch" onclick="closeRoadmapLayer()" aria-label="收起路线管理"><span><span class="eyebrow">我的学习</span><span class="subject-name">${esc(M().name)}</span></span>${I('chevron')}</button></div><div class="layer-scroll"><div class="progress-card"><div class="row"><span>完成进度</span><span class="progress-count"><strong>${Math.round(M().done/6*100)}%</strong><span>${M().done} / 6 个节点</span></span></div><div class="bar"><i style="width:${M().done/6*100}%"></i></div></div><div class="layer-label">切换路线 <span>${maps.length} 条</span></div><div class="layer-maps">${maps.map((m,i)=>`<div class="layer-map ${i===mapIndex?'selected':''}"><button class="route-select" onclick="switchMap(${i})"><span class="route-glyph">${I(m.kind==='geometry'?'orbit':'grid')}</span><span><strong>${esc(m.name)}</strong><small>${m.done} / 6 个节点${i===mapIndex?' · 当前路线':''}</small></span>${i===mapIndex?I('check'):I('chevron')}</button>${maps.length>1?`<button class="remove-route" onclick="deleteMap(${i})" aria-label="移除${esc(m.name)}">${I('trash')}</button>`:''}</div>`).join('')}</div></div><button class="add-unknown" onclick="startNewRoute()"><span class="unknown-icon">${I('plus')}</span><span><strong>探索新的未知</strong><small>添加一条新的学习路线</small></span>${I('arrow')}</button>`);d.classList.add('roadmap-layer');
 const slats=[...d.querySelectorAll('.progress-card,.layer-label,.layer-map,.add-unknown')];
 slats.forEach((el,i)=>{el.classList.add('blind-slat');el.style.setProperty('--slat',i);el.style.setProperty('--close-slat',slats.length-1-i);});
 d.dataset.closeDuration=String(140+Math.max(0,slats.length-1)*12);
 app.classList.add('roadmap-is-open');roadmapWaves.forEach(a=>a.pause());
 d.addEventListener('close',()=>{if(!document.querySelector('.roadmap-layer[open]')){app.classList.remove('roadmap-is-open');roadmapWaves.forEach(a=>{if(a.orbitVisible!==false)a.play();});}});
 d.addEventListener('cancel',e=>{e.preventDefault();dismissDialog(d);});
}
function dismissDialog(d){if(!d||d.classList.contains('is-closing'))return;if(window.matchMedia?.('(prefers-reduced-motion: reduce)').matches){d.close();return;}d.classList.add('is-closing');setTimeout(()=>{d.close();d.classList.remove('is-closing');},Number(d.dataset.closeDuration)||240);}
function closeRoadmapLayer(){dismissDialog(document.querySelector('.roadmap-layer'));}
function dialog(html){const previous=document.querySelector('dialog');previous?.close();previous?.remove();const d=document.createElement('dialog');d.className='dialog';d.innerHTML=html;document.body.append(d);d.showModal();d.addEventListener('click',e=>{if(e.target===d)dismissDialog(d)});return d;}
function deleteMap(i){dialog(`<h2>移除「${esc(maps[i].name)}」？</h2><p class="subtext">仅移除此原型中的演示路线。</p><div class="actions"><button class="secondary" onclick="this.closest('dialog').close()">保留</button><button class="primary" onclick="removeMap(${i})">移除路线</button></div>`);}
function removeMap(i){maps.splice(i,1);mapIndex=Math.min(mapIndex,maps.length-1);document.querySelector('dialog').close();go('home');openRoadmapLayer();}
function newMapDialog(){startNewRoute();}
function startNewRoute(){document.querySelector('dialog')?.close();go('create');}
function backToLayer(){go('home');openRoadmapLayer();}
function createRoutePage(){app.innerHTML=`<header class="create-header acrylic"><button class="close" onclick="backToLayer()" aria-label="返回路线管理">${I('back')}</button><span>新的学习路线</span><span class="header-orbit">${I('spark')}</span></header><main class="create-page scroll-pane"><div class="eyebrow">FOLLOW YOUR CURIOSITY</div><h1 class="page-title">下一站，想探索什么？</h1><p class="subtext">一个问题，一本书，或者一个新的领域。</p><form id="route-form" onsubmit="event.preventDefault();generateMap()"><label for="topic">想学的主题</label><input id="topic" class="text-input" placeholder="例如：线性代数、和声、机器学习" value="${esc(newRoute.topic)}" oninput="newRoute.topic=this.value" maxlength="60" required><label for="goal">你的学习目标 <span>可选</span></label><textarea id="goal" class="text-input" rows="2" placeholder="比如，能读懂机器学习论文里的矩阵公式" oninput="newRoute.goal=this.value">${esc(newRoute.goal)}</textarea><div class="form-section"><div class="row"><h2>参考资料</h2><span class="small">可同时使用两种来源</span></div><label class="file-drop" for="reference-files">${I('upload')}<strong>从本地添加资料</strong><span>PDF、文档、笔记、图片 · 单个不超过 20 MB</span></label><input id="reference-files" type="file" accept=".pdf,.doc,.docx,.txt,.md,.png,.jpg,.jpeg,.webp,.ppt,.pptx" multiple hidden onchange="addReferenceFiles(this)"><div id="reference-list">${referenceList()}</div></div><div class="web-source"><div class="row"><div class="web-title">${I('orbit')}<span><strong>联网搜集资料</strong><small>公开课程、讲义与权威参考</small></span></div><label class="toggle"><input type="checkbox" aria-label="联网搜集资料" ${newRoute.web?'checked':''} onchange="newRoute.web=this.checked;document.getElementById('web-options').hidden=!this.checked"><span></span></label></div><div id="web-options" ${newRoute.web?'':'hidden'}><div class="url-input"><input id="source-url" class="text-input" type="url" placeholder="粘贴指定来源链接（可选）"><button type="button" aria-label="添加来源链接" onclick="addSourceURL()">${I('plus')}</button></div><div id="url-list">${urlList()}</div><label class="download-pref"><input type="checkbox" ${newRoute.download?'checked':''} onchange="newRoute.download=this.checked">保留可下载原文件的入口</label><button class="secondary collect-button" type="button" onclick="collectSources()">${I('spark')} 预览搜集结果</button><div id="web-results">${networkPreview()}</div><p class="prototype-note">原型使用已核验的示例来源；实时联网检索尚未接入。</p></div></div><p class="local-note">本地文件仅保留在当前页面，刷新后需要重新选择。</p></form></main><footer class="create-dock"><button class="primary" form="route-form" type="submit">生成学习路线 ${I('arrow')}</button></footer>`;}
function sizeLabel(n){return n>=1048576?(n/1048576).toFixed(1)+' MB':Math.max(1,Math.round(n/1024))+' KB';}
function referenceList(){return newRoute.files.map((f,i)=>`<div class="reference-row">${I('book')}<div><strong>${esc(f.name)}</strong><small>${sizeLabel(f.size)} · 已选择</small></div><button type="button" aria-label="移除${esc(f.name)}" onclick="removeReference(${i})">${I('close')}</button></div>`).join('');}
function addReferenceFiles(input){let rejected=0;for(const file of Array.from(input.files||[])){if(file.size>20*1024*1024||newRoute.files.length>=8){rejected++;continue;}if(!newRoute.files.some(f=>f.name===file.name&&f.size===file.size))newRoute.files.push(file);}input.value='';document.getElementById('reference-list').innerHTML=referenceList();if(rejected)toast('最多 8 个文件，每个不超过 20 MB');}
function removeReference(i){newRoute.files.splice(i,1);document.getElementById('reference-list').innerHTML=referenceList();}
function addSourceURL(){const input=document.getElementById('source-url');try{const u=new URL(input.value.trim());if(!['https:','http:'].includes(u.protocol))throw Error();if(!newRoute.urls.includes(u.href))newRoute.urls.push(u.href);input.value='';document.getElementById('url-list').innerHTML=urlList();}catch{toast('请填写完整的 http 或 https 链接');}}
function urlList(){return newRoute.urls.map((url,i)=>`<div class="reference-row">${I('orbit')}<div><strong>${esc(new URL(url).hostname)}</strong><small>待读取的指定来源</small></div><button type="button" aria-label="移除来源链接" onclick="newRoute.urls.splice(${i},1);document.getElementById('url-list').innerHTML=urlList()">${I('close')}</button></div>`).join('');}
function detectTopicKind(t){return /线性|linear|矩阵|向量/i.test(t)?'algebra':/射影|projective/i.test(t)?'geometry':'custom';}
function collectSources(){const kind=detectTopicKind(newRoute.topic);newRoute.results=kind==='custom'?[]:sourceCatalog[kind];newRoute.searched=true;document.getElementById('web-results').innerHTML=networkPreview();}
function networkPreview(){if(!newRoute.searched)return '';return newRoute.results.length?`<div class="search-result-label">示例搜集结果 · ${newRoute.results.length} 项</div>${newRoute.results.map(r=>`<a class="search-result" href="${r.url}" target="_blank" rel="noopener noreferrer">${I(r.type==='视频'?'play':'book')}<span>${esc(r.title)}<small>${esc(r.publisher)}</small></span>${I('arrow')}</a>`).join('')}`:'<p class="prototype-note">这个主题暂无内置来源。可以添加本地资料或指定链接；真实检索会在接入 agent 后进行。</p>';}
function generateMap(){const name=newRoute.topic.trim();if(!name){toast('先写下想学的主题');return;}view='generating';render();generationTimer=setTimeout(()=>{const status=document.getElementById('generation-status');if(status)status.textContent='正在演示拆分章节与安排复习…';},650);setTimeout(()=>{const kind=detectTopicKind(name);const template=kind==='algebra'?maps.find(m=>m.kind==='algebra'):kind==='geometry'?maps.find(m=>m.kind==='geometry'):null;const sources=[...newRoute.files.map(f=>({title:f.name,publisher:'你添加的参考资料',type:'本地文件',url:URL.createObjectURL(f),local:true,download:f.name,description:'待备课 agent 解析；当前可打开或保存原文件。'})),...(newRoute.web&&kind!=='custom'?sourceCatalog[kind].map(r=>({...r,downloadURL:newRoute.download?r.downloadURL:null})):[]),...(newRoute.web?newRoute.urls.map(url=>({title:new URL(url).hostname,publisher:'指定来源 · 待读取',type:'链接',url,description:'你指定的参考链接，尚未读取内容。'})):[])];maps.push({name,subtitle:newRoute.goal||'一条从好奇出发的新路径。',lessons:template?[...template.lessons]:['基础概念','建立核心直觉','关键概念复习','方法与应用','第一阶段 · 小试身手','综合实践'],done:0,kind,sources});mapIndex=maps.length-1;newRoute={topic:'',goal:'',files:[],urls:[],web:true,download:true,results:[],searched:false};go('home');toast('演示路线已生成，参考资料已带入');},1350);}
function generationPage(){app.innerHTML=`<main class="generation-page"><span class="generation-orbit">${I('orbit')}</span><h1 class="page-title">为「${esc(newRoute.topic)}」<br>铺好第一段路</h1><p id="generation-status">正在整理你选择的资料…</p><div class="bar"><i style="width:85%"></i></div><p class="prototype-note">路线生成演示 · 未调用真实备课 agent</p></main>`;}
function lessonCardClip(bounds,frame){
 const top=Math.max(0,Math.min(frame.height-48,bounds.top-frame.top-6));
 const bottom=Math.max(0,frame.height-Math.max(top+48,Math.min(frame.height,bounds.bottom-frame.top+6)));
 const left=Math.max(0,Math.min(frame.width-48,bounds.left-frame.left-8));
 const right=Math.max(0,frame.width-Math.max(left+48,Math.min(frame.width,bounds.right-frame.left+8)));
 return `inset(${top}px ${right}px ${bottom}px ${left}px round 24px)`;
}
function animateLessonTransition(underlay,screen,clip,reverse=false){
 const tracks=[];
 const track=(element,frames,duration,delay=0,easing='cubic-bezier(.2,.8,.2,1)')=>{if(element)tracks.push({element,frames,duration,delay,easing});};
 track(underlay,[{opacity:1,transform:'scale(1)'},{opacity:.42,transform:'scale(.978)'}],390);
 track(screen,[{clipPath:clip},{clipPath:'inset(0px 0px 0px 0px round 0px)'}],470,0,'cubic-bezier(.18,.9,.22,1)');
 track(screen.querySelector('.compact-top'),[{opacity:0,transform:'translateY(-6px)'},{opacity:1,transform:'translateY(0)'}],210,95);
 const groups=[screen.querySelector('.back'),screen.querySelector('.course-top'),screen.querySelector('.pills'),screen.querySelector('.tabs'),screen.querySelector('.section-label'),...screen.querySelectorAll('.todo,.source-card'),screen.querySelector('.swipe-tip')];
 groups.forEach((el,index)=>track(el,[{opacity:0,transform:'translateY(12px)'},{opacity:1,transform:'translateY(0)'}],225,82+index*18));
 track(screen.querySelector('.lesson-dock'),[{opacity:0,transform:'translateY(28px) scale(.985)'},{opacity:1,transform:'translateY(0) scale(1)'}],280,165);
 const length=Math.max(...tracks.map(t=>t.delay+t.duration));
 const animations=tracks.map(t=>t.element.animate(t.frames,{duration:t.duration,delay:reverse?length-t.delay-t.duration:t.delay,easing:t.easing,direction:reverse?'reverse':'normal',fill:'both'}));
 const start=document.timeline?.currentTime;if(start!=null)animations.forEach(a=>a.startTime=start);
 return animations;
}
let cancelLessonTransition=null;
function setMotionLite(active){document.documentElement.classList.toggle('motion-lite',!!active);}
function openLesson(i,trigger){
 if(cancelLessonTransition)return;
 if(view==='home')M().scrollPosition=app.querySelector('.home-page')?.scrollTop||0;
 const reduced=window.matchMedia?.('(prefers-reduced-motion: reduce)').matches;
 const origin=trigger?.closest('.stop')||app.querySelector?.('.stop:nth-of-type('+(i+1)+')');
 const bounds=origin?.getBoundingClientRect?.(),frame=app.getBoundingClientRect?.();
 lessonIndex=i;tab='todo';
 if(view!=='home'||reduced||!bounds||!frame||!app.animate){go('lesson');return;}
 const pane=app.querySelector('.home-page'),scroll=pane?.scrollTop||0;
 const oldScreen=document.createElement('div');
 oldScreen.className='lesson-underlay';oldScreen.inert=true;oldScreen.setAttribute('aria-hidden','true');
 oldScreen.append(...app.childNodes);
 oldScreen.querySelectorAll('[id]').forEach(el=>el.removeAttribute('id'));
 view='lesson';render();
 const newScreen=document.createElement('div');
 newScreen.className='lesson-opening';newScreen.inert=true;newScreen.setAttribute('aria-busy','true');
 newScreen.append(...app.childNodes);app.append(oldScreen,newScreen);
 if(pane)pane.scrollTop=scroll;
 const clip=lessonCardClip(bounds,frame),animations=[];setMotionLite(true);
 const finish=()=>{
  if(cancelLessonTransition!==finish)return;
  cancelLessonTransition=null;
  newScreen.querySelector('main')?.classList.add('lesson-revealed');
  animations.forEach(a=>a.cancel());oldScreen.remove();
  newScreen.inert=false;newScreen.removeAttribute('aria-busy');setMotionLite(false);
  if(newScreen.parentNode===app)newScreen.replaceWith(...newScreen.childNodes);
  const heading=app.querySelector('.course-top h1');
  heading?.setAttribute('tabindex','-1');heading?.focus({preventScroll:true});
 };
 cancelLessonTransition=finish;
 try{
  animations.push(...animateLessonTransition(oldScreen,newScreen,clip));
  Promise.allSettled(animations.map(a=>a.finished)).then(finish);
 }catch{finish();}
}
function geoSvg(){return `<svg class="geometry" viewBox="0 0 180 150" aria-hidden="true"><path d="M15 120 90 23 163 120ZM15 120h148M90 23v110M33 98h114M52 72h76" fill="none" stroke="currentColor" stroke-width="1.2" opacity=".6"/><ellipse cx="90" cy="108" rx="66" ry="23" fill="none" stroke="currentColor" stroke-dasharray="4 5" opacity=".5"/><circle cx="90" cy="23" r="5" fill="currentColor"/><circle cx="33" cy="98" r="4" fill="currentColor"/><circle cx="147" cy="98" r="4" fill="currentColor"/></svg>`;}
function lesson(){app.innerHTML=renderHeader(true)+`<main class="page" id="swipe-zone"><button class="back" onclick="go('home')">${I('back')} ${esc(M().name)} / 学习路线</button><div class="course-top"><div><div class="eyebrow">LESSON ${String(lessonIndex+1).padStart(2,'0')}</div><h1 class="page-title">${esc(M().lessons[lessonIndex])}</h1><p class="subtext">${M().kind==='geometry'&&lessonIndex===1?'让平行的两条线，在无穷远处相遇。':'带着问题出发，把概念变成自己的理解。'}</p></div>${geoSvg()}</div><div class="pills"><span class="pill">${I('clock')} 约 17 分钟</span><span class="pill">${I('nodes')} 4 个知识点</span><span class="pill">${I('spark')} 课程已准备好</span></div><div class="tabs" role="tablist"><button role="tab" aria-selected="${tab==='todo'}" class="${tab==='todo'?'active':''}" onclick="setTab('todo')">本课待学 <span class="small">04</span></button><button role="tab" aria-selected="${tab==='materials'}" class="${tab==='materials'?'active':''}" onclick="setTab('materials')">课程资料 ${I('book')}</button></div><div class="lesson-content">${lessonContentHTML()}</div></main><footer class="lesson-dock">${softBlur('bottom')}<button class="primary floating" onclick="launchLearning(this)">${I('play')} 开始学习 <span style="opacity:.6;font-weight:400">· 17 min</span> ${I('arrow')}</button></footer>`;}
function lessonContentHTML(which=tab){return which==='todo'?`<div class="section-label"><span>从这四个小问题开始</span><span class="small">学习时逐步点亮</span></div><div class="todos">${todos().map((t,i)=>`<div class="todo" style="--delay:${i*.08}s"><span class="number">${i+1}</span><div><strong>${t[0]}</strong><p>${t[1]}</p></div><span class="duration">${t[2]}</span></div>`).join('')}</div><p class="swipe-tip">向左滑动，翻开本课资料 ${I('arrow')}</p>`:materials();}
function setTab(t,animate=true,prepared=null){
 if(t===tab)return;
 const update=(preparedPane=prepared)=>{
  tab=t;const current=document.querySelector('.lesson-content');
  if(!current){lesson();bindSwipe();return;}
  const next=preparedPane||document.createElement('div');next.className='lesson-content swipe-settled';if(!preparedPane)next.innerHTML=lessonContentHTML();current.replaceWith(next);
  document.querySelectorAll('.tabs [role="tab"]').forEach((button,i)=>{const active=(i===0)===(tab==='todo');button.classList.toggle('active',active);button.setAttribute('aria-selected',String(active));});
 };
 if(animate)slideAcross(document.querySelector('.lesson-content'),t==='materials'?1:-1,update);else update();
}
const sourceCatalog={algebra:[{title:'ZoomNotes for Linear Algebra',publisher:'MIT · Gilbert Strang',type:'PDF',url:'https://ocw.mit.edu/courses/18-06-linear-algebra-spring-2010/4d876a9159e32543eb0d73b4d4382f4c_MIT18_06S10ZoomNotes.pdf#page=8',downloadURL:'https://ocw.mit.edu/courses/18-06-linear-algebra-spring-2010/4d876a9159e32543eb0d73b4d4382f4c_MIT18_06S10ZoomNotes.pdf',description:'80 页讲义 · 第 1.1 节为向量的线性组合，对应本课的数乘与加法。'},{title:'The geometry of linear equations',publisher:'MIT OpenCourseWare · 18.06',type:'视频',url:'https://ocw.mit.edu/courses/18-06-linear-algebra-spring-2010/resources/lecture-1-the-geometry-of-linear-equations/',description:'Gilbert Strang 的第 1 讲，从几何角度理解线性方程。'},{title:'Linear Algebra · Study Materials',publisher:'MIT OpenCourseWare',type:'资料集',url:'https://ocw.mit.edu/courses/18-06-linear-algebra-spring-2010/pages/study-materials/',description:'教材补充材料与往年练习、考试的来源入口。'}],geometry:[{title:'Point at Infinity',publisher:'Wolfram MathWorld',type:'词条',url:'https://mathworld.wolfram.com/PointatInfinity.html',description:'无穷远点的参考定义与相关概念。'},{title:'Projective Geometry',publisher:'Wolfram MathWorld',type:'词条',url:'https://mathworld.wolfram.com/ProjectiveGeometry.html',description:'射影几何的基本概念、公理与书目。'}]};
function materials(){const sources=M().sources||sourceCatalog[M().kind]||[];return `<div class="section-label"><span>本课资料来源</span><span class="small">${sources.length} 项参考</span></div><p class="source-explainer">查看原始讲义、课程与参考资料。</p>${sources.map((r,i)=>`<article class="source-card"><div class="source-top"><span class="source-mark">${I(r.type==='视频'?'play':r.local?'upload':'book')}</span><div><small>${esc(r.publisher)}</small><h3>${esc(r.title)}</h3></div><span class="source-type">${esc(r.type)}</span></div><p>${esc(r.description)}</p><div class="source-actions"><a href="${esc(r.url)}" target="_blank" rel="noopener noreferrer">${r.local?'打开文件':'查看原文'} ${I('arrow')}</a>${r.local?`<a href="${r.url}" download="${esc(r.download)}">保存文件 ${I('upload')}</a>`:r.downloadURL?`<a href="${r.downloadURL}" target="_blank" rel="noopener noreferrer">打开可下载 PDF ${I('upload')}</a>`:''}</div></article>`).join('')||'<div class="empty-note">还没有参考资料。<br>可在新增路线时上传文件或添加来源链接。</div>'}<p class="prototype-note">${M().sources?'本地文件和指定链接尚未交给 agent 解析。':'已核验的示例来源，用于预览 agent 资料列表；不是实时检索结果。'}</p>`;}
let launching=false;
function launchLearning(button){if(launching)return;launching=true;button.classList.add('launching');button.disabled=true;setTimeout(()=>{launching=false;startLearning();},window.matchMedia?.('(prefers-reduced-motion: reduce)').matches?0:420);}
function startLearning(){chatRenderCount=0;advancing=false;planExpanded=true;planLastRendered=-1;knowledgeDone=0;questionStep=0;lessonComplete=false;quizType='single';selected=[];quizResult=null;photo=null;chatMessages=[];go('chat');clearTimeout(planIntroTimer);planIntroTimer=setTimeout(()=>{if(view==='chat'&&planExpanded)setLearningPlanExpanded(false);},2400);}
let planExpanded=true,planLastRendered=-1,chatRenderCount=0,advancing=false,planIntroTimer=null,planResize=null;
function completionMark(){return `<svg class="plan-checkmark" viewBox="0 0 28 28" aria-hidden="true"><circle class="check-disc" cx="14" cy="14" r="11"/><circle class="check-ring" cx="14" cy="14" r="10"/><path class="check-stroke" d="m8.5 14 3.5 3.5 7.5-8"/></svg><span class="check-rays" aria-hidden="true">${[[-15,-12],[0,-20],[16,-12],[18,10],[0,21],[-17,10]].map(([x,y],i)=>`<i style="--ray-x:${x}px;--ray-y:${y}px;--ray-turn:${i*60}deg"></i>`).join('')}</span>`;}
function learningPlanHTML(){const items=todos(),completed=knowledgeDone;const html=`${softBlur()}<button class="plan-toggle" onclick="toggleLearningPlan()" aria-expanded="${planExpanded}" aria-controls="learning-plan-steps"><span class="plan-heading">${I('nodes')} 学习计划</span><span class="plan-summary">${completed} / ${items.length} 已完成 ${I('chevron')}</span></button>${planExpanded?`<ol class="plan-steps" id="learning-plan-steps">${items.map((item,i)=>{const done=i<completed,active=!done&&i===completed;return `<li class="plan-step ${done?'is-done':active?'is-current':'is-pending'} ${done&&i===completed-1&&completed!==planLastRendered?'just-completed':active&&completed!==planLastRendered?'just-activated':''}" ${active?'aria-current="step"':''}><span class="plan-marker" aria-hidden="true">${done?completionMark():active?'<span class="plan-working"></span>':'<span class="plan-empty"></span>'}</span><span class="plan-task">${esc(item[0])}</span><span class="plan-state">${done?'已完成':active?'正在学':'待学习'}</span></li>`}).join('')}</ol>`:`<div class="plan-collapsed">${completed===items.length?'本课计划已完成':`<span class="plan-working"></span><span>正在学 · ${esc(items[completed][0])}</span>`}</div>`}`;planLastRendered=completed;return html;}
function setLearningPlanExpanded(expanded){
 const panel=document.getElementById('learning-plan');if(!panel)return;
 const before=panel.getBoundingClientRect().height;
 planResize?.cancel();planExpanded=expanded;panel.innerHTML=learningPlanHTML();
 const after=panel.getBoundingClientRect().height;
 if(!window.matchMedia?.('(prefers-reduced-motion: reduce)').matches&&panel.animate){
  const motion=panel.animate([{height:before+'px'},{height:after+'px'}],{duration:340,easing:'cubic-bezier(.22,.8,.3,1)'});
  planResize=motion;motion.finished.then(()=>{if(planResize===motion)planResize=null;}).catch(()=>{});
 }
}
function toggleLearningPlan(){clearTimeout(planIntroTimer);setLearningPlanExpanded(!planExpanded);}
function chat(){chatRenderCount++;const geo=M().kind!=='algebra';app.innerHTML=`<main class="chat-page"><header class="chat-header">${softBlur()}<button class="back" aria-label="返回课程" onclick="go('lesson')">${I('back')}</button><div class="chat-headinfo"><h2>${esc(M().lessons[lessonIndex])}</h2><span class="small">Orbit · 和你一起理解</span></div><button class="secondary" onclick="go('home')">${I('nodes')} 路线</button></header><section class="chat-progress learning-plan" id="learning-plan" aria-label="串行学习计划">${learningPlanHTML()}</section><div class="chat-body scroll-pane"><div class="assistant-label">${I('orbit')} Orbit</div><div class="message ${chatRenderCount>1?'settled-message':''}"><p>${geo?'先从一个小问题开始：在射影平面里，两条平行线会在哪里相交？':'把向量想成箭头：数乘改变长度和方向，加法把箭头首尾相接。'}</p><p>${geo?'每个方向都对应一个无穷远点。试试下面这道题，我来看看你是否理解了“方向”。':'通过改变系数，我们能到达哪些位置？先选一个你认为对的答案。'}</p>${geo?'':'<div class="formula">w = au + bv</div>'}</div><div id="messages">${chatMessages.map((x,i)=>`<div class="message ${x.role} ${i<chatMessages.length-2?'settled-message':''}">${x.html}</div>`).join('')}</div>${lessonComplete?`<div class="completion"><h3>${I('check')} 这一站，完成了。</h3><p>你已完成本课学习与作答。回到路线，看看下一站。</p><button class="primary" onclick="finishLesson()">回到我的路线 ${I('arrow')}</button></div>`:`<div class="assistant-label question-author">${I('spark')} Orbit 向你提问</div><div id="quiz">${quizHTML()}</div>`}<div id="chat-end"></div></div></main><div class="composer-wrap">${softBlur('bottom')}<form class="composer" onsubmit="event.preventDefault();sendMessage()"><input id="chat-input" aria-label="发送消息" placeholder="有疑问，随时打断我…" autocomplete="off"><button class="send" aria-label="发送消息" type="submit">${I('send')}</button></form><div class="composer-note">预设 AI 提问与反馈演示 · 图片仅在本页预览</div></div>`;}
function scrollChat(){document.getElementById('chat-end')?.scrollIntoView({behavior:'smooth',block:'end'});}
function question(){const geo=M().kind!=='algebra';if(geo)return {single:['同一平面内，两组方向不同的平行线，对应几个无穷远点？',['1 个，所有平行线都交于同一点','2 个，每个方向对应一个无穷远点','0 个，因为平行线永不相交'],[1]],multi:['下面哪些说法正确？',['同一方向的平行线共享一个无穷远点','无穷远点是离原点非常远的有限点','[1 : 2 : 0] 与 [2 : 4 : 0] 表示同一点'],[0,2]],fill:['齐次坐标 [x : y : z] 中，无穷远点的 z 等于多少？'],short:['请用自己的话解释：为什么同一方向的平行线共享一个无穷远点？'],photo:['画出两组方向不同的平行线，并标注它们对应的无穷远点。']}[quizType];return {single:['两个不共线的平面向量，其所有线性组合可以得到什么？',['一条直线','整个平面','只有两个原向量'],[1]],multi:['关于线性组合，哪些说法正确？',['可以使用负数系数','线性组合必须使用正整数系数','取两个系数为 0 可以得到零向量'],[0,2]],fill:['当 a = b = 0 时，au + bv 等于多少向量？（填数字）'],short:['用自己的话解释：线性组合中的系数有什么作用？'],photo:['画两个不共线向量 u、v，再画出 u + v。']}[quizType];}
function quizHTML(){const q=question();return `<section class="quiz-card"><div class="quiz-tag">${I('spark')} 理解检查 · ${{single:'单选',multi:'多选',fill:'填空',short:'简答',photo:'拍照作答'}[quizType]}</div><h3>${q[0]}</h3>${['single','multi'].includes(quizType)?`<div class="options">${q[1].map((s,i)=>`<button class="option ${selected.includes(i)?'selected':''}" onclick="selectAnswer(${i})" aria-pressed="${selected.includes(i)}"><span>${selected.includes(i)?'✓':String.fromCharCode(65+i)}</span>${s}</button>`).join('')}</div>`:quizType==='photo'?`<label class="upload" for="answer-photo">${photo?`<img src="${photo}" alt="你上传的作答照片">`:I('upload')}${photo?'点击重新选择图片':'拍摄或上传你的作答'}<input hidden type="file" id="answer-photo" accept="image/*" capture="environment" onchange="readPhoto(this)"></label>`:`<${quizType==='short'?'textarea':'input'} id="written-answer" class="text-input" placeholder="${quizType==='short'?'写下你的理解…':'在这里填写答案'}" ${quizType==='short'?'rows="3"':''}>${quizType==='short'?'</textarea>':''}`}${quizResult?`<div class="feedback ${quizResult.ok?'':'wrong'}" role="status"><span class="feedback-symbol">${I(quizResult.ok?'check':'review')}</span><span>${quizResult.text}</span></div>`:''}<div class="quiz-actions"><span class="small">${quizType==='multi'?'可选择多个答案':'不用着急，想一想'}</span><button class="primary" onclick="${quizResult?.ok?'advanceLearning(this)':'submitAnswer()'}">${quizResult?.ok?'继续学习':'提交答案'} ${I('arrow')}</button></div></section>`;}
function updateQuiz(){document.getElementById('quiz').innerHTML=quizHTML();}
function changeQuiz(t){quizType=t;selected=[];quizResult=null;photo=null;const input=document.getElementById('chat-input')?.value||'';chat();document.getElementById('chat-input').value=input;}
function selectAnswer(i){if(quizResult?.ok)return;selected=quizType==='multi'?(selected.includes(i)?selected.filter(x=>x!==i):[...selected,i]):[i];quizResult=null;updateQuiz();}
function submitAnswer(){if(quizResult?.ok)return;let ok=false,text='';if(['single','multi'].includes(quizType)){if(!selected.length){toast('先选一个答案吧');return;}const correct=question()[2];ok=correct.length===selected.length&&correct.every(i=>selected.includes(i));text=ok?(M().kind==='algebra'?'答对了！线性组合允许任意实数系数。':'答对了！方向相同，才对应同一个无穷远点。'):'再想一想。'+(M().kind==='algebra'?'系数可以是任意实数，包括 0 和负数。':'不同方向对应不同的无穷远点；齐次坐标可以整体乘以非零常数。')+' 已调整路线中的针对性复习。';if(!ok){M().noticeClosed=false;M().adapted=true;M().lessons[2]=M().kind==='algebra'?'加练 · 系数与线性组合':'加练 · 方向与齐次坐标';}}else if(quizType==='fill'){let answer=document.getElementById('written-answer').value.trim();if(!answer){toast('先填上你的答案');return;}ok=/^(0|零)$/.test(answer);text=ok?'答对了，答案是 0。':'这里应该是 0。回看上面的概念，再试一次。';if(!ok){M().noticeClosed=false;M().adapted=true;M().lessons[2]=M().kind==='algebra'?'加练 · 系数与线性组合':'加练 · 方向与齐次坐标';}}else if(quizType==='short'){const answer=document.getElementById('written-answer').value.trim();if(!answer){toast('写一点你的想法再提交吧');return;}chatMessages.push({role:'user',html:esc(answer)});ok=true;text='已收下你的解释。原型不做自动评分；接入模型后会给出针对性的反馈。';}else{if(!photo){toast('先拍摄或选择作答图片');return;}ok=true;text='图片已在本页准备好。原型尚未接入识图评分，你可以继续体验下一步。';}quizResult={ok,text};updateQuiz();document.querySelector('.quiz-card')?.classList.add(ok?'answer-correct':'answer-retry');}
function advanceLearning(button){if(advancing||!quizResult?.ok)return;advancing=true;button.disabled=true;button.classList.add('next-launch');setTimeout(()=>{advancing=false;nextPoint();},window.matchMedia?.('(prefers-reduced-motion: reduce)').matches?0:190);}
function nextPoint(){if(!quizResult?.ok)return;const kind=quizType;chatMessages.push({role:'assistant',html:`<div class="answered-summary">${I('check')} ${question()[0]}<small>${kind==='short'||kind==='photo'?'作答已提交 · 待 AI 评阅':'已完成理解检查'}</small></div>`});questionStep++;knowledgeDone=Math.min(3,questionStep);quizResult=null;selected=[];photo=null;if(questionStep>=5){knowledgeDone=4;lessonComplete=true;}else{quizType=['single','multi','fill','short','photo'][questionStep];const prompts={multi:'很好。现在换几个说法，看看你能不能辨别它们。',fill:'接着，把这个关键数值补完整。',short:'你已经抓住了主要概念。试着用自己的话讲给我听。',photo:'最后，在纸上画一画。拍照给我，我们一起检查这张图。'};chatMessages.push({role:'assistant',html:`<p>${prompts[quizType]}</p>`});}chat();document.querySelector('.quiz-card')?.classList.add('question-arriving');document.querySelector('.learning-plan')?.classList.add('plan-updated');scrollChat();document.querySelector('.plan-step.just-completed')?.scrollIntoView({block:'nearest',behavior:'smooth'});}
function pointText(i){if(M().kind==='custom')return '此处将展示备课 agent 生成的对应讲解。当前是交互演示。';if(M().kind==='algebra')return ['向量加法可以用首尾相接的方法表示。','数乘改变向量的长度，负系数还会改变方向。','两个不共线的平面向量可以张成整个平面；两个共线向量的组合仍在同一条直线上。','尝试画出 u + v，再解释它为什么是一种线性组合。'][i];return ['平行线在欧氏平面没有交点，在射影平面中我们为它们共同的方向增加一个无穷远点。','无穷远点代表的是方向，不是某个非常大的距离。不同方向对应不同的无穷远点。','在齐次坐标中，z = 0 对应无穷远点。例如 [1 : 2 : 0] 与 [2 : 4 : 0] 表示同一点。','试着解释：平行线是否相交，为什么需要先说明我们讨论的是欧氏平面还是射影平面？'][i];}
function explainPoint(i){chatMessages.push({role:'assistant',html:`<p><strong>${todos()[i][0]}</strong></p><p>${pointText(i)}</p>`});chat();document.getElementById('messages').lastElementChild?.scrollIntoView({behavior:'smooth',block:'center'});}
function readPhoto(input){const file=input.files?.[0];if(!file)return;if(!file.type.startsWith('image/')){toast('请选择图片文件');return;}if(file.size>10*1024*1024){toast('请选择小于 10 MB 的图片');return;}const r=new FileReader();r.onload=()=>{photo=r.result;quizResult=null;updateQuiz();};r.readAsDataURL(file);}
function sendMessage(){const input=document.getElementById('chat-input'),s=input.value.trim();if(!s)return;chatMessages.push({role:'user',html:esc(s)});let reply=/不懂|不会|为什么|困惑/.test(s)?'我们换个角度想：'+pointText(Math.min(knowledgeDone,3)):/懂了|明白|继续/.test(s)?'很好，试着完成这道理解检查，然后点击「继续学习」点亮下一个知识点。':'收到你的想法。我会根据你的回答继续提问；你也可以随时告诉我哪里还不清楚。';chatMessages.push({role:'assistant',html:`<span class="step-label">预设回复演示</span><p>${reply}</p>`});chat();document.getElementById('messages').lastElementChild.scrollIntoView({behavior:'smooth',block:'center'});}
function finishLesson(){M().done=Math.max(M().done,Math.min(6,lessonIndex+1));go('home');toast('本课已完成，学习路线已更新');}
let swipeBusy=false,swipeSuppressUntil=0,wheelQuietUntil=0,activeSwipeTransition=null;
async function returnToRoadmap(){
 if(swipeBusy)return;cancelLessonTransition?.();clearTimeout(planIntroTimer);
 const reduced=window.matchMedia?.('(prefers-reduced-motion: reduce)').matches;
 const oldPane=app.querySelector('.page'),oldScroll=oldPane?.scrollTop||0;
 const outgoing=document.createElement('div');outgoing.className='lesson-opening';outgoing.inert=true;outgoing.setAttribute('aria-hidden','true');
 outgoing.append(...app.childNodes);outgoing.querySelectorAll('[id]').forEach(el=>el.removeAttribute('id'));
 view='home';render();
 const home=app.querySelector('.home-page'),scroll=M().scrollPosition||0;home.classList.add('swipe-settled');home.scrollTop=scroll;
 app.querySelector('.bottom-notice')?.classList.add('settled-notice');
 if(reduced||!outgoing.animate)return;
 const underlay=document.createElement('div');underlay.className='lesson-underlay';underlay.inert=true;underlay.append(...app.childNodes);
 app.append(underlay,outgoing);home.scrollTop=scroll;if(oldPane)oldPane.scrollTop=oldScroll;
 const origin=home.querySelectorAll('.stop')[lessonIndex],frame=app.getBoundingClientRect();
 const bounds=origin?.getBoundingClientRect()||{left:frame.left+24,right:frame.right-24,top:frame.top+120,bottom:frame.top+220};
 const animations=[];swipeBusy=true;setMotionLite(true);
 const finish=()=>{
  if(cancelLessonTransition!==finish)return;
  cancelLessonTransition=null;animations.forEach(a=>a.cancel());outgoing.remove();
  if(underlay.parentNode===app)underlay.replaceWith(...underlay.childNodes);
  home.scrollTop=scroll;swipeBusy=false;setMotionLite(false);origin?.querySelector('.node-hit')?.focus({preventScroll:true});
 };
 cancelLessonTransition=finish;
 try{
  // Replay the same tracks backwards, including each staggered content delay.
  animations.push(...animateLessonTransition(underlay,outgoing,lessonCardClip(bounds,frame),true));
  await Promise.allSettled(animations.map(a=>a.finished));
 }finally{finish();}
}
function swipeSurface(){return view==='home'?document.querySelector('.home-page'):document.querySelector('.lesson-content');}
function swipeTiming(distance,releaseSpeed=null){
 if(releaseSpeed===null)return {duration:720,easing:'cubic-bezier(.14,.68,.16,1)',fill:'both'};
 const speed=Math.max(0,releaseSpeed);
 // About 2x slower than the previous weighted version.
 // Fast flicks still preserve some momentum, but the page visibly carries its mass.
 const duration=Math.max(560,Math.min(1000,820+distance*.24-speed*140));
 return {duration,easing:'cubic-bezier(.10,.64,.14,1)',fill:'both'};
}
function detailDrag(raw,width){
 const sign=Math.sign(raw)||1,d=Math.abs(raw);
 const lead=18,detentStart=width*.31,detentEnd=width*.43,cap=width*.94;
 let x;
 if(d<=lead){
  x=d;
 }else{
  const q=d-lead;
  x=lead+q/(1+q/(width*1.85));
 }
 if(d>detentStart){
  const q0=Math.max(0,detentStart-lead);
  const before=lead+q0/(1+q0/(width*1.85));
  if(d<detentEnd){
   const q=Math.max(0,d-lead);
   const baseAtD=lead+q/(1+q/(width*1.85));
   x=before+(baseAtD-before)*.48;
  }else{
   const qe=Math.max(0,detentEnd-lead);
   const baseEnd=lead+qe/(1+qe/(width*1.85));
   const heldEnd=before+(baseEnd-before)*.48;
   x=heldEnd+(d-detentEnd)*.74;
  }
 }
 x=Math.min(x,cap);
 const slope = d < lead ? 1 : d < detentStart ? .82 : d < detentEnd ? .40 : .70;
 return {offset:sign*x,slope};
}
function createSwipePreview(surface,direction,paintedX=0){
 const home=view==='home',target=home?(mapIndex+direction+maps.length)%maps.length:(direction>0?'materials':'todo');
 if((home&&maps.length<2)||(!home&&target===tab))return null;
 const box=surface.getBoundingClientRect(),appBox=app.getBoundingClientRect(),pane=surface.closest('.page')||app,paneBox=pane.getBoundingClientRect();
 const width=surface.offsetWidth||app.clientWidth,top=Math.max(box.top,paneBox.top,appBox.top),bottom=Math.min(paneBox.bottom,appBox.bottom);
 const gap=home?0:Math.max(32,Math.min(48,width*.12)),travel=width+gap;
 if(bottom<=top)return null;
 const template=document.createElement('template');
 template.innerHTML=home?roadmapHTML(maps[target]):`<div class="lesson-content">${lessonContentHTML(target)}</div>`;
 const preview=template.content.firstElementChild;preview.removeAttribute('id');preview.querySelectorAll('[id]').forEach(el=>el.removeAttribute('id'));preview.classList.add('swipe-settled');
 const host=document.createElement('div');host.className='swipe-preview-stage';host.inert=true;host.setAttribute('aria-hidden','true');
 Object.assign(host.style,{left:(box.left-paintedX-appBox.left)+'px',top:(top-appBox.top)+'px',width:width+'px',height:(bottom-top)+'px'});
 Object.assign(preview.style,{position:'absolute',top:(box.top-top)+'px',left:'0px',width:width+'px',height:home?box.height+'px':'auto',maxHeight:'none',margin:'0',transform:`translate3d(${direction*travel+paintedX}px,0,0)`});
 host.append(preview);app.append(host);if(home)preview.scrollTop=maps[target].scrollPosition||0;
 return {surface:preview,host,width,travel,gap,direction,paint(x){const p=Math.min(1,Math.abs(x)/Math.max(1,width));const s=1-Math.min(.0035,p*.0035);preview.style.transform=`translate3d(${direction*travel+x}px,0,0) scale(${s})`;},dispose(){host.remove();},adopt(commit){
  const scroll=preview.scrollTop;preview.remove();preview.style.cssText='';if(home)preview.id='swipe-zone';commit(preview);preview.scrollTop=scroll;host.remove();
 }};
}
async function settleSwipeBack(surface,preview,startX){
 if(!surface.isConnected){preview?.dispose();return;}
 const reduced=window.matchMedia?.('(prefers-reduced-motion: reduce)').matches;
 if(reduced){surface.style.transform='';preview?.dispose();return;}
 swipeBusy=true;const animations=[],distance=Math.abs(startX);
 const timing={duration:Math.max(480,Math.min(720,470+distance*.96)),easing:'cubic-bezier(.14,.66,.18,1)',fill:'both'};
 const recoil=Math.sign(startX)*Math.min(4,Math.max(1.5,distance*.025));
 let finished=false;
 const finish=()=>{
  if(finished)return;
  finished=true;
  animations.forEach(a=>{try{a.cancel()}catch{}});
  surface.style.transform='';
  preview?.dispose();
  swipeBusy=false;
  if(activeSwipeTransition?.finish===finish)activeSwipeTransition=null;
 };
 activeSwipeTransition={finish,type:'return'};
 try{
  animations.push(surface.animate([
   {transform:`translate3d(${startX}px,0,0) scale(.998)`},
   {transform:`translate3d(${-recoil}px,0,0) scale(.9995)`,offset:.86},
   {transform:'translate3d(0,0,0) scale(1)'}
  ],timing));
  if(preview)animations.push(preview.surface.animate([
   {transform:`translate3d(${preview.direction*preview.travel+startX}px,0,0) scale(.998)`},
   {transform:`translate3d(${preview.direction*preview.travel-recoil}px,0,0) scale(.9995)`,offset:.86},
   {transform:`translate3d(${preview.direction*preview.travel}px,0,0) scale(1)`}
  ],timing));
  await Promise.allSettled(animations.map(a=>a.finished));
 }finally{finish();}
}
async function slideAcross(surface,direction,commit,startX=0,releaseVelocity=null,preparedPreview=null){
 if(swipeBusy){preparedPreview?.dispose();return;}
 if(!surface?.animate||window.matchMedia?.('(prefers-reduced-motion: reduce)').matches){preparedPreview?preparedPreview.adopt(commit):commit();return;}
 const preview=preparedPreview||createSwipePreview(surface,direction,startX);
 if(!preview){surface.style.transform='';commit();return;}
 swipeBusy=true;setMotionLite(true);surface.classList.add('swipe-settled');const animations=[];
 let finished=false,committed=false;
 const finish=()=>{
  if(finished)return;
  finished=true;
  animations.forEach(a=>{try{a.cancel()}catch{}});
  surface.style.transform='';
  surface.classList.remove('swipe-landing');
  preview.surface?.classList?.remove('swipe-landing');
  if(surface.isConnected&&!committed){
   committed=true;
   preview.adopt(commit);
  }else{
   preview.dispose();
  }
  swipeBusy=false;
  setMotionLite(false);
  if(activeSwipeTransition?.finish===finish)activeSwipeTransition=null;
 };
 activeSwipeTransition={finish,type:'commit'};
 try{
  const timing=swipeTiming(Math.max(1,preview.travel-Math.abs(startX)),releaseVelocity===null?null:Math.max(0,-direction*releaseVelocity));
  const seat=Math.min(9,Math.max(5,preview.width*.016));
  surface.classList.add('swipe-landing');preview.surface.classList.add('swipe-landing');
  animations.push(surface.animate([
   {transform:`translate3d(${startX}px,0,0) scale(.998)`},
   {transform:`translate3d(${-direction*(preview.travel+seat)}px,0,0) scale(.997)`,offset:.72},
   {transform:`translate3d(${-direction*preview.travel}px,0,0) scale(1)`}
  ],timing));
  animations.push(preview.surface.animate([
   {transform:`translate3d(${direction*preview.travel+startX}px,0,0) scale(.998)`},
   {transform:`translate3d(${-direction*seat}px,0,0) scale(.999)`,offset:.72},
   {transform:'translate3d(0,0,0) scale(1)'}
  ],timing));
  const start=document.timeline?.currentTime;if(start!=null)animations.forEach(a=>a.startTime=start);
  await Promise.allSettled(animations.map(a=>a.finished));
 }finally{finish();}
}
function bindSwipe(){
 const zone=document.getElementById('swipe-zone');if(!zone)return;
 let gesture=null,dragFrame=null;
 const position=g=>{const x=g.paintX??g.dx;const p=Math.min(1,Math.abs(x)/(g.preview?.width||app.clientWidth));const s=1-Math.min(.0045,p*.0045);g.surface.style.transform=`translate3d(${x}px,0,0) scale(${s})`;g.preview?.paint(x);g.paintedX=x;};
 const paintDrag=g=>{if(dragFrame!==null)return;dragFrame=requestAnimationFrame(()=>{dragFrame=null;if(gesture===g||wheelGesture===g)position(g);});};
 const flushDrag=g=>{if(dragFrame!==null){cancelAnimationFrame(dragFrame);dragFrame=null;}position(g);};
 const prepare=(g,dx,available)=>{
  const direction=dx<0?1:-1;
  if(g.preview&&(!available||g.preview.direction!==direction)){g.preview.dispose();g.preview=null;}
  if(available&&!g.preview)g.preview=createSwipePreview(g.surface,direction,g.paintedX||0);
 };
 const record=(g,x,now)=>{g.samples.push({x,t:now});while(g.samples.length>2&&g.samples[1].t<now-90)g.samples.shift();};
 const speedAt=(g,x,now)=>{const first=g.samples.find(p=>now-p.t<=100);return first?(x-first.x)/Math.max(1,now-first.t):0;};
 const reset=()=>{if(!gesture)return;const g=gesture;flushDrag(g);gesture=null;zone.classList.remove('is-dragging');if(g.horizontal)settleSwipeBack(g.surface,g.preview,g.dx);else g.preview?.dispose();};
 const begin=e=>{
  if(e.button>0||e.isPrimary===false||e.target.closest('input,textarea'))return;
  // The newest touch wins. If a long inertial settle is still playing,
  // complete its state synchronously and hand control to this gesture.
  if(swipeBusy&&activeSwipeTransition?.finish)activeSwipeTransition.finish();
  if(swipeBusy)return;
  gesture={id:e.pointerId,x:e.clientX,y:e.clientY,dx:0,horizontal:false,surface:swipeSurface(),started:performance.now(),samples:[{x:0,t:performance.now()}],preview:null,paintedX:0};
 };
 const move=e=>{
  const g=gesture;if(!g||g.id!==e.pointerId)return;
  const dx=e.clientX-g.x,dy=e.clientY-g.y;
  if(!g.horizontal){if(Math.abs(dy)>10&&Math.abs(dy)>Math.abs(dx)){gesture=null;return;}if(Math.abs(dx)<10||Math.abs(dx)<Math.abs(dy)*1.25)return;g.horizontal=true;zone.classList.add('is-dragging');if(!e.fromTouch){try{zone.setPointerCapture?.(e.pointerId);}catch{}}pressedPointers.forEach(button=>controlMotion.get(button)?.cancel());pressedPointers.clear();}
  e.preventDefault();
  const available=view==='home'?maps.length>1:(dx<0?tab!=='materials':tab!=='todo');
  prepare(g,dx,available);record(g,dx,performance.now());
  const damped=available?detailDrag(dx,g.preview?.width||app.clientWidth):null;
  g.dragSlope=damped?.slope??1;
  g.dx=damped?damped.offset:dx*.18;paintDrag(g);
 };
 const end=e=>{
  const g=gesture;if(!g||g.id!==e.pointerId)return;
  if(!g.horizontal){gesture=null;return;}
  swipeSuppressUntil=performance.now()+90;
  const raw=e.clientX-g.x,velocity=speedAt(g,raw,performance.now());
  const available=view==='home'?maps.length>1:(raw<0?tab!=='materials':tab!=='todo');
  const commitDistance=Math.min(132,app.clientWidth*.335);const fastFlick=Math.abs(raw)>26&&Math.abs(velocity)>.58;if(!available||!(Math.abs(raw)>commitDistance||fastFlick)){reset();return;}
  flushDrag(g);gesture=null;zone.classList.remove('is-dragging');
  const direction=raw<0?1:-1;
  slideAcross(g.surface,direction,prepared=>{if(view==='home')switchMap(mapIndex+direction,false,prepared);else setTab(raw<0?'materials':'todo',false,prepared);},g.dx,velocity*(g.dragSlope??1),g.preview);
 };
 zone.addEventListener('pointerdown',e=>{if(e.pointerType!=='touch')begin(e);});
 zone.addEventListener('pointermove',e=>{if(e.pointerType!=='touch')move(e);});
 zone.addEventListener('pointerup',e=>{if(e.pointerType!=='touch')end(e);});
 zone.addEventListener('pointercancel',e=>{if(e.pointerType!=='touch')reset();});
 const touchEvent=(event,touch)=>({pointerId:'touch-'+touch.identifier,clientX:touch.clientX,clientY:touch.clientY,button:0,target:event.target,fromTouch:true,preventDefault:()=>{if(event.cancelable)event.preventDefault();}});
 zone.addEventListener('touchstart',e=>{if(e.touches.length===1)begin(touchEvent(e,e.touches[0]));else reset();},{passive:true});
 zone.addEventListener('touchmove',e=>{const t=[...e.touches].find(t=>'touch-'+t.identifier===gesture?.id);if(t)move(touchEvent(e,t));},{passive:false});
 zone.addEventListener('touchend',e=>{const t=[...e.changedTouches].find(t=>'touch-'+t.identifier===gesture?.id);if(t)end(touchEvent(e,t));},{passive:true});
 zone.addEventListener('touchcancel',reset,{passive:true});
 // Capture may transfer from a touched child button to this container.
 // The child's bubbling loss is a handoff, not the end of the gesture.
 zone.addEventListener('lostpointercapture',e=>{if(e.target===zone&&gesture?.id===e.pointerId)reset();});
 let wheelGesture=null,wheelTimer=null;
 zone.addEventListener('wheel',e=>{
  if(e.ctrlKey)return;
  const raw=e.deltaX||(e.shiftKey?e.deltaY:0);
  if(!raw||(!e.shiftKey&&Math.abs(raw)<=Math.abs(e.deltaY)*1.15))return;
  e.preventDefault();const now=performance.now();
  if(now<wheelQuietUntil){wheelQuietUntil=Math.max(wheelQuietUntil,now+70);return;}
  if(swipeBusy&&activeSwipeTransition?.finish)activeSwipeTransition.finish();
  if(swipeBusy||gesture)return;
  if(!wheelGesture)wheelGesture={surface:swipeSurface(),dx:0,samples:[{x:0,t:now}],velocity:0,preview:null,paintedX:0};
  const g=wheelGesture,factor=e.deltaMode===1?16:e.deltaMode===2?app.clientWidth:1;
  g.dx=Math.max(-app.clientWidth*.8,Math.min(app.clientWidth*.8,g.dx-raw*factor));
  const available=view==='home'?maps.length>1:(g.dx<0?tab!=='materials':tab!=='todo');
  prepare(g,g.dx,available);
  const damped=available?detailDrag(g.dx,g.preview?.width||app.clientWidth):null;
  const visual=damped?damped.offset:g.dx*.18;
  record(g,g.dx,now);g.velocity=Math.max(-3,Math.min(3,speedAt(g,g.dx,now)))*(damped?.slope??1);g.paintX=visual;paintDrag(g);
  clearTimeout(wheelTimer);
  wheelTimer=setTimeout(()=>{
   flushDrag(g);wheelGesture=null;if(!g.surface.isConnected)return;
   swipeSuppressUntil=performance.now()+80;wheelQuietUntil=performance.now()+80;
   if(!available||Math.abs(g.dx)<Math.min(88,app.clientWidth*.22)){
    settleSwipeBack(g.surface,g.preview,visual);return;
   }
   const direction=g.dx<0?1:-1;
   slideAcross(g.surface,direction,prepared=>{if(view==='home')switchMap(mapIndex+direction,false,prepared);else setTab(g.dx<0?'materials':'todo',false,prepared);},visual,g.velocity,g.preview);
  },40);
 },{passive:false});
}
let toastTimer;function toast(s){const t=document.getElementById('toast');t.textContent=s;t.classList.add('show');clearTimeout(toastTimer);toastTimer=setTimeout(()=>t.classList.remove('show'),3000);}
if(document.modelContext?.registerTool){try{document.modelContext.registerTool({name:'open_learning_lesson',title:'打开学习课程',description:'打开当前路线中的指定课程预习页。',inputSchema:{type:'object',properties:{lessonIndex:{type:'integer',minimum:0,maximum:5}},required:['lessonIndex'],additionalProperties:false},annotations:{readOnlyHint:false},execute:async x=>{if(!Number.isInteger(x.lessonIndex)||x.lessonIndex<0||x.lessonIndex>5)throw Error('课程索引必须为 0 至 5');openLesson(x.lessonIndex);return {page:view,lesson:M().lessons[lessonIndex]};}});}catch{}}
render();

function syncAppHeight(){document.documentElement.style.setProperty('--app-height',(window.visualViewport?.height||window.innerHeight)+'px');}syncAppHeight();window.visualViewport?.addEventListener('resize',syncAppHeight);window.addEventListener('resize',syncAppHeight);

// Keep press/release feedback alive long enough to be seen before replacing a view.
const controlMotion=new WeakMap(),releaseTimes=new WeakMap(),pressedPointers=new Map(),queuedClicks=new WeakSet(),forwardedClicks=new WeakSet();
const reducedFeedback=()=>window.matchMedia?.('(prefers-reduced-motion: reduce)').matches;
function feedbackFace(button){return button.querySelector('.node,.info-face')||button;}
function animateControl(button,releasing=false,pulse=false){
 if(!button||button.disabled||reducedFeedback())return;
 const face=feedbackFace(button);if(!face.animate)return;
 const previous=controlMotion.get(button);
 const scale=parseFloat(getComputedStyle(face).scale)||1;
 previous?.cancel();
 const pressed=button.matches('.node-hit,.primary,.send') ? .91 : .965;
 const frames=releasing?[
  {scale:pulse?1:scale,offset:0},
  ...(pulse?[{scale:pressed,offset:.18}]:[]),
  {scale:1.055,offset:.53},
  {scale:.987,offset:.72},
  {scale:1,offset:1}
 ]:[{scale},{scale:pressed}];
 const animation=face.animate(frames,{duration:releasing?340:85,easing:releasing?'cubic-bezier(.2,.65,.35,1)':'ease-out',fill:'forwards'});
 controlMotion.set(button,animation);
 if(releasing){releaseTimes.set(button,performance.now());animation.finished.then(()=>{if(controlMotion.get(button)===animation){animation.cancel();controlMotion.delete(button);}}).catch(()=>{});}
}
document.addEventListener('pointerdown',e=>{
 if(e.target.closest?.('#learning-plan'))clearTimeout(planIntroTimer);
 const button=e.target.closest?.('button');if(!button||button.disabled||e.button>0)return;
 pressedPointers.set(e.pointerId,button);animateControl(button);
});
document.addEventListener('pointerup',e=>{
 const button=pressedPointers.get(e.pointerId);pressedPointers.delete(e.pointerId);
 if(button)animateControl(button,true);
});
document.addEventListener('pointercancel',e=>{
 const button=pressedPointers.get(e.pointerId);pressedPointers.delete(e.pointerId);
 if(button){controlMotion.get(button)?.cancel();controlMotion.delete(button);}
});
window.addEventListener('blur',()=>{pressedPointers.forEach(button=>controlMotion.get(button)?.cancel());pressedPointers.clear();});
document.addEventListener('click',e=>{
 if(swipeBusy||performance.now()<swipeSuppressUntil){e.preventDefault();e.stopImmediatePropagation();return;}
 const button=e.target.closest?.('button');if(!button||button.disabled)return;
 if(forwardedClicks.has(button)){forwardedClicks.delete(button);return;}
 if(reducedFeedback())return;
 if(performance.now()-(releaseTimes.get(button)||-Infinity)>150)animateControl(button,true,true);
 // Launch actions already own their own transition. Other controls should react almost immediately.
 if(/launchLearning|advanceLearning/.test(button.getAttribute('onclick')||''))return;
 e.preventDefault();e.stopImmediatePropagation();
 if(queuedClicks.has(button))return;
 queuedClicks.add(button);
 setTimeout(()=>{
  queuedClicks.delete(button);
  if(!button.isConnected||button.disabled||swipeBusy||performance.now()<swipeSuppressUntil)return;
  forwardedClicks.add(button);button.click();
 },/openRoadmapLayer|closeRoadmapLayer/.test(button.getAttribute('onclick')||'')?36:52);
},true);
document.addEventListener('focusin',e=>{if(e.target.closest?.('#learning-plan'))clearTimeout(planIntroTimer);});

