use eframe::egui;
use quick_xml::events::Event;
use quick_xml::Reader;
use rayon::prelude::*;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Default)]
pub struct InheritanceTab {
    base_directory: String,
    all_defs: HashMap<String, DefData>,    // 所有 Defs（包括 Abstract 和具體的）
    selected_def_name: String,
    search_query: String,
    is_loading: bool,
    status_message: String,
    expanded_xml: String,
    inheritance_chain: Vec<String>,
}

#[derive(Debug, Clone)]
struct DefData {
    def_name: String,        // defName 或 Name (for Abstract)
    parent_name: Option<String>,
    #[allow(dead_code)]
    file_path: PathBuf,
    #[allow(dead_code)]
    xml_content: String,
    #[allow(dead_code)]
    is_abstract: bool,
    def_type: String,        // ThingDef, RecipeDef, etc.
    raw_nodes: Vec<XmlNode>, // 原始 XML 節點結構
}

#[derive(Debug, Clone)]
struct XmlNode {
    tag: String,
    attributes: Vec<(String, String)>,
    children: Vec<XmlNode>,
    text: Option<String>,
}

impl InheritanceTab {
    pub fn ui(&mut self, ui: &mut egui::Ui, _ctx: &egui::Context) {
        // 頂部控制面板
        ui.horizontal(|ui| {
            ui.label("目錄:");
            ui.text_edit_singleline(&mut self.base_directory);

            if ui.button("📂 選擇目錄").clicked() {
                if let Some(path) = rfd::FileDialog::new().pick_folder() {
                    self.base_directory = path.display().to_string();
                    self.scan_all_defs();
                }
            }

            if ui.button("🔄 掃描 Defs").clicked() && !self.base_directory.is_empty() {
                self.scan_all_defs();
            }

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

        // 搜尋欄
        ui.horizontal(|ui| {
            ui.label("🔍 搜尋 DefName:");
            let response = ui.text_edit_singleline(&mut self.search_query);
            
            if response.changed() {
                self.selected_def_name = String::new();
                self.expanded_xml = String::new();
                self.inheritance_chain.clear();
            }
        });

        ui.separator();

        // 主要內容區域
        ui.horizontal_top(|ui| {
            // 左側: Def 列表
            ui.allocate_ui_with_layout(
                egui::vec2(250.0, ui.available_height()),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.heading("Def 列表");
                    ui.separator();

                    egui::ScrollArea::vertical()
                        .id_salt("def_list")
                        .auto_shrink([false; 2])
                        .show(ui, |ui| {
                            let filtered_defs: Vec<_> = self.all_defs
                                .keys()
                                .filter(|name| {
                                    self.search_query.is_empty() 
                                        || name.to_lowercase().contains(&self.search_query.to_lowercase())
                                })
                                .cloned()
                                .collect();

                            for def_name in filtered_defs {
                                let is_selected = &self.selected_def_name == &def_name;
                                if ui.selectable_label(is_selected, &def_name).clicked() {
                                    self.selected_def_name = def_name.clone();
                                    self.expand_inheritance();
                                }
                            }
                        });
                },
            );

            ui.separator();

            // 右側: 詳細資訊
            ui.allocate_ui_with_layout(
                egui::vec2(ui.available_width(), ui.available_height()),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    if !self.selected_def_name.is_empty() {

                        // 顯示繼承鏈
                        if !self.inheritance_chain.is_empty() {
                            ui.label("📜 繼承鏈:");
                            ui.horizontal_wrapped(|ui| {
                                for (i, name) in self.inheritance_chain.iter().enumerate() {
                                    if i > 0 {
                                        ui.label("→");
                                    }
                                    ui.label(name);
                                }
                            });
                            ui.separator();
                        }

                        // 顯示展開後的 XML
                        ui.horizontal(|ui| {
                            ui.label("📄 展開的 XML:");
                        
                            // 複製按鈕
                            if ui.button("📋 複製 XML").clicked() {
                                ui.output_mut(|o| o.copied_text = self.expanded_xml.clone());
                            }
                        });
                    
                        egui::ScrollArea::vertical()
                            .id_salt("expanded_xml")
                            .show(ui, |ui| {
                                ui.add(
                                    egui::TextEdit::multiline(&mut self.expanded_xml.as_str())
                                        .code_editor()
                                        .desired_width(f32::INFINITY)
                                        .desired_rows(30),
                                );
                            });
                    } else {
                        ui.label("請從左側選擇一個 Def");
                    }
                },
            );
        });
    }

