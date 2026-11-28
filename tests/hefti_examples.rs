//! Hefti（@yanqirenshi/hefti）の見本を使う、`melta check` の黒箱のテスト。
//!
//! Hefti の README の約束「ほかの言語の実装では、examples/valid/ がすべて通り、
//! examples/invalid/ がすべて拒否されることを確かめる」を、Melta で確かめる。
//!
//! 見本は `scripts/sync-schema.sh` が `tests/fixtures/hefti/examples/` に置く（このテストは置かない・書き換えない）。
//!
//! - `valid/<本の名前>/` … それぞれを `<出力フォルダ>/structure/original/` に写すと、check が通る
//! - `invalid/index/<違反>.json` … `valid/sized_book` の `index.json` と入れ替えると、check が拒否する
//! - `invalid/part/<違反>.json` … `valid/sized_book` の分けたファイルの 1 つと入れ替えると、check が拒否する
//!
//! invalid の見本は、Hefti の `scripts/build-invalid.mjs` が `sized_book` を 1 か所だけ壊して作ったもの。
//! part の見本をどのファイルと入れ替えるかは、build-invalid.mjs と同じ決め方で求める（`part_target`）。
//!
//! 件数は決め打ちにせず、フォルダの中を数えて回す。0 件なら失敗させる（見本の置き忘れを見逃さないため）。
//! ループの中では止まらずに、失敗した見本の名前をすべて集めてから、まとめて失敗させる。

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use common::{copy_dir, melta_bin, read_json};
use serde_json::Value;

/// build-invalid.mjs の FILE_TARGET（file_ で始まる違反を入れるファイル）。
const FILE_TARGET: &str = "004_s1.1.1.json";

fn examples_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("hefti")
        .join("examples")
}

fn sized_book() -> PathBuf {
    examples_dir().join("valid").join("sized_book")
}

/// フォルダの中の項目（名前の順）。フォルダが無いか、条件に合うものが 0 件なら失敗させる。
fn list(dir: &Path, want_dir: bool) -> Vec<PathBuf> {
    assert!(
        dir.is_dir(),
        "Hefti の見本のフォルダ {} がありません（scripts/sync-schema.sh で置かれるはず）",
        dir.display()
    );
    let mut v: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| if want_dir { p.is_dir() } else { p.is_file() && p.extension().is_some_and(|x| x == "json") })
        .collect();
    v.sort();
    assert!(!v.is_empty(), "Hefti の見本のフォルダ {} に、見本が 1 つもありません", dir.display());
    v
}

fn name_of(p: &Path) -> String {
    p.file_name().unwrap().to_string_lossy().to_string()
}

// ------------------------------------------------------------ 実行

struct Checked {
    code: Option<i32>,
    out: String,
}

/// 一時フォルダに `<出力フォルダ>/structure/original/` を作り、`book` の中身を写す。
/// `replace` があれば、そのファイル（名前 → 中身のファイル）を入れ替えてから、`melta check` を実行する。
fn check_book(book: &Path, replace: Option<(&str, &Path)>) -> Checked {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("book");
    let original = out.join("structure").join("original");
    copy_dir(book, &original);
    if let Some((name, from)) = replace {
        let to = original.join(name);
        assert!(to.is_file(), "入れ替える先の {name} が、{} にありません", book.display());
        fs::copy(from, &to).unwrap();
    }
    let o = Command::new(melta_bin())
        .arg("check")
        .arg(&out)
        .arg("original")
        .output()
        .expect("melta を起動できない");
    Checked {
        code: o.status.code(),
        out: format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)),
    }
}

/// 拒否されたことと、スキーマで拒否されたこと（別の理由で落ちたのではないこと）を確かめる。
/// 問題がなければ None、あれば失敗の説明を返す。
fn rejected_by_schema(c: &Checked, file: &str) -> Option<String> {
    if c.code == Some(0) {
        return Some(format!("check が通ってしまった\n{}", c.out));
    }
    if c.code == Some(101) || c.out.contains("panicked") {
        return Some(format!("check がパニックした\n{}", c.out));
    }
    // check は問題を 1 行ずつ出し、スキーマの問題には「[スキーマ]」が付く（src/check.rs の validate）。
    // その行に、入れ替えたファイルの名前が出ていることで、そのファイルがスキーマで拒否されたと見る。
    let hit = c.out.lines().any(|l| l.contains("スキーマ") && l.contains(file));
    if !hit {
        return Some(format!("拒否はされたが、{file} のスキーマの問題として出ていない（別の理由で落ちている）\n{}", c.out));
    }
    None
}

// ------------------------------------------------------------ part の見本の入れ替え先

/// body 以下の要素を出現順にたどる（section の中には入る。section_ref の先には入らない）。
/// build-invalid.mjs の walk と同じ。
fn walk<'a>(nodes: &'a Value, out: &mut Vec<&'a Value>) {
    for n in nodes.as_array().into_iter().flatten() {
        out.push(n);
        if n["type"] == "section" {
            walk(&n["children"], out);
        }
    }
}

