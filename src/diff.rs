//! 版どうしの差分。ID で対応を取り、変わった・足された・消えた要素を Markdown にする。

use crate::edition::*;
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

struct Item {
    key: String,
    kind: &'static str,
    text: Value,
    note: Option<String>,
    origin_ids: Vec<String>,
}

fn collect(ed: &Edition) -> Vec<Item> {
    fn push(items: &mut Vec<Item>, key: String, kind: &'static str, o: &Value, text: Value) {
        let origin_ids = o
            .get("origin")
            .and_then(|x| x.get("ids"))
            .and_then(|x| x.as_array())
            .map(|a| a.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default();
        items.push(Item { key, kind, text, note: str_of(o, "note").map(|s| s.to_string()), origin_ids });
    }
    fn sentences(items: &mut Vec<Item>, c: &Value) {
        for s in c.get("sentences").and_then(|s| s.as_array()).into_iter().flatten() {
            if let (Some(id), Some(t)) = (str_of(s, "id"), s.get("text")) {
                push(items, id.to_string(), "センテンス", s, t.clone());
            }
        }
    }
    fn node(items: &mut Vec<Item>, n: &Value) {
        let id = str_of(n, "id").unwrap_or("?").to_string();
        match str_of(n, "type") {
            Some("section") => {
                push(items, id, "見出し", n, n.get("title").cloned().unwrap_or(Value::Null));
                for c in n.get("children").and_then(|c| c.as_array()).into_iter().flatten() {
                    node(items, c);
                }
            }
            Some("paragraph") => sentences(items, n),
            Some("list") => {
                for it in n.get("items").and_then(|i| i.as_array()).into_iter().flatten() {
                    sentences(items, it);
                }
            }
            Some("caption") => push(items, id, "キャプション", n, n.get("text").cloned().unwrap_or(Value::Null)),
            Some("figure") => push(items, id, "図の文字", n, n.get("text").cloned().unwrap_or(Value::Null)),
            Some("table") => {
                for c in n.get("cells").and_then(|c| c.as_array()).into_iter().flatten() {
                    let r = c.get("row").and_then(|x| x.as_i64()).unwrap_or(-1);
                    let col = c.get("col").and_then(|x| x.as_i64()).unwrap_or(-1);
                    push(items, format!("{id}[{r},{col}]"), "表のセル", c, c.get("text").cloned().unwrap_or(Value::Null));
                }
            }
            _ => {}
        }
    }
    let mut items = Vec::new();
    for p in &ed.parts {
        for n in p.value.get("body").and_then(|b| b.as_array()).into_iter().flatten() {
            node(&mut items, n);
        }
    }
    items
}

fn show(v: &Value) -> String {
    let s = match v {
        Value::String(s) => s.clone(),
        Value::Array(a) => a.iter().map(|x| x.as_str().unwrap_or("").to_string()).collect::<Vec<_>>().join(" / "),
        other => other.to_string(),
    };
    s.replace('\r', "\\r").replace('\n', "\\n")
}

fn heading(out: &mut String, item: &Item) {
    out.push_str(&format!("### {}（{}）\n\n", item.key, item.kind));
}

pub fn run(out: &Path, a: &str, b: &str) -> Res<String> {
    let ea = Edition::load(out, a)?;
    let eb = Edition::load(out, b)?;
    let ia = collect(&ea);
    let ib = collect(&eb);
    let ma: HashMap<&str, &Item> = ia.iter().map(|i| (i.key.as_str(), i)).collect();
    let mb: HashMap<&str, &Item> = ib.iter().map(|i| (i.key.as_str(), i)).collect();

    let changed: Vec<(&Item, &Item)> = ib.iter().filter_map(|y| ma.get(y.key.as_str()).filter(|x| x.text != y.text).map(|x| (*x, y))).collect();
    let added: Vec<&Item> = ib.iter().filter(|y| !ma.contains_key(y.key.as_str())).collect();
    let removed: Vec<&Item> = ia.iter().filter(|x| !mb.contains_key(x.key.as_str())).collect();

    if changed.is_empty() && added.is_empty() && removed.is_empty() {
        return Ok(format!("{a} と {b} に、差分はありません\n"));
    }
    let retired_notes = eb.index.get("edition").and_then(|e| e.get("retired_notes"));

    let mut s = format!("# 差分: {a} → {b}\n\n変更 {} 件、追加 {} 件、削除 {} 件\n\n", changed.len(), added.len(), removed.len());
    if !changed.is_empty() {
        s.push_str(&format!("## 変更（{} 件）\n\n", changed.len()));
        for (x, y) in &changed {
            heading(&mut s, y);
            s.push_str(&format!("- 直す前: {}\n- 直した後: {}\n", show(&x.text), show(&y.text)));
            if let Some(n) = &y.note {
                s.push_str(&format!("- 理由: {n}\n"));
            }
            s.push('\n');
        }
    }
    if !added.is_empty() {
        s.push_str(&format!("## 追加（{} 件）\n\n", added.len()));
        for y in &added {
            heading(&mut s, y);
            s.push_str(&format!("- 追加した文: {}\n", show(&y.text)));
            if !y.origin_ids.is_empty() {
                s.push_str(&format!("- 分けた元: {}\n", y.origin_ids.join(", ")));
            }
            if let Some(n) = &y.note {
                s.push_str(&format!("- 理由: {n}\n"));
            }
            s.push('\n');
        }
    }
    if !removed.is_empty() {
        s.push_str(&format!("## 削除（{} 件）\n\n", removed.len()));
        for x in &removed {
            heading(&mut s, x);
            s.push_str(&format!("- 消した文: {}\n", show(&x.text)));
            if let Some(n) = retired_notes.and_then(|r| str_of(r, &x.key)) {
                s.push_str(&format!("- 理由: {n}\n"));
            }
            s.push('\n');
        }
    }
    Ok(s)
}
