use redis::aio::ConnectionManager;
use uuid::Uuid;
use crate::models::DeductionTransaction;
use log::warn;

const DEDUCTION_AVAILABLE_FIELD: &str = "deduction_available";
const DEDUCTION_LIMIT_FIELD: &str = "deduction_limit";

#[derive(Clone)]
pub struct RedisCache {
    pub conn: Option<ConnectionManager>,
    prefix: String,
}

#[allow(dead_code)]
impl RedisCache {
    pub fn new(conn: Option<ConnectionManager>, prefix: String) -> Self {
        Self { conn, prefix }
    }

    fn dev_key(&self, dev_uuid: Uuid) -> String {
        format!("{}:dev:{}", self.prefix, dev_uuid)
    }

    fn pending_key(&self, dev_uuid: Uuid) -> String {
        format!("{}:pending:{}", self.prefix, dev_uuid)
    }

    fn transaction_key(&self, tx_token: Uuid) -> String {
        format!("{}:tx:{}", self.prefix, tx_token)
    }

    fn processed_key(&self, tx_token: Uuid) -> String {
        format!("{}:processed:{}", self.prefix, tx_token)
    }

    pub async fn get_deduction_data(
        &mut self,
        dev_uuid: Uuid,
    ) -> Option<(i32, i32)> {
        let key = self.dev_key(dev_uuid);
        if let Some(ref mut conn) = self.conn {
            use redis::AsyncCommands;
            let available: Option<i32> = conn.hget(&key, DEDUCTION_AVAILABLE_FIELD).await.ok().flatten();
            let limit: Option<i32> = conn.hget(&key, DEDUCTION_LIMIT_FIELD).await.ok().flatten();

            if let (Some(avail), Some(lim)) = (available, limit) {
                return Some((avail, lim));
            }
        }
        None
    }

    pub async fn set_deduction_data(
        &mut self,
        dev_uuid: Uuid,
        available: i32,
        limit: i32,
    ) {
        let key = self.dev_key(dev_uuid);
        if let Some(ref mut conn) = self.conn {
            use redis::AsyncCommands;
            let _: Result<(), _> = conn.hset_multiple(&key, &[
                (DEDUCTION_AVAILABLE_FIELD, available),
                (DEDUCTION_LIMIT_FIELD, limit),
            ]).await;
        }
    }

    pub async fn get_pending_count(
        &mut self,
        dev_uuid: Uuid,
    ) -> i64 {
        let key = self.pending_key(dev_uuid);
        if let Some(ref mut conn) = self.conn {
            use redis::AsyncCommands;
            let pending: i64 = conn.get(&key).await.unwrap_or(0);
            pending
        } else {
            0
        }
    }

    pub async fn add_pending(
        &mut self,
        dev_uuid: Uuid,
        amount: i64,
        ttl_secs: u64,
    ) {
        let key = self.pending_key(dev_uuid);
        if let Some(ref mut conn) = self.conn {
            use redis::AsyncCommands;
            let _: Result<i64, _> = conn.incr(&key, amount).await;
            let _: Result<(), _> = conn.expire(&key, ttl_secs as i64).await;
        }
    }

    pub async fn remove_pending(
        &mut self,
        dev_uuid: Uuid,
        amount: i64,
    ) {
        let key = self.pending_key(dev_uuid);
        if let Some(ref mut conn) = self.conn {
            use redis::AsyncCommands;
            let _: Result<i64, _> = conn.decr(&key, amount).await;
        }
    }

    pub async fn incr_pending_count(
        &mut self,
        dev_uuid: Uuid,
    ) {
        let key = self.pending_key(dev_uuid);
        if let Some(ref mut conn) = self.conn {
            use redis::AsyncCommands;
            let _: Result<i64, _> = conn.incr(&key, 1).await;
        }
    }

    pub async fn decr_pending_count(
        &mut self,
        dev_uuid: Uuid,
    ) {
        let key = self.pending_key(dev_uuid);
        if let Some(ref mut conn) = self.conn {
            use redis::AsyncCommands;
            let _: Result<i64, _> = conn.decr(&key, 1).await;
        }
    }

