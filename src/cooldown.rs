use crate::settings::{CAVEGEN_COOLDOWN, DM_COOLDOWN};
use crate::CooldownTimer;
use serenity::{client::Context, model::channel::Message, prelude::RwLock};
use std::{sync::Arc, time::SystemTime};

pub async fn check_cooldown(ctx: &Context, msg: &Message) -> bool {
    let cooldown_lock = get_cooldown_lock(ctx).await;

    let used_times = cooldown_lock.read().await;
    let oldest_elapsed = used_times.first().unwrap().elapsed().unwrap().as_secs();
    let is_dm = msg.is_private();

    // 10 second global cooldown, 3 second DM cooldown
    !(
        (is_dm && oldest_elapsed <= DM_COOLDOWN)
        || oldest_elapsed < CAVEGEN_COOLDOWN
    )
}

pub async fn update_cooldown(ctx: &Context) {
    let cooldown_lock = get_cooldown_lock(ctx).await;

    let mut used_times = cooldown_lock.write().await;
    used_times.rotate_left(1);
    *used_times.last_mut().unwrap() = SystemTime::now();
}

async fn get_cooldown_lock(ctx: &Context) -> Arc<RwLock<[SystemTime; 5]>> {
    let data_read = ctx.data.read().await;
    data_read
        .get::<CooldownTimer>()
        .expect("Expected CooldownTimer in TypeMap.")
        .clone()
}
