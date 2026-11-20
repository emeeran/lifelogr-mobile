import type { Folder } from "./api";

/** Folders in tree order with depth and a "Parent / Child" label. */
export function flattenFolders(folders: Folder[]): (Folder & { depth: number; label: string })[] {
  const kids = new Map<string | null, Folder[]>();
  for (const f of folders) {
    const k = f.parent_id && folders.some((p) => p.id === f.parent_id) ? f.parent_id : null;
    kids.set(k, [...(kids.get(k) ?? []), f]);
  }
  const out: (Folder & { depth: number; label: string })[] = [];
  const walk = (parent: string | null, depth: number, prefix: string) => {
    for (const f of kids.get(parent) ?? []) {
      const label = prefix ? `${prefix} / ${f.name}` : f.name;
      out.push({ ...f, depth, label });
      if (depth < 32) walk(f.id, depth + 1, label);
    }
  };
  walk(null, 0, "");
  return out;
}

/** ids of a folder and all its descendants (for the client-side acyclic guard). */
export function descendants(folders: Folder[], id: string): Set<string> {
  const s = new Set([id]);
  let grew = true;
  while (grew) {
    grew = false;
    for (const f of folders) if (f.parent_id && s.has(f.parent_id) && !s.has(f.id)) (s.add(f.id), (grew = true));
  }
  return s;
}
