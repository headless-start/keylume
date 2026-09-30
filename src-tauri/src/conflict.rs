//! Detect other programs that drive the keyboard (the vendor driver) and pause Keylume
//! while they run: two programs talking to the firmware at once wedge its config channel.

use std::sync::Arc;
use std::thread;
use std::time::Duration;

use keylume_core::Service;
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System};

/// Process-name prefixes of known conflicting apps (lower-case).
const CONFLICTS: &[(&str, &str)] = &[("epomaker driver", "EPOMAKER Driver"), ("iot_driver", "EPOMAKER Driver")];

fn matches(name: &str) -> Option<&'static str> {
    let n = name.to_lowercase();
    CONFLICTS.iter().find(|(prefix, _)| n.starts_with(prefix)).map(|(_, label)| *label)
}

/// One pass over the process list: the conflicting app running, if any.
/// Names only: no CPU or memory sampling, so it's cheap.
fn scan(sys: &mut System) -> Option<&'static str> {
    sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::new());
    sys.processes().values().find_map(|p| matches(&p.name().to_string_lossy()))
}

/// Poll every few seconds; pause/resume the service when a conflicting app comes and goes.
pub fn watch(service: Arc<Service>) {
    thread::Builder::new()
        .name("keylume-watch".into())
        .spawn(move || {
            let mut sys = System::new_with_specifics(RefreshKind::new());
            let mut last = None;
            loop {
                let now = scan(&mut sys);
                if now != last {
                    service.pause(now.map(|label| format!("{label} is running")));
                    last = now;
                }
                thread::sleep(Duration::from_secs(3));
            }
        })
        .expect("spawn process watcher");
}

/// Close the conflicting programs (only ever called from an explicit user click).
pub fn close_all() -> usize {
    let mut sys = System::new_with_specifics(RefreshKind::new());
    sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::new());
    sys.processes().values().filter(|p| matches(&p.name().to_string_lossy()).is_some()).filter(|p| p.kill()).count()
}

#[cfg(test)]
mod tests {
    use super::matches;

    #[test]
    fn recognises_the_vendor_driver() {
        assert_eq!(matches("EPOMAKER Driver.exe"), Some("EPOMAKER Driver"));
        assert_eq!(matches("iot_driver_v192.exe"), Some("EPOMAKER Driver"));
        assert_eq!(matches("iot_driver"), Some("EPOMAKER Driver"));
        assert_eq!(matches("keylume-app.exe"), None);
        assert_eq!(matches("explorer.exe"), None);
    }
}
