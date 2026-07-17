use objc2::rc::Retained;
use objc2_foundation::{NSString, NSUserDefaults};

const KEY_AUTO_REFRESH: &str = "autoRefreshEnabled";
const KEY_SHOW_APPS: &str = "showAppUsage";
const KEY_SHOW_CPU: &str = "showCpu";
const KEY_SHOW_GPU: &str = "showGpu";

/// User-toggleable settings that persist across launches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settings {
    pub auto_refresh_enabled: bool,
    pub show_app_usage: bool,
    pub show_cpu: bool,
    pub show_gpu: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            auto_refresh_enabled: true,
            show_app_usage: true,
            show_cpu: true,
            show_gpu: true,
        }
    }
}

/// Thin wrapper over `NSUserDefaults`. For the app bundle this persists to
/// `~/Library/Preferences/com.nicomontero.rami.plist` — the same file the
/// Homebrew Cask's `zap` removes on uninstall.
pub struct SettingsStore {
    defaults: Retained<NSUserDefaults>,
}

impl Default for SettingsStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SettingsStore {
    pub fn new() -> Self {
        Self {
            defaults: NSUserDefaults::standardUserDefaults(),
        }
    }

    pub fn load(&self) -> Settings {
        resolve_settings(
            self.read_stored_bool(KEY_AUTO_REFRESH),
            self.read_stored_bool(KEY_SHOW_APPS),
            self.read_stored_bool(KEY_SHOW_CPU),
            self.read_stored_bool(KEY_SHOW_GPU),
        )
    }

    pub fn set_auto_refresh_enabled(&self, value: bool) {
        self.defaults
            .setBool_forKey(value, &NSString::from_str(KEY_AUTO_REFRESH));
    }

    pub fn set_show_app_usage(&self, value: bool) {
        self.defaults
            .setBool_forKey(value, &NSString::from_str(KEY_SHOW_APPS));
    }

    pub fn set_show_cpu(&self, value: bool) {
        self.defaults
            .setBool_forKey(value, &NSString::from_str(KEY_SHOW_CPU));
    }

    pub fn set_show_gpu(&self, value: bool) {
        self.defaults
            .setBool_forKey(value, &NSString::from_str(KEY_SHOW_GPU));
    }

    fn read_stored_bool(&self, key: &str) -> Option<bool> {
        let key = NSString::from_str(key);
        // objectForKey is None only when the key was never written, letting us
        // distinguish "unset" (use the default) from an explicitly stored `false`.
        self.defaults
            .objectForKey(&key)
            .is_some()
            .then(|| self.defaults.boolForKey(&key))
    }
}

fn resolve_settings(
    auto_refresh_enabled: Option<bool>,
    show_app_usage: Option<bool>,
    show_cpu: Option<bool>,
    show_gpu: Option<bool>,
) -> Settings {
    let fallback = Settings::default();
    Settings {
        auto_refresh_enabled: resolve_bool(auto_refresh_enabled, fallback.auto_refresh_enabled),
        show_app_usage: resolve_bool(show_app_usage, fallback.show_app_usage),
        show_cpu: resolve_bool(show_cpu, fallback.show_cpu),
        show_gpu: resolve_bool(show_gpu, fallback.show_gpu),
    }
}

fn resolve_bool(stored: Option<bool>, fallback: bool) -> bool {
    stored.unwrap_or(fallback)
}

#[cfg(test)]
mod tests {
    use super::{resolve_bool, resolve_settings};

    #[test]
    fn resolve_bool_uses_fallback_only_when_unset() {
        assert!(resolve_bool(None, true));
        assert!(!resolve_bool(None, false));
        // A stored value always wins over the default, including an explicit false.
        assert!(!resolve_bool(Some(false), true));
        assert!(resolve_bool(Some(true), false));
    }

    #[test]
    fn module_visibility_defaults_on_and_persists_independently() {
        let defaults = resolve_settings(None, None, None, None);
        assert!(defaults.auto_refresh_enabled);
        assert!(defaults.show_app_usage);
        assert!(defaults.show_cpu);
        assert!(defaults.show_gpu);

        let persisted = resolve_settings(Some(false), Some(true), Some(false), Some(true));
        assert!(!persisted.auto_refresh_enabled);
        assert!(persisted.show_app_usage);
        assert!(!persisted.show_cpu);
        assert!(persisted.show_gpu);
    }
}
