const $ = (sel) => document.querySelector(sel);
const $$ = (sel) => Array.from(document.querySelectorAll(sel));

let S = { config: null, languages: [], version: "" };
let srcLang = "auto";
let tgtLang = "zh";
let pickerFor = "src";
let recentLangs = [];

const THEME_KEY = "tb-theme";
const RECENT_KEY = "tb-recent-langs";

async function invoke(method, payload) {
  if (!(window.__TAURI__ && window.__TAURI__.core)) throw new Error("请在桌面程序中运行");
  return await window.__TAURI__.core.invoke("ui_call", { method, args: payload === undefined ? [] : [payload] });
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
  const saved = localStorage.getItem(THEME_KEY) || "light";
  applyTheme(saved);
  $("#btn-theme").addEventListener("click", () => {
    applyTheme(document.documentElement.dataset.theme === "dark" ? "light" : "dark");
  });
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

  function item(code) {
    const active = (pickerFor === "src" ? srcLang : tgtLang) === code;
    return `<div class="lang-item ${active ? "active" : ""}" data-code="${code}">${langName(code)}</div>`;
  }

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

/* ---------- 初始化 ---------- */

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

/* ---------- 交互 ---------- */

function bind() {
  $("#btn-src").addEventListener("click", () => openLangPanel("src"));
  $("#btn-tgt").addEventListener("click", () => openLangPanel("tgt"));
  $("#lang-search").addEventListener("input", (e) => renderLangList(e.target.value));
  $$("[data-lang-close]").forEach((btn) =>
    btn.addEventListener("click", () => ($("#lang-panel").hidden = true))
  );

  $("#btn-swap").addEventListener("click", () => {
    const from = srcLang === "auto" ? "zh" : srcLang;
    const to = tgtLang;
    srcLang = to;
    tgtLang = from;
    updateLangLabels();
    if ($("#input").value.trim()) doTranslate();
  });

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

  $("#btn-clear").addEventListener("click", () => {
    $("#input").value = "";
    $("#char-count").textContent = "0 / 6000 字";
    $("#output").classList.add("hidden");
    $("#output-placeholder").classList.remove("hidden");
    $("#output-error").classList.add("hidden");
    $("#meta").textContent = "";
    $("#input").focus();
  });

  $("#btn-copy").addEventListener("click", async () => {
    const text = $("#output").textContent;
    if (!text) return;
    await navigator.clipboard.writeText(text);
    toast("已复制");
  });

  $("#btn-glossary").addEventListener("click", openGlossary);
  $("#btn-history").addEventListener("click", openHistory);
  $("#btn-settings").addEventListener("click", () => openDrawer("drawer-settings"));

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

  document.addEventListener("keydown", (e) => {
    if (e.key !== "Escape") return;
    if (!$("#lang-panel").hidden) return ($("#lang-panel").hidden = true);
    const open = $$(".drawer").find((d) => !d.hidden);
    if (open) open.hidden = true;
  });

  document.addEventListener("click", (e) => {
    if ($("#lang-panel").hidden) return;
    if (e.target.closest("#lang-panel") || e.target.closest("#btn-src") || e.target.closest("#btn-tgt")) return;
    $("#lang-panel").hidden = true;
  });
}

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

function currentTone() {
  return ($("#f-tone") && $("#f-tone").value) || "natural";
}

/* ---------- 抽屉 ---------- */

function openDrawer(id) {
  $$(".drawer").forEach((d) => (d.hidden = d.id !== id));
  $("#" + id).hidden = false;
}

async function openGlossary() {
  openDrawer("drawer-glossary");
  await refreshGlossary();
}

async function refreshGlossary() {
  const rows = (await invoke("glossary_list")) || [];
  const list = $("#glossary-list");
  if (!rows.length) {
    list.innerHTML = `<p class="hint">还没有条目。添加后翻译会优先使用这里的译法。</p>`;
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

async function openHistory() {
  openDrawer("drawer-history");
  const rows = (await invoke("history_list", 100)) || [];
  const list = $("#history-list");
  if (!rows.length) {
    list.innerHTML = `<p class="hint">还没有翻译记录。</p>`;
    return;
  }
  list.innerHTML = rows
    .map(
      (r) => `<div class="history-row" data-source="${escapeAttr(r.source)}">
        <div class="htop"><span>${langName(r.src)} → ${langName(r.tgt)}${r.cached ? " · 缓存" : ""}</span><span>${formatTime(r.ts)}</span></div>
        <div class="hsrc">${escapeHtml(r.source)}</div>
      </div>`
    )
    .join("");
  list.querySelectorAll(".history-row").forEach((row) =>
    row.addEventListener("click", () => {
      $("#input").value = row.dataset.source;
      $("#input").dispatchEvent(new Event("input"));
      $("#drawer-history").hidden = true;
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

/* ---------- 工具 ---------- */

function escapeHtml(text) {
  return String(text).replace(/[&<>]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;" }[c]));
}
function escapeAttr(text) {
  return escapeHtml(text).replace(/"/g, "&quot;");
}

boot();
