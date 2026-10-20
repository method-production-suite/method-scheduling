// Method Schedule in a browser (docs/research/web-session.md, phase 2): the part of Tauri
// the page (crates/method-schedule-app/ui) uses, over method-schedule-web's Host running
// as WebAssembly in this page. Loaded as a classic script before the page's modules, so
// window.__TAURI__ is there when api.js looks for it. Method Budget's shim, for one window.
//
//   core.invoke(cmd, args)          Host.dispatch, with a reply as the desktop gives it
//                                   (a refusal rejects with its message)
//   event.listen / emit             one event bus for the page
//   webviewWindow                   the page's window label, "main"
//
// Where the desktop shows an Open dialog the browser's file picker shows, and the picked
// file goes in as `file: {name, bytes}` (bytes as base64); a reply's `downloads` are
// offered as downloads and the rest is the desktop's reply. Commands only the desktop has
// (windows, autosave, recovery, backups, the updater, printing to a file) are refused with
// a plain message, or answered as "nothing there" where the page asks at start-up. The
// preferences are kept in this browser's storage.
//
// The page's address can say ?open=<url of a .mtsc file> to start with that schedule,
// ?sample=1 to start with the Field Station sample, and ?readonly=1 to show every
// schedule read-only; otherwise it starts with a new, untitled schedule.
//
// A partner embedding the page needs this file beside the page, with
// method_schedule_web.js and method_schedule_web_bg.wasm beside this file (all of it as
// `cargo xtask web` lays it out).

