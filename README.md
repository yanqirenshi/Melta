# Melta

文章を、一文ずつ読むためのアプリです。
文章を「。」ごとに一行に区切り、今読んでいる行だけを黄色く光らせます。
ほかの行は薄く表示されるので、一文に集中して読めます。

## 起動する

```bash
npm install
npm run dev
```

ブラウザで http://localhost:5173 を開きます。

## 操作

| 操作 | 動き |
|---|---|
| → (右矢印キー) | 次の行に進む |
| ← (左矢印キー) | 前の行に戻る |
| 行をクリック | その行に移動する |

今読んでいる行は、いつも画面の真ん中に来るようにスクロールします。
画面の上には、文書のタイトルと、何行目を読んでいるか (例: 3 / 12) と、進み具合のバーが出ます。

## 文章の書き方

文章は `src/documents/` フォルダに、TSX のファイルとして置きます。
`Doc` で全体を囲み、見出しは `H`、段落は `P` で囲みます。

```tsx
import { Doc, H, P } from '../reader/Reader';

export default function MyDocument() {
  return (
    <Doc title="文書のタイトル">
      <H>見出し</H>
      <P>
        一つ目の文です。
        二つ目の文です。
      </P>
    </Doc>
  );
}
```

- `H` (見出し) は、中身全体を一行として扱います。
- `P` (段落) は、「。」ごとに区切って、複数の行にします。
- 行の区切りは「。」だけで決まります。
  ソースの中で改行しても、行は分かれません。
- かっこの中の「。」では区切りません。
  対象のかっこは `「」` `『』` `（）` `()` `【】` `［］` `[]` です。
  たとえば、`彼は「わかった。ありがとう。」と言った。` は一行になります。

### 表示する文書を変える

表示する文書は `src/main.tsx` で選びます。
今は `Sample` (サンプル文書) を表示しています。
新しい文書を作ったら、`import` と、描くコンポーネントを書き換えます。

```tsx
import MyDocument from './documents/MyDocument';

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <MyDocument />
  </StrictMode>,
);
```

## コマンド

| コマンド | 内容 |
|---|---|
| `npm run dev` | 開発サーバーを起動する (http://localhost:5173) |
| `npm run build` | 型を検査して、`dist/` にビルドする |
| `npm run preview` | ビルドしたものを、ブラウザで確かめる |

## フォルダの構成

```
.
├── index.html
├── src/
│   ├── main.tsx                入り口 (表示する文書を選ぶ)
│   ├── documents/
│   │   └── Sample.tsx          サンプル文書
│   └── reader/
│       ├── Reader.tsx          Doc・H・P のタグと、読む画面
│       ├── reader.css          見た目
│       └── splitSentences.ts   「。」で文章を区切る処理
└── hub/                        iHub のアプリ (下の「hub/」を見る)
```

## 使っているもの

- React 18
- TypeScript
- Vite 5

## hub/

`hub/` は、[iHub](https://github.com/yanqirenshi/iHub) のアプリです。
設計データ (ER 図・クラス図・画面遷移図・WBS など) を、図や表で見て、編集する画面です。
Melta とは別の npm プロジェクトなので、`hub/` の中でコマンドを実行します。

### 起動する

Node.js 22 以上が要ります。

```bash
cd hub
npm install
npx ihub-srv init data
npm start
```

`npx ihub-srv init data` は、最初の一回だけ実行します。
雛形から、データのフォルダ (`hub/data/`) を作ります。
`npm start` は、画面 (http://localhost:3000) と API サーバー (ポート 4000) を一緒に起動します。
データは `hub/data/` に JSON で置かれ、API サーバーが読み書きします。

GitHub と連携するときは、`.env.local.template` を `.env.local` にコピーして、トークンなどを書きます。
`.env.local` は git に入りません。

### コマンド

| コマンド | 内容 |
|---|---|
| `npm start` | 画面と API サーバーを一緒に起動する |
| `npm run build` | 画面を `build/` にビルドする |
| `npm run serve` | ビルドした画面と API を、一緒に出す |
| `npm run test:data` | `data/` を、スキーマとルールで検証する |
