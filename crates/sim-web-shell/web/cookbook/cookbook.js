const app = document.querySelector("#cookbook-app");
const apiRoot = app?.dataset.apiRoot || "/api/cookbook";
const tree = document.querySelector("#cookbook-tree");
const pane = document.querySelector("#recipe-pane");
const search = document.querySelector("#cookbook-search");

const state = {
  libs: [],
  hasLibTree: false,
  families: [],
  recipes: [],
  selected: null,
  visibleIds: null,
};

async function loadCookbook(preferred = {}) {
  const data = await fetchJson(apiRoot);
  state.hasLibTree = hasLibTree(data);
  state.libs = libsOf(data);
  state.families = familiesOf(data);
  state.recipes = recipesOf(data);
  state.visibleIds = null;
  state.selected = initialSelection(preferred);
  renderTree();
  if (state.selected) {
    await selectRecipe(state.selected);
  } else {
    renderEmpty("No recipes loaded.");
  }
}

function initialSelection(preferred) {
  if (preferred.recipeId && state.recipes.some((recipe) => recipe.id === preferred.recipeId)) {
    return preferred.recipeId;
  }
  if (preferred.lib) {
    const sameLib = state.recipes.find((recipe) => {
      if (recipe.lib !== preferred.lib) return false;
      if (preferred.action) return recipe.action === preferred.action;
      return recipe.action !== "unload";
    });
    if (sameLib) return sameLib.id;
  }
  return state.recipes[0]?.id || null;
}

function libsOf(data) {
  return Array.isArray(data.libs) ? data.libs : [];
}

function hasLibTree(data) {
  return Array.isArray(data.libs);
}

function recipesOf(data) {
  if (Array.isArray(data.recipes)) {
    return data.recipes;
  }
  return recipesFromLibs(libsOf(data));
}

function recipesFromLibs(libs) {
  const recipes = [];
  for (const lib of libs || []) {
    recipes.push(...libRecipes(lib));
  }
  return recipes;
}

function libRecipes(lib) {
  const recipes = [];
  recipes.push(...(lib.recipes || []));
  for (const group of lib.groups || []) {
    recipes.push(...(group.recipes || []));
  }
  return recipes;
}

// The index may carry a two-level `families` tree (family -> domain book ->
// chapter -> recipe). Flat `books` data is wrapped in a single unnamed family,
// so the browser renders every recipe from either shape.
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

function treeStateKey(kind, id) {
  return `sim-cookbook:${kind}:${id}`;
}

function readTreeState(kind, id) {
  try {
    return localStorage.getItem(treeStateKey(kind, id));
  } catch (_error) {
    return null;
  }
}

function writeTreeState(kind, id, open) {
  try {
    localStorage.setItem(treeStateKey(kind, id), open ? "1" : "0");
  } catch (_error) {
    // Keep browsing functional when storage is unavailable.
  }
}

function setDetailsOpen(details, kind, id, defaultOpen, forceOpen = false) {
  const saved = readTreeState(kind, id);
  details.open = forceOpen || (saved == null ? defaultOpen : saved === "1");
  details.addEventListener("toggle", () => {
    if (state.visibleIds !== null) return;
    writeTreeState(kind, id, details.open);
  });
}

// Render the cookbook tree. The modern API provides `libs` as the top level;
// older payloads fall back to the compatibility family -> domain -> chapter
// shape below. A group with no visible recipe (search filtered) is skipped.
function renderTree() {
  tree.replaceChildren();
  if (!state.recipes.length && !state.libs.length) {
    tree.append(empty("No recipes loaded."));
    return;
  }
  if (state.hasLibTree) {
    renderLibTree();
  } else {
    renderFamilyTree();
  }
}

function renderLibTree() {
  const visible = state.visibleIds;
  const searching = visible !== null;
  let count = 0;
  for (const lib of state.libs) {
    const libEl = document.createElement("details");
    libEl.className = "lib";
    setDetailsOpen(libEl, "lib", lib.id, true, searching);
    const summary = document.createElement("summary");
    summary.className = "lib-title";
    summary.textContent = lib.title || lib.id;
    libEl.append(summary);
    let libCount = 0;
    const directRecipes = visibleRecipes(lib.recipes || [], visible);
    if (directRecipes.length) {
      const list = document.createElement("ul");
      list.className = "recipe-list lib-recipes";
      for (const recipe of directRecipes) {
        count += 1;
        libCount += 1;
        list.append(recipeItem(recipe));
      }
      libEl.append(list);
    }
    for (const group of lib.groups || []) {
      const recipes = visibleRecipes(group.recipes || [], visible);
      if (!recipes.length) continue;
      const groupEl = document.createElement("details");
      groupEl.className = "group";
      setDetailsOpen(groupEl, "group", `${lib.id}/${group.name}`, false, searching);
      const groupTitle = document.createElement("summary");
      groupTitle.className = "group-title";
      groupTitle.textContent = group.title || group.name;
      groupEl.append(groupTitle);
      const list = document.createElement("ul");
      list.className = "recipe-list";
      for (const recipe of recipes) {
        count += 1;
        libCount += 1;
        list.append(recipeItem(recipe));
      }
      groupEl.append(list);
      libEl.append(groupEl);
    }
    if (libCount) {
      tree.append(libEl);
    }
  }
  if (!count) {
    tree.append(empty("No recipes found."));
  }
}

