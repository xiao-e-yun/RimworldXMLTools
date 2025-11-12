use eframe::egui;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// 共享的應用設置
#[derive(Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub base_path: String,  // 統一的基礎路徑
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            base_path: String::new(),
        }
    }
}

impl AppSettings {
    /// 從檔案載入設置
    pub fn load() -> Self {
        if let Ok(config_path) = Self::config_path() {
            if let Ok(content) = std::fs::read_to_string(config_path) {
                if let Ok(settings) = serde_json::from_str(&content) {
                    return settings;
                }
            }
        }
        Self::default()
    }

    /// 儲存設置到檔案
    pub fn save(&self) {
        if let Ok(config_path) = Self::config_path() {
            if let Some(parent) = config_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(json) = serde_json::to_string_pretty(self) {
                let _ = std::fs::write(config_path, json);
            }
        }
    }

    /// 獲取設置檔案路徑
    fn config_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
        let mut path = if cfg!(target_os = "windows") {
            PathBuf::from(std::env::var("APPDATA")?)
        } else {
            PathBuf::from(std::env::var("HOME")?)
        };
        
        path.push("RimWorldXMLTools");
        path.push("settings.json");
        Ok(path)
    }
}

/// 設置分頁
pub struct SettingsTab {
    settings: Arc<Mutex<AppSettings>>,
    status_message: String,
}

impl SettingsTab {
    pub fn new(settings: Arc<Mutex<AppSettings>>) -> Self {
        Self {
            settings,
            status_message: String::new(),
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, _ctx: &egui::Context) {
        ui.heading("⚙️ 路徑設置");
        ui.separator();

        ui.label("在此處設置統一的工作目錄路徑。所有功能將使用此路徑作為基礎目錄。");
        ui.add_space(10.0);

        let mut settings = self.settings.lock().unwrap();
        let mut changed = false;

        // 統一的基礎路徑
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.label("� 工作目錄:");
            });
            
            ui.horizontal(|ui| {
                if ui.text_edit_singleline(&mut settings.base_path).changed() {
                    changed = true;
                }

                if ui.button("📂 選擇目錄").clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_folder() {
                        settings.base_path = path.display().to_string();
                        changed = true;
                    }
                }
            });
            
            ui.label("此路徑將用於所有功能：Def 瀏覽器、繼承展開、標籤查找器");
        });

        ui.add_space(20.0);

        // 操作按鈕
        ui.horizontal(|ui| {
            if ui.button("💾 儲存設置").clicked() || changed {
                settings.save();
                self.status_message = "✅ 設置已儲存".to_string();
            }

            if ui.button("🔄 重置為空").clicked() {
                *settings = AppSettings::default();
                settings.save();
                self.status_message = "✅ 已重置路徑".to_string();
            }

            if !self.status_message.is_empty() {
                ui.colored_label(egui::Color32::from_rgb(0, 200, 0), &self.status_message);
            }
        });

        ui.add_space(10.0);
        ui.separator();
        
        // 顯示設置檔案位置
        if let Ok(config_path) = AppSettings::config_path() {
            ui.label(format!("💾 設置檔案: {}", config_path.display()));
        }
        
        ui.add_space(10.0);
        
        // 說明資訊
        ui.group(|ui| {
            ui.label("ℹ️ 使用說明:");
            ui.label("• 設置的路徑會在切換到各個分頁時自動載入");
            ui.label("• 在各分頁中選擇新目錄會自動更新此設置");
            ui.label("• 建議選擇 RimWorld 的 Data 資料夾");
        });
    }
}
