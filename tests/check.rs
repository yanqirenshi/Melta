//! 確かめるコマンド（`check`）のテスト。
//!
//! 壊した版は、`melta new` で作った版をテストの中で書き換えて作る。
//! 書き換えた後の JSON の書式は気にしない（check は書式ではなく中身を確かめる）。

mod common;

use common::*;
use serde_json::{json, Value};

fn setup() -> (Book, String) {
    let b = Book::new();
    let e = b.new_edition(ORIGINAL, "fix");
    b.check_ok(&e);
    (b, e)
}

/// 分けたファイルの JSON を、関数で書き換える。
fn edit_part(b: &Book, e: &str, file: &str, f: impl FnOnce(&mut Value)) {
    let mut v = b.part(e, file);
    f(&mut v);
    b.write_part(e, file, &v);
}

fn edit_index(b: &Book, e: &str, f: impl FnOnce(&mut Value)) {
    let mut v = b.index(e);
    f(&mut v);
    b.write_index(e, &v);
}

/// s1 のセクションの children（002_s1.json）。
fn s1_children(v: &mut Value) -> &mut Vec<Value> {
    v["body"][0]["children"].as_array_mut().unwrap()
}

// ------------------------------------------------------------ 通るもの

#[test]
fn original_passes() {
    let b = Book::new();
    b.check_ok(ORIGINAL);
}

#[test]
fn edited_edition_passes() {
    // 約束 6
    let (b, e) = setup();
    b.run("set-text", [e.as_str(), "p2.2", "--text", "二番目の文は日本語です。", "--note", "n"]).ok();
    b.run("set-text", [e.as_str(), "t1", "--cell", "1,0", "--text", "みかん"]).ok();
    b.run("insert", [e.as_str(), "--first-in", "p2", "--text", "先頭。"]).ok();
    b.run("split", [e.as_str(), "l1.1.1", "--text", "a。", "--text", "b。"]).ok();
    b.run("delete", [e.as_str(), "p5.2"]).ok();
    b.check_ok(&e);
}

#[test]
fn unknown_properties_are_allowed() {
    // スキーマは知らないプロパティを許す
    let (b, e) = setup();
    edit_part(&b, &e, "002_s1.json", |v| {
        s1_children(v)[0]["sentences"][0]["comment"] = json!("後工程が足した情報");
    });
    b.check_ok(&e);
}

#[test]
fn check_rejects_unknown_edition() {
    let b = Book::new();
    b.run("check", ["99_nothing"]).fails("無い版");
    b.run("check", ["../structure/original"]).fails("不正な版の名前");
}

// ------------------------------------------------------------ 1. スキーマ

#[test]
fn detects_schema_violation_in_part() {
    let (b, e) = setup();
    edit_part(&b, &e, "002_s1.json", |v| {
        s1_children(v)[0]["sentences"][0].as_object_mut().unwrap().remove("text");
    });
    b.check_fails(&e, "センテンスに text が無い");
}

#[test]
fn detects_bad_sentence_id_pattern() {
    let (b, e) = setup();
    edit_part(&b, &e, "002_s1.json", |v| {
        s1_children(v)[0]["sentences"][0]["id"] = json!("x2.1");
    });
    b.check_fails(&e, "センテンスの ID の形が違う");
}

#[test]
fn detects_schema_violation_in_index() {
    let (b, e) = setup();
    edit_index(&b, &e, |v| v["version"] = json!(3));
    b.check_fails(&e, "index.json の version が 2 でない");
}

#[test]
fn detects_broken_json() {
    let (b, e) = setup();
    let p = b.edition(&e).join("003_s1.1.json");
    std::fs::write(&p, "{ \"version\": 2, ").unwrap();
    b.check_fails(&e, "JSON として読めない");
}

