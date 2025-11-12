use eframe::egui;
use quick_xml::events::Event;
use quick_xml::Reader;
use rayon::prelude::*;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Default)]
pub struct DefBrowserTab {
    base_directory: String,
    defs: BTreeMap<String, Vec<DefEntry>>, // DefType -> List of entries
    selected_def_type: Option<String>,
    selected_def_entry: Option<usize>,
    is_loading: bool,
    status_message: String,
}

#[derive(Debug, Clone)]
struct DefEntry {
    def_name: String,
    file_path: PathBuf,
    xml_content: String,
    def_type: String,
}

impl DefBrowserTab {
    pub fn ui(&mut self, ui: &mut egui::Ui, _ctx: &egui::Context) {
        // 頂部控制面板
        ui.horizontal(|ui| {
            ui.label("目錄:");
            ui.text_edit_singleline(&mut self.base_directory);

            if ui.button("📂 選擇目錄").clicked() {
                if let Some(path) = rfd::FileDialog::new().pick_folder() {
                    self.base_directory = path.display().to_string();
                    // 選擇目錄後自動掃描
                    self.scan_defs();
                }
            }

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
    let bytes = content.as_bytes();
    let mut reader = Reader::from_str(&content);
    reader.config_mut().trim_text(true);

    let mut entries = Vec::new();
    let mut buf = Vec::new();
    let mut current_def_type: Option<String> = None;
    let mut current_def_name: Option<String> = None;
    let mut def_start_pos: usize = 0;
    let mut def_depth = 0;
    let mut inside_defs = false;
    let mut inside_defname = false;

    loop {
        let event_pos = reader.buffer_position() as usize;
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();

                if name == "Defs" {
                    inside_defs = true;
                } else if inside_defs && def_depth == 0 && name.ends_with("Def") {
                    // 開始一個新的 Def，記錄起始位置
                    current_def_type = Some(name.clone());
                    current_def_name = None;
                    def_start_pos = event_pos;
                    def_depth = 1;
                } else if def_depth > 0 {
                    if name == "defName" {
                        inside_defname = true;
                    }
                    def_depth += 1;
                }
            }
            Ok(Event::Text(e)) => {
                if inside_defname {
                    if let Ok(text) = e.unescape() {
                        current_def_name = Some(text.trim().to_string());
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();

                if name == "defName" {
                    inside_defname = false;
                }

                if def_depth > 0 {
                    def_depth -= 1;

                    if def_depth == 0 {
                        // Def 結束，記錄結束位置並提取 XML 內容
                        let def_end_pos = reader.buffer_position() as usize;

                        if let (Some(def_type), Some(def_name)) =
                            (&current_def_type, &current_def_name)
                        {
                            // 提取從 def_start_pos 到 def_end_pos 的內容
                            if def_start_pos < bytes.len() && def_end_pos <= bytes.len() {
                                let xml_slice: &[u8] = &bytes[def_start_pos..def_end_pos];
                                if let Ok(xml_content) = String::from_utf8(xml_slice.to_vec()) {
                                    entries.push(DefEntry {
                                        def_name: def_name.clone(),
                                        file_path: path.to_path_buf(),
                                        xml_content: format_xml(&xml_content),
                                        def_type: def_type.clone(),
                                    });
                                }
                            }
                        }
                        current_def_type = None;
                        current_def_name = None;
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

// 簡單格式化 XML 使其更易讀，保留縮排結構
fn format_xml(xml: &str) -> String {
    xml.lines()
        .map(|line| line.trim_end()) // 只移除行尾空白
        .filter(|line| !line.trim().is_empty()) // 過濾空行
        .collect::<Vec<_>>()
        .join("\n")
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
