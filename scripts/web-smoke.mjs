// The browser build's headless smoke test (`cargo xtask web --smoke`): loads the built
// module under Node, as the shim does in a page, and runs the page's commands on the
// Field Station sample. Nothing is shown. Usage: node scripts/web-smoke.mjs <built folder>,
// the folder holding method_schedule_web.js and method_schedule_web_bg.wasm.

import { readFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

const dir = resolve(process.argv[2] || 'target/web/method-schedule-web');
const m = await import(pathToFileURL(join(dir, 'method_schedule_web.js')).href);
await m.default({ module_or_path: await readFile(join(dir, 'method_schedule_web_bg.wasm')) });

let passed = 0;
function check(what, ok) {
  if (!ok) {
    console.log(`not ok: ${what}`);
    process.exit(1);
  }
  passed += 1;
}

const host = new m.Host();
const run = (cmd, args = {}) => JSON.parse(host.dispatch(cmd, JSON.stringify(args)));

const info = JSON.parse(host.hostInfo());
check('the host says it is the web one', info.kind === 'web' && info.commands_version >= 1);
check('the commands are listed', m.commands().includes('get_view') && m.commands().includes('export_report'));

const opened = run('open_sample');
check('the sample opens', opened.doc === 1 && opened.view.title.startsWith('Field Station'));
const view = run('get_view');
const board = view.board.id;
const sub = view.board.sub_boards[0].id;
check('the board has days', view.board.sub_boards[0].days.length > 0);

const renamed = run('rename_board', { board, name: 'Second unit' });
check('an edit changes the schedule', renamed.boards.some((b) => b.name === 'Second unit') && renamed.can_undo);
const undone = run('undo');
check('undo takes it back', !undone.boards.some((b) => b.name === 'Second unit'));

const report = run('export_report', { sub_board: sub, report: 'dood', format: 'csv' });
check('a report exports as a download', report.done === true && report.downloads[0].name.endsWith('.csv'));
const csv = Buffer.from(report.downloads[0].bytes, 'base64').toString('utf8');
check('the report has rows', csv.split('\n').length > 3);

const saved = host.saveMtsc();
check('the schedule saves as .mtsc text', saved.name.endsWith('.mtsc') && saved.text.length > 1000);
const again = JSON.parse(host.openMtsc(saved.text, saved.name));
check('the saved text opens again', again.doc === 2 && again.view.title === opened.view.title);

host.setReadOnly(true);
let refused = '';
try {
  run('rename_board', { board, name: 'X' });
} catch (e) {
  refused = e.message;
}
check('read-only refuses an edit', refused.includes('read-only'));
let unknown = '';
try {
  run('no_such_command');
} catch (e) {
  unknown = e.message;
}
check('an unknown command is refused in words', unknown.includes('no command'));

console.log(`web smoke: ${passed} checks passed`);
