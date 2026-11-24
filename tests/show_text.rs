//! `show --cell` / `show --index`（1 つだけ出す）と、`show --text`（文字をそのまま出す）のテスト。
//!
//! 引用符、バックスラッシュ、改行、タブ、末尾の空白・改行、4 バイトの文字は、
//! テスト用の原本の p7、c4、t3、f3（005_s2.json の末尾）に入れてある。

mod common;

use common::*;
use serde_json::{json, Value};

/// `show ... --text` が成功し、標準出力のバイト列が `want` の UTF-8 と完全に同じであることを確かめる。
fn assert_text(b: &Book, args: &[&str], want: &str) {
    let r = b.run("show", args);
    r.ok();
    assert!(
        r.stdout_bytes == want.as_bytes(),
        "show {args:?} の標準出力が、文字そのものと違う\n期待: {:?}\n実際: {:?}",
        want,
        String::from_utf8_lossy(&r.stdout_bytes)
    );
}

#[test]
fn fixture_has_special_characters() {
    // テスト用の原本に、確かめたい文字が入っていることの確認
    let all = [P7_1, P7_2, C4, T3_0_0, T3_0_1, F3_0, F3_1].concat();
    for (c, name) in [('"', "引用符"), ('\\', "バックスラッシュ"), ('\n', "改行"), ('\t', "タブ"), ('😀', "絵文字"), ('𠮷', "4 バイトの漢字")] {
        assert!(all.contains(c), "テスト用の原本に {name} が無い");
    }
    let b = Book::new();
    assert_eq!(b.text(ORIGINAL, "p7.1"), P7_1);
    assert_eq!(b.text(ORIGINAL, "p7.2"), P7_2);
    assert_eq!(b.get(ORIGINAL, "c4")["text"], C4);
    assert_eq!(b.get(ORIGINAL, "f3")["text"], json!([F3_0, F3_1]));
}

// ============================================================ show --cell

#[test]
fn show_cell_outputs_the_cell_object() {
    let b = Book::new();
    let r = b.run("show", [ORIGINAL, "t1", "--cell", "1,0"]);
    r.ok();
    let v = r.json();
    assert_eq!(v["text"], "りんご", "show --cell の出力がセルでない:\n{}", r.stdout);
    assert_eq!(v["row"], 1);
    assert_eq!(v["col"], 0);
    // 1 つのセルだけで、ほかのセルは出ない
    assert!(!r.stdout.contains("項目") && !r.stdout.contains("合計"), "ほかのセルが出ている:\n{}", r.stdout);
}

#[test]
fn show_cell_special_characters_as_json() {
    let b = Book::new();
    let r = b.run("show", [ORIGINAL, "t3", "--cell", "0,1"]);
    r.ok();
    assert_eq!(r.json()["text"], T3_0_1);
}

#[test]
fn show_cell_merged_and_big_table() {
    let b = Book::new();
    // 結合しているセルは、左上のマスで指す
    let r = b.run("show", [ORIGINAL, "t1", "--cell", "2,0"]);
    r.ok();
    assert_eq!(r.json()["text"], "合計");
    assert_eq!(r.json()["col_span"], 2);
    let r = b.run("show", [ORIGINAL, "t2", "--cell", "24,1"]);
    r.ok();
    assert_eq!(r.json()["text"], "240");
    let r = b.run("show", [ORIGINAL, "t2", "--cell", "10,1"]);
    r.ok();
    assert_eq!(r.json()["row_span"], 2);
}

#[test]
fn show_cell_of_edited_edition() {
    let b = Book::new();
    let e = b.new_edition(ORIGINAL, "fix");
    b.run("set-text", [e.as_str(), "t1", "--cell", "1,0", "--text", "みかん", "--note", "n"]).ok();
    let v = b.run("show", [e.as_str(), "t1", "--cell", "1,0"]).json();
    assert_eq!(v["text"], "みかん");
    assert_eq!(v["origin"]["text"], "りんご");
    assert_eq!(v["note"], "n");
}

#[test]
fn show_cell_errors() {
    let b = Book::new();
    b.run("show", [ORIGINAL, "t1", "--cell", "2,1"]).fails("結合で隠れたマス");
    b.run("show", [ORIGINAL, "t2", "--cell", "11,1"]).fails("結合で隠れたマス（行の結合）");
    b.run("show", [ORIGINAL, "t1", "--cell", "9,9"]).fails("表の外");
    b.run("show", [ORIGINAL, "t1", "--cell", "3,0"]).fails("表の外の行");
    b.run("show", [ORIGINAL, "t1", "--cell", "x"]).fails("--cell の形が違う");
    b.run("show", [ORIGINAL, "t2", "--cell", "1,0", "--rows", "0-3"]).fails("--cell と --rows の両方");
    for id in ["p2.1", "p2", "f1", "c1", "s1"] {
        b.run("show", [ORIGINAL, id, "--cell", "0,0"]).fails(&format!("表でない {id} に --cell"));
    }
    b.run("show", [ORIGINAL, "--file", "004_s1.2.json", "--cell", "1,0"]).fails("--file と --cell");
    // 実装が決めた仕様: --cell と --index を一緒に指定したらエラー
    b.run("show", [ORIGINAL, "t1", "--cell", "1,0", "--index", "0"]).fails("--cell と --index の両方");
    b.run("show", [ORIGINAL, "f1", "--cell", "1,0", "--index", "0"]).fails("--cell と --index の両方（図）");
    b.run("show", [ORIGINAL, "t1", "--cell", "1,0", "--index", "0", "--text"]).fails("--cell と --index の両方に --text");
}

