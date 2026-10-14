export type Kind = 'search'|'forbidden'|'required';
export interface Rule { id:string; word:string; kind:Kind; enabled:boolean; }
export interface ScanOptions { caseSensitive:boolean; wholeWord:boolean; skipCommonDirs:boolean; excludeDirs:string[]; }
export interface Profile { id:string; name:string; icon:string; color:string; rules:Rule[]; created:number; options?:ScanOptions; }
export interface Hit { word:string; kind:Kind; location:string; context:string; matched?:string; }
export interface FileResult { path:string; status:'passed'|'issue'|'unreadable'; hits:Hit[]; missing:string[]; error?:string; }
export interface ScanProgress { scanned:number; total:number; current:string; }
export interface ScanReport { files:FileResult[]; total:number; scanned:number; cancelled:boolean; durationMs?:number; }
