pub mod multiplexer;
pub mod stdio;
pub mod types;

pub use multiplexer::{GatewayMultiplexer, UpstreamServer};
pub use stdio::{DeathReceiver, DeathSender, StdioConnection};
pub use types::{
    PromptDefinition, RegisteredPrompt, RegisteredResource, RegisteredTool, ResourceDefinition,
    ToolCallParams, ToolDefinition, UpstreamServerConfig, UpstreamServerStatus,
};
