// Deterministic browser renderer for the domain-neutral `scene/heatmap` row.
//
// The Scene producer owns detector and reduction semantics. This module only
// validates the browser-facing row, maps its finite scalars to named palette
// data, paints a bounded canvas, and exposes the exact caller-prepared cells for
// accessible inspection. It never fetches an image or resamples the Scene.
"use strict";

const MAX_HEATMAP_CELLS = 1024 * 1024;
const MAX_CSS_WIDTH = 760;
const MAX_CSS_HEIGHT = 640;
const MAX_CANVAS_EDGE = 4096;
const MAX_CANVAS_PIXELS = 4 * 1024 * 1024;
const MAX_DEVICE_PIXEL_RATIO = 4;
const MASK_COLOR = "#202832";
const MASK_HATCH_COLOR = "#aeb8c2";

function palette(stops, cyclic = false) {
  return Object.freeze({
    cyclic,
    stops: Object.freeze(stops.map(([at, color]) => Object.freeze([at, color]))),
  });
}

// Palette endpoints and cyclic wrapping are defined exactly once. The same
// records drive canvas colors, the visual legend, and exact-output tests.
export const HEATMAP_PALETTES = Object.freeze({
  viridis: palette([
    [0, "#440154"],
    [0.25, "#3b528b"],
    [0.5, "#21918c"],
    [0.75, "#5ec962"],
    [1, "#fde725"],
  ]),
  "blue-red": palette([
    [0, "#2166ac"],
    [0.5, "#f7f7f7"],
    [1, "#b2182b"],
  ]),
  "cyclic-phase": palette([
    [0, "#3b4cc0"],
    [0.25, "#2ca25f"],
    [0.5, "#f6e84a"],
    [0.75, "#d73027"],
    [1, "#3b4cc0"],
  ], true),
});

function element(doc, tag, className) {
  const node = doc.createElement(tag);
  node.className = className;
  return node;
}

function clamp(value, min, max) {
  return Math.min(max, Math.max(min, value));
}

function byteToHex(value) {
  return Math.round(value).toString(16).padStart(2, "0");
}

function rgb(color) {
  return [
    Number.parseInt(color.slice(1, 3), 16),
    Number.parseInt(color.slice(3, 5), 16),
    Number.parseInt(color.slice(5, 7), 16),
  ];
}

function interpolationUnit(definition, unit) {
  if (definition.cyclic) {
    const wrapped = unit % 1;
    return wrapped < 0 ? wrapped + 1 : wrapped;
  }
  return clamp(unit, 0, 1);
}

/// Return the exact hexadecimal color for a named palette and normalized unit.
export function paletteColor(name, unit) {
  const definition = HEATMAP_PALETTES[name];
  if (!definition) {
    throw new RangeError(`unknown heatmap palette '${String(name)}'`);
  }
  const numericUnit = Number(unit);
  if (!Number.isFinite(numericUnit)) {
    throw new TypeError("heatmap palette unit must be finite");
  }
  const normalized = interpolationUnit(definition, numericUnit);
  const stops = definition.stops;
  let upper = 1;
  while (upper < stops.length && normalized > stops[upper][0]) upper += 1;
  const [lowAt, lowColor] = stops[upper - 1];
  const [highAt, highColor] = stops[Math.min(upper, stops.length - 1)];
  if (normalized === lowAt || highAt === lowAt) return lowColor;
  if (normalized === highAt) return highColor;
  const amount = (normalized - lowAt) / (highAt - lowAt);
  const low = rgb(lowColor);
  const high = rgb(highColor);
  return `#${low.map((component, index) =>
    byteToHex(component + ((high[index] - component) * amount))).join("")}`;
}

/// Return the CSS legend gradient derived from the same palette stop records.
export function paletteGradient(name) {
  const definition = HEATMAP_PALETTES[name];
  if (!definition) {
    throw new RangeError(`unknown heatmap palette '${String(name)}'`);
  }
  const stops = definition.stops
    .map(([at, color]) => `${color} ${at * 100}%`)
    .join(", ");
  return `linear-gradient(90deg, ${stops})`;
}

function validationError(message) {
  return { ok: false, message };
}

