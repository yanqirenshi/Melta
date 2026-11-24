//! 編集するコマンド（`set-text`、`insert`、`split`、`delete`）のテスト。

mod common;

use common::*;
use serde_json::{json, Value};

/// original から 01_fix を作る。
fn setup() -> (Book, String) {
    let b = Book::new();
    let e = b.new_edition(ORIGINAL, "fix");
    (b, e)
}

/// 版のすべての要素（ID → 要素）から、origin と note を除いたもの。
fn strip(el: &Value) -> Value {
    let mut v = el.clone();
    if let Some(o) = v.as_object_mut() {
        o.remove("origin");
        o.remove("note");
    }
    v
}

/// 約束 3: 直した要素、足した要素、消した要素と、それらを含む入れ物のほかは、ID も中身も変わらない。
fn assert_others_unchanged(b: &Book, before: &str, after: &str, touched: &[&str]) {
    let x = b.all_elements(before);
    let y = b.all_elements(after);
    // 入れ物（パラグラフ、項目、箇条書き、セクション）は中身が変わるので、子でない要素だけ比べる
    let is_leaf = |v: &Value| !(v.get("sentences").is_some() || v.get("items").is_some() || v.get("children").is_some());
    for (id, el) in &x {
        if touched.contains(&id.as_str()) {
            continue;
        }
        let other = y.get(id).unwrap_or_else(|| panic!("編集していない {id} が {after} から消えた"));
        if is_leaf(el) {
            assert_eq!(strip(el), strip(other), "編集していない {id} の中身が変わった");
        }
    }
}

// ============================================================ set-text

#[test]
fn set_text_paragraph_sentence() {
    let (b, e) = setup();
    b.run("set-text", [e.as_str(), "p2.2", "--text", "二番目の文は日本語です。", "--note", "「曰」を「日」に修正"]).ok();
    let s = b.get(&e, "p2.2");
    assert_eq!(s["text"], "二番目の文は日本語です。");
    assert_eq!(
        s["origin"],
        json!({"edition": "original", "ids": ["p2.2"], "text": "二番目の文は曰本語です。"})
    );
    assert_eq!(s["note"], "「曰」を「日」に修正");
    assert_eq!(b.sentence_ids(&e, "p2"), ["p2.1", "p2.2", "p2.3"]);
    assert_others_unchanged(&b, ORIGINAL, &e, &["p2.2"]);
    b.check_ok(&e);
}

#[test]
fn set_text_without_note() {
    let (b, e) = setup();
    b.run("set-text", [e.as_str(), "p2.2", "--text", "二番目の文は日本語です。"]).ok();
    let s = b.get(&e, "p2.2");
    assert_eq!(s["text"], "二番目の文は日本語です。");
    assert_eq!(s["origin"]["text"], "二番目の文は曰本語です。");
    assert!(s.get("note").is_none(), "--note を付けていないのに note のプロパティがある（null も不可）: {s}");
    b.check_ok(&e);
}

#[test]
fn insert_and_split_without_note() {
    let (b, e) = setup();
    b.run("insert", [e.as_str(), "--after", "p2.1", "--text", "足した。"]).ok();
    b.run("split", [e.as_str(), "p4.1", "--text", "a。", "--text", "b。"]).ok();
    for id in ["p2.4", "p4.1", "p4.3"] {
        let s = b.get(&e, id);
        assert!(s.get("origin").is_some(), "{id} に origin が無い: {s}");
        assert!(s.get("note").is_none(), "--note を付けていないのに {id} に note のプロパティがある（null も不可）: {s}");
    }
    b.check_ok(&e);
}

#[test]
fn set_text_list_item_sentence() {
    let (b, e) = setup();
    b.run("set-text", [e.as_str(), "l1.1.2", "--text", "項目の二つ目の文です。", "--note", "語順"]).ok();
    let s = b.get(&e, "l1.1.2");
    assert_eq!(s["text"], "項目の二つ目の文です。");
    assert_eq!(
        s["origin"],
        json!({"edition": "original", "ids": ["l1.1.2"], "text": "項目の二文目です。"})
    );
    assert_eq!(s["note"], "語順");
    assert_others_unchanged(&b, ORIGINAL, &e, &["l1.1.2"]);
    b.check_ok(&e);
}

#[test]
fn set_text_caption() {
    let (b, e) = setup();
    b.run("set-text", [e.as_str(), "c1", "--text", "図1 試験の図", "--note", "用語"]).ok();
    let c = b.get(&e, "c1");
    assert_eq!(c["text"], "図1 試験の図");
    assert_eq!(c["origin"]["edition"], ORIGINAL);
    assert_eq!(c["origin"]["ids"], json!(["c1"]));
    assert_eq!(c["origin"]["text"], "図1 テストの図");
    assert_eq!(c["note"], "用語");
    assert_others_unchanged(&b, ORIGINAL, &e, &["c1"]);
    b.check_ok(&e);
}

