//! 版を作るコマンド（`new`）のテスト。

mod common;

use common::*;
use serde_json::Value;

#[test]
fn first_edition_is_01() {
    let b = Book::new();
    let e = b.new_edition(ORIGINAL, "ocr-fix");
    assert_eq!(e, "01_ocr-fix");
    assert!(b.edition("01_ocr-fix").is_dir());
}

#[test]
fn number_is_max_plus_one() {
    let b = Book::new();
    assert_eq!(b.new_edition(ORIGINAL, "a"), "01_a");
    assert_eq!(b.new_edition(ORIGINAL, "b"), "02_b");
    // 連番は「数」ではなく「最大 + 1」。05 があれば、次は 06（空いている 03 は使わない）
    copy_dir(&b.edition("01_a"), &b.edition("05_manual"));
    assert_eq!(b.new_edition("01_a", "c"), "06_c");
}

#[test]
fn new_copies_every_file() {
    let b = Book::new();
    let e = b.new_edition(ORIGINAL, "copy");
    let mut want: Vec<String> = PART_FILES.iter().map(|s| s.to_string()).collect();
    want.push("index.json".into());
    want.sort();
    let got: Vec<String> = b.snapshot(&e).keys().cloned().collect();
    assert_eq!(got, want);
    assert_eq!(b.files(&e), PART_FILES.to_vec());
}

#[test]
fn new_adds_edition_and_keeps_the_rest_of_index() {
    let b = Book::new();
    let e = b.new_edition(ORIGINAL, "ocr-fix");
    let base = b.index(ORIGINAL);
    let idx = b.index(&e);

    let ed = &idx["edition"];
    assert_eq!(ed["name"], "01_ocr-fix");
    assert_eq!(ed["base"], ORIGINAL);
    assert_eq!(ed["editor"], "ocr-fix");
    let created = ed["created_at"].as_str().expect("created_at が文字列でない");
    assert!(created.contains('T') && created.len() >= 19, "created_at が日時に見えない: {created}");
    assert_eq!(ed["retired_ids"], Value::Array(vec![]), "最初の版の retired_ids は空");

    // edition 以外は元のまま
    for key in ["version", "title", "source", "split_chars", "pages", "files", "toc"] {
        assert_eq!(idx[key], base[key], "index.json の {key} が元の版と違う");
    }
}

#[test]
fn new_edition_passes_check_and_has_no_diff() {
    // 約束 5（前半）
    let b = Book::new();
    let e = b.new_edition(ORIGINAL, "copy");
    b.check_ok(&e);
    let r = b.run("diff", [ORIGINAL, e.as_str()]);
    r.ok();
    for (id, _) in SENTENCES {
        assert!(!mentions_id(&r.stdout, id), "new しただけなのに diff に {id} が出ている:\n{}", r.stdout);
    }
    for id in ["p2", "l1", "f1", "c1", "t1", "s1"] {
        assert!(!mentions_id(&r.stdout, id), "new しただけなのに diff に {id} が出ている:\n{}", r.stdout);
    }
}

#[test]
fn new_edition_files_are_byte_identical() {
    // 約束 5（後半・理想）: 何も直していないファイルは、元とバイト単位で同じ。
    // 実装が難しいと報告される可能性があるので、別のテストにしている。
    let b = Book::new();
    let e = b.new_edition(ORIGINAL, "copy");
    let base = b.snapshot(ORIGINAL);
    let new = b.snapshot(&e);
    for f in PART_FILES {
        assert!(
            base[f] == new[f],
            "{f} が元の版とバイト単位で違う\n--- original\n{}\n--- {e}\n{}",
            String::from_utf8_lossy(&base[f]),
            String::from_utf8_lossy(&new[f])
        );
    }
}

#[test]
fn unedited_files_stay_byte_identical_after_edit() {
    // 約束 5（後半・理想）の続き: 1 ファイルだけ直したとき、ほかのファイルは元とバイト単位で同じ
    let b = Book::new();
    let e = b.new_edition(ORIGINAL, "fix");
    b.run("set-text", [e.as_str(), "p2.2", "--text", "二番目の文は日本語です。"]).ok();
    let base = b.snapshot(ORIGINAL);
    let new = b.snapshot(&e);
    for f in PART_FILES.iter().filter(|f| **f != "002_s1.json") {
        assert!(base[*f] == new[*f], "直していない {f} が元の版とバイト単位で違う");
    }
}