(function () {
  'use strict';

  const params = new URLSearchParams(window.location.search);
  const LABEL = 'main';
  const here = document.currentScript ? document.currentScript.src : window.location.href;
  const doc = document;
  const PREFS_KEY = 'method-schedule-prefs';
  // The most the page reads into the module: a schedule, a script or a layouts file, and a
  // picture (the host's own limits are 64 MB and 8 MB; a browser tab is kept well under).
  const MAX_SCHEDULE_BYTES = 16 * 1024 * 1024;
  const MAX_PICTURE_BYTES = 8 * 1024 * 1024;

  window.__TAURI__ = {
    core: { invoke: (cmd, args) => invoke(cmd, args || {}) },
    event: {
      listen: (name, handler, opts) => listen(name, handler, opts),
      emit: (name, payload) => emit(name, payload),
      emitTo: (_target, name, payload) => emit(name, payload),
    },
    webviewWindow: { getCurrentWebviewWindow: () => ({ label: LABEL }) },
  };

  // -------------------------------------------------------------------------
  // Starting: the module, the Host, the schedule to start with

  let commands = new Set(); // every command the Host runs
  const ready = (async () => {
    const m = await import(new URL('method_schedule_web.js', here).href);
    await m.default({ module_or_path: new URL('method_schedule_web_bg.wasm', here) });
    const host = new m.Host();
    commands = new Set(m.commands());
    const kept = readPrefs();
    if (kept) {
      try {
        host.dispatch('set_preferences', JSON.stringify({ prefs: kept }));
      } catch (e) {
        console.error(e); // kept by an older page: the defaults instead
      }
    }
    let opened = false;
    const open = params.get('open');
    if (open) {
      try {
        const url = openTarget(open, window.location.href);
        if (!url) throw new Error('only a schedule file (.mtsc) on this site can be opened from the address');
        // Same site only, no following a redirect elsewhere, and no bigger than a schedule
        // file may be.
        const res = await fetch(url, { credentials: 'same-origin', redirect: 'error' });
        if (!res.ok) throw new Error(`the server answered ${res.status}`);
        const size = Number(res.headers.get('content-length'));
        if (size > MAX_SCHEDULE_BYTES) throw new Error('it is too large to be a schedule');
        const text = await res.text();
        if (text.length > MAX_SCHEDULE_BYTES) throw new Error('it is too large to be a schedule');
        const name = decodeURIComponent(url.pathname.split('/').pop() || 'Schedule.mtsc');
        host.openMtsc(text, name);
        opened = true;
      } catch (e) {
        notice(`Couldn't open ${String(open).slice(0, 120)}: ${message(e)}`);
      }
    }
    if (!opened) host.dispatch(params.get('sample') === '1' ? 'open_sample' : 'new_schedule', '{}');
    if (params.get('readonly') === '1') host.setReadOnly(true);
    return host;
  })();
  ready.catch((e) => notice(`Method Schedule couldn't start in this browser: ${message(e)}`));
  window.__METHOD_SCHEDULE_WEB__ = { ready, openTarget };

  // -------------------------------------------------------------------------
  // What the page will read

  /**
   * The schedule `?open=` may load: a `.mtsc` (or `.json`) file on this page's own site
   * over http or https, with no user name or password in it; null for anything else.
   */
  function openTarget(open, here) {
    let url;
    try {
      url = new URL(String(open), here);
    } catch (e) {
      return null;
    }
    const base = new URL(here);
    if (url.origin !== base.origin || !/^https?:$/.test(url.protocol)) return null;
    if (url.username || url.password) return null;
    if (!/\.(mtsc|json)$/i.test(url.pathname)) return null;
    return url;
  }

  // -------------------------------------------------------------------------
  // Events

  const listeners = new Map(); // id -> { name, handler }
  let nextId = 1;

  function listen(name, handler) {
    const id = nextId++;
    listeners.set(id, { name, handler });
    return Promise.resolve(() => listeners.delete(id));
  }

  function emit(name, payload) {
    for (const [id, l] of listeners) {
      if (l.name !== name) continue;
      setTimeout(() => {
        if (!listeners.has(id)) return;
        try {
          l.handler({ event: name, id, payload });
        } catch (e) {
          console.error(e);
        }
      }, 0);
    }
    return Promise.resolve();
  }

  // -------------------------------------------------------------------------
  // Commands

  // Where the desktop shows an Open dialog: what the picker offers.
  const SCHEDULE = '.mtsc,.json';
  const SCRIPT = '.fountain,.fdx,.mscenes,.txt';
  const PICTURE = '.png,.jpg,.jpeg,.gif,.webp';
  const PICKS = {
    // The schedule, and its pictures picked with it (Save downloads them beside it).
    open_schedule: { accept: `${SCHEDULE},.mtst,${PICTURE}`, multiple: true },
    new_from_template: { accept: '.mtst,.mtsc' },
    import_script: { accept: SCRIPT },
    import_script_tags: { accept: '.fdx' },
    revise_pick: { accept: SCRIPT },
    import_layouts: { accept: '.mtlayouts,.json' },
    add_element_image: { accept: PICTURE },
    add_sheet_image: { accept: PICTURE },
  };

  // Desktop commands the page asks at start-up or in passing: answered as "nothing there"
  // rather than refused, so nothing is shown.
  const QUIET = {
    open_pending: null,
    recovery_list: [],
    outside_check: null,
    update_status: { phase: 'idle', version: null, manual: false, said: null, automatic: false, available: false },
    update_whats_new: null,
    // The page prints itself (report.js falls back to the browser's print).
    print_report: { supported: false },
  };

  const NOT_HERE = 'That is only in the Method Schedule app, not in the browser.';

  let host = null;
  let dirty = false;

  async function invoke(cmd, args) {
    host = await ready;
    if (Object.prototype.hasOwnProperty.call(QUIET, cmd)) return QUIET[cmd];
    if (cmd === 'open_link' && args.which === 'suite') {
      window.open('https://methodproduction.com', '_blank', 'noopener');
      return null;
    }
    const pick = cmd === 'what_changed' && args.against === 'file' ? { accept: SCHEDULE } : PICKS[cmd];
    if (pick && args.file === undefined) {
      const files = await pickFiles(pick);
      if (!files.length) return null;
      if (pick.multiple) {
        const isPicture = (f) => /\.(png|jpe?g|gif|webp)$/i.test(f.name);
        const main = files.find((f) => !isPicture(f));
        if (!main) throw 'Pick the schedule file too, with its pictures if it has any.';
        const pictures = await Promise.all(files.filter((f) => f !== main && isPicture(f)).map(fileIn));
        args = { ...args, file: await fileIn(main), pictures };
      } else {
        args = { ...args, file: await fileIn(files[0]) };
      }
    }
    let reply = dispatch(cmd, args);
    if (reply && typeof reply === 'object' && Array.isArray(reply.downloads)) {
      const { downloads, ...rest } = reply;
      for (const f of downloads) download(f.name, decode(f.bytes));
      reply = rest;
    }
    if (cmd === 'set_preferences') {
      writePrefs(reply);
      emit('prefs-changed', reply);
    }
    after(reply);
    return reply;
  }

  /** Runs a command on the Host: its reply, or a rejection with the page's message. */
  function dispatch(cmd, args) {
    if (!commands.has(cmd)) throw NOT_HERE;
    try {
      return JSON.parse(host.dispatch(cmd, JSON.stringify(args)));
    } catch (e) {
      if (e instanceof WebAssembly.RuntimeError) {
        throw 'Method Schedule stopped working in this page. Download what you can with Save, then reload the page.';
      }
      throw message(e);
    }
  }

  /** After a command: the page's title follows the schedule, and leaving asks while unsaved. */
  function after(reply) {
    const view = viewIn(reply);
    if (!view) return;
    dirty = Boolean(view.file && view.file.dirty);
    const name = view.file && view.file.name ? view.file.name : view.title;
    doc.title = `${name}${dirty ? ' (edited)' : ''} - Method Schedule`;
  }

  function viewIn(reply) {
    if (!reply || typeof reply !== 'object') return null;
    if ('board' in reply && 'title' in reply) return reply;
    return viewIn(reply.view);
  }

  window.addEventListener('beforeunload', (e) => {
    if (!dirty) return;
    e.preventDefault();
    e.returnValue = '';
  });

  // -------------------------------------------------------------------------
  // The preferences, kept in this browser

  function readPrefs() {
    try {
      const text = window.localStorage.getItem(PREFS_KEY);
      return text ? JSON.parse(text) : null;
    } catch (e) {
      return null;
    }
  }

  function writePrefs(prefs) {
    try {
      window.localStorage.setItem(PREFS_KEY, JSON.stringify(prefs));
    } catch (e) {
      // A private window or storage turned off: kept for this visit only.
    }
  }

  // -------------------------------------------------------------------------
  // Files in and out

  function pickFiles({ accept, multiple }) {
    return new Promise((resolve) => {
      const input = doc.createElement('input');
      input.type = 'file';
      if (accept) input.accept = accept;
      input.multiple = Boolean(multiple);
      input.style.display = 'none';
      doc.body.append(input);
      let done = false;
      const finish = (files) => {
        if (done) return;
        done = true;
        input.remove();
        resolve(files);
      };
      input.addEventListener('change', () => finish([...(input.files || [])]));
      input.addEventListener('cancel', () => finish([]));
      input.click();
    });
  }

  async function fileIn(f) {
    const picture = /\.(png|jpe?g|gif|webp)$/i.test(f.name);
    const max = picture ? MAX_PICTURE_BYTES : MAX_SCHEDULE_BYTES;
    // Refused before it is read, so a huge file never fills the tab.
    if (f.size > max) throw `${f.name} is too large to open here (${picture ? 'a picture is 8 MB' : 'a file is 16 MB'} at the most).`;
    return { name: f.name, bytes: encode(new Uint8Array(await f.arrayBuffer())) };
  }

  function encode(bytes) {
    let s = '';
    for (let i = 0; i < bytes.length; i += 0x8000) s += String.fromCharCode.apply(null, bytes.subarray(i, i + 0x8000));
    return btoa(s);
  }

  function decode(b64) {
    const s = atob(b64);
    const out = new Uint8Array(s.length);
    for (let i = 0; i < s.length; i++) out[i] = s.charCodeAt(i);
    return out;
  }

  function download(name, bytes, type = 'application/octet-stream') {
    const url = URL.createObjectURL(new Blob([bytes], { type }));
    const a = doc.createElement('a');
    a.href = url;
    a.download = name;
    a.style.display = 'none';
    doc.body.append(a);
    a.click();
    a.remove();
    setTimeout(() => URL.revokeObjectURL(url), 60000);
  }

  // -------------------------------------------------------------------------
  // Messages

  function message(e) {
    if (typeof e === 'string') return e;
    return (e && e.message) || String(e);
  }

  /** A message where the page shows its own (the status line). */
  function notice(text) {
    console.error(text);
    const show = () => {
      const t = doc.getElementById('status');
      if (!t) return;
      t.textContent = text;
      t.classList.add('error');
    };
    if (doc.readyState === 'loading') doc.addEventListener('DOMContentLoaded', show);
    else show();
  }
})();