#[test]
fn detects_odd_shapes_without_panic() {
    // 形のおかしい JSON でも、パニックせずに問題として出す
    let cases: Vec<(&str, &str, Breaker)> = vec![
        ("002_s1.json", "body が文字列", |v| v["body"] = json!("x")),
        ("002_s1.json", "body が空", |v| v["body"] = json!([])),
        ("002_s1.json", "children が数", |v| v["body"][0]["children"] = json!(1)),
        ("002_s1.json", "sentences が null", |v| v["body"][0]["children"][0]["sentences"] = Value::Null),
        ("002_s1.json", "センテンスが文字列", |v| v["body"][0]["children"][0]["sentences"][0] = json!("文")),
        ("002_s1.json", "id が数", |v| v["body"][0]["children"][0]["id"] = json!(2)),
        ("002_s1.json", "type が無い", |v| {
            v["body"][0]["children"][0].as_object_mut().unwrap().remove("type");
        }),
        ("002_s1.json", "section_ref に file が無い", |v| {
            v["body"][0]["children"][4].as_object_mut().unwrap().remove("file");
        }),
        ("004_s1.2.json", "セルが配列でない", |v| v["body"][0]["children"][2]["cells"] = json!({})),
        ("002_s1.json", "ファイル全体が配列", |v| *v = json!([1, 2])),
    ];
    for (file, why, f) in cases {
        let (b, e) = setup();
        edit_part(&b, &e, file, f);
        b.check_fails(&e, why);
    }
}

#[test]
fn detects_odd_index_without_panic() {
    let cases: Vec<(&str, Breaker)> = vec![
        ("files が null", |v| v["files"] = Value::Null),
        ("toc がオブジェクト", |v| v["toc"] = json!({})),
        ("edition が文字列", |v| v["edition"] = json!("01_fix")),
        ("index.json 全体が数", |v| *v = json!(1)),
    ];
    for (why, f) in cases {
        let (b, e) = setup();
        edit_index(&b, &e, f);
        b.check_fails(&e, why);
    }
    // index.json が空のファイル
    let (b, e) = setup();
    std::fs::write(b.edition(&e).join("index.json"), "").unwrap();
    b.check_fails(&e, "index.json が空");
}

#[test]
fn other_commands_do_not_panic_on_broken_edition() {
    // 壊れた版に対しても、ほかのコマンドはパニックしない（成功するか、エラーで終わる）
    let (b, e) = setup();
    std::fs::write(b.edition(&e).join("003_s1.1.json"), "{ \"version\": 2, ").unwrap();
    let runs = [
        b.run("toc", [e.as_str()]),
        b.run("show", [e.as_str(), "p3.1"]),
        b.run("show", [e.as_str(), "--file", "003_s1.1.json"]),
        b.run("set-text", [e.as_str(), "p2.2", "--text", "x"]),
        b.run("diff", [ORIGINAL, e.as_str()]),
        b.run("new", ["--from", e.as_str(), "--editor", "next"]),
        b.run("list", Vec::<&str>::new()),
    ];
    for (i, r) in runs.iter().enumerate() {
        r.not_panicked(&format!("壊れた版に対するコマンド {i}"));
    }
}

// ------------------------------------------------------------ 2. ID の重複

#[test]
fn detects_duplicate_sentence_id_across_files() {
    // 約束 7
    let (b, e) = setup();
    edit_part(&b, &e, "005_s2.json", |v| {
        s1_children(v)[0]["sentences"][0]["id"] = json!("p2.1");
    });
    b.check_fails(&e, "別のファイルのセンテンスと ID が重複");
}

#[test]
fn detects_duplicate_id_in_same_paragraph() {
    let (b, e) = setup();
    edit_part(&b, &e, "002_s1.json", |v| {
        s1_children(v)[0]["sentences"][1]["id"] = json!("p2.1");
    });
    b.check_fails(&e, "同じパラグラフでセンテンスの ID が重複");
}

#[test]
fn detects_duplicate_element_id() {
    let (b, e) = setup();
    edit_part(&b, &e, "004_s1.2.json", |v| {
        s1_children(v)[1]["id"] = json!("c1"); // c2 → c1（002_s1.json にある）
    });
    b.check_fails(&e, "キャプションの ID が重複");
}

// ------------------------------------------------------------ 3. section_ref の先

#[test]
fn detects_section_ref_to_missing_file() {
    // 約束 7
    let (b, e) = setup();
    edit_part(&b, &e, "002_s1.json", |v| {
        let r = s1_children(v).iter_mut().find(|c| c["id"] == "s1.1").unwrap();
        r["file"] = json!("099_s1.1.json");
    });
    b.check_fails(&e, "section_ref の先のファイルが無い");
}

#[test]
fn detects_missing_referenced_file() {
    let (b, e) = setup();
    std::fs::remove_file(b.edition(&e).join("003_s1.1.json")).unwrap();
    b.check_fails(&e, "section_ref の先のファイルを消した");
}

