use eframe::egui;
use quick_xml::events::Event;
use quick_xml::Reader;
use rayon::prelude::*;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use walkdir::WalkDir;
use crate::settings::AppSettings;

pub struct DefBrowserTab {
    base_directory: String,
    defs: BTreeMap<String, Vec<DefEntry>>, // DefType -> List of entries
    selected_def_type: Option<String>,
    selected_def_entry: Option<usize>,
    is_loading: bool,
    status_message: String,
    settings: Arc<Mutex<AppSettings>>,
    initialized: bool,
}

#[derive(Debug, Clone)]
struct DefEntry {
    def_name: String,
    file_path: PathBuf,
    xml_content: String,
    def_type: String,
}

impl DefBrowserTab {
    pub fn new(settings: Arc<Mutex<AppSettings>>) -> Self {
        Self {
            base_directory: String::new(),
            defs: BTreeMap::new(),
            selected_def_type: None,
            selected_def_entry: None,
            is_loading: false,
            status_message: String::new(),
            settings,
            initialized: false,
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, _ctx: &egui::Context) {
        // 每次更新時檢查設置是否變更
        if let Ok(settings) = self.settings.lock() {
            if settings.base_path != self.base_directory {
                self.base_directory = settings.base_path.clone();
                self.initialized = true;
            }
        }

        // 頂部控制面板
        ui.horizontal(|ui| {
            ui.label("目錄:");
            ui.add_enabled(false, egui::TextEdit::singleline(&mut self.base_directory));

            if ui.button("🔄 掃描 Defs").clicked() && !self.base_directory.is_empty() {
                self.scan_defs();
            }

            // 狀態訊息
            if !self.status_message.is_empty() {
                ui.colored_label(
                    if self.is_loading {
                        egui::Color32::from_rgb(255, 165, 0)
                    } else {
                        egui::Color32::from_rgb(0, 200, 0)
                    },
                    &self.status_message,
                );
            }
        });

        ui.separator();

        // 主要內容區域：左側列表右側詳細資訊
        ui.horizontal_top(|ui| {
            // 左側面板
            let width = if ui.available_width() < 400.0 {
                200.0
            } else {
                220.0
            };
            ui.allocate_ui_with_layout(
                egui::vec2(width, ui.available_height()),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.heading("Def 類型");
                    ui.separator();

                    egui::ScrollArea::vertical()
                        .id_salt("def_type_list")
                        .auto_shrink([false; 2])
                        .show(ui, |ui| {
                            for (def_type, entries) in &self.defs {
                                let is_selected = self.selected_def_type.as_ref() == Some(def_type);

                                if ui
                                    .selectable_label(is_selected, format!("[{}]", def_type))
                                    .clicked()
                                {
                                    if is_selected {
                                        // 點擊已選擇的類型，收起
                                        self.selected_def_type = None;
                                        self.selected_def_entry = None;
                                    } else {
                                        // 選擇新類型
                                        self.selected_def_type = Some(def_type.clone());
                                        self.selected_def_entry = None;
                                    }
                                }

                                // 如果此類型被選中，顯示其下的所有條目
                                if is_selected {
                                    ui.indent(format!("indent_{}", def_type), |ui| {
                                        for (idx, entry) in entries.iter().enumerate() {
                                            let entry_selected =
                                                self.selected_def_entry == Some(idx);
                                            if ui
                                                .selectable_label(
                                                    entry_selected,
                                                    format!("  {}", entry.def_name),
                                                )
                                                .clicked()
                                            {
                                                self.selected_def_entry = Some(idx);
                                            }
                                        }
                                    });
                                }
                            }
                        });
                },
            );

            ui.separator();

            // 右側面板
            ui.allocate_ui_with_layout(
                egui::vec2(ui.available_width(), ui.available_height()),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.heading("詳細資訊");
                    ui.separator();

                    egui::ScrollArea::vertical()
                        .id_salt("def_detail_main")
                        .show(ui, |ui| {
                            if let Some(def_type) = &self.selected_def_type {
                                if let Some(entry_idx) = self.selected_def_entry {
                                    if let Some(entries) = self.defs.get(def_type) {
                                        if let Some(entry) = entries.get(entry_idx) {
                                            ui.label(format!("DefName: {}", entry.def_name));
                                            ui.label(format!("類型: {}", entry.def_type));

                                            // 可點擊的檔案路徑
                                            ui.horizontal(|ui| {
                                                ui.label("檔案: ");
                                                if ui
                                                    .link(entry.file_path.display().to_string())
                                                    .clicked()
                                                {
                                                    open_file_with_default_app(&entry.file_path);
                                                }
                                            });

                                            ui.separator();

                                            // 顯示 XML 內容
                                            ui.label("XML 定義:");
                                            egui::ScrollArea::both()
                                                .id_salt("def_xml_content")
                                                .max_height(400.0)
                                                .show(ui, |ui| {
                                                    ui.add(
                                                        egui::TextEdit::multiline(
                                                            &mut entry.xml_content.as_str(),
                                                        )
                                                        .code_editor()
                                                        .desired_width(f32::INFINITY),
                                                    );
                                                });
                                        }
                                    }
                                } else {
                                    ui.label("請選擇一個條目以查看詳細資訊");
                                }
                            } else {
                                ui.label("請選擇一個 Def 類型");
                            }
                        });
                },
            );
        });
    }

    fn scan_defs(&mut self) {
        self.is_loading = true;
        self.status_message = "正在掃描 Defs...".to_string();
        self.defs.clear();
        self.selected_def_type = None;
        self.selected_def_entry = None;

        let base_path = PathBuf::from(&self.base_directory);

        // 尋找所有 Defs 目錄下的 XML 檔案
        let xml_files: Vec<PathBuf> = WalkDir::new(&base_path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path().is_file()
                    && e.path().extension().and_then(|s| s.to_str()) == Some("xml")
                    && e.path().to_str().map_or(false, |s| s.contains("Defs"))
            })
            .map(|e| e.path().to_path_buf())
            .collect();

        self.status_message = format!("找到 {} 個 XML 檔案，正在解析...", xml_files.len());

        // 使用並行處理解析檔案
        let parsed_entries: Vec<DefEntry> = xml_files
            .par_iter()
            .filter_map(|path| parse_defs_from_file(path).ok())
            .flatten()
            .collect();

        // 按 DefType 分組
        for entry in parsed_entries {
            self.defs
                .entry(entry.def_type.clone())
                .or_insert_with(Vec::new)
                .push(entry);
        }

        // 排序每個類型內的條目
        for entries in self.defs.values_mut() {
            entries.sort_by(|a, b| a.def_name.cmp(&b.def_name));
        }

        let total_defs: usize = self.defs.values().map(|v| v.len()).sum();
        self.status_message = format!(
            "掃描完成！找到 {} 種類型，共 {} 個 Defs",
            self.defs.len(),
            total_defs
        );
        self.is_loading = false;
    }
}