    pub async fn deduct(
        &mut self,
        dev_uuid: Uuid,
        amount: i32,
    ) -> Result<(bool, i32), crate::errors::AppError> {
        let key = self.dev_key(dev_uuid);
        if let Some(ref mut conn) = self.conn {
            let script = r#"
                local key = KEYS[1]
                local amount = tonumber(ARGV[1])
                local current = tonumber(redis.call('HGET', key, 'deduction_available'))
                if current == nil then
                    return {-1, 0}
                end
                if current < amount then
                    return {0, current}
                end
                local new_val = current - amount
                redis.call('HSET', key, 'deduction_available', new_val)
                return {1, new_val}
            "#;
            let result: (i64, i64) = redis::cmd("EVAL")
                .arg(script)
                .arg(1)
                .arg(&key)
                .arg(amount)
                .query_async(conn)
                .await
                .map_err(|e| crate::errors::AppError::RedisError(e.to_string()))?;
            match result.0 {
                1 => Ok((true, result.1 as i32)),
                0 => Ok((false, result.1 as i32)),
                -1 => Ok((false, 0)),
                _ => Ok((false, 0)),
            }
        } else {
            Ok((false, 0))
        }
    }

    pub async fn refund(
        &mut self,
        dev_uuid: Uuid,
        amount: i32,
    ) {
        let key = self.dev_key(dev_uuid);
        if let Some(ref mut conn) = self.conn {
            let script = r#"
                local key = KEYS[1]
                local amount = tonumber(ARGV[1])
                local current = tonumber(redis.call('HGET', key, 'deduction_available'))
                local limit = tonumber(redis.call('HGET', key, 'deduction_limit'))
                if current == nil or limit == nil then
                    return 0
                end
                local new_val = math.min(current + amount, limit)
                redis.call('HSET', key, 'deduction_available', new_val)
                return 1
            "#;
            let _: Result<i64, _> = redis::cmd("EVAL")
                .arg(script)
                .arg(1)
                .arg(&key)
                .arg(amount)
                .query_async(conn)
                .await;
        }
    }

    pub async fn set_transaction(
        &mut self,
        tx: &DeductionTransaction,
        ttl_secs: u64,
    ) {
        let key = self.transaction_key(tx.transaction_token);
        if let Some(ref mut conn) = self.conn {
            use redis::AsyncCommands;
            let Ok(serialized) = serde_json::to_string(tx) else {
                return;
            };
            let _: Result<(), _> = conn.set_ex(&key, serialized, ttl_secs).await;
        }
    }

    pub async fn del_transaction(
        &mut self,
        tx_token: Uuid,
    ) {
        let key = self.transaction_key(tx_token);
        if let Some(ref mut conn) = self.conn {
            use redis::AsyncCommands;
            let _: Result<(), _> = conn.del(&key).await;
        }
    }

    pub async fn try_claim_processed(
        &mut self,
        tx_token: Uuid,
    ) -> bool {
        let key = self.processed_key(tx_token);
        if let Some(ref mut conn) = self.conn {
            use redis::AsyncCommands;
            let claimed: bool = conn.set_nx(&key, "1").await.unwrap_or(false);
            if claimed {
                let _: Result<(), _> = conn.expire(&key, 3600).await;
            }
            claimed
        } else {
            true
        }
    }

    pub async fn del_processed(
        &mut self,
        tx_token: Uuid,
    ) {
        let key = self.processed_key(tx_token);
        if let Some(ref mut conn) = self.conn {
            use redis::AsyncCommands;
            let _: Result<(), _> = conn.del(&key).await;
        }
    }

    pub async fn load_initial_data(
        &mut self,
        dev_uuid: Uuid,
        available: i32,
        limit: i32,
    ) {
        let key = self.dev_key(dev_uuid);
        if let Some(ref mut conn) = self.conn {
            use redis::AsyncCommands;
            let exists: bool = conn.exists(&key).await.unwrap_or(false);
            if !exists {
                let _: Result<(), _> = conn.hset_multiple(&key, &[
                    (DEDUCTION_AVAILABLE_FIELD, available),
                    (DEDUCTION_LIMIT_FIELD, limit),
                ]).await;
                warn!("Loaded initial Redis data for dev: available={}, limit={}", available, limit);
            }
        }
    }
}