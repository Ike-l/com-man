use std::process;

use sysinfo::{Pid, System};

pub fn get_memory_usage() -> Option<f64> {
    let mut sys = System::new();
    let pid = Pid::from_u32(process::id());

    sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);

    if let Some(process) = sys.process(pid) {
        return Some(((process.memory() as f64 / 1024.0 / 1024.0) * 100.0).round() / 100.0);
    }

    None
}