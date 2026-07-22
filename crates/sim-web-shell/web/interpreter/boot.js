// SIM Web-UI compatibility shell boot script.
//
// The active shell loads app.js. This compatibility boot only confirms the
// page loaded and reports it in the console; it contains no domain logic and no
// second data model.
"use strict";

(function boot() {
  const shell = document.getElementById("shell");
  if (!shell) {
    return;
  }
  shell.dataset.booted = "compat";
  // eslint-disable-next-line no-console
  console.log("sim-web-shell: compatibility shell booted");
})();
