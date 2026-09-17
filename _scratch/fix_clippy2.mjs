import fs from "node:fs";
const edit = (p, pairs) => {
  let s = fs.readFileSync(p, "utf8");
  for (const [from, to] of pairs) {
    if (!s.includes(from)) throw new Error(`anchor missing in ${p}: ${from.slice(0, 60)}`);
    s = s.replace(from, to);
  }
  fs.writeFileSync(p, s);
};

edit("src/curriculum/mod.rs", [[
  `        .map(|value| strip_trailing_punctuation(value))`,
  `        .map(strip_trailing_punctuation)`,
]]);

edit("src/db/repo/chunks.rs", [[
  `    let placeholders = std::iter::repeat("?")
        .take(ids.len())`,
  `    let placeholders = std::iter::repeat_n("?", ids.len())`,
]]);

edit("src/db/repo/jobs.rs", [[
  `pub async fn update_job(`,
  `#[allow(clippy::too_many_arguments)]
pub async fn update_job(`,
]]);

edit("src/ingest/chunker.rs", [[
  `    fn flush_segment(`,
  `    #[allow(clippy::too_many_arguments)]
    fn flush_segment(`,
]]);

edit("src/knowledge_fs/virtual_path.rs", [[
  `assert_eq!(VirtualPath::parse("../../etc").is_err(), true);`,
  `assert!(VirtualPath::parse("../../etc").is_err());`,
]]);
console.log("ok");
