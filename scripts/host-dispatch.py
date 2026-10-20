#!/usr/bin/env python3
"""Writes crates/method-schedule-session/src/host/generated.rs, the browser host's dispatch
(docs/research/web-session.md, phase 1b): every desktop command whose body needs only the
open schedule (no window, dialog, file or app state), its arguments read as the page sends
them (snake_case for `rename_all = "snake_case"` commands, camelCase otherwise) and its body
copied as written, run against the host's stand-ins for the desktop's helpers.

    python3 scripts/host-dispatch.py          # rewrite it
    python3 scripts/host-dispatch.py --check  # exit 1 when it is out of date (xtask ci)

EXCLUDE lists commands that look schedule-only but use desktop state (image files, the
revision flow's pending pick), and host_info and menu_table, which each host answers itself.
"""
import collections, glob, json, os, re, sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
os.chdir(ROOT)
CHECK = "--check" in sys.argv[1:]
EXCLUDE = {"element_image", "sheet_image", "revise_apply", "host_info", "menu_table"}
SRC='crates/method-schedule-app/src/'
cmds=[]
for path in sorted(glob.glob(SRC+'*.rs')):
    s=open(path).read()
    mod=path.split('/')[-1][:-3]
    for m in re.finditer(r'#\[tauri::command(\(rename_all = "snake_case"\))?\]\s*pub (async )?fn (\w+)(<R: Runtime>)?\(',s):
        start=m.end()
        # params until matching ')'
        depth=1;i=start
        while depth: 
            c=s[i]; depth+= (c=='(') - (c==')'); i+=1
        params=s[start:i-1]
        mm=re.match(r'\s*->\s*([^{]+)\{',s[i:]); ret=mm.group(1).strip() if mm else '()'
        j=s.index('{',i)
        depth=1;k=j+1
        while depth:
            c=s[k]; depth+=(c=='{')-(c=='}'); k+=1
        body=s[j+1:k-1]
        ps=[]
        for p in re.split(r',(?![^<]*>)',params):
            p=p.strip()
            if not p: continue
            n,t=p.split(':',1); ps.append((n.strip(),t.strip()))
        cmds.append(dict(mod=mod,name=m.group(3),snake=bool(m.group(1)),async_=bool(m.group(2)),params=ps,ret=ret,body=body))

c = cmds
HOSTY=re.compile(r'\bwindow\b|\bapp\b|\bsafety\b|\.await|DialogExt|emit|app_handle|AppHandle|Safety|updates|Updates|GUIDE|LICENSE_|NOTICES|crate::help|check\(&state')
def ok(x):
    if x['name'] in EXCLUDE: return False
    b=re.sub(r'(crate::app::)?(edit|strip_edit|board_edit|sheet_edit|edit_with)\(&window, &state,','',x['body'])
    b=re.sub(r'(app::)?finish\(\s*&window,','',b)
    return not HOSTY.search(b)
keep=[x for x in c if ok(x)]
SRC='crates/method-schedule-app/src/'
DROP=re.compile(r'use (tauri|tauri_plugin_dialog|crate::(app|docs|safety_app|windows|menu|updater|help|pdf_mac|outside|templates|diffmerge|export|safety)\b)|use crate::elements::\{[^}]*(pick_image|read_image)')
def camel(n):
    p=n.split('_'); return p[0]+''.join(w.title() for w in p[1:])
bymod=collections.OrderedDict()
for x in keep: bymod.setdefault(x['mod'],[]).append(x)
out=['//! Generated from the desktop\'s commands (crates/method-schedule-app/src) by',
     '//! scripts/host-dispatch.py: each arm is the desktop command\'s body, run on the host.',
     '//! Regenerate after a command changes; don\'t edit by hand.','',
     '#![allow(unused_imports, clippy::all)]','',
     'use serde_json::Value;','use super::{DocState, Error};','']
out.append('/// The desktop commands the host runs, in source order.')
out.append('pub const COMMANDS: &[&str] = &['+''.join(f'\n    "{x["name"]}",' for x in keep)+'\n];')
def is_read(x):
    b=x['body']; return not re.search(r'edit\(|\.undo\(|\.redo\(|set_clipboard',b)
out.append('')
out.append('/// The commands a read-only host still runs: they change no schedule.')
out.append('pub const READS: &[&str] = &['+''.join(f'\n    "{x["name"]}",' for x in keep if is_read(x))+'\n];')
out.append('')
out.append('/// Runs a desktop command; `None` when it isn\'t one of [`COMMANDS`].')
out.append('pub fn dispatch(command: &str, a: &Value, state: &DocState) -> Option<Result<Value, Error>> {')
for m in bymod: out.append(f'    if let Some(r) = {m}_cmds::dispatch(command, a, state) {{\n        return Some(r);\n    }}')
out.append('    None\n}\n')
for m,xs in bymod.items():
    src=open(SRC+m+'.rs').read()
    uses=[u for u in re.findall(r'(?ms)^use [^;]*;\n',src) if not DROP.match(u)]
    out.append(f'mod {m}_cmds {{')
    out.append('    use super::super::{args, board_edit, edit, edit_with, finish, sheet_edit, sheet_order, strip_edit, to_reply, CmdResult, DocState, Error, W};')
    out.append('    use serde::Deserialize;\n    use serde_json::Value;')
    for u in uses: out.append('    '+u.strip().replace('\n','\n    '))
    import os
    if os.path.exists(f'crates/method-schedule-session/src/{m}.rs'):
        out.append(f'    use crate::{m}::*;')
    out.append('')
    out.append('    pub fn dispatch(command: &str, a: &Value, state: &DocState) -> Option<Result<Value, Error>> {')
    out.append('        let window = W;\n        let _ = &window;')
    out.append('        Some(match command {')
    for x in xs:
        ps=[(n,t) for n,t in x['params'] if n not in ('window','state','app','safety')]
        out.append(f'            "{x["name"]}" => {{')
        if ps:
            ren='' if x['snake'] else '\n                #[serde(rename_all = "camelCase")]'
            fields=''.join(('\n                    #[serde(default)]' if t.startswith('Option<') else '')+f'\n                    {n}: {t},' for n,t in ps)
            out.append(f'                #[derive(Deserialize)]{ren}\n                struct A {{{fields}\n                }}')
            names=', '.join(n for n,_ in ps)
            out.append(f'                let A {{ {names} }} = match args::<A>(command, a) {{\n                    Ok(v) => v,\n                    Err(e) => return Some(Err(e)),\n                }};')
        body=x['body'].replace('method_schedule_format::exchange::local_today()','super::super::today()')
        body=body.replace('crate::app::edit(','edit(')
        body='\n'.join('                '+l.strip() if l.strip() else '' for l in body.strip('\n').split('\n'))
        out.append(f'                let r: {x["ret"]} = (|| {{\n'+body+'\n                })();')
        out.append('                to_reply(r)\n            }')
    out.append('            _ => return None,\n        })\n    }\n}\n')
text='\n'.join(out)
import subprocess
text=subprocess.run(['rustfmt','--edition','2021'],input=text,capture_output=True,text=True,check=True).stdout
TARGET='crates/method-schedule-session/src/host/generated.rs'
if CHECK:
    if open(TARGET).read() != text:
        print('host-dispatch: src/host/generated.rs is out of date; run python3 scripts/host-dispatch.py'); sys.exit(1)
    print('host-dispatch: up to date'); sys.exit(0)
open(TARGET,'w').write(text)
print('host-dispatch: wrote',TARGET,'with',len(keep),'commands,',sum(1 for x in keep if is_read(x)),'of them reads')
