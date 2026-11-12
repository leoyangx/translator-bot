const $ = (sel) => document.querySelector(sel);
const $$ = (sel) => Array.from(document.querySelectorAll(sel));

let S = { config: null, languages: [], version: "" };
let srcLang = "auto";
let tgtLang = "zh";
let pickerFor = "src";
let lastDirection = null;
let recentLangs = [];
let currentView = "text";

const THEME_KEY = "tb-theme";
const RECENT_KEY = "tb-recent-langs";

async function invoke(method, payload) {
  if (!(window.__TAURI__ && window.__TAURI__.core)) throw new Error("请在桌面程序中运行");
  return await window.__TAURI__.core.invoke("ui_call", { method, args: payload === undefined ? [] : [payload] });
}

function nativeInvoke(cmd) {
  if (!(window.__TAURI__ && window.__TAURI__.core)) throw new Error("请在桌面程序中运行");
  return window.__TAURI__.core.invoke(cmd);
}

function toast(message, kind) {
  const el = $("#toast");
  el.textContent = message;
  el.className = "toast show" + (kind === "err" ? " err" : "");
  clearTimeout(el._timer);
  el._timer = setTimeout(() => (el.className = "toast" + (kind === "err" ? " err" : "")), 2200);
}

function zhError(e) {
  const text = (e && (e.message || e.toString())) || "未知错误";
  if (/timeout|超时/i.test(text)) return "连接超时，请检查网络或代理";
  return text;
}

/* ---------- 主题 ---------- */
function applyTheme(theme) {
  document.documentElement.dataset.theme = theme;
  $("#btn-theme").textContent = theme === "dark" ? "☀" : "🌙";
  localStorage.setItem(THEME_KEY, theme);
}
function initTheme() {
  applyTheme(localStorage.getItem(THEME_KEY) || "light");
  $("#btn-theme").addEventListener("click", () =>
    applyTheme(document.documentElement.dataset.theme === "dark" ? "light" : "dark")
  );
}

/* ---------- 视图切换 ---------- */
function switchView(name) {
  currentView = name;
  $$(".tab").forEach((t) => t.classList.toggle("active", t.dataset.view === name));
  $$(".view").forEach((v) => v.classList.toggle("active", v.id === "view-" + name));
  $("#langbar").hidden = name !== "text";
  if (name === "glossary") refreshGlossary();
  if (name === "history") refreshHistory();
  if (name === "text") $("#input").focus();
}

/* ---------- 语言 ---------- */
function langName(code) {
  if (code === "auto") return "自动检测";
  const found = (S.languages || []).find((l) => l.code === code);
  return found ? found.name : code;
}
function updateLangLabels() {
  $("#src-label").textContent = langName(srcLang);
  $("#tgt-label").textContent = langName(tgtLang);
}
function renderLangList(keyword) {
  const kw = (keyword || "").trim().toLowerCase();
  const list = $("#lang-list");
  const rows = [];

  if (pickerFor === "src") {
    rows.push(`<div class="lang-item ${srcLang === "auto" ? "active" : ""}" data-code="auto">自动检测</div>`);
  }

  const matches = (S.languages || []).filter(
    (l) => !kw || l.name.toLowerCase().includes(kw) || l.code.toLowerCase().includes(kw)
  );

  function item(code) {
    const active = (pickerFor === "src" ? srcLang : tgtLang) === code;
    return `<div class="lang-item ${active ? "active" : ""}" data-code="${code}">${langName(code)}</div>`;
  }

  const recent = recentLangs.filter((c) => matches.some((l) => l.code === c));
  if (recent.length && !kw) {
    rows.push(`<div class="lang-caption">最近使用</div>`);
    recent.forEach((code) => rows.push(item(code)));
    rows.push(`<div class="lang-caption">全部语言</div>`);
  }
  matches.forEach((l) => {
    if (recent.includes(l.code) && !kw) return;
    rows.push(item(l.code));
  });

  list.innerHTML = rows.join("");
  list.querySelectorAll(".lang-item").forEach((el) =>
    el.addEventListener("click", () => chooseLang(el.dataset.code))
  );
}
function openLangPanel(which) {
  pickerFor = which;
  $("#lang-panel").hidden = false;
  $("#lang-search").value = "";
  renderLangList("");
  $("#lang-search").focus();
}
function chooseLang(code) {
  if (pickerFor === "src") {
    if (code !== "auto" && code === tgtLang) tgtLang = srcLang === "auto" ? "zh" : srcLang;
    srcLang = code;
  } else {
    if (code === srcLang) srcLang = "auto";
    tgtLang = code;
  }
  pushRecent(code);
  updateLangLabels();
  $("#lang-panel").hidden = true;
  if ($("#input").value.trim()) doTranslate();
}
function pushRecent(code) {
  if (!code || code === "auto") return;
  recentLangs = [code, ...recentLangs.filter((c) => c !== code)].slice(0, 6);
  localStorage.setItem(RECENT_KEY, JSON.stringify(recentLangs));
}

