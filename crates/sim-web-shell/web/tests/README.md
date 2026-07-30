# Browser-level tests

Browser-level end-to-end and golden-Scene tests for the SIM Web-UI live here.
They run under Node with a small DOM shim, so CI can verify the interpreter,
session client, and representative scene fixtures without a browser engine.

`e2e.test.mjs` also accepts one or more `{profile,title,scene}` JSON fixture
paths. `scene-fixture.html?scene=<url>` paints the same fixture in a real browser
through the production Scene interpreter for deterministic screenshot review.
