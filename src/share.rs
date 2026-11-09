//! Share over Tailscale: while the setting is on, rami answers HTTP on
//! `127.0.0.1:SHARE_PORT` with this Mac's readings, so another Mac's rami can
//! show them. rami never listens beyond localhost; `tailscale serve --bg --tcp
//! 47960 tcp://localhost:47960` forwards the port to the tailnet.
//!
//! The body is plain text, one `key=value` per line, after a `rami-share 1`
//! version line. CPU keys are absent until a second request gives the sampler
//! a delta, and GPU keys are absent when the driver reports nothing.
//!
//! `fetch` is the other side: the Mac showing a shared Mac asks over TCP and
//! `decode`s the answer.

use crate::cpu::CpuSampler;
use crate::gpu::GpuSampler;
use crate::iokit::{
    CFDataGetBytePtr, CFDataGetLength, CFDataGetTypeID, CFGetTypeID, CFStringCreateWithCString,
    CfObject, IORegistryEntryCreateCFProperty, IORegistryEntryFromPath, IoObject,
    CF_STRING_ENCODING_UTF8,
};
use crate::memory::MemorySampler;
use crate::model::{CpuSnapshot, GpuSnapshot, MemorySnapshot, PressureSource};
use std::fmt::Write as _;
use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

pub(crate) const SHARE_PORT: u16 = 47960;
const SHARE_VERSION: &str = "rami-share 1";
const IO_TIMEOUT: Duration = Duration::from_secs(1);
const REQUEST_DEADLINE: Duration = Duration::from_millis(500);
const MAX_REQUEST_BYTES: usize = 4096;
const FETCH_TIMEOUT: Duration = Duration::from_secs(2);

/// One answer: the memory, CPU, and GPU readings this Mac's dropdown shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SharedReading {
    pub(crate) name: String,
    /// The product family, e.g. "Mac mini"; the section's title.
    pub(crate) model: String,
    pub(crate) memory: MemorySnapshot,
    pub(crate) cpu: Option<CpuSnapshot>,
    pub(crate) gpu: Option<GpuSnapshot>,
}

/// What the dropdown knows about the shared Mac.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SharedMac {
    Hidden,
    /// Asking; `title` is the model from an earlier reading, if any.
    Loading {
        title: Option<String>,
    },
    Offline {
        title: Option<String>,
    },
    Reading(SharedReading),
}

pub(crate) fn encode(reading: &SharedReading) -> String {
    let memory = &reading.memory;
    let mut body = format!(
        "{SHARE_VERSION}\nname={}\nmodel={}\n",
        reading.name, reading.model
    );
    let mut line = |key: &str, value: &dyn std::fmt::Display| {
        let _ = writeln!(body, "{key}={value}");
    };
    line("used_bytes", &memory.used_bytes);
    line("total_bytes", &memory.total_bytes);
    line("used_percent", &memory.used_percent);
    line("pressure_percent", &memory.pressure_percent);
    line(
        "pressure_source",
        &match memory.pressure_source {
            PressureSource::Kernel => "kernel",
            PressureSource::AvailableFallback => "available",
        },
    );
    line("app_memory_bytes", &memory.app_memory_bytes);
    line("wired_bytes", &memory.wired_bytes);
    line("compressed_bytes", &memory.compressed_bytes);
    line("free_bytes", &memory.free_bytes);
    line("cached_bytes", &memory.cached_bytes);
    line("swap_used_bytes", &memory.swap_used_bytes);
    line("available_bytes", &memory.available_bytes);
    if let Some(cpu) = reading.cpu {
        line("cpu_user_percent", &cpu.user_percent);
        line("cpu_system_percent", &cpu.system_percent);
    }
    if let Some(gpu) = reading.gpu {
        line("gpu_percent", &gpu.utilization_percent);
        if let Some(renderer) = gpu.renderer_percent {
            line("gpu_renderer_percent", &renderer);
        }
        if let Some(tiler) = gpu.tiler_percent {
            line("gpu_tiler_percent", &tiler);
        }
    }
    body
}

