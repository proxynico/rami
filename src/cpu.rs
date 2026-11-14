use crate::model::CpuSnapshot;
use libc::{
    host_processor_info, integer_t, mach_msg_type_number_t, natural_t, vm_address_t, vm_deallocate,
    vm_size_t, CPU_STATE_IDLE, CPU_STATE_MAX, CPU_STATE_NICE, CPU_STATE_SYSTEM, CPU_STATE_USER,
    PROCESSOR_CPU_LOAD_INFO,
};
use std::io;
use std::mem::size_of;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ProcessorTicks {
    user: u32,
    system: u32,
    idle: u32,
    nice: u32,
}

impl ProcessorTicks {
    const fn new(user: u32, system: u32, idle: u32, nice: u32) -> Self {
        Self {
            user,
            system,
            idle,
            nice,
        }
    }
}

pub(crate) struct CpuSampler {
    host_port: libc::mach_port_t,
    previous: Option<Vec<ProcessorTicks>>,
}

impl CpuSampler {
    pub(crate) fn new() -> Self {
        #[allow(deprecated)]
        let host_port = unsafe { libc::mach_host_self() };
        Self {
            host_port,
            previous: None,
        }
    }

    pub(crate) fn sample(&mut self) -> io::Result<Option<CpuSnapshot>> {
        let previous = self.previous.take();
        let current = read_processor_ticks(self.host_port)?;
        let snapshot = previous.and_then(|previous| snapshot_from_ticks(&previous, &current));
        self.previous = Some(current);
        Ok(snapshot)
    }

    pub(crate) fn reset(&mut self) {
        self.previous = None;
    }
}

fn read_processor_ticks(host_port: libc::mach_port_t) -> io::Result<Vec<ProcessorTicks>> {
    let mut processor_count: natural_t = 0;
    let mut info = std::ptr::null_mut::<integer_t>();
    let mut info_count: mach_msg_type_number_t = 0;
    let result = unsafe {
        host_processor_info(
            host_port,
            PROCESSOR_CPU_LOAD_INFO,
            &mut processor_count,
            &mut info,
            &mut info_count,
        )
    };
    if result != 0 {
        return Err(io::Error::other(format!(
            "host_processor_info failed with kern_return_t {result}"
        )));
    }

    let expected_count = processor_count as usize * CPU_STATE_MAX as usize;
    let actual_count = info_count as usize;
    let copied = if info.is_null() || actual_count < expected_count {
        Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            format!(
                "insufficient processor tick count: expected at least {expected_count}, got {actual_count}"
            ),
        ))
    } else {
        let raw = unsafe { std::slice::from_raw_parts(info, actual_count) };
        let mut ticks = Vec::with_capacity(processor_count as usize);
        for cpu in 0..processor_count as usize {
            let offset = cpu * CPU_STATE_MAX as usize;
            ticks.push(ProcessorTicks::new(
                raw[offset + CPU_STATE_USER as usize] as u32,
                raw[offset + CPU_STATE_SYSTEM as usize] as u32,
                raw[offset + CPU_STATE_IDLE as usize] as u32,
                raw[offset + CPU_STATE_NICE as usize] as u32,
            ));
        }
        Ok(ticks)
    };

    if !info.is_null() {
        let size = actual_count.saturating_mul(size_of::<integer_t>()) as vm_size_t;
        let deallocate_result = unsafe {
            #[allow(deprecated)]
            vm_deallocate(libc::mach_task_self(), info as vm_address_t, size)
        };
        if deallocate_result != 0 {
            return Err(io::Error::other(format!(
                "vm_deallocate failed with kern_return_t {deallocate_result}"
            )));
        }
    }

    copied
}

fn snapshot_from_ticks(
    previous: &[ProcessorTicks],
    current: &[ProcessorTicks],
) -> Option<CpuSnapshot> {
    if previous.len() != current.len() || current.is_empty() {
        return None;
    }
    let (mut user, mut system, mut idle) = (0_u64, 0_u64, 0_u64);
    for (before, after) in previous.iter().zip(current) {
        user += after.user.wrapping_sub(before.user) as u64
            + after.nice.wrapping_sub(before.nice) as u64;
        system += after.system.wrapping_sub(before.system) as u64;
        idle += after.idle.wrapping_sub(before.idle) as u64;
    }
    let total = user + system + idle;
    Some(CpuSnapshot {
        user_percent: percent(user, total),
        system_percent: percent(system, total),
    })
}

fn percent(part: u64, total: u64) -> u8 {
    if total == 0 {
        return 0;
    }
    (part as f64 / total as f64 * 100.0)
        .round()
        .clamp(0.0, 100.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn successive_processor_ticks_produce_clamped_utilization() {
        let previous = [ProcessorTicks::new(100, 40, 360, 0)];
        let current = [ProcessorTicks::new(175, 65, 410, 0)];

        let snapshot = snapshot_from_ticks(&previous, &current)
            .expect("matching samples should produce a snapshot");

        assert_eq!(snapshot.user_percent, 50);
        assert_eq!(snapshot.system_percent, 17);
    }

    #[test]
    fn wrapping_tick_counters_stay_within_percentage_bounds() {
        let previous = [ProcessorTicks::new(u32::MAX - 4, 20, 30, 0)];
        let current = [ProcessorTicks::new(5, 20, 30, 0)];

        let snapshot = snapshot_from_ticks(&previous, &current).unwrap();

        assert_eq!(snapshot.user_percent, 100);
        assert!(snapshot.system_percent <= 100);
    }

    #[test]
    #[ignore = "requires live macOS processor counters"]
    fn smoke_samples_live_processor_ticks() {
        let started = std::time::Instant::now();
        let mut sampler = CpuSampler::new();
        assert_eq!(sampler.sample().unwrap(), None);
        std::thread::sleep(std::time::Duration::from_millis(20));
        let snapshot = sampler
            .sample()
            .expect("live processor tick read should succeed")
            .expect("second live sample should produce a delta");
        let elapsed = started.elapsed();
        eprintln!(
            "CPU tick sample took {elapsed:?}: user {}% system {}%",
            snapshot.user_percent, snapshot.system_percent
        );
        assert!(snapshot.user_percent <= 100);
        assert!(snapshot.system_percent <= 100);
    }
}
