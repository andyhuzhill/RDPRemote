use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

/// JWT 声明
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub device_id: String,
    pub exp: usize,
    pub iat: usize,
}

/// 认证管理器
pub struct AuthManager {
    secret: String,
    #[allow(dead_code)]
    token_duration: Duration,
}

impl AuthManager {
    /// 创建新的认证管理器
    pub fn new(secret: impl Into<String>) -> Self {
        Self {
            secret: secret.into(),
            token_duration: Duration::hours(24),
        }
    }

    /// 生成 JWT 令牌
    #[allow(dead_code)]
    pub fn generate_token(&self, device_id: &str) -> Result<String, anyhow::Error> {
        let now = Utc::now();
        let exp = (now + self.token_duration).timestamp() as usize;
        let iat = now.timestamp() as usize;

        let claims = Claims {
            device_id: device_id.to_string(),
            exp,
            iat,
        };

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.secret.as_bytes()),
        )?;

        Ok(token)
    }

    /// 验证 JWT 令牌
    pub fn verify_token(&self, token: &str) -> Result<String, anyhow::Error> {
        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.secret.as_bytes()),
            &Validation::default(),
        )?;

        Ok(token_data.claims.device_id)
    }

    /// 获取令牌有效期（小时）
    #[allow(dead_code)]
    pub fn token_duration_hours(&self) -> i64 {
        self.token_duration.num_hours()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_and_verify_token() {
        let auth = AuthManager::new("test-secret");
        let device_id = "device-123";

        let token = auth.generate_token(device_id).unwrap();
        let verified_id = auth.verify_token(&token).unwrap();

        assert_eq!(verified_id, device_id);
    }

    #[test]
    fn test_invalid_token_fails() {
        let auth = AuthManager::new("test-secret");
        let result = auth.verify_token("invalid.token.here");
        assert!(result.is_err());
    }

    #[test]
    fn test_wrong_secret_fails() {
        let auth1 = AuthManager::new("secret1");
        let auth2 = AuthManager::new("secret2");

        let token = auth1.generate_token("device-123").unwrap();
        let result = auth2.verify_token(&token);
        assert!(result.is_err());
    }
}