/// Validate the bounded browser-facing subset of a `scene/heatmap` row.
export function validateHeatmapNode(node) {
  if (!node || node.kind !== "scene/heatmap") {
    return validationError("expected scene/heatmap");
  }
  const rows = Number(node.rows);
  const cols = Number(node.cols);
  if (!Number.isSafeInteger(rows) || !Number.isSafeInteger(cols) || rows <= 0 || cols <= 0) {
    return validationError("heatmap rows and cols must be positive safe integers");
  }
  const cells = rows * cols;
  if (!Number.isSafeInteger(cells) || cells > MAX_HEATMAP_CELLS) {
    return validationError(`heatmap cell count exceeds ${MAX_HEATMAP_CELLS}`);
  }
  if (!Array.isArray(node.values) || node.values.length !== cells) {
    return validationError(`heatmap values must contain exactly ${cells} cells`);
  }
  if (!Array.isArray(node.valid) || node.valid.length !== cells) {
    return validationError(`heatmap valid mask must contain exactly ${cells} cells`);
  }
  if (node.values.some((value) => !Number.isFinite(Number(value)))) {
    return validationError("heatmap values must be finite numbers");
  }
  if (node.valid.some((value) => typeof value !== "boolean")) {
    return validationError("heatmap valid mask must contain booleans");
  }
  const min = Number(node.min);
  const max = Number(node.max);
  if (!Number.isFinite(min) || !Number.isFinite(max) || min > max) {
    return validationError("heatmap range must be finite with min <= max");
  }
  const paletteName = String(node.palette || "");
  if (!Object.hasOwn(HEATMAP_PALETTES, paletteName)) {
    return validationError(`unknown heatmap palette '${paletteName}'`);
  }
  const label = typeof node.label === "string" ? node.label.trim() : "";
  const detector = typeof node.detector === "string" ? node.detector.trim() : "";
  if (!label || !detector) {
    return validationError("heatmap label and detector must be non-empty");
  }
  if (node.advisory != null && (typeof node.advisory !== "string" || !node.advisory.trim())) {
    return validationError("heatmap advisory must be non-empty when present");
  }
  if (node.footprint && Number(node.footprint.cells) !== cells) {
    return validationError(`heatmap footprint must report ${cells} cells`);
  }
  return {
    ok: true,
    rows,
    cols,
    cells,
    min,
    max,
    palette: paletteName,
    label,
    detector,
  };
}

function formatScalar(value) {
  if (Object.is(value, -0)) return "-0";
  return Number(value).toString();
}

/// Return the stable accessible inspection record for one row-major cell.
export function inspectHeatmapCell(node, row, col) {
  const checked = validateHeatmapNode(node);
  if (!checked.ok) throw new TypeError(checked.message);
  const boundedRow = clamp(Math.trunc(Number(row) || 0), 0, checked.rows - 1);
  const boundedCol = clamp(Math.trunc(Number(col) || 0), 0, checked.cols - 1);
  const index = (boundedRow * checked.cols) + boundedCol;
  const valid = node.valid[index];
  const value = Number(node.values[index]);
  const prefix = `Cell row ${boundedRow + 1}, column ${boundedCol + 1}`;
  return Object.freeze({
    row: boundedRow,
    col: boundedCol,
    index,
    valid,
    value,
    text: valid ? `${prefix}: ${formatScalar(value)}.` : `${prefix}: masked.`,
  });
}

function normalizedValue(value, min, max) {
  return max === min ? 0.5 : (value - min) / (max - min);
}

function requestedDpr(value) {
  const dpr = Number(value);
  return Number.isFinite(dpr) ? clamp(dpr, 1, MAX_DEVICE_PIXEL_RATIO) : 1;
}

/// Calculate bounded logical and backing-store dimensions for the canvas.
export function heatmapCanvasGeometry(rows, cols, availableWidth, devicePixelRatio = 1) {
  const widthLimit = Number.isFinite(Number(availableWidth)) && Number(availableWidth) > 0
    ? Math.floor(Number(availableWidth))
    : Math.min(MAX_CSS_WIDTH, Math.max(1, cols * 24));
  let cssWidth = clamp(widthLimit, 1, MAX_CSS_WIDTH);
  let cssHeight = Math.max(1, Math.round(cssWidth * (rows / cols)));
  if (cssHeight > MAX_CSS_HEIGHT) {
    cssHeight = MAX_CSS_HEIGHT;
    cssWidth = Math.max(1, Math.round(cssHeight * (cols / rows)));
  }

  const desiredDpr = requestedDpr(devicePixelRatio);
  const pixelBound = Math.sqrt(MAX_CANVAS_PIXELS / (cssWidth * cssHeight));
  const edgeBound = Math.min(MAX_CANVAS_EDGE / cssWidth, MAX_CANVAS_EDGE / cssHeight);
  const effectiveDpr = Math.min(desiredDpr, pixelBound, edgeBound);
  return Object.freeze({
    cssWidth,
    cssHeight,
    width: Math.max(1, Math.floor(cssWidth * effectiveDpr)),
    height: Math.max(1, Math.floor(cssHeight * effectiveDpr)),
    requestedDpr: desiredDpr,
    effectiveDpr,
  });
}