#[test]
fn set_text_figure_text() {
    // --index n で、図の text の配列の n 番目（0 始まり）を直す。
    // origin は図の要素に 1 つで、origin.text は直す前の配列の全体。
    // 理由は note ではなく notes（キーが index の文字列、値が理由）に入る
    let (b, e) = setup();
    b.run("set-text", [e.as_str(), "f1", "--index", "0", "--text", "図の中の文字", "--note", "「宇」を「字」に修正"]).ok();
    let f = b.get(&e, "f1");
    assert_eq!(f["text"], json!(["図の中の文字", "矢印"]));
    assert_eq!(f["src"], "figures/p002_1_0.png");
    assert_eq!(
        f["origin"],
        json!({"edition": "original", "ids": ["f1"], "text": ["図の中の文宇", "矢印"]})
    );
    assert_eq!(f["notes"], json!({"0": "「宇」を「字」に修正"}));
    assert_no_note(&f);
    assert_others_unchanged(&b, ORIGINAL, &e, &["f1"]);
    b.check_ok(&e);

    // 2 つ目を --note 無しで直しても、origin は最初のまま（base の配列の全体）。
    // --note を付けずに直した index は、notes にキーを作らない
    b.run("set-text", [e.as_str(), "f1", "--index", "1", "--text", "矢じるし"]).ok();
    let f = b.get(&e, "f1");
    assert_eq!(f["text"], json!(["図の中の文字", "矢じるし"]));
    assert_eq!(f["origin"]["text"], json!(["図の中の文宇", "矢印"]));
    assert_eq!(f["notes"], json!({"0": "「宇」を「字」に修正"}));
    assert_no_note(&f);
    b.check_ok(&e);
}

/// 図には note を付けない（理由は notes に入る）。
fn assert_no_note(f: &Value) {
    assert!(f.get("note").is_none(), "図に note が付いている（notes に入るはず）: {f}");
}

#[test]
fn set_text_figure_back_to_base_removes_origin_and_notes() {
    let (b, e) = setup();
    b.run("set-text", [e.as_str(), "f1", "--index", "0", "--text", "図の中の文字", "--note", "n"]).ok();
    b.run("set-text", [e.as_str(), "f1", "--index", "1", "--text", "矢じるし", "--note", "m"]).ok();
    // index 0 だけ戻すと、index 0 の理由だけが消え、配列が base と違うので origin は残る
    b.run("set-text", [e.as_str(), "f1", "--index", "0", "--text", "図の中の文宇"]).ok();
    let f = b.get(&e, "f1");
    assert!(f.get("origin").is_some(), "配列がまだ base と違うのに origin が消えた");
    assert_eq!(f["notes"], json!({"1": "m"}));
    // 配列の全体が base と同じに戻ったら、origin と notes は消える
    b.run("set-text", [e.as_str(), "f1", "--index", "1", "--text", "矢印"]).ok();
    let f = b.get(&e, "f1");
    assert_eq!(f["text"], json!(["図の中の文宇", "矢印"]));
    assert!(f.get("origin").is_none(), "base の配列に戻ったのに origin が残っている: {f}");
    assert!(f.get("notes").is_none(), "base の配列に戻ったのに notes が残っている: {f}");
    assert_no_note(&f);
    b.check_ok(&e);
}

#[test]
fn figure_notes_keep_reasons_for_each_index() {
    // 今回の改善のきっかけ: 同じ図の 2 つの index を別々の理由で直したら、両方の理由が残る
    let (b, e) = setup();
    b.run("set-text", [e.as_str(), "f2", "--index", "0", "--text", "1.", "--note", "項番の「.」を補った"]).ok();
    b.run("set-text", [e.as_str(), "f2", "--index", "3", "--text", "出力", "--note", "「カ」を「力」に修正"]).ok();
    let f = b.get(&e, "f2");
    assert_eq!(f["text"], json!(["1.", "入力", "2", "出力", "3.", "保存"]));
    assert_eq!(f["notes"], json!({"0": "項番の「.」を補った", "3": "「カ」を「力」に修正"}));
    assert_eq!(
        f["origin"],
        json!({"edition": "original", "ids": ["f2"], "text": ["1", "入力", "2", "出カ", "3.", "保存"]})
    );
    assert_no_note(&f);
    b.check_ok(&e);

    // 3 つ目の index を直しても、前の 2 つの理由は残る
    b.run("set-text", [e.as_str(), "f2", "--index", "2", "--text", "2.", "--note", "項番の「.」を補った（2）"]).ok();
    assert_eq!(
        b.get(&e, "f2")["notes"],
        json!({"0": "項番の「.」を補った", "2": "項番の「.」を補った（2）", "3": "「カ」を「力」に修正"})
    );
    b.check_ok(&e);
}

#[test]
fn figure_notes_overwrite_and_keep() {
    // 同じ index を --note 付きで直し直したら上書き、--note 無しなら前の理由が残る
    let (b, e) = setup();
    b.run("set-text", [e.as_str(), "f2", "--index", "3", "--text", "出刀", "--note", "n1"]).ok();
    b.run("set-text", [e.as_str(), "f2", "--index", "3", "--text", "出力", "--note", "n2"]).ok();
    assert_eq!(b.get(&e, "f2")["notes"], json!({"3": "n2"}));
    b.run("set-text", [e.as_str(), "f2", "--index", "3", "--text", "出力。"]).ok();
    let f = b.get(&e, "f2");
    assert_eq!(f["text"][3], "出力。");
    assert_eq!(f["notes"], json!({"3": "n2"}), "--note 無しで直し直したら、前の理由が残る");
    assert_eq!(f["origin"]["text"][3], "出カ", "origin は最初のまま");
    b.check_ok(&e);
}

