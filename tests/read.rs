//! 読むコマンド（`list`、`toc`、`show`）のテスト。

mod common;

use common::*;
use serde_json::Value;

// ------------------------------------------------------------ list

#[test]
fn list_shows_original() {
    let b = Book::new();
    let r = b.run("list", Vec::<&str>::new());
    r.ok();
    assert!(r.stdout.contains(ORIGINAL), "list に original が出ていない:\n{}", r.stdout);
}

#[test]
fn list_shows_lineage_of_stacked_editions() {
    // 約束 9: 01_a を元に 02_b を作り、list で系譜が出る
    let b = Book::new();
    let a = b.new_edition(ORIGINAL, "a");
    let bb = b.new_edition(&a, "b");
    assert_eq!(a, "01_a");
    assert_eq!(bb, "02_b");

    let r = b.run("list", Vec::<&str>::new());
    r.ok();
    for name in [ORIGINAL, "01_a", "02_b"] {
        assert!(r.stdout.contains(name), "list に {name} が出ていない:\n{}", r.stdout);
    }
    // 02_b の行に、元の版 01_a と編集者 b が出る
    let line = r
        .stdout
        .lines()
        .find(|l| l.contains("02_b"))
        .expect("02_b の行が無い");
    assert!(line.contains("01_a"), "02_b の base（01_a）が同じ行に出ていない: {line}");
    // 01_a の行に、元の版 original が出る
    let line = r
        .stdout
        .lines()
        .find(|l| l.contains("01_a") && !l.contains("02_b"))
        .expect("01_a の行が無い");
    assert!(line.contains(ORIGINAL), "01_a の base（original）が同じ行に出ていない: {line}");
}

// ------------------------------------------------------------ toc

#[test]
fn toc_shows_sections_and_files() {
    let b = Book::new();
    let r = b.run("toc", [ORIGINAL]);
    r.ok();
    for id in ["s1", "s1.1", "s1.2", "s1.2.1", "s2"] {
        assert!(mentions_id(&r.stdout, id), "toc にセクション {id} が出ていない:\n{}", r.stdout);
    }
    for f in PART_FILES {
        assert!(r.stdout.contains(f), "toc にファイル {f} が出ていない:\n{}", r.stdout);
    }
    assert!(r.stdout.contains("1.2.1 入れ子の見出し"), "toc に見出しが出ていない:\n{}", r.stdout);
}

#[test]
fn toc_of_new_edition() {
    let b = Book::new();
    let e = b.new_edition(ORIGINAL, "copy");
    let r = b.run("toc", [&e]);
    r.ok();
    assert!(mentions_id(&r.stdout, "s1.2.1"));
}

#[test]
fn toc_rejects_unknown_edition() {
    let b = Book::new();
    b.run("toc", ["99_nothing"]).fails("無い版");
}

// ------------------------------------------------------------ show

/// JSON の中に、指定した ID を持つオブジェクトがあるか。
fn has_id(v: &Value, id: &str) -> Option<Value> {
    match v {
        Value::Object(o) => {
            if o.get("id").and_then(Value::as_str) == Some(id) {
                return Some(v.clone());
            }
            o.values().find_map(|x| has_id(x, id))
        }
        Value::Array(a) => a.iter().find_map(|x| has_id(x, id)),
        _ => None,
    }
}

#[test]
fn show_sentence() {
    let b = Book::new();
    let r = b.run("show", [ORIGINAL, "p2.2"]);
    r.ok();
    let v = has_id(&r.json(), "p2.2").expect("show の出力に p2.2 が無い");
    assert_eq!(v["text"], "二番目の文は曰本語です。");
}

#[test]
fn show_list_item_sentence() {
    let b = Book::new();
    let r = b.run("show", [ORIGINAL, "l1.1.2"]);
    r.ok();
    let v = has_id(&r.json(), "l1.1.2").expect("show の出力に l1.1.2 が無い");
    assert_eq!(v["text"], "項目の二文目です。");
}

#[test]
fn show_paragraph_figure_caption_section() {
    let b = Book::new();
    for id in ["p2", "l1", "l1.2", "f1", "c1", "t1", "s1.2.1", "p1"] {
        let r = b.run("show", [ORIGINAL, id]);
        r.ok();
        assert!(has_id(&r.json(), id).is_some(), "show {id} の出力に {id} が無い:\n{}", r.stdout);
    }
}

#[test]
fn show_section_in_other_file() {
    // 別ファイルに分かれたセクション（section_ref の先）も ID で探せる
    let b = Book::new();
    let r = b.run("show", [ORIGINAL, "s1.1"]);
    r.ok();
    let v = has_id(&r.json(), "s1.1").expect("show の出力に s1.1 が無い");
    assert!(has_id(&v, "p3.1").is_some() || v["type"] == "section", "s1.1 の中身が出ていない:\n{}", r.stdout);
}

#[test]
fn show_unknown_id_fails() {
    let b = Book::new();
    b.run("show", [ORIGINAL, "p999.1"]).fails("無い ID");
}

