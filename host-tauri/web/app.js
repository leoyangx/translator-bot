const $ = (sel) => document.querySelector(sel);
const $$ = (sel) => Array.from(document.querySelectorAll(sel));

let S = { config: null, languages: [], version: "" };
let translating = false;

async function invoke(method, payload) {
  if (!(window.__TAURI__ && window.__TAURI__.core)) {
    throw new Error("请在桌面程序中运行");
  }
  return await window.__TAURI__.core.invoke("ui_call", { method, args: payload === undefined ? [] : [payload] });
}

function toast(message, kind) {
  const el = $("#toast");
  el.textContent = message;
  el.className = "toast show" + (kind === "err" ? " err" : "");
  clearTimeout(el._timer);
  el._timer = setTimeout(() => {
    el.className = "toast" + (kind === "err" ? " err" : "");
  }, 2400);
}

function zhError(e) {
  const text = (e && (e.message || e.toString())) || "未知错误";
  if (text.includes("timeout") || text.includes("超时")) return "连接超时，请检查网络或代理";
  return text;
}

/* ---------- 初始化 ---------- */

async function boot() {
  try {
    S = await invoke("get_state");
  } catch (e) {
    toast("读取配置失败：" + zhError(e), "err");
    return;
  }
  S.config = S.config || {};
  renderLanguages();
  renderSettings();
  bind();
  $("#input").focus();
}

function renderLanguages() {
  const langs = S.languages || [];
  const tgt = $("#tgt-lang");
  tgt.innerHTML = langs
    .map((l) => `<option value="${l.code}">${l.name}</option>`)
    .join("");
  const src = $("#src-lang");
  src.innerHTML =
    `<option value="auto">自动检测</option>` +
    langs.map((l) => `<option value="${l.code}">${l.name}</option>`).join("");

  const cfg = S.config || {};
  const mine = (cfg.languages && cfg.languages.mine) || "zh";
  const partner = (cfg.languages && cfg.languages.partners && cfg.languages.partners[0]) || "en";
  const swap = mine === "zh";
  src.value = "auto";
  tgt.value = swap ? partner : mine;
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
  setTone($("#f-tone").value);
}

/* ---------- 交互 ---------- */

function bind() {
  $("#btn-translate").addEventListener("click", () => doTranslate());
  $("#input").addEventListener("input", () => {
    const n = $("#input").value.length;
    $("#char-count").textContent = `${n} / 6000 字`;
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
    $("#meta").textContent = "";
    $("#input").focus();
  });

  $("#btn-copy").addEventListener("click", async () => {
    const text = $("#output").textContent;
    if (!text) return;
    await navigator.clipboard.writeText(text);
    toast("已复制");
  });

  $("#btn-swap").addEventListener("click", () => {
    const src = $("#src-lang");
    const tgt = $("#tgt-lang");
    if (src.value === "auto") {
      const current = tgt.value;
      const others = (S.languages || []).filter((l) => l.code !== current);
      tgt.value = others.length ? others[0].code : current;
      return;
    }
    const a = src.value;
    src.value = tgt.value;
    tgt.value = a;
  });

  $$("#tone-group .tone").forEach((btn) =>
    btn.addEventListener("click", () => {
      setTone(btn.dataset.tone);
      if ($("#input").value.trim()) doTranslate();
    })
  );

  $("#btn-glossary").addEventListener("click", openGlossary);
  $("#btn-history").addEventListener("click", openHistory);
  $("#btn-settings").addEventListener("click", () => openDrawer("drawer-settings"));

  $$("[data-close]").forEach((btn) =>
    btn.addEventListener("click", () => btn.closest(".drawer").hidden = true)
  );

  $("#btn-glossary-add").addEventListener("click", addGlossary);
  $("#btn-history-clear").addEventListener("click", async () => {
    await invoke("history_clear");
    $("#history-list").innerHTML = "";
    toast("历史已清空");
  });

  $("#btn-test-model").addEventListener("click", testModel);
  $("#btn-save").addEventListener("click", saveSettings);

  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape") {
      const open = $$(".drawer").find((d) => !d.hidden);
      if (open) open.hidden = true;
    }
  });
}

function setTone(tone) {
  $$("#tone-group .tone").forEach((btn) =>
    btn.classList.toggle("active", btn.dataset.tone === tone)
  );
}

function currentTone() {
  const active = $("#tone-group .tone.active");
  return active ? active.dataset.tone : "natural";
}

async function doTranslate() {
  const text = $("#input").value.trim();
  if (!text) {
    toast("先输入要翻译的内容", "err");
    return;
  }
  if (translating) return;
  translating = true;
  const btn = $("#btn-translate");
  btn.disabled = true;
  $("#output-placeholder").classList.add("hidden");
  $("#output-error").classList.add("hidden");

  const started = Date.now();
  try {
    const res = await invoke("translate", {
      text,
      src: $("#src-lang").value,
      tgt: $("#tgt-lang").value,
      tone: currentTone(),
    });
    if (!res || !res.ok) {
      $("#output").classList.add("hidden");
      $("#output-error").textContent = (res && res.message) || "翻译失败";
      $("#output-error").classList.remove("hidden");
      $("#meta").textContent = "";
      return;
    }
    $("#output-error").classList.add("hidden");
    $("#output").textContent = res.translation;
    $("#output").classList.remove("hidden");
    const parts = [`${res.src_name} → ${res.tgt_name}`, `${res.elapsed_ms || Date.now() - started} ms`];
    if (res.cached) parts.push("缓存命中");
    $("#meta").textContent = parts.join(" · ");
  } catch (e) {
    $("#output").classList.add("hidden");
    $("#output-error").textContent = zhError(e);
    $("#output-error").classList.remove("hidden");
  } finally {
    translating = false;
    btn.disabled = false;
  }
}

/* ---------- 抽屉 ---------- */

function openDrawer(id) {
  $$(".drawer").forEach((d) => (d.hidden = d.id !== id));
  const drawer = $("#" + id);
  drawer.hidden = false;
}

async function openGlossary() {
  openDrawer("drawer-glossary");
  await refreshGlossary();
}

async function refreshGlossary() {
  const rows = await invoke("glossary_list");
  const list = $("#glossary-list");
  if (!rows || !rows.length) {
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
  if (!source || !target) {
    toast("原文和译文都要填", "err");
    return;
  }
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
  const rows = await invoke("history_list", 100);
  const list = $("#history-list");
  if (!rows || !rows.length) {
    list.innerHTML = `<p class="hint">还没有翻译记录。</p>`;
    return;
  }
  list.innerHTML = rows
    .map(
      (r) => `<div class="history-row" data-source="${escapeAttr(r.source)}">
        <div class="htop"><span>${r.src} → ${r.tgt}${r.cached ? " · 缓存" : ""}</span><span>${formatTime(r.ts)}</span></div>
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
  out.textContent = res && res.ok ? res.message : (res && res.message) || "测试失败";
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
