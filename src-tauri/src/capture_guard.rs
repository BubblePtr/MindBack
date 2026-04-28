use anyhow::{anyhow, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisplayState {
    pub is_online: bool,
    pub is_active: bool,
    pub is_asleep: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureBlockReason {
    DisplayOffline,
    DisplayInactive,
    DisplayAsleep,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaptureAvailability {
    reason: Option<CaptureBlockReason>,
}

impl CaptureAvailability {
    pub fn current() -> Self {
        if std::env::var("MINDBACK_SIMULATE_CAPTURE").as_deref() == Ok("1") {
            return Self::allowed();
        }

        Self::from_display_state(current_display_state())
    }

    pub fn from_display_state(display: DisplayState) -> Self {
        if !display.is_online {
            return Self::blocked(CaptureBlockReason::DisplayOffline);
        }
        if display.is_asleep {
            return Self::blocked(CaptureBlockReason::DisplayAsleep);
        }
        if !display.is_active {
            return Self::blocked(CaptureBlockReason::DisplayInactive);
        }

        Self::allowed()
    }

    pub fn is_allowed(self) -> bool {
        self.reason.is_none()
    }

    pub fn ensure_allowed(self) -> Result<()> {
        match self.reason {
            None => Ok(()),
            Some(CaptureBlockReason::DisplayOffline) => {
                Err(anyhow!("当前显示器未在线，已跳过截图"))
            }
            Some(CaptureBlockReason::DisplayInactive) => {
                Err(anyhow!("当前显示器未激活，已跳过截图"))
            }
            Some(CaptureBlockReason::DisplayAsleep) => Err(anyhow!("当前显示器已息屏，已跳过截图")),
        }
    }

    fn allowed() -> Self {
        Self { reason: None }
    }

    fn blocked(reason: CaptureBlockReason) -> Self {
        Self {
            reason: Some(reason),
        }
    }
}

#[cfg(target_os = "macos")]
fn current_display_state() -> DisplayState {
    macos_display_state::current_display_state()
}

#[cfg(not(target_os = "macos"))]
fn current_display_state() -> DisplayState {
    DisplayState {
        is_online: true,
        is_active: true,
        is_asleep: false,
    }
}

#[cfg(target_os = "macos")]
mod macos_display_state {
    use super::DisplayState;

    type CGDirectDisplayID = u32;
    type BooleanT = i32;

    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGMainDisplayID() -> CGDirectDisplayID;
        fn CGDisplayIsActive(display: CGDirectDisplayID) -> BooleanT;
        fn CGDisplayIsAsleep(display: CGDirectDisplayID) -> BooleanT;
        fn CGDisplayIsOnline(display: CGDirectDisplayID) -> BooleanT;
    }

    pub fn current_display_state() -> DisplayState {
        // CoreGraphics exposes exactly the display drawability checks we need:
        // online, active, and not asleep.
        unsafe {
            let display = CGMainDisplayID();
            DisplayState {
                is_online: CGDisplayIsOnline(display) != 0,
                is_active: CGDisplayIsActive(display) != 0,
                is_asleep: CGDisplayIsAsleep(display) != 0,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CaptureAvailability, DisplayState};

    #[test]
    fn display_must_be_online_active_and_awake() {
        assert!(CaptureAvailability::from_display_state(DisplayState {
            is_online: true,
            is_active: true,
            is_asleep: false,
        })
        .is_allowed());

        assert!(!CaptureAvailability::from_display_state(DisplayState {
            is_online: true,
            is_active: false,
            is_asleep: false,
        })
        .is_allowed());

        assert!(!CaptureAvailability::from_display_state(DisplayState {
            is_online: true,
            is_active: true,
            is_asleep: true,
        })
        .is_allowed());
    }
}