#[test]
fn show_cell_and_index_end_with_newline_but_text_does_not() {
    // 実装が決めた仕様: --text 無しの --cell・--index は、これまでの show と同じく JSON の末尾に改行が付く。
    // --text のときは改行が足されない
    let b = Book::new();
    for args in [vec![ORIGINAL, "t1", "--cell", "1,0"], vec![ORIGINAL, "f1", "--index", "1"]] {
        let r = b.run("show", &args);
        r.ok();
        assert!(r.stdout_bytes.ends_with(b"\n"), "show {args:?} の JSON の末尾に改行が無い: {:?}", r.stdout);
        assert!(!r.stdout_bytes.ends_with(b"\n\n"), "show {args:?} の JSON の末尾の改行が 1 つでない: {:?}", r.stdout);
    }
    assert_text(&b, &[ORIGINAL, "t1", "--cell", "1,0", "--text"], "りんご");
    assert_text(&b, &[ORIGINAL, "f1", "--index", "1", "--text"], "矢印");
}

// ============================================================ show --index

#[test]
fn show_index_outputs_json_string() {
    let b = Book::new();
    let r = b.run("show", [ORIGINAL, "f1", "--index", "1"]);
    r.ok();
    assert_eq!(r.json(), json!("矢印"), "show --index の出力が JSON の文字列でない:\n{}", r.stdout);
    let r = b.run("show", [ORIGINAL, "f2", "--index", "3"]);
    r.ok();
    assert_eq!(r.json(), json!("出カ"));
}

#[test]
fn show_index_special_characters_as_json() {
    // JSON の文字列なので、エスケープされて出る（読み戻すと元の文字）
    let b = Book::new();
    for (i, want) in [("0", F3_0), ("1", F3_1)] {
        let r = b.run("show", [ORIGINAL, "f3", "--index", i]);
        r.ok();
        assert_eq!(r.json(), Value::from(want));
    }
}

#[test]
fn show_index_errors() {
    let b = Book::new();
    b.run("show", [ORIGINAL, "f1", "--index", "2"]).fails("配列の外");
    b.run("show", [ORIGINAL, "f2", "--index", "6"]).fails("配列の外");
    b.run("show", [ORIGINAL, "f1", "--index", "-1"]).fails("負の index");
    for id in ["t1", "p2.1", "p2", "c1", "s1"] {
        b.run("show", [ORIGINAL, id, "--index", "0"]).fails(&format!("図でない {id} に --index"));
    }
    b.run("show", [ORIGINAL, "--file", "002_s1.json", "--index", "0"]).fails("--file と --index");
}

// ============================================================ show --text

#[test]
fn show_text_sentence() {
    let b = Book::new();
    assert_text(&b, &[ORIGINAL, "p2.1", "--text"], "最初の文です。");
    assert_text(&b, &[ORIGINAL, "p7.1", "--text"], P7_1);
    assert_text(&b, &[ORIGINAL, "p7.2", "--text"], P7_2); // 末尾の空白と改行もそのまま
    assert_text(&b, &[ORIGINAL, "l1.1.2", "--text"], "項目の二文目です。");
}

#[test]
fn show_text_caption_cell_figure() {
    let b = Book::new();
    assert_text(&b, &[ORIGINAL, "c1", "--text"], "図1 テストの図");
    assert_text(&b, &[ORIGINAL, "c4", "--text"], C4);
    assert_text(&b, &[ORIGINAL, "t1", "--cell", "1,0", "--text"], "りんご");
    assert_text(&b, &[ORIGINAL, "t3", "--cell", "0,0", "--text"], T3_0_0);
    assert_text(&b, &[ORIGINAL, "t3", "--cell", "0,1", "--text"], T3_0_1);
    assert_text(&b, &[ORIGINAL, "f1", "--index", "0", "--text"], "図の中の文宇");
    assert_text(&b, &[ORIGINAL, "f3", "--index", "0", "--text"], F3_0);
    assert_text(&b, &[ORIGINAL, "f3", "--index", "1", "--text"], F3_1);
}

