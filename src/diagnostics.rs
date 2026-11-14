use crate::format::{gb_pair, mem_text};
use crate::login_item::{current_app_bundle_path, LaunchAtLoginStatus, BUNDLE_IDENTIFIER};
use crate::model::{MemorySnapshot, PressureSource};

pub(crate) struct DiagnosticReportInput {
    pub(crate) bundle_path: String,
    pub(crate) process_path: String,
    pub(crate) launch_at_login: LaunchAtLoginStatus,
    pub(crate) memory: Option<MemorySnapshot>,
}

pub(crate) fn current_report_input(
    launch_at_login: LaunchAtLoginStatus,
    memory: Option<MemorySnapshot>,
) -> DiagnosticReportInput {
    let process_path = std::env::current_exe()
        .ok()
        .and_then(|path| path.into_os_string().into_string().ok())
        .unwrap_or_else(|| "<unknown>".to_string());
    let bundle_path = current_app_bundle_path()
        .and_then(|path| path.into_os_string().into_string().ok())
        .unwrap_or_else(|| "<not running from app bundle>".to_string());

    DiagnosticReportInput {
        bundle_path,
        process_path,
        launch_at_login,
        memory,
    }
}

pub(crate) fn build_diagnostic_report(input: DiagnosticReportInput) -> String {
    let header = format!(
        "rami diagnostics\n\
         Version: {}\n\
         Bundle ID: {BUNDLE_IDENTIFIER}\n\
         Bundle path: {}\n\
         Process path: {}\n\
         Architecture: {}\n\
         Launch at login: {}\n",
        env!("CARGO_PKG_VERSION"),
        input.bundle_path,
        input.process_path,
        std::env::consts::ARCH,
        launch_at_login_label(input.launch_at_login),
    );
    match input.memory {
        Some(s) => format!(
            "{header}\
             Memory: {} ({}%)\n\
             Pressure: {}% ({})\n\
             App Memory: {}\n\
             Wired: {}\n\
             Compressed: {}\n\
             Cached: {}\n\
             Other: {}\n\
             Free: {}\n\
             Available: {}\n\
             Swap: {}\n",
            gb_pair(s.used_bytes, s.total_bytes),
            s.used_percent,
            s.pressure_percent,
            pressure_source_label(s.pressure_source),
            mem_text(s.app_memory_bytes),
            mem_text(s.wired_bytes),
            mem_text(s.compressed_bytes),
            mem_text(s.cached_bytes),
            mem_text(s.other_bytes()),
            mem_text(s.free_bytes),
            mem_text(s.available_bytes),
            mem_text(s.swap_used_bytes),
        ),
        None => header + "Memory: unavailable\nSwap: unavailable\n",
    }
}

fn pressure_source_label(source: PressureSource) -> &'static str {
    match source {
        PressureSource::Kernel => "kernel",
        PressureSource::AvailableFallback => "Available fallback",
    }
}

fn launch_at_login_label(status: LaunchAtLoginStatus) -> &'static str {
    match status {
        LaunchAtLoginStatus::Disabled => "Disabled",
        LaunchAtLoginStatus::Enabled => "Enabled",
        LaunchAtLoginStatus::RequiresApproval => "Needs approval",
        LaunchAtLoginStatus::Unavailable => "Unavailable",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::login_item::LaunchAtLoginStatus;
    use crate::model::MemorySnapshot;

    #[test]
    fn report_includes_runtime_paths_login_state_and_memory_rows() {
        let report = build_diagnostic_report(DiagnosticReportInput {
            bundle_path: "/Applications/rami.app".to_string(),
            process_path: "/Applications/rami.app/Contents/MacOS/rami".to_string(),
            launch_at_login: LaunchAtLoginStatus::Enabled,
            memory: Some(MemorySnapshot {
                used_bytes: 9_774_150_902,
                total_bytes: 19_327_352_832,
                used_percent: 51,
                pressure_percent: 27,
                pressure_source: PressureSource::Kernel,
                app_memory_bytes: 6_442_450_944,
                wired_bytes: 2_147_483_648,
                compressed_bytes: 1_073_741_824,
                free_bytes: 3_221_225_472,
                cached_bytes: 0,
                swap_used_bytes: 419_430_400,
                available_bytes: 9_556_301_414,
            }),
        });

        assert!(report.contains("rami diagnostics"));
        assert!(report.contains(concat!("Version: ", env!("CARGO_PKG_VERSION"))));
        assert!(report.contains("Bundle ID: com.nicomontero.rami"));
        assert!(report.contains("Bundle path: /Applications/rami.app"));
        assert!(report.contains("Process path: /Applications/rami.app/Contents/MacOS/rami"));
        assert!(report.contains(&format!("Architecture: {}", std::env::consts::ARCH)));
        assert!(report.contains("Launch at login: Enabled"));
        assert!(report.contains("Memory: 9.1 / 18.0 GB (51%)"));
        assert!(report.contains("Pressure: 27% (kernel)"));
        assert!(report.contains("App Memory: 6.0 GB"));
        assert!(report.contains("Available: 8.9 GB"));
        assert!(report.contains("Swap: 400 MB"));
    }
}
