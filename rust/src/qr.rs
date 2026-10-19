//! Finite QR helpers, owned by the exact account generation and Telegram link.
use crate::{authorization::Runtime, engine::Effects, process::Managed};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde_json::json;
use std::{
    collections::{BTreeMap, HashMap},
    time::Duration,
};

struct Pending {
    generation: u64,
    link: String,
    helper: Managed,
}
#[derive(Default)]
pub struct Codes {
    pending: HashMap<String, Pending>,
}
impl Codes {
    pub fn poll(&mut self, runtime: &mut Runtime) -> Effects {
        let mut effects = Effects::default();
        self.pending.retain(|id, owner| {
            !runtime.stopping
                && runtime
                    .engine
                    .accounts
                    .get(id)
                    .is_some_and(|a| a.generation == owner.generation)
                && runtime
                    .metadata
                    .get(id)
                    .is_some_and(|m| m.auth["state"] == "qr" && m.auth["link"] == owner.link)
        });
        if runtime.stopping {
            return effects;
        }
        for (id, meta) in &mut runtime.metadata {
            if meta.auth["state"] != "qr" {
                continue;
            }
            if let Some(owner) = self.pending.get_mut(id) {
                let output = match owner.helper.poll() {
                    Ok(None) => continue,
                    Ok(Some(output))
                        if output.ok() && output.stdout.starts_with(b"\x89PNG\r\n\x1a\n") =>
                    {
                        Some(output.stdout)
                    }
                    _ => None,
                };
                meta.auth["imagePending"] = json!(false);
                if let Some(png) = output {
                    meta.auth["image"] =
                        json!(format!("data:image/png;base64,{}", STANDARD.encode(png)));
                }
                self.pending.remove(id);
            } else {
                if meta.auth.get("imagePending").is_some() {
                    continue;
                }
                meta.auth["imagePending"] = json!(false);
                let link = meta.auth["link"].as_str().unwrap_or("");
                if link.starts_with("tg://login?")
                    && link.len() <= 2048
                    && let Ok(helper) = Managed::start(
                        "/usr/bin/qrencode",
                        &["-t", "PNG", "-o", "-", "-s", "8", "-m", "4"],
                        &BTreeMap::new(),
                        link.as_bytes(),
                        Duration::from_secs(3),
                        256 * 1024,
                    )
                {
                    self.pending.insert(
                        id.clone(),
                        Pending {
                            generation: runtime.engine.accounts[id].generation,
                            link: link.into(),
                            helper,
                        },
                    );
                    meta.auth["imagePending"] = json!(true);
                }
            }
            effects
                .replies
                .push((0, json!({"event":"auth","account":id,"auth":meta.auth})));
        }
        effects
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;
    #[test]
    fn actual_encoder_publishes_png_and_retires_changed_account_work() {
        let mut runtime = Runtime::new("default".into());
        runtime
            .register(
                "default".into(),
                "Synthetic".into(),
                1,
                vec![],
                Instant::now(),
            )
            .unwrap();
        let mut codes = Codes::default();
        runtime.metadata.get_mut("default").unwrap().auth =
            json!({"state":"qr","link":"tg://login?token=SYNTHETIC"});
        assert_eq!(
            codes.poll(&mut runtime).replies[0].1["auth"]["imagePending"],
            true
        );
        let deadline = Instant::now() + Duration::from_secs(4);
        while runtime.metadata["default"].auth["imagePending"] == true && Instant::now() < deadline
        {
            std::thread::sleep(Duration::from_millis(5));
            codes.poll(&mut runtime);
        }
        let image = runtime.metadata["default"].auth["image"]
            .as_str()
            .expect("actual QR image");
        let png = STANDARD
            .decode(image.strip_prefix("data:image/png;base64,").unwrap())
            .unwrap();
        assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));
        assert!(
            codes.poll(&mut runtime).replies.is_empty(),
            "do not re-encode settled token"
        );
        runtime.metadata.get_mut("default").unwrap().auth =
            json!({"state":"qr","link":"tg://login?token=NEW"});
        codes.poll(&mut runtime);
        runtime.metadata.get_mut("default").unwrap().auth = json!({"state":"phone"});
        assert!(codes.poll(&mut runtime).replies.is_empty());
        assert!(codes.pending.is_empty());
        assert_eq!(runtime.metadata["default"].auth, json!({"state":"phone"}));
    }
}
