// UI host Tab — JS murni tanpa bundler. Semua teks dari host dimasukkan lewat textContent
// (bukan innerHTML), sehingga nama perangkat yang dikirim HP tidak bisa menyuntikkan markup.
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const $ = (id) => document.getElementById(id);
const el = (tag, cls, text) => {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text !== undefined) e.textContent = text;
  return e;
};

let pairTimer = null;

// ------------------------------------------------------------------ status

async function refreshStatus() {
  const s = await invoke("status");
  const pill = $("pill");
  if (s.running) {
    pill.textContent = "Berjalan";
    pill.className = "pill ok";
    $("host-line").textContent = `${s.name} · ${s.os} · v${s.version}`;
  } else {
    pill.textContent = "Berhenti";
    pill.className = "pill bad";
    $("host-line").textContent = s.name;
  }
  const err = $("error");
  err.hidden = !s.error;
  err.textContent = s.error || "";
  $("pair-btn").disabled = !s.running;

  $("addresses").textContent = s.running && s.addresses.length
    ? `Alamat: ${s.addresses.join(", ")} · TCP ${s.session_port} · UDP ${s.discovery_port} · sidik jari ${s.fingerprint}`
    : "";
  return s;
}

// --------------------------------------------------------------- perangkat

const fmtAgo = (secs) => {
  if (!secs) return "belum pernah";
  const d = Math.max(0, Math.floor(Date.now() / 1000) - secs);
  if (d < 60) return "baru saja";
  if (d < 3600) return `${Math.floor(d / 60)} menit lalu`;
  if (d < 86400) return `${Math.floor(d / 3600)} jam lalu`;
  return `${Math.floor(d / 86400)} hari lalu`;
};

async function refreshDevices() {
  let list = [];
  try { list = await invoke("devices"); } catch { /* host belum siap */ }
  const ul = $("devices");
  ul.replaceChildren();
  $("devices-empty").hidden = list.length > 0;
  for (const d of list) {
    const li = el("li");
    li.append(el("span", "dot" + (d.online ? " on" : "")));
    const grow = el("div", "grow");
    grow.append(el("b", "", d.name || "(tanpa nama)"));
    grow.append(el("small", "", `${d.platform} · ${d.online ? "tersambung" : "terakhir " + fmtAgo(d.last_seen)}`));
    li.append(grow);
    const btn = el("button", "danger", "Cabut");
    btn.onclick = async () => {
      if (!confirm(`Cabut akses “${d.name}”? HP itu harus dipasangkan ulang dengan PIN.`)) return;
      await invoke("revoke", { id: d.id });
      refreshDevices();
    };
    li.append(btn);
    ul.append(li);
  }
}

// ----------------------------------------------------------------- pairing

async function startPairing() {
  try {
    const { pin, ttl_secs } = await invoke("begin_pairing");
    showPin(pin, ttl_secs);
  } catch (e) {
    alert(String(e));
  }
}

function showPin(pin, ttl) {
  $("pin").textContent = pin.slice(0, 3) + " " + pin.slice(3);
  const dlg = $("pair-dialog");
  if (!dlg.open) dlg.showModal();
  let left = ttl;
  clearInterval(pairTimer);
  const tick = () => {
    $("pin-status").textContent = left > 0 ? `Berlaku ${left} detik lagi` : "PIN kedaluwarsa.";
    if (left-- <= 0) clearInterval(pairTimer);
  };
  tick();
  pairTimer = setInterval(tick, 1000);
}

function closePairDialog() {
  clearInterval(pairTimer);
  const dlg = $("pair-dialog");
  if (dlg.open) dlg.close();
}

$("pair-btn").onclick = startPairing;
$("pair-cancel").onclick = async () => {
  await invoke("cancel_pairing");
  closePairDialog();
};

// ------------------------------------------------------------------ setelan

async function loadSettings() {
  const s = await invoke("get_settings");
  $("sens").value = s.sens;
  $("sens-val").textContent = `×${Number(s.sens).toFixed(1)}`;
  $("natural").checked = s.natural;
  $("lyrics").value = s.lyrics_dir || "";
  $("obs-enabled").checked = !!s.obs_enabled;
  $("obs-host").value = s.obs_host || "127.0.0.1";
  $("obs-port").value = s.obs_port || 4455;
  $("obs-fields").hidden = !s.obs_enabled;
  $("autostart").checked = await invoke("autostart_enabled");
}

