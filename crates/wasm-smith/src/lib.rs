//! A WebAssembly test case generator.
//!
//! ## Usage
//!
//! First, use [`cargo fuzz`](https://github.com/rust-fuzz/cargo-fuzz) to define
//! a new fuzz target:
//!
//! ```shell
//! $ cargo fuzz add my_wasm_smith_fuzz_target
//! ```
//!
//! Next, add `wasm-smith` to your dependencies:
//!
//! ```toml
//! # fuzz/Cargo.toml
//!
//! [dependencies]
//! wasm-smith = "0.4.0"
//! ```
//!
//! Then, define your fuzz target so that it takes arbitrary
//! `wasm_smith::Module`s as an argument, convert the module into serialized
//! Wasm bytes via the `to_bytes` method, and then feed it into your system:
//!
//! ```no_run
//! // fuzz/fuzz_targets/my_wasm_smith_fuzz_target.rs
//!
//! #![no_main]
//!
//! use libfuzzer_sys::fuzz_target;
//! use wasm_smith::Module;
//!
//! fuzz_target!(|module: Module| {
//!     let wasm_bytes = module.to_bytes();
//!
//!     // Your code here...
//! });
//! ```
//!
//! Finally, start fuzzing:
//!
//! ```shell
//! $ cargo fuzz run my_wasm_smith_fuzz_target
//! ```
//!
//! > **Note:** For a real world example, also check out [the `validate` fuzz
//! > target](https://github.com/fitzgen/wasm-smith/blob/main/fuzz/fuzz_targets/validate.rs)
//! > defined in this repository. Using the `wasmparser` crate, it checks that
//! > every module generated by `wasm-smith` validates successfully.
//!
//! ## Design
//!
//! The design and implementation strategy of wasm-smith is outlined in
//! [this article](https://fitzgeraldnick.com/2020/08/24/writing-a-test-case-generator.html).

#![deny(missing_docs, missing_debug_implementations)]
// Needed for the `instructions!` macro in `src/code_builder.rs`.
#![recursion_limit = "512"]

mod component;
mod config;
mod core;

pub use crate::core::{
    ConfiguredModule, InstructionKind, InstructionKinds, MaybeInvalidModule, Module,
};
use arbitrary::{Result, Unstructured};
pub use component::{Component, ConfiguredComponent};
pub use config::{Config, DefaultConfig, SwarmConfig};

use std::{collections::HashSet, str};

/// Do something an arbitrary number of times.
///
/// The callback can return `false` to exit the loop early.
pub(crate) fn arbitrary_loop<'a>(
    u: &mut Unstructured<'a>,
    min: usize,
    max: usize,
    mut f: impl FnMut(&mut Unstructured<'a>) -> Result<bool>,
) -> Result<()> {
    assert!(max >= min);
    for _ in 0..min {
        if !f(u)? {
            break;
        }
    }
    for _ in 0..(max - min) {
        let keep_going = u.arbitrary().unwrap_or(false);
        if !keep_going {
            break;
        }

        if !f(u)? {
            break;
        }
    }

    Ok(())
}

// Mirror what happens in `Arbitrary for String`, but do so with a clamped size.
pub(crate) fn limited_str<'a>(max_size: usize, u: &mut Unstructured<'a>) -> Result<&'a str> {
    let size = u.arbitrary_len::<u8>()?;
    let size = std::cmp::min(size, max_size);
    match str::from_utf8(&u.peek_bytes(size).unwrap()) {
        Ok(s) => {
            u.bytes(size).unwrap();
            Ok(s.into())
        }
        Err(e) => {
            let i = e.valid_up_to();
            let valid = u.bytes(i).unwrap();
            let s = unsafe {
                debug_assert!(str::from_utf8(valid).is_ok());
                str::from_utf8_unchecked(valid)
            };
            Ok(s)
        }
    }
}

pub(crate) fn limited_string(max_size: usize, u: &mut Unstructured) -> Result<String> {
    Ok(limited_str(max_size, u)?.into())
}

pub(crate) fn unique_string(
    max_size: usize,
    names: &mut HashSet<String>,
    u: &mut Unstructured,
) -> Result<String> {
    let mut name = limited_string(max_size, u)?;
    while names.contains(&name) {
        name.push_str(&format!("{}", names.len()));
    }
    names.insert(name.clone());
    Ok(name)
}

pub(crate) fn unique_non_empty_string(
    max_size: usize,
    names: &mut HashSet<String>,
    u: &mut Unstructured,
) -> Result<String> {
    let mut s = unique_string(max_size, names, u)?;
    while s.is_empty() || names.contains(&s) {
        use std::fmt::Write;
        write!(&mut s, "{}", names.len()).unwrap();
    }
    Ok(s)
}
