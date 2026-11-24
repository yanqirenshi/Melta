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
        text: String,
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
        Command::Show { out, edition, id, rows, file } => {
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
        Command::New { out, from, editor } => println!("{}", edit::new_edition(&out, &from, &editor)?),
        Command::SetText { out, edition, id, text, note, cell, index } => {
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
