//! Share over Tailscale: while the setting is on, rami answers HTTP on
//! `127.0.0.1:SHARE_PORT` with this Mac's readings, so another Mac's rami can
//! show them. rami never listens beyond localhost; `tailscale serve --bg --tcp
//! 47960 tcp://localhost:47960` forwards the port to the tailnet.
//!
//! The body is plain text, one `key=value` per line, after a `rami-share 1`
//! version line. CPU keys are absent until a second request gives the sampler
//! a delta, and GPU keys are absent when the driver reports nothing.

use crate::cpu::CpuSampler;
use crate::gpu::GpuSampler;
use crate::memory::MemorySampler;
use crate::model::{CpuSnapshot, GpuSnapshot, MemorySnapshot, PressureSource};
use std::fmt::Write as _;
use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

pub(crate) const SHARE_PORT: u16 = 47960;
const SHARE_VERSION: &str = "rami-share 1";
const IO_TIMEOUT: Duration = Duration::from_secs(1);
const MAX_REQUEST_BYTES: usize = 4096;

/// One answer: everything this Mac's dropdown shows except app names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SharedReading {
    pub(crate) name: String,
    pub(crate) memory: MemorySnapshot,
    pub(crate) cpu: Option<CpuSnapshot>,
    pub(crate) gpu: Option<GpuSnapshot>,
}

pub(crate) fn encode(reading: &SharedReading) -> String {
    let memory = &reading.memory;
    let mut body = format!("{SHARE_VERSION}\nname={}\n", reading.name);
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
    for stream in listener.incoming() {
        if stop.load(Ordering::SeqCst) {
            break;
        }
        let Ok(mut stream) = stream else { continue };
        let response = match memory.sample() {
            Ok(snapshot) => {
                let reading = SharedReading {
                    name: name.clone(),
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

/// Read the request head, whatever it asks, then send `response`.
fn answer(stream: &mut TcpStream, response: &str) -> io::Result<()> {
    stream.set_read_timeout(Some(IO_TIMEOUT))?;
    stream.set_write_timeout(Some(IO_TIMEOUT))?;
    let mut request = Vec::new();
    let mut buffer = [0u8; 512];
    while request.len() < MAX_REQUEST_BYTES && !request.windows(4).any(|w| w == b"\r\n\r\n") {
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
    fn response_carries_the_body_length() {
        let response = http_response("200 OK", "abc\n");
        assert!(response.starts_with("HTTP/1.0 200 OK\r\n"));
        assert!(response.contains("Content-Length: 4\r\n"));
        assert!(response.ends_with("\r\n\r\nabc\n"));
    }
}
