//! An object-safe keyboard interface so the service can hold any driver (a Rongyuan
//! board, a LampArray keyboard, or a simulator) behind one `Box<dyn Board>`.
//!
//! What a board can do is in [`DeviceInfo::features`]; the service asks before calling.
//! Anything a driver doesn't implement answers [`DeviceError::Unsupported`].

use keylume_device::boards::Features;
use keylume_device::{DeviceError, DeviceInfo, Keyboard, Result, Transport};
use keylume_proto::keymap::{KeyAction, Keymap};
use keylume_proto::macros::Macro;
use keylume_proto::picture::Frame;
use keylume_proto::settings::{KeyboardOptions, ReportRate, SleepTimers};
use keylume_proto::stream::BANDS;
use keylume_proto::{Effect, Layout, Rgb, SideLight};

pub trait Board: Send {
    fn info(&self) -> &DeviceInfo;
    fn features(&self) -> &Features {
        &self.info().features
    }
    /// The board's keys and lights (slot = the light's index in a [`Frame`]).
    fn layout(&self) -> &Layout;
    fn is_busy(&self) -> bool {
        false
    }
    fn firmware_version(&self) -> Result<u16> {
        Err(DeviceError::Unsupported("report a firmware version"))
    }
    fn effect(&self) -> Result<Effect> {
        Err(DeviceError::Unsupported("say which animation it shows"))
    }
    fn set_effect(&self, _e: &Effect) -> Result<()> {
        Err(DeviceError::Unsupported("run animations itself"))
    }
    fn side_light(&self) -> Result<SideLight> {
        Err(DeviceError::Unsupported("light a side strip"))
    }
    fn set_side_light(&self, _s: &SideLight) -> Result<()> {
        Err(DeviceError::Unsupported("light a side strip"))
    }
    /// Show a picture (stored in `layer` on boards with picture layers).
    fn write_picture(&self, layer: u8, frame: &Frame, brightness: u8) -> Result<()>;
    fn read_picture(&self, _layer: u8) -> Result<Frame> {
        Err(DeviceError::Unsupported("read its lights back"))
    }
    fn select_layer(&self, _layer: u8, _brightness: u8) -> Result<()> {
        Err(DeviceError::Unsupported("store pictures"))
    }
    fn stream_levels(&self, _l: &[u8; BANDS]) -> Result<()> {
        Err(DeviceError::Unsupported("show music bars"))
    }
    /// Music bars in a colour (a Rongyuan board colours them from its base effect, so it
    /// ignores `color` and `rainbow`).
    fn stream_bars(&self, l: &[u8; BANDS], _color: Rgb, _rainbow: bool) -> Result<()> {
        self.stream_levels(l)
    }
    fn stream_color(&self, _c: Rgb) -> Result<()> {
        Err(DeviceError::Unsupported("show a live colour"))
    }
    fn keymap(&self, _profile: u8) -> Result<Keymap> {
        Err(DeviceError::Unsupported("remap keys"))
    }
    fn set_keymap(&self, _profile: u8, _km: &Keymap) -> Result<()> {
        Err(DeviceError::Unsupported("remap keys"))
    }
    fn fn_keymap(&self, _idx: u8) -> Result<Keymap> {
        Err(DeviceError::Unsupported("remap keys"))
    }
    fn set_fn_keymap(&self, _idx: u8, _km: &Keymap) -> Result<()> {
        Err(DeviceError::Unsupported("remap keys"))
    }
    fn macro_at(&self, _i: u8) -> Result<Macro> {
        Err(DeviceError::Unsupported("store macros"))
    }
    fn set_macro(&self, _i: u8, _m: &Macro) -> Result<()> {
        Err(DeviceError::Unsupported("store macros"))
    }
    fn report_rate(&self) -> Result<ReportRate> {
        Err(DeviceError::Unsupported("change its settings"))
    }
    fn set_report_rate(&self, _profile: u8, _r: ReportRate) -> Result<()> {
        Err(DeviceError::Unsupported("change its settings"))
    }
    fn debounce(&self) -> Result<u8> {
        Err(DeviceError::Unsupported("change its settings"))
    }
    fn set_debounce(&self, _v: u8) -> Result<()> {
        Err(DeviceError::Unsupported("change its settings"))
    }
    fn sleep_timers(&self) -> Result<SleepTimers> {
        Err(DeviceError::Unsupported("change its settings"))
    }
    fn set_sleep_timers(&self, _t: SleepTimers) -> Result<()> {
        Err(DeviceError::Unsupported("change its settings"))
    }
    fn profile(&self) -> Result<u8> {
        Err(DeviceError::Unsupported("switch onboard profiles"))
    }
    fn set_profile(&self, _p: u8) -> Result<()> {
        Err(DeviceError::Unsupported("switch onboard profiles"))
    }
    fn options(&self) -> Result<KeyboardOptions> {
        Err(DeviceError::Unsupported("change its settings"))
    }
    fn set_options(&self, _profile: u8, _o: KeyboardOptions) -> Result<()> {
        Err(DeviceError::Unsupported("change its settings"))
    }
    fn factory_reset(&self) -> Result<()> {
        Err(DeviceError::Unsupported("be reset"))
    }
    /// A bare GET that only tells whether the keyboard is still there.
    fn probe(&self) -> Result<()>;
    /// Wait until the board takes commands again.
    fn settle(&self) {}
    /// Hand the lights back to the board (a host-driven board shows its own lighting
    /// again); called when the service lets go of it.
    fn release(&self) {}
    /// Simulator counters, for tests (None on hardware).
    fn sim_stats(&self) -> Option<keylume_device::sim::SimStats> {
        None
    }
}

