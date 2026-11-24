//! `set-text --text-file`（文字をファイルや標準入力から受け取る）のテスト。
//!
//! 今回のきっかけは、`show` の JSON から文を取り出して `--text` に渡し、`\"` のまま書き込んでセルをこわしたこと。
//! `show --text` → ファイル → `set-text --text-file` の往復で、版が 1 バイトも変わらないことを中心に確かめる。

mod common;

use std::fs;

use common::*;
use serde_json::json;

fn setup() -> (Book, String) {
    let b = Book::new();
    let e = b.new_edition(ORIGINAL, "fix");
    (b, e)
}

/// 対象の指定（ID と、--cell / --index）。
struct Target {
    id: &'static str,
    extra: &'static [&'static str],
    text: &'static str,
}

/// 特殊な文字を含む対象と、ふつうの対象。
const TARGETS: [Target; 10] = [
    Target { id: "p7.1", extra: &[], text: P7_1 },
    Target { id: "p7.2", extra: &[], text: P7_2 },
    Target { id: "p2.2", extra: &[], text: "二番目の文は曰本語です。" },
    Target { id: "l1.1.2", extra: &[], text: "項目の二文目です。" },
    Target { id: "c4", extra: &[], text: C4 },
    Target { id: "t3", extra: &["--cell", "0,0"], text: T3_0_0 },
    Target { id: "t3", extra: &["--cell", "0,1"], text: T3_0_1 },
    Target { id: "t1", extra: &["--cell", "1,0"], text: "りんご" },
    Target { id: "f3", extra: &["--index", "0"], text: F3_0 },
    Target { id: "f3", extra: &["--index", "1"], text: F3_1 },
];

impl Target {
    fn args<'a>(&'a self, edition: &'a str, tail: &[&'a str]) -> Vec<&'a str> {
        let mut v = vec![edition, self.id];
        v.extend_from_slice(self.extra);
        v.extend_from_slice(tail);
        v
    }

    fn label(&self) -> String {
        format!("{} {}", self.id, self.extra.join(" "))
    }

    /// 版の中の、今の文字。
    fn current(&self, b: &Book, edition: &str) -> String {
        let el = b.get(edition, self.id);
        match self.extra.first().copied() {
            Some("--cell") => {
                let (r, c) = self.extra[1].split_once(',').unwrap();
                let (r, c): (u64, u64) = (r.parse().unwrap(), c.parse().unwrap());
                let cell = el["cells"].as_array().unwrap().iter().find(|x| x["row"] == r && x["col"] == c).unwrap();
                cell["text"].as_str().unwrap().to_string()
            }
            Some("--index") => {
                let i: usize = self.extra[1].parse().unwrap();
                el["text"][i].as_str().unwrap().to_string()
            }
            _ => el["text"].as_str().unwrap().to_string(),
        }
    }

    /// 直した要素（センテンス、キャプション、セル、図）。origin・note・notes を見るため。
    fn edited(&self, b: &Book, edition: &str) -> serde_json::Value {
        let el = b.get(edition, self.id);
        if self.extra.first().copied() == Some("--cell") {
            let (r, c) = self.extra[1].split_once(',').unwrap();
            let (r, c): (u64, u64) = (r.parse().unwrap(), c.parse().unwrap());
            return el["cells"].as_array().unwrap().iter().find(|x| x["row"] == r && x["col"] == c).unwrap().clone();
        }
        el
    }
}

/// `show --text` の出力を、ファイルに書く。
fn show_text_to_file(b: &Book, e: &str, t: &Target, path: &std::path::Path) -> Vec<u8> {
    let r = b.run("show", t.args(e, &["--text"]));
    r.ok();
    assert!(r.stdout_bytes == t.text.as_bytes(), "show --text {} の出力が文字そのものと違う: {:?}", t.label(), r.stdout);
    fs::write(path, &r.stdout_bytes).unwrap();
    r.stdout_bytes.clone()
}

// ============================================================ 往復で版が変わらない

