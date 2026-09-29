// Editor profil deck (langkah 2.3). Logika murni ada di deck-logic.js; berkas ini hanya DOM
// dan pemanggilan command Tauri. Semua teks masuk lewat textContent/value, bukan innerHTML.
(function () {
  "use strict";
  const L = window.DeckLogic;
  const { invoke } = window.__TAURI__.core;
  const $ = (id) => document.getElementById(id);

  const ICONS = {
    skip_previous: "⏮", play_pause: "⏯", skip_next: "⏭", volume_up: "🔊", volume_down: "🔉",
    volume_off: "🔇", copy: "⧉", cut: "✂", undo: "↶", redo: "↷", select_all: "▦", lock: "🔒", paste: "📋", screenshot: "📷", mic: "🎙", record: "⏺",
    stop: "⏹", browser: "🌐", folder: "📁", app: "▣", scene: "🎬",
  };

  let profiles = [];
  let ed = null; // keadaan editor (lihat toEditor)
  let selected = null; // indeks sel terpilih
  let dirty = false;

  const say = (text, isError) => {
    const m = $("deck-msg");
    m.textContent = text;
    m.style.color = isError ? "var(--bad)" : "";
    if (text) setTimeout(() => { if (m.textContent === text) m.textContent = ""; }, 4000);
  };

  async function load(keepId) {
    profiles = await invoke("deck_list");
    const sel = $("deck-select");
    sel.replaceChildren();
    for (const p of profiles) {
      const o = document.createElement("option");
      o.value = p.id;
      o.textContent = p.name;
      sel.append(o);
    }
    const target = profiles.find((p) => p.id === keepId) || profiles[0];
    if (target) {
      sel.value = target.id;
      open(target);
    }
  }

  function open(profile) {
    ed = L.toEditor(profile);
    selected = null;
    dirty = false;
    $("deck-name").value = ed.name;
    $("deck-cols").value = ed.cols;
    $("deck-rows").value = ed.rows;
    $("cell-editor").hidden = true;
    renderGrid();
  }

  function renderGrid() {
    const grid = $("deck-grid");
    grid.style.gridTemplateColumns = `repeat(${ed.cols}, 1fr)`;
    grid.replaceChildren();
    for (let i = 0; i < ed.cols * ed.rows; i++) {
      const cell = ed.cells.find((c) => c.index === i);
      const div = document.createElement("div");
      div.className = "deck-cell" + (cell ? " filled" : "") + (i === selected ? " selected" : "");
      if (cell) {
        const ico = document.createElement("span");
        ico.className = "ico";
        ico.textContent = ICONS[cell.icon] || (cell.label.trim()[0] || "•").toUpperCase();
        const lbl = document.createElement("span");
        lbl.className = "lbl";
        lbl.textContent = cell.label;
        div.append(ico, lbl);
        if (cell.color) div.style.background = cell.color + "33";
        div.draggable = true;
        div.addEventListener("dragstart", (e) => e.dataTransfer.setData("text/plain", String(i)));
      } else {
        div.textContent = "+";
      }
      div.addEventListener("dragover", (e) => { e.preventDefault(); div.classList.add("over"); });
      div.addEventListener("dragleave", () => div.classList.remove("over"));
      div.addEventListener("drop", (e) => {
        e.preventDefault();
        const from = Number(e.dataTransfer.getData("text/plain"));
        if (Number.isInteger(from)) {
          ed.cells = L.moveCell(ed.cells, from, i);
          if (selected === from) selected = i;
          dirty = true;
          renderGrid();
        }
      });
      div.addEventListener("click", () => selectCell(i));
      grid.append(div);
    }
  }

  // ------------------------------------------------------------- editor sel

  function fillTypeSelect() {
    const s = $("cell-type");
    s.replaceChildren();
    for (const t of L.ACTION_TYPES) {
      const o = document.createElement("option");
      o.value = t;
      o.textContent = L.LABELS[t];
      s.append(o);
    }
  }

  function renderValueField(type, value) {
    const wrap = $("cell-value-wrap");
    wrap.replaceChildren();
    let field;
    if (type === "media_key") {
      field = document.createElement("select");
      for (const k of L.MEDIA_KEYS) {
        const o = document.createElement("option");
        o.value = k;
        o.textContent = k;
        field.append(o);
      }
      field.value = L.MEDIA_KEYS.includes(value) ? value : L.MEDIA_KEYS[0];
    } else if (type === "multi") {
      field = document.createElement("textarea");
      field.placeholder = "satu langkah per baris, mis.\nhotkey: ctrl+c\nurl: https://example.com\nmedia: mute";
      field.value = value || "";
    } else {
      field = document.createElement("input");
      field.type = "text";
      field.spellcheck = false;
      field.placeholder = {
        hotkey: "mis. ctrl+shift+p (meta = Cmd/Win)",
        open_url: "https://…",
        launch_app: "mis. C:\\Program Files\\App\\app.exe",
        open_path: "mis. D:\\Dokumen",
        obs_scene: "nama scene",
      }[type] || "";
      field.value = value || "";
    }
    field.id = "cell-value";
    wrap.append(field);
  }

  function selectCell(i) {
    selected = i;
    const cell = ed.cells.find((c) => c.index === i);
    $("cell-editor").hidden = false;
    $("cell-error").hidden = true;
    $("cell-label").value = cell ? cell.label : "";
    $("cell-icon").value = cell ? cell.icon || "" : "";
    $("cell-color").value = cell && cell.color ? cell.color : "#334155";
    $("cell-color-on").checked = !!(cell && cell.color);
    const type = cell ? cell.action.type : "hotkey";
    $("cell-type").value = type;
    renderValueField(type, cell ? L.actionValue(cell.action) : "");
    $("cell-remove").disabled = !cell;
    renderGrid();
  }

  $("cell-type").addEventListener("change", () => renderValueField($("cell-type").value, ""));

  $("cell-apply").onclick = () => {
    const err = $("cell-error");
    try {
      const label = $("cell-label").value.trim();
      if (!label) throw new Error("Label wajib diisi");
      const action = L.buildAction($("cell-type").value, $("cell-value").value);
      const cell = {
        index: selected,
        label,
        icon: $("cell-icon").value.trim() || null,
        color: $("cell-color-on").checked ? $("cell-color").value : null,
        action,
      };
      ed.cells = ed.cells.filter((c) => c.index !== selected).concat(cell);
      dirty = true;
      err.hidden = true;
      renderGrid();
    } catch (e) {
      err.textContent = String(e.message || e);
      err.hidden = false;
    }
  };

  $("cell-remove").onclick = () => {
    ed.cells = ed.cells.filter((c) => c.index !== selected);
    dirty = true;
    $("cell-editor").hidden = true;
    selected = null;
    renderGrid();
  };

  // -------------------------------------------------------------- profil

  function resize() {
    const cols = Math.min(8, Math.max(1, Number($("deck-cols").value) || ed.cols));
    const rows = Math.min(8, Math.max(1, Number($("deck-rows").value) || ed.rows));
    if (cols === ed.cols && rows === ed.rows) return;
    const { cells, dropped } = L.resizeGrid(ed.cells, ed.cols, cols, rows);
    if (dropped > 0 && !confirm(`${dropped} tombol akan terbuang karena di luar grid baru. Lanjutkan?`)) {
      $("deck-cols").value = ed.cols;
      $("deck-rows").value = ed.rows;
      return;
    }
    ed.cells = cells;
    ed.cols = cols;
    ed.rows = rows;
    selected = null;
    $("cell-editor").hidden = true;
    dirty = true;
    renderGrid();
  }
  $("deck-cols").addEventListener("change", resize);
  $("deck-rows").addEventListener("change", resize);
  $("deck-name").addEventListener("input", () => { ed.name = $("deck-name").value; dirty = true; });

  $("deck-select").addEventListener("change", () => {
    if (dirty && !confirm("Perubahan belum disimpan. Buang?")) {
      $("deck-select").value = ed.id;
      return;
    }
    open(profiles.find((p) => p.id === $("deck-select").value));
  });

  $("deck-new").onclick = () => {
    if (dirty && !confirm("Perubahan belum disimpan. Buang?")) return;
    const name = "Profil baru";
    const id = L.slug(name, profiles.map((p) => p.id));
    ed = { id, name, cols: 3, rows: 2, cells: [] };
    selected = null;
    dirty = true;
    $("deck-name").value = name;
    $("deck-cols").value = 3;
    $("deck-rows").value = 2;
    $("cell-editor").hidden = true;
    renderGrid();
  };

  $("deck-save").onclick = async () => {
    try {
      if (!ed.name.trim()) throw new Error("Nama profil wajib diisi");
      // Profil baru (belum ada di host) mengambil id dari namanya.
      if (!profiles.some((p) => p.id === ed.id)) {
        ed.id = L.slug(ed.name, profiles.map((p) => p.id));
      }
      await invoke("deck_save", { profile: L.fromEditor(ed) });
      dirty = false;
      say("Tersimpan. Perangkat memuat ulang profil saat membuka mode Deck.");
      await load(ed.id);
    } catch (e) {
      say(String(e.message || e), true);
    }
  };

  $("deck-delete").onclick = async () => {
    if (!confirm(`Hapus profil “${ed.name}”?`)) return;
    try {
      if (profiles.some((p) => p.id === ed.id)) await invoke("deck_delete", { id: ed.id });
      await load();
    } catch (e) {
      say(String(e), true);
    }
  };

  fillTypeSelect();
  load().catch((e) => say(String(e), true));
})();
