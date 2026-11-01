const $ = (sel, root = document) => root.querySelector(sel);
const $$ = (sel, root = document) => Array.from(root.querySelectorAll(sel));

async function rpc(method, ...args) {
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
  try {
    S = await api().get_state();
  } catch (err) {
    toast("读取配置失败：" + err, "err");
    return;
  }
  S.config = S.config || {};
  S.config.app = S.config.app || {};
  renderAll();
  bindStatic();
  loadAutostart();
  loadStats();
  setInterval(refreshStatus, 5000);
  refreshStatus();
  loadLogs();
  const hash = location.hash.replace("#", "");
  if (hash) switchView(hash);
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

function modelCard(m, index) {
  const id = m.id || `model-${Date.now()}-${index}`;
  return `
    <div class="model-card" data-id="${id}">
      <div class="model-head">
        <input class="m-name" value="${escapeAttr(m.name || "")}" placeholder="显示名称，例如 DeepSeek">
        <label class="mini-radio"><input type="radio" name="default-model" ${m.default ? "checked" : ""}> 默认</label>
        <button class="btn danger" data-act="delete">删除</button>
      </div>
      <div class="model-grid">
        <label class="field">接口地址<input class="m-base" value="${escapeAttr(m.base_url || "")}" placeholder="https://api.deepseek.com/v1"></label>
        <label class="field">模型名<input class="m-model" value="${escapeAttr(m.model || "")}" placeholder="deepseek-v4-pro"></label>
        <label class="field">API Key<input class="m-key" type="password" value="${escapeAttr(m.api_key || "")}" placeholder="sk-..."></label>
        <label class="field">用途
          <select class="m-kind">
            <option value="text" ${m.kind !== "vision" ? "selected" : ""}>文本翻译</option>
            <option value="vision" ${m.kind === "vision" ? "selected" : ""}>看图翻译</option>
          </select>
        </label>
      </div>
      <div class="model-foot">
        <button class="btn" data-act="test">测试连接</button>
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

function renderUpdate(info) {
  const banner = $("#update-banner");
  if (info && info.available) {
    banner.classList.add("show");
    $("#update-title").textContent = `发现新版本 v${info.version}`;
    $("#update-notes").textContent = (info.notes || "").slice(0, 120);
  } else {
    banner.classList.remove("show");
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
    if (btn.dataset.act === "test") {
      const result = card.querySelector(".test-result");
      result.className = "test-result";
      result.textContent = "测试中…";
      btn.disabled = true;
      const res = await api().test_model(readModel(card));
      btn.disabled = false;
      if (res.ok) {
        result.className = "test-result ok";
        result.textContent = `${res.message} · 可用模型 ${(res.models || []).length} 个`;
      } else {
        result.className = "test-result err";
        result.textContent = res.message;
      }
    }
  });

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

  $("#btn-check-update").addEventListener("click", async () => {
    const btn = $("#btn-check-update");
    btn.disabled = true;
    const info = await api().check_update();
    btn.disabled = false;
    if (info.available) {
      renderUpdate(info);
      toast(`发现新版本 v${info.version}`, "ok");
    } else {
      toast(info.reason || "已经是最新版本", "ok");
    }
  });

  $("#btn-do-update").addEventListener("click", async () => {
    const btn = $("#btn-do-update");
    btn.disabled = true;
    btn.textContent = "下载中…";
    const res = await api().apply_update();
    if (!res.ok) {
      btn.disabled = false;
      btn.textContent = "下载并更新";
      toast(res.message || "更新失败", "err");
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
    name: card.querySelector(".m-name").value.trim(),
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
    update: S.config.update || { enabled: true, repo: "" },
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
  $("#stat-uptime").textContent = running ? `已运行 ${st.uptime}` : "点「启动」开始工作";

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