    fn scan_all_defs(&mut self) {
        self.is_loading = true;
        self.status_message = "正在掃描 Defs...".to_string();
        self.all_defs.clear();
        self.selected_def_name.clear();
        self.expanded_xml.clear();
        self.inheritance_chain.clear();

        let base_path = PathBuf::from(&self.base_directory);

        // 尋找所有 XML 檔案
        let xml_files: Vec<PathBuf> = WalkDir::new(&base_path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path().is_file()
                    && e.path().extension().and_then(|s| s.to_str()) == Some("xml")
            })
            .map(|e| e.path().to_path_buf())
            .collect();

        self.status_message = format!("找到 {} 個 XML 檔案，正在解析...", xml_files.len());

        // 並行解析
        let parsed_defs: Vec<DefData> = xml_files
            .par_iter()
            .filter_map(|path| parse_def_data(path).ok())
            .flatten()
            .collect();

        // 存儲所有 Defs
        for def_data in parsed_defs {
            self.all_defs.insert(def_data.def_name.clone(), def_data);
        }

        self.status_message = format!(
            "掃描完成！找到 {} 個 Defs（包括抽象定義）",
            self.all_defs.len()
        );
        self.is_loading = false;
    }

    fn expand_inheritance(&mut self) {
        self.inheritance_chain.clear();
        self.expanded_xml.clear();

        if let Some(def_data) = self.all_defs.get(&self.selected_def_name) {
            // 建立繼承鏈
            let mut chain = vec![def_data.def_name.clone()];
            let mut current_parent = def_data.parent_name.clone();

            while let Some(parent_name) = current_parent {
                chain.push(parent_name.clone());
                if let Some(parent_def) = self.all_defs.get(&parent_name) {
                    current_parent = parent_def.parent_name.clone();
                } else {
                    break;
                }
            }

            chain.reverse();
            self.inheritance_chain = chain.clone();

            // 合併節點（從最頂層父類開始）
            let mut merged_nodes: BTreeMap<String, XmlNode> = BTreeMap::new();

            for ancestor_name in &chain {
                if let Some(ancestor) = self.all_defs.get(ancestor_name) {
                    for node in &ancestor.raw_nodes {
                        merge_node(&mut merged_nodes, node);
                    }
                }
            }

            // 生成展開的 XML
            self.expanded_xml = generate_expanded_xml(
                &self.selected_def_name,
                &def_data.def_type,
                &merged_nodes,
            );
        }
    }
}

// 合併節點：對於 <li> 標籤進行合併，其他標籤覆蓋
fn merge_node(merged: &mut BTreeMap<String, XmlNode>, node: &XmlNode) {
    let key = node.tag.clone();
    
    if merged.contains_key(&key) {
        // 已存在此標籤
        let existing = merged.get_mut(&key).unwrap();
        
        // 檢查是否包含 <li> 子節點
        let has_li_children = node.children.iter().any(|c| c.tag == "li");
        
        if has_li_children {
            // 合併 <li> 子節點
            for child in &node.children {
                if child.tag == "li" {
                    // 檢查是否已存在相同的 <li>（比較文本和屬性）
                    let child_text = child.text.as_ref().map(|s| s.as_str()).unwrap_or("");
                    let exists = existing.children.iter().any(|c| {
                        if c.tag != "li" {
                            return false;
                        }
                        let c_text = c.text.as_ref().map(|s| s.as_str()).unwrap_or("");
                        // 文本相同且屬性相同才算重複
                        c_text == child_text && c.attributes == child.attributes
                    });
                    if !exists {
                        existing.children.push(child.clone());
                    }
                } else {
                    // 非 <li> 子節點遞歸合併
                    let mut child_map: BTreeMap<String, XmlNode> = existing
                        .children
                        .iter()
                        .filter(|c| c.tag != "li")
                        .map(|c| (c.tag.clone(), c.clone()))
                        .collect();
                    
                    merge_node(&mut child_map, child);
                    
                    existing.children.retain(|c| c.tag == "li");
                    existing.children.extend(child_map.into_values());
                }
            }
        } else {
            // 完全覆蓋（包括 text 和子節點）
            *existing = node.clone();
        }
    } else {
        // 新標籤，直接插入
        merged.insert(key, node.clone());
    }
}

