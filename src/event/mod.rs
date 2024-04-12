pub mod bp;
pub mod hw;
pub mod raw;
pub mod sw;
pub mod tp;

#[derive(Clone, Debug)]
pub struct Event(pub(super) EventConfig);

#[derive(Clone, Debug)]
pub(super) struct EventConfig {
    pub ty: u32,
    pub config: u64,
    pub config1: u64,
    pub config2: u64,
    pub config3: u64,
    pub bp_type: u32,
}

macro_rules! try_from {
    ($ty:ty, $value:ident, $impl: expr) => {
        impl TryFrom<&$ty> for crate::event::Event {
            type Error = std::io::Error;

            fn try_from($value: &$ty) -> std::result::Result<Self, Self::Error> {
                $impl
            }
        }

        impl TryFrom<$ty> for crate::event::Event {
            type Error = std::io::Error;

            fn try_from(value: $ty) -> std::result::Result<Self, Self::Error> {
                (&value).try_into()
            }
        }
    };
}
use try_from;
