#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod bind;

pub use bind::{Bind, State, StateWithData};

#[cfg(feature = "egui")]
pub mod egui;

#[cfg(feature = "egui")]
pub use egui::ContextExt;

/// A macro to run initialization code only once, even in the presence of multiple threads.
/// Returns `true` if the code was executed in this call, `false` otherwise.
#[macro_export]
macro_rules! run_once {
    { $($tokens:tt)* } => {{
        static INIT_ONCE_BLOCK_: std::sync::Once = std::sync::Once::new();
        let mut init_once_block_executed = false;
        INIT_ONCE_BLOCK_.call_once(|| {
            $($tokens)*
            init_once_block_executed = true;
        });
        init_once_block_executed
    }};
}
