//! Full-featured high-level wrapper for the `perf_event_open` system call.
//!
//! ## Example
//!
//! Count how many instructions executed for the (inefficient) fibonacci caculation
//! and samples the user stack for it.
//!
//! ```rust
//! use perf_event_open::config::{Cpu, Opts, Proc, SampleOn, Size};
//! use perf_event_open::count::Counter;
//! use perf_event_open::event::hw::Hardware;
//!
//! // Count retired instructions on current process, all CPUs.
//! let event = Hardware::Instr;
//! let target = (Proc::CURRENT, Cpu::ALL);
//!
//! let mut opts = Opts::default();
//! opts.sample_on = SampleOn::Freq(1000); // 1000 samples per second.
//! opts.sample_format.user_stack = Some(Size(8)); // Dump 8-bytes user stack in sample.
//!
//! let counter = Counter::new(event, target, opts).unwrap();
//! let sampler = counter.sampler(10).unwrap(); // Allocate 2^10 pages to store samples.
//!
//! counter.enable().unwrap(); // Start the counter.
//! fn fib(n: usize) -> usize {
//!     match n {
//!         0 => 0,
//!         1 => 1,
//!         n => fib(n - 1) + fib(n - 2),
//!     }
//! }
//! std::hint::black_box(fib(30));
//! counter.disable().unwrap(); // Stop the counter.
//!
//! let instrs = counter.stat().unwrap().count;
//! println!("{} instructions retired", instrs);
//!
//! for it in sampler.iter() {
//!     println!("{:-?}", it);
//! }
//! ```
//!
//! ## Kernel compatibility
//!
//! Any Linux kernel since 4.0 is supported.
//!
//! Please use the Linux version features to ensure your binary is compatible with
//! the target host kernel. These features are backwards compatible, e.g.
//! `linux-6.11` works with Linux 6.12 but may not work with Linux 6.10.
//!
//! The `legacy` feature is compatible with the oldest LTS kernel that still in
//! maintaince, or you can use the `latest` feature if you dont't care about the
//! kernel compatibility.

pub mod config;
pub mod count;
pub mod event;
mod ffi;
pub mod sample;
