use crate::process_memory::{pid_still_matches_usage, AppMemoryUsage};
use objc2_app_kit::NSRunningApplication;
use std::io;

pub fn quit_app_group(usage: &AppMemoryUsage) -> io::Result<bool> {
    if !usage.can_quit || usage.pids.is_empty() {
        return Ok(false);
    }

    let mut sent_any = false;
    for pid in &usage.pids {
        if *pid <= 0 || !pid_still_matches_usage(*pid, usage) {
            continue;
        }
        let Some(app) = NSRunningApplication::runningApplicationWithProcessIdentifier(*pid) else {
            continue;
        };
        sent_any |= app.terminate();
    }

    Ok(sent_any)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quit_app_group_on_invalid_pid_ignores_stale_pid_without_panicking() {
        let usage = AppMemoryUsage {
            name: "Missing".to_string(),
            group_key: "/Applications/Missing.app".to_string(),
            footprint_bytes: 1,
            pids: vec![999_999],
            can_quit: true,
            delta_bytes: None,
        };
        assert!(!quit_app_group(&usage).expect("stale pid should be ignored"));
    }
}
