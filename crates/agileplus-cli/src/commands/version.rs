// SPDX-License-Identifier: MIT OR Apache-2.0
use clap::Args;

#[derive(Debug, Args, Default)]
pub struct VersionArgs;

pub fn run() {
    println!("agileplus-cli {}", env!("CARGO_PKG_VERSION"));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_args_is_default_constructible() {
        let _ = VersionArgs;
    }
}
