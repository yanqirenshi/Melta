//! 結合テストの共通部品。
//!
//! `melta` は黒箱として扱う。
//! 実行ファイルは Cargo が渡す `CARGO_BIN_EXE_melta` で起動し、
//! 確かめるのは、終了コード、書かれたファイルの中身、出力に含まれる ID や版の名前だけにする。
//! 出力の文言は実装が決めるので、完全一致では比べない。
//!
//! テスト用の原本 `tests/fixtures/book/` は、毎回一時フォルダへコピーしてから使う。

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

pub const ORIGINAL: &str = "original";

/// テスト用の原本の、分けたファイルの一覧（読み順）。
pub const PART_FILES: [&str; 5] = [
    "001_front.json",
    "002_s1.json",
    "003_s1.1.json",
    "004_s1.2.json",
    "005_s2.json",
];

/// テスト用の原本のすべてのセンテンスの ID と本文。
pub const SENTENCES: [(&str, &str); 15] = [
    ("p1.1", "これはテスト用の本です。"),
    ("p1.2", "見出しより前の本文です。"),
    ("p2.1", "最初の文です。"),
    ("p2.2", "二番目の文は曰本語です。"),
    ("p2.3", "三番目の文です。"),
    ("l1.1.1", "一つ目の項目です。"),
    ("l1.1.2", "項目の二文目です。"),
    ("l1.2.1", "二つ目の項目です。"),
    ("p3.1", "この段落には文が一つしかありません。"),
    ("p4.1", "表の前の段落です。"),
    ("p4.2", "表を見てください。"),
    ("p5.1", "入れ子のセクションの本文です。"),
    ("p5.2", "二つ目の文です。"),
    ("p6.1", "大きな表の前の段落です。"),
    ("p6.2", "行は二十五あります。"),
];

// ------------------------------------------------------------ 作業場所

/// 一時フォルダにコピーしたテスト用の原本。
/// `out` が `<出力フォルダ>`（`structure/` の親）。
pub struct Book {
    _tmp: TempDir,
    pub out: PathBuf,
}

impl Book {
    /// `tests/fixtures/book/` を一時フォルダへコピーする。
    pub fn new() -> Book {
        let tmp = tempfile::tempdir().expect("一時フォルダを作れない");
        let out = tmp.path().join("book");
        copy_dir(&fixture_dir(), &out);
        Book { _tmp: tmp, out }
    }

    pub fn structure(&self) -> PathBuf {
        self.out.join("structure")
    }

    /// 版のフォルダ（`structure/<版>/`）。
    pub fn edition(&self, name: &str) -> PathBuf {
        self.structure().join(name)
    }

    pub fn original(&self) -> PathBuf {
        self.edition(ORIGINAL)
    }

    // -------------------------------------------------------- コマンドの実行

