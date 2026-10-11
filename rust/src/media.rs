//! Native ffmpeg preparation contract. Callers validate descriptor-anchored
//! source/output membership before executing these explicit argument vectors.
use crate::process;
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use std::{collections::BTreeMap, io, time::Duration};
const PREFIX: [&str; 4] = ["-hide_banner", "-loglevel", "error", "-nostdin"];
fn argv(tail: &[&str]) -> Vec<String> {
    PREFIX.iter().chain(tail).map(|s| (*s).into()).collect()
}
pub fn voice_argv(path: &str) -> Vec<String> {
    argv(&[
        "-f",
        "pulse",
        "-i",
        "default",
        "-t",
        "600",
        "-ac",
        "1",
        "-ar",
        "48000",
        "-c:a",
        "libopus",
        "-b:a",
        "32k",
        "-application",
        "voip",
        "-f",
        "ogg",
        path,
    ])
}
pub fn video_note_argv(source: &str, target: &str) -> Vec<String> {
    argv(&[
        "-i",
        source,
        "-t",
        "60",
        "-vf",
        "crop='min(iw,ih)':'min(iw,ih)',scale=480:480,fps=30",
        "-c:v",
        "libx264",
        "-preset",
        "veryfast",
        "-crf",
        "26",
        "-pix_fmt",
        "yuv420p",
        "-c:a",
        "aac",
        "-b:a",
        "64k",
        "-ac",
        "1",
        "-movflags",
        "+faststart",
        target,
    ])
}
pub fn note_animation_argv(source: &str, target: &str) -> Vec<String> {
    argv(&[
        "-t",
        "60",
        "-i",
        source,
        "-an",
        "-sn",
        "-dn",
        "-vf",
        "crop='min(iw,ih)':'min(iw,ih)',fps=20,scale=240:240:flags=lanczos",
        "-c:v",
        "libwebp_anim",
        "-quality",
        "60",
        "-compression_level",
        "3",
        "-loop",
        "0",
        "-f",
        "webp",
        target,
    ])
}
pub fn profile_photo_argv(source: &str, target: &str) -> Vec<String> {
    argv(&[
        "-i",
        source,
        "-frames:v",
        "1",
        "-vf",
        "crop='min(iw,ih)':'min(iw,ih)',scale='min(1280,iw)':'min(1280,ih)'",
        "-c:v",
        "mjpeg",
        "-q:v",
        "3",
        "-f",
        "image2",
        target,
    ])
}
pub fn camera_inputs(camera: &str) -> Vec<String> {
    [
        "-t",
        "60",
        "-f",
        "v4l2",
        "-framerate",
        "30",
        "-i",
        camera,
        "-t",
        "60",
        "-f",
        "pulse",
        "-i",
        "default",
    ]
    .iter()
    .map(|s| (*s).into())
    .collect()
}
pub fn video_capture_argv(target: &str, preview: &str, inputs: &[String]) -> Vec<String> {
    let mut args = argv(&[]);
    args.extend(if inputs.is_empty() {
        camera_inputs("/dev/video0")
    } else {
        inputs.to_vec()
    });
    args.extend(["-filter_complex","[0:v]crop='min(iw,ih)':'min(iw,ih)',scale=480:480,split=2[note][look];[look]fps=8,scale=160:160[small]","-map","[note]","-map","1:a","-c:v","libx264","-preset","ultrafast","-crf","23","-pix_fmt","yuv420p","-c:a","aac","-b:a","64k","-ac","1",target,"-map","[small]","-f","image2","-update","1","-q:v","6",preview].iter().map(|s|(*s).into()));
    args
}
pub fn levels(pcm: &[u8], samples: usize) -> Vec<u8> {
    let count = pcm.len() / 2;
    let buckets = count.min(samples);
    let mut peaks = Vec::with_capacity(buckets);
    for i in 0..buckets {
        let start = i * count / buckets;
        let end = (i + 1) * count / buckets;
        let peak = (start..end)
            .map(|n| i16::from_le_bytes([pcm[n * 2], pcm[n * 2 + 1]]).unsigned_abs())
            .max()
            .unwrap_or(0);
        peaks.push(peak);
    }
    let top = peaks.iter().copied().max().unwrap_or(0).max(1);
    peaks
        .into_iter()
        .map(|p| (31.0 * f64::from(p) / f64::from(top)).round_ties_even() as u8)
        .collect()
}
pub fn pack(levels: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0; levels.len().saturating_mul(5).div_ceil(8)];
    for (i, value) in levels.iter().enumerate() {
        let bit = i * 5;
        let value = value.min(&31);
        bytes[bit / 8] |= *value << (bit % 8);
        if bit % 8 > 3 {
            bytes[bit / 8 + 1] |= *value >> (8 - bit % 8);
        }
    }
    bytes
}
pub fn probe_projection(info: &Value) -> [u64; 3] {
    let value = &info["format"]["duration"];
    let duration = value
        .as_f64()
        .or_else(|| value.as_str().and_then(|s| s.parse::<f64>().ok()))
        .unwrap_or(0.0);
    let video = info["streams"]
        .as_array()
        .and_then(|a| a.iter().find(|s| s["codec_type"] == "video"))
        .unwrap_or(&Value::Null);
    let dimension = |v: &Value| {
        v.as_f64()
            .filter(|n| (0.0..=16384.0).contains(n))
            .map(|n| n as u64)
            .unwrap_or(0)
    };
    [
        if (0.0..=86400.0).contains(&duration) {
            duration.round_ties_even() as u64
        } else {
            0
        },
        dimension(&video["width"]),
        dimension(&video["height"]),
    ]
}
pub fn duration(source: &str) -> io::Result<f64> {
    duration_cancel(source, &std::sync::atomic::AtomicBool::new(false))
}
pub fn duration_cancel(source: &str, cancel: &std::sync::atomic::AtomicBool) -> io::Result<f64> {
    duration_with_budget(source, cancel, Duration::from_secs(20))
}
fn duration_with_budget(
    source: &str,
    cancel: &std::sync::atomic::AtomicBool,
    budget: Duration,
) -> io::Result<f64> {
    let out = process::run_cancel(
        "/usr/bin/ffprobe",
        &[
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=nw=1:nk=1",
            source,
        ],
        &BTreeMap::new(),
        &[],
        budget,
        4096,
        cancel,
    )?;
    if !out.ok() {
        return Err(io::Error::other("could not read the recording"));
    }
    String::from_utf8(out.stdout)
        .ok()
        .and_then(|s| s.lines().next().and_then(|s| s.trim().parse::<f64>().ok()))
        .filter(|n| n.is_finite() && *n >= 0.0)
        .ok_or_else(|| io::Error::other("could not read the recording"))
}
pub fn probe(source: &str) -> [u64; 3] {
    probe_cancel(source, &std::sync::atomic::AtomicBool::new(false))
}
pub fn probe_cancel(source: &str, cancel: &std::sync::atomic::AtomicBool) -> [u64; 3] {
    let result = process::run_cancel(
        "/usr/bin/ffprobe",
        &[
            "-v",
            "error",
            "-show_entries",
            "format=duration:stream=codec_type,width,height",
            "-of",
            "json",
            source,
        ],
        &BTreeMap::new(),
        &[],
        Duration::from_secs(20),
        64 * 1024,
        cancel,
    );
    result
        .ok()
        .filter(process::Output::ok)
        .and_then(|out| serde_json::from_slice::<Value>(&out.stdout).ok())
        .map(|info| probe_projection(&info))
        .unwrap_or([0; 3])
}
pub fn prepare_voice(source: &str) -> io::Result<(u64, String)> {
    prepare_voice_cancel(source, &std::sync::atomic::AtomicBool::new(false))
}
pub fn prepare_voice_cancel(
    source: &str,
    cancel: &std::sync::atomic::AtomicBool,
) -> io::Result<(u64, String)> {
    let seconds = duration_cancel(source, cancel)?;
    let argv = argv(&["-i", source, "-ac", "1", "-ar", "4000", "-f", "s16le", "-"]);
    let out = process::run_cancel(
        "/usr/bin/ffmpeg",
        &argv.iter().map(String::as_str).collect::<Vec<_>>(),
        &BTreeMap::new(),
        &[],
        Duration::from_secs(60),
        605 * 4000 * 2,
        cancel,
    )?;
    if !out.ok() {
        return Err(io::Error::other("could not read the recording"));
    }
    Ok((
        seconds.round_ties_even().max(0.0) as u64,
        STANDARD.encode(pack(&levels(&out.stdout, 100))),
    ))
}
pub fn prepare_video_note(source: &str, target: &str) -> io::Result<f64> {
    prepare_video_note_cancel(source, target, &std::sync::atomic::AtomicBool::new(false))
}
pub fn prepare_video_note_cancel(
    source: &str,
    target: &str,
    cancel: &std::sync::atomic::AtomicBool,
) -> io::Result<f64> {
    prepare_video_note_until(
        source,
        target,
        cancel,
        std::time::Instant::now() + Duration::from_secs(200),
    )
}
pub fn prepare_video_note_until(
    source: &str,
    target: &str,
    cancel: &std::sync::atomic::AtomicBool,
    deadline: std::time::Instant,
) -> io::Result<f64> {
    let args = video_note_argv(source, target);
    let budget = deadline
        .saturating_duration_since(std::time::Instant::now())
        .min(Duration::from_secs(180));
    if budget.is_zero() {
        return Err(io::Error::other("recording preparation deadline exceeded"));
    }
    if !process::run_cancel(
        "/usr/bin/ffmpeg",
        &args.iter().map(String::as_str).collect::<Vec<_>>(),
        &BTreeMap::new(),
        &[],
        budget,
        64 * 1024,
        cancel,
    )?
    .ok()
    {
        return Err(io::Error::other("could not convert the recording"));
    }
    let budget = deadline
        .saturating_duration_since(std::time::Instant::now())
        .min(Duration::from_secs(20));
    if budget.is_zero() {
        return Err(io::Error::other("recording preparation deadline exceeded"));
    }
    duration_with_budget(target, cancel, budget)
}
pub fn project(value: &Value) -> Value {
    let source = value["source"].as_str().unwrap_or("/fixture/source");
    let target = value["target"].as_str().unwrap_or("/fixture/target");
    match value["kind"].as_str() {
        Some("voice_argv") => json!(voice_argv(target)),
        Some("video_note_argv") => json!(video_note_argv(source, target)),
        Some("note_animation_argv") => json!(note_animation_argv(source, target)),
        Some("profile_photo_argv") => json!(profile_photo_argv(source, target)),
        Some("video_capture_argv") => json!(video_capture_argv(
            target,
            value["preview"].as_str().unwrap_or("/fixture/preview"),
            &[]
        )),
        Some("probe") => json!(probe_projection(&value["info"])),
        Some("pcm") => {
            let bytes = STANDARD
                .decode(value["data"].as_str().unwrap_or(""))
                .unwrap_or_default();
            let levels = levels(
                &bytes,
                value["samples"].as_u64().unwrap_or(100).min(1000) as usize,
            );
            json!({"levels":levels,"packed":STANDARD.encode(pack(&levels))})
        }
        _ => Value::Null,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pcm_and_packed_waveform_preserve_extreme_values() {
        assert_eq!(levels(&[0, 128, 255, 127, 0, 0], 100), vec![31, 31, 0]);
        assert_eq!(pack(&[31, 0, 31]), vec![31, 124]);
        assert!(levels(&[], 100).is_empty());
    }
}