#[test]
fn round_trip_through_file_changes_nothing() {
    // show --text > a.txt; set-text --text-file a.txt で、版が 1 バイトも変わらない（origin も付かない）
    for t in &TARGETS {
        let (b, e) = setup();
        let before = b.snapshot(&e);
        let path = b.scratch("a.txt");
        show_text_to_file(&b, &e, t, &path);
        let p = path.to_str().unwrap();
        b.run("set-text", t.args(&e, &["--text-file", p])).ok();
        assert!(b.snapshot(&e) == before, "{} の往復で、版が変わった", t.label());
        let el = t.edited(&b, &e);
        assert!(el.get("origin").is_none(), "{} の往復で origin が付いた: {el}", t.label());
    }
}

#[test]
fn round_trip_through_stdin_changes_nothing() {
    // --text-file - で、標準入力から読む
    for t in &TARGETS {
        let (b, e) = setup();
        let before = b.snapshot(&e);
        let r = b.run("show", t.args(&e, &["--text"]));
        r.ok();
        b.run_stdin("set-text", t.args(&e, &["--text-file", "-"]), &r.stdout_bytes).ok();
        assert!(b.snapshot(&e) == before, "{} の標準入力での往復で、版が変わった", t.label());
    }
}

#[test]
fn round_trip_on_edited_edition_changes_nothing() {
    // 直した後の文でも、往復で変わらない（origin・note もそのまま）
    let (b, e) = setup();
    b.run("set-text", [e.as_str(), "p7.1", "--text", "直した \"文\" \\。", "--note", "n"]).ok();
    b.run("set-text", [e.as_str(), "f3", "--index", "1", "--text", "直した😀", "--note", "m"]).ok();
    let before = b.snapshot(&e);
    for (id, extra) in [("p7.1", vec![]), ("f3", vec!["--index", "1"])] {
        let mut args = vec![e.as_str(), id];
        args.extend(extra.iter().copied());
        let r = b.run("show", args.iter().copied().chain(["--text"]));
        r.ok();
        let path = b.scratch("a.txt");
        fs::write(&path, &r.stdout_bytes).unwrap();
        b.run("set-text", args.iter().copied().chain(["--text-file", path.to_str().unwrap()])).ok();
    }
    assert!(b.snapshot(&e) == before, "直した後の文の往復で、版が変わった");
}

// ============================================================ 1 か所だけ置き換える（今回のきっかけ）

#[test]
fn replace_one_place_via_file() {
    // show --text の出力の 1 か所だけを置き換えて --text-file で渡すと、その 1 か所だけが変わる
    let cases: [(&Target, &str, &str); 5] = [
        (&TARGETS[0], "引用", "引き写し"),
        (&TARGETS[4], "記号", "きごう"),
        (&TARGETS[6], "行2", "二行目"),
        (&TARGETS[5], "B", "C"),
        (&TARGETS[9], "二行目", "2 行目"),
    ];
    for (t, from, to) in cases {
        let (b, e) = setup();
        let path = b.scratch("a.txt");
        let got = show_text_to_file(&b, &e, t, &path);
        let replaced = String::from_utf8(got).unwrap().replacen(from, to, 1);
        fs::write(&path, replaced.as_bytes()).unwrap();
        let p = path.to_str().unwrap();
        b.run("set-text", t.args(&e, &["--text-file", p, "--note", "一か所だけ直す"])).ok();

        assert_eq!(t.current(&b, &e), replaced, "{} の文字が、置き換えた文字になっていない", t.label());
        let el = t.edited(&b, &e);
        match t.extra.first().copied() {
            Some("--index") => {
                // 図: origin は図の要素に 1 つで、text は直す前の配列の全体。理由は notes
                assert_eq!(el["origin"], json!({"edition": "original", "ids": ["f3"], "text": [F3_0, F3_1]}));
                let mut notes = serde_json::Map::new();
                notes.insert(t.extra[1].to_string(), json!("一か所だけ直す"));
                assert_eq!(el["notes"], serde_json::Value::Object(notes));
                assert!(el.get("note").is_none());
            }
            Some("--cell") => {
                assert_eq!(el["origin"], json!({"edition": "original", "ids": [t.id], "text": t.text}));
                assert_eq!(el["note"], "一か所だけ直す");
            }
            _ => {
                assert_eq!(el["origin"], json!({"edition": "original", "ids": [t.id], "text": t.text}));
                assert_eq!(el["note"], "一か所だけ直す");
            }
        }
        b.check_ok(&e);
        // diff に、その要素だけが出る
        let out = b.run("diff", [ORIGINAL, e.as_str()]);
        out.ok();
        for (id, _) in SENTENCES {
            if id != t.id {
                assert!(!mentions_id(&out.stdout, id), "{} だけを直したのに diff に {id} が出ている:\n{}", t.label(), out.stdout);
            }
        }
    }
}

