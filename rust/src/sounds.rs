//! Compatible deterministic per-person notification motifs and PCM WAV.
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
const RATE: usize = 32000;
const SCALE: [i32; 5] = [0, 2, 4, 7, 9];
struct Dice(u64);
impl Dice {
    fn new(text: &str) -> Self {
        let hash = Sha256::digest(text.as_bytes());
        Self(u64::from_be_bytes(
            hash[..8].try_into().expect("SHA256 prefix"),
        ))
    }
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }
    fn unit(&mut self) -> f64 {
        self.next() as f64 / 18446744073709551616.0
    }
    fn between(&mut self, low: f64, high: f64) -> f64 {
        low + (high - low) * self.unit()
    }
    fn pick(&mut self, options: &[i32]) -> i32 {
        options[self.next() as usize % options.len()]
    }
}
fn round(value: f64, places: i32) -> f64 {
    let factor = 10f64.powi(places);
    (value * factor).round_ties_even() / factor
}
pub fn motif(seed: &str, variant: u64) -> Value {
    let mut dice = Dice::new(&format!("omagram-sound:{seed}:{variant}"));
    let root = 62 + (dice.next() % 11) as i32;
    let count = dice.pick(&[2, 2, 3, 3, 3]);
    let gap = dice.between(0.085, 0.15);
    let swing = dice.between(0.0, 0.3);
    let mut degrees = vec![dice.pick(&[0, 1, 2, 2, 3, 4])];
    for _ in 1..count {
        degrees.push((degrees.last().unwrap() + dice.pick(&[-2, -1, 1, 1, 2, 2, 3])).clamp(0, 7));
    }
    let last = *degrees.last().unwrap();
    if ![0, 2, 3].contains(&(last % 5)) {
        let candidates: Vec<_> = [last + 1, last - 1]
            .into_iter()
            .filter(|d| (0..8).contains(d) && [0, 2, 3].contains(&(d % 5)))
            .collect();
        *degrees.last_mut().unwrap() = dice.pick(&candidates);
    }
    if degrees.iter().all(|d| *d == degrees[0]) {
        degrees[0] += if degrees[0] + 1 < 8 { 1 } else { -1 };
    }
    let mut at = 0.0;
    let mut notes = Vec::new();
    for (i, degree) in degrees.into_iter().enumerate() {
        if i > 0 {
            at += gap
                * if i % 2 == 1 {
                    1.0 + swing
                } else {
                    1.0 - swing / 2.0
                };
        }
        notes.push(json!({"midi":root+12*(degree/5)+SCALE[(degree%5) as usize],"degree":degree,"at":round(at,4),"velocity":round(dice.between(0.72,1.0),3)}));
    }
    if let Some(note) = notes.last_mut() {
        note["velocity"] = json!(note["velocity"].as_f64().unwrap().max(0.9));
    }
    json!({"seed":seed,"variant":variant,"root":root,"notes":notes})
}
fn noise(dice: &mut Dice, y: &mut f64, cutoff: f64) -> f64 {
    let a = (-2.0 * std::f64::consts::PI * cutoff / RATE as f64).exp();
    *y = (1.0 - a) * dice.between(-1.0, 1.0) + a * *y;
    *y
}
pub fn render(m: &Value, style: &str) -> Result<Vec<u8>, &'static str> {
    let (ring, spacing, shift, cutoff) = match style {
        "pop" => (0.25, 1.0, -9.0, 3000.0),
        "drop" => (0.32, 1.1, -5.0, 2400.0),
        "knock" => (0.2, 0.95, -9.0, 3500.0),
        _ => return Err("no such sound style"),
    };
    let notes = m["notes"]
        .as_array()
        .filter(|a| !a.is_empty() && a.len() <= 3)
        .ok_or("invalid sound motif")?;
    let last = notes.last().unwrap()["at"].as_f64().ok_or("invalid note")? * spacing;
    let mut out = vec![0.0; ((last + ring + 0.06).min(1.2) * RATE as f64) as usize];
    let mut dice = Dice::new(&format!(
        "omagram-noise:{}:{}",
        m["seed"].as_str().unwrap_or(""),
        m["variant"].as_u64().unwrap_or(0)
    ));
    let pi = std::f64::consts::PI;
    for note in notes {
        let start = (note["at"].as_f64().ok_or("invalid note")? * spacing * RATE as f64) as usize;
        if start >= out.len() {
            continue;
        }
        let freq =
            440.0 * 2f64.powf((note["midi"].as_f64().ok_or("invalid note")? + shift - 69.0) / 12.0);
        let velocity = note["velocity"].as_f64().ok_or("invalid note")?;
        let mut phase = 0.0;
        let mut y = 0.0;
        for n in 0..(out.len() - start).min((ring * RATE as f64) as usize) {
            let t = n as f64 / RATE as f64;
            let value = match style {
                "pop" => {
                    let mut value = if t < 0.02 {
                        1.2 * (-t / 0.004).exp() * noise(&mut dice, &mut y, 1500.0)
                    } else {
                        0.0
                    };
                    if t >= 0.005 {
                        let u = t - 0.005;
                        phase += 2.0 * pi * freq * (1.0 + 0.5 * (-u / 0.008).exp()) / RATE as f64;
                        let body = (u / 0.002).min(1.0) * (-u / 0.04).exp();
                        value +=
                            body * (phase.sin() + 0.18 * (-u / 0.01).exp() * (2.0 * phase).sin());
                    }
                    value
                }
                "drop" => {
                    phase += 2.0 * pi * freq * (1.0 - 0.3 * (-t / 0.02).exp()) / RATE as f64;
                    (t / 0.004).min(1.0)
                        * (-t / 0.055).exp()
                        * (phase.sin() + 0.1 * (2.0 * phase).sin())
                }
                _ => {
                    let mut value = 0.0;
                    for (ratio, level, tau) in
                        [(1.0, 1.0, 0.035), (2.57, 0.4, 0.014), (4.9, 0.12, 0.006)]
                    {
                        let w = 2.0 * pi * freq * ratio / RATE as f64;
                        value += level * (-t / tau).exp() * (w * n as f64).sin();
                    }
                    if t < 0.03 {
                        value += 2.0 * (-t / 0.006).exp() * noise(&mut dice, &mut y, 700.0);
                    }
                    value * (t / 0.0008).min(1.0)
                }
            };
            out[start + n] += velocity * value;
        }
    }
    let a = (-2.0 * pi * cutoff / RATE as f64).exp();
    let mut y = 0.0;
    for value in &mut out {
        y = (1.0 - a) * *value + a * y;
        *value = y;
    }
    let peak = out
        .iter()
        .map(|v| v.abs())
        .fold(0.0, f64::max)
        .max(f64::MIN_POSITIVE);
    for value in &mut out {
        *value = (1.4 * *value / peak).tanh() / 1.4f64.tanh();
    }
    let peak = out
        .iter()
        .map(|v| v.abs())
        .fold(0.0, f64::max)
        .max(f64::MIN_POSITIVE);
    let total = out.len();
    let fade = 1920.min(total).max(1);
    for (i, value) in out.iter_mut().enumerate() {
        let mut gain = 0.28 / peak;
        if i < 64 {
            gain *= i as f64 / 64.0;
        }
        if i >= total - fade {
            gain *= 0.5 + 0.5 * (pi * (i - (total - fade) + 1) as f64 / fade as f64).cos();
        }
        *value *= gain;
    }
    let bytes = (out.len() * 2) as u32;
    let mut wav = Vec::with_capacity(44 + bytes as usize);
    wav.extend(b"RIFF");
    wav.extend((bytes + 36).to_le_bytes());
    wav.extend(b"WAVEfmt ");
    wav.extend(16u32.to_le_bytes());
    wav.extend(1u16.to_le_bytes());
    wav.extend(1u16.to_le_bytes());
    wav.extend((RATE as u32).to_le_bytes());
    wav.extend((RATE as u32 * 2).to_le_bytes());
    wav.extend(2u16.to_le_bytes());
    wav.extend(16u16.to_le_bytes());
    wav.extend(b"data");
    wav.extend(bytes.to_le_bytes());
    for value in out {
        wav.extend(((value.clamp(-1.0, 1.0) * 32767.0).round_ties_even() as i16).to_le_bytes());
    }
    Ok(wav)
}
pub fn project(value: &Value) -> Value {
    let seed = value["seed"]
        .as_i64()
        .map(|v| v.to_string())
        .unwrap_or_else(|| value["seed"].as_str().unwrap_or("0").into());
    let m = motif(&seed, value["variant"].as_u64().unwrap_or(0));
    if value["operation"] == "wav" {
        match render(&m, value["style"].as_str().unwrap_or("drop")) {
            Ok(wav) => {
                use base64::Engine;
                json!(base64::engine::general_purpose::STANDARD.encode(wav))
            }
            Err(error) => json!({"error":error}),
        }
    } else {
        m
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deterministic_sound_is_bounded_mono_pcm() {
        let m = motif("7", 0);
        assert_eq!(m, motif("7", 0));
        for style in ["pop", "drop", "knock"] {
            let wav = render(&m, style).unwrap();
            assert_eq!(&wav[..4], b"RIFF");
            assert!(wav.len() <= 44 + 32000 * 2 * 12 / 10);
            assert_eq!(u32::from_le_bytes(wav[24..28].try_into().unwrap()), 32000);
        }
    }
}
