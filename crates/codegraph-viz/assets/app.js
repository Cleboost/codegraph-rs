/* global ForceGraph, ForceGraph3D */

const KIND_COLORS = {
  function: '#5eead4',
  method: '#2dd4bf',
  class: '#818cf8',
  struct: '#a78bfa',
  interface: '#c084fc',
  trait: '#e879f9',
  module: '#f472b6',
  variable: '#94a3b8',
  constant: '#fbbf24',
  enum: '#fb923c',
  import: '#64748b',
  component: '#38bdf8',
  default: '#64748b',
};

const EDGE_COLORS = {
  calls: '#5eead4',
  imports: '#818cf8',
  extends: '#c084fc',
  implements: '#a78bfa',
  references: '#94a3b8',
  default: '#3d465c',
};

const state = {
  boot: { depth: 2 },
  graph: { nodes: [], edges: [], seed: null, truncated: false },
  selectedId: null,
  hoverId: null,
  graph2d: null,
  graph3d: null,
  activeView: 'table',
  paused: false,
  rotate3d: false,
  rotateRaf: null,
};

// ── API ──────────────────────────────────────────────

async function api(path) {
  const res = await fetch(path);
  if (!res.ok) {
    const err = await res.json().catch(() => ({}));
    throw new Error(err.error || res.statusText);
  }
  return res.json();
}

function setLoading(on) {
  document.getElementById('loading').classList.toggle('hidden', !on);
}

// ── Graph data ───────────────────────────────────────

function nodeColor(n) {
  return KIND_COLORS[n.kind] || KIND_COLORS.default;
}

function kindTag(kind) {
  const c = nodeColor({ kind });
  return `<span class="kind-tag" style="background:${c}22;color:${c};border:1px solid ${c}44">${escapeHtml(kind)}</span>`;
}

function graphData() {
  const allNodes = [];
  const seen = new Set();
  const add = (n) => {
    if (!n || seen.has(n.id)) return;
    seen.add(n.id);
    allNodes.push(n);
  };
  if (state.graph.seed) add(state.graph.seed);
  state.graph.nodes.forEach(add);

  const links = state.graph.edges.map((e) => ({
    source: e.from,
    target: e.to,
    kind: e.kind,
    color: EDGE_COLORS[e.kind] || EDGE_COLORS.default,
  }));

  return {
    nodes: allNodes.map((n) => ({
      id: n.id,
      name: n.name,
      kind: n.kind,
      val: nodeVal(n),
      color: nodeColor(n),
      raw: n,
    })),
    links,
  };
}

function nodeVal(n) {
  const base = n.kind === 'function' || n.kind === 'method' ? 2.5 : n.kind === 'class' ? 2 : 1.2;
  if (n.id === state.selectedId) return base * 2.2;
  if (n.id === state.hoverId) return base * 1.5;
  return base;
}

/** 3D: no hover resize — recreating spheres every frame kills FPS. */
function nodeVal3d(n) {
  const base = n.kind === 'function' || n.kind === 'method' ? 2.2 : n.kind === 'class' ? 1.8 : 1;
  if (n.id === state.selectedId) return base * 1.6;
  return base;
}

function graph3dPerfTier(nodeCount, linkCount) {
  if (nodeCount > 200 || linkCount > 500) return 'heavy';
  if (nodeCount > 80 || linkCount > 200) return 'medium';
  return 'light';
}

function applyGraph3dPerf(fg, nodeCount, linkCount) {
  const tier = graph3dPerfTier(nodeCount, linkCount);
  const particles = tier === 'light' && linkCount < 120 ? 1 : 0;
  const resolution = tier === 'heavy' ? 5 : tier === 'medium' ? 7 : 9;
  fg.linkDirectionalParticles(particles)
    .linkDirectionalArrowLength(0)
    .nodeResolution(resolution)
    .d3AlphaDecay(tier === 'heavy' ? 0.04 : 0.028)
    .warmupTicks(tier === 'heavy' ? 30 : 50)
    .cooldownTicks(tier === 'heavy' ? 20 : 40);
  const renderer = fg.renderer();
  if (renderer) {
    const pr = tier === 'heavy' ? 1 : Math.min(window.devicePixelRatio, 1.35);
    renderer.setPixelRatio(pr);
  }
  state._3dTier = tier;
}