// ------------------------------------------------------------ 4. files・toc と実際のファイル

#[test]
fn detects_file_missing_from_index_files() {
    // 約束 7: files と実際のファイルの不一致（実際にあるファイルが files に無い）
    let (b, e) = setup();
    edit_index(&b, &e, |v| {
        let files = v["files"].as_array_mut().unwrap();
        files.retain(|f| f["file"] != "005_s2.json");
    });
    b.check_fails(&e, "005_s2.json が files に無い");
}

#[test]
fn detects_index_files_entry_without_file() {
    // files にあるのに、実際のファイルが無い
    let (b, e) = setup();
    edit_index(&b, &e, |v| {
        v["files"].as_array_mut().unwrap().push(json!({"file": "006_s3.json", "id": "s3", "chars": 1, "json_chars": 1}));
    });
    b.check_fails(&e, "files にある 006_s3.json が無い");
}

#[test]
fn detects_stray_part_file() {
    // files に無い分けたファイルが、版のフォルダにある
    let (b, e) = setup();
    let mut v = b.part(&e, "005_s2.json");
    v["file"] = json!("006_s2.json");
    b.write_part(&e, "006_s2.json", &v);
    b.check_fails(&e, "files に無い 006_s2.json がある");
}

#[test]
fn detects_toc_mismatch() {
    let (b, e) = setup();
    edit_index(&b, &e, |v| {
        // s1.2 の子の s1.2.1 を目次から消す
        v["toc"][0]["children"][1]["children"] = json!([]);
    });
    b.check_fails(&e, "toc に s1.2.1 が無い");
}

#[test]
fn detects_toc_file_mismatch() {
    let (b, e) = setup();
    edit_index(&b, &e, |v| {
        v["toc"][0]["children"][0]["file"] = json!("004_s1.2.json"); // s1.1 は 003_s1.1.json
    });
    b.check_fails(&e, "toc の file が実際と違う");
}

#[test]
fn detects_part_file_name_mismatch() {
    let (b, e) = setup();
    edit_part(&b, &e, "003_s1.1.json", |v| v["file"] = json!("009_s1.1.json"));
    b.check_fails(&e, "ファイルの中の file がファイル名と違う");
}

// ------------------------------------------------------------ 5. retired_ids

#[test]
fn detects_retired_id_still_present() {
    // 約束 7
    let (b, e) = setup();
    edit_index(&b, &e, |v| v["edition"]["retired_ids"] = json!(["p2.1"]));
    b.check_fails(&e, "retired_ids の p2.1 が版に残っている");
}

#[test]
fn detects_malformed_retired_ids() {
    // retired_ids は「消した ID の一覧」（配列）。配列でなければ、問題として出す。
    // 文字列 "p2.1" の p2.1 は版に残っているので、見逃すと約束 5（retired_ids の ID が残っていない）も確かめられない。
    // edition の形を check が確かめるかは、仕様に書かれていない（確認したい点）。
    let (b, e) = setup();
    edit_index(&b, &e, |v| v["edition"]["retired_ids"] = json!("p2.1"));
    b.check_fails(&e, "retired_ids が配列でなく文字列");
}

/// edition を壊す方法の一覧（理由, 壊し方）。
type Breaker = fn(&mut Value);

