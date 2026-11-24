//! コマンドの定義（clap）と、コマンドごとの処理。
//! 結果は標準出力、エラーは標準エラー。成功は 0、失敗は 0 以外。

use crate::edition::*;
use crate::{check, diff, edit};
use clap::{Parser, Subcommand};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "melta", version, about = "Tyggja の原本から、編集した版を作る道具")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// 版の一覧と系譜（name、base、editor、created_at）を出す
    List { out: PathBuf },
    /// 目次（toc）とファイルの一覧（files）を出す
    Toc { out: PathBuf, edition: String },
    /// ID で要素を探して JSON で出す（--file でファイルをそのまま出す）
    Show {
        out: PathBuf,
        edition: String,
        id: Option<String>,
        /// 表の行を絞る（例: 1-20。セルの row の値で、両端を含む）
        #[arg(long)]
        rows: Option<String>,
        /// 1 ファイルをそのまま出す
        #[arg(long)]
        file: Option<String>,
        /// 表のセル 1 つだけ出す（<row>,<col>）
        #[arg(long)]
        cell: Option<String>,
        /// 図の中の文字の n 番目だけ出す（0 から）
        #[arg(long)]
        index: Option<usize>,
        /// JSON にせず、文字そのものを出す（エスケープ・引用符・末尾の改行なし）
        #[arg(long)]
        text: bool,
    },
    /// 要素の一覧（入れ子の深さ・type・ID・大きさ・先頭の文字）を、読み順に出す。読むだけ
    Outline {
        out: PathBuf,
        edition: String,
        id: Option<String>,
        /// このファイルの中の要素を出す
        #[arg(long)]
        file: Option<String>,
    },
    /// 元の版を全部コピーして、新しい版を作る（作った版の名前を出す）
    New {
        out: PathBuf,
        #[arg(long)]
        from: String,
        #[arg(long)]
        editor: String,
    },
    /// センテンス・キャプション・図の中の文字・表のセルを直す
    SetText {
        out: PathBuf,
        edition: String,
        id: String,
        #[arg(long, allow_hyphen_values = true)]
        text: Option<String>,
        /// 新しい文字をファイルから読む（そのまま。- は標準入力）
        #[arg(long, allow_hyphen_values = true)]
        text_file: Option<String>,
        #[arg(long, allow_hyphen_values = true)]
        note: Option<String>,
        /// 表のセル（<row>,<col>）
        #[arg(long)]
        cell: Option<String>,
        /// 図の中の文字の何番目か（0 から。図では必須）
        #[arg(long)]
        index: Option<usize>,
    },
    /// センテンスを足す
    Insert {
        out: PathBuf,
        edition: String,
        /// このセンテンスの後ろに足す
        #[arg(long, conflicts_with = "first_in")]
        after: Option<String>,
        /// このパラグラフ（項目）の先頭に足す
        #[arg(long)]
        first_in: Option<String>,
        #[arg(long, allow_hyphen_values = true)]
        text: String,
        #[arg(long, allow_hyphen_values = true)]
        note: Option<String>,
    },
    /// センテンスを分ける（1 つ目が元の ID を引き継ぐ）
    Split {
        out: PathBuf,
        edition: String,
        id: String,
        #[arg(long, allow_hyphen_values = true, required = true)]
        text: Vec<String>,
        #[arg(long, allow_hyphen_values = true)]
        note: Option<String>,
    },
    /// センテンスを消す
    Delete {
        out: PathBuf,
        edition: String,
        id: String,
        #[arg(long, allow_hyphen_values = true)]
        note: Option<String>,
    },
    /// 版の形式を確かめる（問題があれば終了コードが 0 以外）
    Check { out: PathBuf, edition: String },
    /// 2 つの版の差分を Markdown で出す
    Diff { out: PathBuf, a: String, b: String },
}

