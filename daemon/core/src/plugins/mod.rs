// Plugin system for neurox. EP-0005.
// Lets users install optional clients (GUI, CLI, TUI, memory) from
// independent repositories. Each plugin is a tarball with a manifest,
// a binary, optional libraries, and assets.

pub mod dynamic;
pub mod manager;
pub mod manifest;
pub mod registry;
pub mod verify;

pub use dynamic::{
    PluginProxyTool, PluginRegisterRequest, PluginSkillDef, PluginState, PluginStatus,
    PluginToolDef, PluginToolRegistry, SharedPluginToolRegistry,
};
pub use manager::{PluginInfo, PluginInstallStatus, PluginManager};
pub use manifest::{PluginCapability, PluginManifest, PluginVersion};
pub use registry::{Registry, RegistryEntry, RegistryVersion};

#[cfg(test)]
mod plugins_test;