let hover3dRaf = null;
function schedule3dLinkRefresh() {
  if (state._3dTier === 'heavy') return;
  if (hover3dRaf) return;
  hover3dRaf = requestAnimationFrame(() => {
    hover3dRaf = null;
    if (state.graph3d) state.graph3d.linkColor(linkColorFn3d);
  });
}

function pause3dPhysicsIfIdle() {
  if (state.graph3d && state.activeView === 'graph3d' && !state.paused && !state.rotate3d) {
    state.graph3d.pauseAnimation();
    state._3dPhysicsDone = true;
  }
}

function linkEndpoints(l) {
  return {
    src: typeof l.source === 'object' ? l.source.id : l.source,
    tgt: typeof l.target === 'object' ? l.target.id : l.target,
  };
}

function linkColorFn(l) {
  const hi = state.selectedId || state.hoverId;
  if (!hi) return l.color + '66';
  const { src, tgt } = linkEndpoints(l);
  return src === hi || tgt === hi ? l.color : l.color + '22';
}

function linkWidthFn(l) {
  const hi = state.selectedId || state.hoverId;
  if (!hi) return 1;
  const { src, tgt } = linkEndpoints(l);
  return src === hi || tgt === hi ? 2 : 0.4;
}

function linkColorFn3d(l) {
  if (state._3dTier === 'heavy') return l.color + '55';
  const hi = state.selectedId || state.hoverId;
  if (!hi) return l.color + '77';
  const { src, tgt } = linkEndpoints(l);
  return src === hi || tgt === hi ? l.color : l.color + '28';
}

function updateGraphCounts() {
  const data = graphData();
  const perf =
    state.activeView === 'graph3d' && state._3dTier && state._3dTier !== 'light'
      ? ` · ${state._3dTier} perf`
      : '';
  const text = `${data.nodes.length} nodes · ${data.links.length} edges${perf}`;
  document.getElementById('graph-count').textContent = text;
  document.getElementById('graph-count').classList.toggle('hidden', !data.nodes.length);
  document.getElementById('hud-stats').textContent = text;
}

function renderLegend() {
  const kinds = [...new Set(graphData().nodes.map((n) => n.kind))].sort();
  const el = document.getElementById('legend');
  if (!kinds.length) {
    el.innerHTML = '';
    return;
  }
  el.innerHTML = kinds.map((k) => kindTag(k)).join('');
}

// ── Table ────────────────────────────────────────────

function renderTable() {
  const el = document.getElementById('table-view');
  const data = graphData();
  if (!data.nodes.length) {
    el.innerHTML = '<p class="muted empty-hint">No nodes yet — try Search or Load</p>';
    return;
  }
  const sorted = [...data.nodes].sort((a, b) => a.name.localeCompare(b.name));
  const rows = sorted
    .map(
      (n) => `<tr data-id="${n.id}" class="${state.selectedId === n.id ? 'selected' : ''}">
        <td>${kindTag(n.kind)}${escapeHtml(n.name)}</td>
        <td class="muted">${escapeHtml(shortPath(n.raw.file || ''))}</td>
      </tr>`
    )
    .join('');
  el.innerHTML = `<table><thead><tr><th>Name</th><th>File</th></tr></thead><tbody>${rows}</tbody></table>`;
  el.querySelectorAll('tbody tr').forEach((tr) => {
    tr.addEventListener('click', () => selectNode(Number(tr.dataset.id)));
  });
}

function shortPath(p) {
  const parts = String(p).split(/[/\\]/);
  return parts.length > 3 ? '…/' + parts.slice(-2).join('/') : p;
}

// ── 2D graph ─────────────────────────────────────────