function maskedHatch(context, x, y, width, height) {
  context.fillStyle = MASK_COLOR;
  context.fillRect(x, y, width, height);
  if (width <= 0 || height <= 0) return;
  context.save();
  context.beginPath();
  context.rect(x, y, width, height);
  context.clip();
  context.strokeStyle = MASK_HATCH_COLOR;
  context.lineWidth = 1;
  context.beginPath();
  const stride = Math.max(3, Math.min(8, Math.floor(Math.min(width, height) / 2) || 3));
  for (let offset = -height; offset < width; offset += stride) {
    context.moveTo(x + offset, y + height);
    context.lineTo(x + offset + height, y);
  }
  context.stroke();
  context.restore();
}

/// Paint caller-prepared row-major cells into a bounded canvas backing store.
export function paintHeatmapCanvas(canvas, node, options = {}) {
  const checked = validateHeatmapNode(node);
  if (!checked.ok) throw new TypeError(checked.message);
  const geometry = heatmapCanvasGeometry(
    checked.rows,
    checked.cols,
    options.availableWidth,
    options.devicePixelRatio,
  );
  canvas.width = geometry.width;
  canvas.height = geometry.height;
  canvas.style.width = `${geometry.cssWidth}px`;
  canvas.style.height = `${geometry.cssHeight}px`;
  canvas.dataset.cssWidth = String(geometry.cssWidth);
  canvas.dataset.cssHeight = String(geometry.cssHeight);
  canvas.dataset.pixelWidth = String(geometry.width);
  canvas.dataset.pixelHeight = String(geometry.height);
  canvas.dataset.requestedDpr = String(geometry.requestedDpr);
  canvas.dataset.effectiveDpr = String(geometry.effectiveDpr);

  const context = canvas.getContext("2d", { alpha: false });
  if (!context) throw new Error("2D canvas rendering is unavailable");
  context.imageSmoothingEnabled = false;
  context.clearRect(0, 0, geometry.width, geometry.height);
  for (let row = 0; row < checked.rows; row += 1) {
    const y = Math.floor((row * geometry.height) / checked.rows);
    const bottom = Math.floor(((row + 1) * geometry.height) / checked.rows);
    for (let col = 0; col < checked.cols; col += 1) {
      const x = Math.floor((col * geometry.width) / checked.cols);
      const right = Math.floor(((col + 1) * geometry.width) / checked.cols);
      const width = right - x;
      const height = bottom - y;
      if (width === 0 || height === 0) continue;
      const index = (row * checked.cols) + col;
      if (!node.valid[index]) {
        maskedHatch(context, x, y, width, height);
        continue;
      }
      context.fillStyle = paletteColor(
        checked.palette,
        normalizedValue(Number(node.values[index]), checked.min, checked.max),
      );
      context.fillRect(x, y, width, height);
    }
  }
  return geometry;
}

function heatmapError(doc, message) {
  const error = element(doc, "div", "scene-heatmap-error");
  error.setAttribute("role", "alert");
  error.textContent = `Heatmap unavailable: ${message}`;
  return error;
}

function availableCanvasWidth(viewport, checked) {
  if (typeof viewport.getBoundingClientRect === "function") {
    const width = Number(viewport.getBoundingClientRect().width);
    if (Number.isFinite(width) && width > 0) return width;
  }
  return Math.min(MAX_CSS_WIDTH, Math.max(1, checked.cols * 24));
}

function cellFromPointer(canvas, checked, event) {
  if (typeof canvas.getBoundingClientRect !== "function") return null;
  const rect = canvas.getBoundingClientRect();
  if (!(rect.width > 0) || !(rect.height > 0)) return null;
  const x = Number(event.clientX) - rect.left;
  const y = Number(event.clientY) - rect.top;
  if (!Number.isFinite(x) || !Number.isFinite(y) || x < 0 || y < 0
      || x >= rect.width || y >= rect.height) {
    return null;
  }
  return {
    row: Math.min(checked.rows - 1, Math.floor((y / rect.height) * checked.rows)),
    col: Math.min(checked.cols - 1, Math.floor((x / rect.width) * checked.cols)),
  };
}

function keyboardCell(current, checked, event) {
  switch (event.key) {
    case "ArrowUp":
      return { row: current.row - 1, col: current.col };
    case "ArrowDown":
      return { row: current.row + 1, col: current.col };
    case "ArrowLeft":
      return { row: current.row, col: current.col - 1 };
    case "ArrowRight":
      return { row: current.row, col: current.col + 1 };
    case "Home":
      return { row: current.row, col: 0 };
    case "End":
      return { row: current.row, col: checked.cols - 1 };
    default:
      return null;
  }
}