#[test]
fn figure_edit_without_note_has_no_notes() {
    // --note を付けずに直した index は notes にキーを作らない。キーが無ければ notes のプロパティも無い
    let (b, e) = setup();
    b.run("set-text", [e.as_str(), "f2", "--index", "2", "--text", "2."]).ok();
    let f = b.get(&e, "f2");
    assert_eq!(f["text"][2], "2.");
    assert!(f.get("origin").is_some());
    assert!(f.get("notes").is_none(), "--note を付けていないのに notes がある（空のオブジェクトも不可）: {f}");
    assert_no_note(&f);
    b.check_ok(&e);
}

#[test]
fn figure_notes_disappear_when_empty() {
    // 理由のある index を base に戻し、notes が空になったら notes のプロパティが消える。
    // 理由の無い index がまだ base と違うので、origin は残る
    let (b, e) = setup();
    b.run("set-text", [e.as_str(), "f2", "--index", "0", "--text", "1.", "--note", "n0"]).ok();
    b.run("set-text", [e.as_str(), "f2", "--index", "2", "--text", "2."]).ok();
    b.run("set-text", [e.as_str(), "f2", "--index", "0", "--text", "1"]).ok();
    let f = b.get(&e, "f2");
    assert_eq!(f["text"], json!(["1", "入力", "2.", "出カ", "3.", "保存"]));
    assert!(f.get("notes").is_none(), "notes が空になったのに、notes のプロパティが残っている: {f}");
    assert!(f.get("origin").is_some(), "配列がまだ base と違うのに origin が消えた: {f}");
    b.check_ok(&e);
}

#[test]
fn figure_index_back_to_base_with_note_removes_only_that_key() {
    // base に戻すときに --note を付けても、その index のキーは消える
    let (b, e) = setup();
    b.run("set-text", [e.as_str(), "f2", "--index", "0", "--text", "1.", "--note", "n0"]).ok();
    b.run("set-text", [e.as_str(), "f2", "--index", "3", "--text", "出力", "--note", "n3"]).ok();
    b.run("set-text", [e.as_str(), "f2", "--index", "0", "--text", "1", "--note", "戻した"]).ok();
    assert_eq!(b.get(&e, "f2")["notes"], json!({"3": "n3"}));
    b.check_ok(&e);
}

#[test]
fn set_text_figure_needs_valid_index() {
    let (b, e) = setup();
    let before = b.snapshot(&e);
    b.run("set-text", [e.as_str(), "f1", "--text", "x"]).fails("図に --index が無い");
    b.run("set-text", [e.as_str(), "f1", "--index", "2", "--text", "x"]).fails("--index が配列の外");
    assert!(b.snapshot(&e) == before, "失敗したのに版が変わった");
}

#[test]
fn set_text_table_cell() {
    let (b, e) = setup();
    b.run("set-text", [e.as_str(), "t1", "--cell", "1,0", "--text", "みかん", "--note", "誤認識"]).ok();
    let t = b.get(&e, "t1");
    let cells = t["cells"].as_array().unwrap();
    assert_eq!(cells.len(), 5, "セルの数が変わった");
    let c = cells.iter().find(|c| c["row"] == 1 && c["col"] == 0).unwrap();
    assert_eq!(c["text"], "みかん");
    // origin と note は、セルのオブジェクトに付く
    assert_eq!(c["origin"], json!({"edition": "original", "ids": ["t1"], "text": "りんご"}));
    assert_eq!(c["note"], "誤認識");
    assert!(t.get("origin").is_none() && t.get("note").is_none(), "表の要素に origin・note が付いている: {t}");
    // ほかのセルは元のまま
    let base = b.get(ORIGINAL, "t1");
    for (x, y) in base["cells"].as_array().unwrap().iter().zip(cells) {
        if x["row"] == 1 && x["col"] == 0 {
            continue;
        }
        assert_eq!(strip(x), strip(y));
    }
    assert_eq!(t["n_row"], 3);
    assert_eq!(t["n_col"], 2);
    assert_others_unchanged(&b, ORIGINAL, &e, &["t1"]);
    b.check_ok(&e);
}

#[test]
fn set_text_table_cell_in_merged_area_fails() {
    // (2,1) は (2,0) の結合で隠れたマス。セルが無いので直せない
    let (b, e) = setup();
    let before = b.snapshot(&e);
    b.run("set-text", [e.as_str(), "t1", "--cell", "2,1", "--text", "x"]).fails("セルの無いマス");
    b.run("set-text", [e.as_str(), "t1", "--cell", "9,9", "--text", "x"]).fails("表の外のマス");
    assert!(b.snapshot(&e) == before, "失敗したのに版が変わった");
}