/// Parse a `rami-share 1` body. `None` for another version or a missing
/// memory field; CPU and GPU are optional.
pub(crate) fn decode(body: &str) -> Option<SharedReading> {
    let mut lines = body.lines();
    if lines.next()? != SHARE_VERSION {
        return None;
    }
    let fields: std::collections::HashMap<&str, &str> =
        lines.filter_map(|line| line.split_once('=')).collect();
    let text = |key: &str| fields.get(key).map(|value| value.to_string());
    let number = |key: &str| fields.get(key)?.parse::<u64>().ok();
    let percent = |key: &str| fields.get(key)?.parse::<u8>().ok();
    let memory = MemorySnapshot {
        used_bytes: number("used_bytes")?,
        total_bytes: number("total_bytes")?,
        used_percent: percent("used_percent")?,
        pressure_percent: percent("pressure_percent")?,
        pressure_source: match *fields.get("pressure_source")? {
            "kernel" => PressureSource::Kernel,
            _ => PressureSource::AvailableFallback,
        },
        app_memory_bytes: number("app_memory_bytes")?,
        wired_bytes: number("wired_bytes")?,
        compressed_bytes: number("compressed_bytes")?,
        free_bytes: number("free_bytes")?,
        cached_bytes: number("cached_bytes")?,
        swap_used_bytes: number("swap_used_bytes")?,
        available_bytes: number("available_bytes")?,
    };
    let cpu = percent("cpu_user_percent")
        .zip(percent("cpu_system_percent"))
        .map(|(user_percent, system_percent)| CpuSnapshot {
            user_percent,
            system_percent,
        });
    let gpu = percent("gpu_percent").map(|utilization_percent| GpuSnapshot {
        utilization_percent,
        renderer_percent: percent("gpu_renderer_percent"),
        tiler_percent: percent("gpu_tiler_percent"),
    });
    Some(SharedReading {
        name: text("name")?,
        model: text("model")?,
        memory,
        cpu,
        gpu,
    })
}

/// Ask `host`'s rami for its shared reading. Blocks for up to a few seconds;
/// call it off the main thread.
pub(crate) fn fetch(host: &str) -> io::Result<SharedReading> {
    let address = (host, SHARE_PORT)
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| io::Error::other(format!("{host} did not resolve")))?;
    let deadline = Instant::now() + FETCH_TIMEOUT;
    let mut stream = TcpStream::connect_timeout(&address, FETCH_TIMEOUT)?;
    stream.set_write_timeout(Some(FETCH_TIMEOUT))?;
    stream.write_all(format!("GET / HTTP/1.0\r\nHost: {host}\r\n\r\n").as_bytes())?;
    let mut response = Vec::new();
    let mut buffer = [0u8; 1024];
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(io::Error::new(io::ErrorKind::TimedOut, "answer too slow"));
        }
        stream.set_read_timeout(Some(remaining))?;
        match stream.read(&mut buffer)? {
            0 => break,
            read => response.extend_from_slice(&buffer[..read]),
        }
        if response.len() > MAX_REQUEST_BYTES {
            return Err(io::Error::other("answer too long"));
        }
    }
    let response = String::from_utf8(response).map_err(io::Error::other)?;
    parse_response(&response).ok_or_else(|| io::Error::other("unreadable shared reading"))
}

/// A whole `200` answer whose body is exactly `Content-Length` bytes, so a
/// cut-off answer is never read as numbers.
fn parse_response(response: &str) -> Option<SharedReading> {
    let (head, body) = response.split_once("\r\n\r\n")?;
    let mut lines = head.lines();
    if !lines.next()?.starts_with("HTTP/1.0 200") {
        return None;
    }
    let length = lines.find_map(|line| {
        line.strip_prefix("Content-Length: ")?
            .trim()
            .parse::<usize>()
            .ok()
    })?;
    (body.len() == length).then(|| decode(body)).flatten()
}