    /// `melta <cmd> <出力フォルダ> <rest...>` を実行する。
    pub fn run<I, S>(&self, cmd: &str, rest: I) -> Run
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut c = Command::new(melta_bin());
        c.arg(cmd).arg(&self.out);
        for a in rest {
            c.arg(a);
        }
        Run::from(c.output().expect("melta を起動できない"))
    }

    /// `melta new <出力フォルダ> --from <from> --editor <editor>` が成功することを確かめ、版の名前を返す。
    pub fn new_edition(&self, from: &str, editor: &str) -> String {
        let r = self.run("new", ["--from", from, "--editor", editor]);
        r.ok();
        let name = r.stdout.trim().to_string();
        assert!(
            self.edition(&name).join("index.json").is_file(),
            "melta new の標準出力が版の名前になっていない、または版ができていない: {:?}",
            r.stdout
        );
        name
    }

    /// `melta check` が成功することを確かめる。
    pub fn check_ok(&self, edition: &str) {
        self.run("check", [edition]).ok();
    }

    /// `melta check` が失敗することを確かめる（パニックではなく、問題として出す）。
    pub fn check_fails(&self, edition: &str, why: &str) {
        let r = self.run("check", [edition]);
        assert!(
            !r.success(),
            "melta check が壊れた版を見逃した（{why}）\nstdout:\n{}\nstderr:\n{}",
            r.stdout,
            r.stderr
        );
        r.not_panicked(why);
    }

    // -------------------------------------------------------- 版の読み書き

    pub fn index(&self, edition: &str) -> Value {
        read_json(&self.edition(edition).join("index.json"))
    }

    pub fn write_index(&self, edition: &str, v: &Value) {
        write_json(&self.edition(edition).join("index.json"), v);
    }

    pub fn part(&self, edition: &str, file: &str) -> Value {
        read_json(&self.edition(edition).join(file))
    }

    pub fn write_part(&self, edition: &str, file: &str, v: &Value) {
        write_json(&self.edition(edition).join(file), v);
    }

    /// 版の分けたファイルの名前（`index.json` の `files` の順）。
    pub fn files(&self, edition: &str) -> Vec<String> {
        self.index(edition)["files"]
            .as_array()
            .expect("index.json の files が配列でない")
            .iter()
            .map(|f| f["file"].as_str().unwrap().to_string())
            .collect()
    }

    /// 版のすべての分けたファイルを、ファイル名 → JSON で読む。
    pub fn parts(&self, edition: &str) -> BTreeMap<String, Value> {
        self.files(edition)
            .into_iter()
            .map(|f| {
                let v = self.part(edition, &f);
                (f, v)
            })
            .collect()
    }

    /// ID で要素を探す（section_ref は除く。センテンス、箇条書きの項目も探す）。
    pub fn find(&self, edition: &str, id: &str) -> Option<Value> {
        for (_, part) in self.parts(edition) {
            if let Some(v) = find_in(&part["body"], id) {
                return Some(v);
            }
        }
        None
    }

    pub fn get(&self, edition: &str, id: &str) -> Value {
        self.find(edition, id)
            .unwrap_or_else(|| panic!("版 {edition} に ID {id} が無い"))
    }

    /// センテンスの本文。
    pub fn text(&self, edition: &str, id: &str) -> String {
        self.get(edition, id)["text"]
            .as_str()
            .unwrap_or_else(|| panic!("{id} の text が文字列でない"))
            .to_string()
    }

    /// パラグラフ、または箇条書きの項目の、センテンスの ID（配列の順）。
    pub fn sentence_ids(&self, edition: &str, container: &str) -> Vec<String> {
        self.get(edition, container)["sentences"]
            .as_array()
            .unwrap_or_else(|| panic!("{container} に sentences が無い"))
            .iter()
            .map(|s| s["id"].as_str().unwrap().to_string())
            .collect()
    }

    /// 版のすべての要素の ID → 要素（section_ref は除く）。
    pub fn all_elements(&self, edition: &str) -> BTreeMap<String, Value> {
        let mut m = BTreeMap::new();
        for (_, part) in self.parts(edition) {
            collect(&part["body"], &mut m);
        }
        m
    }

    /// `edition.retired_ids`（無ければ空）。
    pub fn retired_ids(&self, edition: &str) -> Vec<String> {
        self.index(edition)["edition"]["retired_ids"]
            .as_array()
            .map(|a| a.iter().map(|v| v.as_str().unwrap().to_string()).collect())
            .unwrap_or_default()
    }

    /// 版のフォルダの下のすべてのファイルの中身（相対パス → バイト列）。
    pub fn snapshot(&self, edition: &str) -> BTreeMap<String, Vec<u8>> {
        snapshot(&self.edition(edition))
    }

    /// `structure/` の下の版のフォルダ名。
    pub fn edition_dirs(&self) -> Vec<String> {
        let mut v: Vec<String> = fs::read_dir(self.structure())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        v.sort();
        v
    }
}

// ------------------------------------------------------------ 実行結果

pub struct Run {
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl Run {
    fn from(o: Output) -> Run {
        Run {
            code: o.status.code(),
            stdout: String::from_utf8_lossy(&o.stdout).to_string(),
            stderr: String::from_utf8_lossy(&o.stderr).to_string(),
        }
    }

    pub fn success(&self) -> bool {
        self.code == Some(0)
    }

    /// 成功（終了コード 0）を確かめる。
    pub fn ok(&self) -> &Run {
        assert!(
            self.success(),
            "melta が失敗した（終了コード {:?}）\nstdout:\n{}\nstderr:\n{}",
            self.code,
            self.stdout,
            self.stderr
        );
        self
    }

    /// 失敗（終了コードが 0 以外）を確かめる。
    pub fn fails(&self, why: &str) -> &Run {
        assert!(
            !self.success(),
            "melta が成功してしまった（{why}）\nstdout:\n{}\nstderr:\n{}",
            self.stdout,
            self.stderr
        );
        assert!(
            !self.stderr.trim().is_empty(),
            "失敗したのに標準エラーに何も出ていない（{why}）"
        );
        self.not_panicked(why);
        self
    }

    /// パニックで終わっていないことを確かめる（Rust のパニックは終了コード 101 で、標準エラーに "panicked" が出る）。
    pub fn not_panicked(&self, why: &str) -> &Run {
        assert!(
            self.code != Some(101) && !self.stderr.contains("panicked"),
            "melta がパニックした（{why}）\nstderr:\n{}",
            self.stderr
        );
        self
    }

    /// 標準出力を JSON として読む。
    pub fn json(&self) -> Value {
        serde_json::from_str(self.stdout.trim()).unwrap_or_else(|e| {
            panic!("標準出力が JSON として読めない: {e}\nstdout:\n{}", self.stdout)
        })
    }
}

// ------------------------------------------------------------ 道具

pub fn melta_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_melta"))
}