#[test]
fn set_text_on_container_fails() {
    // 本文はセンテンスにある。パラグラフ・項目・箇条書き・セクションの set-text はできない
    let (b, e) = setup();
    let before = b.snapshot(&e);
    for id in ["p2", "l1", "l1.1", "s1"] {
        b.run("set-text", [e.as_str(), id, "--text", "x"]).fails(&format!("{id} に set-text"));
    }
    b.run("set-text", [e.as_str(), "p999.1", "--text", "x"]).fails("無い ID");
    assert!(b.snapshot(&e) == before, "失敗したのに版が変わった");
}

#[test]
fn set_text_twice_keeps_first_origin() {
    let (b, e) = setup();
    b.run("set-text", [e.as_str(), "p2.2", "--text", "一回目。", "--note", "n1"]).ok();
    b.run("set-text", [e.as_str(), "p2.2", "--text", "二回目。", "--note", "n2"]).ok();
    let s = b.get(&e, "p2.2");
    assert_eq!(s["text"], "二回目。");
    assert_eq!(
        s["origin"],
        json!({"edition": "original", "ids": ["p2.2"], "text": "二番目の文は曰本語です。"}),
        "2 回直しても、origin は最初のまま（base の文）"
    );
    assert_eq!(s["note"], "n2", "--note を付けたら note は上書き");
    b.check_ok(&e);
}

#[test]
fn set_text_twice_without_note_keeps_note() {
    let (b, e) = setup();
    b.run("set-text", [e.as_str(), "p2.2", "--text", "一回目。", "--note", "n1"]).ok();
    b.run("set-text", [e.as_str(), "p2.2", "--text", "二回目。"]).ok();
    let s = b.get(&e, "p2.2");
    assert_eq!(s["text"], "二回目。");
    assert_eq!(s["note"], "n1", "--note を付けなければ、前の note が残る");
    assert_eq!(s["origin"]["text"], "二番目の文は曰本語です。");
    b.check_ok(&e);
}

#[test]
fn set_text_table_cell_twice_and_back() {
    let (b, e) = setup();
    b.run("set-text", [e.as_str(), "t1", "--cell", "1,0", "--text", "みかん", "--note", "n1"]).ok();
    b.run("set-text", [e.as_str(), "t1", "--cell", "1,0", "--text", "ぶどう", "--note", "n2"]).ok();
    let cell = |b: &Book| -> Value {
        b.get(&e, "t1")["cells"].as_array().unwrap().iter().find(|c| c["row"] == 1 && c["col"] == 0).unwrap().clone()
    };
    let c = cell(&b);
    assert_eq!(c["text"], "ぶどう");
    assert_eq!(c["origin"], json!({"edition": "original", "ids": ["t1"], "text": "りんご"}));
    assert_eq!(c["note"], "n2");
    b.run("set-text", [e.as_str(), "t1", "--cell", "1,0", "--text", "りんご"]).ok();
    let c = cell(&b);
    assert_eq!(c["text"], "りんご");
    assert!(c.get("origin").is_none() && c.get("note").is_none(), "base のセルに戻ったのに origin・note が残っている: {c}");
    b.check_ok(&e);
}

#[test]
fn set_text_back_to_base_removes_origin_and_note() {
    let (b, e) = setup();
    b.run("set-text", [e.as_str(), "p2.2", "--text", "一回目。", "--note", "n1"]).ok();
    b.run("set-text", [e.as_str(), "p2.2", "--text", "二番目の文は曰本語です。", "--note", "戻す"]).ok();
    let s = b.get(&e, "p2.2");
    assert_eq!(s["text"], "二番目の文は曰本語です。");
    assert!(s.get("origin").is_none(), "base の文に戻ったのに origin が残っている: {s}");
    assert!(s.get("note").is_none(), "base の文に戻ったのに note が残っている: {s}");
    b.check_ok(&e);
    // 戻した後は、元の版との diff に出ない
    let r = b.run("diff", [ORIGINAL, e.as_str()]);
    r.ok();
    assert!(!mentions_id(&r.stdout, "p2.2"), "元に戻したのに diff に p2.2 が出ている:\n{}", r.stdout);
}

#[test]
fn origin_points_to_previous_edition() {
    // origin は 1 つ前の版（base）との対応
    let b = Book::new();
    let a = b.new_edition(ORIGINAL, "a");
    b.run("set-text", [a.as_str(), "p2.2", "--text", "一回目。"]).ok();
    let bb = b.new_edition(&a, "b");
    b.run("set-text", [bb.as_str(), "p2.2", "--text", "二回目。", "--note", "n"]).ok();
    assert_eq!(
        b.get(&bb, "p2.2")["origin"],
        json!({"edition": "01_a", "ids": ["p2.2"], "text": "一回目。"})
    );
    b.check_ok(&bb);
}

// ============================================================ insert

#[test]
fn insert_after() {
    let (b, e) = setup();
    b.run("insert", [e.as_str(), "--after", "p2.1", "--text", "足した文です。", "--note", "脱落"]).ok();
    // 番号は p2 でこれまでに使った最大（3）+ 1。並びは配列の順
    assert_eq!(b.sentence_ids(&e, "p2"), ["p2.1", "p2.4", "p2.2", "p2.3"]);
    let s = b.get(&e, "p2.4");
    assert_eq!(s["text"], "足した文です。");
    assert_eq!(s["origin"]["edition"], ORIGINAL);
    assert_eq!(s["origin"]["ids"], json!([]));
    assert_eq!(s["note"], "脱落");
    assert_others_unchanged(&b, ORIGINAL, &e, &[]);
    b.check_ok(&e);
}