/// The listener thread. Dropping it stops the thread and waits until the port
/// is free, so a new server can bind it straight away.
pub(crate) struct ShareServer {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl ShareServer {
    pub(crate) fn start() -> io::Result<Self> {
        let listener = TcpListener::bind(local_address())?;
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let thread = thread::Builder::new()
            .name("rami-share".to_string())
            .spawn(move || serve(listener, &thread_stop))?;
        Ok(Self {
            stop,
            thread: Some(thread),
        })
    }
}

impl Drop for ShareServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // `accept` blocks, so a throwaway connection wakes the thread to see
        // the flag and drop the listener.
        let _ = TcpStream::connect_timeout(&local_address(), IO_TIMEOUT);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn local_address() -> SocketAddr {
    SocketAddr::from((Ipv4Addr::LOCALHOST, SHARE_PORT))
}

fn serve(listener: TcpListener, stop: &AtomicBool) {
    let mut memory = match MemorySampler::new() {
        Ok(memory) => memory,
        Err(err) => {
            eprintln!("share: memory sampler failed: {err}");
            return;
        }
    };
    let mut cpu = CpuSampler::new();
    let mut gpu = GpuSampler::new();
    let name = host_name();
    let model = product_model().unwrap_or_else(|| name.clone());
    for stream in listener.incoming() {
        if stop.load(Ordering::SeqCst) {
            break;
        }
        let Ok(mut stream) = stream else { continue };
        let response = match memory.sample() {
            Ok(snapshot) => {
                let reading = SharedReading {
                    name: name.clone(),
                    model: model.clone(),
                    memory: snapshot,
                    cpu: cpu.sample().ok().flatten(),
                    gpu: gpu.sample().ok().flatten(),
                };
                http_response("200 OK", &encode(&reading))
            }
            Err(err) => http_response("503 Service Unavailable", &format!("{err}\n")),
        };
        if let Err(err) = answer(&mut stream, &response) {
            eprintln!("share: request failed: {err}");
        }
    }
}

/// Read the request head, whatever it asks, then send `response`. The whole
/// exchange is bounded by `REQUEST_DEADLINE`, so a client that drips bytes
/// can hold the listener, and a stop waiting on it, only that long.
fn answer(stream: &mut TcpStream, response: &str) -> io::Result<()> {
    let deadline = Instant::now() + REQUEST_DEADLINE;
    stream.set_write_timeout(Some(REQUEST_DEADLINE))?;
    let mut request = Vec::new();
    let mut buffer = [0u8; 512];
    while request.len() < MAX_REQUEST_BYTES && !request.windows(4).any(|w| w == b"\r\n\r\n") {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        stream.set_read_timeout(Some(remaining))?;
        match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => request.extend_from_slice(&buffer[..read]),
            Err(err)
                if matches!(
                    err.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) =>
            {
                break
            }
            Err(err) => return Err(err),
        }
    }
    stream.write_all(response.as_bytes())
}