function initGraph2d() {
  const el = document.getElementById('graph2d-view');
  const fg = ForceGraph()(el)
    .backgroundColor('rgba(0,0,0,0)')
    .nodeLabel((n) => `<div style="padding:4px 8px;font-size:12px"><b>${n.name}</b><br/><span style="opacity:.7">${n.kind}</span></div>`)
    .nodeColor((n) => n.color)
    .nodeVal((n) => n.val)
    .nodeRelSize(5)
    .linkColor(linkColorFn)
    .linkWidth(linkWidthFn)
    .linkDirectionalArrowLength(4)
    .linkDirectionalArrowRelPos(1)
    .linkDirectionalParticles(2)
    .linkDirectionalParticleWidth(2)
    .linkDirectionalParticleSpeed(0.004)
    .d3AlphaDecay(0.015)
    .d3VelocityDecay(0.25)
    .warmupTicks(80)
    .cooldownTicks(120)
    .enableNodeDrag(true)
    .onNodeClick((n) => selectNode(n.id))
    .onNodeHover((n) => {
      state.hoverId = n ? n.id : null;
      el.style.cursor = n ? 'pointer' : null;
      refreshGraphStyles();
    })
    .nodeCanvasObjectMode((n) => (n.id === state.selectedId ? 'after' : undefined))
    .nodeCanvasObject((n, ctx, globalScale) => {
      if (n.id !== state.selectedId) return;
      const r = Math.sqrt(n.val) * 5 + 4;
      ctx.beginPath();
      ctx.arc(n.x, n.y, r / globalScale, 0, 2 * Math.PI);
      ctx.fillStyle = n.color + '33';
      ctx.fill();
      ctx.strokeStyle = n.color;
      ctx.lineWidth = 2 / globalScale;
      ctx.stroke();
    });

  state.graph2d = fg;
  resizeGraphs();
}

function renderGraph2d() {
  const data = graphData();
  if (!state.graph2d) initGraph2d();
  state.graph2d.graphData(data);
  if (!state.paused) {
    setTimeout(() => {
      if (state.activeView === 'graph2d' && state.graph2d) {
        state.graph2d.zoomToFit(500, 60);
      }
    }, 600);
  }
}

// ── 3D graph ─────────────────────────────────────────

function initGraph3d() {
  const el = document.getElementById('graph3d-view');
  const fg = ForceGraph3D()(el)
    .backgroundColor('rgba(0,0,0,0)')
    .showNavInfo(false)
    .enableNodeDrag(false)
    .nodeLabel((n) => `${n.kind}: ${n.name}`)
    .nodeColor((n) => n.color)
    .nodeVal(nodeVal3d)
    .nodeOpacity(0.9)
    .nodeResolution(7)
    .linkColor(linkColorFn3d)
    .linkWidth(0.35)
    .linkOpacity(0.55)
    .linkDirectionalParticles(0)
    .linkDirectionalArrowLength(0)
    .d3AlphaDecay(0.028)
    .d3VelocityDecay(0.35)
    .warmupTicks(40)
    .cooldownTicks(30)
    .onNodeClick((n) => selectNode(n.id))
    .onNodeHover((n) => {
      state.hoverId = n ? n.id : null;
      el.style.cursor = n ? 'pointer' : null;
      schedule3dLinkRefresh();
    })
    .onEngineStop(() => {
      pause3dPhysicsIfIdle();
      if (state.activeView === 'graph3d' && !state._didFit3d) {
        state._didFit3d = true;
        fg.zoomToFit(500, 80);
      }
    });

  const renderer = fg.renderer();
  if (renderer) renderer.setPixelRatio(Math.min(window.devicePixelRatio, 1.35));

  state.graph3d = fg;
  resizeGraphs();
}

function refreshGraphStyles() {
  if (state.graph2d) {
    state.graph2d
      .nodeVal((n) => nodeVal(n))
      .linkColor(linkColorFn)
      .linkWidth(linkWidthFn)
      .nodeCanvasObjectMode((n) => (n.id === state.selectedId ? 'after' : undefined));
  }
  if (state.graph3d) {
    state.graph3d.nodeVal(nodeVal3d).linkColor(linkColorFn3d);
  }
}

