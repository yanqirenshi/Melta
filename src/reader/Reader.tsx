import {
  Children,
  isValidElement,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from 'react';
import { splitSentences } from './splitSentences';
import './reader.css';

/* ------------------------------------------------------------------
 * 文章を書くためのタグ
 *
 *   <Doc title="タイトル">
 *     <H>見出し</H>
 *     <P>段落の文章。「。」ごとに一行として扱われる。</P>
 *   </Doc>
 *
 * H と P は目印のためのタグで、実際の表示は Doc が行う。
 * ---------------------------------------------------------------- */

type TextProps = { children?: ReactNode };

/** 見出し。中身全体を一行として扱う。 */
export function H(_: TextProps) {
  return null;
}

/** 段落。「。」で区切って複数行にする。 */
export function P(_: TextProps) {
  return null;
}

type Block = { kind: 'h' | 'p'; lines: { index: number; text: string }[] };

// children から文字だけを取り出す。
function textOf(node: ReactNode): string {
  if (node == null || typeof node === 'boolean') return '';
  if (typeof node === 'string' || typeof node === 'number') return String(node);
  if (Array.isArray(node)) return node.map(textOf).join('');
  if (isValidElement<TextProps>(node)) return textOf(node.props.children);
  return '';
}

// <H> と <P> を読み取り、通し番号つきの行に分ける。
function parseBlocks(children: ReactNode): { blocks: Block[]; count: number } {
  const blocks: Block[] = [];
  let index = 0;

  Children.forEach(children, (child) => {
    if (!isValidElement<TextProps>(child)) return;
    const text = textOf(child.props.children);

    if (child.type === H) {
      const line = text.trim();
      if (line) blocks.push({ kind: 'h', lines: [{ index: index++, text: line }] });
    } else if (child.type === P) {
      const lines = splitSentences(text).map((t) => ({ index: index++, text: t }));
      if (lines.length) blocks.push({ kind: 'p', lines });
    }
  });

  return { blocks, count: index };
}

type DocProps = { title?: string; children?: ReactNode };

export function Doc({ title, children }: DocProps) {
  const { blocks, count } = useMemo(() => parseBlocks(children), [children]);
  const [current, setCurrent] = useState(0);
  const lineRefs = useRef<(HTMLSpanElement | null)[]>([]);

  // → で次の行、← で前の行。
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'ArrowRight') {
        e.preventDefault();
        setCurrent((i) => Math.min(i + 1, count - 1));
      } else if (e.key === 'ArrowLeft') {
        e.preventDefault();
        setCurrent((i) => Math.max(i - 1, 0));
      }
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [count]);

  // ハイライト中の行が画面の真ん中に来るようにスクロールする。
  useEffect(() => {
    lineRefs.current[current]?.scrollIntoView({ block: 'center', behavior: 'smooth' });
  }, [current]);

  const renderLine = ({ index, text }: { index: number; text: string }) => (
    <span
      key={index}
      ref={(el) => (lineRefs.current[index] = el)}
      className={'line' + (index === current ? ' is-current' : '')}
      onClick={() => setCurrent(index)}
    >
      {text}
    </span>
  );

  return (
    <div className="reader">
      <header className="reader-bar">
        <span className="reader-title">{title}</span>
        <span className="reader-hint">← 前の行　→ 次の行</span>
        <span className="reader-count">
          {count ? current + 1 : 0} / {count}
        </span>
        <div className="reader-progress" style={{ width: `${count ? ((current + 1) / count) * 100 : 0}%` }} />
      </header>

      <article className="reader-body">
        {blocks.map((block, i) =>
          block.kind === 'h' ? (
            <h2 key={i}>{block.lines.map(renderLine)}</h2>
          ) : (
            <p key={i}>{block.lines.map(renderLine)}</p>
          ),
        )}
      </article>
    </div>
  );
}
