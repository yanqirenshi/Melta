//! 版（`<出力フォルダ>/structure/<版>/`）の読み書き。
//!
//! JSON の書き出しは Tyggja の `structure.py` の `dumps` と同じ書式にする
//! （センテンス 1 つを 1 行、UTF-8、日本語はエスケープしない）。
//! 直した要素の `origin` は、センテンスや表のセルの 1 行の中に収める。

use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

pub type Res<T> = Result<T, String>;

pub const STRUCTURE_DIR: &str = "structure";
pub const ORIGINAL: &str = "original";
pub const INDEX_FILE: &str = "index.json";
const ONE_LINE_CHARS: usize = 150;

// ---------------------------------------------------------------- 名前とパス

/// 版の名前に使えるか（`/`、`\`、`..` を含まない）。
pub fn validate_edition_name(name: &str) -> Res<()> {
    if name.is_empty() || name == "." || name == ".." || name.contains('/') || name.contains('\\') || name.contains("..") {
        return Err(format!(
            "版の名前 {name:?} は使えません（/ や \\ や .. を含まない名前にしてください）"
        ));
    }
    Ok(())
}

pub fn edition_dir(out: &Path, name: &str) -> Res<PathBuf> {
    validate_edition_name(name)?;
    Ok(out.join(STRUCTURE_DIR).join(name))
}

/// フォルダの中のファイル名として使えるか（パスをたどれない名前）。
pub fn validate_file_name(name: &str) -> Res<()> {
    if name.is_empty() || name.contains('/') || name.contains('\\') || name.contains("..") {
        return Err(format!("ファイル名 {name:?} は使えません"));
    }
    Ok(())
}

pub fn read_json(path: &Path) -> Res<Value> {
    read_json_eol(path).map(|(v, _)| v)
}

/// 改行の形式（CRLF か）も返す。書き戻すときに、元の形式を保つため。
pub fn read_json_eol(path: &Path) -> Res<(Value, bool)> {
    let text = fs::read_to_string(path).map_err(|e| format!("{} を読めません: {e}", path.display()))?;
    serde_json::from_str(&text)
        .map(|v| (v, text.contains("\r\n")))
        .map_err(|e| format!("{} が JSON として読めません（{} 行目 {} 桁目: {e}）", path.display(), e.line(), e.column()))
}

/// 行の区切りを、元の形式（CRLF か LF）にそろえる。JSON の文字列に生の改行は入らないので、そのまま置き換えてよい。
pub fn with_eol(text: String, crlf: bool) -> String {
    if crlf {
        text.replace('\n', "\r\n")
    } else {
        text
    }
}

/// 版のフォルダの `.json` の名前（index.json は除く）。名前順。
pub fn json_files(dir: &Path) -> Res<Vec<String>> {
    let mut names = Vec::new();
    let rd = fs::read_dir(dir).map_err(|e| format!("{} を読めません: {e}", dir.display()))?;
    for ent in rd {
        let ent = ent.map_err(|e| e.to_string())?;
        let name = ent.file_name().to_string_lossy().to_string();
        if name.ends_with(".json") && name != INDEX_FILE && ent.path().is_file() {
            names.push(name);
        }
    }
    names.sort();
    Ok(names)
}

/// structure/ の下にある版の名前（index.json があるフォルダ）。名前順。
pub fn edition_names(out: &Path) -> Res<Vec<String>> {
    let root = out.join(STRUCTURE_DIR);
    let rd = fs::read_dir(&root).map_err(|e| format!("{} を読めません: {e}", root.display()))?;
    let mut names = Vec::new();
    for ent in rd {
        let ent = ent.map_err(|e| e.to_string())?;
        if ent.path().is_dir() && ent.path().join(INDEX_FILE).is_file() {
            names.push(ent.file_name().to_string_lossy().to_string());
        }
    }
    names.sort();
    Ok(names)
}

// ---------------------------------------------------------------- JSON の書き出し

fn scalar(v: &Value) -> bool {
    !v.is_object() && !v.is_array()
}

fn json_str(s: &str) -> String {
    serde_json::to_string(s).unwrap()
}

/// Python の `json.dumps(ensure_ascii=False)` と同じ、区切りに空白を入れた 1 行の JSON。
pub fn oneline(v: &Value) -> String {
    match v {
        Value::Object(m) => {
            let items: Vec<String> = m.iter().map(|(k, x)| format!("{}: {}", json_str(k), oneline(x))).collect();
            format!("{{{}}}", items.join(", "))
        }
        Value::Array(a) => {
            let items: Vec<String> = a.iter().map(oneline).collect();
            format!("[{}]", items.join(", "))
        }
        _ => serde_json::to_string(v).unwrap(),
    }
}

