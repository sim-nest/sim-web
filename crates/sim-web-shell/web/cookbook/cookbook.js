const app = document.querySelector("#cookbook-app");
const apiRoot = app?.dataset.apiRoot || "/api/cookbook";
const tree = document.querySelector("#cookbook-tree");
const pane = document.querySelector("#recipe-pane");
const search = document.querySelector("#cookbook-search");

const state = {
  families: [],
  recipes: [],
  selected: null,
  visibleIds: null,
};

async function loadCookbook() {
  const data = await fetchJson(apiRoot);
  state.families = familiesOf(data);
  state.recipes = data.recipes || [];
  state.visibleIds = null;
  state.selected = state.recipes[0]?.id || null;
  renderTree();
  if (state.selected) {
    await selectRecipe(state.selected);
  } else {
    renderEmpty("No recipes loaded.");
  }
}

// The index carries a two-level `families` tree (family -> domain book ->
// chapter -> recipe). An older server without it degrades to a single unnamed
// family over the flat `books` list, so the browser still renders every recipe.
function familiesOf(data) {
  if (Array.isArray(data.families) && data.families.length) {
    return data.families;
  }
  return [{ family: null, books: data.books || [] }];
}

async function fetchJson(url, options = {}) {
  const response = await fetch(url, options);
  const data = await response.json().catch(() => ({}));
  if (!response.ok) {
    throw new Error(data.error || `HTTP ${response.status}`);
  }
  return data;
}

// Render the two-level grouped tree: family (level 1) -> domain book (level 2)
// -> chapter -> recipe leaf. Family and domain are collapsible <details> so the
// whole constellation browses by subsystem; each leaf is badged runnable vs
// descriptor. A group with no visible recipe (search filtered) is skipped.
function renderTree() {
  tree.replaceChildren();
  if (!state.recipes.length) {
    tree.append(empty("No recipes loaded."));
    return;
  }
  const visible = state.visibleIds;
  let count = 0;
  for (const family of state.families) {
    const familyEl = document.createElement("details");
    familyEl.className = "family";
    familyEl.open = true;
    if (family.family) {
      const summary = document.createElement("summary");
      summary.className = "family-title";
      summary.textContent = family.family;
      familyEl.append(summary);
    }
    let familyCount = 0;
    for (const book of family.books || []) {
      const domainEl = document.createElement("details");
      domainEl.className = "domain";
      domainEl.open = true;
      const domainTitle = document.createElement("summary");
      domainTitle.className = "domain-title";
      domainTitle.textContent = book.title;
      domainEl.append(domainTitle);
      let domainCount = 0;
      for (const chapter of book.chapters || []) {
        const recipes = (chapter.recipes || []).filter((recipe) => {
          return !visible || visible.has(recipe.id);
        });
        if (!recipes.length) {
          continue;
        }
        const chapterLabel = document.createElement("div");
        chapterLabel.className = "chapter-label";
        chapterLabel.textContent = chapter.title;
        domainEl.append(chapterLabel);
        const list = document.createElement("ul");
        list.className = "recipe-list";
        for (const recipe of recipes) {
          count += 1;
          familyCount += 1;
          domainCount += 1;
          list.append(recipeItem(recipe));
        }
        domainEl.append(list);
      }
      if (domainCount) {
        familyEl.append(domainEl);
      }
    }
    if (familyCount) {
      tree.append(familyEl);
    }
  }
  if (!count) {
    tree.append(empty("No recipes found."));
  }
}

// A recipe leaf: a runnable/descriptor badge plus the recipe title, wired to
// select the recipe. `runnable` defaults to true when the field is absent.
function recipeItem(recipe) {
  const item = document.createElement("li");
  const button = document.createElement("button");
  button.type = "button";
  button.className = "recipe-button";
  button.append(recipeBadge(recipe.runnable !== false));
  const label = document.createElement("span");
  label.className = "recipe-label";
  label.textContent = recipe.title;
  button.append(label);
  button.dataset.recipeId = recipe.id;
  if (recipe.id === state.selected) {
    button.classList.add("selected");
    button.setAttribute("aria-current", "page");
  }
  button.addEventListener("click", () => selectRecipe(recipe.id));
  item.append(button);
  return item;
}

function recipeBadge(runnable) {
  const badge = document.createElement("span");
  badge.className = `badge ${runnable ? "runnable" : "descriptor"}`;
  badge.textContent = runnable ? "run" : "doc";
  badge.title = runnable
    ? "Runnable: computes on click"
    : "Descriptor: documented, not run in the sandbox";
  return badge;
}

async function selectRecipe(id) {
  state.selected = id;
  renderTree();
  const recipe = await fetchJson(`${apiRoot}/recipe/${encodeURIComponent(id)}`);
  renderRecipe(recipe);
}