$("obs-enabled").onchange = () => { $("obs-fields").hidden = !$("obs-enabled").checked; };

$("obs-test").onclick = async () => {
  const msg = $("settings-msg");
  try {
    await saveAll();
    msg.textContent = await invoke("obs_test");
  } catch (e) {
    msg.textContent = String(e);
  }
};

$("sens").oninput = () => { $("sens-val").textContent = `×${Number($("sens").value).toFixed(1)}`; };

async function saveAll() {
  await invoke("save_settings", {
    new: {
      sens: Number($("sens").value),
      natural: $("natural").checked,
      lyrics_dir: $("lyrics").value.trim() || null,
      obs_enabled: $("obs-enabled").checked,
      obs_host: $("obs-host").value.trim(),
      obs_port: Number($("obs-port").value) || 4455,
    },
  });
  const pw = $("obs-password").value;
  if (pw) {
    await invoke("obs_set_password", { password: pw });
    $("obs-password").value = "";
  }
  await invoke("set_autostart", { enabled: $("autostart").checked });
}

$("save-settings").onclick = async () => {
  const msg = $("settings-msg");
  try {
    await saveAll();
    msg.textContent = "Tersimpan.";
  } catch (e) {
    msg.textContent = String(e);
  }
  setTimeout(() => (msg.textContent = ""), 4000);
};

// ----------------------------------------------------------- preflight/izin

async function runChecks(status) {
  const ul = $("checks");
  ul.replaceChildren();
  const add = (state, title, detail, action) => {
    const li = el("li");
    li.append(el("span", "dot " + state));
    const g = el("div", "grow");
    g.append(el("b", "", title));
    g.append(el("small", "", detail));
    li.append(g);
    if (action) {
      const b = el("button", "", action.label);
      b.onclick = action.run;
      li.append(b);
    }
    ul.append(li);
  };

  add(
    status.running ? "on" : "bad",
    "Layanan Tab",
    status.running ? "Mendengarkan koneksi dari HP." : (status.error || "Tidak berjalan."),
  );
  add(
    status.addresses.length ? "on" : "warn",
    "Jaringan",
    status.addresses.length ? `Terhubung ke ${status.addresses.length} jaringan.` : "Tidak ada jaringan aktif — sambungkan ke Wi-Fi yang sama dengan HP.",
  );

  const fw = await invoke("firewall_status");
  if (fw.applicable) {
    add(fw.ok ? "on" : "bad", "Windows Firewall", fw.detail, fw.ok ? null : {
      label: "Perbaiki",
      run: async () => {
        try { await invoke("firewall_fix"); } catch (e) { alert(String(e)); }
        runChecks(await refreshStatus());
      },
    });
  }

  if (status.input_permission !== "not_required") {
    add(
      status.input_permission === "granted" ? "on" : "bad",
      "Izin Accessibility",
      status.input_permission === "granted"
        ? "Tab boleh menggerakkan kursor dan mengetik."
        : "Tanpa izin ini kursor tidak bergerak. Buka Pengaturan Sistem → Privasi → Aksesibilitas dan aktifkan Tab.",
      status.input_permission === "granted" ? null : {
        label: "Buka pengaturan",
        run: () => invoke("open_permission_settings"),
      },
    );
  }
}

$("recheck").onclick = async () => runChecks(await refreshStatus());

// -------------------------------------------------------------------- event

listen("host-event", async (e) => {
  const ev = e.payload;
  switch (ev.kind) {
    case "started":
    case "start_failed":
      runChecks(await refreshStatus());
      refreshDevices();
      break;
    case "request_pairing":
      startPairing();
      break;
    case "pairing_started":
      showPin(ev.pin, Math.round(ev.ttl_ms / 1000));
      break;
    case "pairing_failed":
      $("pin-status").textContent = `PIN salah (sisa ${ev.attempts_left} percobaan)`;
      break;
    case "pairing_succeeded":
      closePairDialog();
      refreshDevices();
      break;
    case "pairing_ended":
      closePairDialog();
      break;
    case "device_connected":
    case "device_disconnected":
    case "device_revoked":
      refreshDevices();
      break;
  }
});

(async () => {
  await loadSettings();
  const s = await refreshStatus();
  await runChecks(s);
  await refreshDevices();
  setInterval(refreshDevices, 15000); // memperbarui "terakhir dilihat"
})();
