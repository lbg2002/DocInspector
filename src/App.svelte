<script lang="ts">
 import { onMount } from 'svelte';
 import { invoke } from '@tauri-apps/api/core';
 import { listen } from '@tauri-apps/api/event';
 import { open, save } from '@tauri-apps/plugin-dialog';
 import { readFile, writeTextFile } from '@tauri-apps/plugin-fs';
 import {
  FolderOpen, Files, Plus, Search, Settings2, ShieldCheck, Pencil, Trash2, Copy,
  ArrowLeft, Play, X, Check, FileText, ChevronRight, Download, RotateCcw,
  PanelLeftClose, PanelLeftOpen, Clock3, FileWarning, CircleCheck, ClipboardCopy,
  Upload, ListFilter, Sun, Moon, Sparkles, SlidersHorizontal, ExternalLink,
  ArrowUpDown, Layers
 } from 'lucide-svelte';
 import type { Profile, Rule, Kind, ScanOptions, ScanProgress, ScanReport, FileResult, Hit } from './lib/types';

 type Page = 'home' | 'editor' | 'results' | 'settings';
 type Theme = 'dark' | 'light';
 type SortMode = 'status' | 'hits' | 'name';
 type HitFilter = 'all' | 'forbidden' | 'required' | 'search';

 interface HistoryMeta {
  id: string;
  profileId: string;
  profileName: string;
  timestamp: number;
  sources: string[];
  total: number;
  scanned: number;
  issues: number;
  unreadable: number;
  cancelled: boolean;
  truncated: boolean;
  durationMs?: number;
 }

 interface PresetTemplate {
  key: string;
  name: string;
  desc: string;
  color: string;
  rules: Array<{ word: string; kind: Kind }>;
 }

 const mobile = /Android/i.test(navigator.userAgent);
 const STORAGE = 'docinspector_profiles_v1';
 const ACTIVE = 'docinspector_active_v2';
 const THEME_STORAGE = 'docinspector_theme_v1';
 const DEFAULT_OPTIONS: ScanOptions = {
  caseSensitive: true,
  wholeWord: false,
  skipCommonDirs: true,
  excludeDirs: []
 };

 const PALETTE = [
  '#8b7cf6', '#60a5fa', '#34d399', '#f472b6',
  '#fb923c', '#38bdf8', '#a3e635', '#f87171'
 ];

 const PRESET_TEMPLATES: PresetTemplate[] = [
  {
   key: 'thesis',
   name: '论文终稿检查',
   desc: '排查未完成标记并核验论文核心章节要素',
   color: '#8b7cf6',
   rules: [
    { word: 'TODO', kind: 'forbidden' },
    { word: '待补充', kind: 'forbidden' },
    { word: '占位符', kind: 'forbidden' },
    { word: 'XXX', kind: 'forbidden' },
    { word: '摘要', kind: 'required' },
    { word: '参考文献', kind: 'required' }
   ]
  },
  {
   key: 'privacy',
   name: '敏感与内部信息排查',
   desc: '外发材料前检查敏感定级词与密钥字样',
   color: '#f87171',
   rules: [
    { word: '内部公开', kind: 'forbidden' },
    { word: '机密', kind: 'forbidden' },
    { word: '秘密', kind: 'forbidden' },
    { word: '内部资料', kind: 'forbidden' },
    { word: 'password', kind: 'forbidden' },
    { word: 'api_key', kind: 'forbidden' },
    { word: 'secret', kind: 'forbidden' }
   ]
  },
  {
   key: 'code',
   name: '代码调试与待办清理',
   desc: '代码发布前定位遗留调试输出与临时标记',
   color: '#38bdf8',
   rules: [
    { word: 'TODO', kind: 'forbidden' },
    { word: 'FIXME', kind: 'forbidden' },
    { word: 'HACK', kind: 'forbidden' },
    { word: 'console.log', kind: 'search' },
    { word: 'dbg!', kind: 'search' },
    { word: 'println!', kind: 'search' }
   ]
  },
  {
   key: 'contract',
   name: '合同与公文规范核查',
   desc: '检查待定模糊表述并确认签署要素完备',
   color: '#34d399',
   rules: [
    { word: '待定', kind: 'forbidden' },
    { word: '暂定', kind: 'forbidden' },
    { word: '某公司', kind: 'forbidden' },
    { word: '甲方', kind: 'required' },
    { word: '乙方', kind: 'required' }
   ]
  }
 ];

 const id = () => Math.random().toString(36).slice(2) + Date.now().toString(36);

 const buildPresetProfile = (preset: PresetTemplate): Profile => ({
  id: id(),
  name: preset.name,
  icon: '◆',
  color: preset.color,
  created: Date.now(),
  options: { ...DEFAULT_OPTIONS },
  rules: preset.rules.map(r => ({ id: id(), word: r.word, kind: r.kind, enabled: true }))
 });

 const template = (): Profile => ({
  id: id(),
  name: '论文终稿检查',
  icon: '◆',
  color: '#8b7cf6',
  created: Date.now(),
  options: { ...DEFAULT_OPTIONS },
  rules: ['TODO', '待补充', '占位符'].map(word => ({
   id: id(),
   word,
   kind: 'forbidden',
   enabled: true
  }))
 });

 let profiles: Profile[] = $state([]);
 let active = $state('');
 let page: Page = $state('home');
 let theme: Theme = $state('dark');
 let sidebar = $state(true);
 let sources: string[] = $state([]);
 let keyword = $state('');
 let busy = $state(false);
 let cancelling = $state(false);
 let progress: ScanProgress | null = $state(null);
 let result: ScanReport | null = $state(null);
 let history: HistoryMeta[] = $state([]);
 let openedHistoryId = $state('');
 let historyScope: 'current' | 'all' = $state('current');
 let tab: 'all' | 'issue' | 'passed' | 'unreadable' = $state('all');
 let sortMode: SortMode = $state('status');
 let selected = $state('');
 let notice = $state('');
 let draggingFiles = $state(false);
 let cancelledMobile = false;

 // Quick inline rule creator on workspace
 let quickWord = $state('');
 let quickKind: Kind = $state('forbidden');
 let ruleKindFilter: 'all' | Kind = $state('all');

 // Editor state
 let editingId = $state('');
 let draftName = $state('');
 let draftColor = $state('#8b7cf6');
 let draftRules: Rule[] = $state([]);
 let newWord = $state('');
 let newKind: Kind = $state('forbidden');
 let editorFilter: 'all' | Kind = $state('all');
 let editorQuery = $state('');

 // Settings / import state
 let importText = $state('');

 // Results workbench state
 let resultQuery = $state('');
 let splitPercent = $state(36);
 let hitFilter: HitFilter = $state('all');
 let hitWordFilter = $state('');
 let contextMenu: { path: string; x: number; y: number } | null = $state(null);

 const profile: Profile | undefined = $derived(profiles.find(p => p.id === active));
 const options: ScanOptions = $derived({ ...DEFAULT_OPTIONS, ...profile?.options });
 const visibleHistory = $derived(
  historyScope === 'current' ? history.filter(h => h.profileId === active) : history
 );

 const ruleBreakdown = $derived.by(() => {
  const rules = profile?.rules ?? [];
  return {
   total: rules.length,
   enabled: rules.filter(r => r.enabled).length,
   forbidden: rules.filter(r => r.kind === 'forbidden' && r.enabled).length,
   required: rules.filter(r => r.kind === 'required' && r.enabled).length,
   search: rules.filter(r => r.kind === 'search' && r.enabled).length
  };
 });

 const workspaceVisibleRules = $derived.by(() => {
  const rules = profile?.rules ?? [];
  if (ruleKindFilter === 'all') return rules;
  return rules.filter(r => r.kind === ruleKindFilter);
 });

 const editorFilteredRules = $derived.by(() => {
  const q = editorQuery.trim().toLocaleLowerCase();
  return draftRules
   .map((rule, index) => ({ rule, index }))
   .filter(({ rule }) => {
    if (editorFilter !== 'all' && rule.kind !== editorFilter) return false;
    if (q && !rule.word.toLocaleLowerCase().includes(q)) return false;
    return true;
   });
 });

 const issues = $derived(result?.files.filter(f => f.status === 'issue').length ?? 0);
 const unreadable = $derived(result?.files.filter(f => f.status === 'unreadable').length ?? 0);
 const passed = $derived(result?.files.filter(f => f.status === 'passed').length ?? 0);
 const passRate = $derived(
  result && result.scanned > 0 ? Math.round((passed / result.scanned) * 100) : 0
 );

 const filtered: FileResult[] = $derived.by(() => {
  if (!result) return [];
  const q = resultQuery.trim().toLocaleLowerCase();
  const base = result.files.filter(f => {
   if (tab !== 'all' && f.status !== tab) return false;
   return (
    !q ||
    f.path.toLocaleLowerCase().includes(q) ||
    f.hits.some(
     h => h.word.toLocaleLowerCase().includes(q) || h.context.toLocaleLowerCase().includes(q)
    ) ||
    f.missing.some(w => w.toLocaleLowerCase().includes(q))
   );
  });
  const rank = (s: FileResult['status']) => (s === 'issue' ? 0 : s === 'unreadable' ? 1 : 2);
  return [...base].sort((a, b) => {
   if (sortMode === 'status') {
    const diff = rank(a.status) - rank(b.status);
    if (diff !== 0) return diff;
    const issueDiff =
     b.missing.length + b.hits.length - (a.missing.length + a.hits.length);
    if (issueDiff !== 0) return issueDiff;
   } else if (sortMode === 'hits') {
    const countDiff =
     b.hits.length + b.missing.length - (a.hits.length + a.missing.length);
    if (countDiff !== 0) return countDiff;
   }
   return a.path.localeCompare(b.path, 'zh-CN');
  });
 });

 const chosen: FileResult | undefined = $derived(
  filtered.find(f => f.path === selected) ?? filtered[0]
 );

 const chosenWordCounts = $derived.by(() => {
  if (!chosen) return [];
  const counts = new Map<string, { word: string; kind: Kind; count: number }>();
  for (const h of chosen.hits) {
   const key = `${h.kind}:${h.word}`;
   const prev = counts.get(key);
   if (prev) prev.count += 1;
   else counts.set(key, { word: h.word, kind: h.kind, count: 1 });
  }
  return Array.from(counts.values());
 });

 const filteredChosenHits = $derived.by(() => {
  if (!chosen) return [];
  return chosen.hits.filter(h => {
   if (hitFilter === 'forbidden' && h.kind !== 'forbidden') return false;
   if (hitFilter === 'search' && h.kind !== 'search') return false;
   if (hitFilter === 'required' && h.kind !== 'required') return false;
   if (hitWordFilter && h.word !== hitWordFilter) return false;
   return true;
  });
 });

 const title = (p: string) => p.split(/[\\/]/).pop() || p;
 const parentDir = (p: string) => {
  const idx = Math.max(p.lastIndexOf('/'), p.lastIndexOf('\\'));
  return idx > 0 ? p.slice(0, idx) : '当前位置';
 };
 const fileExtBadge = (p: string) => {
  const base = title(p);
  const dot = base.lastIndexOf('.');
  if (dot <= 0 || dot === base.length - 1) return 'FILE';
  return base.slice(dot + 1).toUpperCase().slice(0, 5);
 };
 const kindName = (k: Kind) => ({ search: '检索', forbidden: '禁止', required: '必需' }[k]);
 const kindFullLabel = (k: Kind) =>
  ({ search: '仅检索', forbidden: '禁止出现', required: '必须包含' }[k]);
 const statusName = (s: string) =>
  ({ passed: '通过', issue: '问题', unreadable: '未检查' }[s] || s);
 const formatTime = (stamp: number) =>
  new Date(stamp * 1000).toLocaleString('zh-CN', {
   month: '2-digit',
   day: '2-digit',
   hour: '2-digit',
   minute: '2-digit'
  });
 const countRules = (p: Profile) => p.rules.filter(r => r.enabled).length;

 function showNotice(msg: string) {
  notice = msg;
 }

 function applyTheme(next: Theme) {
  theme = next;
  document.documentElement.setAttribute('data-theme', next);
  try {
   localStorage.setItem(THEME_STORAGE, next);
  } catch {}
 }

 function toggleTheme() {
  applyTheme(theme === 'dark' ? 'light' : 'dark');
 }

 function saveProfiles() {
  try {
   localStorage.setItem(STORAGE, JSON.stringify(profiles));
   localStorage.setItem(ACTIVE, active);
  } catch (e) {
   showNotice('无法保存方案：' + String(e));
  }
 }

 async function loadHistory() {
  try {
   history = await invoke<HistoryMeta[]>('list_scan_history');
  } catch {
   // Running in browser preview or empty history
  }
 }

 onMount(() => {
  try {
   const savedTheme = localStorage.getItem(THEME_STORAGE);
   if (savedTheme === 'light' || savedTheme === 'dark') {
    applyTheme(savedTheme);
   } else {
    applyTheme('dark');
   }
  } catch {
   applyTheme('dark');
  }

  try {
   const saved = JSON.parse(localStorage.getItem(STORAGE) || 'null');
   profiles =
    Array.isArray(saved) && saved.length > 0
     ? saved.filter(p => p && typeof p.id === 'string' && Array.isArray(p.rules))
     : [template()];
   if (!profiles.length) profiles = [template()];
  } catch {
   profiles = [template()];
  }

  active = localStorage.getItem(ACTIVE) || profiles[0]?.id || '';
  if (!profiles.some(p => p.id === active)) active = profiles[0]?.id || '';
  saveProfiles();

  try {
   const saved = Number(localStorage.getItem('docinspector_split_v1'));
   if (saved >= 22 && saved <= 65) splitPercent = saved;
  } catch {}

  void loadHistory();

  let unlistenProgress: (() => void) | undefined;
  let unlistenDrop: (() => void) | undefined;
  let unlistenDragEnter: (() => void) | undefined;
  let unlistenDragLeave: (() => void) | undefined;

  listen<ScanProgress>('scan-progress', e => {
   if (!progress || e.payload.scanned >= progress.scanned) progress = e.payload;
  })
   .then(off => {
    unlistenProgress = off;
   })
   .catch(() => {});

  listen<{ paths?: string[] }>('tauri://drag-drop', e => {
   draggingFiles = false;
   const dropped = e.payload?.paths;
   if (Array.isArray(dropped) && dropped.length && !busy) {
    sources = [...new Set([...sources, ...dropped])];
    if (page !== 'home') page = 'home';
    showNotice(`已拖入 ${dropped.length} 个项目`);
   }
  })
   .then(off => {
    unlistenDrop = off;
   })
   .catch(() => {});

  listen('tauri://drag-enter', () => {
   if (!busy && !mobile) draggingFiles = true;
  })
   .then(off => {
    unlistenDragEnter = off;
   })
   .catch(() => {});

  listen('tauri://drag-leave', () => {
   draggingFiles = false;
  })
   .then(off => {
    unlistenDragLeave = off;
   })
   .catch(() => {});

  return () => {
   unlistenProgress?.();
   unlistenDrop?.();
   unlistenDragEnter?.();
   unlistenDragLeave?.();
  };
 });

 function chooseProfile(pid: string) {
  if (busy) return;
  active = pid;
  ruleKindFilter = 'all';
  if (page === 'editor') {
   const target = profiles.find(p => p.id === pid);
   if (target) {
    editingId = target.id;
    draftName = target.name;
    draftColor = target.color || '#8b7cf6';
    draftRules = target.rules.map(r => ({ ...r }));
   }
  } else {
   page = 'home';
  }
  saveProfiles();
 }

 function updateOptions(patch: Partial<ScanOptions>) {
  if (!profile) return;
  profiles = profiles.map(p =>
   p.id === active ? { ...p, options: { ...DEFAULT_OPTIONS, ...p.options, ...patch } } : p
  );
  saveProfiles();
 }

 function toggleWorkspaceRule(ruleId: string) {
  if (!profile || busy) return;
  profiles = profiles.map(p =>
   p.id === active
    ? {
       ...p,
       rules: p.rules.map(r => (r.id === ruleId ? { ...r, enabled: !r.enabled } : r))
      }
    : p
  );
  saveProfiles();
 }

 function removeWorkspaceRule(ruleId: string) {
  if (!profile || busy) return;
  profiles = profiles.map(p =>
   p.id === active ? { ...p, rules: p.rules.filter(r => r.id !== ruleId) } : p
  );
  saveProfiles();
 }

 function addQuickWorkspaceRules() {
  if (!profile || busy) return;
  const words = [
   ...new Set(
    quickWord
     .split(/[\n,，;；]+/)
     .map(x => x.trim())
     .filter(Boolean)
   )
  ];
  if (!words.length) return;
  const nextRules = [...profile.rules];
  let added = 0;
  for (const word of words) {
   if (!nextRules.some(r => r.word === word && r.kind === quickKind)) {
    nextRules.push({ id: id(), word, kind: quickKind, enabled: true });
    added += 1;
   }
  }
  profiles = profiles.map(p => (p.id === active ? { ...p, rules: nextRules } : p));
  quickWord = '';
  saveProfiles();
  if (added > 0) showNotice(`已向当前方案添加 ${added} 条规则`);
 }

 function createFromPreset(preset: PresetTemplate) {
  if (busy) return;
  const created = buildPresetProfile(preset);
  const existingNames = new Set(profiles.map(p => p.name));
  if (existingNames.has(created.name)) {
   let idx = 2;
   while (existingNames.has(`${preset.name} ${idx}`)) idx += 1;
   created.name = `${preset.name} ${idx}`;
  }
  profiles = [...profiles, created];
  active = created.id;
  saveProfiles();
  page = 'home';
  showNotice(`已创建预设方案「${created.name}」`);
 }

 function onSplitterDown(e: PointerEvent) {
  if (e.button !== 0 || mobile) return;
  (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  e.preventDefault();
 }

 function onSplitterMove(e: PointerEvent) {
  if (!(e.currentTarget as HTMLElement).hasPointerCapture(e.pointerId)) return;
  const parent = (e.currentTarget as HTMLElement).parentElement;
  if (!parent) return;
  const bounds = parent.getBoundingClientRect();
  splitPercent = Math.max(
   22,
   Math.min(65, Math.round(((e.clientX - bounds.left) / bounds.width) * 100))
  );
 }

 function onSplitterUp(e: PointerEvent) {
  const el = e.currentTarget as HTMLElement;
  if (el.hasPointerCapture(e.pointerId)) el.releasePointerCapture(e.pointerId);
  try {
   localStorage.setItem('docinspector_split_v1', String(splitPercent));
  } catch {}
 }

 async function openDocument(path: string, reveal: boolean) {
  if (mobile) {
   showNotice('手机端暂不支持打开系统文件');
   return;
  }
  try {
   await invoke('open_document', { path, reveal });
  } catch (e) {
   showNotice('打开失败：' + String(e));
  }
 }

 function openContext(e: MouseEvent, path: string) {
  if (mobile) return;
  e.preventDefault();
  contextMenu = {
   path,
   x: Math.min(e.clientX, window.innerWidth - 210),
   y: Math.min(e.clientY, window.innerHeight - 160)
  };
 }

 function createProfile() {
  if (busy) return;
  editingId = '';
  draftName = '';
  draftColor = PALETTE[profiles.length % PALETTE.length];
  draftRules = [];
  newWord = '';
  editorFilter = 'all';
  editorQuery = '';
  page = 'editor';
 }

 function editProfile() {
  if (!profile || busy) return;
  editingId = profile.id;
  draftName = profile.name;
  draftColor = profile.color || '#8b7cf6';
  draftRules = profile.rules.map(r => ({ ...r }));
  newWord = '';
  editorFilter = 'all';
  editorQuery = '';
  page = 'editor';
 }

 function updateRule(i: number, changes: Partial<Rule>) {
  draftRules = draftRules.map((r, j) => (j === i ? { ...r, ...changes } : r));
 }

 function addRules() {
  const words = [
   ...new Set(
    newWord
     .split(/[\n,，;；]+/)
     .map(x => x.trim())
     .filter(Boolean)
   )
  ];
  for (const word of words) {
   if (!draftRules.some(r => r.word === word && r.kind === newKind)) {
    draftRules = [...draftRules, { id: id(), word, kind: newKind, enabled: true }];
   }
  }
  newWord = '';
 }

 function applyPresetToDraft(preset: PresetTemplate) {
  if (!draftName.trim()) draftName = preset.name;
  draftColor = preset.color;
  const next = [...draftRules];
  for (const item of preset.rules) {
   if (!next.some(r => r.word === item.word && r.kind === item.kind)) {
    next.push({ id: id(), word: item.word, kind: item.kind, enabled: true });
   }
  }
  draftRules = next;
  showNotice(`已导入「${preset.name}」规则`);
 }

 function setAllDraftRulesEnabled(enabled: boolean) {
  draftRules = draftRules.map(r =>
   editorFilter === 'all' || r.kind === editorFilter ? { ...r, enabled } : r
  );
 }

 function saveProfile() {
  const name = draftName.trim();
  if (!name) {
   showNotice('请输入方案名称');
   return;
  }
  const unique = new Set<string>();
  const rules: Rule[] = [];
  for (const rule of draftRules) {
   const word = rule.word.trim();
   if (!word) continue;
   const key = rule.kind + '\0' + word;
   if (!unique.has(key)) {
    unique.add(key);
    rules.push({ ...rule, word });
   }
  }
  if (editingId) {
   profiles = profiles.map(p =>
    p.id === editingId ? { ...p, name, color: draftColor, rules } : p
   );
   active = editingId;
  } else {
   const newProfile: Profile = {
    id: id(),
    name,
    created: Date.now(),
    icon: '◆',
    color: draftColor || '#8b7cf6',
    options: { ...DEFAULT_OPTIONS },
    rules
   };
   profiles = [...profiles, newProfile];
   active = newProfile.id;
  }
  saveProfiles();
  showNotice('方案已保存');
  page = 'home';
 }

 function duplicateProfile() {
  if (!profile || busy) return;
  const cloned: Profile = {
   ...profile,
   id: id(),
   name: profile.name + ' 副本',
   created: Date.now(),
   rules: profile.rules.map(r => ({ ...r, id: id() }))
  };
  profiles = [...profiles, cloned];
  active = cloned.id;
  saveProfiles();
  showNotice(`已复制方案「${cloned.name}」`);
 }

 function removeProfile() {
  if (!profile || busy) return;
  const name = profile.name;
  if (
   !confirm(
    '删除方案「' + name + '」？历史记录将保留在本机，可在“全部记录”中查看。'
   )
  )
   return;
  profiles = profiles.filter(p => p.id !== active);
  active = profiles[0]?.id || '';
  saveProfiles();
 }

 async function pick(directory: boolean) {
  if (busy) return;
  if (mobile && directory) {
   showNotice('Android 暂不支持文件夹递归，请选择文件');
   return;
  }
  try {
   const v = await open({
    directory,
    multiple: !directory,
    ...(mobile ? { pickerMode: 'document' as const } : {})
   });
   if (v) {
    const a = Array.isArray(v) ? v : [v];
    sources = [...new Set([...sources, ...a])];
   }
  } catch (e) {
   showNotice('无法选择文件：' + String(e));
  }
 }

 function simpleRules(): Rule[] {
  return [
   ...new Set(
    keyword
     .split(/[\n,，;；]+/)
     .map(x => x.trim())
     .filter(Boolean)
   )
  ].map(word => ({ id: id(), word, kind: 'search' as Kind, enabled: true }));
 }

 async function startScan() {
  if (busy) return;
  if (!profile) {
   showNotice('请先选择或创建一个方案');
   return;
  }
  if (!sources.length) {
   showNotice('请先选择要检查的文件或文件夹');
   return;
  }
  const rules = [...profile.rules.filter(r => r.enabled), ...simpleRules()];
  const scanOptions: ScanOptions = { ...DEFAULT_OPTIONS, ...profile.options };
  if (!rules.some(r => r.word.trim())) {
   showNotice('请先启用至少一条规则或输入临时检索词');
   return;
  }
  const scanProfile = { id: profile.id, name: profile.name };
  const scanSources = [...sources];
  busy = true;
  cancelling = false;
  cancelledMobile = false;
  result = null;
  openedHistoryId = '';
  selected = '';
  progress = null;
  tab = 'all';
  hitFilter = 'all';
  hitWordFilter = '';
  resultQuery = '';
  page = 'results';
  notice = '';
  const started = Date.now();
  try {
   let completed: ScanReport;
   if (mobile) {
    const files: FileResult[] = [];
    for (const uri of scanSources) {
     if (cancelledMobile) break;
     let name = '选中文件';
     try {
      name = decodeURIComponent(uri.split('/').pop() || '选中文件');
      const bytes = await readFile(uri);
      if (bytes.length > 24 * 1024 * 1024) throw Error('文件超过 24 MB');
      const scanned = await invoke<FileResult>('scan_bytes', {
       name,
       bytes: Array.from(bytes),
       rules,
       options: scanOptions
      });
      files.push({ ...scanned, path: name });
     } catch (e) {
      files.push({
       path: name,
       status: 'unreadable',
       hits: [],
       missing: [],
       error: String(e)
      });
     }
     progress = { scanned: files.length, total: scanSources.length, current: name };
    }
    completed = {
     files,
     total: scanSources.length,
     scanned: files.length,
     cancelled: cancelledMobile,
     durationMs: Date.now() - started
    };
   } else {
    completed = await invoke<ScanReport>('scan_paths', {
     paths: scanSources,
     rules,
     options: scanOptions
    });
   }
   result = completed;
   selected = completed.files[0]?.path || '';
   if (completed.scanned > 0) {
    try {
     const meta = await invoke<HistoryMeta>('save_scan_history', {
      profileId: scanProfile.id,
      profileName: scanProfile.name,
      sources: scanSources,
      report: completed
     });
     history = [meta, ...history].slice(0, 30);
     openedHistoryId = meta.id;
    } catch (e) {
     showNotice('检查完成，但历史保存失败：' + String(e));
    }
   }
  } catch (e) {
   showNotice('扫描失败：' + String(e));
  } finally {
   busy = false;
   cancelling = false;
  }
 }

 async function cancelScan() {
  if (!busy) return;
  cancelling = true;
  if (mobile) {
   cancelledMobile = true;
   return;
  }
  try {
   await invoke('cancel_scan');
  } catch (e) {
   showNotice(String(e));
  }
 }

 async function openHistory(entry: HistoryMeta) {
  if (busy) return;
  try {
   const old = await invoke<ScanReport>('load_scan_history', { id: entry.id });
   sources = [...entry.sources];
   result = old;
   openedHistoryId = entry.id;
   selected = old.files[0]?.path || '';
   tab = 'all';
   hitFilter = 'all';
   hitWordFilter = '';
   resultQuery = '';
   progress = null;
   page = 'results';
  } catch (e) {
   showNotice('无法打开记录：' + String(e));
  }
 }

 async function deleteHistory(entry: HistoryMeta) {
  if (busy) return;
  if (!confirm('删除这条扫描历史？')) return;
  try {
   await invoke('delete_scan_history', { id: entry.id });
   history = history.filter(h => h.id !== entry.id);
   if (openedHistoryId === entry.id) openedHistoryId = '';
  } catch (e) {
   showNotice('删除失败：' + String(e));
  }
 }

 async function exportCsv() {
  if (!result) return;
  try {
   if (mobile) {
    const csv = await invoke<string>('export_csv_text', { report: result });
    try {
     const uri = await save({
      defaultPath: 'docinspector-results.csv',
      filters: [{ name: 'CSV', extensions: ['csv'] }]
     });
     if (uri) {
      await writeTextFile(uri, csv);
      showNotice('已保存 CSV');
     }
    } catch (e) {
     try {
      await navigator.clipboard.writeText(csv);
      showNotice('CSV 已复制到剪贴板');
     } catch {
      showNotice('保存失败：' + String(e));
     }
    }
   } else {
    const path = await save({
     defaultPath: 'docinspector-results.csv',
     filters: [{ name: 'CSV', extensions: ['csv'] }]
    });
    if (path) {
     await invoke('export_csv', { path, report: result });
     showNotice('CSV 报告已导出');
    }
   }
  } catch (e) {
   showNotice('导出失败：' + String(e));
  }
 }

 async function copySummaryReport() {
  if (!result) return;
  const lines: string[] = [
   `# DocInspector 检查报告`,
   `- 检查方案：${profile?.name || '默认方案'}`,
   `- 扫描统计：已检查 ${result.scanned} 个文件 · 通过 ${passed} · 问题 ${issues} · 未检查 ${unreadable}`,
   ''
  ];
  for (const f of result.files.filter(x => x.status !== 'passed')) {
   lines.push(`## [${statusName(f.status)}] ${title(f.path)}`);
   lines.push(`路径：${f.path}`);
   if (f.error) lines.push(`- 异常原因：${f.error}`);
   for (const m of f.missing) lines.push(`- [缺失必含词] ${m}`);
   for (const h of f.hits.slice(0, 20)) {
    lines.push(`- [${kindName(h.kind)}] ${h.word} (${h.location})：${h.context.replace(/\s+/g, ' ').trim()}`);
   }
   lines.push('');
  }
  try {
   await navigator.clipboard.writeText(lines.join('\n'));
   showNotice('检查摘要已复制到剪贴板');
  } catch (e) {
   showNotice('复制失败：' + String(e));
  }
 }

 async function copyProfileJson() {
  try {
   await navigator.clipboard.writeText(JSON.stringify({ version: 1, profiles }, null, 2));
   showNotice('全部方案 JSON 已复制到剪贴板');
  } catch (e) {
   showNotice('无法复制配置：' + String(e));
  }
 }

 function importProfiles() {
  if (!importText.trim()) {
   showNotice('请先粘贴方案 JSON');
   return;
  }
  try {
   const data = JSON.parse(importText);
   if (!Array.isArray(data.profiles)) throw Error('方案格式不正确');
   const imported: Profile[] = data.profiles
    .filter(
     (p: any) =>
      p &&
      typeof p.name === 'string' &&
      Array.isArray(p.rules) &&
      p.rules.every(
       (r: any) =>
        r &&
        typeof r.word === 'string' &&
        ['search', 'forbidden', 'required'].includes(r.kind)
      )
    )
    .map((p: Profile) => ({
     ...p,
     id: id(),
     color: p.color || '#8b7cf6',
     rules: p.rules.map(r => ({ ...r, id: id(), enabled: r.enabled !== false }))
    }));
   if (!imported.length) throw Error('没有可导入的方案');
   profiles = [...profiles, ...imported];
   active = imported[0].id;
   importText = '';
   saveProfiles();
   showNotice('已导入 ' + imported.length + ' 个方案');
   page = 'home';
  } catch (e) {
   showNotice('导入失败：' + String(e));
  }
 }

 function copyPath(path: string, label = '路径已复制') {
  void navigator.clipboard
   .writeText(path)
   .then(() => showNotice(label), e => showNotice('复制失败：' + String(e)));
 }

 function jumpIssueFile(delta: number) {
  const issueFiles = filtered.filter(f => f.status === 'issue' || f.status === 'unreadable');
  if (!issueFiles.length) return;
  const curIdx = issueFiles.findIndex(f => f.path === chosen?.path);
  const nextIdx =
   curIdx === -1
    ? 0
    : (curIdx + delta + issueFiles.length) % issueFiles.length;
  selected = issueFiles[nextIdx].path;
  hitFilter = 'all';
  hitWordFilter = '';
 }

 function highlightChunks(
  context: string,
  target: string
 ): Array<{ text: string; mark: boolean }> {
  if (!target) return [{ text: context, mark: false }];
  const parts: Array<{ text: string; mark: boolean }> = [];
  const lowerCtx = context.toLocaleLowerCase();
  const lowerTarget = target.toLocaleLowerCase();
  const step = target.length;
  if (step === 0) return [{ text: context, mark: false }];
  let cursor = 0;
  while (cursor < context.length) {
   const found = lowerCtx.indexOf(lowerTarget, cursor);
   if (found === -1) {
    parts.push({ text: context.slice(cursor), mark: false });
    break;
   }
   if (found > cursor) {
    parts.push({ text: context.slice(cursor, found), mark: false });
   }
   parts.push({ text: context.slice(found, found + step), mark: true });
   cursor = found + step;
  }
  return parts.length ? parts : [{ text: context, mark: false }];
 }
</script>

<svelte:head><title>DocInspector</title></svelte:head>

<div class="shell" data-theme={theme}>
 {#if draggingFiles}
  <div class="drop-overlay" role="presentation">
   <div class="drop-box">
    <FolderOpen size={42} />
    <h3>松开鼠标以添加文件或文件夹</h3>
    <p>支持直接拖入多个文档、代码文件或整个项目目录</p>
   </div>
  </div>
 {/if}

 <aside class:collapsed={!sidebar}>
  <div class="brand">
   <div class="brand-icon" title="DocInspector 本地文档检查器">
    <ShieldCheck size={20} />
   </div>
   {#if sidebar}
    <div class="brand-text">
     <span class="brand-title">DocInspector</span>
     <span class="brand-sub">本地文档合规与检索</span>
    </div>
   {/if}
   <button
    class="icon push"
    title={sidebar ? '收起侧栏' : '展开侧栏'}
    aria-label="切换侧栏"
    onclick={() => (sidebar = !sidebar)}
   >
    {#if sidebar}<PanelLeftClose size={17} />{:else}<PanelLeftOpen size={17} />{/if}
   </button>
  </div>

  <div class="side-nav">
   <button
    class:current={page === 'home'}
    title="检查工作台"
    onclick={() => (page = 'home')}
   >
    <Layers size={17} />
    {#if sidebar}<span>检查工作台</span>{/if}
   </button>
   <button
    class:current={page === 'results'}
    title="扫描结果"
    onclick={() => (page = 'results')}
    disabled={!result && !busy}
   >
    <FileText size={17} />
    {#if sidebar}
     <span>扫描结果</span>
     {#if busy}
      <span class="nav-pill running">扫描中</span>
     {:else if result}
      <span class="nav-pill" class:has-issue={issues > 0}>
       {issues > 0 ? `${issues} 问题` : `${result.scanned} 文件`}
      </span>
     {/if}
    {/if}
   </button>
   <button
    class:current={page === 'editor'}
    title="编辑当前方案规则"
    onclick={editProfile}
    disabled={!profile || busy}
   >
    <SlidersHorizontal size={17} />
    {#if sidebar}<span>规则配置</span>{/if}
   </button>
  </div>

  {#if sidebar}
   <div class="side-caption">
    <span>检查方案 ({profiles.length})</span>
    <button class="icon side-add-btn" title="新建方案" onclick={createProfile} disabled={busy}>
     <Plus size={16} />
    </button>
   </div>
  {/if}

  <div class="scheme-list">
   {#each profiles as p (p.id)}
    <button
     class:active={active === p.id}
     class="scheme-item"
     title={`${p.name}（${countRules(p)} 条启用规则）`}
     onclick={() => chooseProfile(p.id)}
     disabled={busy}
    >
     <span class="scheme-dot" style:background={p.color || '#8b7cf6'}></span>
     {#if sidebar}
      <span class="scheme-name">{p.name}</span>
      <span class="scheme-badge">{countRules(p)}</span>
     {/if}
    </button>
   {/each}
   {#if !profiles.length}
    <button class="scheme-new" onclick={createProfile}>
     <Plus size={16} />{#if sidebar}<span>新建方案</span>{/if}
    </button>
   {/if}
  </div>

  <div class="side-bottom">
   <button
    class="theme-btn"
    title={theme === 'dark' ? '切换为浅色外观' : '切换为深色外观'}
    onclick={toggleTheme}
   >
    {#if theme === 'dark'}<Sun size={17} />{:else}<Moon size={17} />{/if}
    {#if sidebar}<span>{theme === 'dark' ? '浅色外观' : '深色外观'}</span>{/if}
   </button>
   <button
    class:current={page === 'settings'}
    title="设置与方案迁移"
    onclick={() => (page = 'settings')}
   >
    <Settings2 size={17} />
    {#if sidebar}<span>设置与迁移</span>{/if}
   </button>
  </div>
 </aside>

 <main>
  {#if page === 'home'}
   <header class="workspace-header">
    <div class="heading-group">
     {#if mobile}
      <select
       class="mobile-scheme"
       aria-label="选择检查方案"
       value={active}
       onchange={e => chooseProfile(e.currentTarget.value)}
      >
       {#each profiles as p}<option value={p.id}>{p.name}</option>{/each}
      </select>
     {/if}
     {#if profile}
      <span class="header-color-dot" style:background={profile.color || '#8b7cf6'}></span>
     {/if}
     <div class="heading-titles">
      <h1>{profile?.name || '新建检查方案'}</h1>
      {#if profile}
       <div class="heading-pills">
        <span class="heading-meta">{ruleBreakdown.enabled} / {ruleBreakdown.total} 条规则启用</span>
        {#if ruleBreakdown.forbidden > 0}
         <span class="meta-tag danger">⛔ 禁止 {ruleBreakdown.forbidden}</span>
        {/if}
        {#if ruleBreakdown.required > 0}
         <span class="meta-tag success">✅ 必需 {ruleBreakdown.required}</span>
        {/if}
        {#if ruleBreakdown.search > 0}
         <span class="meta-tag info">🔎 检索 {ruleBreakdown.search}</span>
        {/if}
       </div>
      {/if}
     </div>
    </div>
    <div class="header-actions">
     {#if result}
      <button class="outline accent-outline" onclick={() => (page = 'results')}>
       <FileText size={15} /> 查看上次结果 ({result.scanned})
      </button>
     {/if}
     {#if profile}
      <button class="outline" title="编辑规则与名称" onclick={editProfile}>
       <Pencil size={15} /> <span>编辑规则</span>
      </button>
      <button class="icon" aria-label="复制方案" title="复制当前方案" onclick={duplicateProfile}>
       <Copy size={17} />
      </button>
      <button class="icon danger-hover" aria-label="删除方案" title="删除当前方案" onclick={removeProfile}>
       <Trash2 size={17} />
      </button>
     {/if}
    </div>
   </header>

   <div class="content workspace">
    {#if profile}
     <!-- Card 1: Current Scheme Rules Overview & Quick Add -->
     <section class="panel-card rules-overview-card">
      <div class="section-head">
       <div class="section-title-group">
        <h2>当前方案规则</h2>
        <span class="section-sub">点击规则标签可临时启用或停用，也可直接快速补充关键词</span>
       </div>
       <div class="rule-filter-pills">
        <button
         class:active={ruleKindFilter === 'all'}
         onclick={() => (ruleKindFilter = 'all')}
        >
         全部 ({profile.rules.length})
        </button>
        <button
         class:active={ruleKindFilter === 'forbidden'}
         onclick={() => (ruleKindFilter = 'forbidden')}
        >
         ⛔ 禁止 ({profile.rules.filter(r => r.kind === 'forbidden').length})
        </button>
        <button
         class:active={ruleKindFilter === 'required'}
         onclick={() => (ruleKindFilter = 'required')}
        >
         ✅ 必需 ({profile.rules.filter(r => r.kind === 'required').length})
        </button>
        <button
         class:active={ruleKindFilter === 'search'}
         onclick={() => (ruleKindFilter = 'search')}
        >
         🔎 检索 ({profile.rules.filter(r => r.kind === 'search').length})
        </button>
       </div>
      </div>

      {#if workspaceVisibleRules.length}
       <div class="rule-chip-cloud">
        {#each workspaceVisibleRules as r (r.id)}
         <div
          class="rule-chip"
          class:disabled={!r.enabled}
          class:forbidden={r.kind === 'forbidden'}
          class:required={r.kind === 'required'}
          class:search={r.kind === 'search'}
         >
          <button
           class="rule-chip-toggle"
           title={r.enabled ? '点击停用此规则' : '点击启用此规则'}
           onclick={() => toggleWorkspaceRule(r.id)}
          >
           <span class="chip-kind">{kindName(r.kind)}</span>
           <span class="chip-word">{r.word}</span>
          </button>
          <button
           class="rule-chip-remove"
           title="移除此关键词"
           aria-label="移除关键词"
           onclick={() => removeWorkspaceRule(r.id)}
          >
           <X size={12} />
          </button>
         </div>
        {/each}
       </div>
      {:else}
       <div class="inline-empty">
        <span>当前分类下还没有关键词规则，可在下方直接添加或进入完整规则编辑器。</span>
       </div>
      {/if}

      <div class="quick-rule-bar">
       <select bind:value={quickKind} aria-label="快速添加规则类型">
        <option value="forbidden">⛔ 禁止出现</option>
        <option value="required">✅ 必须包含</option>
        <option value="search">🔎 仅检索</option>
       </select>
       <input
        type="text"
        bind:value={quickWord}
        placeholder="快速添加关键词到当前方案（支持逗号分隔批量添加，按回车确认）"
        onkeydown={e => {
         if (e.key === 'Enter') {
          e.preventDefault();
          addQuickWorkspaceRules();
         }
        }}
       />
       <button class="outline" onclick={addQuickWorkspaceRules} disabled={!quickWord.trim()}>
        <Plus size={15} /> 添加到方案
       </button>
      </div>
     </section>

     <!-- Card 2: File & Folder Sources + Scan Controls -->
     <section class="panel-card source-section">
      <div class="section-head">
       <div class="section-title-group">
        <h2>选择待检查文件或目录</h2>
        <span class="section-sub">支持拖拽文件/文件夹至窗口，本地只读解析，绝不修改原文件</span>
       </div>
       {#if sources.length}
        <button class="text-btn danger-text" onclick={() => (sources = [])}>
         <Trash2 size={14} /> 清空已选 ({sources.length})
        </button>
       {/if}
      </div>

      <div class="sources">
       {#if !mobile}
        <button class="source-card" onclick={() => pick(true)}>
         <span class="source-icon folder"><FolderOpen size={24} /></span>
         <div class="source-copy">
          <strong>选择文件夹（递归扫描）</strong>
          <small>自动遍历子目录中的 PDF、Word、PPT、表格及源码</small>
         </div>
         <ChevronRight size={18} class="source-arrow" />
        </button>
       {/if}
       <button class="source-card" onclick={() => pick(false)}>
        <span class="source-icon files"><Files size={24} /></span>
        <div class="source-copy">
         <strong>选择文件（支持多选）</strong>
         <small>精准检查指定文档：PDF / DOCX / PPTX / XLSX / TXT / 代码</small>
        </div>
        <ChevronRight size={18} class="source-arrow" />
       </button>
      </div>

      {#if sources.length}
       <div class="path-list">
        {#each sources as source (source)}
         <div class="path-chip">
          <span class="path-ext">{fileExtBadge(source)}</span>
          <div class="path-text" title={source}>
           <strong>{title(source)}</strong>
           <small>{source}</small>
          </div>
          <button
           title="移除此来源"
           aria-label="移除来源"
           onclick={() => (sources = sources.filter(x => x !== source))}
          >
           <X size={14} />
          </button>
         </div>
        {/each}
       </div>
      {/if}

      <div class="scan-controls">
       <div class="search-input">
        <Search size={17} />
        <input
         bind:value={keyword}
         placeholder="临时附加检索词（可选，仅本次扫描生效，多个词以逗号分隔）"
         onkeydown={e => {
          if (e.key === 'Enter' && sources.length) {
           e.preventDefault();
           void startScan();
          }
         }}
        />
        {#if keyword}
         <button class="icon" title="清空临时检索词" onclick={() => (keyword = '')}>
          <X size={15} />
         </button>
        {/if}
       </div>
       <button class="primary scan-launch-btn" onclick={startScan}>
        <Play size={16} fill="currentColor" />
        <span>开始扫描{sources.length ? ` (${sources.length})` : ''}</span>
       </button>
      </div>

      <div class="scan-options-bar">
       <div class="option-toggles">
        <label class="option-pill" class:checked={options.caseSensitive}>
         <input
          type="checkbox"
          checked={options.caseSensitive}
          onchange={e => updateOptions({ caseSensitive: e.currentTarget.checked })}
         />
         <span>区分大小写</span>
        </label>
        <label class="option-pill" class:checked={options.wholeWord}>
         <input
          type="checkbox"
          checked={options.wholeWord}
          onchange={e => updateOptions({ wholeWord: e.currentTarget.checked })}
         />
         <span>完整英文单词</span>
        </label>
        {#if !mobile}
         <label class="option-pill" class:checked={options.skipCommonDirs}>
          <input
           type="checkbox"
           checked={options.skipCommonDirs}
           onchange={e => updateOptions({ skipCommonDirs: e.currentTarget.checked })}
          />
          <span>跳过代码缓存目录 (.git / node_modules / target)</span>
         </label>
        {/if}
       </div>
       {#if !mobile}
        <label class="exclude-inline">
         <span>自定义排除目录</span>
         <input
          type="text"
          value={options.excludeDirs.join(', ')}
          placeholder="输入要忽略的文件夹名，用逗号分隔"
          oninput={e =>
           updateOptions({
            excludeDirs: [
             ...new Set(
              e.currentTarget.value
               .split(/[,，;；\n]+/)
               .map(v => v.trim())
               .filter(Boolean)
             )
            ]
           })}
         />
        </label>
       {/if}
      </div>
     </section>

     <!-- Card 3: Preset Scheme Library Quick Bar -->
     <section class="panel-card preset-section">
      <div class="section-head">
       <div class="section-title-group">
        <h2><Sparkles size={16} /> 常用方案模板库</h2>
        <span class="section-sub">一键基于典型场景创建独立检查方案</span>
       </div>
      </div>
      <div class="preset-grid">
       {#each PRESET_TEMPLATES as preset (preset.key)}
        <div class="preset-card">
         <div class="preset-top">
          <span class="preset-dot" style:background={preset.color}></span>
          <strong>{preset.name}</strong>
          <span class="preset-count">{preset.rules.length} 词</span>
         </div>
         <p>{preset.desc}</p>
         <div class="preset-Sample">
          {#each preset.rules.slice(0, 4) as item}
           <span class="mini-tag {item.kind}">{item.word}</span>
          {/each}
          {#if preset.rules.length > 4}
           <span class="mini-tag more">+{preset.rules.length - 4}</span>
          {/if}
         </div>
         <button class="outline preset-use-btn" onclick={() => createFromPreset(preset)}>
          <Plus size={14} /> 使用此模板新建
         </button>
        </div>
       {/each}
      </div>
     </section>

     <!-- Card 4: Scan History -->
     <section class="panel-card history-section">
      <div class="section-head">
       <div class="section-title-group">
        <h2>扫描历史记录</h2>
        <span class="section-count">{visibleHistory.length}</span>
       </div>
       <div class="rule-filter-pills">
        <button
         class:active={historyScope === 'current'}
         onclick={() => (historyScope = 'current')}
        >
         当前方案 ({history.filter(h => h.profileId === active).length})
        </button>
        <button
         class:active={historyScope === 'all'}
         onclick={() => (historyScope = 'all')}
        >
         全部方案 ({history.length})
        </button>
       </div>
      </div>

      {#if visibleHistory.length}
       <div class="history-list">
        {#each visibleHistory as entry (entry.id)}
         <div class="history-line" class:opened={openedHistoryId === entry.id}>
          <button class="history-record" onclick={() => openHistory(entry)}>
           <span
            class="history-symbol"
            class:has-issue={entry.issues > 0}
            class:all-pass={entry.issues === 0 && entry.unreadable === 0}
           >
            {#if entry.issues > 0}
             <FileWarning size={18} />
            {:else if entry.unreadable > 0}
             <Clock3 size={18} />
            {:else}
             <CircleCheck size={18} />
            {/if}
           </span>
           <span class="history-main">
            <div class="history-title-row">
             <strong>
              {entry.sources.length === 1
               ? title(entry.sources[0])
               : entry.sources.length + ' 个扫描来源'}
             </strong>
             {#if historyScope === 'all'}
              <span class="history-scheme-tag">{entry.profileName}</span>
             {/if}
            </div>
            <span class="history-time">
             {formatTime(entry.timestamp)} · 已检查 {entry.scanned} 个文件{entry.durationMs != null
              ? ' · 耗时 ' + (entry.durationMs / 1000).toFixed(2) + 's'
              : ''}{entry.cancelled ? ' · 已中途取消' : ''}
            </span>
           </span>
           <span class="history-stats">
            {#if entry.issues}
             <span class="history-badge issue">{entry.issues} 个问题文件</span>
            {/if}
            {#if entry.unreadable}
             <span class="history-badge unreadable">{entry.unreadable} 未检查</span>
            {/if}
            {#if !entry.issues && !entry.unreadable}
             <span class="history-badge pass">全部通过</span>
            {/if}
           </span>
           <ChevronRight size={17} class="history-chevron" />
          </button>
          <button
           class="icon history-delete"
           title="删除此条记录"
           aria-label="删除历史"
           onclick={() => deleteHistory(entry)}
          >
           <Trash2 size={16} />
          </button>
         </div>
        {/each}
       </div>
      {:else}
       <div class="history-empty">
        <Clock3 size={24} />
        <span>暂无扫描记录，选择文件并开始扫描后将自动保存在此处</span>
       </div>
      {/if}
     </section>
    {:else}
     <div class="welcome-empty">
      <ShieldCheck size={42} />
      <h2>创建您的第一个文档检查方案</h2>
      <p>自定义禁止词、必含词与检索词，快速核查文档合规性</p>
      <div class="welcome-actions">
       <button class="primary" onclick={createProfile}>
        <Plus size={16} /> 新建空白方案
       </button>
       <button class="outline" onclick={() => createFromPreset(PRESET_TEMPLATES[0])}>
        <Sparkles size={16} /> 使用「论文终稿检查」模板
       </button>
      </div>
     </div>
    {/if}
   </div>

  {:else if page === 'editor'}
   <header class="workspace-header">
    <div class="header-with-back">
     <button class="icon" title="返回工作台" onclick={() => (page = 'home')}>
      <ArrowLeft size={20} />
     </button>
     <div>
      <h1>{editingId ? '编辑检查方案' : '新建检查方案'}</h1>
      <p>配置方案名称、主题标识色以及禁止出现 / 必须包含 / 仅检索三类关键词</p>
     </div>
    </div>
    <div class="header-actions">
     <button class="outline" onclick={() => (page = 'home')}>取消</button>
     <button class="primary" onclick={saveProfile}><Check size={17} /> 保存方案</button>
    </div>
   </header>

   <div class="content editor">
    <section class="panel-card editor-meta-card">
     <div class="editor-meta-grid">
      <label class="field">
       <span>方案名称</span>
       <input bind:value={draftName} placeholder="例如：论文终稿检查 / 敏感词排查" />
      </label>
      <div class="field">
       <span>方案标识色</span>
       <div class="color-picker">
        {#each PALETTE as c}
         <button
          type="button"
          class="color-swatch"
          class:selected={draftColor === c}
          style:background={c}
          title={`选择颜色 ${c}`}
          aria-label={`选择颜色 ${c}`}
          onclick={() => (draftColor = c)}
         ></button>
        {/each}
       </div>
      </div>
     </div>

     <div class="preset-import-bar">
      <span class="preset-import-label"><Sparkles size={14} /> 快速导入预设词库：</span>
      {#each PRESET_TEMPLATES as preset}
       <button
        type="button"
        class="mini-preset-btn"
        onclick={() => applyPresetToDraft(preset)}
       >
        + {preset.name}
       </button>
      {/each}
     </div>
    </section>

    <section class="panel-card rule-composer">
     <div class="section-head">
      <div class="section-title-group">
       <h2>批量添加关键词规则</h2>
       <span class="section-sub">支持一次粘贴多行或用逗号、分号分隔多个关键词（按 Ctrl+Enter 快速添加）</span>
      </div>
     </div>

     <div class="rule-add">
      <select bind:value={newKind} aria-label="新增规则类型">
       <option value="forbidden">⛔ 禁止出现（命中判为问题）</option>
       <option value="required">✅ 必须包含（缺失判为问题）</option>
       <option value="search">🔎 仅检索（仅定位不判违规）</option>
      </select>
      <textarea
       bind:value={newWord}
       rows="2"
       placeholder="输入关键词，支持换行、逗号或分号批量添加"
       onkeydown={e => {
        if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') {
         e.preventDefault();
         addRules();
        }
       }}
      ></textarea>
      <button class="primary" onclick={addRules} disabled={!newWord.trim()}>
       <Plus size={16} /> 批量添加
      </button>
     </div>

     <div class="editor-rule-toolbar">
      <div class="rule-filter-pills">
       <button
        class:active={editorFilter === 'all'}
        onclick={() => (editorFilter = 'all')}
       >
        全部 ({draftRules.length})
       </button>
       <button
        class:active={editorFilter === 'forbidden'}
        onclick={() => (editorFilter = 'forbidden')}
       >
        ⛔ 禁止 ({draftRules.filter(r => r.kind === 'forbidden').length})
       </button>
       <button
        class:active={editorFilter === 'required'}
        onclick={() => (editorFilter = 'required')}
       >
        ✅ 必须 ({draftRules.filter(r => r.kind === 'required').length})
       </button>
       <button
        class:active={editorFilter === 'search'}
        onclick={() => (editorFilter = 'search')}
       >
        🔎 检索 ({draftRules.filter(r => r.kind === 'search').length})
       </button>
      </div>

      <div class="editor-batch-actions">
       <div class="mini-search">
        <Search size={14} />
        <input bind:value={editorQuery} placeholder="筛选规则词…" />
       </div>
       {#if draftRules.length}
        <button class="text-btn" onclick={() => setAllDraftRulesEnabled(true)}>全部启用</button>
        <button class="text-btn" onclick={() => setAllDraftRulesEnabled(false)}>全部停用</button>
        <button
         class="text-btn danger-text"
         onclick={() => {
          if (confirm('确定清空当前筛选下的规则吗？')) {
           draftRules =
            editorFilter === 'all'
             ? []
             : draftRules.filter(r => r.kind !== editorFilter);
          }
         }}
        >
         清空
        </button>
       {/if}
      </div>
     </div>

     <div class="rule-list">
      {#each editorFilteredRules as { rule, index } (rule.id)}
       <div class="editable-rule" class:disabled={!rule.enabled}>
        <input
         type="checkbox"
         checked={rule.enabled}
         aria-label="启用规则"
         onchange={e => updateRule(index, { enabled: e.currentTarget.checked })}
        />
        <select
         class="kind-select {rule.kind}"
         value={rule.kind}
         aria-label="规则类型"
         onchange={e => updateRule(index, { kind: e.currentTarget.value as Kind })}
        >
         <option value="forbidden">⛔ 禁止出现</option>
         <option value="required">✅ 必须包含</option>
         <option value="search">🔎 仅检索</option>
        </select>
        <input
         class="rule-word"
         value={rule.word}
         aria-label="关键词"
         oninput={e => updateRule(index, { word: e.currentTarget.value })}
        />
        <button
         class="icon danger-hover"
         title="删除关键词"
         aria-label="删除关键词"
         onclick={() => (draftRules = draftRules.filter(r => r.id !== rule.id))}
        >
         <X size={16} />
        </button>
       </div>
      {:else}
       <div class="empty-small">暂无匹配的关键词规则，请在上方输入或点击预设模板导入</div>
      {/each}
     </div>
    </section>
   </div>

  {:else if page === 'results'}
   <header class="workspace-header results-header">
    <div class="header-with-back">
     <button class="icon" title="返回工作台" onclick={() => (page = 'home')}>
      <ArrowLeft size={20} />
     </button>
     <div>
      <div class="results-title-row">
       <h1>扫描结果</h1>
       {#if profile}
        <span class="scheme-pill">
         <span class="scheme-dot" style:background={profile.color || '#8b7cf6'}></span>
         {profile.name}
        </span>
       {/if}
       {#if result?.durationMs != null}
        <span class="duration-pill">耗时 {(result.durationMs / 1000).toFixed(2)}s</span>
       {/if}
      </div>
      {#if openedHistoryId}
       <p>正在查看历史记录存档</p>
      {:else if busy}
       <p>正在并发提取与匹配文档内容…</p>
      {:else if result?.cancelled}
       <p>扫描已手动停止（展示已完成部分）</p>
      {:else if result}
       <p>共扫描 {result.scanned} 个文件 · 通过率 {passRate}%</p>
      {/if}
     </div>
    </div>
    <div class="header-actions">
     {#if busy}
      <button class="outline danger-outline" disabled={cancelling} onclick={cancelScan}>
       <X size={16} />{cancelling ? '正在取消…' : '停止扫描'}
      </button>
     {:else if result}
      {#if issues + unreadable > 0}
       <button class="outline" title="定位到上一个问题文件" onclick={() => jumpIssueFile(-1)}>
        ↑ 上一问题
       </button>
       <button class="outline" title="定位到下一个问题文件" onclick={() => jumpIssueFile(1)}>
        ↓ 下一问题
       </button>
      {/if}
      <button class="outline" title="复制 Markdown 格式摘要报告" onclick={copySummaryReport}>
       <Copy size={15} /> 复制摘要
      </button>
      <button class="outline" onclick={exportCsv}>
       <Download size={15} /> 导出 CSV
      </button>
      <button class="primary" onclick={startScan}>
       <RotateCcw size={15} /> 重新扫描
      </button>
     {/if}
    </div>
   </header>

   {#if busy}
    <div class="progress">
     <div class="progress-label">
      <span>正在扫描：{progress?.scanned || 0} / {progress?.total || '…'} 个文件</span>
      <span class="truncate">{progress?.current || '正在准备文件队列…'}</span>
     </div>
     <div class="progress-bar">
      <div
       style:width={progress?.total
        ? Math.min(100, 100 * (progress.scanned / progress.total)) + '%'
        : '12%'}
      ></div>
     </div>
    </div>
   {/if}

   {#if result}
    <div class="result-area">
     <!-- Interactive KPI Filter Cards -->
     <div class="stats">
      <button
       class="stat-card"
       class:selected={tab === 'all'}
       onclick={() => (tab = 'all')}
      >
       <div class="stat-info">
        <small>已检查文件</small>
        <strong>{result.scanned}</strong>
       </div>
       <span class="stat-badge neutral">100%</span>
      </button>
      <button
       class="stat-card pass"
       class:selected={tab === 'passed'}
       onclick={() => (tab = 'passed')}
      >
       <div class="stat-info">
        <small>检查通过</small>
        <strong>{passed}</strong>
       </div>
       <span class="stat-badge pass">{passRate}%</span>
      </button>
      <button
       class="stat-card issue"
       class:selected={tab === 'issue'}
       onclick={() => (tab = 'issue')}
      >
       <div class="stat-info">
        <small>发现问题</small>
        <strong>{issues}</strong>
       </div>
       <span class="stat-badge issue">{issues > 0 ? '需处理' : '无违规'}</span>
      </button>
      <button
       class="stat-card unreadable"
       class:selected={tab === 'unreadable'}
       onclick={() => (tab = 'unreadable')}
      >
       <div class="stat-info">
        <small>未检查 / 异常</small>
        <strong>{unreadable}</strong>
       </div>
       <span class="stat-badge unreadable">{unreadable > 0 ? '需留意' : '正常'}</span>
      </button>
     </div>

     <!-- Filter & Search Bar -->
     <div class="result-toolbar">
      <div class="result-tabs">
       {#each [
        { key: 'all', name: '全部', count: result.files.length },
        { key: 'issue', name: '问题文件', count: issues },
        { key: 'passed', name: '已通过', count: passed },
        { key: 'unreadable', name: '未检查', count: unreadable }
       ] as t}
        <button
         class:active={tab === t.key}
         onclick={() => {
          tab = t.key as typeof tab;
         }}
        >
         <span>{t.name}</span>
         <span class="tab-count">{t.count}</span>
        </button>
       {/each}
      </div>

      <div class="result-controls-right">
       <div class="result-search">
        <Search size={15} />
        <input
         value={resultQuery}
         placeholder="筛选文件名、路径、关键词或原文上下文…"
         oninput={e => (resultQuery = e.currentTarget.value)}
        />
        {#if resultQuery}
         <button class="icon" title="清空筛选" onclick={() => (resultQuery = '')}>
          <X size={14} />
         </button>
        {/if}
       </div>

       <div class="sort-box" title="文件列表排序方式">
        <ArrowUpDown size={14} />
        <select
         aria-label="排序方式"
         value={sortMode}
         onchange={e => (sortMode = e.currentTarget.value as SortMode)}
        >
         <option value="status">问题优先</option>
         <option value="hits">按命中数量</option>
         <option value="name">按文件名 A-Z</option>
        </select>
       </div>
      </div>
     </div>

     <!-- Dual Column Inspector -->
     <div class="result-columns" style:--list-width={splitPercent + '%'}>
      <div class="file-results">
       {#each filtered as f (f.path)}
        {@const forbiddenCount = f.hits.filter(h => h.kind === 'forbidden').length}
        {@const searchCount = f.hits.filter(h => h.kind !== 'forbidden').length}
        <button
         class:active={chosen?.path === f.path}
         class="file-result"
         onclick={() => {
          selected = f.path;
          hitFilter = 'all';
          hitWordFilter = '';
         }}
         ondblclick={() => openDocument(f.path, false)}
         oncontextmenu={e => openContext(e, f.path)}
        >
         <span class="file-ext-badge {f.status}">{fileExtBadge(f.path)}</span>
         <span class="truncate file-result-main">
          <strong>{title(f.path)}</strong>
          <small title={f.path}>{parentDir(f.path)}</small>
          {#if forbiddenCount > 0 || f.missing.length > 0 || searchCount > 0}
           <span class="file-mini-stats">
            {#if forbiddenCount > 0}
             <span class="mini-stat danger">⛔ {forbiddenCount}</span>
            {/if}
            {#if f.missing.length > 0}
             <span class="mini-stat warn">缺 {f.missing.length}</span>
            {/if}
            {#if searchCount > 0}
             <span class="mini-stat info">🔎 {searchCount}</span>
            {/if}
           </span>
          {/if}
         </span>
         <span
          class="status"
          class:issue={f.status === 'issue'}
          class:unreadable={f.status === 'unreadable'}
         >
          {statusName(f.status)}
         </span>
        </button>
       {:else}
        <div class="empty-small">没有符合当前筛选条件的文件</div>
       {/each}
      </div>

      <button
       type="button"
       class="split-handle"
       aria-label="左右拖动调整列表宽度"
       title="拖动调整分栏宽度"
       onpointerdown={onSplitterDown}
       onpointermove={onSplitterMove}
       onpointerup={onSplitterUp}
       onpointercancel={onSplitterUp}
       onkeydown={e => {
        if (e.key === 'ArrowLeft' || e.key === 'ArrowRight') {
         splitPercent = Math.max(
          22,
          Math.min(65, splitPercent + (e.key === 'ArrowRight' ? 2 : -2))
         );
         try {
          localStorage.setItem('docinspector_split_v1', String(splitPercent));
         } catch {}
        }
       }}
      >
       <span></span>
      </button>

      <div class="detail">
       {#if chosen}
        <div class="detail-head">
         <div class="detail-title-bar">
          <div class="detail-title-left">
           <span class="file-ext-badge large {chosen.status}">{fileExtBadge(chosen.path)}</span>
           <h2>{title(chosen.path)}</h2>
           <span
            class="status"
            class:issue={chosen.status === 'issue'}
            class:unreadable={chosen.status === 'unreadable'}
           >
            {statusName(chosen.status)}
           </span>
          </div>
          {#if !mobile}
           <div class="file-actions">
            <button class="outline" onclick={() => openDocument(chosen!.path, false)}>
             <ExternalLink size={14} /> 打开文件
            </button>
            <button class="outline" onclick={() => openDocument(chosen!.path, true)}>
             <FolderOpen size={14} /> 打开所在目录
            </button>
           </div>
          {/if}
         </div>
         <p class="detail-path">
          <span>{chosen.path}</span>
          <button
           class="icon"
           title="复制完整路径"
           onclick={() => copyPath(chosen!.path, '完整路径已复制')}
          >
           <ClipboardCopy size={14} />
          </button>
         </p>

         {#if chosen.hits.length > 0 || chosen.missing.length > 0}
          <div class="detail-filter-bar">
           <div class="rule-filter-pills">
            <button
             class:active={hitFilter === 'all' && !hitWordFilter}
             onclick={() => {
              hitFilter = 'all';
              hitWordFilter = '';
             }}
            >
             全部证据 ({chosen.hits.length + chosen.missing.length})
            </button>
            {#if chosen.hits.some(h => h.kind === 'forbidden')}
             <button
              class:active={hitFilter === 'forbidden'}
              onclick={() => {
               hitFilter = 'forbidden';
               hitWordFilter = '';
              }}
             >
              ⛔ 禁止词 ({chosen.hits.filter(h => h.kind === 'forbidden').length})
             </button>
            {/if}
            {#if chosen.missing.length > 0}
             <button
              class:active={hitFilter === 'required'}
              onclick={() => {
               hitFilter = 'required';
               hitWordFilter = '';
              }}
             >
              ✅ 缺失必含词 ({chosen.missing.length})
             </button>
            {/if}
            {#if chosen.hits.some(h => h.kind === 'search')}
             <button
              class:active={hitFilter === 'search'}
              onclick={() => {
               hitFilter = 'search';
               hitWordFilter = '';
              }}
             >
              🔎 检索命中 ({chosen.hits.filter(h => h.kind === 'search').length})
             </button>
            {/if}
           </div>

           {#if chosenWordCounts.length > 1}
            <div class="word-filter-chips">
             {#each chosenWordCounts as item (`${item.kind}:${item.word}`)}
              <button
               class="word-pill {item.kind}"
               class:active={hitWordFilter === item.word}
               onclick={() =>
                (hitWordFilter = hitWordFilter === item.word ? '' : item.word)}
              >
               <span>{item.word}</span>
               <small>{item.count}</small>
              </button>
             {/each}
            </div>
           {/if}
          </div>
         {/if}
        </div>

        {#if chosen.error}
         <div class="alert">
          <FileWarning size={18} />
          <div>
           <strong>无法完成文档内容检查</strong>
           <p>{chosen.error}</p>
          </div>
         </div>
        {/if}

        {#if hitFilter === 'all' || hitFilter === 'required'}
         {#each chosen.missing as word}
          <div class="hit missing">
           <span><FileWarning size={16} /> 必须包含 · <strong>{word}</strong></span>
           <small>全文未找到该必需词</small>
          </div>
         {/each}
        {/if}

        {#each filteredChosenHits as hit}
         <div class="hit" class:forbidden-hit={hit.kind === 'forbidden'}>
          <div class="hit-label" class:danger={hit.kind === 'forbidden'}>
           <div class="hit-label-left">
            <span class="hit-kind-badge {hit.kind}">{kindFullLabel(hit.kind)}</span>
            <strong>{hit.word}</strong>
           </div>
           <div class="hit-label-right">
            <small class="location-tag">{hit.location}</small>
            <button
             class="icon mini-copy"
             title="复制此段上下文"
             onclick={() => copyPath(hit.context, '原文片段已复制')}
            >
             <ClipboardCopy size={13} />
            </button>
           </div>
          </div>
          <p>
           {#each highlightChunks(hit.context, hit.matched || hit.word) as chunk}
            {#if chunk.mark}
             <mark class={hit.kind}>{chunk.text}</mark>
            {:else}
             {chunk.text}
            {/if}
           {/each}
          </p>
         </div>
        {/each}

        {#if !chosen.error && !chosen.missing.length && !chosen.hits.length}
         <div class="pass-banner">
          <CircleCheck size={32} />
          <h3>文档检查通过</h3>
          <p>未检出任何禁止出现的关键词，且满足当前方案的全部必含词要求。</p>
         </div>
        {/if}
       {:else}
        <div class="empty-small">选择左侧文件查看命中位置与上下文证据</div>
       {/if}
      </div>
     </div>
    </div>
   {:else if !busy}
    <div class="empty-main">
     <FileText size={36} />
     <h2>尚无扫描结果</h2>
     <p>请先在工作台选择文件或文件夹并启动扫描</p>
     <button class="primary" onclick={() => (page = 'home')}>返回工作台</button>
    </div>
   {/if}

  {:else if page === 'settings'}
   <header class="workspace-header">
    <div class="header-with-back">
     <button class="icon" onclick={() => (page = 'home')} title="返回工作台">
      <ArrowLeft size={19} />
     </button>
     <div>
      <h1>设置与方案迁移</h1>
      <p>管理界面外观、导出备份或批量导入检查方案 JSON，查看支持格式说明</p>
     </div>
    </div>
   </header>

   <div class="content settings">
    <section class="panel-card">
     <div class="section-head">
      <h2>界面外观</h2>
     </div>
     <div class="settings-row">
      <div>
       <strong>主题模式</strong>
       <p class="row-sub">支持深色星夜模式与高对比浅色纸感模式切换</p>
      </div>
      <div class="rule-filter-pills">
       <button class:active={theme === 'dark'} onclick={() => applyTheme('dark')}>
        <Moon size={14} /> 深色模式
       </button>
       <button class:active={theme === 'light'} onclick={() => applyTheme('light')}>
        <Sun size={14} /> 浅色模式
       </button>
      </div>
     </div>
    </section>

    <section class="panel-card">
     <div class="section-head">
      <h2>方案备份与迁移</h2>
     </div>
     <div class="settings-row">
      <div>
       <strong>导出全部检查方案 (JSON)</strong>
       <p class="row-sub">将本机保存的 {profiles.length} 个检查方案及匹配选项复制到剪贴板</p>
      </div>
      <button class="outline" onclick={copyProfileJson}>
       <Copy size={15} /> 复制全部方案 JSON
      </button>
     </div>
     <label class="field">
      <span>从 JSON 导入方案</span>
      <textarea
       rows="5"
       bind:value={importText}
       placeholder="在此粘贴导出的方案 JSON 内容…"
      ></textarea>
     </label>
     <button class="primary" onclick={importProfiles} disabled={!importText.trim()}>
      <Upload size={16} /> 导入方案
     </button>
    </section>

    <section class="panel-card">
     <div class="section-head">
      <h2>支持的文件类型与定位粒度</h2>
     </div>
     <div class="format-support-grid">
      <div class="format-item">
       <strong>📄 版式与办公文档</strong>
       <p>PDF（精确到页码与行号）、DOCX（正文/页眉/页脚/批注/脚注精确到段落）、PPTX（精确到幻灯片页码与演讲者备注段落）</p>
      </div>
      <div class="format-item">
       <strong>📊 电子表格</strong>
       <p>XLSX、XLS、ODS、CSV（精确到工作表名称及单元格行列坐标 R#C#）</p>
      </div>
      <div class="format-item">
       <strong>📝 文本与配置文件</strong>
       <p>TXT、MD、RST、JSON、XML、HTML、YAML、TOML、INI、LOG（自动兼容 UTF-8 / UTF-16 / GB18030 中文编码，精确到行号）</p>
      </div>
      <div class="format-item">
       <strong>💻 源代码文件</strong>
       <p>Rust、Python、JS/TS、C/C++、Java、Go、SQL、Shell、LaTeX/BibTeX、Svelte、Vue 等常见编程语言代码</p>
      </div>
     </div>
    </section>
   </div>
  {/if}

  {#if contextMenu}
   <div
    class="context-backdrop"
    role="presentation"
    onclick={() => (contextMenu = null)}
   ></div>
   <div
    class="file-context-menu"
    style:left={contextMenu.x + 'px'}
    style:top={contextMenu.y + 'px'}
   >
    <button
     onclick={() => {
      const p = contextMenu!.path;
      contextMenu = null;
      void openDocument(p, false);
     }}
    >
     打开文件
    </button>
    <button
     onclick={() => {
      const p = contextMenu!.path;
      contextMenu = null;
      void openDocument(p, true);
     }}
    >
     打开所在目录
    </button>
    <button
     onclick={() => {
      const p = contextMenu!.path;
      contextMenu = null;
      copyPath(p, '完整路径已复制');
     }}
    >
     复制完整路径
    </button>
    <button
     onclick={() => {
      const p = contextMenu!.path;
      contextMenu = null;
      copyPath(title(p), '文件名已复制');
     }}
    >
     复制文件名
    </button>
   </div>
  {/if}

  {#if notice}
   <div class="toast" role="status">
    <span>{notice}</span>
    <button title="关闭提示" onclick={() => (notice = '')}><X size={15} /></button>
   </div>
  {/if}
 </main>
</div>
