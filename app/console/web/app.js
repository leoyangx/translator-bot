const $ = (sel, root = document) => root.querySelector(sel);
const $$ = (sel, root = document) => Array.from(root.querySelectorAll(sel));

const DBG_VISIBLE = location.search.includes("debug");

function dbg(msg) {
  let el = DBG_VISIBLE ? document.getElementById("dbg") : null;
  if (!DBG_VISIBLE) {
    try {
      if (window.__TAURI__ && window.__TAURI__.core) {
        window.__TAURI__.core.invoke("front_log", { message: msg });
      }
    } catch (e) {
      /* ignore */
    }
    return;
  }
  if (!el) {
    el = document.createElement("div");
    el.id = "dbg";
    el.style.cssText =
      "position:fixed;left:0;bottom:0;right:0;z-index:9999;background:rgba(0,0,0,.85);color:#ff8080;font:11px/1.5 monospace;padding:6px;white-space:pre-wrap;max-height:40vh;overflow:auto";
    document.body.appendChild(el);
  }
  el.textContent += msg + "\n";
  try {
    if (window.__TAURI__ && window.__TAURI__.core) {
      window.__TAURI__.core.invoke("front_log", { message: msg });
    }
  } catch (e) {
    /* ignore */
  }
}

async function rpc(method, ...args) {
  if (window.__TAURI__ && window.__TAURI__.core) {
    try {
      return await window.__TAURI__.core.invoke("backend_call", { method, args });
    } catch (e) {
      dbg("invoke " + method + " -> " + (e && (e.message || e)));
      throw e;
    }
  }
  const res = await fetch(`/api/${method}`, {
    method: "POST",
    headers: { "Content-Type": "application/json", "X-Console": "1" },
    body: JSON.stringify(args),
  });
  return await res.json();
}

const api = () => {
  if (window.pywebview && window.pywebview.api) return window.pywebview.api;
  return new Proxy({}, { get: (_, method) => (...args) => rpc(method, ...args) });
};

dbg("boot: tauri=" + (!!(window.__TAURI__ && window.__TAURI__.core)));
window.addEventListener("error", (e) => dbg("ERR " + e.message + " @" + e.filename + ":" + e.lineno));
window.addEventListener("unhandledrejection", (e) => dbg("REJ " + (e.reason && (e.reason.message || e.reason))));

let S = { config: null, languages: [], tones: [], defaults: {} };
let booted = false;

function bootReady() {
  if (booted) return;
  booted = true;
  init();
}

window.addEventListener("pywebviewready", bootReady);
document.addEventListener("DOMContentLoaded", () => setTimeout(bootReady, 60));

async function init() {
  dbg("init start");
  try {
    S = await api().get_state();
  } catch (err) {
    dbg("init get_state failed: " + err);
    toast("读取配置失败：" + err, "err");
    return;
  }
  dbg("init state: " + typeof S + " version=" + (S && S.version));
  S.config = S.config || {};
  S.config.app = S.config.app || {};
  renderAll();
  bindStatic();
  loadAutostart();
  loadStats();
  setInterval(refreshStatus, 5000);
  setInterval(refreshOverviewLog, 10000);
  refreshOverviewLog();
  setInterval(tickUptime, 1000);
  refreshStatus();
  loadLogs();
  const hash = location.hash.replace("#", "");
  if (hash) switchView(hash);
}

function formatDuration(totalSec) {
  const sec = Math.max(0, Math.floor(totalSec));
  const h = Math.floor(sec / 3600);
  const m = Math.floor((sec % 3600) / 60);
  const s = sec % 60;
  if (h > 0) return `${h} 小时 ${m} 分`;
  if (m > 0) return `${m} 分 ${s} 秒`;
  return `${s} 秒`;
}

function tickUptime() {
  const el = $("#stat-uptime");
  if (!el) return;
  if (!S.startedAt) {
    el.textContent = "点「启动」开始工作";
    return;
  }
  const sec = Math.max(0, Math.floor(Date.now() / 1000 - S.startedAt));
  el.textContent = `已运行 ${formatDuration(sec)}`;
}

