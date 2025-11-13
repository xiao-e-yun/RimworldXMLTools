use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::HashSet;
use std::fs::File;
use std::io::BufReader;

pub fn extract_tag_values(
    path: &std::path::Path,
    tag_name: &str,
) -> Result<HashSet<String>, Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    let file = BufReader::new(file);
    let mut reader = Reader::from_reader(file);
    reader.config_mut().trim_text(true);

    let mut values = HashSet::new();
    let mut buf = Vec::new();
    let mut inside_target_tag = false;
    let mut inside_li = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name = e.name();
                if let Ok(tag) = std::str::from_utf8(name.as_ref()) {
                    if tag.to_lowercase() == tag_name.to_lowercase() {
                        inside_target_tag = true;
                    } else if tag == "li" && inside_target_tag {
                        inside_li = true;
                    }
                }
            }
            Ok(Event::Text(e)) => {
                if inside_li {
                    if let Ok(text) = e.unescape() {
                        let trimmed = text.trim();
                        if !trimmed.is_empty() {
                            values.insert(trimmed.to_string());
                        }
                    }
                } else if inside_target_tag {
                    // 處理沒有 <li> 的情況
                    if let Ok(text) = e.unescape() {
                        let trimmed = text.trim();
                        if !trimmed.is_empty() {
                            values.insert(trimmed.to_string());
                        }
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let name = e.name();
                if let Ok(tag) = std::str::from_utf8(name.as_ref()) {
                    if tag == tag_name {
                        inside_target_tag = false;
                    } else if tag == "li" {
                        inside_li = false;
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break, // 忽略解析錯誤
            _ => {}
        }
        buf.clear();
    }

    Ok(values)
}
