// Session management module for right-now daemon
// This module is shared between the daemon and CLI binaries

pub mod attention;
pub mod config;
pub mod markdown;
pub mod notify;
pub mod persistence;
pub mod protocol;
pub mod runtime;
pub mod shell_integration;

// Daemon client (Unix only for now)
#[cfg(unix)]
pub mod daemon_client;