async function refreshOverviewLog() {
  const pre = $("#ov-log");
  if (!pre) return;
  try {
    const lines = await api().read_logs(12);
    const tail = (lines || []).slice(-12).join("\n");
    pre.textContent = tail || "（暂无日志）";
  } catch (e) {
    pre.textContent = "读取日志失败：" + zhError(e);
  }
}

/* ---------------- 渲染 ---------------- */
function renderAll() {
  const cfg = S.config;
  $("#app-version").textContent = S.version;
  $("#about-version").textContent = S.version;
  $("#f-token").value = cfg.telegram?.token || "";
  $("#f-admin-ids").value = (cfg.telegram?.admin_ids || []).join(", ");
  renderLanguages();
  renderModels();
  renderFeatures();
  renderPrompts();
  renderUpdate(S.update || {});
}

function renderLanguages() {
  const cfg = S.config;
  const mine = cfg.languages?.mine || "zh";
  const partners = cfg.languages?.partners || [];

  $("#f-my-lang").innerHTML = S.languages
    .map((l) => `<option value="${l.code}" ${l.code === mine ? "selected" : ""}>${l.name}</option>`)
    .join("");

  $("#partner-chips").innerHTML = S.languages
    .map((l) => `<button class="chip ${partners.includes(l.code) ? "on" : ""}" data-lang="${l.code}">${l.name}</button>`)
    .join("");

  $$("#partner-chips .chip").forEach((chip) => {
    chip.addEventListener("click", () => {
      const code = chip.dataset.lang;
      const list = S.config.languages.partners;
      const idx = list.indexOf(code);
      if (idx >= 0) list.splice(idx, 1);
      else list.push(code);
      chip.classList.toggle("on");
    });
  });
}

const PROVIDERS = [
  { label: "DeepSeek 官方", name: "DeepSeek", base: "https://api.deepseek.com/v1" },
  { label: "OpenAI", name: "OpenAI", base: "https://api.openai.com/v1" },
  { label: "Google Gemini", name: "Gemini", base: "https://generativelanguage.googleapis.com/v1beta/openai/" },
  { label: "xAI Grok", name: "Grok", base: "https://api.x.ai/v1" },
  { label: "Anthropic Claude（需中转）", name: "Claude", base: "" },
  { label: "Mistral", name: "Mistral", base: "https://api.mistral.ai/v1" },
  { label: "Groq", name: "Groq", base: "https://api.groq.com/openai/v1" },
  { label: "Together AI", name: "Together", base: "https://api.together.xyz/v1" },
  { label: "OpenRouter 中转", name: "OpenRouter", base: "https://openrouter.ai/api/v1" },
  { label: "通义千问（阿里云）", name: "通义千问", base: "https://dashscope.aliyuncs.com/compatible-mode/v1" },
  { label: "智谱 GLM", name: "智谱", base: "https://open.bigmodel.cn/api/paas/v4" },
  { label: "月之暗面 Kimi", name: "Kimi", base: "https://api.moonshot.cn/v1" },
  { label: "硅基流动 中转", name: "硅基流动", base: "https://api.siliconflow.cn/v1" },
  { label: "本地 Ollama", name: "Ollama", base: "http://127.0.0.1:11434/v1" },
  { label: "自定义 / 中转站", name: "", base: "" },
];

const modelCache = {};

function providerFor(base) {
  const value = (base || "").trim().replace(/\/$/, "");
  if (!value) return null;
  return PROVIDERS.find((p) => p.base && p.base.replace(/\/$/, "") === value) || null;
}

function presetOptions(m) {
  const current = providerFor(m.base_url);
  const label = current ? current.label : "自定义 / 中转站";
  return PROVIDERS.map(
    (p) => `<option value="${escapeAttr(p.label)}" ${p.label === label ? "selected" : ""}>${escapeAttr(p.label)}</option>`
  ).join("");
}

