use std::path::Path;

use anyhow::{Context, Result};
use bindgen::RustTarget;
use itertools::Itertools;

use crate::version::{v, Version};

#[rustfmt::skip]
pub const IOC_OPS: [(Version, &str); 4] = [
    (v!(4, 1), "PERF_IOC_OP_SET_BPF      = PERF_EVENT_IOC_SET_BPF          ,"),
    (v!(4, 7), "PERF_IOC_OP_PAUSE_OUTPUT = PERF_EVENT_IOC_PAUSE_OUTPUT     ,"),
    (v!(4,16), "PERF_IOC_OP_QUERY_BPF    = PERF_EVENT_IOC_QUERY_BPF        ,"),
    (v!(4,17), "PERF_IOC_OP_MODIFY_ATTRS = PERF_EVENT_IOC_MODIFY_ATTRIBUTES,"),
];

pub fn bindgen<P>(version: &Version, headers_dir: P, to: P) -> Result<()>
where
    P: AsRef<Path>,
{
    let bpf_h = match version >= &v!(5, 1) {
        true => "#include <linux/bpf.h>",
        false => "",
    };
    let enabled_ioctls = IOC_OPS
        .into_iter()
        .filter_map(|(since, entry)| (version >= &since).then_some(entry))
        .join("\n");
    let contents = format!(
        "
        #include <linux/time.h>
        #include <linux/perf_event.h>
        #include <linux/hw_breakpoint.h>
        {}

        enum perf_ioc_ops {{
            PERF_IOC_OP_ENABLE     = PERF_EVENT_IOC_ENABLE    ,
            PERF_IOC_OP_DISABLE    = PERF_EVENT_IOC_DISABLE   ,
            PERF_IOC_OP_REFRESH    = PERF_EVENT_IOC_REFRESH   ,
            PERF_IOC_OP_RESET      = PERF_EVENT_IOC_RESET     ,
            PERF_IOC_OP_PERIOD     = PERF_EVENT_IOC_PERIOD    ,
            PERF_IOC_OP_SET_OUTPUT = PERF_EVENT_IOC_SET_OUTPUT,
            PERF_IOC_OP_SET_FILTER = PERF_EVENT_IOC_SET_FILTER,
            PERF_IOC_OP_ID         = PERF_EVENT_IOC_ID        ,
            {}
        }};
        ",
        bpf_h, enabled_ioctls,
    );

    bindgen::Builder::default()
        .rust_target(env!("CARGO_PKG_RUST_VERSION").parse::<RustTarget>()?)
        .derive_default(true)
        .generate_comments(false)
        .prepend_enum_name(false)
        .translate_enum_integer_types(true)
        .header_contents("wrapper.h", &contents)
        .clang_arg(format!(
            "-I{}",
            headers_dir
                .as_ref()
                .to_str()
                .context("invalid headers dir")?
        ))
        .allowlist_file(r#".*wrapper\.h.*"#)
        .allowlist_file(r#".*perf_event\.h.*"#)
        .allowlist_item("CLOCK_.*")
        .allowlist_item("HW_BREAKPOINT_.*")
        .allowlist_item("BPF_TAG_SIZE")
        .generate()
        .context("failed to generate bindings")?
        .write_to_file(to)
        .context("failed to write generated bindings to file")?;

    Ok(())
}
