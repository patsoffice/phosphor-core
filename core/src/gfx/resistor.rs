//! Resistor-weighted DAC palette computation.
//!
//! Many arcade machines generate RGB palette values by driving resistor networks
//! from PROM data. This module provides two computation models:
//!
//! - **Linear weights** ([`compute_resistor_weights`] + [`combine_weights`]):
//!   Pre-compute per-bit weights auto-scaled so all-bits-on = 255, then combine
//!   via linear sum. Matches MAME's `compute_resistor_weights` convention. Works
//!   well when the pulldown resistance is large relative to the DAC resistors
//!   (e.g., Namco Pac-Man, Crystal Castles).
//!
//! - **Exact voltage divider** ([`compute_resistor_net`]): Computes the actual
//!   conductance ratio for each combination of bits, including the pulldown.
//!   Physically accurate but NOT auto-scaled to 255. Used when the pulldown is
//!   strong enough that the linear approximation breaks down (e.g., DK/TKG-04).

/// Compute per-bit weights for a resistor-weighted DAC, auto-scaled so that
/// all-bits-on produces 255.
///
/// Each element in `resistors` is a resistance value in ohms (weakest-first by
/// convention, but any order works). The returned `Vec` has the same length as
/// `resistors`, with `weights[i]` corresponding to `resistors[i]`.
///
/// - Without pulldown: `weight_i = (1/R_i) / sum(1/R_j) * 255`
/// - With pulldown: `weight_i = (pd/(R_i+pd)) / sum(pd/(R_j+pd)) * 255`
pub fn compute_resistor_weights(resistors: &[f64], pulldown: Option<f64>) -> Vec<f64> {
    match pulldown {
        None => {
            let total: f64 = resistors.iter().map(|r| 1.0 / r).sum();
            resistors
                .iter()
                .map(|r| (1.0 / r) / total * 255.0)
                .collect()
        }
        Some(pd) => {
            let raw: Vec<f64> = resistors.iter().map(|r| pd / (r + pd)).collect();
            let sum: f64 = raw.iter().sum();
            raw.iter().map(|w| w / sum * 255.0).collect()
        }
    }
}

/// Combine pre-computed weights with individual bit values (0 or 1) into an
/// 8-bit color value.
///
/// `weights` and `bits` must have the same length. Returns
/// `round(sum(weight_i * bit_i))` clamped to 0–255.
pub fn combine_weights(weights: &[f64], bits: &[u8]) -> u8 {
    let val: f64 = weights.iter().zip(bits).map(|(w, &b)| *w * b as f64).sum();
    val.round().min(255.0) as u8
}

/// Compute an 8-bit color value from the exact resistor voltage divider.
///
/// Unlike the linear weight model, this accounts for the nonlinear interaction
/// between the pulldown and active-bit conductances. The result is NOT auto-scaled:
/// all-bits-on gives a value < 255 when a pulldown is present.
///
/// `bits` contains the analog drive level for each resistor (typically 0.0 or 1.0).
/// `resistors` contains the corresponding resistance values in ohms.
/// `pulldown` is the pulldown resistance to ground in ohms.
///
/// Formula: `V = sum(bit_i/R_i) / (sum(bit_i/R_i) + 1/pulldown)`, scaled to 0–255.
pub fn compute_resistor_net(bits: &[f64], resistors: &[f64], pulldown: f64) -> u8 {
    let active: f64 = bits.iter().zip(resistors).map(|(b, r)| b / r).sum();
    let total = active + 1.0 / pulldown;
    let voltage = if total > 0.0 { active / total } else { 0.0 };
    (voltage * 255.0).round().min(255.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn namco_weights_match_original() {
        // Namco Pac-Man / Galaga: 3-bit R/G (1K/470/220Ω, no pulldown)
        let w = compute_resistor_weights(&[1000.0, 470.0, 220.0], None);
        assert_eq!(w.len(), 3);
        // All bits on should sum to 255
        assert_eq!(combine_weights(&w, &[1, 1, 1]), 255);
        // Individual bits: verify against original compute_resistor_scale output
        assert_eq!(combine_weights(&w, &[1, 0, 0]), 33);
        assert_eq!(combine_weights(&w, &[0, 1, 0]), 71);
        assert_eq!(combine_weights(&w, &[0, 0, 1]), 151);
    }

    #[test]
    fn namco_2bit_blue_weights() {
        // Namco Pac-Man / Galaga: 2-bit B (470/220Ω, no pulldown)
        let w = compute_resistor_weights(&[470.0, 220.0], None);
        assert_eq!(combine_weights(&w, &[1, 1]), 255);
        assert_eq!(combine_weights(&w, &[1, 0]), 81);
        assert_eq!(combine_weights(&w, &[0, 1]), 174);
    }

    #[test]
    fn crystal_castles_weights_match_original() {
        // Crystal Castles: 3-bit (22K/10K/4.7KΩ + 1K pulldown)
        let w = compute_resistor_weights(&[22_000.0, 10_000.0, 4_700.0], Some(1_000.0));
        // Original constants: [36, 75, 144]
        assert_eq!(combine_weights(&w, &[1, 0, 0]), 36);
        assert_eq!(combine_weights(&w, &[0, 1, 0]), 75);
        assert_eq!(combine_weights(&w, &[0, 0, 1]), 144);
        assert_eq!(combine_weights(&w, &[1, 1, 1]), 255);
    }

    #[test]
    fn tkg04_darlington_3bit_match() {
        // TKG-04 Darlington: 1K/470/220Ω + 470Ω pulldown, all bits on → 200
        assert_eq!(
            compute_resistor_net(&[1.0, 1.0, 1.0], &[1000.0, 470.0, 220.0], 470.0),
            200
        );
        // All bits off → 0
        assert_eq!(
            compute_resistor_net(&[0.0, 0.0, 0.0], &[1000.0, 470.0, 220.0], 470.0),
            0
        );
        // Single bit (weakest, 1KΩ)
        assert_eq!(
            compute_resistor_net(&[1.0, 0.0, 0.0], &[1000.0, 470.0, 220.0], 470.0),
            82
        );
    }

    #[test]
    fn tkg04_emitter_2bit_match() {
        // TKG-04 Emitter follower: 470/220Ω + 680Ω pulldown, all bits on → 209
        assert_eq!(
            compute_resistor_net(&[1.0, 1.0], &[470.0, 220.0], 680.0),
            209
        );
        assert_eq!(compute_resistor_net(&[0.0, 0.0], &[470.0, 220.0], 680.0), 0);
    }

    #[test]
    fn all_zeros_is_black() {
        let w = compute_resistor_weights(&[1000.0, 470.0, 220.0], None);
        assert_eq!(combine_weights(&w, &[0, 0, 0]), 0);

        let w2 = compute_resistor_weights(&[22_000.0, 10_000.0, 4_700.0], Some(1_000.0));
        assert_eq!(combine_weights(&w2, &[0, 0, 0]), 0);
    }
}
