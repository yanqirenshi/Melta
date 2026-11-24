//! 版を作る（new）と、要素の編集（set-text / insert / split / delete）、ID の採番。

use crate::edition::*;
use serde_json::{json, Map, Value};
use std::fs;
use std::path::Path;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

fn require_editable(name: &str) -> Res<()> {
    if name == ORIGINAL {
        return Err("original は書き換えられません（原本です）。melta new で版を作って、その版を編集してください".to_string());
    }
    Ok(())
}

fn is_sentence(v: &Value) -> bool {
    v.get("type").is_none() && v.get("id").map_or(false, |x| x.is_string()) && v.get("text").map_or(false, |x| x.is_string())
}

// ---------------------------------------------------------------- new

fn valid_editor(name: &str) -> bool {
    !name.is_empty()
        && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && name.chars().any(|c| c != '-')
}

/// 元の版から持ち越した `origin` と `note` を消す。消したら true。
fn strip_origin(v: &mut Value) -> bool {
    let mut changed = false;
    match v {
        Value::Object(m) => {
            if m.contains_key("origin") {
                m.remove("origin");
                m.remove("note");
                m.remove("notes");
                changed = true;
            }
            for x in m.values_mut() {
                changed |= strip_origin(x);
            }
        }
        Value::Array(a) => {
            for x in a {
                changed |= strip_origin(x);
            }
        }
        _ => {}
    }
    changed
}

fn next_serial(out: &Path) -> Res<u64> {
    let mut max = 0;
    for name in edition_names(out)? {
        if let Some((head, _)) = name.split_once('_') {
            if let Ok(n) = head.parse::<u64>() {
                max = max.max(n);
            }
        }
    }
    Ok(max + 1)
}

/// 元の版を全部コピーして、新しい版を作る。版の名前を返す。
pub fn new_edition(out: &Path, from: &str, editor: &str) -> Res<String> {
    if !valid_editor(editor) {
        return Err(format!("編集者名 {editor:?} は使えません（英小文字・数字・ハイフンだけにしてください）"));
    }
    let src = edition_dir(out, from)?;
    let index_path = src.join(INDEX_FILE);
    if !index_path.is_file() {
        return Err(format!("{} がありません（元の版 {from} が structure/ の下にありません）", index_path.display()));
    }
    let (mut index, index_crlf) = read_json_eol(&index_path)?;
    if from != ORIGINAL || index.get("edition").is_some() {
        if let Some(first) = edition_problems(index.get("edition")).first() {
            return Err(format!("元の版 {from} の {first}（版を作れません）"));
        }
    }
    let files = json_files(&src)?;

    let name = format!("{:02}_{editor}", next_serial(out)?);
    let dst = edition_dir(out, &name)?;
    fs::create_dir(&dst).map_err(|e| format!("{} を作れません: {e}", dst.display()))?;

    for file in &files {
        let bytes = fs::read(src.join(file)).map_err(|e| format!("{file} を読めません: {e}"))?;
        let mut value: Value = serde_json::from_slice(&bytes).map_err(|e| format!("{file} が JSON として読めません: {e}"))?;
        let crlf = bytes.windows(2).any(|w| w == b"\r\n");
        let data = if strip_origin(&mut value) { with_eol(format!("{}\n", dumps(&value, 0, false)), crlf).into_bytes() } else { bytes };
        fs::write(dst.join(file), data).map_err(|e| format!("{file} を書けません: {e}"))?;
    }

    let retired = index
        .get("edition")
        .and_then(|e| e.get("retired_ids"))
        .cloned()
        .filter(|r| r.is_array())
        .unwrap_or_else(|| json!([]));
    let now = OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc());
    let created_at = now.replace_nanosecond(0).unwrap().format(&Rfc3339).map_err(|e| e.to_string())?;
    index["edition"] = json!({
        "name": name,
        "base": from,
        "editor": editor,
        "created_at": created_at,
        "retired_ids": retired,
    });
    fs::write(dst.join(INDEX_FILE), with_eol(format!("{}\n", dumps(&index, 0, true)), index_crlf)).map_err(|e| format!("index.json を書けません: {e}"))?;
    Ok(name)
}

// ---------------------------------------------------------------- 採番

