#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod xml_parser;
mod browser;
mod finder;
mod inheritance;

use eframe::egui;
use finder::TagFinderTab;
use browser::DefBrowserTab;
use inheritance::InheritanceTab;

fn main() -> eframe::Result {
    // 載入圖標
    let icon_data = load_icon();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.0, 600.0])
            .with_title("RimWorld XML Tools")
            .with_icon(icon_data.unwrap_or_default()),
        ..Default::default()
    };

    eframe::run_native(
        "RimWorld XML Tools",
        options,
        Box::new(|cc| {
            // 設置中文字體
            setup_custom_fonts(&cc.egui_ctx);
            Ok(Box::new(XmlToolsApp::default()))
        }),
    )
}

fn load_icon() -> Option<egui::IconData> {
    // 從嵌入的資源載入圖標
    let png_bytes = include_bytes!("../assets/icon.png");

    eframe::icon_data::from_png_bytes(png_bytes).ok()
}

fn setup_custom_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    // 添加 Windows 系統中文字體
    // 嘗試載入微軟正黑體或其他中文字體
    if let Ok(font_data) = std::fs::read("C:\\Windows\\Fonts\\msjh.ttc") {
        fonts.font_data.insert(
            "microsoft_jhenghei".to_owned(),
            egui::FontData::from_owned(font_data).tweak(egui::FontTweak {
                scale: 1.0,
                y_offset_factor: 0.0,
                y_offset: 0.0,
                baseline_offset_factor: 0.0,
            }),
        );

        // 設置字體優先順序
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .insert(0, "microsoft_jhenghei".to_owned());

        fonts
            .families
            .entry(egui::FontFamily::Monospace)
            .or_default()
            .push("microsoft_jhenghei".to_owned());
    } else if let Ok(font_data) = std::fs::read("C:\\Windows\\Fonts\\msyh.ttc") {
        // 備用: 微軟雅黑體
        fonts.font_data.insert(
            "microsoft_yahei".to_owned(),
            egui::FontData::from_owned(font_data).tweak(egui::FontTweak {
                scale: 1.0,
                y_offset_factor: 0.0,
                y_offset: 0.0,
                baseline_offset_factor: 0.0,
            }),
        );

        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .insert(0, "microsoft_yahei".to_owned());

        fonts
            .families
            .entry(egui::FontFamily::Monospace)
            .or_default()
            .push("microsoft_yahei".to_owned());
    }

    ctx.set_fonts(fonts);
}

#[derive(Default)]
struct XmlToolsApp {
    finder: TagFinderTab,
    browser: DefBrowserTab,
    inheritance: InheritanceTab,
    active_tab: usize,
}

impl eframe::App for XmlToolsApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.selectable_value(&mut self.active_tab, 0, "📚 Def 瀏覽器");
                ui.selectable_value(&mut self.active_tab, 1, "🔗 展開繼承");
                ui.selectable_value(&mut self.active_tab, 2, "🔍 標籤查找器");
                // 未來可以添加更多分頁
                // ui.selectable_value(&mut self.active_tab, 3, "📊 統計分析");
                // ui.selectable_value(&mut self.active_tab, 4, "🔧 工具箱");
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            match self.active_tab {
                0 => self.browser.ui(ui, ctx),
                1 => self.inheritance.ui(ui, ctx),
                2 => self.finder.ui(ui, ctx),
                // 未來可以添加更多分頁處理
                // 3 => self.statistics.ui(ui, ctx),
                // 4 => self.toolbox.ui(ui, ctx),
                _ => {
                    ui.heading("未實現的功能");
                }
            }
        });
    }
}