/// 終了コードを返す。
pub fn run(cli: Cli) -> Res<i32> {
    match cli.command {
        Command::List { out } => {
            for name in edition_names(&out)? {
                let index = read_json(&edition_dir(&out, &name)?.join(INDEX_FILE))?;
                let e = index.get("edition");
                let f = |k: &str| e.and_then(|e| str_of(e, k)).unwrap_or("-");
                println!("{}\t{}\t{}\t{}", name, f("base"), f("editor"), f("created_at"));
            }
        }
        Command::Toc { out, edition } => {
            let ed = Edition::load(&out, &edition)?;
            println!("# toc");
            fn toc(nodes: &Value, depth: usize) {
                for n in nodes.as_array().into_iter().flatten() {
                    println!(
                        "{}{}\t{}\t{}",
                        "  ".repeat(depth),
                        str_of(n, "id").unwrap_or("?"),
                        str_of(n, "title").unwrap_or(""),
                        str_of(n, "file").unwrap_or("")
                    );
                    if let Some(c) = n.get("children") {
                        toc(c, depth + 1);
                    }
                }
            }
            if let Some(t) = ed.index.get("toc") {
                toc(t, 0);
            }
            println!("# files");
            for f in ed.index.get("files").and_then(|f| f.as_array()).into_iter().flatten() {
                println!(
                    "{}\t{}\t{}\t{}",
                    str_of(f, "file").unwrap_or("?"),
                    str_of(f, "id").unwrap_or("-"),
                    f.get("chars").map(|x| x.to_string()).unwrap_or_default(),
                    f.get("json_chars").map(|x| x.to_string()).unwrap_or_default()
                );
            }
        }
        Command::Show { out, edition, id, rows, file, cell, index, text } => {
            if file.is_some() && (rows.is_some() || cell.is_some() || index.is_some() || text) {
                return Err("--file は、--rows、--cell、--index、--text と一緒に使えません".to_string());
            }
            if cell.is_some() && rows.is_some() {
                return Err("--cell と --rows は同時に指定できません".to_string());
            }
            if cell.is_some() && index.is_some() {
                return Err("--cell と --index は同時に指定できません".to_string());
            }
            if text && rows.is_some() {
                return Err("--text は --rows と一緒に使えません".to_string());
            }
            match (&file, &id) {
                (Some(_), Some(_)) => return Err("ID と --file は同時に指定できません".to_string()),
                (None, None) => return Err("ID か --file のどちらか 1 つを指定してください".to_string()),
                _ => {}
            }
            if let Some(file) = file {
                validate_file_name(&file)?;
                let path = edition_dir(&out, &edition)?.join(&file);
                let text = fs::read_to_string(&path).map_err(|e| format!("{} を読めません: {e}", path.display()))?;
                print!("{text}");
            } else {
                let id = id.ok_or("ID か --file を指定してください")?;
                let ed = Edition::load(&out, &edition)?;
                let (pi, ptr) = ed.find_id(&id).ok_or_else(|| format!("ID {id} の要素が見つかりません"))?;
                let found = ed.parts[pi].value.pointer(&ptr).unwrap();
                if cell.is_some() || index.is_some() || text {
                    let out_text = show_one(found, &id, cell.as_deref(), index, text)?;
                    print!("{out_text}");
                    return Ok(0);
                }
                let mut node = ed.parts[pi].value.pointer(&ptr).unwrap().clone();
                if let Some(r) = rows {
                    if str_of(&node, "type") != Some("table") {
                        return Err(format!("--rows は表の ID にだけ使えます（{id}）"));
                    }
                    let (lo, hi) = match r.split_once('-') {
                        Some((a, b)) => (a.trim().parse::<u64>(), b.trim().parse::<u64>()),
                        None => (r.trim().parse::<u64>(), r.trim().parse::<u64>()),
                    };
                    let (lo, hi) = (lo.map_err(|_| format!("--rows が読めません: {r:?}"))?, hi.map_err(|_| format!("--rows が読めません: {r:?}"))?);
                    if let Some(cells) = node.get_mut("cells").and_then(|c| c.as_array_mut()) {
                        cells.retain(|c| c.get("row").and_then(|x| x.as_u64()).map_or(false, |x| lo <= x && x <= hi));
                    }
                }
                println!("{}", dumps(&node, 0, false));
            }
        }
        Command::Outline { out, edition, id, file } => {
            let ed = Edition::load(&out, &edition)?;
            match (&file, &id) {
                (Some(_), Some(_)) => return Err("ID と --file は同時に指定できません".to_string()),
                (None, None) => return Err("ID か --file のどちらか 1 つを指定してください".to_string()),
                _ => {}
            }
            let mut lines = Vec::new();
            if let Some(file) = file {
                validate_file_name(&file)?;
                let part = ed.parts.iter().find(|p| p.name == file).ok_or_else(|| format!("ファイル {file} は、index.json の files にありません"))?;
                for n in part.value.get("body").and_then(|b| b.as_array()).into_iter().flatten() {
                    outline_node(n, 0, &mut lines);
                }
            } else {
                let id = id.unwrap();
                let (pi, ptr) = ed.find_id(&id).ok_or_else(|| format!("ID {id} の要素が見つかりません"))?;
                let node = ed.parts[pi].value.pointer(&ptr).unwrap();
                for c in node.get("children").and_then(|c| c.as_array()).into_iter().flatten() {
                    outline_node(c, 0, &mut lines);
                }
            }
            for l in lines {
                println!("{l}");
            }
        }
        Command::New { out, from, editor } => println!("{}", edit::new_edition(&out, &from, &editor)?),
        Command::SetText { out, edition, id, text, text_file, note, cell, index } => {
            let text = match (text, text_file) {
                (Some(_), Some(_)) => return Err("--text と --text-file は同時に指定できません".to_string()),
                (None, None) => return Err("--text か --text-file のどちらか 1 つを指定してください".to_string()),
                (Some(t), None) => t,
                (None, Some(p)) => {
                    let bytes = if p == "-" {
                        let mut b = Vec::new();
                        std::io::Read::read_to_end(&mut std::io::stdin(), &mut b).map_err(|e| format!("標準入力を読めません: {e}"))?;
                        b
                    } else {
                        fs::read(&p).map_err(|e| format!("{p} を読めません: {e}"))?
                    };
                    if bytes.is_empty() {
                        return Err("--text-file の中身が空です（失敗したパイプの結果かもしれません）。文を空にする指定は --text \"\" を使ってください".to_string());
                    }
                String::from_utf8(bytes).map_err(|_| "--text-file の中身が UTF-8 ではありません".to_string())?
                }
            };
            println!("{}", edit::set_text(&out, &edition, &id, &text, note.as_deref(), cell.as_deref(), index)?)
        }
        Command::Insert { out, edition, after, first_in, text, note } => {
            println!("{}", edit::insert(&out, &edition, after.as_deref(), first_in.as_deref(), &text, note.as_deref())?)
        }
        Command::Split { out, edition, id, text, note } => {
            for i in edit::split(&out, &edition, &id, &text, note.as_deref())? {
                println!("{i}");
            }
        }
        Command::Delete { out, edition, id, note } => println!("{}", edit::delete(&out, &edition, &id, note.as_deref())?),
        Command::Check { out, edition } => {
            let problems = check::run(&out, &edition)?;
            if problems.is_empty() {
                println!("問題はありません");
            } else {
                for p in &problems {
                    println!("{p}");
                }
                return Ok(1);
            }
        }
        Command::Diff { out, a, b } => print!("{}", diff::run(&out, &a, &b)?),
    }
    Ok(0)
}

