//! `agent-lib` — reusable logic shared by every neurox agent subprocess.
//!
//! EP-0004 wave 5b: this crate holds the bits that were duplicated
//! between `daemon/agents/agent/` (the production default
//! binary) and `daemon/agents/_template/` (the scaffold for new
//! agents). Both directories now become thin wrappers that declare
//! their identity and import the rest from here.

pub mod identity;
pub mod llm;
pub mod main_loop;
pub mod memory;
pub mod mode;
pub mod skills;
pub mod tools_filter;

pub use identity::Identity;
pub use llm::LlmClient;
pub use main_loop::{run_agent_loop, AgentContext, AgentRequest, AgentResponse};
pub use memory::{ChatMessage, WorkingMemory};
pub use mode::{detect_mode, Mode};
pub use skills::{load_skills, match_skills, Skill};
pub use tools_filter::filter_tools;