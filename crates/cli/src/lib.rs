//! utilties for parsing user inputs to run cryo

#![warn(missing_docs, unreachable_pub, unused_crate_dependencies)]
#![deny(unused_must_use, rust_2018_idioms)]
#![doc(test(
    no_crate_inject,
    attr(deny(warnings, rust_2018_idioms), allow(dead_code, unused_variables))
))]

mod args;
mod parse;
mod reports;
mod run;
mod summaries;

// used in main.rs but not lib.rs
use eyre as _;
use tokio as _;

pub use args::Args;
pub use parse::parse_opts;
pub use run::run;
