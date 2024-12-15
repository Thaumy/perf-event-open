#![allow(warnings)]

macro_rules! select {
    ($([$($since:literal)? .. $($before:literal)?],)+) => {
        $(select!($($since)? .. $($before)?);)+
    };
    // internal switches
    ($since:literal ..) => {
        #[cfg(feature = $since)]
        include!(concat!("bindings/.", $since, ".rs"));
    };
    ($since:literal .. $before:literal) => {
        #[cfg(not(feature = $before))]
        #[cfg(feature = $since)]
        include!(concat!("bindings/.", $since, ".rs"));
    };
    (.. $before:literal) => {
        #[cfg(not(feature = $before))]
        include!(concat!("bindings/.", $before, ".rs"));
    };
}

select! {
    ["linux-6.13"..            ],
    ["linux-6.11".."linux-6.13"],
    ["linux-6.8" .."linux-6.11"],
    ["linux-6.6" .."linux-6.8" ],
    ["linux-6.3" .."linux-6.6" ],
    ["linux-6.1" .."linux-6.3" ],
    ["linux-6.0" .."linux-6.1" ],
    ["linux-5.18".."linux-6.0" ],
    ["linux-5.17".."linux-5.18"],
    ["linux-5.16".."linux-5.17"],
    ["linux-5.13".."linux-5.16"],
    ["linux-5.12".."linux-5.13"],
    ["linux-5.11".."linux-5.12"],
    ["linux-5.9" .."linux-5.11"],
    ["linux-5.7" .."linux-5.9" ],
    ["linux-5.5" .."linux-5.7" ],
    ["linux-5.4" .."linux-5.5" ],
    ["linux-5.1" .."linux-5.4" ],
    ["linux-4.17".."linux-5.1" ],
    ["linux-4.16".."linux-4.17"],
    ["linux-4.15".."linux-4.16"],
    ["linux-4.14".."linux-4.15"],
    ["linux-4.12".."linux-4.14"],
    ["linux-4.10".."linux-4.12"],
    ["linux-4.8" .."linux-4.10"],
    ["linux-4.7" .."linux-4.8" ],
    ["linux-4.5" .."linux-4.7" ],
    ["linux-4.4" .."linux-4.5" ],
    ["linux-4.3" .."linux-4.4" ],
    ["linux-4.2" .."linux-4.3" ],
    ["linux-4.1" .."linux-4.2" ],
    [            .."linux-4.1" ],
}