function renderGraph3d() {
  const data = graphData();
  if (!state.graph3d) initGraph3d();
  applyGraph3dPerf(state.graph3d, data.nodes.length, data.links.length);
  state._didFit3d = false;
  state._3dPhysicsDone = false;
  state.graph3d.resumeAnimation();
  state.graph3d.graphData(data);
}

function focusSelected3d() {
  if (!state.graph3d || !state.selectedId) return;
  const data = state.graph3d.graphData();
  const node = data.nodes.find((n) => n.id === state.selectedId);
  if (!node || node.x == null) return;
  const dist = 120;
  state.graph3d.cameraPosition(
    { x: node.x, y: node.y, z: node.z + dist },
    node,
    1200
  );
}

function toggleRotate3d() {
  const btn = document.getElementById('btn-rotate');
  if (state.rotate3d) {
    if (state.rotateRaf) cancelAnimationFrame(state.rotateRaf);
    state.rotateRaf = null;
    state.rotate3d = false;
    btn.classList.remove('active');
    pause3dPhysicsIfIdle();
    return;
  }
  state.rotate3d = true;
  btn.classList.add('active');
  if (state.graph3d && state._3dPhysicsDone) state.graph3d.resumeAnimation();
  let angle = 0;
  let last = performance.now();
  const spin = (now) => {
    if (!state.rotate3d || !state.graph3d) return;
    const dt = Math.min(now - last, 50);
    last = now;
    angle += dt * 0.00035;
    const dist = 280;
    state.graph3d.cameraPosition({
      x: dist * Math.sin(angle),
      y: dist * 0.35,
      z: dist * Math.cos(angle),
    });
    state.rotateRaf = requestAnimationFrame(spin);
  };
  state.rotateRaf = requestAnimationFrame(spin);
}

// ── Detail panel ─────────────────────────────────────

function renderDetail() {
  const content = document.getElementById('detail-content');
  const actions = document.getElementById('detail-actions');
  const id = state.selectedId;
  if (!id) {
    content.innerHTML = '<p class="muted empty-hint">Click a node to inspect</p>';
    actions.classList.add('hidden');
    return;
  }
  const n = graphData().nodes.find((x) => x.id === id)?.raw;
  if (!n) {
    content.innerHTML = '<p class="muted empty-hint">Loading…</p>';
    api(`/api/node/${id}`).then(showDetail);
    return;
  }
  showDetail(n);
}

function showDetail(n) {
  const content = document.getElementById('detail-content');
  document.getElementById('detail-actions').classList.remove('hidden');
  content.innerHTML = `
    ${kindTag(n.kind)}
    <div class="node-title">${escapeHtml(n.name)}</div>
    ${n.qualified_name ? `<p class="muted">${escapeHtml(n.qualified_name)}</p>` : ''}
    <code>${escapeHtml(n.file)}:${n.start_line}</code>
    ${n.signature ? `<code>${escapeHtml(n.signature)}</code>` : ''}
    ${n.docstring ? `<p style="margin-top:.5rem">${escapeHtml(n.docstring)}</p>` : ''}
  `;
}

function selectNode(id) {
  state.selectedId = id;
  refreshGraphStyles();
  renderAll();
  if (state.activeView === 'graph3d') focusSelected3d();
}

// ── Render orchestration ─────────────────────────────

function renderAll() {
  document.getElementById('truncated-badge').classList.toggle('hidden', !state.graph.truncated);
  updateGraphCounts();
  renderLegend();
  if (state.activeView === 'table') renderTable();
  if (state.activeView === 'graph2d') renderGraph2d();
  if (state.activeView === 'graph3d') renderGraph3d();
  renderDetail();
}

function resizeGraphs() {
  const panel = document.getElementById('panel-left');
  const w = panel.clientWidth;
  const h = panel.clientHeight;
  if (state.graph2d) state.graph2d.width(w).height(h);
  if (state.graph3d) state.graph3d.width(w).height(h);
}

// ── Data loading ─────────────────────────────────────

