import { useState } from 'react';

type Value = string | number | null;

/** Click-to-sort table columns: `th(key, label)` renders the header, `sort(rows, get)` orders rows. */
export function useSort<K extends string>(initialKey: K, initialDir: 'asc' | 'desc' = 'desc') {
  const [key, setKey] = useState<K>(initialKey);
  const [dir, setDir] = useState(initialDir);

  const toggle = (k: K) => {
    if (k === key) setDir(d => (d === 'asc' ? 'desc' : 'asc'));
    else { setKey(k); setDir('asc'); }
  };

  // Empty values always sink to the bottom, whichever direction.
  const sort = <T,>(rows: T[], get: (row: T, k: K) => Value) =>
    [...rows].sort((a, b) => {
      const x = get(a, key), y = get(b, key);
      if (x === y) return 0;
      if (x === null) return 1;
      if (y === null) return -1;
      const c = typeof x === 'number' && typeof y === 'number' ? x - y : String(x).localeCompare(String(y));
      return dir === 'asc' ? c : -c;
    });

  const th = (k: K, label: string, className = '') => (
    <th
      className={`${className} sortable-header`}
      aria-sort={k === key ? (dir === 'asc' ? 'ascending' : 'descending') : undefined}
      onClick={() => toggle(k)}
    >
      {label}
      {k === key && <span className="sort-arrow">{dir === 'asc' ? ' ▲' : ' ▼'}</span>}
    </th>
  );

  return { sort, th };
}
