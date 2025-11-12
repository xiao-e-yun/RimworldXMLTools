use eframe::egui;
use rayon::prelude::*;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use walkdir::WalkDir;

use crate::xml_parser::extract_tag_values;

pub struct SearchResult {
    pub values: Vec<String>,
    pub xml_count: usize,
}

#[derive(Default)]
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
}

impl TagFinderTab {
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
        // 檢查後台搜尋結果
        self.check_search_results();

        ui.heading("XML 標籤查找器");
        ui.add_space(10.0);

        // 標籤名稱輸入
        ui.horizontal(|ui| {
            ui.label("標籤名稱:");
            let response = ui
                .text_edit_singleline(&mut self.tag_name)
                .on_hover_text("例如: stuffCategories, thingClass");

            // 檢測輸入變化
            if response.changed() && self.tag_name != self.last_tag_name {
                self.last_tag_name = self.tag_name.clone();
                if !self.tag_name.is_empty() && !self.search_path.is_empty() {
                    self.search_xml_files(ctx.clone());
                }
            }
        });

        ui.add_space(5.0);

        // 搜尋路徑輸入
        ui.horizontal(|ui| {
            ui.label("搜尋路徑:");
            let response = ui.text_edit_singleline(&mut self.search_path);

            // 檢測輸入變化
            if response.changed() && self.search_path != self.last_search_path {
                self.last_search_path = self.search_path.clone();
                if !self.tag_name.is_empty() && !self.search_path.is_empty() {
                    self.search_xml_files(ctx.clone());
                }
            }

            if ui.button("瀏覽...").clicked() {
                if let Some(path) = rfd::FileDialog::new().pick_folder() {
                    self.search_path = path.display().to_string();
                    self.last_search_path = self.search_path.clone();
                    if !self.tag_name.is_empty() {
                        self.search_xml_files(ctx.clone());
                    }
                }
            }
        });

        ui.add_space(10.0);

        // 手動搜尋按鈕
        if ui
            .add_enabled(!self.is_searching, egui::Button::new("🔍 重新搜尋"))
            .clicked()
        {
            self.search_xml_files(ctx.clone());
        }

        ui.add_space(10.0);

        // 狀態訊息
        if !self.status_message.is_empty() {
            if self.is_searching {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(&self.status_message);
                });
            } else {
                ui.label(&self.status_message);
            }
            ui.add_space(5.0);
        }

        // 結果顯示（逗號分隔格式）
        if !self.results.is_empty() {
            ui.separator();
            ui.add_space(5.0);

            // 限制顯示前 50 項
            const MAX_DISPLAY: usize = 50;
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

            // 完整的結果（用於複製）
            let full_results = self.results.join(", ");

            // 複製按鈕
            ui.horizontal(|ui| {
                if ui.button("📋 複製結果").clicked() {
                    ui.output_mut(|o| o.copied_text = full_results.clone());
                }

                if self.results.len() > MAX_DISPLAY {
                    ui.label(format!(
                        "（顯示前 {} 項，共 {} 項）",
                        MAX_DISPLAY,
                        self.results.len()
                    ));
                }
            });

            ui.add_space(5.0);

            // 顯示逗號分隔的結果
            egui::ScrollArea::vertical()
                .max_height(400.0)
                .show(ui, |ui| {
                    ui.label(&comma_separated);
                });
        }
    }
}
