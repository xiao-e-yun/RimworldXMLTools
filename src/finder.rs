use eframe::egui;
use rayon::prelude::*;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use walkdir::WalkDir;

use crate::settings::AppSettings;
use crate::xml_parser::extract_tag_values;

pub struct SearchResult {
    pub values: Vec<String>,
    pub xml_count: usize,
}

pub struct TagFinderTab {
    tag_name: String,
    search_path: String,
    results: Vec<String>,
    status_message: String,
    is_searching: bool,
    last_tag_name: String,
    last_search_path: String,
    search_results: Arc<Mutex<Option<SearchResult>>>,
    cancel_flag: Arc<AtomicBool>,
    settings: Arc<Mutex<AppSettings>>,
    initialized: bool,
}

impl TagFinderTab {
    pub fn new(settings: Arc<Mutex<AppSettings>>) -> Self {
        Self {
            tag_name: String::new(),
            search_path: String::new(),
            results: Vec::new(),
            status_message: String::new(),
            is_searching: false,
            last_tag_name: String::new(),
            last_search_path: String::new(),
            search_results: Arc::new(Mutex::new(None)),
            cancel_flag: Arc::new(AtomicBool::new(false)),
            settings,
            initialized: false,
        }
    }

    pub fn search_xml_files(&mut self, ctx: egui::Context) {
        // 取消之前的搜尋
        self.cancel_flag.store(true, Ordering::Relaxed);

        self.results.clear();
        self.status_message = "搜尋中...".to_string();
        self.is_searching = true;

        if self.tag_name.is_empty() {
            self.status_message = "錯誤: 請輸入標籤名稱".to_string();
            self.is_searching = false;
            return;
        }

        if self.search_path.is_empty() {
            self.status_message = "錯誤: 請選擇搜尋路徑".to_string();
            self.is_searching = false;
            return;
        }

        let path = PathBuf::from(&self.search_path);
        if !path.exists() {
            self.status_message = format!("錯誤: 路徑不存在: {}", self.search_path);
            self.is_searching = false;
            return;
        }

        let tag_name = self.tag_name.clone();
        let search_results = self.search_results.clone();

        // 創建新的取消旗標
        let cancel_flag = Arc::new(AtomicBool::new(false));
        self.cancel_flag = cancel_flag.clone();

        // 在後台執行緒中執行搜尋
        std::thread::spawn(move || {
            // 收集所有 XML 檔案路徑
            let xml_files: Vec<PathBuf> = WalkDir::new(&path)
                .follow_links(true)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| {
                    e.file_type().is_file()
                        && e.path().extension().map_or(false, |ext| ext == "xml")
                })
                .map(|e| e.path().to_path_buf())
                .collect();

            let xml_count = xml_files.len();

            // 使用 rayon 平行處理 XML 檔案，並檢查取消旗標
            let values: HashSet<String> = xml_files
                .par_iter()
                .filter(|_| !cancel_flag.load(Ordering::Relaxed))
                .filter_map(|path| extract_tag_values(path, &tag_name).ok())
                .flatten()
                .collect();

            // 如果被取消，不儲存結果
            if cancel_flag.load(Ordering::Relaxed) {
                return;
            }

            // 排序結果
            let mut sorted_values: Vec<String> = values.into_iter().collect();
            sorted_values.sort();

            // 儲存結果
            if let Ok(mut result) = search_results.lock() {
                *result = Some(SearchResult {
                    values: sorted_values,
                    xml_count,
                });
            }

            // 請求重繪 UI
            ctx.request_repaint();
        });
    }

    fn check_search_results(&mut self) {
        if let Ok(mut result) = self.search_results.lock() {
            if let Some(search_result) = result.take() {
                self.results = search_result.values;
                self.status_message = format!(
                    "掃描了 {} 個 XML 檔案，找到 {} 個唯一值",
                    search_result.xml_count,
                    self.results.len()
                );
                self.is_searching = false;
            }
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        // 每次更新時檢查設置是否變更
        let mut should_search = false;
        if let Ok(settings) = self.settings.lock() {
            if settings.base_path != self.search_path {
                self.search_path = settings.base_path.clone();
                self.last_search_path = self.search_path.clone();
                self.initialized = true;
                // 如果有標籤名稱,標記需要重新搜尋
                if !self.tag_name.is_empty() && !self.search_path.is_empty() {
                    should_search = true;
                }
            }
        }
        
        // 在鎖釋放後執行搜尋
        if should_search {
            self.search_xml_files(ctx.clone());
        }

        // 檢查後台搜尋結果
        self.check_search_results();

        // 頂部控制面板
        ui.horizontal(|ui| {
            ui.label("目錄:");
            
            // 檢測輸入變化 - 設為唯讀
            ui.add_enabled(false, egui::TextEdit::singleline(&mut self.search_path));

            // 狀態訊息
            if !self.status_message.is_empty() {
                ui.colored_label(
                    if self.is_searching {
                        egui::Color32::from_rgb(255, 165, 0)
                    } else {
                        egui::Color32::from_rgb(0, 200, 0)
                    },
                    &self.status_message,
                );
            }
        });

        ui.separator();

        ui.horizontal(|ui| {
            ui.label("🔍");
            let response = ui.text_edit_singleline(&mut self.tag_name);

            // 檢測輸入變化
            if response.changed() && self.tag_name != self.last_tag_name {
                self.last_tag_name = self.tag_name.clone();
                if !self.tag_name.is_empty() && !self.search_path.is_empty() {
                    self.search_xml_files(ctx.clone());
                }
            }
        });
        
        ui.separator();

        // 結果顯示區域
        if !self.results.is_empty() {
            // 複製按鈕
            ui.horizontal(|ui| {
                ui.label(format!("找到 {} 個唯一值:", self.results.len()));
                
                if ui.button("📋 複製結果").clicked() {
                    ui.output_mut(|o| o.copied_text = self.results.join(", "));
                }
            });

            ui.separator();

            const MAX_DISPLAY: usize = 100;
            let display_results = if self.results.len() > MAX_DISPLAY {
                &self.results[..MAX_DISPLAY]
            } else {
                &self.results[..]
            };

            let comma_separated = if self.results.len() > MAX_DISPLAY {
                format!("{}, ...", display_results.join(", "))
            } else {
                display_results.join(", ")
            };

            if self.results.len() > MAX_DISPLAY {
                ui.label(format!("（顯示前 {} 項，共 {} 項）", MAX_DISPLAY, self.results.len()));
            }

            // 顯示逗號分隔的結果
            egui::ScrollArea::vertical()
                .id_salt("tag_results")
                .show(ui, |ui| {
                    ui.label(&comma_separated);
                });
        } else if !self.is_searching && !self.status_message.is_empty() {
            ui.label("沒有找到結果");
        }
    }
}