async function loadSubgraph(opts = {}) {
  const depth = Number(document.getElementById('depth-input').value) || state.boot.depth || 2;
  const params = new URLSearchParams({ depth: String(depth) });

  if (opts.seed != null) params.set('seed', String(opts.seed));
  else if (opts.query) params.set('query', opts.query);
  else if (opts.prefix !== undefined) params.set('prefix', opts.prefix);
  else if (state.selectedId != null) params.set('seed', String(state.selectedId));
  else {
    const q = document.getElementById('search-input').value.trim();
    if (q) params.set('query', q);
    else if (state.boot.target) params.set('query', state.boot.target);
    else if (state.boot.prefix) params.set('prefix', state.boot.prefix);
  }

  setLoading(true);
  try {
    const data = await api(`/api/subgraph?${params}`);
    state.graph = data;
    if (data.seed) state.selectedId = data.seed.id;
    renderAll();
  } catch (err) {
    document.getElementById('detail-content').innerHTML =
      `<p class="muted empty-hint">${escapeHtml(err.message)}</p>`;
  } finally {
    setLoading(false);
  }
}

async function loadStatus() {
  try {
    const s = await api('/api/status');
    document.getElementById('status-bar').textContent =
      `${s.files.toLocaleString()} files · ${s.nodes.toLocaleString()} nodes · ${s.edges.toLocaleString()} edges · schema v${s.schema_version}`;
  } catch (_) {}
}

async function doSearch() {
  const q = document.getElementById('search-input').value.trim();
  if (!q) {
    document.getElementById('search-results').classList.add('hidden');
    return;
  }
  const hits = await api(`/api/search?q=${encodeURIComponent(q)}&limit=30`);
  const panel = document.getElementById('search-results');
  if (!hits.length) {
    panel.innerHTML = '<div class="item muted">No results</div>';
    panel.classList.remove('hidden');
    return;
  }
  panel.innerHTML = hits
    .map(
      (h) =>
        `<div class="item" data-id="${h.id}">
          ${kindTag(h.kind)}${escapeHtml(h.name)}
          <span class="muted"> — ${escapeHtml(shortPath(h.file))}</span>
        </div>`
    )
    .join('');
  panel.classList.remove('hidden');
  panel.querySelectorAll('.item').forEach((el) => {
    el.addEventListener('click', () => {
      panel.classList.add('hidden');
      if (el.dataset.id) loadSubgraph({ seed: Number(el.dataset.id) });
    });
  });
}

function mergeHits(hits, rootId) {
  const ids = new Set(state.graph.nodes.map((n) => n.id));
  if (state.graph.seed) ids.add(state.graph.seed.id);
  hits.nodes.forEach((n) => {
    if (!ids.has(n.id)) {
      state.graph.nodes.push(n);
      ids.add(n.id);
    }
  });
  const edgeKey = (e) => `${e.from}-${e.to}-${e.kind}`;
  const keys = new Set(state.graph.edges.map(edgeKey));
  hits.edges.forEach((e) => {
    if (!keys.has(edgeKey(e))) state.graph.edges.push(e);
  });
  state.graph.truncated = state.graph.truncated || hits.truncated;
  state.selectedId = rootId;
  renderAll();
  if (state.activeView === 'graph2d' && state.graph2d) {
    setTimeout(() => state.graph2d.zoomToFit(400, 60), 400);
  }
  if (state.activeView === 'graph3d' && state.graph3d) {
    const data = graphData();
    applyGraph3dPerf(state.graph3d, data.nodes.length, data.links.length);
    state._3dPhysicsDone = false;
    state.graph3d.resumeAnimation();
  }
}

