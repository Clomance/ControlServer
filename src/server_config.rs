use serde::{Deserialize, Serialize};
use serde_json::Value;



const CONFIG_FILE_PATH: &'static str = "./config.json";



pub struct ConfigError {
    pub text: String
}

impl ConfigError {
    pub fn new<S: ToString>(s: S) -> ConfigError {
        Self {
            text: s.to_string()
        }
    }
}


#[derive(Serialize,Deserialize)]
pub struct Config {
    /// Адрес веб-сервера.
    pub web_server_address: String,

    /// Порт веб-сервера.
    pub web_server_port: u16,

    pub volume: f32
}

impl Config {
    /// Загружает данные из файла конфигураций.
    pub fn load() -> Result<Config, ConfigError> {
        //  Чтение данных файла конфигураций
        let Ok(buffer) = std::fs::read(CONFIG_FILE_PATH) else {
            return Err(ConfigError::new(format!("Ошибка чтения файла настроек {CONFIG_FILE_PATH}")))
        };

        // Парсинг данные файла конфигураций
        let Ok(config) = serde_json::from_slice::<Value>(&buffer) else {
            return Err(ConfigError::new(format!("Ошибка чтения файла настроек {CONFIG_FILE_PATH}")))
        };


        // Чтение данных веб-сервера
        let mut web_server_address = "0.0.0.0".to_string();
        let mut web_server_port = 8080u16;
        if let Some(address) = config["web_server_address"].as_str().map(|s| s.to_string()) {
            web_server_address = address
        }

        if let Some(port) = config["web_server_port"].as_u64() {
            web_server_port = port as u16
        }

        let mut volume = 0.5f32;
        if let Some(volume_config) = config["volume"].as_f64() {
            volume = volume_config as f32
        }

        Ok (
            Self {
                web_server_address,
                web_server_port,
                volume
            }
        )
    }

    pub async fn save(&self) {
        let Ok(data) = serde_json::to_string_pretty(self) else { return };

        let _ = tokio::fs::write(CONFIG_FILE_PATH, data).await;
    }
}