fn head(s: &str) -> String {
    s.chars().take(20).map(|c| if c == '\t' || c == '\n' || c == '\r' { ' ' } else { c }).collect()
}

fn first_sentence(n: &Value) -> String {
    let s = n.get("sentences").and_then(|s| s.get(0)).and_then(|s| str_of(s, "text")).unwrap_or("");
    head(s)
}

/// outline の 1 行（タブ区切り: 深さ, type, ID, 大きさ, 先頭の文字）を足す。センテンスと item は出さない。
fn outline_node(n: &Value, depth: usize, lines: &mut Vec<String>) {
    let ty = str_of(n, "type").unwrap_or("?");
    let id = str_of(n, "id").unwrap_or("?");
    let size = dumps(n, 0, false).chars().count();
    let (size_col, text) = match ty {
        "section" => (size.to_string(), head(str_of(n, "title").unwrap_or(""))),
        "section_ref" => ("-".to_string(), str_of(n, "file").unwrap_or("").to_string()),
        "paragraph" => (size.to_string(), first_sentence(n)),
        "list" => (size.to_string(), n.get("items").and_then(|i| i.get(0)).map(first_sentence).unwrap_or_default()),
        "caption" => (size.to_string(), head(str_of(n, "text").unwrap_or(""))),
        "figure" => (size.to_string(), head(n.get("text").and_then(|t| t.get(0)).and_then(|t| t.as_str()).unwrap_or(""))),
        "table" => {
            let cells = n.get("cells").and_then(|c| c.as_array()).cloned().unwrap_or_default();
            let mut rows: Vec<u64> = cells.iter().filter_map(|c| c.get("row").and_then(|r| r.as_u64())).collect();
            rows.sort();
            rows.dedup();
            (format!("{size} ({} 行, {} セル)", rows.len(), cells.len()), head(cells.first().and_then(|c| str_of(c, "text")).unwrap_or("")))
        }
        _ => return,
    };
    lines.push(format!("{depth}\t{ty}\t{id}\t{size_col}\t{text}"));
    if ty == "section" {
        for c in n.get("children").and_then(|c| c.as_array()).into_iter().flatten() {
            outline_node(c, depth + 1, lines);
        }
    }
}

