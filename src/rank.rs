use crate::graph::Graph;

pub struct RankRun {
    pub ranks: Vec<f64>,
    pub iterations: u32,
    pub converged: bool,
}

const REDUCTION_BLOCK: usize = 1024;
const ALPHA: f64 = 0.85;
const TOLERANCE: f64 = 1e-6;
const MAX_ITERATIONS: u32 = 100;

fn probability_mass(values: &[f64]) -> f64 {
    let mut total = 0.0;
    let mut block = 0;
    while block < values.len() {
        let end = (block + REDUCTION_BLOCK).min(values.len());
        let mut partial = 0.0;
        for v in &values[block..end] {
            partial += v;
        }
        total += partial;
        block += REDUCTION_BLOCK;
    }
    total
}

/// Personalized PageRank, single-threaded, fixed-block canonical reductions, strict IEEE.
pub fn pagerank(g: &Graph) -> RankRun {
    let n = g.row_offsets.len() - 1;
    if n == 0 {
        return RankRun { ranks: vec![], iterations: 0, converged: true };
    }
    let teleport = vec![1.0 / n as f64; n];
    let mut current: Vec<f64> = teleport.clone();
    let mut next = vec![0.0f64; n];
    let mut scaled = vec![0.0f64; n];

    let mut iter = 0u32;
    let mut converged = false;
    while iter < MAX_ITERATIONS {
        // Dangling mass: fixed blocks, canonical fold.
        let mut dangling = 0.0;
        let mut block = 0;
        while block < n {
            let end = (block + REDUCTION_BLOCK).min(n);
            let mut partial = 0.0;
            for i in block..end {
                if g.w_out_deg[i] <= 0.0 {
                    partial += current[i];
                }
            }
            dangling += partial;
            block += REDUCTION_BLOCK;
        }

        for i in 0..n {
            scaled[i] = if g.w_out_deg[i] > 0.0 { current[i] / g.w_out_deg[i] } else { 0.0 };
        }

        let teleport_scale = ALPHA * dangling + (1.0 - ALPHA);
        for t in 0..n {
            let mut incoming = 0.0;
            for e in g.row_offsets[t] as usize..g.row_offsets[t + 1] as usize {
                incoming += g.values[e] as f64 * scaled[g.col_indices[e] as usize];
            }
            next[t] = ALPHA * incoming + teleport_scale * teleport[t];
        }

        // L1 residual: fixed blocks.
        let mut residual = 0.0;
        let mut block = 0;
        while block < n {
            let end = (block + REDUCTION_BLOCK).min(n);
            let mut partial = 0.0;
            for i in block..end {
                partial += (next[i] - current[i]).abs();
            }
            residual += partial;
            block += REDUCTION_BLOCK;
        }

        std::mem::swap(&mut current, &mut next);
        if residual < TOLERANCE {
            iter += 1;
            converged = true;
            break;
        }
        iter += 1;
    }

    RankRun { ranks: current, iterations: iter, converged }
}