/// Tyggja の `dumps` と同じ書式。スカラーだけの辞書と、スカラーだけの配列は 1 行に書く。
/// `origin` は、辞書の中でも 1 行に書く（直したセンテンスやセルを 1 行に保つため）。
/// `fit` が真のときは、短い辞書を 1 行にする（index.json 用）。
pub fn dumps(v: &Value, level: usize, fit: bool) -> String {
    let pad = " ".repeat(level + 1);
    let end = " ".repeat(level);
    match v {
        Value::Object(m) => {
            if m.is_empty() || m.iter().all(|(k, x)| scalar(x) || k == "origin") {
                return oneline(v);
            }
            if fit {
                let one = oneline(v);
                if one.chars().count() <= ONE_LINE_CHARS {
                    return one;
                }
            }
            let items: Vec<String> = m
                .iter()
                .map(|(k, x)| {
                    let body = if k == "origin" { oneline(x) } else { dumps(x, level + 1, fit) };
                    format!("{pad}{}: {body}", json_str(k))
                })
                .collect();
            format!("{{\n{}\n{end}}}", items.join(",\n"))
        }
        Value::Array(a) => {
            if a.is_empty() || a.iter().all(scalar) {
                return oneline(v);
            }
            let items: Vec<String> = a.iter().map(|x| format!("{pad}{}", dumps(x, level + 1, fit))).collect();
            format!("[\n{}\n{end}]", items.join(",\n"))
        }
        _ => serde_json::to_string(v).unwrap(),
    }
}

// ---------------------------------------------------------------- 木をたどる

/// `pred` に当たる最初の要素の JSON ポインタを返す（深さ優先、書かれた順）。
pub fn find_path(v: &Value, pred: &dyn Fn(&Value) -> bool, path: &mut String) -> Option<String> {
    if pred(v) {
        return Some(path.clone());
    }
    let len = path.len();
    match v {
        Value::Object(m) => {
            for (k, x) in m {
                path.push('/');
                path.push_str(k);
                if let Some(r) = find_path(x, pred, path) {
                    return Some(r);
                }
                path.truncate(len);
            }
        }
        Value::Array(a) => {
            for (i, x) in a.iter().enumerate() {
                path.push('/');
                path.push_str(&i.to_string());
                if let Some(r) = find_path(x, pred, path) {
                    return Some(r);
                }
                path.truncate(len);
            }
        }
        _ => {}
    }
    None
}

/// すべての辞書を、書かれた順に訪ねる。
pub fn walk_objects(v: &Value, f: &mut dyn FnMut(&Value)) {
    match v {
        Value::Object(m) => {
            f(v);
            for x in m.values() {
                walk_objects(x, f);
            }
        }
        Value::Array(a) => {
            for x in a {
                walk_objects(x, f);
            }
        }
        _ => {}
    }
}

pub fn str_of<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(|x| x.as_str())
}

fn chars(text: &str) -> usize {
    text.chars().filter(|c| !c.is_whitespace()).count()
}

fn sentences_chars(v: &Value) -> usize {
    v.get("sentences")
        .and_then(|s| s.as_array())
        .map(|a| a.iter().map(|s| chars(str_of(s, "text").unwrap_or(""))).sum())
        .unwrap_or(0)
}

/// 本文の文字数（空白を除く）。`section_ref` の先は数えない。index.json の `files[].chars` の値。
pub fn body_chars(nodes: &[Value]) -> usize {
    nodes
        .iter()
        .map(|n| match str_of(n, "type") {
            Some("section") => {
                chars(str_of(n, "title").unwrap_or(""))
                    + n.get("children").and_then(|c| c.as_array()).map(|c| body_chars(c)).unwrap_or(0)
            }
            Some("paragraph") => sentences_chars(n),
            Some("list") => n
                .get("items")
                .and_then(|i| i.as_array())
                .map(|a| a.iter().map(sentences_chars).sum())
                .unwrap_or(0),
            Some("caption") => chars(str_of(n, "text").unwrap_or("")),
            _ => 0,
        })
        .sum()
}

// ---------------------------------------------------------------- 版

pub struct Part {
    pub name: String,
    pub value: Value,
    pub dirty: bool,
    pub crlf: bool,
}

pub struct Edition {
    pub name: String,
    pub dir: PathBuf,
    pub index: Value,
    pub parts: Vec<Part>,
    pub index_dirty: bool,
    pub index_crlf: bool,
}