/* ---------- 启动 ---------- */
async function boot() {
  initTheme();
  recentLangs = JSON.parse(localStorage.getItem(RECENT_KEY) || "[]");
  try {
    S = await invoke("get_state");
  } catch (e) {
    toast("读取配置失败：" + zhError(e), "err");
    return;
  }
  S.config = S.config || {};
  S.languages = S.languages || [];

  const cfg = S.config || {};
  const mine = (cfg.languages && cfg.languages.mine) || "zh";
  const partner = (cfg.languages && cfg.languages.partners && cfg.languages.partners[0]) || "en";
  srcLang = "auto";
  tgtLang = mine === "zh" ? partner : mine;
  updateLangLabels();
  renderSettings();
  bind();
  $("#input").focus();
}

function renderSettings() {
  const cfg = S.config || {};
  const model = (cfg.models || [])[0] || {};
  $("#f-base").value = model.base_url || "";
  $("#f-key").value = model.api_key || "";
  $("#f-model").value = model.model || "";
  $("#f-proxy").value = cfg.proxy || "";
  $("#f-tone").value = (cfg.features && cfg.features.default_tone) || "natural";
  $("#version-label").textContent = "v" + (S.version || "");
}

/* ---------- 事件 ---------- */
function bind() {
  $$(".tab").forEach((tab) => tab.addEventListener("click", () => switchView(tab.dataset.view)));

  $("#btn-menu").addEventListener("click", (e) => {
    e.stopPropagation();
    $("#menu-pop").hidden = !$("#menu-pop").hidden;
  });
  document.addEventListener("click", () => ($("#menu-pop").hidden = true));
  $("#menu-pop").addEventListener("click", (e) => e.stopPropagation());
  $$("[data-menu]").forEach((item) =>
    item.addEventListener("click", async () => {
      $("#menu-pop").hidden = true;
      const action = item.dataset.menu;
      if (action === "settings") $("#drawer-settings").hidden = false;
      if (action === "data") await invoke("open_folder", "data");
      if (action === "about") toast(`翻译 v${S.version} · 本地运行，数据不出本机`);
      if (action === "quit") nativeInvoke("plugin:process|exit").catch(() => invoke("quit_app"));
    })
  );

  $("#btn-src").addEventListener("click", () => openLangPanel("src"));
  $("#btn-tgt").addEventListener("click", () => openLangPanel("tgt"));
  $("#lang-search").addEventListener("input", (e) => renderLangList(e.target.value));
  $$("[data-lang-close]").forEach((btn) =>
    btn.addEventListener("click", () => ($("#lang-panel").hidden = true))
  );

  $("#btn-swap").addEventListener("click", swapAndTranslate);
  $("#btn-translate").addEventListener("click", () => doTranslate());
  $("#input").addEventListener("input", () => {
    $("#char-count").textContent = `${$("#input").value.length} / 6000 字`;
  });
  $("#input").addEventListener("keydown", (e) => {
    if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      doTranslate();
    }
  });

  document.addEventListener("keydown", (e) => {
    if (e.ctrlKey && e.shiftKey && e.key.toLowerCase() === "s") {
      e.preventDefault();
      swapAndTranslate();
    }
  });

  $("#btn-clear").addEventListener("click", () => {
    $("#input").value = "";
    $("#char-count").textContent = "0 / 6000 字";
    $("#output").classList.add("hidden");
    $("#output-placeholder").classList.remove("hidden");
    $("#output-error").classList.add("hidden");
    $("#meta").textContent = "";
    lastDirection = null;
    $("#input").focus();
  });

  $("#btn-copy").addEventListener("click", async () => {
    const text = $("#output").textContent;
    if (!text) return;
    await navigator.clipboard.writeText(text);
    toast("已复制");
  });

  $$("[data-close]").forEach((btn) =>
    btn.addEventListener("click", () => (btn.closest(".drawer").hidden = true))
  );

  $("#btn-glossary-add").addEventListener("click", addGlossary);
  $("#btn-history-clear").addEventListener("click", async () => {
    await invoke("history_clear");
    $("#history-list").innerHTML = `<p class="hint">还没有翻译记录。</p>`;
    toast("历史已清空");
  });
  $("#btn-test-model").addEventListener("click", testModel);
  $("#btn-save").addEventListener("click", saveSettings);
  $("#btn-check-update").addEventListener("click", checkUpdate);

  document.addEventListener("keydown", (e) => {
    if (e.key !== "Escape") return;
    if (!$("#lang-panel").hidden) return ($("#lang-panel").hidden = true);
    if (!$("#menu-pop").hidden) return ($("#menu-pop").hidden = true);
    const open = $$(".drawer").find((d) => !d.hidden);
    if (open) open.hidden = true;
  });

  document.addEventListener("click", (e) => {
    if ($("#lang-panel").hidden) return;
    if (e.target.closest("#lang-panel") || e.target.closest("#btn-src") || e.target.closest("#btn-tgt")) return;
    $("#lang-panel").hidden = true;
  });
}