impl<T: Transport + Send> Board for Keyboard<T> {
    fn info(&self) -> &DeviceInfo {
        &self.info
    }
    fn layout(&self) -> &Layout {
        &self.layout
    }
    fn is_busy(&self) -> bool {
        Keyboard::is_busy(self)
    }
    fn firmware_version(&self) -> Result<u16> {
        Keyboard::firmware_version(self)
    }
    fn effect(&self) -> Result<Effect> {
        Keyboard::effect(self)
    }
    fn set_effect(&self, e: &Effect) -> Result<()> {
        Keyboard::set_effect(self, e)
    }
    fn side_light(&self) -> Result<SideLight> {
        Keyboard::side_light(self)
    }
    fn set_side_light(&self, s: &SideLight) -> Result<()> {
        Keyboard::set_side_light(self, s)
    }
    fn write_picture(&self, l: u8, f: &Frame, b: u8) -> Result<()> {
        Keyboard::write_picture(self, l, f, b)
    }
    fn read_picture(&self, l: u8) -> Result<Frame> {
        Keyboard::read_picture(self, l)
    }
    fn select_layer(&self, l: u8, b: u8) -> Result<()> {
        Keyboard::select_layer(self, l, b)
    }
    fn stream_levels(&self, l: &[u8; BANDS]) -> Result<()> {
        Keyboard::stream_levels(self, l)
    }
    fn stream_color(&self, c: Rgb) -> Result<()> {
        Keyboard::stream_color(self, c)
    }
    fn keymap(&self, p: u8) -> Result<Keymap> {
        Keyboard::keymap(self, p)
    }
    fn set_keymap(&self, p: u8, k: &Keymap) -> Result<()> {
        Keyboard::set_keymap(self, p, k)
    }
    fn fn_keymap(&self, i: u8) -> Result<Keymap> {
        Keyboard::fn_keymap(self, i)
    }
    fn set_fn_keymap(&self, i: u8, k: &Keymap) -> Result<()> {
        Keyboard::set_fn_keymap(self, i, k)
    }
    fn macro_at(&self, i: u8) -> Result<Macro> {
        Keyboard::macro_at(self, i)
    }
    fn set_macro(&self, i: u8, m: &Macro) -> Result<()> {
        Keyboard::set_macro(self, i, m)
    }
    fn report_rate(&self) -> Result<ReportRate> {
        Keyboard::report_rate(self)
    }
    fn set_report_rate(&self, p: u8, r: ReportRate) -> Result<()> {
        Keyboard::set_report_rate(self, p, r)
    }
    fn debounce(&self) -> Result<u8> {
        Keyboard::debounce(self)
    }
    fn set_debounce(&self, v: u8) -> Result<()> {
        Keyboard::set_debounce(self, v)
    }
    fn sleep_timers(&self) -> Result<SleepTimers> {
        Keyboard::sleep_timers(self)
    }
    fn set_sleep_timers(&self, t: SleepTimers) -> Result<()> {
        Keyboard::set_sleep_timers(self, t)
    }
    fn profile(&self) -> Result<u8> {
        Keyboard::profile(self)
    }
    fn set_profile(&self, p: u8) -> Result<()> {
        Keyboard::set_profile(self, p)
    }
    fn options(&self) -> Result<KeyboardOptions> {
        Keyboard::options(self)
    }
    fn set_options(&self, p: u8, o: KeyboardOptions) -> Result<()> {
        Keyboard::set_options(self, p, o)
    }
    fn factory_reset(&self) -> Result<()> {
        Keyboard::factory_reset(self)
    }
    fn probe(&self) -> Result<()> {
        Keyboard::probe(self)
    }
    fn settle(&self) {
        Keyboard::settle(self)
    }
    fn sim_stats(&self) -> Option<keylume_device::sim::SimStats> {
        self.transport().sim_stats()
    }
}

