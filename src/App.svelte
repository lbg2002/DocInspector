<script lang="ts">
 import { onMount } from 'svelte';
 import { invoke } from '@tauri-apps/api/core';
 import { listen } from '@tauri-apps/api/event';
 import { open, save } from '@tauri-apps/plugin-dialog';
 import { readFile, writeTextFile } from '@tauri-apps/plugin-fs';
 import { FolderOpen, Files, Plus, Search, Settings2, ShieldCheck, Pencil, Trash2, Copy, ArrowLeft, Play, X, Check, FileText, ChevronRight, Download, RotateCcw, PanelLeftClose, PanelLeftOpen, Clock3, FileWarning, CircleCheck, Ellipsis, ClipboardCopy, Upload, ListFilter } from 'lucide-svelte';
 import type { Profile, Rule, Kind, ScanProgress, ScanReport, FileResult } from './lib/types';

 type Page = 'home' | 'editor' | 'results' | 'settings';
 interface HistoryMeta {
  id:string; profileId:string; profileName:string; timestamp:number;
  sources:string[]; total:number; scanned:number; issues:number; unreadable:number;
  cancelled:boolean; truncated:boolean;
 }

 const mobile = /Android/i.test(navigator.userAgent);
 const STORAGE = 'docinspector_profiles_v1';
 const ACTIVE = 'docinspector_active_v2';
 const id = () => Math.random().toString(36).slice(2) + Date.now().toString(36);
 const template = ():Profile => ({
  id:id(), name:'论文终稿检查', icon:'◆', color:'#a79be8', created:Date.now(),
  rules:['TODO','待补充','占位符'].map(word=>({id:id(),word,kind:'forbidden',enabled:true}))
 });

 let profiles:Profile[] = $state([]);
 let active = $state('');
 let page:Page = $state('home');
 let sidebar = $state(true);
 let sources:string[] = $state([]);
 let keyword = $state('');
 let busy = $state(false);
 let cancelling = $state(false);
 let progress:ScanProgress|null = $state(null);
 let result:ScanReport|null = $state(null);
 let history:HistoryMeta[] = $state([]);
 let openedHistoryId = $state('');
 let tab:'all'|'issue'|'passed'|'unreadable' = $state('all');
 let selected = $state('');
 let notice = $state('');
 let cancelledMobile = false;

 let editingId = $state('');
 let draftName = $state('');
 let draftRules:Rule[] = $state([]);
 let newWord = $state('');
 let newKind:Kind = $state('forbidden');
 let importText = $state('');

 const profile:Profile|undefined = $derived(profiles.find(p=>p.id===active));
 const profileHistory = $derived(history.filter(h=>h.profileId===active));
 const filtered:FileResult[] = $derived(result?.files.filter(f=>tab==='all'||f.status===tab)??[]);
 const chosen:FileResult|undefined = $derived(result?.files.find(f=>f.path===selected));
 const issues = $derived(result?.files.filter(f=>f.status==='issue').length??0);
 const unreadable = $derived(result?.files.filter(f=>f.status==='unreadable').length??0);
 const passed = $derived(result?.files.filter(f=>f.status==='passed').length??0);

 const title = (p:string) => p.split(/[\\/]/).pop()||p;
 const kindName = (k:Kind)=>({search:'检索',forbidden:'禁止',required:'必需'}[k]);
 const statusName = (s:string)=>({passed:'通过',issue:'问题',unreadable:'未检查'}[s]||s);
 const formatTime = (stamp:number) => new Date(stamp*1000).toLocaleString('zh-CN',{month:'2-digit',day:'2-digit',hour:'2-digit',minute:'2-digit'});
 const countRules = (p:Profile)=>p.rules.filter(r=>r.enabled).length;
 function saveProfiles() {
  try {
   localStorage.setItem(STORAGE,JSON.stringify(profiles));
   localStorage.setItem(ACTIVE,active);
  } catch(e) {notice='无法保存方案：'+String(e);}
 }
 async function loadHistory() {
  try {history = await invoke<HistoryMeta[]>('list_scan_history');}
  catch(e) {notice='读取历史失败：'+String(e);}
 }
 onMount(()=>{
  try {
   const saved=JSON.parse(localStorage.getItem(STORAGE)||'null');
   profiles=Array.isArray(saved)?saved.filter(p=>p && typeof p.id==='string' && Array.isArray(p.rules)):[template()];
  }catch {profiles=[template()];}
  active=localStorage.getItem(ACTIVE)||profiles[0]?.id||'';
  if(!profiles.some(p=>p.id===active))active=profiles[0]?.id||'';
  saveProfiles();
  void loadHistory();
  const dispose=listen<ScanProgress>('scan-progress',e=>{progress=e.payload;});
  return ()=>{void dispose.then(off=>off());};
 });

 function chooseProfile(pid:string) {
  if(busy)return;
  active=pid;
  page='home';
  saveProfiles();
 }
 function createProfile() {
  if(busy)return;
  editingId='';
  draftName='';
  draftRules=[];
  newWord='';
  page='editor';
 }
 function editProfile() {
  if(!profile || busy)return;
  editingId=profile.id;
  draftName=profile.name;
  draftRules=profile.rules.map(r=>({...r}));
  newWord='';
  page='editor';
 }
 function updateRule(i:number,changes:Partial<Rule>) {
  draftRules=draftRules.map((r,j)=>j===i?{...r,...changes}:r);
 }
 function addRules() {
  const words=[...new Set(newWord.split(/[\n,，;；]+/).map(x=>x.trim()).filter(Boolean))];
  for(const word of words) {
   if(!draftRules.some(r=>r.word===word&&r.kind===newKind)){
    draftRules=[...draftRules,{id:id(),word,kind:newKind,enabled:true}];
   }
  }
  newWord='';
 }
 function saveProfile() {
  const name=draftName.trim();
  if(!name){notice='请输入方案名称';return;}
  const unique=new Set<string>();
  const rules:Rule[]=[];
  for(const rule of draftRules){
   const word=rule.word.trim();
   if(!word)continue;
   const key=rule.kind+'\0'+word;
   if(!unique.has(key)){unique.add(key);rules.push({...rule,word});}
  }
  if(editingId){
   profiles=profiles.map(p=>p.id===editingId?{...p,name,rules}:p);
   active=editingId;
  } else {
   const newProfile:Profile={id:id(),name,created:Date.now(),icon:'◆',color:'#8bacd8',rules};
   profiles=[...profiles,newProfile];
   active=newProfile.id;
  }
  saveProfiles();
  notice='方案已保存';
  page='home';
 }
 function duplicateProfile() {
  if(!profile||busy)return;
  const cloned:Profile={...profile,id:id(),name:profile.name+' 副本',
   created:Date.now(),rules:profile.rules.map(r=>({...r,id:id()}))};
  profiles=[...profiles,cloned];
  active=cloned.id;
  saveProfiles();
 }
 function removeProfile() {
  if(!profile||busy)return;
  const name=profile.name;
  if(!confirm('删除方案「'+name+'」？历史记录将保留在本机，但该方案的记录不再列于其他方案下。'))return;
  profiles=profiles.filter(p=>p.id!==active);
  active=profiles[0]?.id||'';
  saveProfiles();
 }
 async function pick(directory:boolean) {
  if(busy)return;
  if(mobile&&directory){notice='Android 暂不支持文件夹递归，请选择文件';return;}
  try {
   const v=await open({directory,multiple:!directory,...(mobile?{pickerMode:'document' as const}:{})});
   if(v){const a=Array.isArray(v)?v:[v];sources=[...new Set([...sources,...a])];}
  } catch(e){notice='无法选择文件：'+String(e);}
 }
 function simpleRules():Rule[] {
  return [...new Set(keyword.split(/[\n,，;；]+/).map(x=>x.trim()).filter(Boolean))]
   .map(word=>({id:id(),word,kind:'search' as Kind,enabled:true}));
 }
 async function startScan() {
  if(busy)return;
  if(!profile){notice='请先选择或创建一个方案';return;}
  if(!sources.length){notice='请先选择文件或文件夹';return;}
  const rules=[...profile.rules.filter(r=>r.enabled),...simpleRules()];
  if(!rules.some(r=>r.word.trim())){notice='请添加检查关键词';return;}
  const scanProfile={id:profile.id,name:profile.name};
  const scanSources=[...sources];
  busy=true;cancelling=false;cancelledMobile=false;
  result=null;openedHistoryId='';selected='';progress=null;tab='all';page='results';notice='';
  try {
   let completed:ScanReport;
   if(mobile) {
    const files:FileResult[]=[];
    for(const uri of scanSources){
     if(cancelledMobile)break;
     let name='选中文件';
     try {
      name=decodeURIComponent(uri.split('/').pop()||'选中文件');
      const bytes=await readFile(uri);
      if(bytes.length>24*1024*1024)throw Error('文件超过 24 MB');
      const scanned=await invoke<FileResult>('scan_bytes',{name,bytes:Array.from(bytes),rules});
      files.push({...scanned,path:name});
     }catch(e){
      files.push({path:name,status:'unreadable',hits:[],missing:[],error:String(e)});
     }
     progress={scanned:files.length,total:scanSources.length,current:name};
    }
    completed={files,total:scanSources.length,scanned:files.length,cancelled:cancelledMobile};
   } else {
    completed=await invoke<ScanReport>('scan_paths',{paths:scanSources,rules});
   }
   result=completed;
   selected=completed.files[0]?.path||'';
   if(completed.scanned>0) {
    try{
     const meta=await invoke<HistoryMeta>('save_scan_history',{
      profileId:scanProfile.id,profileName:scanProfile.name,sources:scanSources,report:completed
     });
     history=[meta,...history].slice(0,30);
     openedHistoryId=meta.id;
    }catch(e){notice='检查完成，但历史保存失败：'+String(e);}
   }
  }catch(e){notice='扫描失败：'+String(e);}
  finally{busy=false;cancelling=false;}
 }
 async function cancelScan() {
  if(!busy)return;
  cancelling=true;
  if(mobile){cancelledMobile=true;return;}
  try{await invoke('cancel_scan');}catch(e){notice=String(e);}
 }
 async function openHistory(entry:HistoryMeta) {
  if(busy)return;
  try {
   const old=await invoke<ScanReport>('load_scan_history',{id:entry.id});
   sources=[...entry.sources];
   result=old;openedHistoryId=entry.id;
   selected=old.files[0]?.path||'';
   tab='all';progress=null;page='results';
  }catch(e){notice='无法打开记录：'+String(e);}
 }
 async function deleteHistory(entry:HistoryMeta) {
  if(busy)return;
  if(!confirm('删除这条扫描历史？'))return;
  try{
   await invoke('delete_scan_history',{id:entry.id});
   history=history.filter(h=>h.id!==entry.id);
   if(openedHistoryId===entry.id)openedHistoryId='';
  }catch(e){notice='删除失败：'+String(e);}
 }
 async function exportCsv() {
  if(!result)return;
  try{
   if(mobile){
    const csv=await invoke<string>('export_csv_text',{report:result});
    try{
     const uri=await save({defaultPath:'docinspector-results.csv',filters:[{name:'CSV',extensions:['csv']}]});
     if(uri){await writeTextFile(uri,csv);notice='已保存 CSV';}
    }catch(e){
     try{await navigator.clipboard.writeText(csv);notice='CSV 已复制到剪贴板';}
     catch{notice='保存失败：'+String(e);}
    }
   }else{
    const path=await save({defaultPath:'docinspector-results.csv',filters:[{name:'CSV',extensions:['csv']}]});
    if(path){await invoke('export_csv',{path,report:result});notice='报告已导出';}
   }
  }catch(e){notice='导出失败：'+String(e);}
 }
 async function copyProfileJson() {
  try{await navigator.clipboard.writeText(JSON.stringify({version:1,profiles},null,2));notice='方案 JSON 已复制';}
  catch(e){notice='无法复制配置：'+String(e);}
 }
 function importProfiles() {
  if(!importText.trim()){notice='请先粘贴方案 JSON';return;}
  try{
   const data=JSON.parse(importText);
   if(!Array.isArray(data.profiles))throw Error('方案格式不正确');
   const imported:Profile[]=data.profiles.filter((p:any)=>
    p && typeof p.name==='string' && Array.isArray(p.rules) &&
    p.rules.every((r:any)=>r && typeof r.word==='string' && ['search','forbidden','required'].includes(r.kind)))
    .map((p:Profile)=>({
     ...p,id:id(),rules:p.rules.map(r=>({...r,id:id(),enabled:r.enabled!==false}))
    }));
   if(!imported.length)throw Error('没有可导入的方案');
   profiles=[...profiles,...imported];
   active=imported[0].id;importText='';
   saveProfiles();notice='已导入 '+imported.length+' 个方案';page='home';
  }catch(e){notice='导入失败：'+String(e);}
 }
 function copyPath(path:string) {
  void navigator.clipboard.writeText(path).then(()=>notice='路径已复制',e=>notice='复制失败：'+String(e));
 }
