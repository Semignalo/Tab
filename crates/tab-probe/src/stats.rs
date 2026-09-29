//! Statistik latensi. Alat ukur harus jujur: persentil dihitung dari data mentah, tidak
//! di-smoothing, dan nilai terburuk tidak dibuang.

#[derive(Debug, Clone, PartialEq)]
pub struct Summary {
    pub count: usize,
    pub min: f64,
    pub p50: f64,
    pub p95: f64,
    pub p99: f64,
    pub max: f64,
    pub mean: f64,
}

/// Persentil metode nearest-rank pada data terurut: p ∈ (0, 100].
fn percentile(sorted: &[f64], p: f64) -> f64 {
    let rank = ((p / 100.0) * sorted.len() as f64).ceil() as usize;
    sorted[rank.clamp(1, sorted.len()) - 1]
}

pub fn summarize(samples_ms: &[f64]) -> Option<Summary> {
    if samples_ms.is_empty() {
        return None;
    }
    let mut v = samples_ms.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    Some(Summary {
        count: v.len(),
        min: v[0],
        p50: percentile(&v, 50.0),
        p95: percentile(&v, 95.0),
        p99: percentile(&v, 99.0),
        max: v[v.len() - 1],
        mean: v.iter().sum::<f64>() / v.len() as f64,
    })
}

/// Hitung batch yang hilang dari celah pada urutan `InputBatch.s` yang diterima host.
pub fn count_gaps(seqs: &[u64]) -> u64 {
    seqs.windows(2)
        .map(|w| w[1].saturating_sub(w[0]).saturating_sub(1))
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentiles_use_nearest_rank() {
        let data: Vec<f64> = (1..=100).map(|x| x as f64).collect();
        let s = summarize(&data).unwrap();
        assert_eq!(
            (s.p50, s.p95, s.p99, s.max, s.min),
            (50.0, 95.0, 99.0, 100.0, 1.0)
        );
    }

    #[test]
    fn single_sample_and_empty() {
        let s = summarize(&[4.2]).unwrap();
        assert_eq!((s.p50, s.p99, s.max), (4.2, 4.2, 4.2));
        assert!(summarize(&[]).is_none());
    }

    #[test]
    fn worst_case_is_never_discarded() {
        let mut data = vec![1.0; 99];
        data.push(500.0);
        let s = summarize(&data).unwrap();
        assert_eq!(s.max, 500.0);
        assert_eq!(s.p99, 1.0, "p99 dari 100 sampel adalah sampel ke-99");
        data.push(500.0);
        assert_eq!(summarize(&data).unwrap().p99, 500.0);
    }

    #[test]
    fn gaps_are_counted() {
        assert_eq!(count_gaps(&[1, 2, 3, 4]), 0);
        assert_eq!(count_gaps(&[1, 2, 5, 6, 10]), 2 + 3);
        assert_eq!(count_gaps(&[]), 0);
    }
}