function modelSelectHtml(m, disabled) {
  const cached = modelCache[m.id || ""] || [];
  if (!cached.length) {
    return `<option value="">${disabled ? "填写 API Key 后自动拉取" : "点「拉取模型」获取"}</option>`;
  }
  const current = m.model || "";
  return cached
    .map(
      (v) =>
        `<option value="${escapeAttr(v)}" ${v === current ? "selected" : ""}>${escapeAttr(v)}</option>`
    )
    .join("");
}

function modelCard(m, index) {
  const id = m.id || `model-${Date.now()}-${index}`;
  const preset = providerFor(m.base_url);
  const hasModels = (modelCache[id] || []).length > 0;
  const title = preset ? preset.label : m.name || "自定义服务商";
  return `
    <div class="model-card" data-id="${id}" data-name="${escapeAttr(preset ? preset.name : m.name || "")}">
      <div class="model-head">
        <div class="model-title">${escapeAttr(title)}</div>
        <label class="mini-radio"><input type="radio" name="default-model" ${m.default ? "checked" : ""}> 默认</label>
        <button class="icon-btn" data-act="delete" title="删除这个模型">✕</button>
      </div>
      <div class="model-grid">
        <label class="field">服务商
          <select class="m-preset">${presetOptions(m)}</select>
        </label>
        <label class="field">接口地址
          <input class="m-base" value="${escapeAttr(m.base_url || "")}" placeholder="https://api.openai.com/v1">
        </label>
        <label class="field">API Key
          <input class="m-key" type="password" value="${escapeAttr(m.api_key || "")}" placeholder="sk-...">
        </label>
        <label class="field">模型
          <select class="m-model" ${hasModels ? "" : "disabled"}>${modelSelectHtml(m, !hasModels)}</select>
        </label>
      </div>
      <div class="model-foot">
        <button class="btn" data-act="test">拉取模型并测试</button>
        <span class="test-result"></span>
      </div>
    </div>`;
}

function renderModels() {
  const list = S.config.models || [];
  $("#model-list").innerHTML = list.length
    ? list.map((m, i) => modelCard(m, i)).join("")
    : `<div class="empty">还没有模型，点下面「添加模型」开始</div>`;
}

function fillModelSelect(card, names) {
  const sel = card.querySelector(".m-model");
  if (!sel) return;
  sel.disabled = false;
  const current = sel.value || "";
  sel.innerHTML = names
    .map(
      (v) =>
        `<option value="${escapeAttr(v)}" ${v === current ? "selected" : ""}>${escapeAttr(v)}</option>`
    )
    .join("");
  if (!sel.value) sel.value = names[0];
}

async function fetchModels(card, quiet) {
  const payload = readModel(card);
  if (!payload.base_url || !payload.api_key) return false;
  if (!quiet) {
    const out = card.querySelector(".test-result");
    out.className = "test-result";
    out.textContent = "拉取模型列表中…";
  }
  const res = await api().list_models(payload);
  if (res && res.ok && (res.models || []).length) {
    modelCache[card.dataset.id] = res.models;
    fillModelSelect(card, res.models);
    if (!quiet) {
      const out = card.querySelector(".test-result");
      out.className = "test-result ok";
      out.textContent = `已加载 ${res.models.length} 个模型`;
    }
    return true;
  }
  if (!quiet) {
    const out = card.querySelector(".test-result");
    out.className = "test-result err";
    out.textContent = (res && res.message) || "拉取失败，检查接口地址和 Key";
  }
  return false;
}

function renderFeatures() {
  const f = S.config.features || {};
  $("#f-bilingual").checked = !!f.bilingual;
  $("#f-copyblock").checked = f.copy_block !== false;
  $("#f-tone").checked = !!f.tone_buttons;
  $("#f-explain").checked = !!f.explain_button;
  $("#f-context").checked = !!f.context;
  $("#f-cache").checked = !!f.cache;
  $("#f-glossary").checked = !!f.glossary;
  $("#f-autodetect").checked = !!f.auto_detect;
  $("#f-group-mode").value = f.group_mode || "reply_or_mention";
  $("#f-max-chars").value = f.max_input_chars || 6000;
}

