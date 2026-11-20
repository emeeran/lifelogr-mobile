import { marked } from "marked";
import DOMPurify from "dompurify";

marked.setOptions({ gfm: true, breaks: true });

/** Markdown → sanitized HTML. #hashtags become clickable chips. */
export function renderMarkdown(src: string): string {
  const html = marked.parse(src, { async: false }) as string;
  const withTags = html.replace(
    /(^|[\s>(])#([\p{L}\p{N}_\-/]*\p{L}[\p{L}\p{N}_\-/]*)/gu,
    (_m, pre, tag) => `${pre}<a class="hashtag" data-tag="${tag}">#${tag}</a>`,
  );
  return DOMPurify.sanitize(withTags, { ADD_ATTR: ["data-tag"] });
}

/** Split an FTS snippet (matches wrapped in \u0001…\u0002) into plain/highlight parts. */
export function snippetParts(s: string): { text: string; hit: boolean }[] {
  const out: { text: string; hit: boolean }[] = [];
  const re = /\u0001([^\u0002]*)\u0002/g;
  let last = 0;
  let m: RegExpExecArray | null;
  while ((m = re.exec(s))) {
    if (m.index > last) out.push({ text: s.slice(last, m.index), hit: false });
    out.push({ text: m[1], hit: true });
    last = m.index + m[0].length;
  }
  if (last < s.length) out.push({ text: s.slice(last), hit: false });
  return out;
}
