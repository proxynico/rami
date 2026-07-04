use objc2::ffi::NSInteger;
use objc2::rc::Retained;
use objc2::{extern_class, extern_methods};
use objc2_foundation::{NSError, NSObject};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

pub const BUNDLE_IDENTIFIER: &str = "com.nicomontero.rami";

/// `sfltool dumpbtm` is slow and on Apple's deprecation path. We only need it
/// to detect launch-at-login enabled via System Settings (the case SMAppService
/// reports as Disabled). Status is re-read on every refresh tick, so keep this
/// TTL long: it bounds how often the background `sfltool` check re-runs.
const EXTERNAL_CHECK_TTL: Duration = Duration::from_secs(300);

#[link(name = "ServiceManagement", kind = "framework")]
unsafe extern "C" {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchAtLoginStatus {
    Disabled,
    Enabled,
    EnabledExternal,
    RequiresApproval,
    Unavailable,
}

impl LaunchAtLoginStatus {
    pub fn menu_title(self) -> &'static str {
        match self {
            Self::Unavailable => "Launch at Login (App Bundle Only)",
            Self::EnabledExternal => "Launch at Login (System Settings)",
            Self::RequiresApproval => "Launch at Login (Needs Approval)",
            Self::Disabled | Self::Enabled => "Launch at Login",
        }
    }

    pub fn should_enable_menu_item(self) -> bool {
        !matches!(self, Self::Unavailable | Self::EnabledExternal)
    }

    pub fn should_show_checked_state(self) -> bool {
        matches!(
            self,
            Self::Enabled | Self::EnabledExternal | Self::RequiresApproval
        )
    }
}

impl From<NSInteger> for LaunchAtLoginStatus {
    fn from(raw: NSInteger) -> Self {
        match raw {
            0 => Self::Disabled,
            1 => Self::Enabled,
            2 => Self::RequiresApproval,
            _ => Self::Unavailable,
        }
    }
}

extern_class!(
    #[unsafe(super(NSObject))]
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub struct SMAppService;
);

impl SMAppService {
    extern_methods!(
        #[unsafe(method(mainAppService))]
        #[unsafe(method_family = none)]
        pub fn main_app_service() -> Retained<Self>;

        #[unsafe(method(status))]
        #[unsafe(method_family = none)]
        pub fn status(&self) -> NSInteger;

        #[unsafe(method(registerAndReturnError:_))]
        #[unsafe(method_family = none)]
        pub unsafe fn register_and_return_error(&self) -> Result<(), Retained<NSError>>;

        #[unsafe(method(unregisterAndReturnError:_))]
        #[unsafe(method_family = none)]
        pub unsafe fn unregister_and_return_error(&self) -> Result<(), Retained<NSError>>;
    );
}

pub struct LaunchAtLoginController {
    service: Retained<SMAppService>,
    external_cache: Arc<Mutex<Option<(bool, Instant)>>>,
    external_check_in_flight: Arc<Mutex<bool>>,
}

impl Default for LaunchAtLoginController {
    fn default() -> Self {
        Self::new()
    }
}

impl LaunchAtLoginController {
    pub fn new() -> Self {
        Self {
            service: SMAppService::main_app_service(),
            external_cache: Arc::new(Mutex::new(None)),
            external_check_in_flight: Arc::new(Mutex::new(false)),
        }
    }

    pub fn status(&self) -> LaunchAtLoginStatus {
        let service_status = self.service.status().into();
        if matches!(
            service_status,
            LaunchAtLoginStatus::Enabled | LaunchAtLoginStatus::RequiresApproval
        ) {
            return service_status;
        }
        if self.external_login_item_is_enabled_cached() {
            return LaunchAtLoginStatus::EnabledExternal;
        }
        service_status
    }

    pub fn toggle(&self) -> Result<LaunchAtLoginStatus, Retained<NSError>> {
        match self.status() {
            LaunchAtLoginStatus::Enabled => unsafe { self.service.unregister_and_return_error()? },
            LaunchAtLoginStatus::Disabled | LaunchAtLoginStatus::RequiresApproval => unsafe {
                self.service.register_and_return_error()?
            },
            LaunchAtLoginStatus::EnabledExternal | LaunchAtLoginStatus::Unavailable => {
                return Ok(self.status())
            }
        }
        // The SMAppService state we just changed is cheap to re-read, but clear
        // the external cache so the post-toggle status reflects reality.
        *self.external_cache.lock().expect("external cache lock") = None;
        Ok(self.status())
    }

    fn external_login_item_is_enabled_cached(&self) -> bool {
        let now = Instant::now();
        let cache = self.external_cache.lock().expect("external cache lock");
        if let Some(value) = fresh_cached(*cache, now, EXTERNAL_CHECK_TTL) {
            return value;
        }
        let stale_value = cache.map(|(value, _)| value);
        drop(cache);
        self.maybe_start_background_external_check();
        stale_value.unwrap_or(false)
    }

    fn maybe_start_background_external_check(&self) {
        let mut in_flight = self
            .external_check_in_flight
            .lock()
            .expect("external check in-flight lock");
        if *in_flight {
            return;
        }
        *in_flight = true;
        let cache = Arc::clone(&self.external_cache);
        let in_flight_flag = Arc::clone(&self.external_check_in_flight);
        thread::spawn(move || {
            let result = external_login_item_is_enabled();
            let now = Instant::now();
            *cache.lock().expect("external cache lock") = Some((result, now));
            *in_flight_flag
                .lock()
                .expect("external check in-flight lock") = false;
        });
    }
}