function renderPrompts() {
  const p = S.config.prompts || {};
  const defaults = S.defaults || {};
  const ta = $("#f-prompt-translation");
  const tb = $("#f-prompt-explain");
  ta.value = p.translation || defaults.translation_prompt || "";
  tb.value = p.explain || defaults.explain_prompt || "";
  ta.placeholder = defaults.translation_prompt || "";
  tb.placeholder = defaults.explain_prompt || "";
  $("#prompt-hint").textContent = p.translation ? "使用自定义提示词" : "使用内置默认提示词";
}

function closeModal() {
  const root = $("#modal-root");
  if (root) root.classList.remove("show");
}

function showModal(title, bodyHtml, actions) {
  const root = $("#modal-root");
  if (!root) return;
  $("#modal-title").textContent = title;
  $("#modal-body").innerHTML = bodyHtml || "";
  const box = $("#modal-actions");
  box.innerHTML = "";
  (actions || []).forEach((action) => {
    const btn = document.createElement("button");
    btn.className = "btn" + (action.primary ? " primary" : "");
    btn.textContent = action.label;
    btn.addEventListener("click", () => {
      if (action.onClick) action.onClick();
    });
    box.appendChild(btn);
  });
  root.classList.add("show");
}

function renderUpdate(info) {
  if (!info || !info.available) return;
  S.pendingUpdateVersion = info.version;
  const notes = (info.notes || "包含若干体验改进与问题修复。").slice(0, 400);
  showModal(
    `发现新版本 v${info.version}`,
    `<div class="modal-notes">${escapeAttr(notes)}</div>`,
    [
      { label: "暂不更新", onClick: closeModal },
      { label: "下载并更新", primary: true, onClick: () => startInstall() },
    ]
  );
}

async function startInstall() {
  if (S.version && S.pendingUpdateVersion &&
      String(S.pendingUpdateVersion).trim().toLowerCase() === String(S.version).trim().toLowerCase()) {
    showModal("已经是最新版本", '<div class="modal-notes">当前版本已经是最新的了。</div>',
      [{ label: "知道了", primary: true, onClick: closeModal }]);
    return;
  }
  showModal(
    "正在下载更新",
    '<div class="upd-bar"><i id="upd-bar-fill"></i></div><div class="hint" id="upd-text">准备中…</div>',
    [{ label: "后台下载", onClick: closeModal }]
  );
  if (!(window.__TAURI__ && window.__TAURI__.core)) {
    showModal(
      "无法更新",
      '<div class="modal-notes">请在安装版程序里使用「检查更新」。</div>',
      [{ label: "知道了", primary: true, onClick: closeModal }]
    );
    return;
  }
  try {
    await window.__TAURI__.core.invoke("update_install");
    showModal("更新已就绪", '<div class="modal-notes">程序会很快自动重启完成安装。</div>', []);
  } catch (e) {
    showModal(
      "更新失败",
      `<div class="modal-notes">${escapeAttr(zhError(e))}</div>`,
      [{ label: "知道了", primary: true, onClick: closeModal }]
    );
  }
}