/// `show <表> --rows <range>` で出たセルの (row, col) の一覧。
fn shown_cells(b: &Book, table: &str, range: &str) -> Vec<(u64, u64)> {
    let r = b.run("show", [ORIGINAL, table, "--rows", range]);
    r.ok();
    let v = has_id(&r.json(), table).unwrap_or_else(|| panic!("show の出力に {table} が無い"));
    let mut cells: Vec<(u64, u64)> = v["cells"]
        .as_array()
        .unwrap_or_else(|| panic!("{table} に cells が無い"))
        .iter()
        .map(|c| (c["row"].as_u64().unwrap(), c["col"].as_u64().unwrap()))
        .collect();
    cells.sort();
    cells
}

#[test]
fn show_table_rows() {
    // --rows a-b は、セルの row の値（0 始まり）で a 以上 b 以下を出す。
    // t2 は 25 行（row 0〜24）。
    let b = Book::new();
    let cells = shown_cells(&b, "t2", "3-7");
    let rows: std::collections::BTreeSet<u64> = cells.iter().map(|c| c.0).collect();
    assert_eq!(rows, (3..=7).collect(), "--rows 3-7 で出た行が違う");
    assert_eq!(cells.len(), 10, "--rows 3-7 で出たセルの数が違う: {cells:?}");

    // 行を絞っても、セルの row と text は元のまま
    let r = b.run("show", [ORIGINAL, "t2", "--rows", "3-7"]);
    let v = has_id(&r.json(), "t2").unwrap();
    let c = v["cells"].as_array().unwrap().iter().find(|c| c["row"] == 5 && c["col"] == 0).expect("行 5 のセルが無い");
    assert_eq!(c["text"], "行5");
}

#[test]
fn show_table_rows_boundaries() {
    let b = Book::new();
    // 先頭の行（0）と最後の行（24）
    let rows = |range: &str| -> Vec<u64> {
        let mut v: Vec<u64> = shown_cells(&b, "t2", range).iter().map(|c| c.0).collect();
        v.dedup();
        v
    };
    assert_eq!(rows("0-0"), [0]);
    assert_eq!(rows("0-1"), [0, 1]);
    assert_eq!(rows("23-24"), [23, 24]);
    assert_eq!(rows("1-20"), (1..=20).collect::<Vec<u64>>());
}

#[test]
fn show_table_rows_merged_cells() {
    // 結合セルは、row が範囲に入るものだけが出る。
    // t2 の (10,1) は 2 行に結合していて、(11,1) のセルは無い。
    let b = Book::new();
    assert_eq!(shown_cells(&b, "t2", "11-12"), [(11, 0), (12, 0), (12, 1)], "row 10 の結合セルが出てはいけない");
    assert_eq!(shown_cells(&b, "t2", "9-10"), [(9, 0), (9, 1), (10, 0), (10, 1)]);
    // t1 の (2,0) は 2 列に結合している
    assert_eq!(shown_cells(&b, "t1", "2-2"), [(2, 0)]);
    assert_eq!(shown_cells(&b, "t1", "0-1"), [(0, 0), (0, 1), (1, 0), (1, 1)]);
}

#[test]
fn show_whole_table_without_rows() {
    let b = Book::new();
    let r = b.run("show", [ORIGINAL, "t2"]);
    r.ok();
    let v = has_id(&r.json(), "t2").unwrap();
    assert_eq!(v["cells"].as_array().unwrap().len(), 49);
}

#[test]
fn show_file() {
    // --file で 1 ファイルをそのまま出す（ID は付けない）
    let b = Book::new();
    let r = b.run("show", [ORIGINAL, "--file", "003_s1.1.json"]);
    r.ok();
    let expected = b.part(ORIGINAL, "003_s1.1.json");
    assert_eq!(r.json(), expected, "--file の出力がファイルの中身と違う");
}

#[test]
fn show_needs_exactly_one_of_id_and_file() {
    let b = Book::new();
    b.run("show", [ORIGINAL, "p3.1", "--file", "003_s1.1.json"]).fails("ID と --file の両方");
    b.run("show", [ORIGINAL]).fails("ID も --file も無い");
}

#[test]
fn show_file_unknown_fails() {
    let b = Book::new();
    b.run("show", [ORIGINAL, "--file", "099_s9.json"]).fails("無いファイル");
}

#[test]
fn show_file_rejects_path_outside_edition() {
    let b = Book::new();
    b.run("show", [ORIGINAL, "--file", "../original/index.json"])
        .fails("版の外を指すファイル名");
}

#[test]
fn show_reads_edited_edition() {
    let b = Book::new();
    let e = b.new_edition(ORIGINAL, "fix");
    b.run("set-text", [e.as_str(), "p2.2", "--text", "二番目の文は日本語です。"]).ok();
    let r = b.run("show", [e.as_str(), "p2.2"]);
    r.ok();
    assert_eq!(has_id(&r.json(), "p2.2").unwrap()["text"], "二番目の文は日本語です。");
    // 元の版は元のまま
    let r = b.run("show", [ORIGINAL, "p2.2"]);
    assert_eq!(has_id(&r.json(), "p2.2").unwrap()["text"], "二番目の文は曰本語です。");
}