fn edition_breakers() -> Vec<(&'static str, Breaker)> {
    fn ed(v: &mut Value) -> &mut serde_json::Map<String, Value> {
        v["edition"].as_object_mut().unwrap()
    }
    vec![
        // 1. edition があり、オブジェクトである
        ("edition が無い", |v| {
            v.as_object_mut().unwrap().remove("edition");
        }),
        ("edition が文字列", |v| v["edition"] = json!("01_fix")),
        ("edition が配列", |v| v["edition"] = json!([])),
        ("edition が null", |v| v["edition"] = Value::Null),
        // 2. name、base、editor、created_at が文字列である
        ("name が数", |v| v["edition"]["name"] = json!(1)),
        ("name が無い", |v| {
            ed(v).remove("name");
        }),
        ("base が null", |v| v["edition"]["base"] = Value::Null),
        ("base が無い", |v| {
            ed(v).remove("base");
        }),
        ("editor が配列", |v| v["edition"]["editor"] = json!(["fix"])),
        ("editor が無い", |v| {
            ed(v).remove("editor");
        }),
        ("created_at が数", |v| v["edition"]["created_at"] = json!(20261124)),
        ("created_at が無い", |v| {
            ed(v).remove("created_at");
        }),
        // 3. retired_ids が文字列の配列である（無いのも不可）
        ("retired_ids が無い", |v| {
            ed(v).remove("retired_ids");
        }),
        ("retired_ids が文字列", |v| v["edition"]["retired_ids"] = json!("p2.9")),
        ("retired_ids に数が入っている", |v| v["edition"]["retired_ids"] = json!(["p2.9", 3])),
        ("retired_ids が null", |v| v["edition"]["retired_ids"] = Value::Null),
        // 4. retired_notes は、あればオブジェクトで、値が文字列である
        ("retired_notes が配列", |v| v["edition"]["retired_notes"] = json!(["重複"])),
        ("retired_notes が文字列", |v| v["edition"]["retired_notes"] = json!("重複")),
        ("retired_notes の値が数", |v| v["edition"]["retired_notes"] = json!({"p2.9": 1})),
        ("retired_notes の値が null", |v| v["edition"]["retired_notes"] = json!({"p2.9": null})),
    ]
}

#[test]
fn detects_malformed_edition() {
    for (why, f) in edition_breakers() {
        let (b, e) = setup();
        edit_index(&b, &e, f);
        b.check_fails(&e, why);
    }
}

#[test]
fn well_formed_retired_notes_is_fine() {
    let (b, e) = setup();
    edit_index(&b, &e, |v| {
        v["edition"]["retired_ids"] = json!(["p2.9"]);
        v["edition"]["retired_notes"] = json!({"p2.9": "重複"});
    });
    b.check_ok(&e);
    // retired_notes は無くてもよい
    edit_index(&b, &e, |v| {
        v["edition"].as_object_mut().unwrap().remove("retired_notes");
    });
    b.check_ok(&e);
}

#[test]
fn edits_stop_on_malformed_edition() {
    // 形が壊れた版に対して、編集のコマンドはエラーで止まり、版のファイルを変えない
    for (why, f) in edition_breakers() {
        let (b, e) = setup();
        edit_index(&b, &e, f);
        let before = b.snapshot(&e);
        let runs = [
            ("set-text", b.run("set-text", [e.as_str(), "p2.2", "--text", "x。"])),
            ("set-text --cell", b.run("set-text", [e.as_str(), "t1", "--cell", "1,0", "--text", "x"])),
            ("set-text --index", b.run("set-text", [e.as_str(), "f1", "--index", "0", "--text", "x"])),
            ("insert", b.run("insert", [e.as_str(), "--after", "p2.1", "--text", "x。"])),
            ("split", b.run("split", [e.as_str(), "p2.2", "--text", "a。", "--text", "b。"])),
            ("delete", b.run("delete", [e.as_str(), "p2.3", "--note", "n"])),
        ];
        for (cmd, r) in &runs {
            r.fails(&format!("{why} の版に {cmd}"));
        }
        assert!(b.snapshot(&e) == before, "{why} の版に編集のコマンドを実行したら、版が変わった");
    }
}

#[test]
fn new_stops_on_malformed_base_edition() {
    // 壊し方ごとに確かめ、見逃したものをまとめて報告する
    let mut missed = Vec::new();
    for (why, f) in edition_breakers() {
        let (b, e) = setup();
        edit_index(&b, &e, f);
        let before = b.snapshot(&e);
        let dirs = b.edition_dirs();
        let r = b.run("new", ["--from", e.as_str(), "--editor", "next"]);
        r.not_panicked(why);
        if r.success() || r.stderr.trim().is_empty() {
            missed.push(format!("{why}（stdout: {}）", r.stdout.trim()));
        } else {
            assert_eq!(b.edition_dirs(), dirs, "元の版の {why} なのに、新しい版のフォルダができた");
        }
        assert!(b.snapshot(&e) == before, "new で元の版が変わった（{why}）");
    }
    assert!(missed.is_empty(), "new が、edition の壊れた元の版から版を作った:\n{}", missed.join("\n"));
}

// ------------------------------------------------------------ 図の notes

/// 005_s2.json の f2（s2 の children の 4 つ目。text は 6 要素）。
fn f2(v: &mut Value) -> &mut Value {
    let f = &mut v["body"][0]["children"][3];
    assert_eq!(f["id"], "f2", "テスト用の原本の f2 の位置が変わった");
    f
}