/* ---------------- 交互 ---------------- */
function bindStatic() {
  $$(".nav button").forEach((btn) => btn.addEventListener("click", () => switchView(btn.dataset.view)));

  $("#btn-save").addEventListener("click", save);
  document.addEventListener("keydown", (e) => {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "s") {
      e.preventDefault();
      save();
    }
  });

  $("#btn-start").addEventListener("click", () => control("start_bot"));
  $("#btn-stop").addEventListener("click", () => control("stop_bot"));
  $("#btn-restart").addEventListener("click", () => control("restart_bot"));

  $("#btn-quit").addEventListener("click", async () => {
    const ok = confirm("确定退出翻译机器人？退出后机器人会停止工作。");
    if (!ok) return;
    await api().quit_app();
  });

  $("#btn-add-model").addEventListener("click", () => {
    S.config.models = S.config.models || [];
    S.config.models.push({
      id: `model-${Date.now()}`,
      name: "新模型",
      base_url: "https://api.deepseek.com/v1",
      api_key: "",
      model: "",
      kind: "text",
      default: S.config.models.length === 0,
    });
    renderModels();
  });

  $("#model-list").addEventListener("click", async (e) => {
    const btn = e.target.closest("[data-act]");
    if (!btn) return;
    const card = btn.closest(".model-card");
    if (!card) return;
    if (btn.dataset.act === "delete") {
      S.config.models = (S.config.models || []).filter((m) => (m.id || "") !== card.dataset.id);
      renderModels();
      return;
    }
    if (btn.dataset.act === "preset") {
      return;
    }
    if (btn.dataset.act === "test") {
      const result = card.querySelector(".test-result");
      result.className = "test-result";
      result.textContent = "拉取模型并测试连接… 0 秒";
      btn.disabled = true;
      const startedAt = Date.now();
      const timer = setInterval(() => {
        const sec = Math.floor((Date.now() - startedAt) / 1000);
        result.textContent = `拉取模型并测试连接… ${sec} 秒`;
      }, 500);
      const payload = readModel(card);
      let res;
      try {
        res = await api().test_model(payload);
      } catch (e) {
        res = { ok: false, message: zhError(e) };
      } finally {
        clearInterval(timer);
        btn.disabled = false;
      }
      if (res.ok) {
        result.className = "test-result ok";
        result.textContent = `${res.message} · 可用模型 ${(res.models || []).length} 个`;
        if (res.models && res.models.length) {
          modelCache[card.dataset.id] = res.models;
          fillModelSelect(card, res.models);
        }
      } else {
        result.className = "test-result err";
        result.textContent = res.message;
      }
    }
  });


  $("#btn-test-token").addEventListener("click", async () => {
    const btn = $("#btn-test-token");
    const out = $("#token-result");
    btn.disabled = true;
    out.className = "test-result";
    out.textContent = "检测中…";
    try {
      const res = await api().test_token($("#f-token").value.trim());
      out.className = "test-result " + (res.ok ? "ok" : "err");
      out.textContent = res.message || (res.ok ? "已连接" : "连接失败");
    } catch (e) {
      out.className = "test-result err";
      out.textContent = "检测失败：" + zhError(e);
    }
    btn.disabled = false;
  });

  $("#model-list").addEventListener("change", (e) => {
    const card = e.target.closest(".model-card");
    if (!card) return;
    const select = e.target.closest(".m-preset");
    if (!select) return;
    const preset = PROVIDERS.find((p) => p.label === select.value);
    if (!preset) return;
    card.dataset.name = preset.name;
    const titleEl = card.querySelector(".model-title");
    if (titleEl) titleEl.textContent = preset.label;
    card.querySelector(".m-base").value = preset.base;
    delete modelCache[card.dataset.id];
    const modelSel = card.querySelector(".m-model");
    if (modelSel) {
      modelSel.disabled = true;
      modelSel.innerHTML = `<option value="">填写 API Key 后自动拉取</option>`;
    }
    const out = card.querySelector(".test-result");
    if (out) {
      out.className = "test-result";
      out.textContent = "";
    }
    const key = card.querySelector(".m-key");
    if (key && key.value.trim() && preset.base) fetchModels(card, false);
  });

  $("#model-list").addEventListener(
    "input",
    (e) => {
      if (!e.target.closest(".m-key")) return;
      const card = e.target.closest(".model-card");
      if (!card) return;
      clearTimeout(card._fetchTimer);
      card._fetchTimer = setTimeout(() => {
        const payload = readModel(card);
        if (payload.base_url && payload.api_key) fetchModels(card, true);
      }, 700);
    },
    true
  );

  $("#btn-test").addEventListener("click", async () => {
    const text = $("#test-input").value.trim();
    if (!text) return toast("先输入要翻译的内容", "err");
    const btn = $("#btn-test");
    btn.disabled = true;
    $("#test-result").textContent = "翻译中…";
    $("#test-meta").textContent = "";
    const res = await api().test_translate(text, $("#test-tone").value);
    btn.disabled = false;
    if (res.ok) {
      $("#test-result").textContent = res.translation;
      $("#test-meta").textContent = `${res.src_name} → ${res.tgt_name} · ${res.elapsed_ms} ms · ${res.tokens} tokens`;
    } else {
      $("#test-result").textContent = "翻译失败";
      $("#test-meta").textContent = res.message || "";
    }
  });

  $("#btn-reset-translation").addEventListener("click", () => {
    $("#f-prompt-translation").value = S.defaults.translation_prompt || "";
    $("#prompt-hint").textContent = "已恢复默认（保存后生效）";
  });
  $("#btn-reset-explain").addEventListener("click", () => {
    $("#f-prompt-explain").value = S.defaults.explain_prompt || "";
  });

  $("#btn-refresh-log").addEventListener("click", loadLogs);
  $("#btn-open-logs").addEventListener("click", () => api().open_folder("logs"));
  $("#btn-diagnostics").addEventListener("click", () => exportDiag("#log-hint"));
  $("#btn-diag-2").addEventListener("click", () => exportDiag(null));
  $("#btn-open-root").addEventListener("click", () => api().open_folder("root"));
  $("#btn-open-data").addEventListener("click", () => api().open_folder("data"));

  $("#btn-quick-logs").addEventListener("click", () => switchView("logs"));
  $("#ov-log-open").addEventListener("click", () => switchView("logs"));
  $("#btn-quick-test").addEventListener("click", () => switchView("prompts"));
  $("#btn-quick-data").addEventListener("click", () => api().open_folder("data"));
  $("#btn-quick-diag").addEventListener("click", () => exportDiag(null));

  $("#f-autostart").addEventListener("change", async (e) => {
    const res = await api().set_autostart(e.target.checked);
    if (!res || res.ok === false) {
      e.target.checked = !e.target.checked;
      toast((res && res.message) || "设置失败", "err");
    } else {
      toast(res.enabled ? "已开启开机自启" : "已关闭开机自启", "ok");
    }
  });

  if (window.__TAURI__ && window.__TAURI__.event) {
    window.__TAURI__.event.listen("update-progress", (e) => {
      const d = (e && e.payload) || {};
      const text = $("#upd-text");
      const fill = $("#upd-bar-fill");
      if (!text) return;
      if (d.total && d.total > 0) {
        const pct = Math.min(100, Math.round((d.chunk / d.total) * 100));
        text.textContent = `已下载 ${pct}%`;
        if (fill) fill.style.width = pct + "%";
      } else {
        text.textContent = "正在下载…";
      }
    });
  }

  $("#modal-mask").addEventListener("click", closeModal);
  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape") closeModal();
  });

  $("#btn-check-update").addEventListener("click", async () => {
    const btn = $("#btn-check-update");
    btn.disabled = true;
    let info = null;
    if (window.__TAURI__ && window.__TAURI__.core) {
      try {
        info = await window.__TAURI__.core.invoke("update_check");
      } catch (e) {
        // 原生更新不可用时回退到内置更新接口，避免把 ACL 报错暴露给用户
        info = null;
      }
    }
    if (!info) {
      btn.disabled = false;
      showModal(
        "无法检查更新",
        '<div class="modal-notes">请在安装版程序里使用「检查更新」。</div>',
        [{ label: "知道了", primary: true, onClick: closeModal }]
      );
      return;
    }
    btn.disabled = false;
    const sameVersion =
      info.version && S.version &&
      String(info.version).trim().toLowerCase() === String(S.version).trim().toLowerCase();
    if (info.available && sameVersion) {
      info.available = false;
      info.version = null;
    }
    if (info.available) {
      renderUpdate(info);
      toast(`发现新版本 v${info.version}`, "ok");
    } else {
      toast(info.error ? zhError(info.error) : info.reason || "已经是最新版本", info.error ? "err" : "ok");
    }
  });


}