impl Edition {
    /// index.json の `files` の順にファイルを読む。
    pub fn load(out: &Path, name: &str) -> Res<Edition> {
        let dir = edition_dir(out, name)?;
        let index_path = dir.join(INDEX_FILE);
        if !index_path.is_file() {
            return Err(format!("{} がありません（版 {name} が structure/ の下にありません）", index_path.display()));
        }
        let (index, index_crlf) = read_json_eol(&index_path)?;
        let files = index
            .get("files")
            .and_then(|f| f.as_array())
            .ok_or_else(|| format!("{} に files（ファイルの一覧）がありません", index_path.display()))?;
        let mut parts = Vec::new();
        for f in files {
            let file = str_of(f, "file").ok_or_else(|| format!("{} の files に file の無い項目があります", index_path.display()))?;
            validate_file_name(file)?;
            let path = dir.join(file);
            if !path.is_file() {
                return Err(format!("index.json の files にある {file} が、版 {name} にありません"));
            }
            let (value, crlf) = read_json_eol(&path)?;
            parts.push(Part { name: file.to_string(), value, dirty: false, crlf });
        }
        Ok(Edition { name: name.to_string(), dir, index, parts, index_dirty: false, index_crlf })
    }

    /// この版の元の版の名前（`edition.base`）。
    pub fn base(&self) -> Res<String> {
        if let Some(first) = edition_problems(self.index.get("edition")).first() {
            return Err(format!("版 {} の {first}（編集を続けられません）", self.name));
        }
        self.index
            .get("edition")
            .and_then(|e| str_of(e, "base"))
            .map(|s| s.to_string())
            .ok_or_else(|| format!("版 {} の index.json に edition.base がありません（melta new で作った版ではありません）", self.name))
    }

    pub fn retired_ids(&self) -> Vec<String> {
        self.index
            .get("edition")
            .and_then(|e| e.get("retired_ids"))
            .and_then(|r| r.as_array())
            .map(|a| a.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default()
    }

    /// ID を持つ要素（`section_ref` は除く）を探す。返すのは（ファイルの位置, JSON ポインタ）。
    pub fn find_id(&self, id: &str) -> Option<(usize, String)> {
        let pred = |v: &Value| str_of(v, "id") == Some(id) && str_of(v, "type") != Some("section_ref");
        for (i, p) in self.parts.iter().enumerate() {
            if let Some(body) = p.value.get("body") {
                let mut path = "/body".to_string();
                if let Some(r) = find_path(body, &pred, &mut path) {
                    return Some((i, r));
                }
            }
        }
        None
    }

    /// 直したファイルと、index.json の `files` の文字数を書き戻す。
    pub fn save(&mut self) -> Res<()> {
        let mut any = false;
        for p in self.parts.iter_mut().filter(|p| p.dirty) {
            let text = format!("{}\n", dumps(&p.value, 0, false));
            let json_chars = text.chars().count();
            let text = with_eol(text, p.crlf);
            let n_chars = p.value.get("body").and_then(|b| b.as_array()).map(|b| body_chars(b)).unwrap_or(0);
            if let Some(files) = self.index.get_mut("files").and_then(|f| f.as_array_mut()) {
                for f in files.iter_mut().filter(|f| str_of(f, "file") == Some(p.name.as_str())) {
                    f["chars"] = Value::from(n_chars);
                    f["json_chars"] = Value::from(json_chars);
                }
            }
            fs::write(self.dir.join(&p.name), text).map_err(|e| format!("{} を書けません: {e}", p.name))?;
            p.dirty = false;
            any = true;
        }
        if any || self.index_dirty {
            let text = with_eol(format!("{}\n", dumps(&self.index, 0, true)), self.index_crlf);
            fs::write(self.dir.join(INDEX_FILE), text).map_err(|e| format!("index.json を書けません: {e}"))?;
            self.index_dirty = false;
        }
        Ok(())
    }
}

/// `edition` の形の問題（なければ空）。`edition` は index.json の `edition` の値。
pub fn edition_problems(e: Option<&Value>) -> Vec<String> {
    let mut p = Vec::new();
    let Some(e) = e else { return vec!["index.json に edition がありません".to_string()] };
    let Some(m) = e.as_object() else { return vec!["edition がオブジェクトではありません".to_string()] };
    for k in ["name", "base", "editor", "created_at"] {
        if !m.get(k).map_or(false, |v| v.is_string()) {
            p.push(format!("edition.{k} が文字列ではありません"));
        }
    }
    match m.get("retired_ids") {
        Some(Value::Array(a)) if a.iter().all(|x| x.is_string()) => {}
        _ => p.push("edition.retired_ids が、文字列の配列ではありません".to_string()),
    }
    match m.get("retired_notes") {
        None => {}
        Some(Value::Object(n)) if n.values().all(|x| x.is_string()) => {}
        Some(_) => p.push("edition.retired_notes が、文字列を値に持つオブジェクトではありません".to_string()),
    }
    p
}
