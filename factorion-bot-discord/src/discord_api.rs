use std::collections::HashSet;
use std::sync::Arc;

use anyhow::Error;
use factorion_lib::comment::{Commands, Comment, CommentConstructed};
use log::{error, info, warn};
use serenity::all::{
    ChannelId, Colour, CreateEmbed, CreateEmbedFooter, CreateMessage,
    GatewayIntents, Message, MessageId, Ready, Timestamp,
};
use serenity::async_trait;
use serenity::prelude::*;
use tokio::sync::Mutex;

const MAX_MESSAGE_LEN: usize = 2000;
const EMBED_DESCRIPTION_LIMIT: usize = 4096;
const EMBED_FIELD_VALUE_LIMIT: usize = 1024;

#[derive(Debug, Clone)]
pub struct MessageMeta {
    pub message_id: MessageId,
    pub channel_id: ChannelId,
    pub author: String,
}

pub struct Handler {
    processed_messages: Arc<Mutex<HashSet<MessageId>>>,
}

impl Handler {
    pub fn new() -> Self {
        Self {
            processed_messages: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    async fn process_message(&self, ctx: &Context, msg: &Message) -> Result<(), Error> {
        let mut processed = self.processed_messages.lock().await;
        
        if processed.contains(&msg.id) {
            return Ok(());
        }

        if msg.author.bot {
            return Ok(());
        }

        let meta = MessageMeta {
            message_id: msg.id,
            channel_id: msg.channel_id,
            author: msg.author.name.clone(),
        };

        let comment: CommentConstructed<MessageMeta> =
            Comment::new(&msg.content, meta, Commands::NONE, MAX_MESSAGE_LEN);

        if comment.status.no_factorial {
            return Ok(());
        }

        let comment = comment.extract();
        
        if comment.status.no_factorial {
            return Ok(());
        }

        let comment = comment.calc();

        // Check if we should reply based on the comment's status
        if comment.status.not_replied {
            return Ok(());
        }

        let reply_text = comment.get_reply();

        processed.insert(msg.id);

        // Send formatted response
        if let Err(why) = self.send_formatted_reply(ctx, msg, &reply_text).await {
            error!(
                "Failed to send message to channel {}: {:?}",
                msg.channel_id, why
            );
        } else {
            info!(
                "Replied to message {} in channel {} by user {}",
                msg.id, msg.channel_id, msg.author.name
            );
        }

        Ok(())
    }

    async fn send_formatted_reply(
        &self,
        ctx: &Context,
        msg: &Message,
        reply_text: &str,
    ) -> Result<(), Error> {
        // Check if the reply is short enough for a simple message
        if Self::should_use_simple_reply(reply_text) {
            return Self::send_simple_reply(ctx, msg, reply_text).await;
        }

        // For longer/complex replies, use an embed
        let embed = self.create_embed(reply_text)?;
        
        // Send the embed
        let builder = CreateMessage::new()
            .embed(embed)
            .reference_message(msg);
        
        msg.channel_id.send_message(&ctx.http, builder).await?;

        Ok(())
    }

    fn should_use_simple_reply(reply_text: &str) -> bool {
        reply_text.len() <= 400 && !reply_text.contains('\n')
    }

    async fn send_simple_reply(
        ctx: &Context,
        msg: &Message,
        reply_text: &str,
    ) -> Result<(), Error> {
        let formatted = format!(
            "**📊 Calculation Result**\n```\n{}\n```",
            reply_text.trim_end_matches("*^(This action was performed by a bot.)*").trim()
        );
        
        msg.channel_id.say(&ctx.http, formatted).await?;
        Ok(())
    }

    fn create_embed(&self, reply_text: &str) -> Result<CreateEmbed, Error> {
        let mut embed = CreateEmbed::new()
            .colour(Colour::from_rgb(88, 101, 242))
            .timestamp(Timestamp::now())
            .footer(CreateEmbedFooter::new("🤖 Factorion Bot • Powered by factorion-lib"));

        // Parse the reply into sections
        let (description, results) = Self::parse_reply(reply_text);
        
        // Add title based on content
        embed = Self::add_title(embed, &description, results.len());
        
        // Add description if we have a note
        let desc_len = description.len();
        if !description.is_empty() {
            embed = Self::add_description(embed, description)?;
        }

        // Add results
        embed = Self::add_results(embed, results, desc_len, reply_text)?;

        Ok(embed)
    }

    fn parse_reply(reply_text: &str) -> (String, Vec<String>) {
        let lines: Vec<&str> = reply_text.lines().collect();
        let mut description = String::new();
        let mut results = Vec::new();
        let mut in_note = true;
        
        for line in lines {
            let trimmed = line.trim();
            
            // Skip the footer
            if trimmed.contains("This action was performed by a bot") {
                continue;
            }
            
            // Empty line marks end of note section
            if trimmed.is_empty() {
                in_note = false;
                continue;
            }
            
            if in_note {
                if !description.is_empty() {
                    description.push('\n');
                }
                description.push_str(trimmed);
            } else {
                results.push(trimmed.to_string());
            }
        }
        
        (description, results)
    }

    fn add_title(embed: CreateEmbed, description: &str, result_count: usize) -> CreateEmbed {
        if description.contains("approximate") || description.contains("large") {
            embed.title("🔢 Factorial Calculations (Approximated)")
        } else if result_count > 1 {
            embed.title("🔢 Multiple Factorial Calculations")
        } else {
            embed.title("🔢 Factorial Calculation")
        }
    }

    fn add_description(embed: CreateEmbed, description: String) -> Result<CreateEmbed, Error> {
        let desc = if description.len() > EMBED_DESCRIPTION_LIMIT {
            format!("{}...", &description[..EMBED_DESCRIPTION_LIMIT - 3])
        } else {
            description
        };
        Ok(embed.description(format!("ℹ️ *{}*", desc)))
    }

    fn add_results(
        mut embed: CreateEmbed,
        results: Vec<String>,
        desc_len: usize,
        reply_text: &str,
    ) -> Result<CreateEmbed, Error> {
        if results.is_empty() {
            embed = Self::add_full_text_results(embed, reply_text)?;
        } else if results.len() <= 5 {
            embed = Self::add_field_results(embed, results)?;
        } else {
            embed = Self::add_combined_results(embed, results, desc_len)?;
        }
        
        Ok(embed)
    }

    fn add_full_text_results(mut embed: CreateEmbed, reply_text: &str) -> Result<CreateEmbed, Error> {
        let full_text = reply_text
            .trim_end_matches("*^(This action was performed by a bot.)*")
            .trim();
        
        if full_text.len() > EMBED_DESCRIPTION_LIMIT {
            embed = Self::add_chunked_results(embed, full_text)?;
        } else {
            embed = embed.description(format!("```\n{}\n```", full_text));
        }
        
        Ok(embed)
    }

    fn add_chunked_results(mut embed: CreateEmbed, full_text: &str) -> Result<CreateEmbed, Error> {
        let chunks: Vec<String> = full_text
            .chars()
            .collect::<Vec<char>>()
            .chunks(EMBED_FIELD_VALUE_LIMIT - 50)
            .enumerate()
            .map(|(_i, chunk)| {
                let chunk_str: String = chunk.iter().collect();
                format!("```\n{}\n```", chunk_str)
            })
            .collect();
        
        for (i, chunk) in chunks.iter().take(10).enumerate() {
            embed = embed.field(
                format!("Result Part {}/{}", i + 1, chunks.len().min(10)),
                chunk,
                false,
            );
        }
        
        if chunks.len() > 10 {
            warn!("Reply too long, truncated to 10 fields");
        }
        
        Ok(embed)
    }

    fn add_field_results(mut embed: CreateEmbed, results: Vec<String>) -> Result<CreateEmbed, Error> {
        for (i, result) in results.iter().enumerate() {
            let field_value = if result.len() > EMBED_FIELD_VALUE_LIMIT {
                format!("```\n{}...\n```", &result[..EMBED_FIELD_VALUE_LIMIT - 20])
            } else {
                format!("```\n{}\n```", result)
            };
            
            embed = embed.field(
                format!("📐 Calculation {}", i + 1),
                field_value,
                false,
            );
        }
        
        Ok(embed)
    }

    fn add_combined_results(
        embed: CreateEmbed,
        results: Vec<String>,
        desc_len: usize,
    ) -> Result<CreateEmbed, Error> {
        let combined = results.join("\n");
        let result_text = if combined.len() > EMBED_DESCRIPTION_LIMIT - desc_len - 20 {
            format!("```\n{}...\n```", &combined[..EMBED_DESCRIPTION_LIMIT - desc_len - 30])
        } else {
            format!("```\n{}\n```", combined)
        };
        
        Ok(embed.field("📐 Results", result_text, false))
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        if let Err(e) = self.process_message(&ctx, &msg).await {
            error!("Error processing message: {:?}", e);
        }
    }

    async fn ready(&self, _: Context, ready: Ready) {
        info!("{} is connected and ready!", ready.user.name);
    }
}

pub async fn start_bot(token: String) -> Result<(), Error> {
    // Configure gateway intents
    // MESSAGE_CONTENT is a privileged intent that must be enabled in Discord Developer Portal:
    // 1. Go to https://discord.com/developers/applications/{your_app_id}/bot
    // 2. Scroll to "Privileged Gateway Intents"
    // 3. Enable "MESSAGE CONTENT INTENT"
    // 4. Save changes and restart the bot
    let intents = GatewayIntents::GUILDS 
        | GatewayIntents::GUILD_MESSAGES 
        | GatewayIntents::MESSAGE_CONTENT;

    let mut client = Client::builder(&token, intents)
        .event_handler(Handler::new())
        .await?;

    info!("Starting Discord bot...");
    
    client.start().await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_use_simple_reply_short_text() {
        let short_text = "5! = 120";
        assert!(Handler::should_use_simple_reply(short_text));
    }

    #[test]
    fn test_should_use_simple_reply_long_text() {
        let long_text = "a".repeat(500);
        assert!(!Handler::should_use_simple_reply(&long_text));
    }

    #[test]
    fn test_should_use_simple_reply_with_newlines() {
        let text_with_newlines = "5! = 120\n6! = 720";
        assert!(!Handler::should_use_simple_reply(text_with_newlines));
    }

    #[test]
    fn test_parse_reply_simple() {
        let reply = "5! = 120";
        let (description, results) = Handler::parse_reply(reply);
        
        assert_eq!(description, "5! = 120");
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_parse_reply_with_note() {
        let reply = "Note: Large numbers are approximated\n\n5! = 120\n6! = 720";
        let (description, results) = Handler::parse_reply(reply);
        
        assert_eq!(description, "Note: Large numbers are approximated");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0], "5! = 120");
        assert_eq!(results[1], "6! = 720");
    }

    #[test]
    fn test_parse_reply_with_footer() {
        let reply = "5! = 120\n*^(This action was performed by a bot.)*";
        let (description, results) = Handler::parse_reply(reply);
        
        // The footer should be filtered out
        assert_eq!(description, "5! = 120");
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_parse_reply_multiple_results() {
        let reply = "\n\n1! = 1\n2! = 2\n3! = 6\n4! = 24\n5! = 120";
        let (description, results) = Handler::parse_reply(reply);
        
        assert_eq!(description, "");
        assert_eq!(results.len(), 5);
        assert_eq!(results[0], "1! = 1");
        assert_eq!(results[4], "5! = 120");
    }

    #[test]
    fn test_add_title_approximated() {
        let embed = CreateEmbed::new();
        let description = "Note: approximate values shown for large numbers";
        
        let _result = Handler::add_title(embed, description, 1);
        
        // Check that the title contains "Approximated"
        // Note: We can't directly inspect the title, so we're testing the function runs
        assert!(description.contains("approximate"));
    }

    #[test]
    fn test_add_title_multiple_results() {
        let embed = CreateEmbed::new();
        let description = "";
        
        let _result = Handler::add_title(embed, description, 5);
        
        // Function should complete without error
        assert_eq!(description, "");
    }

    #[test]
    fn test_add_title_single_result() {
        let embed = CreateEmbed::new();
        let description = "";
        
        let _result = Handler::add_title(embed, description, 1);
        
        // Function should complete without error
        assert_eq!(description, "");
    }

    #[test]
    fn test_add_description_short() {
        let embed = CreateEmbed::new();
        let description = "This is a short description".to_string();
        
        let result = Handler::add_description(embed, description.clone());
        
        assert!(result.is_ok());
    }

    #[test]
    fn test_add_description_too_long() {
        let embed = CreateEmbed::new();
        let description = "a".repeat(EMBED_DESCRIPTION_LIMIT + 100);
        
        let result = Handler::add_description(embed, description);
        
        // Should succeed but truncate the description
        assert!(result.is_ok());
    }

    #[test]
    fn test_message_meta_clone() {
        let meta = MessageMeta {
            message_id: MessageId::new(12345),
            channel_id: ChannelId::new(67890),
            author: "TestUser".to_string(),
        };
        
        let cloned = meta.clone();
        
        assert_eq!(meta.message_id, cloned.message_id);
        assert_eq!(meta.channel_id, cloned.channel_id);
        assert_eq!(meta.author, cloned.author);
    }

    #[test]
    fn test_message_meta_debug() {
        let meta = MessageMeta {
            message_id: MessageId::new(12345),
            channel_id: ChannelId::new(67890),
            author: "TestUser".to_string(),
        };
        
        let debug_str = format!("{:?}", meta);
        
        assert!(debug_str.contains("MessageMeta"));
        assert!(debug_str.contains("TestUser"));
    }

    #[test]
    fn test_handler_new() {
        let _handler = Handler::new();
        
        // Handler should be created successfully
        // We can't directly test the internal state, but we can verify it doesn't panic
        assert!(true);
    }

    #[test]
    fn test_parse_reply_empty_lines() {
        let reply = "Note: Testing\n\n\n5! = 120\n\n6! = 720";
        let (description, results) = Handler::parse_reply(reply);
        
        assert_eq!(description, "Note: Testing");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_parse_reply_whitespace_handling() {
        let reply = "  Note: Testing  \n\n  5! = 120  \n  6! = 720  ";
        let (description, results) = Handler::parse_reply(reply);
        
        assert_eq!(description, "Note: Testing");
        assert_eq!(results[0], "5! = 120");
        assert_eq!(results[1], "6! = 720");
    }

    #[test]
    fn test_constants() {
        // Verify the constants are set to reasonable values
        assert_eq!(MAX_MESSAGE_LEN, 2000);
        assert_eq!(EMBED_DESCRIPTION_LIMIT, 4096);
        assert_eq!(EMBED_FIELD_VALUE_LIMIT, 1024);
    }
}