fn parse_def_data(path: &Path) -> Result<Vec<DefData>, Box<dyn std::error::Error>> {
    let content = fs::read_to_string(path)?;
    let mut reader = Reader::from_str(&content);
    reader.config_mut().trim_text(true);

    let mut results = Vec::new();
    let mut buf = Vec::new();
    let mut inside_defs = false;
    let mut def_depth = 0;
    let mut current_def_type: Option<String> = None;
    let mut current_def_name: Option<String> = None;
    let mut current_parent_name: Option<String> = None;
    let mut is_abstract = false;
    let mut node_stack: Vec<XmlNode> = Vec::new();
    let mut root_nodes: Vec<XmlNode> = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                
                if name == "Defs" {
                    inside_defs = true;
                } else if inside_defs && def_depth == 0 && name.ends_with("Def") {
                    // 新的 Def 開始
                    current_def_type = Some(name.clone());
                    def_depth = 1;
                    current_def_name = None;
                    current_parent_name = None;
                    is_abstract = false;
                    root_nodes.clear();
                    node_stack.clear();
                    
                    // 解析屬性
                    for attr in e.attributes().filter_map(|a| a.ok()) {
                        let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                        let value = String::from_utf8_lossy(&attr.value).to_string();
                        
                        if key == "Abstract" && value == "True" {
                            is_abstract = true;
                        } else if key == "ParentName" {
                            current_parent_name = Some(value.clone());
                        } else if key == "Name" {
                            current_def_name = Some(value.clone());
                        }
                    }
                } else if def_depth > 0 {
                    // Def 內的子節點
                    def_depth += 1;
                    
                    let mut attributes = Vec::new();
                    for attr in e.attributes().filter_map(|a| a.ok()) {
                        attributes.push((
                            String::from_utf8_lossy(attr.key.as_ref()).to_string(),
                            String::from_utf8_lossy(&attr.value).to_string(),
                        ));
                    }
                    
                    let node = XmlNode {
                        tag: name.clone(),
                        attributes,
                        children: Vec::new(),
                        text: None,
                    };
                    
                    node_stack.push(node);
                }
            }
            Ok(Event::Empty(ref e)) => {
                // 空標籤 <tag />
                if def_depth > 0 {
                    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    let mut attributes = Vec::new();
                    for attr in e.attributes().filter_map(|a| a.ok()) {
                        attributes.push((
                            String::from_utf8_lossy(attr.key.as_ref()).to_string(),
                            String::from_utf8_lossy(&attr.value).to_string(),
                        ));
                    }
                    
                    let node = XmlNode {
                        tag: name.clone(),
                        attributes,
                        children: Vec::new(),
                        text: None,
                    };
                    
                    if let Some(parent) = node_stack.last_mut() {
                        parent.children.push(node);
                    } else {
                        root_nodes.push(node);
                    }
                }
            }
            Ok(Event::Text(e)) => {
                if def_depth > 0 && !node_stack.is_empty() {
                    if let Ok(text) = e.unescape() {
                        let trimmed = text.trim();
                        if !trimmed.is_empty() {
                            let last = node_stack.last_mut().unwrap();
                            
                            // 特殊處理 defName
                            if last.tag == "defName" && current_def_name.is_none() {
                                current_def_name = Some(trimmed.to_string());
                            }
                            
                            last.text = Some(trimmed.to_string());
                        }
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                
                if def_depth > 0 && name.ends_with("Def") {
                    def_depth -= 1;
                    
                    if def_depth == 0 {
                        // Def 結束
                        if let Some(def_name) = &current_def_name {
                            results.push(DefData {
                                def_name: def_name.clone(),
                                parent_name: current_parent_name.clone(),
                                file_path: path.to_path_buf(),
                                xml_content: String::new(),
                                is_abstract,
                                def_type: current_def_type.clone().unwrap_or_default(),
                                raw_nodes: root_nodes.clone(),
                            });
                        }
                    }
                } else if def_depth > 0 {
                    def_depth -= 1;
                    
                    // 彈出完成的節點
                    if let Some(completed_node) = node_stack.pop() {
                        if let Some(parent) = node_stack.last_mut() {
                            parent.children.push(completed_node);
                        } else {
                            root_nodes.push(completed_node);
                        }
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

    Ok(results)
}

fn generate_expanded_xml(
    def_name: &str,
    def_type: &str,
    nodes: &BTreeMap<String, XmlNode>,
) -> String {
    let mut xml = String::new();
    
    xml.push_str(&format!("<{}>\n", def_type));
    xml.push_str(&format!("  <defName>{}</defName>\n", def_name));
    
    // 生成所有其他節點
    for (_, node) in nodes {
        if node.tag != "defName" {
            generate_node_xml(&mut xml, node, 1);
        }
    }
    
    xml.push_str(&format!("</{}>\n", def_type));
    xml
}

fn generate_node_xml(xml: &mut String, node: &XmlNode, indent_level: usize) {
    let indent = "  ".repeat(indent_level);
    
    // 檢查是否是簡單節點（只有文本，無子節點）
    let is_simple = node.children.is_empty() && node.text.is_some();
    let is_empty = node.children.is_empty() && node.text.is_none();
    
    if is_simple {
        // 簡單節點：單行輸出
        let text = node.text.as_ref().unwrap();
        if node.attributes.is_empty() {
            xml.push_str(&format!("{}<{}>{}</{}>\n", indent, node.tag, text, node.tag));
        } else {
            xml.push_str(&format!("{}<{}", indent, node.tag));
            for (key, value) in &node.attributes {
                xml.push_str(&format!(" {}=\"{}\"", key, value));
            }
            xml.push_str(&format!(">{}</{}>\n", text, node.tag));
        }
    } else if is_empty {
        // 空節點：自閉合標籤
        if node.attributes.is_empty() {
            xml.push_str(&format!("{}<{} />\n", indent, node.tag));
        } else {
            xml.push_str(&format!("{}<{}", indent, node.tag));
            for (key, value) in &node.attributes {
                xml.push_str(&format!(" {}=\"{}\"", key, value));
            }
            xml.push_str(" />\n");
        }
    } else {
        // 複雜節點：多行輸出
        // 開標籤
        if node.attributes.is_empty() {
            xml.push_str(&format!("{}<{}>\n", indent, node.tag));
        } else {
            xml.push_str(&format!("{}<{}", indent, node.tag));
            for (key, value) in &node.attributes {
                xml.push_str(&format!(" {}=\"{}\"", key, value));
            }
            xml.push_str(">\n");
        }
        
        // 文本內容（如果有的話，在有子節點的情況下較少見）
        if let Some(text) = &node.text {
            xml.push_str(&format!("{}  {}\n", indent, text));
        }
        
        // 子節點
        for child in &node.children {
            if child.tag == "li" && child.children.is_empty() {
                // <li> 標籤特殊處理：總是單行
                if let Some(text) = &child.text {
                    // 有文本內容
                    if child.attributes.is_empty() {
                        xml.push_str(&format!("{}  <li>{}</li>\n", indent, text));
                    } else {
                        xml.push_str(&format!("{}  <li", indent));
                        for (key, value) in &child.attributes {
                            xml.push_str(&format!(" {}=\"{}\"", key, value));
                        }
                        xml.push_str(&format!(">{}</li>\n", text));
                    }
                } else {
                    // 空 <li> 標籤
                    if child.attributes.is_empty() {
                        xml.push_str(&format!("{}  <li />\n", indent));
                    } else {
                        xml.push_str(&format!("{}  <li", indent));
                        for (key, value) in &child.attributes {
                            xml.push_str(&format!(" {}=\"{}\"", key, value));
                        }
                        xml.push_str(" />\n");
                    }
                }
            } else {
                generate_node_xml(xml, child, indent_level + 1);
            }
        }
        
        // 閉標籤
        xml.push_str(&format!("{}</{}>\n", indent, node.tag));
    }
}
