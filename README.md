# perf-event-open

Full-featured high-level wrapper for the `perf_event_open` system call.

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

Count how many instructions executed for the (inefficient) fibonacci caculation
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
opts.sample_format.user_stack = Some(Size(8)); // Dump 8-bytes user stack in sample.

let counter = Counter::new(event, target, opts).unwrap();
let sampler = counter.sampler(10).unwrap(); // Allocate 2^10 pages to store samples.

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
```

On my machine, this gives the following output:

```text
73973233 instructions retired
(Kernel, Sample { record_id: RecordId { .. }, user_stack: [16, 0, 0, 0, 0, 0, 0, 0], .. })
(Kernel, Sample { record_id: RecordId { .. }, user_stack: [16, 0, 0, 0, 0, 0, 0, 0], .. })
(Kernel, Sample { record_id: RecordId { .. }, user_stack: [16, 0, 0, 0, 0, 0, 0, 0], .. })
(Kernel, Sample { record_id: RecordId { .. }, user_stack: [16, 0, 0, 0, 0, 0, 0, 0], .. })
(Kernel, Sample { record_id: RecordId { .. }, user_stack: [16, 0, 0, 0, 0, 0, 0, 0], .. })
(User, Sample { record_id: RecordId { .. }, user_stack: [2, 0, 0, 0, 0, 0, 0, 0], .. })
(User, Sample { record_id: RecordId { .. }, user_stack: [1, 0, 0, 0, 0, 0, 0, 0], .. })
(User, Sample { record_id: RecordId { .. }, user_stack: [1, 0, 0, 0, 0, 0, 0, 0], .. })
```

For more use cases, please check the
[docs](https://docs.rs/perf-event-open/latest/perf_event_open/).

## Kernel compatibility

Any Linux kernel since 4.0 is supported.

Please use the Linux version features to ensure your binary is compatible with
the target host kernel. These features are backwards compatible, e.g.
`linux-6.11` works with Linux 6.12 but may not work with Linux 6.10.

The `legacy` feature is compatible with the oldest LTS kernel that still in
maintaince, or you can use the `latest` feature if you dont't care about the
kernel compatibility.

## MSRV

We will keep the MSRV (minimum supported rust version) as little as possible if
no dependencies require a higher MSRV, currently
[1.80.0](https://releases.rs/docs/1.80.0).

## License

This project is licensed under the MIT license.