/// 入れ物（パラグラフや item）に、足す文の番号を決める。今ある ID と retired_ids の、番号の最大 + 1。
fn next_number(container_id: &str, container: &Value, retired: &[String]) -> u64 {
    let prefix = format!("{container_id}.");
    let mut max = 0;
    let existing = container
        .get("sentences")
        .and_then(|s| s.as_array())
        .map(|a| a.iter().filter_map(|s| str_of(s, "id")).collect::<Vec<_>>())
        .unwrap_or_default();
    for id in existing.into_iter().chain(retired.iter().map(|s| s.as_str())) {
        if let Some(rest) = id.strip_prefix(&prefix) {
            if !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()) {
                if let Ok(n) = rest.parse::<u64>() {
                    max = max.max(n);
                }
            }
        }
    }
    max + 1
}

fn with_note(mut obj: Map<String, Value>, note: Option<&str>) -> Value {
    if let Some(n) = note {
        obj.insert("note".to_string(), Value::from(n));
    }
    Value::Object(obj)
}

// ---------------------------------------------------------------- set-text

/// `text` を持つ要素の本文を直す。`origin`（直す前）と `note`（理由）を足す。
/// 同じ版で 2 回直したら `origin` は最初のまま。base の文に戻ったら `origin` と `note` を消す。
fn apply_text(target: &mut Value, elem_id: &str, base: &str, new_text: Value, note: Option<&str>) -> bool {
    let obj = target.as_object_mut().unwrap();
    let current = obj.get("text").cloned().unwrap_or(Value::Null);
    match obj.get("origin").and_then(|o| o.get("text")).cloned() {
        Some(orig_text) => {
            if orig_text == new_text {
                obj.insert("text".to_string(), new_text);
                obj.remove("origin");
                obj.remove("note");
                return true;
            }
        }
        None => {
            if obj.contains_key("origin") {
                // この版で足した文。origin はそのまま
            } else {
                if current == new_text {
                    return false;
                }
                obj.insert("origin".to_string(), json!({"edition": base, "ids": [elem_id], "text": current}));
            }
        }
    }
    obj.insert("text".to_string(), new_text);
    if let Some(n) = note {
        obj.insert("note".to_string(), Value::from(n));
    }
    true
}

pub fn parse_cell(s: &str) -> Res<(u64, u64)> {
    let (r, c) = s.split_once(',').ok_or_else(|| format!("--cell は <row>,<col> の形で指定してください（{s:?}）"))?;
    let r = r.trim().parse::<u64>().map_err(|_| format!("--cell の row が数ではありません: {r:?}"))?;
    let c = c.trim().parse::<u64>().map_err(|_| format!("--cell の col が数ではありません: {c:?}"))?;
    Ok((r, c))
}

pub fn set_text(out: &Path, edition: &str, id: &str, text: &str, note: Option<&str>, cell: Option<&str>, index: Option<usize>) -> Res<String> {
    if cell.is_some() && index.is_some() {
        return Err("--cell と --index は同時に指定できません".to_string());
    }
    require_editable(edition)?;
    let mut ed = Edition::load(out, edition)?;
    let base = ed.base()?;
    let (pi, ptr) = ed.find_id(id).ok_or_else(|| format!("ID {id} の要素が見つかりません"))?;
    let node = ed.parts[pi].value.pointer_mut(&ptr).unwrap();
    let kind = str_of(node, "type").map(|s| s.to_string());

    let changed = match kind.as_deref() {
        None if is_sentence(node) => {
            if cell.is_some() || index.is_some() {
                return Err("センテンスには --cell と --index は使えません".to_string());
            }
            apply_text(node, id, &base, Value::from(text), note)
        }
        Some("caption") => {
            if cell.is_some() || index.is_some() {
                return Err("キャプションには --cell と --index は使えません".to_string());
            }
            apply_text(node, id, &base, Value::from(text), note)
        }
        Some("figure") => {
            if cell.is_some() {
                return Err("図には --cell は使えません（--index で、図の中の文字の何番目か（0 から）を指します）".to_string());
            }
            let arr = node.get("text").and_then(|t| t.as_array()).cloned().unwrap_or_default();
            let i = index.ok_or_else(|| format!("図 {id} の文字を直すには --index <n>（0 から）が要ります（文字は {} 個）", arr.len()))?;
            if i >= arr.len() {
                return Err(format!("--index {i} は範囲外です（図 {id} の文字は {} 個）", arr.len()));
            }
            let mut new_arr = arr;
            new_arr[i] = Value::from(text);
            apply_figure(node, id, &base, i, new_arr, note)
        }
        Some("table") => {
            let (r, c) = parse_cell(cell.ok_or("表のセルを直すには --cell <row>,<col> が要ります")?)?;
            let cells = node.get_mut("cells").and_then(|c| c.as_array_mut()).ok_or("表に cells がありません")?;
            let target = cells
                .iter_mut()
                .find(|x| x.get("row").and_then(|v| v.as_u64()) == Some(r) && x.get("col").and_then(|v| v.as_u64()) == Some(c))
                .ok_or_else(|| format!("表 {id} に、行 {r}・列 {c} のセルがありません"))?;
            apply_text(target, id, &base, Value::from(text), note)
        }
        Some("section") => return Err(format!("{id} は見出しです。見出しの文字は、今回の道具では直せません")),
        _ => return Err(format!("{id} は、set-text で直せる要素ではありません（センテンス・キャプション・図・表のセルが対象です）")),
    };
    if changed {
        ed.parts[pi].dirty = true;
        ed.save()?;
    }
    Ok(id.to_string())
}

