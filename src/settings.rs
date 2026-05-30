use objc2::rc::Retained;
use objc2_foundation::{NSString, NSUserDefaults};

const KEY_AUTO_REFRESH: &str = "autoRefreshEnabled";
const KEY_SHOW_APPS: &str = "showAppUsage";

/// User-toggleable settings that persist across launches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settings {
    pub auto_refresh_enabled: bool,
    pub show_app_usage: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            auto_refresh_enabled: true,
            show_app_usage: true,
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
        let fallback = Settings::default();
        Settings {
            auto_refresh_enabled: self.read_bool(KEY_AUTO_REFRESH, fallback.auto_refresh_enabled),
            show_app_usage: self.read_bool(KEY_SHOW_APPS, fallback.show_app_usage),
        }
    }

    pub fn set_auto_refresh_enabled(&self, value: bool) {
        self.defaults
            .setBool_forKey(value, &NSString::from_str(KEY_AUTO_REFRESH));
    }

    pub fn set_show_app_usage(&self, value: bool) {
        self.defaults
            .setBool_forKey(value, &NSString::from_str(KEY_SHOW_APPS));
    }

    fn read_bool(&self, key: &str, fallback: bool) -> bool {
        let key = NSString::from_str(key);
        // objectForKey is None only when the key was never written, letting us
        // distinguish "unset" (use the default) from an explicitly stored `false`.
        let stored = self
            .defaults
            .objectForKey(&key)
            .is_some()
            .then(|| self.defaults.boolForKey(&key));
        resolve_bool(stored, fallback)
    }
}

fn resolve_bool(stored: Option<bool>, fallback: bool) -> bool {
    stored.unwrap_or(fallback)
}

#[cfg(test)]
mod tests {
    use super::resolve_bool;

    #[test]
    fn resolve_bool_uses_fallback_only_when_unset() {
        assert!(resolve_bool(None, true));
        assert!(!resolve_bool(None, false));
        // A stored value always wins over the default, including an explicit false.
        assert!(!resolve_bool(Some(false), true));
        assert!(resolve_bool(Some(true), false));
    }
}
