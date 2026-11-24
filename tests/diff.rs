//! 版どうしの差分（`diff`）のテスト。
//!
//! 出力は Markdown で、文言は実装が決める。
//! ここでは、出る（または出ない）ID と、直す前の文・直した後の文・note が含まれるかだけを確かめる。

mod common;

use common::*;

fn setup() -> (Book, String) {
    let b = Book::new();
    let e = b.new_edition(ORIGINAL, "fix");
    (b, e)
}

fn diff(b: &Book, x: &str, y: &str) -> String {
    let r = b.run("diff", [x, y]);
    r.ok();
    r.stdout.clone()
}

#[test]
fn diff_same_edition_is_empty() {
    let b = Book::new();
    let out = diff(&b, ORIGINAL, ORIGINAL);
    for (id, _) in SENTENCES {
        assert!(!mentions_id(&out, id), "同じ版どうしの diff に {id} が出ている:\n{out}");
    }
}

#[test]
fn diff_shows_changed_sentence_with_before_after_and_note() {
    let (b, e) = setup();
    b.run("set-text", [e.as_str(), "p2.2", "--text", "二番目の文は日本語です。", "--note", "「曰」を「日」に修正"]).ok();
    let out = diff(&b, ORIGINAL, &e);
    assert!(mentions_id(&out, "p2.2"), "diff に p2.2 が無い:\n{out}");
    assert!(out.contains("二番目の文は曰本語です。"), "diff に直す前の文が無い:\n{out}");
    assert!(out.contains("二番目の文は日本語です。"), "diff に直した後の文が無い:\n{out}");
    assert!(out.contains("「曰」を「日」に修正"), "diff に note が無い:\n{out}");
    // 直していない文は出ない
    for id in ["p2.1", "p2.3", "p1.1", "l1.1.1", "p6.1"] {
        assert!(!mentions_id(&out, id), "直していない {id} が diff に出ている:\n{out}");
    }
}

#[test]
fn diff_shows_changed_list_item_caption_and_cell() {
    let (b, e) = setup();
    b.run("set-text", [e.as_str(), "l1.1.2", "--text", "項目の二つ目の文です。"]).ok();
    b.run("set-text", [e.as_str(), "c1", "--text", "図1 試験の図"]).ok();
    b.run("set-text", [e.as_str(), "t1", "--cell", "1,0", "--text", "みかん"]).ok();
    let out = diff(&b, ORIGINAL, &e);
    assert!(mentions_id(&out, "l1.1.2"), "diff に l1.1.2 が無い:\n{out}");
    assert!(out.contains("項目の二つ目の文です。"));
    assert!(mentions_id(&out, "c1"), "diff に c1 が無い:\n{out}");
    assert!(out.contains("図1 試験の図") && out.contains("図1 テストの図"));
    assert!(mentions_id(&out, "t1"), "diff に t1 が無い:\n{out}");
    assert!(out.contains("みかん") && out.contains("りんご"));
}

#[test]
fn diff_shows_added_sentence() {
    let (b, e) = setup();
    b.run("insert", [e.as_str(), "--after", "p2.1", "--text", "足した文です。", "--note", "脱落"]).ok();
    let out = diff(&b, ORIGINAL, &e);
    assert!(mentions_id(&out, "p2.4"), "diff に足した p2.4 が無い:\n{out}");
    assert!(out.contains("足した文です。"));
    assert!(out.contains("脱落"), "diff に note が無い:\n{out}");
}

#[test]
fn diff_shows_deleted_sentence() {
    let (b, e) = setup();
    b.run("delete", [e.as_str(), "p2.3", "--note", "重複"]).ok();
    let out = diff(&b, ORIGINAL, &e);
    assert!(mentions_id(&out, "p2.3"), "diff に消した p2.3 が無い:\n{out}");
    assert!(out.contains("三番目の文です。"), "diff に消した文の中身が無い:\n{out}");
    assert!(out.contains("重複"), "diff に消した理由（retired_notes）が無い:\n{out}");
}

#[test]
fn diff_shows_split_sentence() {
    let (b, e) = setup();
    b.run("split", [e.as_str(), "p2.2", "--text", "二番目の文は。", "--text", "曰本語です。"]).ok();
    let out = diff(&b, ORIGINAL, &e);
    assert!(mentions_id(&out, "p2.2"), "diff に p2.2 が無い:\n{out}");
    assert!(mentions_id(&out, "p2.4"), "diff に分けた p2.4 が無い:\n{out}");
    assert!(out.contains("二番目の文は曰本語です。"));
}

/// Markdown の出力から、見出しの行に `id` がある項目の本文（次の見出しの行の手前まで）を取り出す。
fn item_block(out: &str, id: &str) -> String {
    let lines: Vec<&str> = out.lines().collect();
    let start = lines
        .iter()
        .position(|l| l.trim_start().starts_with('#') && mentions_id(l, id))
        .unwrap_or_else(|| panic!("diff に {id} の見出しが無い:\n{out}"));
    let end = lines[start + 1..]
        .iter()
        .position(|l| l.trim_start().starts_with('#'))
        .map(|k| start + 1 + k)
        .unwrap_or(lines.len());
    lines[start + 1..end].join("\n")
}