#[test]
fn replace_one_place_via_stdin() {
    // 標準入力でも、同じ結果になる
    for (t, from, to) in [(&TARGETS[0], "引用", "引き写し"), (&TARGETS[6], "行2", "二行目"), (&TARGETS[9], "二行目", "2 行目")] {
        let (b1, e1) = setup();
        let (b2, e2) = setup();
        let replaced = t.text.replacen(from, to, 1);

        let path = b1.scratch("a.txt");
        fs::write(&path, replaced.as_bytes()).unwrap();
        b1.run("set-text", t.args(&e1, &["--text-file", path.to_str().unwrap(), "--note", "n"])).ok();
        b2.run_stdin("set-text", t.args(&e2, &["--text-file", "-", "--note", "n"]), replaced.as_bytes()).ok();

        assert_eq!(t.current(&b2, &e2), replaced);
        // created_at 以外は、ファイルの場合と同じ
        let x = b1.snapshot(&e1);
        let y = b2.snapshot(&e2);
        for (f, bytes) in &x {
            if f == "index.json" {
                continue;
            }
            assert!(y.get(f) == Some(bytes), "{} を標準入力で直した結果が、ファイルで直した結果と違う（{f}）", t.label());
        }
        assert_eq!(t.edited(&b1, &e1), t.edited(&b2, &e2));
    }
}

// ============================================================ ファイルの中身がそのまま新しい文字になる

#[test]
fn file_content_is_used_byte_for_byte() {
    // 末尾の改行や CRLF、BOM でない先頭の空白も含めて、1 バイトも変わらない
    let contents: [&str; 5] = [
        "末尾に改行。\n",
        "末尾に改行が二つ。\n\n",
        "CRLF の\r\n改行。\r\n",
        "  先頭と末尾の空白  ",
        "\"引用\" \\ \t 😀𠮷",
    ];
    for content in contents {
        let (b, e) = setup();
        let path = b.scratch("a.txt");
        fs::write(&path, content.as_bytes()).unwrap();
        b.run("set-text", [e.as_str(), "p2.2", "--text-file", path.to_str().unwrap()]).ok();
        assert_eq!(b.text(&e, "p2.2"), content, "ファイルの中身がそのまま新しい文字になっていない");
        // show --text で、同じバイト列が出る
        let r = b.run("show", [e.as_str(), "p2.2", "--text"]);
        r.ok();
        assert!(r.stdout_bytes == content.as_bytes(), "直した文の show --text が、ファイルの中身と違う: {:?}", r.stdout);
        b.check_ok(&e);
    }
}

#[test]
fn bom_is_part_of_text() {
    // 実装が決めた仕様: BOM は特別扱いしない（そのまま文字になる）
    let (b, e) = setup();
    let content = "\u{feff}BOM 付きの文。\n";
    let path = b.scratch("bom.txt");
    fs::write(&path, content.as_bytes()).unwrap();
    assert_eq!(&fs::read(&path).unwrap()[..3], &[0xef, 0xbb, 0xbf]);
    b.run("set-text", [e.as_str(), "p2.2", "--text-file", path.to_str().unwrap()]).ok();
    assert_eq!(b.text(&e, "p2.2"), content, "BOM がそのまま文字になっていない");
    let r = b.run("show", [e.as_str(), "p2.2", "--text"]);
    r.ok();
    assert!(r.stdout_bytes == content.as_bytes(), "BOM 付きの文の show --text が、ファイルの中身と違う");
    // 標準入力でも同じ
    let (b2, e2) = setup();
    b2.run_stdin("set-text", [e2.as_str(), "t1", "--cell", "1,0", "--text-file", "-"], content.as_bytes()).ok();
    let r = b2.run("show", [e2.as_str(), "t1", "--cell", "1,0", "--text"]);
    r.ok();
    assert!(r.stdout_bytes == content.as_bytes(), "標準入力の BOM がそのまま文字になっていない");
    b.check_ok(&e);
    b2.check_ok(&e2);
}