#[test]
fn insert_after_last() {
    let (b, e) = setup();
    b.run("insert", [e.as_str(), "--after", "p2.3", "--text", "最後に足した文です。"]).ok();
    assert_eq!(b.sentence_ids(&e, "p2"), ["p2.1", "p2.2", "p2.3", "p2.4"]);
    b.check_ok(&e);
}

#[test]
fn insert_first_in_paragraph() {
    let (b, e) = setup();
    b.run("insert", [e.as_str(), "--first-in", "p2", "--text", "先頭の文です。"]).ok();
    assert_eq!(b.sentence_ids(&e, "p2"), ["p2.4", "p2.1", "p2.2", "p2.3"]);
    assert_eq!(b.get(&e, "p2.4")["origin"], json!({"edition": "original", "ids": []}));
    b.check_ok(&e);
}

#[test]
fn insert_first_in_list_item() {
    let (b, e) = setup();
    b.run("insert", [e.as_str(), "--first-in", "l1.2", "--text", "項目の先頭の文です。"]).ok();
    assert_eq!(b.sentence_ids(&e, "l1.2"), ["l1.2.2", "l1.2.1"]);
    b.run("insert", [e.as_str(), "--after", "l1.1.1", "--text", "項目に足した文です。"]).ok();
    assert_eq!(b.sentence_ids(&e, "l1.1"), ["l1.1.1", "l1.1.3", "l1.1.2"]);
    assert_others_unchanged(&b, ORIGINAL, &e, &[]);
    b.check_ok(&e);
}

#[test]
fn insert_numbers_keep_growing() {
    let (b, e) = setup();
    b.run("insert", [e.as_str(), "--after", "p2.1", "--text", "一つ目。"]).ok();
    b.run("insert", [e.as_str(), "--after", "p2.1", "--text", "二つ目。"]).ok();
    // 番号は ID の大小ではなく、使った番号の最大 + 1。並びは配列の順
    assert_eq!(b.sentence_ids(&e, "p2"), ["p2.1", "p2.5", "p2.4", "p2.2", "p2.3"]);
    b.check_ok(&e);
}

#[test]
fn insert_in_other_file_section() {
    // section_ref の先のファイルにあるパラグラフにも足せる
    let (b, e) = setup();
    b.run("insert", [e.as_str(), "--after", "p5.1", "--text", "入れ子に足した文。"]).ok();
    assert_eq!(b.sentence_ids(&e, "p5"), ["p5.1", "p5.3", "p5.2"]);
    b.check_ok(&e);
}

#[test]
fn insert_bad_target_fails() {
    let (b, e) = setup();
    let before = b.snapshot(&e);
    b.run("insert", [e.as_str(), "--after", "p999.1", "--text", "x"]).fails("無いセンテンス");
    b.run("insert", [e.as_str(), "--after", "p2", "--text", "x"]).fails("--after にパラグラフ");
    b.run("insert", [e.as_str(), "--first-in", "p2.1", "--text", "x"]).fails("--first-in にセンテンス");
    b.run("insert", [e.as_str(), "--first-in", "t1", "--text", "x"]).fails("--first-in に表");
    b.run("insert", [e.as_str(), "--first-in", "l1", "--text", "x"]).fails("--first-in に箇条書き（項目ではない）");
    b.run("insert", [e.as_str(), "--text", "x"]).fails("--after も --first-in も無い");
    b.run("insert", [e.as_str(), "--after", "p2.1", "--first-in", "p2", "--text", "x"])
        .fails("--after と --first-in の両方");
    assert!(b.snapshot(&e) == before, "失敗したのに版が変わった");
}

// ============================================================ split

#[test]
fn split_sentence() {
    let (b, e) = setup();
    b.run("split", [e.as_str(), "p2.2", "--text", "二番目の文は。", "--text", "曰本語です。", "--note", "区切りの誤り"]).ok();
    assert_eq!(b.sentence_ids(&e, "p2"), ["p2.1", "p2.2", "p2.4", "p2.3"]);
    let first = b.get(&e, "p2.2");
    let second = b.get(&e, "p2.4");
    assert_eq!(first["text"], "二番目の文は。");
    assert_eq!(second["text"], "曰本語です。");
    for s in [&first, &second] {
        assert_eq!(
            s["origin"],
            json!({"edition": "original", "ids": ["p2.2"], "text": "二番目の文は曰本語です。"}),
            "分けた文は、すべてが元の ID と元の文の全体を持つ: {s}"
        );
        assert_eq!(s["note"], "区切りの誤り");
    }
    assert_others_unchanged(&b, ORIGINAL, &e, &["p2.2"]);
    b.check_ok(&e);
}