fn parse_defs_from_file(path: &Path) -> Result<Vec<DefEntry>, Box<dyn std::error::Error>> {
    let content = fs::read_to_string(path)?;
    let mut reader = Reader::from_str(&content);
    reader.config_mut().trim_text(true);

    let mut entries = Vec::new();
    let mut buf = Vec::new();
    let mut current_def_type: Option<String> = None;
    let mut current_def_name: Option<String> = None;
    let mut def_depth = 0;
    let mut inside_defs = false;
    let mut inside_defname = false;
    let mut xml_parts: Vec<String> = Vec::new();
    let mut capturing = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();

                if name == "Defs" {
                    inside_defs = true;
                } else if inside_defs && def_depth == 0 && name.ends_with("Def") {
                    // 開始一個新的 Def
                    current_def_type = Some(name.clone());
                    current_def_name = None;
                    def_depth = 1;
                    xml_parts.clear();
                    capturing = true;
                    
                    // 記錄開始標籤
                    let attrs: Vec<String> = e.attributes()
                        .filter_map(|a| a.ok())
                        .map(|attr| {
                            format!("{}=\"{}\"",
                                String::from_utf8_lossy(attr.key.as_ref()),
                                String::from_utf8_lossy(&attr.value))
                        })
                        .collect();
                    
                    if attrs.is_empty() {
                        xml_parts.push(format!("<{}>", name));
                    } else {
                        xml_parts.push(format!("<{} {}>", name, attrs.join(" ")));
                    }
                } else if def_depth > 0 {
                    if name == "defName" {
                        inside_defname = true;
                    }
                    def_depth += 1;
                    
                    if capturing {
                        let attrs: Vec<String> = e.attributes()
                            .filter_map(|a| a.ok())
                            .map(|attr| {
                                format!("{}=\"{}\"",
                                    String::from_utf8_lossy(attr.key.as_ref()),
                                    String::from_utf8_lossy(&attr.value))
                            })
                            .collect();
                        
                        if attrs.is_empty() {
                            xml_parts.push(format!("<{}>", name));
                        } else {
                            xml_parts.push(format!("<{} {}>", name, attrs.join(" ")));
                        }
                    }
                }
            }
            Ok(Event::Empty(ref e)) => {
                if capturing && def_depth > 0 {
                    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    let attrs: Vec<String> = e.attributes()
                        .filter_map(|a| a.ok())
                        .map(|attr| {
                            format!("{}=\"{}\"",
                                String::from_utf8_lossy(attr.key.as_ref()),
                                String::from_utf8_lossy(&attr.value))
                        })
                        .collect();
                    
                    if attrs.is_empty() {
                        xml_parts.push(format!("<{} />", name));
                    } else {
                        xml_parts.push(format!("<{} {} />", name, attrs.join(" ")));
                    }
                }
            }
            Ok(Event::Text(e)) => {
                if inside_defname {
                    if let Ok(text) = e.unescape() {
                        current_def_name = Some(text.trim().to_string());
                    }
                }
                if capturing {
                    if let Ok(text) = e.unescape() {
                        let trimmed = text.trim();
                        if !trimmed.is_empty() {
                            xml_parts.push(trimmed.to_string());
                        }
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();

                if name == "defName" {
                    inside_defname = false;
                }

                if def_depth > 0 {
                    if capturing {
                        xml_parts.push(format!("</{}>", name));
                    }
                    
                    def_depth -= 1;

                    if def_depth == 0 && name.ends_with("Def") {
                        // Def 結束
                        if let (Some(def_type), Some(def_name)) =
                            (&current_def_type, &current_def_name)
                        {
                            entries.push(DefEntry {
                                def_name: def_name.clone(),
                                file_path: path.to_path_buf(),
                                xml_content: format_xml(&xml_parts.join("")),
                                def_type: def_type.clone(),
                            });
                        }
                        current_def_type = None;
                        current_def_name = None;
                        capturing = false;
                    }
                }

                if name == "Defs" {
                    inside_defs = false;
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }

        buf.clear();
    }

    Ok(entries)
}

// 簡單格式化 XML 使其更易讀
fn format_xml(xml: &str) -> String {
    let mut result = String::new();
    let mut indent_level = 0;
    let mut chars = xml.chars().peekable();
    let mut after_text = false; // 追蹤是否剛輸出了文本內容
    
    while let Some(ch) = chars.next() {
        if ch == '<' {
            // 收集完整的標籤
            let mut tag = String::from('<');
            let mut is_closing = false;
            let mut is_self_closing = false;
            
            // 檢查是否是結束標籤
            if chars.peek() == Some(&'/') {
                is_closing = true;
            }
            
            // 收集標籤內容
            while let Some(&next_ch) = chars.peek() {
                tag.push(chars.next().unwrap());
                if next_ch == '>' {
                    // 檢查是否是自閉合標籤
                    if tag.ends_with("/>") {
                        is_self_closing = true;
                    }
                    break;
                }
            }
            
            // 輸出標籤
            if is_closing {
                // 結束標籤
                if after_text {
                    // 如果前面有文本內容，標籤直接跟在後面（同一行）
                    result.push_str(&tag);
                    result.push('\n');
                    after_text = false;
                } else {
                    // 否則，先減少縮排再輸出
                    if indent_level > 0 {
                        indent_level -= 1;
                    }
                    result.push_str(&"  ".repeat(indent_level));
                    result.push_str(&tag);
                    result.push('\n');
                }
            } else if is_self_closing {
                // 自閉合標籤
                result.push_str(&"  ".repeat(indent_level));
                result.push_str(&tag);
                result.push('\n');
                after_text = false;
            } else {
                // 開始標籤
                result.push_str(&"  ".repeat(indent_level));
                result.push_str(&tag);
                
                // 檢查下一個字符是否是文本內容（不是 '<'）
                if let Some(&next_ch) = chars.peek() {
                    if next_ch != '<' {
                        // 收集文本內容直到下一個標籤
                        let mut text = String::new();
                        while let Some(&ch) = chars.peek() {
                            if ch == '<' {
                                break;
                            }
                            text.push(chars.next().unwrap());
                        }
                        
                        let trimmed = text.trim();
                        if !trimmed.is_empty() {
                            result.push_str(trimmed);
                            after_text = true;
                        }
                        // 文本後不增加縮排，因為下一個應該是結束標籤
                    } else {
                        // 下一個是標籤，換行並增加縮排
                        result.push('\n');
                        indent_level += 1;
                        after_text = false;
                    }
                } else {
                    result.push('\n');
                    indent_level += 1;
                    after_text = false;
                }
            }
        }
    }
    
    result
}

// 使用系統預設程式打開檔案
fn open_file_with_default_app(path: &Path) {
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", "", path.to_str().unwrap_or("")])
            .spawn();
    }

    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(path).spawn();
    }

    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("xdg-open").arg(path).spawn();
    }
}