/// invalid/part の見本を、sized_book のどのファイルと入れ替えるか。
/// build-invalid.mjs の mutatePart と同じ決め方:
/// - `file_<違反>.json` は FILE_TARGET
/// - `<要素の種類>--<違反>.json` は、sized_book のファイルを名前の順に見て、
///   その種類の要素を最初に含むファイル
fn part_target(example: &str) -> String {
    if example.starts_with("file_") {
        return FILE_TARGET.to_string();
    }
    let kind = example
        .split_once("--")
        .unwrap_or_else(|| panic!("見本の名前 {example} が「<要素の種類>--<違反>.json」の形でない"))
        .0;
    for p in list(&sized_book(), false) {
        let name = name_of(&p);
        if name == "index.json" {
            continue;
        }
        let v = read_json(&p);
        let mut nodes = Vec::new();
        walk(&v["body"], &mut nodes);
        if nodes.iter().any(|n| n["type"] == kind) {
            return name;
        }
    }
    panic!("sized_book に {kind} の要素がありません");
}

// ------------------------------------------------------------ テスト

#[test]
fn hefti_valid_examples_pass_check() {
    let books = list(&examples_dir().join("valid"), true);
    let mut failed = Vec::new();
    for book in &books {
        let c = check_book(book, None);
        if c.code != Some(0) {
            failed.push(format!("valid/{}（終了コード {:?}）\n{}", name_of(book), c.code, c.out));
        }
    }
    assert!(
        failed.is_empty(),
        "Hefti の正しい見本 {} 冊のうち {} 冊を、check が拒否した:\n\n{}",
        books.len(),
        failed.len(),
        failed.join("\n")
    );
}

#[test]
fn hefti_invalid_index_examples_are_rejected() {
    let examples = list(&examples_dir().join("invalid").join("index"), false);
    let base = sized_book();
    let mut failed = Vec::new();
    for ex in &examples {
        let c = check_book(&base, Some(("index.json", ex)));
        if let Some(why) = rejected_by_schema(&c, "index.json") {
            failed.push(format!("invalid/index/{}: {why}", name_of(ex)));
        }
    }
    assert!(
        failed.is_empty(),
        "Hefti の壊れた index の見本 {} 件のうち {} 件を、check がスキーマで拒否しなかった:\n\n{}",
        examples.len(),
        failed.len(),
        failed.join("\n")
    );
}

#[test]
fn hefti_invalid_part_examples_are_rejected() {
    let examples = list(&examples_dir().join("invalid").join("part"), false);
    let base = sized_book();
    let mut failed = Vec::new();
    for ex in &examples {
        let target = part_target(&name_of(ex));
        let c = check_book(&base, Some((&target, ex)));
        if let Some(why) = rejected_by_schema(&c, &target) {
            failed.push(format!("invalid/part/{}（{target} と入れ替え）: {why}", name_of(ex)));
        }
    }
    assert!(
        failed.is_empty(),
        "Hefti の壊れた part の見本 {} 件のうち {} 件を、check がスキーマで拒否しなかった:\n\n{}",
        examples.len(),
        failed.len(),
        failed.join("\n")
    );
}

// ------------------------------------------------------------ 見本と入れ替え先の確かめ（テスト自体の誤りを防ぐ）

#[test]
fn hefti_invalid_examples_differ_from_their_targets() {
    // 入れ替え先の決め方が正しければ、壊れた見本は、入れ替え先の正しいファイルと 1 か所だけ違う。
    // 同じだったら（入れ替え先の取り違え、見本の作り直し忘れ）、拒否のテストが意味を持たない
    let base = sized_book();
    let mut failed = Vec::new();
    let index_examples = list(&examples_dir().join("invalid").join("index"), false);
    for ex in &index_examples {
        if read_json(ex) == read_json(&base.join("index.json")) {
            failed.push(format!("invalid/index/{} が、正しい index.json と同じ", name_of(ex)));
        }
    }
    let part_examples = list(&examples_dir().join("invalid").join("part"), false);
    for ex in &part_examples {
        let target = part_target(&name_of(ex));
        let bad = read_json(ex);
        let good = read_json(&base.join(&target));
        if bad == good {
            failed.push(format!("invalid/part/{} が、入れ替え先の {target} と同じ", name_of(ex)));
        }
        // 壊れた見本の file（ファイル名）は、どの壊し方でも残っているので、入れ替え先と合うはず
        if bad.get("file").is_some() && bad["file"] != good["file"] {
            failed.push(format!(
                "invalid/part/{} の file（{}）が、入れ替え先の {target} と合わない",
                name_of(ex),
                bad["file"]
            ));
        }
    }
    assert!(failed.is_empty(), "見本と入れ替え先が合わない:\n{}", failed.join("\n"));
}

#[test]
fn hefti_valid_examples_include_readme_books() {
    // 件数は決め打ちにしないが、Hefti の README の表にある正しい見本 8 冊の名前が、欠けていないかを見る
    let names: Vec<String> = list(&examples_dir().join("valid"), true).iter().map(|p| name_of(p)).collect();
    for want in [
        "rich_book",
        "rich_split_book",
        "sized_book",
        "no_heading_book",
        "empty_book",
        "big_table_book",
        "deep_book",
        "merged_table_book",
    ] {
        assert!(names.iter().any(|n| n == want), "Hefti の正しい見本 {want} が置かれていない: {names:?}");
    }
}
