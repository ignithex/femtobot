use crate::bus::{MessageBus, OutboundMessage};
use crate::tools::ToolError;
use rig::completion::request::ToolDefinition;
use rig::tool::Tool;
use serde::Deserialize;

#[derive(Clone)]
pub struct SendMessageTool {
    bus: MessageBus,
}

impl SendMessageTool {
    pub fn new(bus: MessageBus) -> Self {
        Self { bus }
    }
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct SendMessageArgs {
    /// Destination channel (e.g. "telegram")
    pub channel: String,
    /// Destination chat id (e.g. Telegram chat id)
    pub chat_id: String,
    /// Message text to send
    pub content: String,
}

impl Tool for SendMessageTool {
    const NAME: &'static str = "send_message";
    type Args = SendMessageArgs;
    type Output = String;
    type Error = ToolError;

    fn definition(
        &self,
        _prompt: String,
    ) -> impl std::future::Future<Output = ToolDefinition> + Send {
        async {
            ToolDefinition {
                name: Self::NAME.to_string(),
                description: "Send a message to a specific channel/chat. This is the delivery path for proactive notifications; in cron-triggered turns, call this tool whenever a user-visible notification should be sent.".to_string(),
                parameters: serde_json::to_value(schemars::schema_for!(SendMessageArgs)).unwrap(),
            }
        }
    }

    fn call(
        &self,
        args: Self::Args,
    ) -> impl std::future::Future<Output = Result<Self::Output, Self::Error>> + Send {
        let bus = self.bus.clone();
        async move {
            let channel = args.channel.trim().to_string();
            let chat_id = args.chat_id.trim().to_string();
            let content = args.content.trim().to_string();

            if channel.is_empty() {
                return Err(ToolError::msg("Missing required field: channel"));
            }
            if chat_id.is_empty() {
                return Err(ToolError::msg("Missing required field: chat_id"));
            }
            if content.is_empty() {
                return Err(ToolError::msg("Missing required field: content"));
            }

            bus.publish_outbound(OutboundMessage {
                channel,
                chat_id,
                content,
            })
            .await;

            Ok("Message sent.".to_string())
        }
    }
}
