# perf-event-open

Full-featured support for the `perf_event_open` syscall.

[![Crates.io][crates-badge]][crates-url]
[![MIT licensed][license-badge]][license-url]

[crates-badge]: https://img.shields.io/crates/v/perf_event_open.svg
[crates-url]: https://crates.io/crates/perf_event_open
[license-badge]: https://img.shields.io/badge/license-MIT-blue.svg
[license-url]: https://github.com/Thaumy/perf-event-open/blob/main/LICENSE

[`perf_event_open`](https://man7.org/linux/man-pages/man2/perf_event_open.2.html)
is a Linux system call widely used in performance monitoring, which provides
access to the hardware Performance Monitoring Unit (PMU), allowing us to count
and sample performance events. It is the core of the `perf` tool and many other
performance engineering utilities.

## Example

Count how many instructions executed for the (inefficient) fibonacci calculation
and samples the user stack for it.

```rust
use perf_event_open::config::{Cpu, Opts, Proc, SampleOn, Size};
use perf_event_open::count::Counter;
use perf_event_open::event::hw::Hardware;

// Count retired instructions on current process, all CPUs.
let event = Hardware::Instr;
let target = (Proc::CURRENT, Cpu::ALL);

let mut opts = Opts::default();
opts.sample_on = SampleOn::Freq(1000); // 1000 samples per second.
opts.sample_format.user_stack = Some(Size(8)); // Dump an 8-byte user stack in each sample.

let counter = Counter::new(event, target, opts).unwrap();
let sampler = counter.sampler(10).unwrap(); // Use 2^10 pages for the sample ring buffer.

counter.enable().unwrap(); // Start the counter.
fn fib(n: usize) -> usize {
    match n {
        0 => 0,
        1 => 1,
        n => fib(n - 1) + fib(n - 2),
    }
}
std::hint::black_box(fib(30));
counter.disable().unwrap(); // Stop the counter.

let instrs = counter.stat().unwrap().count;
println!("{} instructions retired", instrs);

for it in sampler.iter() {
    println!("{:-?}", it);
}

// Example output:
// 73973233 instructions retired
// (Kernel, Sample { record_id: RecordId { .. }, user_stack: [16, 0, 0, 0, 0, 0, 0, 0], .. })
// (Kernel, Sample { record_id: RecordId { .. }, user_stack: [16, 0, 0, 0, 0, 0, 0, 0], .. })
// (Kernel, Sample { record_id: RecordId { .. }, user_stack: [16, 0, 0, 0, 0, 0, 0, 0], .. })
// (Kernel, Sample { record_id: RecordId { .. }, user_stack: [16, 0, 0, 0, 0, 0, 0, 0], .. })
// (Kernel, Sample { record_id: RecordId { .. }, user_stack: [16, 0, 0, 0, 0, 0, 0, 0], .. })
// (User, Sample { record_id: RecordId { .. }, user_stack: [2, 0, 0, 0, 0, 0, 0, 0], .. })
// (User, Sample { record_id: RecordId { .. }, user_stack: [1, 0, 0, 0, 0, 0, 0, 0], .. })
// (User, Sample { record_id: RecordId { .. }, user_stack: [1, 0, 0, 0, 0, 0, 0, 0], .. })
```

For more use cases, please refer to
[documentation](https://docs.rs/perf-event-open/latest/perf_event_open/).

## Compatibility

Any Linux kernel since 4.0 is supported.

Please use the Linux version features to ensure your binary is compatible
with the target host kernel. These features are backwards compatible, e.g.
`linux-6.11` works with Linux 6.12 but may not work with Linux 6.10.

The `latest` feature is an alias for the latest `linux-` feature;
only choose it if you don't care about kernel compatibility.

Calling Linux-specific functions (e.g., `Counter::new`) on non-Linux targets
will return an error, but configuration and profiling result types are
cross-platform compatible.

## MSRV

We will keep the MSRV (minimum supported rust version) as little as possible
if no dependencies require a higher MSRV, currently
[1.80.0](https://releases.rs/docs/1.80.0).

## License

This project is licensed under the MIT license.
