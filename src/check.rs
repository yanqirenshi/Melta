//! 版の形式の確認。問題を 1 行ずつ返す。

use crate::edition::*;
use serde_json::Value;
use std::collections::{BTreeSet, HashMap};
use std::path::Path;

// スキーマは Hefti（@yanqirenshi/hefti）から取り込んだもの。直接は書き換えず、`scripts/sync-schema.sh` で更新する。
// 取り込んだ版は `schema/HEFTI_VERSION`。
const INDEX_SCHEMA: &str = include_str!("../schema/structure-index.schema.json");
const PART_SCHEMA: &str = include_str!("../schema/structure-part.schema.json");

fn validator(src: &str) -> Res<jsonschema::Validator> {
    let schema: Value = serde_json::from_str(src).map_err(|e| e.to_string())?;
    jsonschema::validator_for(&schema).map_err(|e| format!("スキーマを読めません: {e}"))
}

fn validate(v: &jsonschema::Validator, instance: &Value, label: &str, problems: &mut Vec<String>) {
    for e in v.iter_errors(instance) {
        problems.push(format!("[スキーマ] {label}: {}: {e}", e.instance_path()));
    }
}

/// 版を確かめる。返すのは問題の一覧（空なら問題なし）。
pub fn run(out: &Path, name: &str) -> Res<Vec<String>> {
    let dir = edition_dir(out, name)?;
    if !dir.join(INDEX_FILE).is_file() {
        return Err(format!("{} がありません（版 {name} が structure/ の下にありません）", dir.join(INDEX_FILE).display()));
    }
    let mut problems: Vec<String> = Vec::new();
    let index = match read_json(&dir.join(INDEX_FILE)) {
        Ok(v) => v,
        Err(e) => return Ok(vec![format!("[JSON] {e}")]),
    };
    validate(&validator(INDEX_SCHEMA)?, &index, INDEX_FILE, &mut problems);
    let part_validator = validator(PART_SCHEMA)?;

    // --- 4. files と実際のファイル
    let actual: BTreeSet<String> = json_files(&dir)?.into_iter().collect();
    let mut listed: Vec<(String, Option<String>)> = Vec::new(); // (file, id)
    if let Some(files) = index.get("files").and_then(|f| f.as_array()) {
        for f in files {
            if let Some(file) = str_of(f, "file") {
                if listed.iter().any(|(n, _)| n == file) {
                    problems.push(format!("[files] index.json の files に {file} が 2 回あります"));
                }
                listed.push((file.to_string(), str_of(f, "id").map(|s| s.to_string())));
            }
        }
    }
    for (file, _) in &listed {
        if !actual.contains(file) {
            problems.push(format!("[files] index.json の files にある {file} が、フォルダにありません"));
        }
    }
    for file in &actual {
        if !listed.iter().any(|(n, _)| n == file) {
            problems.push(format!("[files] {file} が、index.json の files にありません"));
        }
    }

    // --- 読み込みとスキーマ
    let mut parts: Vec<(String, Value)> = Vec::new();
    for file in &actual {
        match read_json(&dir.join(file)) {
            Ok(v) => {
                validate(&part_validator, &v, file, &mut problems);
                if str_of(&v, "file") != Some(file.as_str()) {
                    problems.push(format!("[files] {file} の file プロパティが、ファイル名と合っていません"));
                }
                if let Some((_, lid)) = listed.iter().find(|(n, _)| n == file) {
                    if v.get("id").and_then(|x| x.as_str()) != lid.as_deref() {
                        problems.push(format!("[files] {file} の id が、index.json の files の id と合っていません"));
                    }
                }
                parts.push((file.clone(), v));
            }
            Err(e) => problems.push(format!("[JSON] {e}")),
        }
    }

    // --- 2. ID の重複、セクションの一覧
    let mut seen: HashMap<String, String> = HashMap::new();
    let mut sections: HashMap<String, (String, i64, String)> = HashMap::new(); // id → (file, level, title)
    for (file, part) in &parts {
        let Some(body) = part.get("body") else { continue };
        walk_objects(body, &mut |o| {
            if str_of(o, "type") == Some("section_ref") {
                return;
            }
            if let Some(id) = str_of(o, "id") {
                if let Some(prev) = seen.get(id) {
                    problems.push(format!("[ID] {id} が重複しています（{prev} と {file}）"));
                } else {
                    seen.insert(id.to_string(), file.clone());
                }
                if str_of(o, "type") == Some("section") {
                    sections.insert(
                        id.to_string(),
                        (file.clone(), o.get("level").and_then(|l| l.as_i64()).unwrap_or(0), str_of(o, "title").unwrap_or("").to_string()),
                    );
                }
            }
        });
    }

    // --- 図の notes（index ごとの理由）
    for (file, part) in &parts {
        let Some(body) = part.get("body") else { continue };
        walk_objects(body, &mut |o| {
            if str_of(o, "type") != Some("figure") {
                return;
            }
            let Some(notes) = o.get("notes") else { return };
            let id = str_of(o, "id").unwrap_or("?");
            let len = o.get("text").and_then(|t| t.as_array()).map_or(0, |a| a.len());
            match notes.as_object() {
                None => problems.push(format!("[notes] {file} の図 {id} の notes がオブジェクトではありません")),
                Some(m) => {
                    for (k, v) in m {
                        if !k.parse::<usize>().map_or(false, |i| i < len) {
                            problems.push(format!("[notes] {file} の図 {id} の notes のキー {k:?} が、文字の範囲内の index ではありません"));
                        }
                        if !v.is_string() {
                            problems.push(format!("[notes] {file} の図 {id} の notes[{k}] が文字列ではありません"));
                        }
                    }
                }
            }
        });
    }

    // --- 3. section_ref
    for (file, part) in &parts {
        let Some(body) = part.get("body") else { continue };
        walk_objects(body, &mut |o| {
            if str_of(o, "type") != Some("section_ref") {
                return;
            }
            let id = str_of(o, "id").unwrap_or("?");
            match str_of(o, "file") {
                Some(target) if actual.contains(target) => {
                    let ok = parts
                        .iter()
                        .find(|(n, _)| n == target)
                        .and_then(|(_, p)| p.get("body")?.get(0)?.get("id")?.as_str().map(|s| s == id))
                        .unwrap_or(false);
                    if !ok {
                        problems.push(format!("[section_ref] {file} の section_ref {id} と、{target} の中身が合っていません"));
                    }
                }
                Some(target) => problems.push(format!("[section_ref] {file} の section_ref {id} の先 {target} が、フォルダにありません")),
                None => problems.push(format!("[section_ref] {file} の section_ref {id} に file がありません")),
            }
        });
    }

    // --- 4. toc
    let mut in_toc: BTreeSet<String> = BTreeSet::new();
    fn walk_toc(nodes: &Value, sections: &HashMap<String, (String, i64, String)>, in_toc: &mut BTreeSet<String>, problems: &mut Vec<String>) {
        for n in nodes.as_array().into_iter().flatten() {
            let id = str_of(n, "id").unwrap_or("?");
            in_toc.insert(id.to_string());
            match sections.get(id) {
                None => problems.push(format!("[toc] 目次の {id} のセクションが、ファイルにありません")),
                Some((file, level, title)) => {
                    if str_of(n, "file") != Some(file.as_str()) {
                        problems.push(format!("[toc] 目次の {id} の file が、実際のファイル（{file}）と合っていません"));
                    }
                    if n.get("level").and_then(|l| l.as_i64()) != Some(*level) {
                        problems.push(format!("[toc] 目次の {id} の level が、セクションと合っていません"));
                    }
                    if str_of(n, "title") != Some(title.as_str()) {
                        problems.push(format!("[toc] 目次の {id} の title が、セクションの見出しと合っていません"));
                    }
                }
            }
            if let Some(c) = n.get("children") {
                walk_toc(c, sections, in_toc, problems);
            }
        }
    }
    if let Some(toc) = index.get("toc") {
        walk_toc(toc, &sections, &mut in_toc, &mut problems);
    }
    let mut missing: Vec<&String> = sections.keys().filter(|k| !in_toc.contains(*k)).collect();
    missing.sort();
    for id in missing {
        problems.push(format!("[toc] セクション {id} が、index.json の目次にありません"));
    }

    // --- 5. retired_ids
    let edition = index.get("edition");
    if let Some(r) = edition.and_then(|e| e.get("retired_ids")).and_then(|r| r.as_array()) {
        for id in r.iter().filter_map(|x| x.as_str()) {
            if let Some(file) = seen.get(id) {
                problems.push(format!("[retired_ids] 消した ID {id} が、{file} に残っています"));
            }
        }
    }

    // --- 6. edition.name
    if name != ORIGINAL {
        for p in edition_problems(edition) {
            problems.push(format!("[edition] {p}"));
        }
        match edition {
            None => {}
            Some(e) => {
                if str_of(e, "name") != Some(name) {
                    problems.push(format!(
                        "[edition] edition.name {:?} が、フォルダ名 {name:?} と同じではありません",
                        str_of(e, "name").unwrap_or("(なし)")
                    ));
                }
            }
        }
    }
    Ok(problems)
}
