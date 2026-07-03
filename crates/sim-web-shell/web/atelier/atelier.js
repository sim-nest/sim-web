const app = document.querySelector("#atelier-app");
const apiRoot = app?.dataset.apiRoot || "/api/atelier";

function el(tag, className, text) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

async function loadAtelier() {
  const response = await fetch(apiRoot);
  if (!response.ok) throw new Error(`Atelier API ${response.status}`);
  return response.json();
}

function renderStatus(data) {
  const mount = document.querySelector("#atelier-status");
  mount.replaceChildren();
  const cache = data.startup?.cache || {};
  for (const [key, value] of Object.entries(cache)) {
    const status = value === "current" || value === "ready" ? "ok" : "pending";
    mount.append(el("span", `badge ${status}`, `${key}: ${value}`));
  }
  const dirty = data.startup?.dirty_repos || [];
  const missing = data.startup?.missing_siblings || [];
  if (dirty.length) mount.append(el("span", "badge warn", `dirty: ${dirty.length}`));
  if (missing.length) mount.append(el("span", "badge error", `missing: ${missing.length}`));
}

function renderNavigation(data) {
  const mount = document.querySelector("#atelier-navigation");
  mount.replaceChildren(section("Navigation", data.navigation || [], renderNavSection));
}

function renderNavSection(entry) {
  const wrap = el("section", "atelier-section");
  wrap.append(el("h2", "", entry.kind || "section"));
  const list = el("ul", "atelier-list");
  for (const item of (entry.items || []).slice(0, 24)) {
    list.append(el("li", "", item));
  }
  wrap.append(list);
  return wrap;
}

function renderPanels(data) {
  const mount = document.querySelector("#atelier-panels");
  mount.replaceChildren(section("Panels", data.panels || [], (panel) => {
    const item = el("li");
    item.append(el("strong", "", panel.title || panel.id));
    item.append(document.createTextNode(` ${panel.source || ""}`));
    return item;
  }));
}

function renderRadar(data) {
  const mount = document.querySelector("#atelier-radar");
  mount.replaceChildren(section("Retrieval Radar", data.radar || [], (panel) => {
    const wrap = el("section", "atelier-section");
    wrap.append(el("h2", "", panel.panel || "radar"));
    const list = el("ul", "atelier-list");
    for (const hint of panel.hints || []) {
      const span = hint.span || {};
      list.append(el("li", "", `${hint.title} (${span.repo || "repo"}:${span.line || 1})`));
    }
    wrap.append(list);
    return wrap;
  }));
}

function renderFirewall(data) {
  const mount = document.querySelector("#atelier-firewall");
  const rules = data.firewall?.rules || [];
  const findings = data.firewall?.findings || [];
  const values = [
    ["rules", String(rules.length)],
    ["findings", String(findings.length)],
  ];
  const dl = el("dl", "atelier-kv");
  for (const [key, value] of values) {
    dl.append(el("dt", "", key), el("dd", "", value));
  }
  mount.replaceChildren(el("h2", "", "Guideline Firewall"), dl);
}

function section(title, rows, renderRow) {
  const wrap = el("section", "atelier-section");
  wrap.append(el("h2", "", title));
  if (!rows.length) {
    wrap.append(el("p", "", "No entries."));
    return wrap;
  }
  const list = el("ul", "atelier-list");
  for (const row of rows) list.append(renderRow(row));
  wrap.append(list);
  return wrap;
}

function render(data) {
  renderStatus(data);
  renderNavigation(data);
  renderPanels(data);
  renderRadar(data);
  renderFirewall(data);
}

if (app) {
  loadAtelier().then(render).catch((err) => {
    document.querySelector("#atelier-status").replaceChildren(
      el("span", "badge error", err.message),
    );
  });
}

export { render };