</script>

<svelte:head><title>DocInspector</title></svelte:head>

<div class="shell">
 <aside class:collapsed={!sidebar}>
  <div class="brand">
   <div class="brand-icon"><ShieldCheck size={20}/></div>
   {#if sidebar}<span>DocInspector</span>{/if}
   <button class="icon push" title="收起或展开侧栏" aria-label="切换侧栏" onclick={()=>sidebar=!sidebar}>
    {#if sidebar}<PanelLeftClose size={17}/>{:else}<PanelLeftOpen size={17}/>{/if}
   </button>
  </div>
  {#if sidebar}
   <div class="side-caption">检查方案 <button class="icon" title="新建方案" onclick={createProfile}><Plus size={17}/></button></div>
  {/if}
  <div class="scheme-list">
   {#each profiles as p (p.id)}
    <button class:active={active===p.id} class="scheme-item" title={p.name}
     onclick={()=>chooseProfile(p.id)} disabled={busy}>
     <span class="scheme-dot" style:background={p.color||'#8bacd8'}></span>
     {#if sidebar}<span class="scheme-name">{p.name}</span>{/if}
    </button>
   {/each}
   {#if !profiles.length}
    <button class="scheme-new" onclick={createProfile}><Plus size={17}/>{#if sidebar} 新建方案{/if}</button>
   {/if}
  </div>
  <div class="side-bottom">
   <button class:current={page==='settings'} title="设置" onclick={()=>page='settings'}>
    <Settings2 size={18}/>{#if sidebar}<span>设置</span>{/if}
   </button>
  </div>
 </aside>

 <main>
  {#if page==='home'}
   <header class="workspace-header">
    <div class="heading-group">
     {#if mobile}<select class="mobile-scheme" value={active} onchange={e=>chooseProfile(e.currentTarget.value)}>
      {#each profiles as p}<option value={p.id}>{p.name}</option>{/each}
     </select>{/if}
     <h1>{profile?.name||'新建检查方案'}</h1>
     {#if profile}<span class="heading-meta">{countRules(profile)} 条规则</span>{/if}
    </div>
    <div class="header-actions">
     {#if profile}
      <button class="icon" aria-label="编辑方案" title="编辑方案" onclick={editProfile}><Pencil size={19}/></button>
      <button class="icon" aria-label="复制方案" title="复制方案" onclick={duplicateProfile}><Copy size={18}/></button>
      <button class="icon" aria-label="删除方案" title="删除方案" onclick={removeProfile}><Trash2 size={18}/></button>
     {/if}
    </div>
   </header>
   <div class="content workspace">
    {#if profile}
     <section class="source-section">
      <div class="section-head"><h2>扫描文件</h2>
       {#if sources.length}<button class="text-btn" onclick={()=>sources=[]}>清空</button>{/if}
      </div>
      <div class="sources">
       {#if !mobile}
        <button class="source-card" onclick={()=>pick(true)}>
         <span class="source-icon"><FolderOpen size={24}/></span>
         <strong>选择文件夹</strong><ChevronRight size={18} class="source-arrow"/>
        </button>
       {/if}
       <button class="source-card" onclick={()=>pick(false)}>
        <span class="source-icon"><Files size={24}/></span>
        <strong>选择文件</strong><ChevronRight size={18} class="source-arrow"/>
       </button>
      </div>
      {#if sources.length}
       <div class="path-list">
        {#each sources as source (source)}
         <div class="path-chip"><FolderOpen size={14}/><span title={source}>{source}</span>
          <button title="移除来源" aria-label="移除来源" onclick={()=>sources=sources.filter(x=>x!==source)}><X size={14}/></button>
         </div>
        {/each}
       </div>
      {/if}
      <div class="scan-controls">
       <div class="search-input"><Search size={17}/><input bind:value={keyword} placeholder="临时添加检索词（可选，以逗号分隔）"/></div>
       <button class="primary" onclick={startScan}><Play size={15} fill="currentColor"/> 开始扫描</button>
      </div>
     </section>
     <section class="history-section">
      <div class="section-head"><h2>扫描历史</h2><span class="section-count">{profileHistory.length}</span></div>
      {#if profileHistory.length}
       <div class="history-list">
        {#each profileHistory as entry (entry.id)}
         <div class="history-line">
          <button class="history-record" onclick={()=>openHistory(entry)}>
           <span class="history-symbol"><Clock3 size={18}/></span>
           <span class="history-main">
            <strong>{entry.sources.length===1?title(entry.sources[0]):entry.sources.length+' 个来源'}</strong>
            <span class="history-time">{formatTime(entry.timestamp)} · {entry.scanned} 个文件{entry.cancelled?' · 已取消':''}</span>
           </span>
           <span class="history-stats">
            {#if entry.issues}<span class="history-issue">{entry.issues} 个问题文件</span>{/if}
            {#if entry.unreadable}<span class="history-unreadable">{entry.unreadable} 未检查</span>{/if}
            {#if !entry.issues&&!entry.unreadable}<span class="history-pass">全部通过</span>{/if}
           </span>
           <ChevronRight size={17} color="#879ab6"/>
          </button>
          <button class="icon history-delete" title="删除历史" aria-label="删除历史" onclick={()=>deleteHistory(entry)}><Trash2 size={16}/></button>
         </div>
        {/each}
       </div>
      {:else}
       <div class="history-empty"><Clock3 size={24}/><span>暂无扫描记录</span></div>
      {/if}
     </section>
    {:else}
     <div class="welcome-empty">
      <ShieldCheck size={36}/><h2>添加第一个检查方案</h2>
      <button class="primary" onclick={createProfile}><Plus size={16}/> 新建方案</button>
     </div>
    {/if}
   </div>

  {:else if page==='editor'}
   <header>
    <div class="header-with-back">
     <button class="icon" title="返回" onclick={()=>page='home'}><ArrowLeft size={20}/></button>
     <h1>{editingId?'编辑方案':'新建方案'}</h1>
    </div>
    <button class="primary" onclick={saveProfile}><Check size={17}/> 保存</button>
   </header>
   <div class="content editor">
    <label class="field">方案名称<input bind:value={draftName} placeholder="输入方案名称" autofocus/></label>
    <div class="section-head"><h2>关键词规则</h2><span class="section-count">{draftRules.length}</span></div>
    <div class="rule-composer">
     <div class="rule-add">
      <select bind:value={newKind} aria-label="新增规则类型">
       <option value="forbidden">禁止出现</option><option value="required">必须包含</option><option value="search">仅检索</option>
      </select>
      <textarea bind:value={newWord} rows="2" placeholder="输入关键词，支持换行或逗号批量添加"></textarea>
      <button class="primary" onclick={addRules}><Plus size={16}/> 添加</button>
     </div>
     <div class="rule-list">
      {#each draftRules as rule,i (rule.id)}
       <div class="editable-rule">
        <input type="checkbox" checked={rule.enabled} aria-label="启用规则" onchange={e=>updateRule(i,{enabled:e.currentTarget.checked})}/>
        <select value={rule.kind} aria-label="规则类型" onchange={e=>updateRule(i,{kind:e.currentTarget.value as Kind})}>
         <option value="forbidden">禁止</option><option value="required">必须</option><option value="search">检索</option>
        </select>
        <input class="rule-word" value={rule.word} aria-label="关键词" oninput={e=>updateRule(i,{word:e.currentTarget.value})}/>
        <button class="icon" title="删除关键词" aria-label="删除关键词" onclick={()=>draftRules=draftRules.filter(r=>r.id!==rule.id)}><X size={16}/></button>
       </div>
      {:else}<div class="empty-small">添加关键词</div>{/each}
     </div>
    </div>
   </div>

  {:else if page==='results'}
   <header>
    <div class="header-with-back">
     <button class="icon" title="返回当前方案" onclick={()=>page='home'}><ArrowLeft size={20}/></button>
     <div><h1>扫描结果</h1>
      {#if openedHistoryId}<p>已保存的扫描记录</p>{:else if busy}<p>正在检查文件</p>{:else if result?.cancelled}<p>已取消</p>{/if}
     </div>
    </div>
    <div class="header-actions">
     {#if busy}
      <button class="outline" disabled={cancelling} onclick={cancelScan}><X size={16}/>{cancelling?'正在取消':'停止'}</button>
     {:else if result}
      <button class="outline" onclick={exportCsv}><Download size={16}/> 导出 CSV</button>
      <button class="outline" onclick={startScan}><RotateCcw size={16}/> 重新扫描</button>
     {/if}
    </div>
   </header>
   {#if busy}
    <div class="progress">
     <div class="progress-label">已处理 {progress?.scanned||0} / {progress?.total||'…'} <span class="truncate">{progress?.current||'正在扫描'}</span></div>
     <div class="progress-bar"><div style:width={progress?.total?Math.min(100,100*(progress.scanned/progress.total))+'%':'8%'}></div></div>
    </div>
   {/if}
   {#if result}
    <div class="result-area">
     <div class="stats">
      <div><strong>{result.scanned}</strong><small>已检查</small></div>
      <div><strong>{passed}</strong><small>通过</small></div>
      <div><strong>{issues}</strong><small>有问题</small></div>
      <div><strong>{unreadable}</strong><small>未检查</small></div>
     </div>
     <div class="result-tabs">
      {#each [{key:'all',name:'全部'},{key:'issue',name:'问题'},{key:'passed',name:'通过'},{key:'unreadable',name:'未检查'}] as t}
       <button class:active={tab===t.key} onclick={()=>{tab=t.key as typeof tab;}}>{t.name}</button>
      {/each}
     </div>
     <div class="result-columns">
      <div class="file-results">
       {#each filtered as f,i}
        <button class:active={selected===f.path} class="file-result" onclick={()=>selected=f.path}>
         <FileText size={18}/>
         <span class="truncate"><strong>{title(f.path)}</strong><small title={f.path}>{f.path}</small></span>
         <span class="status" class:issue={f.status==='issue'} class:unreadable={f.status==='unreadable'}>{statusName(f.status)}</span>
        </button>
       {:else}<div class="empty-small">没有符合筛选的文件</div>{/each}
      </div>
      <div class="detail">
       {#if chosen}
        <div class="detail-head"><h2>{title(chosen.path)}</h2><p>{chosen.path} <button class="icon" title="复制路径" onclick={()=>copyPath(chosen!.path)}><ClipboardCopy size={14}/></button></p></div>
        {#if chosen.error}<div class="alert">{chosen.error}</div>{/if}
        {#each chosen.missing as word}
         <div class="hit missing"><span><FileWarning size={16}/> 必须包含 · {word}</span><small>未找到</small></div>
        {/each}
        {#each chosen.hits as hit}
         <div class="hit">
          <div class="hit-label" class:danger={hit.kind==='forbidden'}>{kindName(hit.kind)} · {hit.word}<small>{hit.location}</small></div>
          <p>{#if hit.context.includes(hit.word)}{hit.context.slice(0,hit.context.indexOf(hit.word))}<mark>{hit.word}</mark>{hit.context.slice(hit.context.indexOf(hit.word)+hit.word.length)}{:else}{hit.context}{/if}</p>
         </div>
        {/each}
        {#if !chosen.error&&!chosen.missing.length&&!chosen.hits.length}
         <div class="empty-small">没有命中关键词</div>
        {/if}
       {:else}<div class="empty-small">选择左侧文件查看详情</div>{/if}
      </div>
     </div>
    </div>
   {:else if !busy}
    <div class="empty-main"><FileText size={32}/><h2>尚无扫描结果</h2><button class="primary" onclick={()=>page='home'}>返回方案</button></div>
   {/if}

  {:else if page==='settings'}
   <header><div class="header-with-back"><button class="icon" onclick={()=>page='home'} title="返回"><ArrowLeft size={19}/></button><h1>设置</h1></div></header>
   <div class="content settings">
    <div class="section-head"><h2>方案迁移</h2></div>
    <div class="settings-row"><span>复制全部方案 JSON</span><button class="outline" onclick={copyProfileJson}><Copy size={15}/> 复制</button></div>
    <label class="field">导入方案<textarea rows="5" bind:value={importText} placeholder="粘贴方案 JSON"></textarea></label>
    <button class="outline" onclick={importProfiles}><Upload size={16}/> 导入方案</button>
    <div class="section-head settings-section"><h2>文档支持</h2></div>
    <p>PDF、DOCX、PPTX、XLSX、XLS、ODS、TXT、MD、CSV、LOG，以及 Python、Rust、JavaScript、TypeScript、C/C++、Java、Shell 等代码。扫描件 OCR 暂不支持。</p>
   </div>
  {/if}
  {#if notice}
   <div class="toast" role="status">{notice}
    <button title="关闭提示" onclick={()=>notice=''}><X size={15}/></button>
   </div>
  {/if}
 </main>
</div>