#[test]
fn split_into_three_in_list_item() {
    let (b, e) = setup();
    b.run("split", [e.as_str(), "l1.1.1", "--text", "一つ目の", "--text", "項目", "--text", "です。"]).ok();
    assert_eq!(b.sentence_ids(&e, "l1.1"), ["l1.1.1", "l1.1.3", "l1.1.4", "l1.1.2"]);
    for id in ["l1.1.1", "l1.1.3", "l1.1.4"] {
        assert_eq!(
            b.get(&e, id)["origin"],
            json!({"edition": "original", "ids": ["l1.1.1"], "text": "一つ目の項目です。"})
        );
    }
    b.check_ok(&e);
}

#[test]
fn split_needs_two_texts() {
    let (b, e) = setup();
    let before = b.snapshot(&e);
    b.run("split", [e.as_str(), "p2.2", "--text", "一つだけ。"]).fails("--text が 1 つ");
    b.run("split", [e.as_str(), "p2", "--text", "a", "--text", "b"]).fails("パラグラフを split");
    b.run("split", [e.as_str(), "p999.1", "--text", "a", "--text", "b"]).fails("無い ID");
    assert!(b.snapshot(&e) == before, "失敗したのに版が変わった");
}

// ============================================================ delete

#[test]
fn delete_sentence() {
    let (b, e) = setup();
    b.run("delete", [e.as_str(), "p2.2", "--note", "重複"]).ok();
    assert_eq!(b.sentence_ids(&e, "p2"), ["p2.1", "p2.3"]);
    assert!(b.find(&e, "p2.2").is_none());
    assert_eq!(b.retired_ids(&e), vec!["p2.2".to_string()]);
    assert_others_unchanged(&b, ORIGINAL, &e, &["p2.2"]);
    b.check_ok(&e);
}

#[test]
fn delete_note_goes_to_retired_notes() {
    // 実装が決めた仕様: delete の --note は index.json の edition.retired_notes（ID → 理由）に入る
    let (b, e) = setup();
    b.run("delete", [e.as_str(), "p2.2", "--note", "重複"]).ok();
    b.run("delete", [e.as_str(), "l1.1.1"]).ok();
    let ed = &b.index(&e)["edition"];
    assert_eq!(ed["retired_notes"]["p2.2"], "重複");
    assert!(
        ed.get("retired_notes").and_then(|n| n.get("l1.1.1")).is_none(),
        "--note を付けていない l1.1.1 の理由がある: {ed}"
    );
    b.check_ok(&e);
}

#[test]
fn set_text_on_section_title_is_rejected() {
    // 実装が決めた仕様: 見出し（section の title）は set-text で直せない
    let (b, e) = setup();
    let before = b.snapshot(&e);
    for id in ["s1", "s1.1", "s1.2", "s1.2.1", "s2"] {
        b.run("set-text", [e.as_str(), id, "--text", "新しい見出し"]).fails(&format!("見出し {id} に set-text"));
    }
    assert!(b.snapshot(&e) == before, "失敗したのに版が変わった");
}

#[test]
fn set_text_big_table_cells() {
    // 大きな表（t2、25 行）のセルを直す。結合セルと、最後の行も
    let (b, e) = setup();
    b.run("set-text", [e.as_str(), "t2", "--cell", "24,1", "--text", "二百四十", "--note", "n"]).ok();
    b.run("set-text", [e.as_str(), "t2", "--cell", "10,1", "--text", "百"]).ok();
    let before = b.snapshot(&e);
    b.run("set-text", [e.as_str(), "t2", "--cell", "11,1", "--text", "x"]).fails("結合で隠れたマス");
    b.run("set-text", [e.as_str(), "t2", "--cell", "25,0", "--text", "x"]).fails("表の外の行");
    assert!(b.snapshot(&e) == before, "失敗したのに版が変わった");

    let t = b.get(&e, "t2");
    let cells = t["cells"].as_array().unwrap();
    assert_eq!(cells.len(), 49, "セルの数が変わった");
    let at = |r: u64, c: u64| cells.iter().find(|x| x["row"] == r && x["col"] == c).unwrap().clone();
    assert_eq!(at(24, 1)["text"], "二百四十");
    assert_eq!(at(24, 1)["origin"], json!({"edition": "original", "ids": ["t2"], "text": "240"}));
    assert_eq!(at(10, 1)["text"], "百");
    assert_eq!(at(10, 1)["row_span"], 2, "結合が変わった");
    assert_eq!(at(10, 1)["origin"]["text"], "100");
    assert_others_unchanged(&b, ORIGINAL, &e, &["t2"]);
    b.check_ok(&e);

    // 直したセルは、--rows で絞った show にも出る
    let r = b.run("show", [e.as_str(), "t2", "--rows", "24-24"]);
    r.ok();
    assert!(r.stdout.contains("二百四十"), "--rows 24-24 に直したセルが無い:\n{}", r.stdout);
}

#[test]
fn delete_list_item_sentence() {
    let (b, e) = setup();
    b.run("delete", [e.as_str(), "l1.1.1"]).ok();
    assert_eq!(b.sentence_ids(&e, "l1.1"), ["l1.1.2"]);
    assert_eq!(b.retired_ids(&e), vec!["l1.1.1".to_string()]);
    b.check_ok(&e);
}

