//! 要素の一覧を出すコマンド（`outline`）のテスト。
//!
//! 1 行の内容はタブ区切りで、入れ子の深さ、type、ID、大きさ、先頭の文字。
//! 出力の細かい形は実装が決めるので、列の位置や文言では比べない。
//! 確かめるのは、出る ID と出ない ID、読み順、section_ref の先のファイル名、
//! 実行の前後でファイルが変わらないこと、outline で得た ID を show に渡せること。

mod common;

use std::collections::BTreeSet;

use common::*;

/// 出力の行のうち、タブで区切った欄のどれかが `id` と同じ行。
fn line_of<'a>(out: &'a str, id: &str) -> Option<(usize, &'a str)> {
    out.lines()
        .enumerate()
        .find(|(_, l)| l.split('\t').any(|f| f.trim() == id))
}

fn has(out: &str, id: &str) -> bool {
    line_of(out, id).is_some()
}

/// `ids` が、この順に、別々の行に出ることを確かめる。
fn assert_in_order(out: &str, ids: &[&str]) {
    let mut last: Option<(usize, &str)> = None;
    for id in ids {
        let (n, _) = line_of(out, id).unwrap_or_else(|| panic!("outline に {id} が無い:\n{out}"));
        if let Some((m, prev)) = last {
            assert!(n > m, "outline で {id} が {prev} より前に出ている（読み順でない）:\n{out}");
        }
        last = Some((n, id));
    }
}