fn http_response(status: &str, body: &str) -> String {
    format!(
        "HTTP/1.0 {status}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

/// The product family from the device tree: "Mac mini (2024)" becomes
/// "Mac mini".
fn product_model() -> Option<String> {
    let product =
        IoObject::new(unsafe { IORegistryEntryFromPath(0, c"IODeviceTree:/product".as_ptr()) })?;
    let key = CfObject::new(unsafe {
        CFStringCreateWithCString(
            std::ptr::null(),
            c"product-name".as_ptr(),
            CF_STRING_ENCODING_UTF8,
        )
    })?;
    let data = CfObject::new(unsafe {
        IORegistryEntryCreateCFProperty(product.id(), key.get(), std::ptr::null(), 0)
    })?;
    if unsafe { CFGetTypeID(data.get()) } != unsafe { CFDataGetTypeID() } {
        return None;
    }
    let bytes = unsafe {
        std::slice::from_raw_parts(
            CFDataGetBytePtr(data.get()),
            CFDataGetLength(data.get()).max(0) as usize,
        )
    };
    model_family(&String::from_utf8_lossy(bytes))
}

fn model_family(product_name: &str) -> Option<String> {
    let name = product_name.trim_end_matches('\0');
    let family = name.split(" (").next().unwrap_or(name).trim();
    (!family.is_empty()).then(|| family.to_string())
}

/// The short host name, e.g. "nicomini".
fn host_name() -> String {
    let mut buffer = [0u8; 256];
    let result = unsafe { libc::gethostname(buffer.as_mut_ptr().cast(), buffer.len()) };
    if result != 0 {
        return "Mac".to_string();
    }
    let end = buffer.iter().position(|&b| b == 0).unwrap_or(buffer.len());
    let full = String::from_utf8_lossy(&buffer[..end]);
    full.split('.').next().unwrap_or("Mac").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reading(cpu: Option<CpuSnapshot>, gpu: Option<GpuSnapshot>) -> SharedReading {
        SharedReading {
            name: "nicomini".to_string(),
            model: "Mac mini".to_string(),
            memory: MemorySnapshot {
                used_bytes: 12_550_406_144,
                total_bytes: 17_179_869_184,
                used_percent: 73,
                pressure_percent: 37,
                pressure_source: PressureSource::Kernel,
                app_memory_bytes: 6_488_817_664,
                wired_bytes: 2_288_009_216,
                compressed_bytes: 3_773_579_264,
                free_bytes: 257_277_952,
                cached_bytes: 3_824_025_600,
                swap_used_bytes: 5_418_188_800,
                available_bytes: 4_000_000_000,
            },
            cpu,
            gpu,
        }
    }

    #[test]
    fn encodes_every_reading_as_one_line_after_the_version() {
        let body = encode(&reading(
            Some(CpuSnapshot {
                user_percent: 10,
                system_percent: 8,
            }),
            Some(GpuSnapshot {
                utilization_percent: 3,
                renderer_percent: Some(2),
                tiler_percent: None,
            }),
        ));
        assert_eq!(
            body,
            "rami-share 1\n\
             name=nicomini\n\
             model=Mac mini\n\
             used_bytes=12550406144\n\
             total_bytes=17179869184\n\
             used_percent=73\n\
             pressure_percent=37\n\
             pressure_source=kernel\n\
             app_memory_bytes=6488817664\n\
             wired_bytes=2288009216\n\
             compressed_bytes=3773579264\n\
             free_bytes=257277952\n\
             cached_bytes=3824025600\n\
             swap_used_bytes=5418188800\n\
             available_bytes=4000000000\n\
             cpu_user_percent=10\n\
             cpu_system_percent=8\n\
             gpu_percent=3\n\
             gpu_renderer_percent=2\n"
        );
    }

    #[test]
    fn omits_cpu_and_gpu_until_they_have_a_reading() {
        let body = encode(&reading(None, None));
        assert!(body.ends_with("available_bytes=4000000000\n"));
        assert!(!body.contains("cpu_") && !body.contains("gpu_"));
    }

    #[test]
    fn decode_reads_back_what_encode_wrote() {
        let full = reading(
            Some(CpuSnapshot {
                user_percent: 10,
                system_percent: 8,
            }),
            Some(GpuSnapshot {
                utilization_percent: 3,
                renderer_percent: Some(2),
                tiler_percent: None,
            }),
        );
        assert_eq!(decode(&encode(&full)), Some(full));
        let bare = reading(None, None);
        assert_eq!(decode(&encode(&bare)), Some(bare));
    }

    #[test]
    fn decode_refuses_another_version_or_missing_memory() {
        let body = encode(&reading(None, None));
        assert_eq!(decode(&body.replace("rami-share 1", "rami-share 2")), None);
        assert_eq!(decode(&body.replace("used_bytes=", "x=")), None);
    }

    #[test]
    fn only_a_whole_ok_answer_parses() {
        let body = encode(&reading(None, None));
        let whole = http_response("200 OK", &body);
        assert!(parse_response(&whole).is_some());
        assert_eq!(parse_response(&whole[..whole.len() - 3]), None, "cut off");
        assert_eq!(
            parse_response(&format!("{whole}extra=1\n")),
            None,
            "too long"
        );
        assert_eq!(
            parse_response(&http_response("503 Service Unavailable", &body)),
            None
        );
    }

    #[test]
    fn model_family_drops_the_year() {
        assert_eq!(
            model_family("Mac mini (2024)\0").as_deref(),
            Some("Mac mini")
        );
        assert_eq!(model_family("MacBook Pro").as_deref(), Some("MacBook Pro"));
        assert_eq!(model_family("\0"), None);
    }

    #[test]
    #[ignore = "requires a live device tree"]
    fn smoke_reads_this_macs_model() {
        let model = product_model().expect("IODeviceTree:/product has a product-name");
        eprintln!("model: {model}");
        assert!(model.starts_with("Mac"));
    }

    #[test]
    fn response_carries_the_body_length() {
        let response = http_response("200 OK", "abc\n");
        assert!(response.starts_with("HTTP/1.0 200 OK\r\n"));
        assert!(response.contains("Content-Length: 4\r\n"));
        assert!(response.ends_with("\r\n\r\nabc\n"));
    }
}