#[test]
fn delete_last_sentence_fails() {
    // 入れ物の最後の 1 文は消せない
    let (b, e) = setup();
    let before = b.snapshot(&e);
    b.run("delete", [e.as_str(), "p3.1"]).fails("p3 の最後の 1 文");
    b.run("delete", [e.as_str(), "l1.2.1"]).fails("l1.2 の最後の 1 文");
    assert!(b.snapshot(&e) == before, "失敗したのに版が変わった");

    // 2 文あるパラグラフも、1 文消した後の残りの 1 文は消せない
    b.run("delete", [e.as_str(), "p4.1"]).ok();
    b.run("delete", [e.as_str(), "p4.2"]).fails("p4 の最後の 1 文");
    assert_eq!(b.sentence_ids(&e, "p4"), ["p4.2"]);
}

#[test]
fn delete_non_sentence_fails() {
    let (b, e) = setup();
    let before = b.snapshot(&e);
    for id in ["p2", "l1", "l1.1", "f1", "c1", "t1", "s1.1", "p999.1"] {
        b.run("delete", [e.as_str(), id]).fails(&format!("{id} を delete"));
    }
    assert!(b.snapshot(&e) == before, "失敗したのに版が変わった");
}

#[test]
fn deleted_id_is_not_reused_in_same_edition() {
    // 約束 4: 消した ID は使い回されない
    let (b, e) = setup();
    b.run("delete", [e.as_str(), "p2.3"]).ok();
    b.run("insert", [e.as_str(), "--after", "p2.2", "--text", "足した文。"]).ok();
    assert_eq!(b.sentence_ids(&e, "p2"), ["p2.1", "p2.2", "p2.4"]);
    // split でも同じ
    b.run("delete", [e.as_str(), "p2.4"]).ok();
    b.run("split", [e.as_str(), "p2.2", "--text", "a。", "--text", "b。"]).ok();
    assert_eq!(b.sentence_ids(&e, "p2"), ["p2.1", "p2.2", "p2.5"]);
    let mut retired = b.retired_ids(&e);
    retired.sort();
    assert_eq!(retired, ["p2.3", "p2.4"]);
    b.check_ok(&e);
}

#[test]
fn deleted_id_is_not_reused_across_editions() {
    // 約束 4: 版を重ねても、消した ID は使い回されない
    let b = Book::new();
    let a = b.new_edition(ORIGINAL, "a");
    b.run("delete", [a.as_str(), "p2.3"]).ok();
    let bb = b.new_edition(&a, "b");
    b.run("insert", [bb.as_str(), "--after", "p2.2", "--text", "足した文。"]).ok();
    assert_eq!(b.sentence_ids(&bb, "p2"), ["p2.1", "p2.2", "p2.4"]);
    assert_eq!(b.get(&bb, "p2.4")["origin"], json!({"edition": "01_a", "ids": []}));
    b.check_ok(&bb);
}

#[test]
fn inserted_then_deleted_id_is_retired() {
    // 足した文を消しても、その番号は使わない
    let (b, e) = setup();
    b.run("insert", [e.as_str(), "--after", "p2.3", "--text", "足した文。"]).ok();
    b.run("delete", [e.as_str(), "p2.4"]).ok();
    b.run("insert", [e.as_str(), "--after", "p2.3", "--text", "また足した文。"]).ok();
    assert_eq!(b.sentence_ids(&e, "p2"), ["p2.1", "p2.2", "p2.3", "p2.5"]);
    b.check_ok(&e);
}

// ============================================================ original と版の名前

/// どの編集コマンドも、original を指定したら拒否される（約束 1、2）。
#[test]
fn edits_on_original_are_rejected() {
    let b = Book::new();
    let before = b.snapshot(ORIGINAL);
    let cases: Vec<Vec<&str>> = vec![
        vec!["set-text", ORIGINAL, "p2.2", "--text", "x"],
        vec!["set-text", ORIGINAL, "c1", "--text", "x"],
        vec!["set-text", ORIGINAL, "t1", "--cell", "1,0", "--text", "x"],
        vec!["set-text", ORIGINAL, "f1", "--index", "0", "--text", "x"],
        vec!["insert", ORIGINAL, "--after", "p2.1", "--text", "x"],
        vec!["insert", ORIGINAL, "--first-in", "p2", "--text", "x"],
        vec!["split", ORIGINAL, "p2.2", "--text", "a", "--text", "b"],
        vec!["delete", ORIGINAL, "p2.2"],
    ];
    for c in &cases {
        b.run(c[0], &c[1..]).fails(&format!("original への {}", c[0]));
    }
    assert!(b.snapshot(ORIGINAL) == before, "original が変わった");
}

#[test]
fn edits_with_bad_edition_names_are_rejected() {
    // 約束 2: 不正な版の名前は拒否される（original を言い換えた名前も含む）
    let b = Book::new();
    let e = b.new_edition(ORIGINAL, "fix");
    let before_original = b.snapshot(ORIGINAL);
    let before_e = b.snapshot(&e);
    let bad_names = [
        "../structure/original".to_string(),
        "./original".to_string(),
        "original/".to_string(),
        format!("{e}/../original"),
        format!("{e}\\..\\original"),
        "..".to_string(),
        "99_nothing".to_string(),
        "".to_string(),
    ];
    for name in &bad_names {
        b.run("set-text", [name.as_str(), "p2.2", "--text", "x"]).fails(&format!("版の名前 {name:?}"));
        b.run("delete", [name.as_str(), "p2.2"]).fails(&format!("版の名前 {name:?}"));
    }
    assert!(b.snapshot(ORIGINAL) == before_original, "original が変わった");
    assert!(b.snapshot(&e) == before_e, "{e} が変わった");
}