/// Change some keys of the Fn layer (`fn_layer`) or of onboard `profile`'s base layer,
/// keeping the rest, then read the layer back. A write the keyboard didn't keep is written
/// once more; the slots it still didn't keep are returned (empty: all done).
pub fn write_keys(b: &dyn Board, fn_layer: bool, profile: u8, keys: &[(usize, KeyAction)]) -> Result<Vec<usize>> {
    let read = || if fn_layer { b.fn_keymap(0) } else { b.keymap(profile) };
    let mut km = read()?;
    for &(slot, action) in keys {
        km.0[slot] = action;
    }
    let mut lost = Vec::new();
    for _ in 0..2 {
        if fn_layer {
            b.set_fn_keymap(0, &km)?;
        } else {
            b.set_keymap(profile, &km)?;
        }
        let now = read()?;
        lost = keys.iter().filter(|&&(slot, action)| now.0[slot] != action).map(|&(slot, _)| slot).collect();
        if lost.is_empty() {
            break;
        }
    }
    Ok(lost)
}

#[cfg(test)]
mod tests {
    use super::*;
    use keylume_device::sim::SimKeyboard;

    /// A keyboard that answers a Fn-layer write but doesn't keep it.
    struct Forgets(Keyboard<keylume_device::sim::SimKeyboard>);
    impl Board for Forgets {
        fn info(&self) -> &DeviceInfo {
            Board::info(&self.0)
        }
        fn layout(&self) -> &Layout {
            Board::layout(&self.0)
        }
        fn write_picture(&self, l: u8, f: &Frame, br: u8) -> Result<()> {
            Board::write_picture(&self.0, l, f, br)
        }
        fn fn_keymap(&self, i: u8) -> Result<Keymap> {
            Board::fn_keymap(&self.0, i)
        }
        fn set_fn_keymap(&self, _i: u8, _k: &Keymap) -> Result<()> {
            Ok(())
        }
        fn probe(&self) -> Result<()> {
            Ok(())
        }
    }

    #[test]
    fn a_key_the_keyboard_keeps_is_done_and_one_it_forgets_is_reported() {
        let f5 = KeyAction::Key { code: 0x3E, modifier: 0, code2: 0 };
        let b = SimKeyboard::keyboard(50);
        let before = Board::fn_keymap(&b, 0).unwrap();
        assert!(write_keys(&b, true, 0, &[(7, f5)]).unwrap().is_empty());
        let after = Board::fn_keymap(&b, 0).unwrap();
        assert_eq!(after.0[7], f5);
        assert!((0..after.0.len()).filter(|&i| i != 7).all(|i| after.0[i] == before.0[i]), "the other keys stay");

        let forgets = Forgets(SimKeyboard::keyboard(50));
        assert_eq!(write_keys(&forgets, true, 0, &[(7, f5), (8, KeyAction::Disabled)]).unwrap(), vec![7]);
    }
}