/* ---------- 翻译 ---------- */
async function doTranslate() {
  const text = $("#input").value.trim();
  if (!text) return toast("先输入要翻译的内容", "err");

  const btn = $("#btn-translate");
  btn.disabled = true;
  $("#output-placeholder").classList.add("hidden");
  $("#output-error").classList.add("hidden");

  try {
    const res = await invoke("translate", { text, src: srcLang, tgt: tgtLang, tone: currentTone() });
    if (!res || !res.ok) {
      $("#output").classList.add("hidden");
      $("#output-error").textContent = (res && res.message) || "翻译失败";
      $("#output-error").classList.remove("hidden");
      $("#meta").textContent = "";
      return;
    }
    $("#output").textContent = res.translation;
    $("#output").classList.remove("hidden");
    lastDirection = { src: res.src, tgt: res.tgt };
    const parts = [`${res.src_name} → ${res.tgt_name}`, `${res.elapsed_ms} ms`];
    if (res.cached) parts.push("缓存命中");
    $("#meta").textContent = parts.join(" · ");
  } catch (e) {
    $("#output").classList.add("hidden");
    $("#output-error").textContent = zhError(e);
    $("#output-error").classList.remove("hidden");
  } finally {
    btn.disabled = false;
  }
}

/** ⇄ 互换：译文搬进输入框 + 语言对调 + 重新翻译 */
async function swapAndTranslate() {
  const output = $("#output").textContent.trim();
  const hasResult = !!output && !$("#output").classList.contains("hidden");

  let newSrc = tgtLang;
  let newTgt = srcLang;
  if (lastDirection) {
    newSrc = lastDirection.tgt;
    newTgt = lastDirection.src;
  }
  if (newSrc === "auto") newSrc = lastDirection ? lastDirection.tgt : "zh";

  srcLang = newSrc;
  tgtLang = newTgt;
  updateLangLabels();

  if (!hasResult) {
    if ($("#input").value.trim()) doTranslate();
    return;
  }

  $("#input").value = output;
  $("#input").dispatchEvent(new Event("input"));
  lastDirection = null;
  $("#output").classList.add("hidden");
  $("#output-placeholder").classList.remove("hidden");
  switchView("text");
  await doTranslate();
}

function currentTone() {
  return ($("#f-tone") && $("#f-tone").value) || "natural";
}

/* ---------- 术语表 ---------- */
async function refreshGlossary() {
  const rows = (await invoke("glossary_list")) || [];
  const list = $("#glossary-list");
  if (!rows.length) {
    list.innerHTML = `<p class="hint">还没有条目。加一条试试：原文 <code>Jian Zhan</code> → 译文 <code>建盏</code>。</p>`;
    return;
  }
  list.innerHTML = rows
    .map(
      (r) => `<div class="glossary-row">
        <span class="src">${escapeHtml(r.source)}</span>
        <span class="arrow">→</span>
        <span class="tgt">${escapeHtml(r.target)}</span>
        <button data-source="${escapeAttr(r.source)}" title="删除">✕</button>
      </div>`
    )
    .join("");
  list.querySelectorAll("button[data-source]").forEach((btn) =>
    btn.addEventListener("click", async () => {
      await invoke("glossary_remove", btn.dataset.source);
      await refreshGlossary();
    })
  );
}

