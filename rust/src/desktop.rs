//! Bounded notification action routes. Fixed commands; no external strings are
//! interpolated into shell source or silently retried after timeout.
use crate::{launcher::Context, process::Managed};
use serde_json::Value;
use std::{io, time::Duration};
pub struct Pending {
    pub account: String,
    pub generation: u64,
    pub target: Value,
    pub helper: Managed,
    pub fallback: bool,
}
pub fn start(
    context: &Context,
    account: String,
    generation: u64,
    target: Value,
    reply: bool,
) -> io::Result<Pending> {
    let helper = if reply {
        Managed::start(
            "/usr/bin/omarchy-shell",
            &[
                "shell",
                "summon",
                crate::launcher::PLUGIN_ID,
                &target.to_string(),
            ],
            &context.environment,
            &[],
            Duration::from_secs(10),
            4096,
        )?
    } else {
        let argv = context.managed_argv(
            &format!("telebar-window-notify-{}", std::process::id()),
            "--host-window",
            &[],
        );
        Managed::start(
            "/usr/bin/systemd-run",
            &argv.iter().map(String::as_str).collect::<Vec<_>>(),
            &context.environment,
            &[],
            Duration::from_secs(5),
            4096,
        )?
    };
    Ok(Pending {
        account,
        generation,
        target,
        helper,
        fallback: reply,
    })
}