function visibleRecipes(recipes, visible) {
  return (recipes || []).filter((recipe) => !visible || visible.has(recipe.id));
}

function renderFamilyTree() {
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
        const recipes = visibleRecipes(chapter.recipes || [], visible);
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
  if (isLifecycleRecipe(recipe)) {
    button.classList.add("lifecycle-recipe");
  }
  button.append(recipeBadge(recipe));
  const label = document.createElement("span");
  label.className = "recipe-label";
  label.textContent = recipe.title;
  button.append(label);
  button.dataset.recipeId = recipe.id;
  if (recipe.action) button.dataset.recipeAction = recipe.action;
  if (recipe.lib) button.dataset.recipeLib = recipe.lib;
  if (recipe.loaded !== undefined) button.dataset.recipeLoaded = String(recipe.loaded);
  if (recipe.id === state.selected) {
    button.classList.add("selected");
    button.setAttribute("aria-current", "page");
  }
  button.addEventListener("click", () => selectRecipe(recipe.id));
  item.append(button);
  return item;
}

function recipeBadge(recipe) {
  const badge = document.createElement("span");
  if (isLifecycleRecipe(recipe)) {
    badge.className = `badge lifecycle ${recipe.action}`;
    badge.textContent = recipe.action;
    badge.title = `${actionLabel(recipe)} ${recipe.lib || "library"} (${loadedLabel(recipe)})`;
    return badge;
  }
  const runnable = isRunnable(recipe);
  badge.className = `badge ${runnable ? "runnable" : "descriptor"}`;
  badge.textContent = runnable ? "run" : "doc";
  badge.title = runnable
    ? "Runnable: computes on click"
    : "Descriptor: documented, not run in the sandbox";
  return badge;
}

function isRunnable(recipe) {
  return recipe.runnable !== false && !(recipe.tags || []).includes("sandbox-descriptor");
}

function isLifecycleRecipe(recipe) {
  return recipe.action === "load" || recipe.action === "unload";
}

function actionLabel(recipe) {
  if (recipe.action === "load") return "Load";
  if (recipe.action === "unload") return "Unload";
  return isRunnable(recipe) ? "Run" : "View";
}

function actionProgress(recipe) {
  if (recipe.action === "load") return "Loading.";
  if (recipe.action === "unload") return "Unloading.";
  return "Running.";
}

function loadedLabel(recipe) {
  if (recipe.loaded === true) return "loaded";
  if (recipe.loaded === false) return "not loaded";
  return "load state unknown";
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
  title.append(recipeBadge(recipe), " ", recipe.title);
  const purpose = document.createElement("div");
  purpose.className = "purpose";
  purpose.append(...renderPurpose(recipe.purpose));
  const lifecycle = lifecycleMeta(recipe);
  const actions = document.createElement("div");
  actions.className = "recipe-actions";
  const copy = document.createElement("button");
  copy.type = "button";
  copy.textContent = "Copy";
  const run = document.createElement("button");
  run.type = "button";
  run.textContent = actionLabel(recipe);
  if (isLifecycleRecipe(recipe)) {
    run.classList.add("lifecycle-action", recipe.action);
  }
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
    try {
      await runSelectedRecipe(recipe, results);
    } catch (error) {
      results.textContent = error.message;
    }
  });
  pane.append(title, purpose);
  if (lifecycle) pane.append(lifecycle);
  pane.append(actions, setup, results, footer);
}

function lifecycleMeta(recipe) {
  if (!recipe.lib && recipe.loaded === undefined && !recipe.action) return null;
  const meta = document.createElement("dl");
  meta.className = "lifecycle-meta";
  appendMeta(meta, "Library", recipe.lib || "unknown");
  appendMeta(meta, "State", loadedLabel(recipe));
  if (recipe.action) appendMeta(meta, "Action", actionLabel(recipe));
  return meta;
}

function appendMeta(list, term, value) {
  const dt = document.createElement("dt");
  dt.textContent = term;
  const dd = document.createElement("dd");
  dd.textContent = value;
  list.append(dt, dd);
}

async function runSelectedRecipe(recipe, results) {
  results.textContent = actionProgress(recipe);
  const outcome = await fetchJson(
    `${apiRoot}/recipe/${encodeURIComponent(recipe.id)}/run`,
    { method: "POST" },
  );
  renderRunResults(results, outcome);
  if (isLifecycleRecipe(recipe) && outcome.ok) {
    await loadCookbook({
      lib: recipe.lib,
      action: recipe.action === "unload" ? "load" : null,
    });
  }
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
