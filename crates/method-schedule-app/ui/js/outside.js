// Changes made outside the app, settled here, and File > Revert to Backup (src/outside.rs;
// decisions.md, entries 179 to 182). Rust finds the change and decides what to offer; this
// asks and draws. Text goes in with textContent only.
//
// The check runs when the window gets focus (Rust sends `outside-check`), when autosave
// finds the file changed, and before Save. Merge is Merge Changes From's dialog
// (diffmerge.js) with the file on disk as theirs and the version last read here as the
// base.

import { invoke, listen } from './api.js';
import { el } from './dom.js';
import { confirmDialog, customDialog, isModalOpen } from './modal.js';
import { runMerge, whatChanged } from './diffmerge.js';
import { backupLine, outsideButtons, outsideMessage } from './logic.js';

/** The question: resolves with the choice (`later` when dismissed). */
function askOutside(dto) {
  return customDialog((close) => {
    const buttons = outsideButtons(dto).map((b) =>
      el('button', {
        class: b.primary ? 'btn btn-primary' : 'btn',
        text: b.label,
        attrs: { type: 'button', ...(b.primary ? { 'data-autofocus': true } : {}) },
        on: { click: () => close(b.choice) },
      }),
    );
    return el(
      'div',
      { class: 'modal modal-wide', attrs: { role: 'alertdialog', 'aria-modal': 'true', 'aria-labelledby': 'outside-title', 'aria-describedby': 'outside-message' } },
      el('h2', { class: 'modal-title', text: 'Changed outside Method Schedule', attrs: { id: 'outside-title' } }),
      el('div', { attrs: { id: 'outside-message' } }, ...outsideMessage(dto).map((t) => el('p', { class: 'modal-message', text: t }))),
      el('div', { class: 'modal-actions' }, ...buttons),
    );
  }).then((c) => c || 'later');
}

/** File > Revert to Backup: resolves with the backup's name, or null. */
function pickBackup(list) {
  return customDialog((close) => {
    const select = el(
      'select',
      { attrs: { id: 'revert-pick', size: String(Math.max(2, Math.min(list.backups.length, 6))), 'data-autofocus': true } },
      ...list.backups.map((b) => el('option', { text: backupLine(b), attrs: { value: b.name } })),
    );
    if (list.backups.length) select.value = list.backups[0].name;
    const revert = el('button', { class: 'btn btn-primary', text: 'Revert', attrs: { type: 'button' }, on: { click: () => close(select.value ? { revert: select.value } : null) } });
    revert.disabled = !list.backups.length;
    // What changed from the picked backup to the schedule as it is now (M4).
    const compare = el('button', { class: 'btn', text: 'What Changed', attrs: { type: 'button' }, on: { click: () => close(select.value ? { compare: select.value } : null) } });
    compare.disabled = !list.backups.length;
    select.addEventListener('dblclick', () => select.value && close({ revert: select.value }));
    const cancel = el('button', { class: 'btn', text: list.backups.length ? 'Cancel' : 'Close', attrs: { type: 'button' }, on: { click: () => close(null) } });
    const say = list.backups.length
      ? `The versions of ${list.file} kept before saves replaced them, newest first. The one you pick becomes the schedule here as one step: Undo takes it back, and the file as it is now goes into the backups first.`
      : `There are no backups of ${list.file} yet. One is kept each time a save replaces the file.`;
    return el(
      'div',
      { class: 'modal modal-wide', attrs: { role: 'dialog', 'aria-modal': 'true', 'aria-labelledby': 'revert-title' } },
      el('h2', { class: 'modal-title', text: 'Revert to Backup', attrs: { id: 'revert-title' } }),
      el('p', { class: 'modal-message', text: say }),
      list.backups.length ? el('label', { class: 'field', attrs: { for: 'revert-pick' } }, 'Backup', select) : null,
      el('div', { class: 'modal-actions' }, cancel, compare, revert),
    );
  });
}

/**
 * Sets up the check. `ctx`: { apply(promise) shows a command's view, say(text, isError),
 * show(view) }. Returns `check(force)`, which resolves true when it asked (Save then
 * leaves the rest to the choice), and `revertToBackup()`.
 */
export function initOutside(ctx) {
  let busy = false;

  async function settle(dto) {
    const choice = await askOutside(dto);
    if (choice === 'merge') {
      if (await runMerge(dto.token, ctx.say, ctx.show)) return;
      // Cancelled: as Later, so focus doesn't ask again about this version.
      await ctx.apply(invoke('outside_resolve', { choice: 'later', version: dto.version }));
      return;
    }
    if (choice === 'load_theirs' && dto.confirm_load) {
      const sure = await confirmDialog({
        title: 'Load Theirs',
        message: `Your unsaved changes here are dropped and ${dto.name} is opened as it is on disk. Undo brings your changes back.`,
        confirmText: 'Load Theirs',
        danger: true,
      });
      if (!sure) return settle(dto);
    }
    await ctx.apply(invoke('outside_resolve', { choice, version: dto.version }));
  }

  async function check(force) {
    if (busy || (!force && isModalOpen())) return false;
    busy = true;
    try {
      const dto = await invoke('outside_check', { force });
      if (!dto) return false;
      await settle(dto);
      return true;
    } catch (e) {
      ctx.say(e, true);
      return false;
    } finally {
      busy = false;
    }
  }

  listen('outside-check', () => check(false));

  return {
    check,
    async revertToBackup() {
      let list;
      try {
        list = await invoke('backup_list');
      } catch (e) {
        ctx.say(e, true);
        return;
      }
      const picked = await pickBackup(list);
      if (picked && picked.compare) await whatChanged('backup', ctx.say, picked.compare);
      else if (picked && picked.revert) await ctx.apply(invoke('revert_to_backup', { name: picked.revert }));
    },
  };
}