async function exportDiag(hintSelector) {
  const res = await api().export_diagnostics();
  if (res.ok) {
    if (hintSelector) $(hintSelector).textContent = "已导出：" + res.path;
    toast("诊断包已导出（密钥已脱敏）", "ok");
  } else {
    toast(res.message || "导出失败", "err");
  }
}

function switchView(name) {
  $$(".nav button").forEach((b) => b.classList.toggle("active", b.dataset.view === name));
  $$(".view").forEach((v) => v.classList.toggle("active", v.id === `view-${name}`));
  const titles = {
    overview: "概览",
    languages: "语言",
    models: "模型",
    features: "功能",
    prompts: "提示词",
    logs: "日志",
    settings: "设置",
  };
  $("#view-title").textContent = titles[name] || "";
  if (name === "logs") loadLogs();
}

function readModel(card) {
  return {
    id: card.dataset.id,
    name: card.dataset.name || card.querySelector(".model-title").textContent.trim(),
    base_url: card.querySelector(".m-base").value.trim(),
    model: card.querySelector(".m-model").value.trim(),
    api_key: card.querySelector(".m-key").value.trim(),
    kind: card.querySelector(".m-kind").value,
    default: card.querySelector('input[name="default-model"]').checked,
  };
}

function collect() {
  const models = $$(".model-card").map(readModel);
  const partners = S.config.languages.partners || [];
  const defaults = S.defaults || {};
  const tPrompt = $("#f-prompt-translation").value.trim();
  const ePrompt = $("#f-prompt-explain").value.trim();

  return {
    version: S.version,
    telegram: {
      token: $("#f-token").value.trim(),
      admin_ids: $("#f-admin-ids")
        .value.split(/[,，\s]+/)
        .filter(Boolean)
        .map((v) => parseInt(v, 10))
        .filter((v) => !isNaN(v)),
    },
    models,
    languages: { mine: $("#f-my-lang").value, partners },
    features: {
      bilingual: $("#f-bilingual").checked,
      copy_block: $("#f-copyblock").checked,
      tone_buttons: $("#f-tone").checked,
      explain_button: $("#f-explain").checked,
      glossary: $("#f-glossary").checked,
      context: $("#f-context").checked,
      cache: $("#f-cache").checked,
      auto_detect: $("#f-autodetect").checked,
      group_mode: $("#f-group-mode").value,
      max_input_chars: parseInt($("#f-max-chars").value, 10) || 6000,
      history_size: S.config.features?.history_size || 3,
    },
    prompts: {
      translation: tPrompt === (defaults.translation_prompt || "").trim() ? "" : tPrompt,
      explain: ePrompt === (defaults.explain_prompt || "").trim() ? "" : ePrompt,
    },
    update: {
      mode: $("#f-update-mode").value,
      custom_proxy: $("#f-update-proxy").value.trim(),
    },
    app: {
      close_to_tray: $("#f-close-tray").checked,
      autostart_bot: $("#f-autostart-bot").checked,
      check_update_on_start: S.config.app?.check_update_on_start !== false,
    },
    proxy: S.config.proxy || "",
  };
}

