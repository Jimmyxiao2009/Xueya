import fs from "node:fs";
const p = "src/knowledge/vector_index.rs";
let s = fs.readFileSync(p, "utf8");
s = s.replace(`    cache: Arc<RwLock<HashMap<(Uuid, Uuid, String), EmbeddingRecord>>>,`, `    cache: EmbeddingCache,`);
s = s.replace(`pub struct SqliteVectorIndex {`, `type EmbeddingCacheKey = (Uuid, Uuid, String);
type EmbeddingCache = Arc<RwLock<HashMap<EmbeddingCacheKey, EmbeddingRecord>>>;

pub struct SqliteVectorIndex {`);
fs.writeFileSync(p, s);
console.log("ok");
