use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 设备认证管理器
pub struct DeviceAuthManager {
    /// 设备代码 -> 密码
    devices: Arc<RwLock<HashMap<String, String>>>,
}

impl DeviceAuthManager {
    pub fn new() -> Self {
        Self {
            devices: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// 注册设备
    pub async fn register_device(&self, device_id: String, password: String) {
        let mut devices = self.devices.write().await;
        devices.insert(device_id, password);
    }
    
    /// 验证密码
    pub async fn verify_password(&self, device_id: &str, password: &str) -> bool {
        let devices = self.devices.read().await;
        match devices.get(device_id) {
            Some(stored_password) => stored_password == password,
            None => false,
        }
    }
    
    /// 生成随机密码
    pub fn generate_password() -> String {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let password: String = (0..6)
            .map(|_| rng.gen_range(0..10).to_string())
            .collect();
        password
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_register_and_verify() {
        let manager = DeviceAuthManager::new();
        manager.register_device("device-1".to_string(), "123456".to_string()).await;
        
        assert!(manager.verify_password("device-1", "123456").await);
        assert!(!manager.verify_password("device-1", "wrong").await);
        assert!(!manager.verify_password("unknown", "123456").await);
    }

    #[test]
    fn test_generate_password() {
        let password = DeviceAuthManager::generate_password();
        assert_eq!(password.len(), 6);
        assert!(password.chars().all(|c| c.is_ascii_digit()));
    }
}