async function save() {
  const btn = $("#btn-save");
  btn.disabled = true;
  const payload = collect();
  const res = await api().save_config(payload);
  btn.disabled = false;
  if (!res.ok) return toast(res.message || "保存失败", "err");

  S.config = payload;
  toast("配置已保存", "ok");
  if (res.issues && res.issues.length) toast("还有待完善：" + res.issues[0], "err");
  if (res.needs_restart) {
    const ok = confirm("配置已保存。机器人正在运行，是否立即重启使其生效？");
    if (ok) control("restart_bot");
  }
}

async function control(action) {
  const out = $("#ctl-result");
  out.className = "test-result";
  out.textContent = "处理中…";
  const res = await api()[action]();
  out.className = "test-result " + (res.ok ? "ok" : "err");
  out.textContent = res.message || "";
  refreshStatus();
}

/* ---------------- 状态 ---------------- */
async function refreshStatus() {
  let st;
  try {
    st = await api().bot_status();
  } catch (e) {
    return;
  }
  const running = st.running;
  const pill = $("#status-pill");
  pill.className = "pill" + (running ? "" : " off");
  pill.innerHTML = `<span class="dot ${running ? "on" : "off"}" style="width:6px;height:6px"></span>${running ? "运行中" : "已停止"}`;

  $("#side-dot").className = "dot " + (running ? "on" : "off");
  $("#side-status-text").textContent = running ? "机器人运行中" : "机器人已停止";
  $("#stat-status").textContent = running ? "运行中" : "已停止";
  const ctl = $("#ctl-state");
  if (ctl) ctl.textContent = running ? `当前状态：运行中${st.pid ? " · PID " + st.pid : ""}` : "当前状态：已停止";
  const ovDot = $("#ov-dot");
  if (ovDot) ovDot.className = "ov-dot" + (running ? " on" : "");
  S.startedAt = running ? st.started_at : null;
  tickUptime();

  const cfg = S.config || {};
  const models = cfg.models || [];
  const def = models.find((m) => m.default) || models[0];
  $("#stat-model").textContent = def ? def.name || def.model : "未配置";
  $("#stat-model-base").textContent = def ? def.base_url : "去「模型」页添加";

  const nameOf = (code) => (S.languages.find((l) => l.code === code) || {}).name || code;
  const partners = (cfg.languages?.partners || []).map(nameOf).filter(Boolean);
  $("#stat-lang").textContent = partners.length
    ? `${partners.join(" / ")} → ${nameOf(cfg.languages?.mine)}`
    : "未设置对方语言";
}

