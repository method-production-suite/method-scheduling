// The website's demo (methodschedule.com/try/, docs/WEBSITE.md). Loaded before the shim,
// which reads the address: unless it names a schedule already, the demo starts with the
// Field Station sample (?sample=1). It shows the note about running in the browser, which
// can be hidden.

(function () {
  'use strict';

  const params = new URLSearchParams(window.location.search);
  if (!params.has('open') && !params.has('sample')) {
    const search = window.location.search ? `${window.location.search}&` : '?';
    window.history.replaceState(null, '', `${search}sample=1${window.location.hash}`);
  }

  function show() {
    const note = document.getElementById('ms-demo');
    if (!note) return;
    note.hidden = false;
    document.body.classList.add('ms-demo-on');
    const close = document.getElementById('ms-demo-close');
    if (close) {
      close.addEventListener('click', () => {
        note.hidden = true;
        document.body.classList.remove('ms-demo-on');
      });
    }
  }

  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', show);
  else show();
})();