#[test]
fn diff_shows_split_source_for_added_sentences() {
    // split で足された文には、分けた元の ID が出る
    let (b, e) = setup();
    b.run("split", [e.as_str(), "p4.1", "--text", "表の", "--text", "前の", "--text", "段落です。", "--note", "n"]).ok();
    let out = diff(&b, ORIGINAL, &e);
    for added in ["p4.3", "p4.4"] {
        let block = item_block(&out, added);
        assert!(mentions_id(&block, "p4.1"), "split で足した {added} の項目に、分けた元 p4.1 が出ていない:\n{block}");
    }
}

#[test]
fn diff_shows_split_source_in_list_item() {
    let (b, e) = setup();
    b.run("split", [e.as_str(), "l1.1.2", "--text", "項目の", "--text", "二文目です。"]).ok();
    let out = diff(&b, ORIGINAL, &e);
    let block = item_block(&out, "l1.1.3");
    assert!(mentions_id(&block, "l1.1.2"), "split で足した l1.1.3 の項目に、分けた元 l1.1.2 が出ていない:\n{block}");
}

#[test]
fn diff_does_not_show_split_source_for_inserted_sentence() {
    // insert で足した文の origin.ids は空なので、分けた元は出ない
    let (b, e) = setup();
    b.run("insert", [e.as_str(), "--after", "p2.1", "--text", "足した文です。", "--note", "脱落"]).ok();
    let out = diff(&b, ORIGINAL, &e);
    let block = item_block(&out, "p2.4");
    assert!(block.contains("足した文です。"), "p2.4 の項目に足した文が無い:\n{block}");
    for (id, _) in SENTENCES {
        assert!(!mentions_id(&block, id), "insert で足した p2.4 の項目に {id} が出ている:\n{block}");
    }
}

#[test]
fn diff_split_and_insert_together() {
    // 同じ版で split と insert をしたとき、split の文にだけ分けた元が出る
    let (b, e) = setup();
    b.run("split", [e.as_str(), "p5.1", "--text", "入れ子の。", "--text", "本文です。"]).ok();
    b.run("insert", [e.as_str(), "--after", "p5.2", "--text", "足した文です。"]).ok();
    let out = diff(&b, ORIGINAL, &e);
    assert!(mentions_id(&item_block(&out, "p5.3"), "p5.1"), "split で足した p5.3 に分けた元が無い:\n{out}");
    let block = item_block(&out, "p5.4");
    for id in ["p5.1", "p5.2", "p5.3"] {
        assert!(!mentions_id(&block, id), "insert で足した p5.4 の項目に {id} が出ている:\n{block}");
    }
}

#[test]
fn diff_reversed_direction() {
    // 版 A と版 B を入れ替えても動き、足した・消したが入れ替わる（出る ID は同じ）
    let (b, e) = setup();
    b.run("insert", [e.as_str(), "--after", "p2.1", "--text", "足した文です。"]).ok();
    let out = diff(&b, &e, ORIGINAL);
    assert!(mentions_id(&out, "p2.4"), "逆向きの diff に p2.4 が無い:\n{out}");
}

#[test]
fn diff_between_stacked_editions() {
    let b = Book::new();
    let a = b.new_edition(ORIGINAL, "a");
    b.run("set-text", [a.as_str(), "p2.2", "--text", "一回目。"]).ok();
    let bb = b.new_edition(&a, "b");
    b.run("set-text", [bb.as_str(), "p4.1", "--text", "二回目。"]).ok();

    // 01_a → 02_b では、02_b で直した p4.1 だけが出る
    let out = diff(&b, &a, &bb);
    assert!(mentions_id(&out, "p4.1"), "diff に p4.1 が無い:\n{out}");
    assert!(!mentions_id(&out, "p2.2"), "01_a で直した p2.2 が、01_a → 02_b の diff に出ている:\n{out}");

    // original → 02_b では、両方が出る（ID で対応を取る）
    let out = diff(&b, ORIGINAL, &bb);
    assert!(mentions_id(&out, "p2.2") && mentions_id(&out, "p4.1"), "diff に p2.2 と p4.1 が無い:\n{out}");
}

#[test]
fn diff_rejects_unknown_editions() {
    let b = Book::new();
    b.run("diff", [ORIGINAL, "99_nothing"]).fails("無い版");
    b.run("diff", ["../x", ORIGINAL]).fails("不正な版の名前");
}

#[test]
fn diff_does_not_modify_anything() {
    let (b, e) = setup();
    b.run("set-text", [e.as_str(), "p2.2", "--text", "x。"]).ok();
    let x = b.snapshot(ORIGINAL);
    let y = b.snapshot(&e);
    diff(&b, ORIGINAL, &e);
    assert!(b.snapshot(ORIGINAL) == x && b.snapshot(&e) == y, "diff が版を書き換えた");
}