/// Render one accessible heatmap through the existing Scene interpreter.
export function renderHeatmap(doc, node) {
  const checked = validateHeatmapNode(node);
  if (!checked.ok) return heatmapError(doc, checked.message);

  const root = element(doc, "figure", "scene-heatmap");
  root.dataset.layout = "flow";
  root.dataset.palette = checked.palette;
  root.setAttribute("role", "group");

  const masked = node.valid.reduce((count, valid) => count + (valid ? 0 : 1), 0);
  const summary = element(doc, "figcaption", "scene-heatmap-summary");
  summary.textContent = `${checked.label}: ${checked.rows} rows by ${checked.cols} columns; `
    + `range ${formatScalar(checked.min)} to ${formatScalar(checked.max)}; `
    + `${masked} masked; ${checked.palette} palette.`;
  root.setAttribute("aria-label", summary.textContent);
  root.appendChild(summary);

  const viewport = element(doc, "div", "scene-heatmap-viewport");
  const canvas = element(doc, "canvas", "scene-heatmap-canvas");
  canvas.setAttribute("role", "img");
  canvas.setAttribute("tabindex", "0");
  viewport.appendChild(canvas);
  root.appendChild(viewport);

  const legend = element(doc, "div", "scene-heatmap-legend");
  legend.setAttribute(
    "aria-label",
    `${checked.palette} palette from ${formatScalar(checked.min)} to ${formatScalar(checked.max)}`,
  );
  const legendMin = element(doc, "span", "scene-heatmap-legend-min");
  legendMin.textContent = formatScalar(checked.min);
  const legendBar = element(doc, "span", "scene-heatmap-legend-bar");
  legendBar.style.background = paletteGradient(checked.palette);
  legendBar.setAttribute("aria-hidden", "true");
  const legendMax = element(doc, "span", "scene-heatmap-legend-max");
  legendMax.textContent = formatScalar(checked.max);
  const maskKey = element(doc, "span", "scene-heatmap-mask-key");
  maskKey.textContent = "masked";
  legend.appendChild(legendMin);
  legend.appendChild(legendBar);
  legend.appendChild(legendMax);
  legend.appendChild(maskKey);
  root.appendChild(legend);

  const inspector = element(doc, "output", "scene-heatmap-inspector");
  inspector.setAttribute("role", "status");
  inspector.setAttribute("aria-live", "polite");
  inspector.setAttribute("aria-atomic", "true");
  root.appendChild(inspector);

  const metadata = element(doc, "div", "scene-heatmap-metadata");
  const detector = element(doc, "div", "scene-heatmap-detector");
  detector.textContent = `Detector: ${checked.detector}`;
  metadata.appendChild(detector);
  if (node.advisory != null) {
    const advisory = element(doc, "div", "scene-heatmap-advisory");
    advisory.setAttribute("role", "note");
    advisory.textContent = String(node.advisory);
    metadata.appendChild(advisory);
  }
  root.appendChild(metadata);

  let current = { row: 0, col: 0 };
  const inspect = (row, col) => {
    const record = inspectHeatmapCell(node, row, col);
    current = record;
    inspector.textContent = record.text;
    canvas.dataset.activeRow = String(record.row);
    canvas.dataset.activeCol = String(record.col);
    canvas.setAttribute("aria-label", `${summary.textContent} ${record.text}`);
  };
  inspect(0, 0);

  canvas.addEventListener("pointermove", (event) => {
    const cell = cellFromPointer(canvas, checked, event);
    if (cell) inspect(cell.row, cell.col);
  });
  canvas.addEventListener("pointerdown", (event) => {
    const cell = cellFromPointer(canvas, checked, event);
    if (cell) inspect(cell.row, cell.col);
  });
  canvas.addEventListener("keydown", (event) => {
    const cell = keyboardCell(current, checked, event);
    if (!cell) return;
    event.preventDefault();
    inspect(cell.row, cell.col);
  });

  const view = doc.defaultView || globalThis;
  const repaint = (width) => paintHeatmapCanvas(canvas, node, {
    availableWidth: width,
    devicePixelRatio: view.devicePixelRatio,
  });
  repaint(availableCanvasWidth(viewport, checked));
  if (typeof view.ResizeObserver === "function") {
    const observer = new view.ResizeObserver((entries) => {
      const entry = entries && entries[0];
      const width = entry && entry.contentRect && Number(entry.contentRect.width);
      repaint(Number.isFinite(width) && width > 0 ? width : availableCanvasWidth(viewport, checked));
    });
    observer.observe(viewport);
    root.disposeScene = () => observer.disconnect();
  }
  return root;
}