/// Return the cached value if it is still within its TTL, else `None`.
fn fresh_cached(cached: Option<(bool, Instant)>, now: Instant, ttl: Duration) -> Option<bool> {
    let (value, sampled_at) = cached?;
    now.duration_since(sampled_at)
        .checked_sub(ttl)
        .is_none()
        .then_some(value)
}

pub(crate) fn current_app_bundle_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    app_bundle_path_from_executable(&exe)
}

fn app_bundle_path_from_executable(path: &Path) -> Option<PathBuf> {
    path.ancestors()
        .find(|ancestor| ancestor.extension().is_some_and(|ext| ext == "app"))
        .map(Path::to_path_buf)
}

fn external_login_item_is_enabled() -> bool {
    let Some(app_path) = current_app_bundle_path() else {
        return false;
    };
    let Some(app_url) = file_url_for_app_path(&app_path) else {
        return false;
    };
    let Ok(output) = Command::new("sfltool").arg("dumpbtm").output() else {
        return false;
    };
    let dump = String::from_utf8_lossy(&output.stdout);
    background_item_dump_has_enabled_app(&dump, BUNDLE_IDENTIFIER, &app_url)
}

fn percent_encode_path_component(component: &str) -> String {
    let mut out = String::with_capacity(component.len());
    for byte in component.bytes() {
        let unreserved =
            matches!(byte, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~');
        if unreserved {
            out.push(byte as char);
        } else {
            out.push('%');
            out.push(char::from(b"0123456789ABCDEF"[(byte >> 4) as usize]));
            out.push(char::from(b"0123456789ABCDEF"[(byte & 0x0f) as usize]));
        }
    }
    out
}

fn file_url_for_app_path(path: &Path) -> Option<String> {
    let path = path.to_str()?;
    let encoded = path
        .split('/')
        .map(percent_encode_path_component)
        .collect::<Vec<_>>()
        .join("/");
    Some(format!("file://{}/", encoded.trim_end_matches('/')))
}

fn background_item_dump_has_enabled_app(dump: &str, bundle_id: &str, app_url: &str) -> bool {
    dump.split("\n\n").any(|entry| {
        entry.contains("Disposition: [enabled")
            && entry.contains(&format!("Bundle Identifier: {bundle_id}"))
            && entry.contains(&format!("URL: {app_url}"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requires_approval_status_uses_explicit_menu_copy() {
        assert_eq!(
            LaunchAtLoginStatus::RequiresApproval.menu_title(),
            "Launch at Login (Needs Approval)"
        );
    }

    #[test]
    fn unavailable_status_disables_the_menu_item() {
        assert!(!LaunchAtLoginStatus::Unavailable.should_enable_menu_item());
    }

    #[test]
    fn external_status_is_checked_but_not_toggled_from_the_menu() {
        assert_eq!(
            LaunchAtLoginStatus::EnabledExternal.menu_title(),
            "Launch at Login (System Settings)"
        );
        assert!(LaunchAtLoginStatus::EnabledExternal.should_show_checked_state());
        assert!(!LaunchAtLoginStatus::EnabledExternal.should_enable_menu_item());
    }

    #[test]
    fn app_bundle_path_from_executable_finds_outer_app() {
        let path = std::path::Path::new("/Applications/rami.app/Contents/MacOS/rami");
        assert_eq!(
            app_bundle_path_from_executable(path).as_deref(),
            Some(std::path::Path::new("/Applications/rami.app"))
        );
    }

    #[test]
    fn fresh_cached_returns_value_within_ttl() {
        let now = Instant::now();
        let cached = Some((true, now));
        assert_eq!(fresh_cached(cached, now, EXTERNAL_CHECK_TTL), Some(true));
        // Just inside the TTL window.
        let just_stale = now + EXTERNAL_CHECK_TTL;
        assert_eq!(
            fresh_cached(Some((false, now)), just_stale, EXTERNAL_CHECK_TTL),
            None
        );
    }

    #[test]
    fn fresh_cached_returns_none_when_unset_or_stale() {
        assert_eq!(fresh_cached(None, Instant::now(), EXTERNAL_CHECK_TTL), None);
    }

    #[test]
    fn file_url_for_app_path_percent_encodes_spaces() {
        assert_eq!(
            file_url_for_app_path(std::path::Path::new("/Applications/My App.app")).as_deref(),
            Some("file:///Applications/My%20App.app/")
        );
    }

    #[test]
    fn background_item_dump_detects_enabled_matching_bundle() {
        let dump = r#"
 #58:
                 Name: rami
          Disposition: [enabled, allowed, notified] (0xb)
           Identifier: 2.com.nicomontero.rami
                  URL: file:///Applications/rami.app/
    Bundle Identifier: com.nicomontero.rami
"#;

        assert!(background_item_dump_has_enabled_app(
            dump,
            "com.nicomontero.rami",
            "file:///Applications/rami.app/"
        ));
        assert!(!background_item_dump_has_enabled_app(
            dump,
            "com.nicomontero.rami",
            "file:///Applications/Other.app/"
        ));
    }

    #[test]
    fn background_item_dump_matches_percent_encoded_app_url() {
        let dump = r#"
 #59:
                 Name: rami
          Disposition: [enabled, allowed, notified] (0xb)
           Identifier: 2.com.nicomontero.rami
                  URL: file:///Applications/My%20App.app/
    Bundle Identifier: com.nicomontero.rami
"#;

        assert!(background_item_dump_has_enabled_app(
            dump,
            "com.nicomontero.rami",
            "file:///Applications/My%20App.app/"
        ));
    }
}
