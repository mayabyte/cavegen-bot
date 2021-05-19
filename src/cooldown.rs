use std::time::SystemTime;

use crate::CooldownTimer;
use serenity::{client::Context, model::channel::Message};

pub async fn check_cooldown(command: &str, ctx: &Context, msg: &Message) -> bool {
    let cooldown_lock = {
        let data_read = ctx.data.read().await;
        data_read
            .get::<CooldownTimer>()
            .expect("Expected CooldownTimer in TypeMap.")
            .clone()
    };

    let command_key = command_key(command, msg);
    let cooldowns = cooldown_lock.read().await;
    if let Some(last_time) = cooldowns.get(&command_key) {
        let elapsed_seconds = last_time.elapsed().unwrap().as_secs();
        let is_dm = msg.is_private();

        if !is_dm && elapsed_seconds < 600u64 {
            false
        }
        // Short cooldown for DMs to avoid spamming
        else if is_dm && elapsed_seconds < 5u64 {
            false
        } else {
            true
        }
    } else {
        true
    }
}

pub async fn update_cooldown(command: &str, ctx: &Context, msg: &Message) {
    let cooldown_lock = {
        let data_read = ctx.data.read().await;
        data_read
            .get::<CooldownTimer>()
            .expect("Expected CooldownTimer in TypeMap.")
            .clone()
    };

    let command_key = command_key(command, msg);
    let mut cooldowns = cooldown_lock.write().await;
    cooldowns.entry(command_key).insert(SystemTime::now());
}

fn command_key(command: &str, msg: &Message) -> String {
    format!("{};{};{}", command, msg.channel_id, msg.author.id)
}
