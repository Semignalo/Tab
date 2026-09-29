const test = require("node:test");
const assert = require("node:assert/strict");
const L = require("../src/deck-logic.js");

test("hotkey diparse dan diformat balik", () => {
  assert.deepEqual(L.parseHotkey(" Ctrl + Shift + P "), ["ctrl", "shift", "p"]);
  assert.throws(() => L.parseHotkey("  + "));
  assert.equal(L.actionValue(L.buildAction("hotkey", "ctrl+c")), "ctrl+c");
});

test("URL hanya http/https/mailto", () => {
  assert.deepEqual(L.buildAction("open_url", "https://example.com"), { type: "open_url", url: "https://example.com" });
  for (const bad of ["file:///C:/x", "javascript:alert(1)", "example.com", ""]) {
    assert.throws(() => L.buildAction("open_url", bad), undefined, bad);
  }
});

test("tombol media harus dari daftar kanonik", () => {
  assert.deepEqual(L.buildAction("media_key", "mute"), { type: "media_key", key: "mute" });
  assert.throws(() => L.buildAction("media_key", "rm -rf"));
});

test("multi: parse baris dan bolak-balik teks", () => {
  const a = L.buildAction("multi", "hotkey: ctrl+c\nurl: https://a.b\n\nmedia: mute");
  assert.equal(a.steps.length, 3);
  assert.equal(L.actionValue(a), "hotkey: ctrl+c\nurl: https://a.b\nmedia: mute");
  assert.throws(() => L.buildAction("multi", "tanpa titik dua"));
  assert.throws(() => L.buildAction("multi", "ngawur: x"));
  assert.throws(() => L.buildAction("multi", "  \n "));
});

test("moveCell menukar sel terisi dan memindah ke sel kosong", () => {
  const cells = [{ index: 0, label: "a" }, { index: 1, label: "b" }];
  const swapped = L.moveCell(cells, 0, 1);
  assert.deepEqual(swapped.map((c) => [c.label, c.index]), [["a", 1], ["b", 0]]);
  const moved = L.moveCell(cells, 0, 5);
  assert.deepEqual(moved.map((c) => [c.label, c.index]), [["a", 5], ["b", 1]]);
  assert.equal(L.moveCell(cells, 3, 4), cells, "sumber kosong: tidak berubah");
  assert.notEqual(swapped, cells, "tidak memutasi masukan");
  assert.equal(cells[0].index, 0);
});

test("resizeGrid menjaga posisi baris/kolom dan membuang yang keluar", () => {
  // grid 3 kolom: sel di (row1,col2) = indeks 5
  const cells = [{ index: 0 }, { index: 5 }, { index: 3 }];
  const { cells: out, dropped } = L.resizeGrid(cells, 3, 2, 2);
  assert.equal(dropped, 1); // (1,2) keluar dari 2 kolom
  assert.deepEqual(out.map((c) => c.index).sort(), [0, 2]); // (1,0) → 2 di grid 2 kolom
});

test("slug aman untuk nama berkas dan unik", () => {
  assert.equal(L.slug("Kerja Keras!", []), "kerja-keras");
  assert.equal(L.slug("../../etc", []), "etc");
  assert.equal(L.slug("!!!", []), "profil");
  assert.equal(L.slug("Media", ["media", "media-2"]), "media-3");
  assert.match(L.slug("Ünïcode ÅÄÖ", []), /^[a-z0-9_-]+$/);
});

test("toEditor/fromEditor bolak-balik dan id aksi mengikuti posisi", () => {
  const profile = {
    id: "p", name: "P", cols: 2, rows: 1,
    buttons: [{ index: 1, label: "x", icon: "mic", action_id: "zzz" }],
    actions: [{ id: "zzz", action: { type: "media_key", key: "mute" } }],
  };
  const ed = L.toEditor(profile);
  assert.equal(ed.cells[0].action.key, "mute");
  const back = L.fromEditor(ed);
  assert.equal(back.buttons[0].action_id, "b1");
  assert.equal(back.actions[0].id, "b1");
  assert.equal(back.buttons[0].icon, "mic");
  assert.ok(!("color" in back.buttons[0]), "color kosong tidak ikut dikirim");
});