#[test]
fn new_from_edited_edition() {
    // 約束 9: 版を重ねられる
    let b = Book::new();
    let a = b.new_edition(ORIGINAL, "a");
    b.run("set-text", [a.as_str(), "p2.2", "--text", "二番目の文は日本語です。", "--note", "曰→日"]).ok();
    let bb = b.new_edition(&a, "b");
    assert_eq!(bb, "02_b");
    let ed = &b.index(&bb)["edition"];
    assert_eq!(ed["name"], "02_b");
    assert_eq!(ed["base"], "01_a");
    assert_eq!(ed["editor"], "b");
    assert_eq!(b.text(&bb, "p2.2"), "二番目の文は日本語です。");
    b.check_ok(&bb);
}

#[test]
fn new_drops_origin_and_note_carried_from_base() {
    // origin は 1 つ前の版との対応だけ。new でコピーすると、持ち越した origin と note は消える
    let b = Book::new();
    let a = b.new_edition(ORIGINAL, "a");
    b.run("set-text", [a.as_str(), "p2.2", "--text", "二番目の文は日本語です。", "--note", "曰→日"]).ok();
    b.run("insert", [a.as_str(), "--after", "p2.3", "--text", "足した文です。"]).ok();
    assert!(b.get(&a, "p2.2").get("origin").is_some());

    let bb = b.new_edition(&a, "b");
    for (id, el) in b.all_elements(&bb) {
        assert!(el.get("origin").is_none(), "{bb} の {id} に、元の版の origin が残っている");
        assert!(el.get("note").is_none(), "{bb} の {id} に、元の版の note が残っている");
    }
    // 中身は引き継ぐ
    assert_eq!(b.text(&bb, "p2.2"), "二番目の文は日本語です。");
    assert_eq!(b.text(&bb, "p2.4"), "足した文です。");
}

#[test]
fn new_inherits_retired_ids() {
    let b = Book::new();
    let a = b.new_edition(ORIGINAL, "a");
    b.run("delete", [a.as_str(), "p2.3"]).ok();
    assert_eq!(b.retired_ids(&a), vec!["p2.3".to_string()]);
    let bb = b.new_edition(&a, "b");
    assert_eq!(b.retired_ids(&bb), vec!["p2.3".to_string()]);
    b.check_ok(&bb);
}

#[test]
fn new_does_not_touch_original_or_base() {
    let b = Book::new();
    let before = b.snapshot(ORIGINAL);
    let a = b.new_edition(ORIGINAL, "a");
    let a_before = b.snapshot(&a);
    b.new_edition(&a, "b");
    assert!(b.snapshot(ORIGINAL) == before, "new で original が変わった");
    assert!(b.snapshot(&a) == a_before, "new で元の版が変わった");
}

#[test]
fn new_accepts_lowercase_digits_and_hyphen() {
    let b = Book::new();
    assert_eq!(b.new_edition(ORIGINAL, "ocr-fix2"), "01_ocr-fix2");
    assert_eq!(b.new_edition(ORIGINAL, "3rd"), "02_3rd");
}

#[test]
fn new_rejects_bad_editor_names() {
    // 約束 2: 不正な編集者名は拒否される
    let b = Book::new();
    for bad in [
        "", "-", "---", "..", "a/b", "a\\b", "../x", "a..b", "OCR", "Ocr-fix", "ocr_fix", "ocr fix", "ocr.fix", "日本語",
    ] {
        let before = b.edition_dirs();
        b.run("new", ["--from", ORIGINAL, "--editor", bad])
            .fails(&format!("編集者名 {bad:?}"));
        assert_eq!(b.edition_dirs(), before, "拒否したのに版のフォルダが増えた（編集者名 {bad:?}）");
    }
    assert!(!b.out.join("x").exists(), "structure/ の外にフォルダができた");
}

#[test]
fn new_rejects_bad_or_missing_base() {
    let b = Book::new();
    for bad in ["../original", "a/b", "a\\b", "..", "99_nothing"] {
        let before = b.edition_dirs();
        b.run("new", ["--from", bad, "--editor", "x"])
            .fails(&format!("元の版 {bad:?}"));
        assert_eq!(b.edition_dirs(), before, "拒否したのに版のフォルダが増えた（元の版 {bad:?}）");
    }
}

#[test]
fn new_requires_from_and_editor() {
    let b = Book::new();
    b.run("new", ["--editor", "x"]).fails("--from が無い");
    b.run("new", ["--from", ORIGINAL]).fails("--editor が無い");
    assert_eq!(b.edition_dirs(), vec![ORIGINAL.to_string()]);
}
