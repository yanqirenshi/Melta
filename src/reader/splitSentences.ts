// 「。」で文章を区切って行の配列にする。
// ただし「」や（）の中にある「。」では区切らない。
// 例: 彼は「わかった。」と言った。 → 1 行
const OPEN = '「『（(【［[';
const CLOSE = '」』）)】］]';

export function splitSentences(text: string): string[] {
  const lines: string[] = [];
  let current = '';
  let depth = 0;

  for (const ch of text) {
    current += ch;
    if (OPEN.includes(ch)) depth++;
    else if (CLOSE.includes(ch)) depth = Math.max(0, depth - 1);
    else if (ch === '。' && depth === 0) {
      lines.push(current);
      current = '';
    }
  }
  lines.push(current);

  return lines.map((line) => line.trim()).filter((line) => line.length > 0);
}