// ---------------------------------------------------------------- insert / split / delete

fn split_ptr(ptr: &str) -> Res<(String, usize)> {
    let (head, idx) = ptr.rsplit_once('/').ok_or("内部エラー: ポインタが不正です")?;
    let (container, seg) = head.rsplit_once('/').ok_or("内部エラー: ポインタが不正です")?;
    if seg != "sentences" {
        return Err("センテンスの ID を指定してください".to_string());
    }
    Ok((container.to_string(), idx.parse().map_err(|_| "内部エラー: 添字が不正です")?))
}

pub fn insert(out: &Path, edition: &str, after: Option<&str>, first_in: Option<&str>, text: &str, note: Option<&str>) -> Res<String> {
    require_editable(edition)?;
    let mut ed = Edition::load(out, edition)?;
    let base = ed.base()?;
    let retired = ed.retired_ids();

    let (pi, cptr, pos) = match (after, first_in) {
        (Some(id), None) => {
            let (pi, ptr) = ed.find_id(id).ok_or_else(|| format!("ID {id} の要素が見つかりません"))?;
            if !is_sentence(ed.parts[pi].value.pointer(&ptr).unwrap()) {
                return Err(format!("--after には、センテンスの ID を指定してください（{id}）"));
            }
            let (cptr, i) = split_ptr(&ptr)?;
            (pi, cptr, i + 1)
        }
        (None, Some(id)) => {
            let (pi, ptr) = ed.find_id(id).ok_or_else(|| format!("ID {id} の要素が見つかりません"))?;
            if ed.parts[pi].value.pointer(&ptr).unwrap().get("sentences").is_none() {
                return Err(format!("--first-in には、パラグラフか箇条書きの項目の ID を指定してください（{id}）"));
            }
            (pi, ptr, 0)
        }
        _ => return Err("--after か --first-in のどちらか 1 つを指定してください".to_string()),
    };

    let container = ed.parts[pi].value.pointer_mut(&cptr).unwrap();
    let cid = str_of(container, "id").ok_or("入れ物に ID がありません")?.to_string();
    let new_id = format!("{cid}.{}", next_number(&cid, container, &retired));
    let mut obj = Map::new();
    obj.insert("id".into(), Value::from(new_id.clone()));
    obj.insert("text".into(), Value::from(text));
    obj.insert("origin".into(), json!({"edition": base, "ids": []}));
    container.get_mut("sentences").and_then(|s| s.as_array_mut()).ok_or("入れ物に sentences がありません")?.insert(pos, with_note(obj, note));
    ed.parts[pi].dirty = true;
    ed.save()?;
    Ok(new_id)
}

pub fn split(out: &Path, edition: &str, id: &str, texts: &[String], note: Option<&str>) -> Res<Vec<String>> {
    require_editable(edition)?;
    if texts.len() < 2 {
        return Err("split には --text を 2 つ以上指定してください".to_string());
    }
    let mut ed = Edition::load(out, edition)?;
    let base = ed.base()?;
    let retired = ed.retired_ids();
    let (pi, ptr) = ed.find_id(id).ok_or_else(|| format!("ID {id} の要素が見つかりません"))?;
    let (cptr, pos) = split_ptr(&ptr)?;
    let container = ed.parts[pi].value.pointer_mut(&cptr).unwrap();
    let cid = str_of(container, "id").ok_or("入れ物に ID がありません")?.to_string();
    let mut number = next_number(&cid, container, &retired);

    let sentences = container.get_mut("sentences").and_then(|s| s.as_array_mut()).ok_or("入れ物に sentences がありません")?;
    let old = sentences[pos].clone();
    let origin = old
        .get("origin")
        .cloned()
        .unwrap_or_else(|| json!({"edition": base, "ids": [id], "text": old.get("text").cloned().unwrap_or(Value::Null)}));
    let note = note.map(|s| s.to_string()).or_else(|| str_of(&old, "note").map(|s| s.to_string()));

    let mut ids = Vec::new();
    let mut parts = Vec::new();
    for (k, t) in texts.iter().enumerate() {
        let new_id = if k == 0 {
            id.to_string()
        } else {
            let s = format!("{cid}.{number}");
            number += 1;
            s
        };
        let mut obj = Map::new();
        obj.insert("id".into(), Value::from(new_id.clone()));
        obj.insert("text".into(), Value::from(t.as_str()));
        obj.insert("origin".into(), origin.clone());
        parts.push(with_note(obj, note.as_deref()));
        ids.push(new_id);
    }
    sentences.splice(pos..pos + 1, parts);
    ed.parts[pi].dirty = true;
    ed.save()?;
    Ok(ids)
}

