//! Cetak metrik nyata tiap detik untuk dibandingkan dengan Task Manager / Activity Monitor.

use tab_metrics::{MetricsSource, SysMetrics};

fn main() {
    let mut src = SysMetrics::new();
    for _ in 0..5 {
        std::thread::sleep(std::time::Duration::from_secs(1));
        let m = src.sample().expect("sampel gagal");
        println!(
            "cpu {:.1}% ({} core) | mem {:.1}/{:.1} GiB | swap {}/{} MiB | rx {} KB/s tx {} KB/s | ac={} pct={:?}",
            m.cpua,
            m.cpu.len(),
            m.mu as f64 / 1073741824.0,
            m.mt as f64 / 1073741824.0,
            m.su >> 20,
            m.st >> 20,
            m.rx / 1024,
            m.tx / 1024,
            m.pwr.ac,
            m.pwr.pct,
        );
        for d in &m.disks {
            println!(
                "   disk {} {:.1}/{:.1} GiB",
                d.n,
                d.u as f64 / 1073741824.0,
                d.t as f64 / 1073741824.0
            );
        }
    }
}
