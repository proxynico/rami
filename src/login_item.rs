use objc2::ffi::NSInteger;
use objc2::rc::Retained;
use objc2::{extern_class, extern_methods};
use objc2_foundation::{NSError, NSObject};
use std::path::{Path, PathBuf};

pub const BUNDLE_IDENTIFIER: &str = "com.nicomontero.rami";

#[link(name = "ServiceManagement", kind = "framework")]
unsafe extern "C" {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchAtLoginStatus {
    Disabled,
    Enabled,
    RequiresApproval,
    Unavailable,
}

impl LaunchAtLoginStatus {
    pub fn menu_title(self) -> &'static str {
        match self {
            Self::Unavailable => "Launch at Login (App Bundle Only)",
            Self::RequiresApproval => "Launch at Login (Needs Approval)",
            Self::Disabled | Self::Enabled => "Launch at Login",
        }
    }

    pub fn should_enable_menu_item(self) -> bool {
        !matches!(self, Self::Unavailable)
    }

    pub fn should_show_checked_state(self) -> bool {
        matches!(self, Self::Enabled | Self::RequiresApproval)
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
        }
    }

    pub fn status(&self) -> LaunchAtLoginStatus {
        status_for(self.service.status(), current_app_bundle_path().is_some())
    }

    pub fn toggle(&self) -> Result<LaunchAtLoginStatus, Retained<NSError>> {
        match self.status() {
            LaunchAtLoginStatus::Enabled => unsafe { self.service.unregister_and_return_error()? },
            LaunchAtLoginStatus::Disabled | LaunchAtLoginStatus::RequiresApproval => unsafe {
                self.service.register_and_return_error()?
            },
            LaunchAtLoginStatus::Unavailable => return Ok(self.status()),
        }
        Ok(self.status())
    }
}

/// SMAppService's `notFound`, which macOS reports for a bundled app that has
/// never registered.
const NOT_FOUND: NSInteger = 3;

/// Inside an app bundle, `notFound` still permits a registration attempt, so
/// it reads as off rather than unavailable.
fn status_for(raw: NSInteger, in_app_bundle: bool) -> LaunchAtLoginStatus {
    if raw == NOT_FOUND && in_app_bundle {
        LaunchAtLoginStatus::Disabled
    } else {
        raw.into()
    }
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
    fn not_found_is_toggleable_inside_an_app_bundle() {
        assert_eq!(status_for(NOT_FOUND, true), LaunchAtLoginStatus::Disabled);
        assert_eq!(
            status_for(NOT_FOUND, false),
            LaunchAtLoginStatus::Unavailable
        );
        assert_eq!(status_for(1, true), LaunchAtLoginStatus::Enabled);
    }

    #[test]
    fn app_bundle_path_from_executable_finds_outer_app() {
        let path = std::path::Path::new("/Applications/rami.app/Contents/MacOS/rami");
        assert_eq!(
            app_bundle_path_from_executable(path).as_deref(),
            Some(std::path::Path::new("/Applications/rami.app"))
        );
    }
}
