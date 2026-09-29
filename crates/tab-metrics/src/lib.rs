//! Sumber metrik sistem.
//!
//! Aturan dari protokol: field yang tidak bisa dibaca **dihilangkan**, bukan diisi nol.
//! Karena itu `capabilities()` jujur — `temps`/`gpu` bernilai `false` sampai ada sensor
//! sungguhan di baliknya, dan `sample()` tidak pernah mengisi `tcpu`/`gpu`.

use std::time::Instant;
use sysinfo::{Disks, MemoryRefreshKind, Networks, System};
use tab_protocol::message::{Disk, Metrics, Power};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MetricsError {
    #[error("sensor tidak tersedia: {0}")]
    Unavailable(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MetricsCaps {
    pub temps: bool,
    pub gpu: bool,
}

pub trait MetricsSource: Send {
    fn capabilities(&self) -> MetricsCaps;
    fn sample(&mut self) -> Result<Metrics, MetricsError>;
}

/// Angka tetap untuk test host dan UI.
#[derive(Debug, Clone)]
pub struct FakeMetrics {
    pub caps: MetricsCaps,
    pub cpu: Vec<f32>,
}

impl Default for FakeMetrics {
    fn default() -> Self {
        Self {
            caps: MetricsCaps::default(),
            cpu: vec![10.0, 20.0, 30.0, 40.0],
        }
    }
}

impl MetricsSource for FakeMetrics {
    fn capabilities(&self) -> MetricsCaps {
        self.caps
    }

    fn sample(&mut self) -> Result<Metrics, MetricsError> {
        let avg = self.cpu.iter().sum::<f32>() / self.cpu.len().max(1) as f32;
        Ok(Metrics {
            cpu: self.cpu.clone(),
            cpua: avg,
            mu: 8 << 30,
            mt: 16 << 30,
            su: 0,
            st: 0,
            disks: vec![Disk {
                n: "fake".into(),
                u: 100,
                t: 200,
            }],
            rx: 1000,
            tx: 500,
            pwr: Power {
                ac: true,
                pct: None,
            },
            tcpu: None,
            gpu: None,
        })
    }
}

/// Pembacaan nyata lewat `sysinfo`.
pub struct SysMetrics {
    sys: System,
    nets: Networks,
    disks: Disks,
    last: Instant,
}

impl SysMetrics {
    pub fn new() -> Self {
        let mut sys = System::new();
        // Pembacaan CPU pertama selalu 0; ambil satu sampel awal supaya sampel yang dikirim
        // ke client sudah bermakna.
        sys.refresh_cpu_usage();
        Self {
            sys,
            nets: Networks::new_with_refreshed_list(),
            disks: Disks::new_with_refreshed_list(),
            last: Instant::now(),
        }
    }
}

impl Default for SysMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsSource for SysMetrics {
    fn capabilities(&self) -> MetricsCaps {
        // Suhu dan GPU belum punya pembaca yang bisa dipercaya di kedua OS (v1): jujur
        // melaporkan `false` supaya client menyembunyikan gauge-nya.
        MetricsCaps {
            temps: false,
            gpu: false,
        }
    }

    fn sample(&mut self) -> Result<Metrics, MetricsError> {
        self.sys.refresh_cpu_usage();
        self.sys
            .refresh_memory_specifics(MemoryRefreshKind::everything());
        self.nets.refresh(true);
        self.disks.refresh(true);

        let elapsed = self.last.elapsed().as_secs_f64().max(0.001);
        self.last = Instant::now();

        let cpu: Vec<f32> = self.sys.cpus().iter().map(|c| c.cpu_usage()).collect();
        let cpua = if cpu.is_empty() {
            0.0
        } else {
            cpu.iter().sum::<f32>() / cpu.len() as f32
        };

        let (rx, tx) = self.nets.values().fold((0u64, 0u64), |(r, t), n| {
            (r + n.received(), t + n.transmitted())
        });

        let mut disks: Vec<Disk> = Vec::new();
        for d in self.disks.list() {
            if d.is_removable() || d.total_space() == 0 {
                continue;
            }
            let name = d.mount_point().to_string_lossy().into_owned();
            if disks.iter().any(|x| x.n == name) {
                continue;
            }
            disks.push(Disk {
                n: name,
                u: d.total_space() - d.available_space(),
                t: d.total_space(),
            });
        }

        Ok(Metrics {
            cpu,
            cpua,
            mu: self.sys.used_memory(),
            mt: self.sys.total_memory(),
            su: self.sys.used_swap(),
            st: self.sys.total_swap(),
            disks,
            rx: (rx as f64 / elapsed) as u64,
            tx: (tx as f64 / elapsed) as u64,
            pwr: read_power(),
            tcpu: None,
            gpu: None,
        })
    }
}

#[cfg(windows)]
fn read_power() -> Power {
    use windows_sys::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
    // SAFETY: struct diisi penuh oleh API bila mengembalikan nilai bukan nol.
    let mut st: SYSTEM_POWER_STATUS = unsafe { std::mem::zeroed() };
    if unsafe { GetSystemPowerStatus(&mut st) } == 0 {
        return Power {
            ac: true,
            pct: None,
        };
    }
    // 255 = tidak diketahui; 128 pada BatteryFlag = tidak ada baterai (desktop).
    let pct = (st.BatteryLifePercent <= 100 && st.BatteryFlag & 128 == 0)
        .then_some(st.BatteryLifePercent);
    Power {
        ac: st.ACLineStatus != 0,
        pct,
    }
}

#[cfg(target_os = "macos")]
fn read_power() -> Power {
    let out = std::process::Command::new("pmset")
        .args(["-g", "batt"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();
    let ac = !out.contains("Battery Power");
    let pct = out.split_whitespace().find_map(|w| {
        w.trim_end_matches([';', '%'])
            .parse::<u8>()
            .ok()
            .filter(|_| w.contains('%'))
    });
    Power { ac, pct }
}

#[cfg(not(any(windows, target_os = "macos")))]
fn read_power() -> Power {
    Power {
        ac: true,
        pct: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_omits_optional_sensors() {
        let m = FakeMetrics::default().sample().unwrap();
        assert!(m.tcpu.is_none() && m.gpu.is_none());
        assert_eq!(m.cpua, 25.0);
    }

    #[test]
    fn real_sample_is_sane_and_honest() {
        let mut src = SysMetrics::new();
        let caps = src.capabilities();
        std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
        let m = src.sample().unwrap();
        assert!(!m.cpu.is_empty());
        assert!(m.mt > 0 && m.mu <= m.mt);
        assert!(m.cpu.iter().all(|c| (0.0..=100.0).contains(c)));
        // Kapabilitas dan isi sampel harus konsisten.
        assert_eq!(m.tcpu.is_some(), caps.temps);
        assert_eq!(m.gpu.is_some(), caps.gpu);
    }
}