#[test]
fn many_edits_never_touch_original() {
    // 約束 1: どの編集コマンドも、structure/original/ を 1 バイトも変えない
    let b = Book::new();
    let before = b.snapshot(ORIGINAL);
    let e = b.new_edition(ORIGINAL, "fix");
    b.run("set-text", [e.as_str(), "p2.2", "--text", "二番目の文は日本語です。", "--note", "n"]).ok();
    b.run("set-text", [e.as_str(), "l1.1.2", "--text", "x。"]).ok();
    b.run("set-text", [e.as_str(), "c1", "--text", "図1"]).ok();
    b.run("set-text", [e.as_str(), "t1", "--cell", "1,0", "--text", "みかん"]).ok();
    b.run("set-text", [e.as_str(), "f1", "--index", "0", "--text", "図の中の文字"]).ok();
    b.run("insert", [e.as_str(), "--after", "p2.1", "--text", "足した。"]).ok();
    b.run("insert", [e.as_str(), "--first-in", "l1.2", "--text", "足した。"]).ok();
    b.run("split", [e.as_str(), "p5.1", "--text", "a。", "--text", "b。"]).ok();
    b.run("delete", [e.as_str(), "p2.3"]).ok();
    b.run("delete", [e.as_str(), "p3.1"]); // 失敗する編集も含める
    b.check_ok(&e);
    assert!(b.snapshot(ORIGINAL) == before, "original が変わった");
}

#[test]
fn edits_do_not_touch_other_editions() {
    let b = Book::new();
    let a = b.new_edition(ORIGINAL, "a");
    let bb = b.new_edition(ORIGINAL, "b");
    let before = b.snapshot(&a);
    b.run("set-text", [bb.as_str(), "p2.2", "--text", "x。"]).ok();
    b.run("insert", [bb.as_str(), "--after", "p2.1", "--text", "y。"]).ok();
    b.run("delete", [bb.as_str(), "p2.3"]).ok();
    assert!(b.snapshot(&a) == before, "{bb} の編集で {a} が変わった");
}

// ============================================================ 1 行に 1 センテンス

/// 版のすべてのファイルで、すべてのセンテンスが 1 行に書かれているか（約束 8）。
fn assert_one_sentence_per_line(b: &Book, edition: &str) {
    let dir = b.edition(edition);
    for f in b.files(edition) {
        let raw = std::fs::read_to_string(dir.join(&f)).unwrap();
        // 1 行ずつ JSON として読めるもののうち、センテンスの形（id と text）のもの
        let mut on_one_line = std::collections::BTreeSet::new();
        for line in raw.lines() {
            let t = line.trim().trim_end_matches(',');
            if let Ok(Value::Object(o)) = serde_json::from_str::<Value>(t) {
                if let (Some(id), Some(_)) = (o.get("id").and_then(Value::as_str), o.get("text").and_then(Value::as_str)) {
                    on_one_line.insert(id.to_string());
                }
            }
        }
        // ファイルの中のすべてのセンテンス
        let part = b.part(edition, &f);
        let mut all = std::collections::BTreeMap::new();
        fn walk(v: &Value, out: &mut std::collections::BTreeMap<String, ()>) {
            match v {
                Value::Array(a) => a.iter().for_each(|x| walk(x, out)),
                Value::Object(o) => {
                    if let Some(Value::Array(ss)) = o.get("sentences") {
                        for s in ss {
                            out.insert(s["id"].as_str().unwrap().to_string(), ());
                        }
                    }
                    o.values().for_each(|x| walk(x, out));
                }
                _ => {}
            }
        }
        walk(&part, &mut all);
        for id in all.keys() {
            assert!(on_one_line.contains(id), "{edition}/{f} のセンテンス {id} が 1 行に書かれていない:\n{raw}");
        }
    }
}

#[test]
fn sentences_are_one_per_line_after_new() {
    let b = Book::new();
    let e = b.new_edition(ORIGINAL, "copy");
    assert_one_sentence_per_line(&b, &e);
}

#[test]
fn sentences_are_one_per_line_after_edits() {
    // origin と note を持つ、直したセンテンスも 1 行に書かれる
    let (b, e) = setup();
    b.run("set-text", [e.as_str(), "p2.2", "--text", "二番目の文は日本語です。", "--note", "「曰」を「日」に修正"]).ok();
    b.run("insert", [e.as_str(), "--after", "l1.1.1", "--text", "足した。", "--note", "脱落"]).ok();
    b.run("split", [e.as_str(), "p5.1", "--text", "a。", "--text", "b。"]).ok();
    b.run("delete", [e.as_str(), "p4.1"]).ok();
    assert_one_sentence_per_line(&b, &e);
}