pub fn delete(out: &Path, edition: &str, id: &str, note: Option<&str>) -> Res<String> {
    require_editable(edition)?;
    let mut ed = Edition::load(out, edition)?;
    ed.base()?;
    let (pi, ptr) = ed.find_id(id).ok_or_else(|| format!("ID {id} の要素が見つかりません"))?;
    let (cptr, pos) = split_ptr(&ptr)?;
    let container = ed.parts[pi].value.pointer_mut(&cptr).unwrap();
    let sentences = container.get_mut("sentences").and_then(|s| s.as_array_mut()).ok_or("入れ物に sentences がありません")?;
    if sentences.len() < 2 {
        return Err(format!("{id} は、入れ物の最後の 1 文なので消せません"));
    }
    sentences.remove(pos);
    ed.parts[pi].dirty = true;

    let e = ed.index.get_mut("edition").and_then(|e| e.as_object_mut()).unwrap();
    let r = e.entry("retired_ids").or_insert_with(|| json!([]));
    if let Some(a) = r.as_array_mut() {
        if !a.iter().any(|x| x.as_str() == Some(id)) {
            a.push(Value::from(id));
        }
    }
    if let Some(n) = note {
        let notes = e.entry("retired_notes").or_insert_with(|| json!({}));
        notes[id] = Value::from(n);
    }
    ed.index_dirty = true;
    ed.save()?;
    Ok(id.to_string())
}

/// 図の中の文字（`text` の配列）の i 番目を直した結果 `new_arr` を書く。
/// 理由は、直した index ごとに `notes`（キーは index の文字列）に持つ。
fn apply_figure(node: &mut Value, id: &str, base: &str, i: usize, new_arr: Vec<Value>, note: Option<&str>) -> bool {
    let obj = node.as_object_mut().unwrap();
    let current = obj.get("text").cloned().unwrap_or(Value::Null);
    let new_val = Value::Array(new_arr.clone());
    let orig_text = obj.get("origin").and_then(|o| o.get("text")).cloned();
    let base_text = orig_text.clone().unwrap_or_else(|| current.clone());
    if orig_text.is_none() && current == new_val {
        return false;
    }
    if base_text == new_val {
        obj.insert("text".to_string(), new_val);
        obj.remove("origin");
        obj.remove("notes");
        return true;
    }
    if !obj.contains_key("origin") {
        obj.insert("origin".to_string(), json!({"edition": base, "ids": [id], "text": current}));
    }
    obj.insert("text".to_string(), new_val);
    let key = i.to_string();
    let same = base_text.as_array().and_then(|a| a.get(i)) == Some(&new_arr[i]);
    if same {
        if let Some(n) = obj.get_mut("notes").and_then(|n| n.as_object_mut()) {
            n.remove(&key);
        }
    } else if let Some(n) = note {
        let notes = obj.entry("notes").or_insert_with(|| json!({}));
        if let Some(m) = notes.as_object_mut() {
            m.insert(key, Value::from(n));
        }
    }
    if obj.get("notes").and_then(|n| n.as_object()).map_or(false, |n| n.is_empty()) {
        obj.remove("notes");
    }
    true
}

/// 表のセルを（row, col）で探す。結合で隠れたマスにはセルが無いので、見つからない。
pub fn find_cell(table: &Value, r: u64, c: u64) -> Option<&Value> {
    table
        .get("cells")?
        .as_array()?
        .iter()
        .find(|x| x.get("row").and_then(|v| v.as_u64()) == Some(r) && x.get("col").and_then(|v| v.as_u64()) == Some(c))
}