/// 出力の欄のうち、要素の ID の形をしたもの（読み順）。
fn ids_in(out: &str) -> Vec<String> {
    let is_id = |f: &str| {
        let mut cs = f.chars();
        matches!(cs.next(), Some('s' | 'p' | 'l' | 'f' | 't' | 'c'))
            && !f[1..].is_empty()
            && f[1..].split('.').all(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
    };
    out.lines()
        .filter_map(|l| l.split('\t').map(str::trim).find(|f| is_id(f)).map(str::to_string))
        .collect()
}

fn outline_file(b: &Book, edition: &str, file: &str) -> String {
    let r = b.run("outline", [edition, "--file", file]);
    r.ok();
    r.stdout.clone()
}

fn outline_id(b: &Book, edition: &str, id: &str) -> String {
    let r = b.run("outline", [edition, id]);
    r.ok();
    r.stdout.clone()
}

// ------------------------------------------------------------ --file

#[test]
fn outline_file_lists_elements_in_reading_order() {
    let b = Book::new();
    let out = outline_file(&b, ORIGINAL, "002_s1.json");
    assert_in_order(&out, &["s1", "p2", "l1", "f1", "c1", "s1.1", "s1.2"]);
}

#[test]
fn outline_does_not_list_sentences_or_items() {
    // センテンスと、箇条書きの item は出ない（パラグラフと list まで）
    let b = Book::new();
    let out = outline_file(&b, ORIGINAL, "002_s1.json");
    for id in ["p2.1", "p2.2", "p2.3", "l1.1", "l1.2", "l1.1.1", "l1.1.2", "l1.2.1"] {
        assert!(!has(&out, id), "outline にセンテンスか item の {id} が出ている:\n{out}");
    }
}

#[test]
fn outline_file_shows_section_ref_target_file() {
    // section_ref は、指す先のファイル名が出る。先のファイルの中身は出ない
    let b = Book::new();
    let out = outline_file(&b, ORIGINAL, "002_s1.json");
    let (_, l) = line_of(&out, "s1.1").expect("s1.1 の行が無い");
    assert!(l.contains("003_s1.1.json"), "section_ref s1.1 の行に、先のファイル名が無い: {l}");
    let (_, l) = line_of(&out, "s1.2").expect("s1.2 の行が無い");
    assert!(l.contains("004_s1.2.json"), "section_ref s1.2 の行に、先のファイル名が無い: {l}");
    for id in ["p3", "p4", "t1", "s1.2.1", "p5"] {
        assert!(!has(&out, id), "--file 002_s1.json に、ほかのファイルの {id} が出ている:\n{out}");
    }
}

#[test]
fn outline_file_with_nested_section() {
    let b = Book::new();
    let out = outline_file(&b, ORIGINAL, "004_s1.2.json");
    assert_in_order(&out, &["s1.2", "p4", "c2", "t1", "s1.2.1", "p5"]);
    for id in ["p4.1", "p5.1", "s1", "p2"] {
        assert!(!has(&out, id), "--file 004_s1.2.json に {id} が出ている:\n{out}");
    }
}

#[test]
fn outline_front_file() {
    let b = Book::new();
    let out = outline_file(&b, ORIGINAL, "001_front.json");
    assert!(has(&out, "p1"), "outline に p1 が無い:\n{out}");
    assert!(!has(&out, "p1.1") && !has(&out, "s1"), "front の outline に余計な ID がある:\n{out}");
}

#[test]
fn outline_lines_are_tab_separated() {
    // 1 行は、深さ、type、ID、大きさ、先頭の文字のタブ区切り（少なくとも 5 欄）
    let b = Book::new();
    for f in PART_FILES {
        let out = outline_file(&b, ORIGINAL, f);
        for l in out.lines().filter(|l| !l.trim().is_empty()) {
            assert!(l.split('\t').count() >= 5, "{f} の outline の行が 5 欄に満たない: {l:?}");
        }
    }
}

#[test]
fn outline_line_has_type_and_head_text() {
    let b = Book::new();
    let out = outline_file(&b, ORIGINAL, "002_s1.json");
    let field = |id: &str, want: &str| {
        let (_, l) = line_of(&out, id).unwrap_or_else(|| panic!("{id} の行が無い:\n{out}"));
        assert!(l.split('\t').any(|f| f.trim() == want), "{id} の行に {want} の欄が無い: {l}");
    };
    field("s1", "section");
    field("p2", "paragraph");
    field("l1", "list");
    field("f1", "figure");
    field("c1", "caption");
    field("s1.1", "section_ref");
    // 先頭の文字
    let head = |id: &str, want: &str| {
        let (_, l) = line_of(&out, id).unwrap();
        assert!(l.contains(want), "{id} の行に、先頭の文字 {want} が無い: {l}");
    };
    head("p2", "最初");
    head("l1", "一つ目");
    head("c1", "図1");
}

#[test]
fn outline_table_shows_rows_and_cells() {
    // 表は、行数とセルの数も出る（t2 は 25 行、セル 49 個）。
    // 数が独立して出ていることを、数字の並びとして取り出して確かめる
    let b = Book::new();
    let out = outline_file(&b, ORIGINAL, "005_s2.json");
    let (_, l) = line_of(&out, "t2").expect("t2 の行が無い");
    let nums: Vec<u64> = l.split(|c: char| !c.is_ascii_digit()).filter_map(|n| n.parse().ok()).collect();
    assert!(nums.contains(&25) && nums.contains(&49), "t2 の行に、行数 25 とセルの数 49 が無い: {l}");
}

#[test]
fn outline_size_grows_with_element() {
    // 大きさは、その要素を JSON にしたときの文字数。
    // 欄の位置は比べないので、「大きな表 t2 の行の数のうち最大のもの」が「小さな段落 p6 の行の数のうち最大のもの」より大きいことだけを確かめる
    let b = Book::new();
    let out = outline_file(&b, ORIGINAL, "005_s2.json");
    let max_num = |id: &str| -> u64 {
        let (_, l) = line_of(&out, id).unwrap();
        l.split(|c: char| !c.is_ascii_digit()).filter_map(|n| n.parse::<u64>().ok()).max().unwrap_or(0)
    };
    assert!(max_num("t2") > 1000, "t2（JSON で 3000 字を超える）の行に、大きさらしい数が無い:\n{out}");
    assert!(max_num("t2") > max_num("p6"), "t2 の大きさが p6 より小さい:\n{out}");
}

// ------------------------------------------------------------ <ID>

#[test]
fn outline_id_lists_elements_under_it() {
    let b = Book::new();
    let out = outline_id(&b, ORIGINAL, "s1.2");
    assert_in_order(&out, &["p4", "c2", "t1", "s1.2.1", "p5"]);
    for id in ["p4.1", "p5.1", "p2", "p6", "s2"] {
        assert!(!has(&out, id), "outline s1.2 に {id} が出ている:\n{out}");
    }
}

#[test]
fn outline_id_excludes_the_element_itself() {
    // 実装が決めた形: ID を指定したとき、その要素自身は出ず、下の要素だけが出る
    let b = Book::new();
    for id in ["s1", "s1.2", "s1.2.1", "s2"] {
        let out = outline_id(&b, ORIGINAL, id);
        assert!(!has(&out, id), "outline {id} に、{id} 自身の行が出ている:\n{out}");
    }
}

#[test]
fn outline_head_text_of_figure_and_list() {
    // 実装が決めた形: 図は text の最初の文字列、list は最初の item の最初のセンテンスが、先頭の文字になる
    let b = Book::new();
    let out = outline_file(&b, ORIGINAL, "002_s1.json");
    let (_, l) = line_of(&out, "f1").unwrap();
    assert!(l.contains("図の中の文宇"), "f1 の行に、text の最初の文字列が無い: {l}");
    assert!(!l.contains("矢印"), "f1 の行に、text の 2 つ目が出ている: {l}");
    let (_, l) = line_of(&out, "l1").unwrap();
    assert!(l.contains("一つ目の項目"), "l1 の行に、最初の item の最初のセンテンスが無い: {l}");
}

#[test]
fn outline_head_text_is_short() {
    // 実装が決めた形: 先頭の文字は 20 字まで。s1.1 の p3 の文は 18 字なので、長い見出しで確かめる
    let b = Book::new();
    let e = b.new_edition(ORIGINAL, "fix");
    let long = "あいうえおかきくけこさしすせそたちつてとなにぬねの。";
    b.run("set-text", [e.as_str(), "p3.1", "--text", long]).ok();
    let out = outline_id(&b, &e, "s1.1");
    let (_, l) = line_of(&out, "p3").unwrap();
    assert!(l.contains("あいうえお"), "p3 の行に先頭の文字が無い: {l}");
    assert!(!l.contains("なにぬねの"), "p3 の行の先頭の文字が 20 字を超えている: {l}");
}

#[test]
fn outline_id_nested_section() {
    let b = Book::new();
    let out = outline_id(&b, ORIGINAL, "s1.2.1");
    assert!(has(&out, "p5"), "outline s1.2.1 に p5 が無い:\n{out}");
    for id in ["p4", "t1", "p5.1"] {
        assert!(!has(&out, id), "outline s1.2.1 に {id} が出ている:\n{out}");
    }
}

#[test]
fn outline_id_with_section_ref_children() {
    let b = Book::new();
    let out = outline_id(&b, ORIGINAL, "s1");
    assert_in_order(&out, &["p2", "l1", "f1", "c1", "s1.1", "s1.2"]);
    assert!(out.contains("003_s1.1.json") && out.contains("004_s1.2.json"), "section_ref の先のファイル名が無い:\n{out}");
    // section_ref の先のファイルの中までは辿らない
    for id in ["p3", "p4", "c2", "t1", "s1.2.1", "p5"] {
        assert!(!has(&out, id), "outline s1 に、別のファイルにある {id} が出ている:\n{out}");
    }
}

#[test]
fn outline_id_of_leaf_element_is_empty() {
    // パラグラフ、list、図、キャプション、表を指定したときは、エラーにせず、標準出力が空
    let b = Book::new();
    for id in ["p1", "p2", "l1", "f1", "c1", "t1", "t2", "f2"] {
        let r = b.run("outline", [ORIGINAL, id]);
        r.ok();
        assert!(r.stdout.trim().is_empty(), "outline {id} の標準出力が空でない:\n{}", r.stdout);
    }
}

#[test]
fn outline_id_in_other_file() {
    // section_ref の先のファイルにあるセクションも、ID で指せる
    let b = Book::new();
    let out = outline_id(&b, ORIGINAL, "s1.1");
    assert!(has(&out, "p3"), "outline s1.1 に p3 が無い:\n{out}");
}

// ------------------------------------------------------------ outline の ID を show に渡す

#[test]
fn outline_ids_cover_all_elements_and_work_with_show() {
    // すべてのファイルの outline に出る ID は、センテンスと item を除いたすべての要素で、
    // どれも show に渡せる
    let b = Book::new();
    let mut seen = BTreeSet::new();
    for f in PART_FILES {
        for id in ids_in(&outline_file(&b, ORIGINAL, f)) {
            seen.insert(id);
        }
    }
    let mut want = BTreeSet::new();
    for (id, el) in b.all_elements(ORIGINAL) {
        let is_sentence_or_item = el.get("type").is_none();
        if !is_sentence_or_item {
            want.insert(id);
        }
    }
    assert_eq!(seen, want, "outline に出た ID と、版の要素（センテンスと item を除く）が合わない");

    for id in &seen {
        let r = b.run("show", [ORIGINAL, id.as_str()]);
        assert!(r.success(), "outline で得た {id} を show に渡せない:\n{}", r.stderr);
        assert!(r.stdout.contains(&format!("\"{id}\"")), "show {id} の出力に {id} が無い:\n{}", r.stdout);
    }
}

// ------------------------------------------------------------ 読むだけ

#[test]
fn outline_does_not_modify_anything() {
    let b = Book::new();
    let e = b.new_edition(ORIGINAL, "fix");
    b.run("set-text", [e.as_str(), "p2.2", "--text", "x。", "--note", "n"]).ok();
    let x = b.snapshot(ORIGINAL);
    let y = b.snapshot(&e);
    for ed in [ORIGINAL, e.as_str()] {
        for f in PART_FILES {
            outline_file(&b, ed, f);
        }
        outline_id(&b, ed, "s1");
        outline_id(&b, ed, "s2");
    }
    assert!(b.snapshot(ORIGINAL) == x, "outline で original が変わった");
    assert!(b.snapshot(&e) == y, "outline で {e} が変わった");
    // edition_dirs は名前の順（01_fix、original）
    assert_eq!(b.edition_dirs(), vec![e.clone(), ORIGINAL.to_string()], "outline で版のフォルダが増えた");
}

#[test]
fn outline_reflects_edited_edition() {
    let b = Book::new();
    let e = b.new_edition(ORIGINAL, "fix");
    b.run("insert", [e.as_str(), "--first-in", "p2", "--text", "先頭に足した文。"]).ok();
    let out = outline_file(&b, &e, "002_s1.json");
    assert_in_order(&out, &["s1", "p2", "l1"]);
    let (_, l) = line_of(&out, "p2").unwrap();
    assert!(l.contains("先頭"), "p2 の先頭の文字が、編集した版の文になっていない: {l}");
}

// ------------------------------------------------------------ エラー

#[test]
fn outline_errors() {
    let b = Book::new();
    b.run("outline", [ORIGINAL, "s1", "--file", "002_s1.json"]).fails("ID と --file の両方");
    b.run("outline", [ORIGINAL]).fails("ID も --file も無い");
    b.run("outline", [ORIGINAL, "--file", "099_s9.json"]).fails("無いファイル");
    b.run("outline", [ORIGINAL, "s9"]).fails("無い ID");
    b.run("outline", [ORIGINAL, "--file", "../original/index.json"]).fails("版の外を指すファイル名");
    b.run("outline", ["99_nothing", "--file", "002_s1.json"]).fails("無い版");
    b.run("outline", ["../structure/original", "s1"]).fails("不正な版の名前");
}