fn sentences_text(n: &Value) -> String {
    n.get("sentences")
        .and_then(|s| s.as_array())
        .map(|a| a.iter().map(|s| str_of(s, "text").unwrap_or("")).collect::<String>())
        .unwrap_or_default()
}

/// `show` の --cell、--index、--text の出力。--text のときは、文字そのもの（改行も足さない）。
/// JSON のときは、末尾に改行を付ける。
fn show_one(node: &Value, id: &str, cell: Option<&str>, index: Option<usize>, text: bool) -> Res<String> {
    let ty = str_of(node, "type");
    let target: Value = if let Some(c) = cell {
        if ty != Some("table") {
            return Err(format!("--cell は表の ID にだけ使えます（{id}）"));
        }
        let (r, col) = edit::parse_cell(c)?;
        edit::find_cell(node, r, col).ok_or_else(|| format!("表 {id} に、行 {r}・列 {col} のセルがありません"))?.clone()
    } else if let Some(i) = index {
        if ty != Some("figure") {
            return Err(format!("--index は図の ID にだけ使えます（{id}）"));
        }
        let arr = node.get("text").and_then(|t| t.as_array()).cloned().unwrap_or_default();
        arr.get(i).cloned().ok_or_else(|| format!("--index {i} は範囲外です（図 {id} の文字は {} 個）", arr.len()))?
    } else {
        node.clone()
    };
    if !text {
        return Ok(format!("{}\n", dumps(&target, 0, false)));
    }
    if cell.is_some() {
        return Ok(str_of(&target, "text").unwrap_or("").to_string());
    }
    if index.is_some() {
        return Ok(target.as_str().unwrap_or("").to_string());
    }
    match ty {
        None if node.get("sentences").is_some() => Ok(sentences_text(node)), // 箇条書きの item
        None if node.get("text").map_or(false, |t| t.is_string()) => Ok(str_of(node, "text").unwrap_or("").to_string()), // センテンス
        Some("paragraph") => Ok(sentences_text(node)),
        Some("caption") => Ok(str_of(node, "text").unwrap_or("").to_string()),
        Some("table") => Err(format!("表 {id} の --text には --cell <row>,<col> が要ります")),
        Some("figure") => Err(format!("図 {id} の --text には --index <n> が要ります")),
        _ => Err(format!("{id} には --text を使えません（センテンス・キャプション・パラグラフ・項目・表のセル・図の文字が対象です）")),
    }
}