pub fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("book")
}

pub fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let p = e.path();
        let q = to.join(e.file_name());
        if p.is_dir() {
            copy_dir(&p, &q);
        } else {
            fs::copy(&p, &q).unwrap();
        }
    }
}

pub fn snapshot(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(base: &Path, dir: &Path, m: &mut BTreeMap<String, Vec<u8>>) {
        for e in fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(base, &p, m);
            } else {
                let rel = p.strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/");
                m.insert(rel, fs::read(&p).unwrap());
            }
        }
    }
    let mut m = BTreeMap::new();
    walk(dir, dir, &mut m);
    m
}

pub fn read_json(path: &Path) -> Value {
    let s = fs::read_to_string(path).unwrap_or_else(|e| panic!("{} が読めない: {e}", path.display()));
    serde_json::from_str(&s).unwrap_or_else(|e| panic!("{} が JSON として読めない: {e}", path.display()))
}

/// テストで版を壊すときに使う（書式は気にしない）。
pub fn write_json(path: &Path, v: &Value) {
    fs::write(path, serde_json::to_string_pretty(v).unwrap() + "\n").unwrap();
}

fn find_in(v: &Value, id: &str) -> Option<Value> {
    match v {
        Value::Array(a) => a.iter().find_map(|x| find_in(x, id)),
        Value::Object(o) => {
            let is_ref = o.get("type").and_then(Value::as_str) == Some("section_ref");
            if !is_ref && o.get("id").and_then(Value::as_str) == Some(id) {
                return Some(v.clone());
            }
            for k in ["children", "sentences", "items"] {
                if let Some(c) = o.get(k) {
                    if let Some(f) = find_in(c, id) {
                        return Some(f);
                    }
                }
            }
            None
        }
        _ => None,
    }
}

fn collect(v: &Value, m: &mut BTreeMap<String, Value>) {
    match v {
        Value::Array(a) => a.iter().for_each(|x| collect(x, m)),
        Value::Object(o) => {
            let is_ref = o.get("type").and_then(Value::as_str) == Some("section_ref");
            if !is_ref {
                if let Some(id) = o.get("id").and_then(Value::as_str) {
                    let prev = m.insert(id.to_string(), v.clone());
                    assert!(prev.is_none(), "ID {id} が重複している");
                }
            }
            for k in ["children", "sentences", "items"] {
                if let Some(c) = o.get(k) {
                    collect(c, m);
                }
            }
        }
        _ => {}
    }
}

/// 版の中で、ID が `prefix.` + 数字 の形の ID（例: `p2.4`）を探す。
pub fn ids_under(elements: &BTreeMap<String, Value>, container: &str) -> Vec<String> {
    let pre = format!("{container}.");
    elements
        .keys()
        .filter(|k| {
            k.strip_prefix(&pre)
                .map(|rest| !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()))
                .unwrap_or(false)
        })
        .cloned()
        .collect()
}

/// 出力に、ID がそのままの形で含まれるか（`p2.1` を探して `p2.10` に当たらないように、前後を確かめる）。
pub fn mentions_id(text: &str, id: &str) -> bool {
    let is_id_char = |c: char| c.is_ascii_alphanumeric() || c == '.';
    let mut start = 0;
    while let Some(pos) = text[start..].find(id) {
        let i = start + pos;
        let before = text[..i].chars().next_back();
        let after_str = &text[i + id.len()..];
        let after = after_str.chars().next();
        // 後ろの「.」は文末の句点の可能性があるので、「.」+ 数字のときだけ ID の続きとみなす
        let after_is_id = match after {
            Some('.') => after_str[1..].chars().next().is_some_and(|c| c.is_ascii_digit()),
            Some(c) => is_id_char(c),
            None => false,
        };
        if !before.is_some_and(is_id_char) && !after_is_id {
            return true;
        }
        start = i + id.len();
    }
    false
}
