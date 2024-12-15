use std::cmp::Ordering;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use anyhow::{anyhow, Context, Result};

#[derive(PartialEq, Eq)]
pub struct Version {
    pub major: usize,
    pub minor: usize,
}

impl Version {
    pub fn from_headers<P>(dir: P) -> Result<Version>
    where
        P: AsRef<Path>,
    {
        let version_h = dir.as_ref().join("linux/version.h");
        let version_h = File::open(version_h).context("failed to open version.h")?;
        let version_h = BufReader::new(version_h);

        let version_code_line = version_h
            .lines()
            .next()
            .ok_or_else(|| anyhow!("version.h is empty"))
            .context("when getting version code line")??;
        let version_code = version_code_line
            .split(' ')
            .nth(2)
            .ok_or_else(|| anyhow!("unknown line format"))?
            .parse::<usize>()
            .context("when parsing version code")?;

        Ok(Self {
            major: version_code >> 16,
            minor: (version_code & 65535) >> 8,
        })
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.major.cmp(&other.major) {
            Ordering::Less => Ordering::Less,
            Ordering::Equal => self.minor.cmp(&other.minor),
            Ordering::Greater => Ordering::Greater,
        }
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

macro_rules! v {
    ($major:expr, $minor:expr) => {
        crate::version::Version {
            major: $major,
            minor: $minor,
        }
    };
}
pub(super) use v;