#[test]
fn figure_notes_well_formed_passes() {
    let (b, e) = setup();
    b.run("set-text", [e.as_str(), "f2", "--index", "0", "--text", "1.", "--note", "n0"]).ok();
    b.run("set-text", [e.as_str(), "f2", "--index", "5", "--text", "保存。", "--note", "n5"]).ok();
    b.check_ok(&e);
    // 手で書いた正しい notes も通る（配列の最後の index まで）
    edit_part(&b, &e, "005_s2.json", |v| f2(v)["notes"] = json!({"0": "a", "5": "b"}));
    b.check_ok(&e);
}

#[test]
fn detects_malformed_figure_notes() {
    let cases: Vec<(&str, Breaker)> = vec![
        ("notes が配列", |v| f2(v)["notes"] = json!(["a"])),
        ("notes が文字列", |v| f2(v)["notes"] = json!("a")),
        ("notes が null", |v| f2(v)["notes"] = Value::Null),
        ("notes のキーが配列の範囲外（6）", |v| f2(v)["notes"] = json!({"6": "a"})),
        ("notes のキーが大きな数", |v| f2(v)["notes"] = json!({"99": "a"})),
        ("notes のキーが負の数", |v| f2(v)["notes"] = json!({"-1": "a"})),
        ("notes のキーが数でない", |v| f2(v)["notes"] = json!({"a": "a"})),
        ("notes のキーが空", |v| f2(v)["notes"] = json!({"": "a"})),
        ("notes の値が数", |v| f2(v)["notes"] = json!({"0": 1})),
        ("notes の値が null", |v| f2(v)["notes"] = json!({"0": null})),
        ("notes の値がオブジェクト", |v| f2(v)["notes"] = json!({"0": {"text": "a"}})),
    ];
    for (why, f) in cases {
        let (b, e) = setup();
        edit_part(&b, &e, "005_s2.json", f);
        b.check_fails(&e, why);
    }
}

#[test]
fn figure_notes_key_range_follows_text_length() {
    // f1 の text は 2 要素なので、キー "2" は範囲外
    let (b, e) = setup();
    edit_part(&b, &e, "002_s1.json", |v| {
        let f = s1_children(v).iter_mut().find(|c| c["id"] == "f1").unwrap();
        f["notes"] = json!({"1": "a"});
    });
    b.check_ok(&e);
    edit_part(&b, &e, "002_s1.json", |v| {
        let f = s1_children(v).iter_mut().find(|c| c["id"] == "f1").unwrap();
        f["notes"] = json!({"1": "a", "2": "b"});
    });
    b.check_fails(&e, "f1 の notes のキー 2 が配列の範囲外");
}

#[test]
fn retired_id_not_present_is_fine() {
    let (b, e) = setup();
    edit_index(&b, &e, |v| v["edition"]["retired_ids"] = json!(["p2.9"]));
    b.check_ok(&e);
}

// ------------------------------------------------------------ 6. edition.name

#[test]
fn detects_edition_name_mismatch() {
    let (b, e) = setup();
    edit_index(&b, &e, |v| v["edition"]["name"] = json!("99_other"));
    b.check_fails(&e, "edition.name がフォルダ名と違う");
}

#[test]
fn detects_copied_edition_folder() {
    // フォルダをコピーして名前を変えただけの版は、edition.name が合わない
    let (b, e) = setup();
    copy_dir(&b.edition(&e), &b.edition("02_copy"));
    b.check_fails("02_copy", "edition.name がフォルダ名と違う（コピーした版）");
}

#[test]
fn check_does_not_modify_edition() {
    let (b, e) = setup();
    b.run("set-text", [e.as_str(), "p2.2", "--text", "x。"]).ok();
    let before = b.snapshot(&e);
    let before_original = b.snapshot(ORIGINAL);
    b.check_ok(&e);
    b.check_ok(ORIGINAL);
    assert!(b.snapshot(&e) == before, "check が版を書き換えた");
    edit_index(&b, &e, |v| v["edition"]["retired_ids"] = json!(["p2.1"]));
    let broken = b.snapshot(&e);
    b.check_fails(&e, "retired_ids");
    assert!(b.snapshot(&e) == broken, "check が壊れた版を書き換えた");
    assert!(b.snapshot(ORIGINAL) == before_original, "check が original を書き換えた");
}