async function loadStats() {
  const res = await api().get_stats();
  if (!res || !res.ok) return;
  $("#stat-calls").textContent = res.calls;
  $("#stat-tokens").textContent = `${res.tokens} tokens · 缓存 ${res.cache}`;
}

async function loadAutostart() {
  try {
    const r = await api().get_autostart();
    $("#f-autostart").checked = !!(r && r.enabled);
  } catch (e) {}
  const app = S.config.app || {};
  const upd = S.config.update || {};
  $("#f-update-mode").value = upd.mode || "auto";
  $("#f-update-proxy").value = upd.custom_proxy || "";
  $("#f-update-proxy").disabled = ($("#f-update-mode").value !== "custom");
  $("#f-update-mode").addEventListener("change", () => {
    $("#f-update-proxy").disabled = ($("#f-update-mode").value !== "custom");
  });
  $("#f-autostart-bot").checked = app.autostart_bot !== false;
  $("#f-close-tray").checked = app.close_to_tray !== false;
}

/* ---------------- 日志 ---------------- */
async function loadLogs() {
  const lines = await api().read_logs(200);
  const box = $("#log-box");
  box.innerHTML = lines.map(colorize).join("\n");
  box.scrollTop = box.scrollHeight;
}

function colorize(line) {
  const escaped = escapeHtml(line);
  if (/ ERROR /.test(line)) return `<span class="err">${escaped}</span>`;
  if (/ WARNING /.test(line)) return `<span class="warn">${escaped}</span>`;
  return escaped;
}

/* ---------------- 工具 ---------------- */
function toast(message, kind = "") {
  const box = $("#toast");
  const el = document.createElement("div");
  el.className = "toast " + kind;
  el.textContent = message;
  box.appendChild(el);
  setTimeout(() => {
    el.style.opacity = "0";
    el.style.transform = "translateY(6px)";
    el.style.transition = "all .2s";
    setTimeout(() => el.remove(), 220);
  }, 3200);
}

function escapeHtml(text) {
  return String(text).replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]));
}
function escapeAttr(text) {
  return escapeHtml(text).replace(/"/g, "&quot;");
}
