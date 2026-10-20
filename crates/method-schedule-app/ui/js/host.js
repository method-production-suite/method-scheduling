// Which host the page runs in (host_info): the desktop app, or a browser running
// method-schedule-web (docs/research/web-session.md, phase 2). In a browser the page
// draws its own menu bar (the menus' table comes from the host) and answers the menus'
// keys itself, and hides what only the desktop does: autosave, recovery, the updater.
//
// Until loadHost() has answered, and wherever host_info can't be asked, the host is the
// desktop: nothing changes there.

let info = { kind: 'desktop', read_only: false };

/** Asks the host once. */
export async function loadHost(invoke) {
  try {
    const i = await invoke('host_info');
    if (i && typeof i === 'object' && typeof i.kind === 'string') info = i;
  } catch (e) {
    // An older desktop build: the desktop.
  }
  return info;
}

/** True in a browser (method-schedule-web). */
export function isWeb() {
  return info.kind === 'web';
}