#[test]
fn show_text_paragraph_and_item_concatenate_sentences() {
    // パラグラフと、箇条書きの item は、センテンスの text を区切り文字なしで連結したもの
    let b = Book::new();
    assert_text(&b, &[ORIGINAL, "p2", "--text"], "最初の文です。二番目の文は曰本語です。三番目の文です。");
    assert_text(&b, &[ORIGINAL, "p7", "--text"], &[P7_1, P7_2].concat());
    assert_text(&b, &[ORIGINAL, "l1.1", "--text"], "一つ目の項目です。項目の二文目です。");
    assert_text(&b, &[ORIGINAL, "l1.2", "--text"], "二つ目の項目です。");
    assert_text(&b, &[ORIGINAL, "p3", "--text"], "この段落には文が一つしかありません。");
}

#[test]
fn show_text_follows_edits() {
    let b = Book::new();
    let e = b.new_edition(ORIGINAL, "fix");
    b.run("insert", [e.as_str(), "--first-in", "p2", "--text", "先頭。"]).ok();
    b.run("delete", [e.as_str(), "p2.3"]).ok();
    assert_text(&b, &[e.as_str(), "p2", "--text"], "先頭。最初の文です。二番目の文は曰本語です。");
    b.run("set-text", [e.as_str(), "f1", "--index", "1", "--text", "矢じるし"]).ok();
    assert_text(&b, &[e.as_str(), "f1", "--index", "1", "--text"], "矢じるし");
}

#[test]
fn show_text_errors() {
    let b = Book::new();
    let cases: Vec<(Vec<&str>, &str)> = vec![
        (vec![ORIGINAL, "s1", "--text"], "section"),
        (vec![ORIGINAL, "s1.2.1", "--text"], "入れ子の section"),
        (vec![ORIGINAL, "l1", "--text"], "list"),
        (vec![ORIGINAL, "t1", "--text"], "--cell の無い表"),
        (vec![ORIGINAL, "f1", "--text"], "--index の無い図"),
        (vec![ORIGINAL, "--file", "003_s1.1.json", "--text"], "--file"),
        (vec![ORIGINAL, "t2", "--rows", "0-3", "--text"], "--rows"),
        (vec![ORIGINAL, "t1", "--cell", "2,1", "--text"], "隠れたマス"),
        (vec![ORIGINAL, "f1", "--index", "5", "--text"], "配列の外"),
        (vec![ORIGINAL, "p999.1", "--text"], "無い ID"),
    ];
    for (args, why) in cases {
        let r = b.run("show", &args);
        r.fails(&format!("{why} に --text"));
        assert!(r.stdout_bytes.is_empty(), "{why} に --text でエラーなのに、標準出力に何か出ている: {:?}", r.stdout);
    }
}

// ============================================================ これまでの show は変わらない

#[test]
fn show_without_text_is_still_json() {
    let b = Book::new();
    let r = b.run("show", [ORIGINAL, "p7.1"]);
    r.ok();
    // JSON として読め、読み戻すと元の文字（出力には \" のようなエスケープが入っている）
    let v = r.json();
    let s = if v["id"] == "p7.1" { v.clone() } else { find_obj(&v, "p7.1").expect("p7.1 が無い") };
    assert_eq!(s["text"], P7_1);
    assert!(r.stdout.contains("\\\""), "--text 無しの show で、引用符がエスケープされていない:\n{}", r.stdout);
}

fn find_obj(v: &Value, id: &str) -> Option<Value> {
    match v {
        Value::Object(o) => {
            if o.get("id").and_then(Value::as_str) == Some(id) {
                return Some(v.clone());
            }
            o.values().find_map(|x| find_obj(x, id))
        }
        Value::Array(a) => a.iter().find_map(|x| find_obj(x, id)),
        _ => None,
    }
}

// ============================================================ 読むだけ

#[test]
fn show_variants_do_not_modify_anything() {
    let b = Book::new();
    let e = b.new_edition(ORIGINAL, "fix");
    b.run("set-text", [e.as_str(), "t3", "--cell", "0,0", "--text", "x"]).ok();
    let x = b.snapshot(ORIGINAL);
    let y = b.snapshot(&e);
    for ed in [ORIGINAL, e.as_str()] {
        b.run("show", [ed, "t3", "--cell", "0,0"]).ok();
        b.run("show", [ed, "t3", "--cell", "0,1", "--text"]).ok();
        b.run("show", [ed, "f3", "--index", "1"]).ok();
        b.run("show", [ed, "f3", "--index", "1", "--text"]).ok();
        b.run("show", [ed, "p7", "--text"]).ok();
        b.run("show", [ed, "c4", "--text"]).ok();
        b.run("show", [ed, "s1", "--text"]); // エラーになるものも
    }
    assert!(b.snapshot(ORIGINAL) == x, "show で original が変わった");
    assert!(b.snapshot(&e) == y, "show で {e} が変わった");
    assert_eq!(b.edition_dirs(), vec![e.clone(), ORIGINAL.to_string()], "show で版のフォルダが増えた");
}
