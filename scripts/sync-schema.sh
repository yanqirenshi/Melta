#!/usr/bin/env bash
# Hefti (npm: @yanqirenshi/hefti) のスキーマを schema/ に取り込む。
#
# 使い方:
#     bash scripts/sync-schema.sh [版]
#
# あわせて、見本（package/examples/ の valid/ と invalid/）を tests/fixtures/hefti/examples/ に、まるごと入れ替えて写す（中身は変換しない）。
# 版を省くと schema/HEFTI_VERSION の版を使う。curl と tar を使う。
# tarball は一時フォルダに展開し、必要な 2 ファイルだけを名指しして取り出す。
# 2 ファイルがそろってから schema/ に写すので、失敗しても schema/ は書き換わらない。
set -euo pipefail

PACKAGE="@yanqirenshi/hefti"
NAME="hefti"
FILES=("structure-index.schema.json" "structure-part.schema.json")
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCHEMA_DIR="$ROOT/schema"
VERSION_FILE="$SCHEMA_DIR/HEFTI_VERSION"

if [ "$#" -ge 1 ]; then
  version="$(printf '%s' "$1" | tr -d '[:space:]')"
elif [ -f "$VERSION_FILE" ]; then
  version="$(tr -d '[:space:]' < "$VERSION_FILE")"
else
  echo "版を引数で指定してください (schema/HEFTI_VERSION がありません)" >&2
  exit 2
fi
if ! [[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "版の形が正しくありません: $version" >&2
  exit 2
fi

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

url="https://registry.npmjs.org/${PACKAGE}/-/${NAME}-${version}.tgz"
curl -fsSL --max-time 60 -o "$tmp/hefti.tgz" "$url"

mkdir "$tmp/x"
members=()
for f in "${FILES[@]}"; do members+=("package/schema/$f"); done
members+=("package/examples")
EXAMPLES_DIR="$ROOT/tests/fixtures/hefti/examples"
tar -xzf "$tmp/hefti.tgz" -C "$tmp/x" "${members[@]}"

# 通常のファイルであることと、2 件がそろっていることを確かめてから、改行を LF にそろえて写す
for f in "${FILES[@]}"; do
  p="$tmp/x/package/schema/$f"
  if [ ! -f "$p" ] || [ -L "$p" ]; then
    echo "通常のファイルではありません: package/schema/$f" >&2
    exit 1
  fi
  tr -d '\r' < "$p" > "$tmp/$f"
done

# 見本は、通常のファイルだけで、1 件以上あることを確かめる
src_examples="$tmp/x/package/examples"
if [ ! -d "$src_examples" ] || [ -L "$src_examples" ]; then
  echo "見本のフォルダがありません: package/examples" >&2
  exit 1
fi
if [ -n "$(find "$src_examples" ! -type f ! -type d -print -quit)" ]; then
  echo "見本に、通常のファイルでもフォルダでもないものがあります" >&2
  exit 1
fi
if [ -z "$(find "$src_examples" -type f -print -quit)" ]; then
  echo "見本が 1 件もありません" >&2
  exit 1
fi

mkdir -p "$SCHEMA_DIR"
for f in "${FILES[@]}"; do cp "$tmp/$f" "$SCHEMA_DIR/$f"; done
# 見本を入れ替える（消すのは tests/fixtures/hefti/examples/ だけ）
rm -rf "$EXAMPLES_DIR"
mkdir -p "$EXAMPLES_DIR"
cp -R "$src_examples/." "$EXAMPLES_DIR/"
printf '%s\n' "$version" > "$VERSION_FILE"
echo "${PACKAGE}@${version} を schema/ に取り込みました"
