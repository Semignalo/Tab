// Logika murni editor deck: parsing/format aksi dan operasi grid. Dipisah dari DOM agar bisa
// diuji dengan `node --test` (lihat host/tests/deck-logic.test.js).
(function (root) {
  "use strict";

  const MEDIA_KEYS = ["media_play", "media_next", "media_prev", "volume_up", "volume_down", "mute"];
  const ACTION_TYPES = ["hotkey", "media_key", "open_url", "launch_app", "open_path", "obs_scene", "multi"];

  const LABELS = {
    hotkey: "Hotkey",
    media_key: "Tombol media",
    open_url: "Buka URL",
    launch_app: "Jalankan aplikasi",
    open_path: "Buka berkas/folder",
    obs_scene: "Scene OBS",
    multi: "Beberapa langkah",
  };

  /** "ctrl+shift+p" → ["ctrl","shift","p"]; menolak kosong. */
  function parseHotkey(text) {
    const keys = String(text).toLowerCase().split("+").map((s) => s.trim()).filter(Boolean);
    if (keys.length === 0) throw new Error("Hotkey kosong");
    return keys;
  }

  /** Satu langkah dalam bentuk teks "jenis: nilai" → objek aksi. */
  function parseStep(line) {
    const i = line.indexOf(":");
    if (i < 0) throw new Error(`Langkah tidak valid: “${line}” (format “jenis: nilai”)`);
    const kind = line.slice(0, i).trim().toLowerCase();
    const value = line.slice(i + 1).trim();
    if (!value) throw new Error(`Nilai kosong pada: “${line}”`);
    switch (kind) {
      case "hotkey": return { type: "hotkey", keys: parseHotkey(value) };
      case "media": return { type: "media_key", key: value };
      case "url": return { type: "open_url", url: value };
      case "app": return { type: "launch_app", path: value };
      case "path": return { type: "open_path", path: value };
      case "obs": return { type: "obs_scene", scene: value };
      default: throw new Error(`Jenis langkah tidak dikenal: ${kind}`);
    }
  }

  function stepToText(a) {
    switch (a.type) {
      case "hotkey": return "hotkey: " + a.keys.join("+");
      case "media_key": return "media: " + a.key;
      case "open_url": return "url: " + a.url;
      case "launch_app": return "app: " + a.path;
      case "open_path": return "path: " + a.path;
      case "obs_scene": return "obs: " + a.scene;
      default: return "";
    }
  }

  /** Nilai satu field form → aksi. `type` salah satu ACTION_TYPES. */
  function buildAction(type, value) {
    const v = String(value ?? "").trim();
    switch (type) {
      case "hotkey": return { type, keys: parseHotkey(v) };
      case "media_key":
        if (!MEDIA_KEYS.includes(v)) throw new Error("Tombol media tidak dikenal");
        return { type, key: v };
      case "open_url":
        if (!/^(https?:\/\/|mailto:)/i.test(v)) throw new Error("URL harus diawali http://, https://, atau mailto:");
        return { type, url: v };
      case "launch_app":
        if (!v) throw new Error("Path aplikasi kosong");
        return { type, path: v };
      case "open_path":
        if (!v) throw new Error("Path kosong");
        return { type, path: v };
      case "obs_scene":
        if (!v) throw new Error("Nama scene kosong");
        return { type, scene: v };
      case "multi": {
        const steps = v.split("\n").map((s) => s.trim()).filter(Boolean).map(parseStep);
        if (steps.length === 0) throw new Error("Belum ada langkah");
        return { type, steps };
      }
      default: throw new Error("Jenis aksi tidak dikenal");
    }
  }

  /** Aksi → nilai untuk field form. */
  function actionValue(a) {
    switch (a.type) {
      case "hotkey": return a.keys.join("+");
      case "media_key": return a.key;
      case "open_url": return a.url;
      case "launch_app":
      case "open_path": return a.path;
      case "obs_scene": return a.scene;
      case "multi": return a.steps.map(stepToText).join("\n");
      default: return "";
    }
  }

  /** Tukar/pindahkan sel `from` ke `to`. Sel tujuan yang terisi ditukar; yang kosong ditempati. */
  function moveCell(cells, from, to) {
    if (from === to) return cells;
    const out = cells.map((c) => ({ ...c }));
    const a = out.find((c) => c.index === from);
    if (!a) return cells;
    const b = out.find((c) => c.index === to);
    a.index = to;
    if (b) b.index = from;
    return out;
  }

  /** Ubah ukuran grid; sel yang jatuh di luar grid baru dibuang. Kembalikan {cells, dropped}. */
  function resizeGrid(cells, oldCols, newCols, newRows) {
    const out = [];
    let dropped = 0;
    for (const c of cells) {
      const row = Math.floor(c.index / oldCols);
      const col = c.index % oldCols;
      if (row < newRows && col < newCols) out.push({ ...c, index: row * newCols + col });
      else dropped++;
    }
    return { cells: out, dropped };
  }

  /** Nama tampilan → id aman untuk nama berkas (a-z, 0-9, '-', '_'). */
  function slug(name, taken) {
    let base = String(name).toLowerCase().normalize("NFKD").replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "").slice(0, 40) || "profil";
    let id = base;
    for (let n = 2; taken.includes(id); n++) id = `${base}-${n}`;
    return id;
  }

  /** Profil dari host → keadaan editor (sel dengan aksinya sendiri). */
  function toEditor(profile) {
    const byId = new Map(profile.actions.map((a) => [a.id, a.action]));
    return {
      id: profile.id,
      name: profile.name,
      cols: profile.cols,
      rows: profile.rows,
      cells: profile.buttons
        .filter((b) => byId.has(b.action_id))
        .map((b) => ({
          index: b.index,
          label: b.label,
          icon: b.icon ?? null,
          color: b.color ?? null,
          action: byId.get(b.action_id),
        })),
    };
  }

  /** Keadaan editor → profil untuk disimpan. Id aksi ikut posisi sel: `b<index>`. */
  function fromEditor(ed) {
    const cells = [...ed.cells].sort((a, b) => a.index - b.index);
    return {
      id: ed.id,
      name: ed.name,
      cols: ed.cols,
      rows: ed.rows,
      buttons: cells.map((c) => ({
        index: c.index,
        label: c.label,
        ...(c.icon ? { icon: c.icon } : {}),
        ...(c.color ? { color: c.color } : {}),
        action_id: `b${c.index}`,
      })),
      actions: cells.map((c) => ({ id: `b${c.index}`, action: c.action })),
    };
  }

  const api = { MEDIA_KEYS, ACTION_TYPES, LABELS, parseHotkey, parseStep, stepToText, buildAction, actionValue, moveCell, resizeGrid, slug, toEditor, fromEditor };
  if (typeof module !== "undefined" && module.exports) module.exports = api;
  else root.DeckLogic = api;
})(typeof window !== "undefined" ? window : globalThis);