#[test]
fn empty_file_or_stdin_is_rejected() {
    // 空のファイル（0 バイト）と空の標準入力は、エラーにする。版は変わらない
    // （失敗したパイプの結果で、文を空にしてしまう事故を防ぐため）
    let (b, e) = setup();
    let path = b.scratch("empty.txt");
    fs::write(&path, b"").unwrap();
    let p = path.to_str().unwrap();
    let before = b.snapshot(&e);
    b.run("set-text", [e.as_str(), "p2.2", "--text-file", p]).fails("空のファイル（センテンス）");
    b.run("set-text", [e.as_str(), "c1", "--text-file", p]).fails("空のファイル（キャプション）");
    b.run("set-text", [e.as_str(), "t1", "--cell", "1,0", "--text-file", p]).fails("空のファイル（セル）");
    b.run("set-text", [e.as_str(), "f1", "--index", "1", "--text-file", p]).fails("空のファイル（図の文字）");
    b.run_stdin("set-text", [e.as_str(), "p2.2", "--text-file", "-"], b"").fails("空の標準入力（センテンス）");
    b.run_stdin("set-text", [e.as_str(), "f1", "--index", "1", "--text-file", "-"], b"").fails("空の標準入力（図の文字）");
    assert!(b.snapshot(&e) == before, "空の入力でエラーなのに、版が変わった");
}

#[test]
fn empty_text_option_still_works() {
    // --text ""（これまでの指定）の動きは変わらない。空の文字にできる
    let (b, e) = setup();
    b.run("set-text", [e.as_str(), "p2.2", "--text", ""]).ok();
    assert_eq!(b.text(&e, "p2.2"), "");
    assert_eq!(b.get(&e, "p2.2")["origin"]["text"], "二番目の文は曰本語です。");
    let r = b.run("show", [e.as_str(), "p2.2", "--text"]);
    r.ok();
    assert!(r.stdout_bytes.is_empty(), "空の文の show --text に何か出ている: {:?}", r.stdout);
    b.check_ok(&e);
}

#[test]
fn whitespace_only_content_is_not_empty() {
    // 空白や改行だけの中身（1 バイト以上）は、エラーにならず、そのまま文字になる
    for content in ["\n", " ", "  \n", "\t", "\r\n", "\u{3000}"] {
        let (b, e) = setup();
        let path = b.scratch("ws.txt");
        fs::write(&path, content.as_bytes()).unwrap();
        b.run("set-text", [e.as_str(), "p2.2", "--text-file", path.to_str().unwrap()]).ok();
        assert_eq!(b.text(&e, "p2.2"), content, "空白や改行だけの中身 {content:?} が、そのまま文字になっていない");
        b.run_stdin("set-text", [e.as_str(), "t1", "--cell", "1,0", "--text-file", "-"], content.as_bytes()).ok();
        let r = b.run("show", [e.as_str(), "t1", "--cell", "1,0", "--text"]);
        r.ok();
        assert!(r.stdout_bytes == content.as_bytes(), "標準入力の {content:?} が、そのまま文字になっていない");
        b.check_ok(&e);
    }
}

#[test]
fn set_text_with_both_cell_and_index_fails() {
    // 実装が決めた仕様（--cell と --index は一緒に使えない）を、set-text にも当てはめた読み方
    let (b, e) = setup();
    let before = b.snapshot(&e);
    b.run("set-text", [e.as_str(), "t1", "--cell", "1,0", "--index", "0", "--text", "x"]).fails("--cell と --index の両方");
    assert!(b.snapshot(&e) == before);
}

#[test]
fn text_file_with_cell_and_index() {
    let (b, e) = setup();
    let path = b.scratch("a.txt");
    fs::write(&path, "みかん\n").unwrap();
    let p = path.to_str().unwrap();
    b.run("set-text", [e.as_str(), "t1", "--cell", "1,0", "--text-file", p, "--note", "n"]).ok();
    fs::write(&path, "矢じるし").unwrap();
    b.run("set-text", [e.as_str(), "f1", "--index", "1", "--text-file", p, "--note", "m"]).ok();
    fs::write(&path, "図1 試験の図").unwrap();
    b.run("set-text", [e.as_str(), "c1", "--text-file", p]).ok();

    let t1 = b.get(&e, "t1");
    let cell = t1["cells"].as_array().unwrap().iter().find(|c| c["row"] == 1 && c["col"] == 0).unwrap().clone();
    assert_eq!(cell["text"], "みかん\n");
    assert_eq!(cell["note"], "n");
    let f1 = b.get(&e, "f1");
    assert_eq!(f1["text"], json!(["図の中の文宇", "矢じるし"]));
    assert_eq!(f1["notes"], json!({"1": "m"}));
    assert_eq!(b.get(&e, "c1")["text"], "図1 試験の図");
    assert!(b.get(&e, "c1").get("note").is_none());
    b.check_ok(&e);
}

