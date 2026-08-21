//! Integration suite over the real binary: every test runs `riso` in a
//! sandboxed home, so nothing on the host leaks into an assertion.

mod common;

mod apps;
mod backgrounds;
mod check;
mod config;
mod data;
mod dev;
mod install;
mod outputs;
mod plugin;
mod state;
mod theme;
mod wire;
