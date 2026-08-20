use crate::iokit::{
    CFDataGetBytePtr, CFDataGetLength, CFDataGetTypeID, CFGetTypeID, CFNumberGetTypeID,
    CFNumberGetValue, CFStringCreateWithCString, CfIndex, CfObject, IOIteratorNext,
    IORegistryEntryCreateCFProperty, IORegistryEntryFromPath, IORegistryEntryGetChildIterator,
    IoObject, IoObjectId, CF_STRING_ENCODING_UTF8,
};
use crate::model::CpuSnapshot;
use libc::{
    host_processor_info, integer_t, mach_msg_type_number_t, natural_t, vm_address_t, vm_deallocate,
    vm_size_t, CPU_STATE_IDLE, CPU_STATE_MAX, CPU_STATE_NICE, CPU_STATE_SYSTEM, CPU_STATE_USER,
    PROCESSOR_CPU_LOAD_INFO,
};
use std::ffi::CStr;
use std::io;
use std::mem::size_of;

const CF_NUMBER_SINT64_TYPE: CfIndex = 4;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CoreKind {
    Efficiency,
    Performance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CoreTopology {
    efficiency: Vec<usize>,
    performance: Vec<usize>,
    processor_count: usize,
}

impl CoreTopology {
    fn from_logical_cpus(cores: &[(usize, CoreKind)], processor_count: usize) -> Option<Self> {
        if processor_count == 0 || cores.len() != processor_count {
            return None;
        }
        let mut kinds = vec![None; processor_count];
        for &(logical_id, kind) in cores {
            let slot = kinds.get_mut(logical_id)?;
            if slot.replace(kind).is_some() {
                return None;
            }
        }
        let efficiency: Vec<_> = kinds
            .iter()
            .enumerate()
            .filter_map(|(index, kind)| (*kind == Some(CoreKind::Efficiency)).then_some(index))
            .collect();
        let performance: Vec<_> = kinds
            .iter()
            .enumerate()
            .filter_map(|(index, kind)| (*kind == Some(CoreKind::Performance)).then_some(index))
            .collect();
        if efficiency.is_empty() || performance.is_empty() {
            return None;
        }
        Some(Self {
            efficiency,
            performance,
            processor_count,
        })
    }
}

#[derive(Debug)]
struct CpuTracker {
    previous: Option<Vec<ProcessorTicks>>,
    topology: Option<CoreTopology>,
}

impl CpuTracker {
    fn new(topology: Option<CoreTopology>) -> Self {
        Self {
            previous: None,
            topology,
        }
    }

    fn record(&mut self, current: Vec<ProcessorTicks>) -> Option<CpuSnapshot> {
        let snapshot = self
            .previous
            .as_deref()
            .and_then(|previous| snapshot_from_ticks(previous, &current, self.topology.as_ref()));
        self.previous = Some(current);
        snapshot
    }

    fn reset(&mut self) {
        self.previous = None;
    }
}

pub(crate) struct CpuSampler {
    host_port: libc::mach_port_t,
    tracker: CpuTracker,
    topology_loaded: bool,
}

impl CpuSampler {
    pub(crate) fn new() -> Self {
        #[allow(deprecated)]
        let host_port = unsafe { libc::mach_host_self() };
        Self {
            host_port,
            tracker: CpuTracker::new(None),
            topology_loaded: false,
        }
    }

    pub(crate) fn sample(&mut self) -> io::Result<Option<CpuSnapshot>> {
        match read_processor_ticks(self.host_port) {
            Ok(ticks) => {
                if !self.topology_loaded {
                    self.tracker.topology = detect_topology(ticks.len());
                    self.topology_loaded = true;
                }
                Ok(self.tracker.record(ticks))
            }
            Err(error) => {
                self.tracker.reset();
                Err(error)
            }
        }
    }

    pub(crate) fn reset(&mut self) {
        self.tracker.reset();
    }
}

impl Default for CpuSampler {
    fn default() -> Self {
        Self::new()
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

    let expected_count = (processor_count as usize)
        .checked_mul(CPU_STATE_MAX as usize)
        .ok_or_else(|| io::Error::other("processor tick count overflow"))?;
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

fn detect_topology(processor_count: usize) -> Option<CoreTopology> {
    // IODeviceTree exposes the logical CPU ID and E/P cluster identity
    // together. Perf-level counts alone cannot establish this ordering.
    let cpus =
        IoObject::new(unsafe { IORegistryEntryFromPath(0, c"IODeviceTree:/cpus".as_ptr()) })?;
    let mut iterator = 0;
    let result = unsafe {
        IORegistryEntryGetChildIterator(cpus.id(), c"IODeviceTree".as_ptr(), &mut iterator)
    };
    if result != 0 {
        return None;
    }
    let iterator = IoObject::new(iterator)?;
    let mut cores = Vec::with_capacity(processor_count);
    loop {
        let Some(cpu) = IoObject::new(unsafe { IOIteratorNext(iterator.id()) }) else {
            break;
        };
        cores.push((
            registry_number(cpu.id(), c"logical-cpu-id")?,
            registry_core_kind(cpu.id())?,
        ));
    }
    CoreTopology::from_logical_cpus(&cores, processor_count)
}

fn registry_number(entry: IoObjectId, key: &CStr) -> Option<usize> {
    let property = registry_property(entry, key)?;
    if unsafe { CFGetTypeID(property.get()) } != unsafe { CFNumberGetTypeID() } {
        return None;
    }
    let mut value = 0_i64;
    let converted = unsafe {
        CFNumberGetValue(
            property.get(),
            CF_NUMBER_SINT64_TYPE,
            (&mut value as *mut i64).cast(),
        )
    };
    (converted != 0)
        .then(|| usize::try_from(value).ok())
        .flatten()
}

fn registry_core_kind(entry: IoObjectId) -> Option<CoreKind> {
    let property = registry_property(entry, c"cluster-type")?;
    if unsafe { CFGetTypeID(property.get()) } != unsafe { CFDataGetTypeID() } {
        return None;
    }
    let length = unsafe { CFDataGetLength(property.get()) };
    if length <= 0 {
        return None;
    }
    let bytes = unsafe { CFDataGetBytePtr(property.get()) };
    if bytes.is_null() {
        return None;
    }
    let bytes = unsafe { std::slice::from_raw_parts(bytes, length as usize) };
    core_kind_from_bytes(bytes)
}

fn core_kind_from_bytes(bytes: &[u8]) -> Option<CoreKind> {
    match bytes.iter().copied().find(|byte| *byte != 0)? {
        b'E' => Some(CoreKind::Efficiency),
        b'P' => Some(CoreKind::Performance),
        _ => None,
    }
}

fn registry_property(entry: IoObjectId, key: &CStr) -> Option<CfObject> {
    let key = CfObject::new(unsafe {
        CFStringCreateWithCString(std::ptr::null(), key.as_ptr(), CF_STRING_ENCODING_UTF8)
    })?;
    CfObject::new(unsafe { IORegistryEntryCreateCFProperty(entry, key.get(), std::ptr::null(), 0) })
}

fn snapshot_from_ticks(
    previous: &[ProcessorTicks],
    current: &[ProcessorTicks],
    topology: Option<&CoreTopology>,
) -> Option<CpuSnapshot> {
    if previous.len() != current.len() || current.is_empty() {
        return None;
    }

    let totals = tick_totals(previous, current);

    let compatible_topology = topology.filter(|topology| topology.processor_count == current.len());
    let efficiency_percent = compatible_topology
        .and_then(|topology| utilization_for_indices(previous, current, &topology.efficiency));
    let performance_percent = compatible_topology
        .and_then(|topology| utilization_for_indices(previous, current, &topology.performance));

    Some(CpuSnapshot {
        user_percent: percent(totals.user, totals.total()),
        system_percent: percent(totals.system, totals.total()),
        efficiency_percent,
        performance_percent,
    })
}

fn utilization_for_indices(
    previous: &[ProcessorTicks],
    current: &[ProcessorTicks],
    indices: &[usize],
) -> Option<u8> {
    let mut totals = TickTotals::default();
    for &index in indices {
        totals.add(previous.get(index)?, current.get(index)?);
    }
    Some(percent(
        totals.user.saturating_add(totals.system),
        totals.total(),
    ))
}

#[derive(Debug, Clone, Copy, Default)]
struct TickTotals {
    user: u64,
    system: u64,
    idle: u64,
}

impl TickTotals {
    fn total(self) -> u64 {
        self.user
            .saturating_add(self.system)
            .saturating_add(self.idle)
    }

    fn add(&mut self, before: &ProcessorTicks, after: &ProcessorTicks) {
        self.user = self
            .user
            .saturating_add(after.user.wrapping_sub(before.user) as u64)
            .saturating_add(after.nice.wrapping_sub(before.nice) as u64);
        self.system = self
            .system
            .saturating_add(after.system.wrapping_sub(before.system) as u64);
        self.idle = self
            .idle
            .saturating_add(after.idle.wrapping_sub(before.idle) as u64);
    }
}

fn tick_totals(previous: &[ProcessorTicks], current: &[ProcessorTicks]) -> TickTotals {
    previous
        .iter()
        .zip(current)
        .fold(TickTotals::default(), |mut totals, (before, after)| {
            totals.add(before, after);
            totals
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

        let snapshot = snapshot_from_ticks(&previous, &current, None)
            .expect("matching samples should produce a snapshot");

        assert_eq!(snapshot.user_percent, 50);
        assert_eq!(snapshot.system_percent, 17);
        assert_eq!(snapshot.efficiency_percent, None);
        assert_eq!(snapshot.performance_percent, None);
    }

    #[test]
    fn io_registry_logical_ids_drive_e_and_p_core_aggregates() {
        let previous = [ProcessorTicks::new(0, 0, 0, 0); 4];
        let current = [
            ProcessorTicks::new(15, 5, 80, 0),
            ProcessorTicks::new(15, 5, 80, 0),
            ProcessorTicks::new(60, 20, 20, 0),
            ProcessorTicks::new(60, 20, 20, 0),
        ];
        let topology = CoreTopology::from_logical_cpus(
            &[
                (0, CoreKind::Efficiency),
                (1, CoreKind::Efficiency),
                (2, CoreKind::Performance),
                (3, CoreKind::Performance),
            ],
            4,
        )
        .expect("complete logical CPU identities should produce a topology");

        let snapshot = snapshot_from_ticks(&previous, &current, Some(&topology)).unwrap();

        assert_eq!(snapshot.efficiency_percent, Some(20));
        assert_eq!(snapshot.performance_percent, Some(80));
    }

    #[test]
    fn tracker_requires_two_fresh_samples_after_reset() {
        let mut tracker = CpuTracker::new(None);
        let first = vec![ProcessorTicks::new(10, 10, 80, 0)];
        let second = vec![ProcessorTicks::new(20, 20, 160, 0)];

        assert_eq!(tracker.record(first.clone()), None);
        assert!(tracker.record(second.clone()).is_some());

        tracker.reset();
        assert_eq!(tracker.record(second), None);
    }

    #[test]
    fn unknown_duplicate_or_incomplete_core_mappings_do_not_mislabel_cores() {
        assert_eq!(core_kind_from_bytes(b"X\0"), None);
        assert!(CoreTopology::from_logical_cpus(
            &[(0, CoreKind::Efficiency), (0, CoreKind::Performance)],
            2,
        )
        .is_none());
        assert!(CoreTopology::from_logical_cpus(
            &[(0, CoreKind::Efficiency), (1, CoreKind::Performance)],
            4,
        )
        .is_none());
    }

    #[test]
    fn topology_count_mismatch_keeps_overall_cpu_without_core_labels() {
        let topology = CoreTopology::from_logical_cpus(
            &[
                (0, CoreKind::Efficiency),
                (1, CoreKind::Efficiency),
                (2, CoreKind::Performance),
                (3, CoreKind::Performance),
            ],
            4,
        )
        .unwrap();
        let previous = [ProcessorTicks::new(0, 0, 0, 0); 3];
        let current = [ProcessorTicks::new(40, 10, 50, 0); 3];

        let snapshot = snapshot_from_ticks(&previous, &current, Some(&topology)).unwrap();

        assert_eq!(snapshot.user_percent, 40);
        assert_eq!(snapshot.system_percent, 10);
        assert_eq!(snapshot.efficiency_percent, None);
        assert_eq!(snapshot.performance_percent, None);
    }

    #[test]
    fn wrapping_tick_counters_stay_within_percentage_bounds() {
        let previous = [ProcessorTicks::new(u32::MAX - 4, 20, 30, 0)];
        let current = [ProcessorTicks::new(5, 20, 30, 0)];

        let snapshot = snapshot_from_ticks(&previous, &current, None).unwrap();

        assert_eq!(snapshot.user_percent, 100);
        assert!(snapshot.system_percent <= 100);
    }

    #[test]
    #[ignore = "requires live macOS processor counters"]
    fn smoke_samples_live_processor_ticks() {
        let started = std::time::Instant::now();
        let mut sampler = CpuSampler::new();
        assert_eq!(sampler.sample().unwrap(), None);
        assert!(sampler.tracker.topology.is_some());
        std::thread::sleep(std::time::Duration::from_millis(20));
        let snapshot = sampler
            .sample()
            .expect("live processor tick read should succeed")
            .expect("second live sample should produce a delta");
        let elapsed = started.elapsed();
        eprintln!(
            "CPU tick sample took {elapsed:?}: user {}% system {}% e-cores {:?} p-cores {:?}",
            snapshot.user_percent,
            snapshot.system_percent,
            snapshot.efficiency_percent,
            snapshot.performance_percent
        );
        assert!(snapshot.user_percent <= 100);
        assert!(snapshot.system_percent <= 100);
        assert!(snapshot.efficiency_percent.is_some());
        assert!(snapshot.performance_percent.is_some());
    }
}
