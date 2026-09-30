//! Live-effect inputs: system audio (loopback), CPU load and the screen's average
//! colour. Each capture runs on its own thread only while a live effect needs it.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use keylume_live::{Inputs, Needs};

#[derive(Default)]
struct Flags {
    audio: AtomicBool,
    cpu: AtomicBool,
    screen: AtomicBool,
}

pub struct Captures {
    latest: Arc<Mutex<Inputs>>,
    want: Arc<Flags>,
    running: Arc<Flags>,
}

impl Captures {
    pub fn new() -> Self {
        Captures { latest: Arc::new(Mutex::new(Inputs::default())), want: Arc::new(Flags::default()), running: Arc::new(Flags::default()) }
    }

    pub fn snapshot(&self) -> Inputs {
        self.latest.lock().unwrap().clone()
    }

    /// Start the captures `needs` asks for; stop the rest.
    pub fn set_needs(&self, needs: Needs) {
        self.want.audio.store(needs.audio, Ordering::SeqCst);
        self.want.cpu.store(needs.cpu, Ordering::SeqCst);
        self.want.screen.store(needs.screen, Ordering::SeqCst);
        if needs.cpu && !self.running.cpu.swap(true, Ordering::SeqCst) {
            spawn_cpu(self.latest.clone(), self.want.clone(), self.running.clone());
        }
        if needs.audio && !self.running.audio.swap(true, Ordering::SeqCst) {
            audio::spawn(self.latest.clone(), self.want.clone(), self.running.clone());
        }
        if needs.screen && !self.running.screen.swap(true, Ordering::SeqCst) {
            screen::spawn(self.latest.clone(), self.want.clone(), self.running.clone());
        }
    }
}

fn spawn_cpu(latest: Arc<Mutex<Inputs>>, want: Arc<Flags>, running: Arc<Flags>) {
    thread::spawn(move || {
        use sysinfo::System;
        let mut sys = System::new();
        let mut smooth = 0.0f32;
        while want.cpu.load(Ordering::SeqCst) {
            sys.refresh_cpu_usage();
            let now = sys.global_cpu_usage() / 100.0;
            smooth += (now - smooth) * 0.3;
            latest.lock().unwrap().cpu = Some(smooth.clamp(0.0, 1.0));
            thread::sleep(Duration::from_millis(400));
        }
        latest.lock().unwrap().cpu = None;
        running.cpu.store(false, Ordering::SeqCst);
    });
}

#[cfg(windows)]
mod audio {
    //! WASAPI loopback via cpal: opening an *input* stream on the default *output*
    //! device captures whatever the PC is playing.
    use super::*;
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use keylume_live::audio::Analyzer;

    pub fn spawn(latest: Arc<Mutex<Inputs>>, want: Arc<Flags>, running: Arc<Flags>) {
        thread::spawn(move || {
            let run = || -> Option<()> {
                let host = cpal::default_host();
                let dev = host.default_output_device()?;
                let cfg = dev.default_output_config().ok()?;
                let channels = cfg.channels() as usize;
                let analyzer = Arc::new(Mutex::new(Analyzer::new(cfg.sample_rate().0)));
                let a = analyzer.clone();
                let stream = dev
                    .build_input_stream(
                        &cfg.into(),
                        move |data: &[f32], _| {
                            let mono: Vec<f32> = data.chunks(channels).map(|c| c.iter().sum::<f32>() / channels as f32).collect();
                            a.lock().unwrap().push(&mono);
                        },
                        |_| {},
                        None,
                    )
                    .ok()?;
                stream.play().ok()?;
                while want.audio.load(Ordering::SeqCst) {
                    let r = analyzer.lock().unwrap().analyze();
                    {
                        let mut l = latest.lock().unwrap();
                        l.bands = Some(r.bands);
                        l.loudness = Some(r.loudness);
                    }
                    thread::sleep(Duration::from_millis(33));
                }
                Some(())
            };
            let _ = run();
            let mut l = latest.lock().unwrap();
            l.bands = None;
            l.loudness = None;
            running.audio.store(false, Ordering::SeqCst);
        });
    }
}

#[cfg(windows)]
mod screen {
    //! Average screen colour: GDI StretchBlt of the whole desktop into one halftoned pixel.
    use super::*;
    use keylume_proto::Rgb;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Gdi::*;

    fn sample() -> Option<Rgb> {
        unsafe {
            let screen = GetDC(HWND::default());
            if screen.is_invalid() {
                return None;
            }
            let w = GetDeviceCaps(screen, HORZRES);
            let h = GetDeviceCaps(screen, VERTRES);
            let mem = CreateCompatibleDC(screen);
            let bmp = CreateCompatibleBitmap(screen, 1, 1);
            let old = SelectObject(mem, bmp);
            SetStretchBltMode(mem, HALFTONE);
            let ok = StretchBlt(mem, 0, 0, 1, 1, screen, 0, 0, w, h, SRCCOPY).as_bool();
            let px = GetPixel(mem, 0, 0).0;
            SelectObject(mem, old);
            let _ = DeleteObject(bmp);
            let _ = DeleteDC(mem);
            ReleaseDC(HWND::default(), screen);
            ok.then_some(Rgb((px & 0xFF) as u8, ((px >> 8) & 0xFF) as u8, ((px >> 16) & 0xFF) as u8))
        }
    }

    pub fn spawn(latest: Arc<Mutex<Inputs>>, want: Arc<Flags>, running: Arc<Flags>) {
        thread::spawn(move || {
            while want.screen.load(Ordering::SeqCst) {
                if let Some(c) = sample() {
                    latest.lock().unwrap().screen = Some(c);
                }
                thread::sleep(Duration::from_millis(50));
            }
            latest.lock().unwrap().screen = None;
            running.screen.store(false, Ordering::SeqCst);
        });
    }
}

#[cfg(not(windows))]
mod audio {
    use super::*;
    pub fn spawn(_: Arc<Mutex<Inputs>>, _: Arc<Flags>, running: Arc<Flags>) {
        running.audio.store(false, Ordering::SeqCst); // TODO(linux): PulseAudio monitor source
    }
}

#[cfg(not(windows))]
mod screen {
    use super::*;
    pub fn spawn(_: Arc<Mutex<Inputs>>, _: Arc<Flags>, running: Arc<Flags>) {
        running.screen.store(false, Ordering::SeqCst); // TODO(linux): portal screenshot
    }
}