#[test]
fn text_file_back_to_base_removes_origin() {
    // ファイルで base の文に戻しても、origin と note は消える
    let (b, e) = setup();
    b.run("set-text", [e.as_str(), "p7.2", "--text", "一時的な文。", "--note", "n"]).ok();
    let path = b.scratch("a.txt");
    fs::write(&path, P7_2.as_bytes()).unwrap();
    b.run("set-text", [e.as_str(), "p7.2", "--text-file", path.to_str().unwrap()]).ok();
    let s = b.get(&e, "p7.2");
    assert_eq!(s["text"], P7_2);
    assert!(s.get("origin").is_none() && s.get("note").is_none(), "base の文に戻したのに origin・note が残っている: {s}");
}

// ============================================================ エラー

#[test]
fn text_file_errors() {
    let (b, e) = setup();
    let ok = b.scratch("ok.txt");
    fs::write(&ok, "文。").unwrap();
    let ok = ok.to_str().unwrap().to_string();
    let bad = b.scratch("bad.txt");
    fs::write(&bad, [0xe6u8, 0x96, 0x87, 0xff, 0xfe, 0x0a]).unwrap(); // UTF-8 でない
    let bad = bad.to_str().unwrap().to_string();
    let missing = b.scratch("missing.txt").to_str().unwrap().to_string();

    let before = b.snapshot(&e);
    b.run("set-text", [e.as_str(), "p2.2", "--text-file", bad.as_str()]).fails("UTF-8 でないファイル");
    b.run("set-text", [e.as_str(), "p2.2", "--text-file", missing.as_str()]).fails("無いファイル");
    b.run("set-text", [e.as_str(), "p2.2", "--text", "x", "--text-file", ok.as_str()]).fails("--text と --text-file の両方");
    b.run("set-text", [e.as_str(), "p2.2"]).fails("--text も --text-file も無い");
    b.run("set-text", [e.as_str(), "p2.2", "--note", "n"]).fails("--note だけ");
    b.run_stdin("set-text", [e.as_str(), "p2.2", "--text-file", "-"], &[0xffu8, 0xfe]).fails("標準入力が UTF-8 でない");
    b.run("set-text", [e.as_str(), "s1", "--text-file", ok.as_str()]).fails("見出しに --text-file");
    b.run("set-text", [e.as_str(), "f1", "--text-file", ok.as_str()]).fails("図に --index が無い");
    b.run("set-text", [e.as_str(), "t1", "--text-file", ok.as_str()]).fails("表に --cell が無い");
    assert!(b.snapshot(&e) == before, "失敗したのに版が変わった");
}

#[test]
fn text_file_on_original_is_rejected() {
    let b = Book::new();
    let before = b.snapshot(ORIGINAL);
    let path = b.scratch("a.txt");
    fs::write(&path, "x").unwrap();
    let p = path.to_str().unwrap();
    b.run("set-text", [ORIGINAL, "p2.2", "--text-file", p]).fails("original に --text-file");
    b.run("set-text", [ORIGINAL, "t1", "--cell", "1,0", "--text-file", p]).fails("original のセルに --text-file");
    b.run("set-text", [ORIGINAL, "f1", "--index", "0", "--text-file", p]).fails("original の図に --text-file");
    b.run_stdin("set-text", [ORIGINAL, "p2.2", "--text-file", "-"], b"x").fails("original に標準入力");
    assert!(b.snapshot(ORIGINAL) == before, "original が変わった");
}

#[test]
fn text_file_is_not_modified() {
    // --text-file に渡したファイルは、読むだけ
    let (b, e) = setup();
    let path = b.scratch("a.txt");
    fs::write(&path, P7_1.as_bytes()).unwrap();
    b.run("set-text", [e.as_str(), "p2.2", "--text-file", path.to_str().unwrap()]).ok();
    assert_eq!(fs::read(&path).unwrap(), P7_1.as_bytes());
}