function escapeHtml(s) {
  return String(s)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

// ── View switching ───────────────────────────────────

function setView(view) {
  state.activeView = view;
  const isGraph = view === 'graph2d' || view === 'graph3d';

  document.querySelectorAll('#view-tabs button').forEach((b) => {
    b.classList.toggle('active', b.dataset.view === view);
  });
  document.querySelectorAll('.view').forEach((v) => v.classList.remove('active'));
  const map = { table: 'table-view', graph2d: 'graph2d-view', graph3d: 'graph3d-view' };
  document.getElementById(map[view]).classList.add('active');

  document.getElementById('graph-hud').classList.toggle('hidden', !isGraph);
  document.getElementById('btn-rotate').classList.toggle('hidden', view !== 'graph3d');

  if (view !== 'graph3d' && state.rotate3d) toggleRotate3d();

  renderAll();
  requestAnimationFrame(resizeGraphs);
}

function fitActiveGraph() {
  if (state.activeView === 'graph2d' && state.graph2d) {
    state.graph2d.zoomToFit(400, 60);
  } else if (state.activeView === 'graph3d' && state.graph3d) {
    state.graph3d.zoomToFit(500, 80);
  }
}

function togglePause() {
  const btn = document.getElementById('btn-pause');
  state.paused = !state.paused;
  btn.textContent = state.paused ? '▶ Resume' : '⏸ Pause';
  btn.classList.toggle('active', state.paused);
  if (state.graph2d) {
    if (state.paused) state.graph2d.pauseAnimation();
    else state.graph2d.resumeAnimation();
  }
  if (state.graph3d) {
    if (state.paused) {
      state.graph3d.pauseAnimation();
    } else {
      state.graph3d.resumeAnimation();
      if (state._3dPhysicsDone && !state.rotate3d) {
        setTimeout(pause3dPhysicsIfIdle, 2500);
      }
    }
  }
}

// ── Events ───────────────────────────────────────────

document.getElementById('view-tabs').addEventListener('click', (e) => {
  const btn = e.target.closest('button');
  if (btn) setView(btn.dataset.view);
});

document.getElementById('search-input').addEventListener('keydown', (e) => {
  if (e.key === 'Enter') doSearch();
});
document.getElementById('search-input').addEventListener('input', () => {
  clearTimeout(state._searchTimer);
  state._searchTimer = setTimeout(doSearch, 280);
});
document.addEventListener('click', (e) => {
  if (!e.target.closest('#search-bar') && !e.target.closest('#search-results')) {
    document.getElementById('search-results').classList.add('hidden');
  }
});

document.getElementById('reload-btn').addEventListener('click', () => loadSubgraph());
document.getElementById('btn-fit').addEventListener('click', fitActiveGraph);
document.getElementById('btn-pause').addEventListener('click', togglePause);
document.getElementById('btn-rotate').addEventListener('click', toggleRotate3d);
document.getElementById('btn-focus').addEventListener('click', () => {
  if (state.activeView === 'graph3d') focusSelected3d();
  else if (state.graph2d && state.selectedId) {
    const n = state.graph2d.graphData().nodes.find((x) => x.id === state.selectedId);
    if (n) state.graph2d.centerAt(n.x, n.y, 800);
  }
});

document.getElementById('btn-callers').addEventListener('click', async () => {
  if (!state.selectedId) return;
  const depth = Number(document.getElementById('depth-input').value) || 1;
  mergeHits(await api(`/api/callers/${state.selectedId}?depth=${depth}`), state.selectedId);
});
document.getElementById('btn-callees').addEventListener('click', async () => {
  if (!state.selectedId) return;
  const depth = Number(document.getElementById('depth-input').value) || 1;
  mergeHits(await api(`/api/callees/${state.selectedId}?depth=${depth}`), state.selectedId);
});
document.getElementById('btn-expand').addEventListener('click', async () => {
  if (!state.selectedId) return;
  mergeHits(await api(`/api/neighbors/${state.selectedId}?depth=1`), state.selectedId);
});

new ResizeObserver(resizeGraphs).observe(document.getElementById('panel-left'));

async function init() {
  try {
    state.boot = await api('/api/boot');
    if (state.boot.depth) document.getElementById('depth-input').value = state.boot.depth;
    if (state.boot.target) document.getElementById('search-input').value = state.boot.target;
  } catch (_) {
    state.boot = { depth: 2 };
  }
  await loadStatus();
  await loadSubgraph();
}

init();