function renderRecipe(recipe) {
  pane.replaceChildren();
  const title = document.createElement("h1");
  // The detail response carries `tags`, not the summary `runnable` flag; a
  // recipe tagged `sandbox-descriptor` is a documented descriptor.
  const runnable = !(recipe.tags || []).includes("sandbox-descriptor");
  title.append(recipeBadge(runnable), " ", recipe.title);
  const purpose = document.createElement("div");
  purpose.className = "purpose";
  purpose.append(...renderPurpose(recipe.purpose));
  const actions = document.createElement("div");
  actions.className = "recipe-actions";
  const copy = document.createElement("button");
  copy.type = "button";
  copy.textContent = "Copy";
  const run = document.createElement("button");
  run.type = "button";
  run.textContent = "Run";
  actions.append(copy, run);
  const setup = document.createElement("pre");
  setup.className = "setup-block";
  const code = document.createElement("code");
  code.textContent = recipe.setup;
  setup.append(code);
  const results = document.createElement("section");
  results.className = "results-panel";
  results.setAttribute("aria-live", "polite");
  results.textContent = "Run this recipe to see pass/fail data.";
  const footer = document.createElement("footer");
  footer.className = "recipe-footer";
  if (recipe.next) {
    const next = document.createElement("button");
    next.type = "button";
    next.textContent = "Next recipe ->";
    next.addEventListener("click", () => selectRecipe(recipe.next));
    footer.append(next);
  } else {
    footer.textContent = "No next recipe.";
  }
  copy.addEventListener("click", async () => {
    await navigator.clipboard.writeText(recipe.setup);
    results.textContent = "Copied.";
  });
  run.addEventListener("click", async () => {
    results.textContent = "Running.";
    try {
      const outcome = await fetchJson(
        `${apiRoot}/recipe/${encodeURIComponent(recipe.id)}/run`,
        { method: "POST" },
      );
      renderRunResults(results, outcome);
    } catch (error) {
      results.textContent = error.message;
    }
  });
  pane.append(title, purpose, actions, setup, results, footer);
}

function renderPurpose(markdown) {
  const nodes = [];
  let paragraph = null;
  for (const raw of (markdown || "").split(/\r?\n/)) {
    const line = raw.trim();
    if (!line) {
      paragraph = null;
      continue;
    }
    if (line.startsWith("# ")) {
      const heading = document.createElement("h2");
      heading.textContent = line.slice(2);
      nodes.push(heading);
      paragraph = null;
    } else {
      if (!paragraph) {
        paragraph = document.createElement("p");
        nodes.push(paragraph);
      } else {
        paragraph.append(" ");
      }
      paragraph.append(line);
    }
  }
  return nodes.length ? nodes : [empty("No purpose provided.")];
}

function renderRunResults(container, outcome) {
  container.replaceChildren();
  const summary = document.createElement("p");
  summary.className = "result-row";
  const status = document.createElement("span");
  status.className = outcome.ok ? "pass" : "fail";
  status.textContent = outcome.ok ? "PASS" : "FAIL";
  summary.append(status, ` forms: ${outcome.forms}`);
  container.append(summary);
  for (const value of outcome.results || []) {
    const row = document.createElement("p");
    row.className = "result-row";
    row.textContent = value;
    container.append(row);
  }
  for (const check of outcome.checks || []) {
    const row = document.createElement("p");
    row.className = "result-row";
    const status = document.createElement("span");
    status.className = check.pass ? "pass" : "fail";
    status.textContent = check.pass ? "pass" : "fail";
    row.append(status, ` expected=${check.expected} actual=${check.actual}`);
    container.append(row);
  }
}

function renderEmpty(message) {
  pane.replaceChildren();
  const title = document.createElement("h1");
  title.textContent = "Cookbook";
  pane.append(title, empty(message));
}

function empty(message) {
  const node = document.createElement("p");
  node.className = "empty-state";
  node.textContent = message;
  return node;
}

search.addEventListener("input", async () => {
  const q = search.value.trim();
  if (!q) {
    state.visibleIds = null;
    renderTree();
    if (!state.selected && state.recipes[0]) {
      await selectRecipe(state.recipes[0].id);
    }
    return;
  }
  const data = await fetchJson(`${apiRoot}/search?q=${encodeURIComponent(q)}`);
  state.visibleIds = new Set((data.recipes || []).map((recipe) => recipe.id));
  if (!state.visibleIds.has(state.selected)) {
    state.selected = data.recipes?.[0]?.id || null;
  }
  renderTree();
  if (state.selected) {
    await selectRecipe(state.selected);
  } else {
    renderEmpty("No recipes found.");
  }
});

loadCookbook().catch((error) => {
  tree.replaceChildren(empty("No recipes loaded."));
  renderEmpty(error.message);
});