async function addGlossary() {
  const source = $("#glossary-source").value.trim();
  const target = $("#glossary-target").value.trim();
  if (!source || !target) return toast("原文和译文都要填", "err");
  const res = await invoke("glossary_save", { source, target });
  if (res && res.ok) {
    $("#glossary-source").value = "";
    $("#glossary-target").value = "";
    await refreshGlossary();
    toast("已添加");
  } else {
    toast((res && res.message) || "添加失败", "err");
  }
}

/* ---------- 历史 ---------- */
async function refreshHistory() {
  const rows = (await invoke("history_list", 500)) || [];
  const list = $("#history-list");
  if (!rows.length) {
    list.innerHTML = `<p class="hint">还没有翻译记录。</p>`;
    return;
  }
  list.innerHTML = rows
    .map(
      (r) => `<div class="history-row" data-source="${escapeAttr(r.source)}" data-src="${r.src}" data-tgt="${r.tgt}">
        <div class="htop"><span>${langName(r.src)} → ${langName(r.tgt)}${r.cached ? " · 缓存" : ""}</span><span>${formatTime(r.ts)}</span></div>
        <div class="hsrc">${escapeHtml(r.source)}</div>
        <div class="hout">${escapeHtml(r.output)}</div>
      </div>`
    )
    .join("");
  list.querySelectorAll(".history-row").forEach((row) =>
    row.addEventListener("click", () => {
      $("#input").value = row.dataset.source;
      $("#input").dispatchEvent(new Event("input"));
      srcLang = row.dataset.src;
      tgtLang = row.dataset.tgt;
      lastDirection = { src: row.dataset.src, tgt: row.dataset.tgt };
      updateLangLabels();
      switchView("text");
      doTranslate();
    })
  );
}

function formatTime(ts) {
  const d = new Date(ts * 1000);
  const pad = (n) => String(n).padStart(2, "0");
  return `${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

/* ---------- 设置 ---------- */
async function testModel() {
  const out = $("#test-result");
  out.textContent = "测试中…";
  const res = await invoke("test_model", {
    base_url: $("#f-base").value.trim(),
    api_key: $("#f-key").value.trim(),
    model: $("#f-model").value.trim(),
    proxy: $("#f-proxy").value.trim(),
  });
  out.textContent = (res && res.message) || (res && res.ok ? "连接正常" : "测试失败");
}

async function saveSettings() {
  const cfg = JSON.parse(JSON.stringify(S.config || {}));
  cfg.models = cfg.models || [];
  const first = cfg.models[0] || { id: "default", name: "默认模型", kind: "text", default: true };
  first.base_url = $("#f-base").value.trim();
  first.api_key = $("#f-key").value.trim();
  first.model = $("#f-model").value.trim();
  first.default = true;
  cfg.models = [first];
  cfg.proxy = $("#f-proxy").value.trim();
  cfg.features = cfg.features || {};
  cfg.features.default_tone = $("#f-tone").value;

  const res = await invoke("save_config", cfg);
  if (res && res.ok) {
    S.config = cfg;
    toast("设置已保存");
    $("#drawer-settings").hidden = true;
  } else {
    toast((res && res.message) || "保存失败", "err");
  }
}

async function checkUpdate() {
  const out = $("#update-result");
  out.textContent = "检查中…";
  try {
    const info = await nativeInvoke("update_check");
    if (info && info.available) {
      out.textContent = `发现新版本 v${info.version}`;
      if (confirm(`发现新版本 v${info.version}，现在下载并更新吗？`)) {
        out.textContent = "正在下载…";
        await nativeInvoke("update_install");
      }
    } else {
      out.textContent = (info && info.error) || "已经是最新版本";
    }
  } catch (e) {
    out.textContent = zhError(e);
  }
}

/* ---------- 工具 ---------- */
function escapeHtml(text) {
  return String(text).replace(/[&<>]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;" }[c]));
}
function escapeAttr(text) {
  return escapeHtml(text).replace(/"/g, "&quot;");
}

boot();
