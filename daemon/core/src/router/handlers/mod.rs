//! EP-0013 A-001: router handlers partitioned by domain.
//!
//! Each module contains handlers for a specific bounded context.
//!
//! Modules:
//! - `agents`: placeholder (full extraction deferred)
//! - `sandbox`: put/get sandbox
//! - `sessions`: list_sessions + get_session_messages
//! - `skills`: list/enable/disable skills
//! - `tools`: invoke_tool
//! - `tools_admin`: list/enable/disable tools

pub mod agents;
pub mod sandbox;
pub mod sessions;
pub mod skills;
pub mod tools;
pub mod tools_admin;
pub mod workspaces;
