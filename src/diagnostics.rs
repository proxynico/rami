use crate::format::{delta_bytes_text, gb_pair, mem_text};
use crate::login_item::{current_app_bundle_path, LaunchAtLoginStatus, BUNDLE_IDENTIFIER};
use crate::model::MemorySnapshot;
use crate::process_memory::{AppMemorySnapshot, AppMemoryUsage};

pub(crate) struct DiagnosticReportInput<'a> {
    pub(crate) version: String,
    pub(crate) bundle_identifier: String,
    pub(crate) bundle_path: Option<String>,
    pub(crate) process_path: String,
    pub(crate) architecture: String,
    pub(crate) launch_at_login: LaunchAtLoginStatus,
    pub(crate) memory: Option<MemorySnapshot>,
    pub(crate) apps: &'a AppMemorySnapshot,
}

pub(crate) fn current_report_input<'a>(
    launch_at_login: LaunchAtLoginStatus,
    memory: Option<MemorySnapshot>,
    apps: &'a AppMemorySnapshot,
) -> DiagnosticReportInput<'a> {
    let process_path = std::env::current_exe()
        .ok()
        .and_then(|path| path.into_os_string().into_string().ok())
        .unwrap_or_else(|| "<unknown>".to_string());
    let bundle_path = current_app_bundle_path()
        .and_then(|path| path.into_os_string().into_string().ok())
        .unwrap_or_else(|| "<not running from app bundle>".to_string());

    DiagnosticReportInput {
        version: env!("CARGO_PKG_VERSION").to_string(),
        bundle_identifier: BUNDLE_IDENTIFIER.to_string(),
        bundle_path: Some(bundle_path),
        process_path,
        architecture: std::env::consts::ARCH.to_string(),
        launch_at_login,
        memory,
        apps,
    }
}

pub(crate) fn build_diagnostic_report(input: DiagnosticReportInput<'_>) -> String {
    let mut out = String::new();
    out.push_str("rami diagnostics\n");
    out.push_str(&format!("Version: {}\n", input.version));
    out.push_str(&format!("Bundle ID: {}\n", input.bundle_identifier));
    out.push_str(&format!(
        "Bundle path: {}\n",
        input.bundle_path.as_deref().unwrap_or("<unknown>")
    ));
    out.push_str(&format!("Process path: {}\n", input.process_path));
    out.push_str(&format!("Architecture: {}\n", input.architecture));
    out.push_str(&format!(
        "Launch at login: {}\n",
        launch_at_login_label(input.launch_at_login)
    ));

    match input.memory {
        Some(snapshot) => {
            out.push_str(&format!(
                "Memory: {} ({}%)\n",
                gb_pair(snapshot.used_bytes, snapshot.total_bytes),
                snapshot.used_percent
            ));
            out.push_str(&format!("Swap: {}\n", mem_text(snapshot.swap_used_bytes)));
        }
        None => out.push_str("Memory: unavailable\nSwap: unavailable\n"),
    }

    out.push_str("Apps:\n");
    match input.apps {
        AppMemorySnapshot::Loaded(rows) if !rows.is_empty() => {
            for row in rows.iter().take(5) {
                out.push_str(&format!("- {}\n", app_row_text(row)));
            }
        }
        AppMemorySnapshot::Hidden => out.push_str("- hidden\n"),
        AppMemorySnapshot::Loading => out.push_str("- loading\n"),
        AppMemorySnapshot::Loaded(_) | AppMemorySnapshot::Unavailable => {
            out.push_str("- unavailable\n")
        }
    }

    out
}

fn launch_at_login_label(status: LaunchAtLoginStatus) -> &'static str {
    match status {
        LaunchAtLoginStatus::Disabled => "Disabled",
        LaunchAtLoginStatus::Enabled => "Enabled",
        LaunchAtLoginStatus::EnabledExternal => "Enabled via System Settings",
        LaunchAtLoginStatus::RequiresApproval => "Needs approval",
        LaunchAtLoginStatus::Unavailable => "Unavailable",
    }
}

fn app_row_text(row: &AppMemoryUsage) -> String {
    let mut text = format!("{}: {}", row.name, mem_text(row.footprint_bytes));
    if let Some(delta) = row.delta_bytes.filter(|delta| *delta >= 50_000_000) {
        text.push_str(&format!(" ({})", delta_bytes_text(delta as u64)));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::login_item::LaunchAtLoginStatus;
    use crate::model::MemorySnapshot;
    use crate::process_memory::{AppMemorySnapshot, AppMemoryUsage};

    #[test]
    fn report_includes_runtime_paths_login_state_and_memory_rows() {
        let apps = AppMemorySnapshot::Loaded(vec![AppMemoryUsage {
            name: "Cursor".to_string(),
            group_key: "/Applications/Cursor.app".to_string(),
            footprint_bytes: 2_100_000_000,
            pids: vec![42, 43],
            can_quit: true,
            delta_bytes: Some(350_000_000),
        }]);
        let report = build_diagnostic_report(DiagnosticReportInput {
            version: "0.1.0".to_string(),
            bundle_identifier: "com.nicomontero.rami".to_string(),
            bundle_path: Some("/Applications/rami.app".to_string()),
            process_path: "/Applications/rami.app/Contents/MacOS/rami".to_string(),
            architecture: "aarch64".to_string(),
            launch_at_login: LaunchAtLoginStatus::EnabledExternal,
            memory: Some(MemorySnapshot {
                used_bytes: 9_100_000_000,
                total_bytes: 18_000_000_000,
                used_percent: 51,
                swap_used_bytes: 400_000_000,
            }),
            apps: &apps,
        });

        assert!(report.contains("rami diagnostics"));
        assert!(report.contains("Version: 0.1.0"));
        assert!(report.contains("Bundle ID: com.nicomontero.rami"));
        assert!(report.contains("Bundle path: /Applications/rami.app"));
        assert!(report.contains("Process path: /Applications/rami.app/Contents/MacOS/rami"));
        assert!(report.contains("Architecture: aarch64"));
        assert!(report.contains("Launch at login: Enabled via System Settings"));
        assert!(report.contains("Memory: 9.1 / 18.0 GB (51%)"));
        assert!(report.contains("Swap: 400 MB"));
        assert!(report.contains("- Cursor: 2.1 GB (+350 MB)"));
    }
}
