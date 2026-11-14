use crate::engine::SettingChange;
use objc2::rc::Retained;
use objc2_foundation::{NSString, NSUserDefaults};

const KEY_AUTO_REFRESH: &str = "autoRefreshEnabled";
const KEY_SHOW_CPU: &str = "showCpu";
const KEY_SHOW_GPU: &str = "showGpu";
const KEY_SHARE: &str = "shareOverTailscale";
const KEY_SHOW_SHARED_MAC: &str = "showSharedMac";
const KEY_SHARED_MAC_HOST: &str = "sharedMacHost";

/// User-toggleable settings that persist across launches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settings {
    pub auto_refresh_enabled: bool,
    pub show_cpu: bool,
    pub show_gpu: bool,
    pub share_over_tailscale: bool,
    pub show_shared_mac: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            auto_refresh_enabled: true,
            show_cpu: true,
            // GPU is opt-in so the default dropdown stays memory-first.
            show_gpu: false,
            share_over_tailscale: false,
            show_shared_mac: true,
        }
    }
}

/// Thin wrapper over `NSUserDefaults`. For the app bundle this persists to
/// `~/Library/Preferences/com.nicomontero.rami.plist`.
pub struct SettingsStore {
    defaults: Retained<NSUserDefaults>,
}

impl SettingsStore {
    pub fn new() -> Self {
        Self {
            defaults: NSUserDefaults::standardUserDefaults(),
        }
    }

    pub fn load(&self) -> Settings {
        let default = Settings::default();
        Settings {
            auto_refresh_enabled: self.read_bool(KEY_AUTO_REFRESH, default.auto_refresh_enabled),
            show_cpu: self.read_bool(KEY_SHOW_CPU, default.show_cpu),
            show_gpu: self.read_bool(KEY_SHOW_GPU, default.show_gpu),
            share_over_tailscale: self.read_bool(KEY_SHARE, default.share_over_tailscale),
            show_shared_mac: self.read_bool(KEY_SHOW_SHARED_MAC, default.show_shared_mac),
        }
    }

    pub fn persist(&self, change: SettingChange) {
        let (key, value) = match change {
            SettingChange::AutoRefresh(value) => (KEY_AUTO_REFRESH, value),
            SettingChange::ShowCpu(value) => (KEY_SHOW_CPU, value),
            SettingChange::ShowGpu(value) => (KEY_SHOW_GPU, value),
            SettingChange::ShareOverTailscale(value) => (KEY_SHARE, value),
            SettingChange::ShowSharedMac(value) => (KEY_SHOW_SHARED_MAC, value),
        };
        self.defaults
            .setBool_forKey(value, &NSString::from_str(key));
    }

    /// The tailnet host whose shared reading the dropdown shows, set once
    /// with `defaults write com.nicomontero.rami sharedMacHost <host>`.
    pub fn shared_mac_host(&self) -> Option<String> {
        let host = self
            .defaults
            .stringForKey(&NSString::from_str(KEY_SHARED_MAC_HOST))?
            .to_string();
        let host = host.trim();
        (!host.is_empty()).then(|| host.to_string())
    }

    fn read_bool(&self, key: &str, default: bool) -> bool {
        let key = NSString::from_str(key);
        // objectForKey is None only when the key was never written, letting us
        // distinguish "unset" (use the default) from an explicitly stored `false`.
        if self.defaults.objectForKey(&key).is_some() {
            self.defaults.boolForKey(&key)
        } else {
            default
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Settings;

    #[test]
    fn defaults_hide_gpu_and_keep_sharing_off() {
        assert_eq!(
            Settings::default(),
            Settings {
                auto_refresh_enabled: true,
                show_cpu: true,
                show_gpu: false,
                share_over_tailscale: false,
                show_shared_mac: true,
            }
        );
    }
}
