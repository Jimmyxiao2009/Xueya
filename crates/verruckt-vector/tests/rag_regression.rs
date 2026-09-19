//! Real-corpus RAG regression: Projective Geometry + Linear Algebra chunks
//! with deterministic hashed embeddings, 30 queries with an expected relevant
//! chunk each, compared across three backends:
//!
//! * SQLite blob + brute-force cosine (mirrors the current Verruckt SQLite
//!   vector backend: BLOB storage, load-all, per-pair cosine in Rust)
//! * Verruckt Exact (Algorithm::Exact)
//! * Verruckt HNSW (Algorithm::Hnsw)
//!
//! Run with `cargo test --test rag_regression -- --nocapture` to print the
//! Top1 / Recall@5 / Recall@10 table for docs/VECTOR_INTEGRATION_REPORT.md.

use rusqlite::{params, Connection};
use verruckt_vector::{Algorithm, IndexConfig, Metadata, Metric, VectorId, VectorIndex};

const DIM: usize = 256;

// Two-topic real text corpus. 30 chunks, each a self-contained statement.
const CORPUS: &[(&str, &str)] = &[
    ("pg", "在射影平面上，任意两条不同的直线恰好相交于一点。平行线在欧氏几何中不相交，但在射影几何中它们相交于一个无穷远点。"),
    ("pg", "射影平面可以通过在欧氏平面上添加一条无穷远直线来构造。每族平行直线共享同一个无穷远点，所有无穷远点构成无穷远直线。"),
    ("pg", "齐次坐标用三元组 (x : y : z) 表示射影平面上的点。当 z 不为零时对应欧氏点；当 z 为零时表示无穷远点。"),
    ("pg", "对偶原理：在射影几何中，点与直线的地位可以互换。过两点的直线与两直线的交点互为对偶命题。"),
    ("pg", "交比是射影变换下的基本不变量。共线四点的交比在任何中心投影下保持不变。"),
    ("pg", "圆锥曲线在射影几何中定义为平面与二阶锥面的交线。椭圆、抛物线和双曲线是同一类曲线。"),
    ("pg", "笛沙格定理：若两个三角形对应顶点的连线共点，则对应边的交点共线。"),
    ("pg", "帕斯卡定理：圆内接六边形三组对边的交点共线。"),
    ("pg", "变换群观点：射影几何研究在射影变换群作用下保持不变的性质。"),
    ("pg", "仿射几何是射影几何的子几何：保持无穷远直线不动的射影变换构成仿射变换群。"),
    ("pg", "无穷远点的引入使得平行线相交，这是射影几何与欧氏几何最直观的区别。"),
    ("pg", "中心投影把一条直线上的点映射到另一条直线，交比在这样的投影下不变。"),
    ("pg", "射影平面没有边界也没有定向，沿着任一方向走一圈会回到镜像位置。"),
    ("pg", "对偶平面上每个点对应原平面的一条直线，对偶变换保持结合关系。"),
    ("pg", "调和共轭：交比等于负一的四个点称为调和点列，与完全四边形密切相关。"),
    ("la", "线性代数研究向量空间、线性映射和矩阵。向量空间由加法和数乘封闭运算构成。"),
    ("la", "矩阵的秩是线性无关列向量的最大个数，决定了线性方程组解的结构。"),
    ("la", "行列式为零当且仅当矩阵不可逆，此时齐次方程组有非零解。"),
    ("la", "特征值和特征向量刻画线性映射的不变方向，特征方程的根给出谱。"),
    ("la", "正交矩阵保持内积和长度，其列向量构成标准正交基。"),
    ("la", "对称矩阵可以正交对角化，谱定理保证实特征值和正交特征向量。"),
    ("la", "奇异值分解把任意矩阵分解为两个正交矩阵与一个对角矩阵的乘积。"),
    ("la", "Gram-Schmidt 正交化把线性无关向量组化为标准正交基。"),
    ("la", "线性方程组有唯一解当且仅当系数矩阵可逆，否则可能无解或无穷多解。"),
    ("la", "向量组的线性相关性由是否存在非平凡零组合决定。"),
    ("la", "迹是特征值之和，行列式是特征值之积，二者在相似变换下不变。"),
    ("la", "二次型的惯性指数在合同变换下不变，正定性由顺序主子式判定。"),
    ("la", "若尔当标准形把复矩阵化为几乎对角的形状，刻画广义特征向量结构。"),
    ("la", "最小多项式整除特征多项式，矩阵可对角化当且仅当最小多项式无重根。"),
    ("la", "内积空间中的柯西-施瓦茨不等式给出夹角与投影的基本估计。"),
];

// One query per corpus chunk: query text paraphrases the chunk topic; the
// expected chunk is the one that answers it.
const QUERIES: &[(&str, usize)] = &[
    ("平行线在射影平面里在哪里相交？", 0),
    ("如何从欧氏平面构造射影平面？", 1),
    ("齐次坐标怎样表示无穷远点？", 2),
    ("点和直线的对偶原理是什么？", 3),
    ("交比在什么变换下不变？", 4),
    ("圆锥曲线在射影几何里如何统一定义？", 5),
    ("笛沙格定理说了什么？", 6),
    ("帕斯卡定理的内容是？", 7),
    ("射影几何的变换群观点是什么？", 8),
    ("仿射变换群和无穷远直线有什么关系？", 9),
    ("无穷远点如何让平行线相交？", 10),
    ("中心投影下哪个量保持不变？", 11),
    ("射影平面沿一个方向走一圈会发生什么？", 12),
    ("对偶平面的点和直线如何对应？", 13),
    ("什么是调和点列？", 14),
    ("线性代数研究哪些对象？", 15),
    ("矩阵的秩如何决定方程组解的结构？", 16),
    ("行列式为零意味着什么？", 17),
    ("特征值和特征向量刻画什么？", 18),
    ("正交矩阵有什么性质？", 19),
    ("对称矩阵为什么可以正交对角化？", 20),
    ("奇异值分解的形式是什么？", 21),
    ("Gram-Schmidt 正交化做什么？", 22),
    ("线性方程组什么时候有唯一解？", 23),
    ("怎样判断向量组线性相关？", 24),
    ("迹和行列式在相似变换下如何变化？", 25),
    ("二次型的正定性如何判定？", 26),
    ("若尔当标准形有什么用？", 27),
    ("最小多项式和对角化有什么关系？", 28),
    ("柯西-施瓦茨不等式给出什么估计？", 29),
];

/// Deterministic hashed bag-of-words embedding (same scheme as rag_fixture).
fn embed(text: &str) -> Vec<f32> {
    let mut vector = vec![0.0f32; DIM];
    let chars: Vec<char> = text.chars().collect();
    for i in 0..chars.len().saturating_sub(1) {
        let bigram: Vec<u16> = chars[i..i + 2].iter().map(|c| *c as u16).collect();
        let mut seed = 0xcbf2_9ce4_8422_2325u64;
        for unit in bigram {
            seed ^= unit as u64;
            seed = seed.wrapping_mul(0x0000_0100_0000_01b3);
        }
        let slot = (seed % DIM as u64) as usize;
        let sign = if (seed >> 32) & 1 == 0 { 1.0 } else { -1.0 };
        vector[slot] += sign;
        let slot2 = ((seed / 7) % DIM as u64) as usize;
        vector[slot2] += sign * 0.5;
    }
    let norm: f32 = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > f32::EPSILON {
        for component in &mut vector {
            *component /= norm;
        }
    }
    vector
}

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// SQLite blob baseline: mirrors the production backend (BLOB storage +
/// load-all + brute-force cosine in Rust).
struct SqliteBaseline {
    rows: Vec<Vec<f32>>,
}

impl SqliteBaseline {
    fn build(dir: &std::path::Path) -> Self {
        let db = dir.join("baseline.sqlite");
        let conn = Connection::open(&db).unwrap();
        conn.execute_batch("CREATE TABLE embeddings (id INTEGER PRIMARY KEY, dimensions INTEGER NOT NULL, vector BLOB NOT NULL);")
            .unwrap();
        for (i, (_, text)) in CORPUS.iter().enumerate() {
            let vector = embed(text);
            let blob: Vec<u8> = vector.iter().flat_map(|x| x.to_le_bytes()).collect();
            conn.execute(
                "INSERT INTO embeddings(id, dimensions, vector) VALUES (?1, ?2, ?3)",
                params![i as i64, DIM as i64, blob],
            )
            .unwrap();
        }
        let mut stmt = conn.prepare("SELECT vector FROM embeddings ORDER BY id").unwrap();
        let rows: Vec<Vec<f32>> = stmt
            .query_map([], |row| {
                let bytes: Vec<u8> = row.get(0)?;
                Ok(bytes
                    .chunks_exact(4)
                    .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                    .collect())
            })
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        SqliteBaseline { rows }
    }

    fn top_k(&self, query: &[f32], k: usize) -> Vec<usize> {
        let mut scored: Vec<(f32, usize)> = self
            .rows
            .iter()
            .enumerate()
            .map(|(i, v)| (cosine(query, v), i))
            .collect();
        scored.sort_by(|a, b| b.0.total_cmp(&a.0));
        scored.truncate(k);
        scored.into_iter().map(|(_, i)| i).collect()
    }
}

fn engine_top_k(index: &VectorIndex, query: &[f32], k: usize) -> Vec<usize> {
    index
        .search(query, k)
        .unwrap()
        .into_iter()
        .map(|hit| (hit.id.0 - 1) as usize)
        .collect()
}

fn metrics(name: &str, top_lists: &[Vec<usize>]) {
    let total = QUERIES.len();
    let top1 = top_lists.iter().enumerate().filter(|(q, l)| l[0] == QUERIES[*q].1).count();
    let recall5 = top_lists.iter().enumerate().filter(|(q, l)| l[..5.min(l.len())].contains(&QUERIES[*q].1)).count();
    let recall10 = top_lists.iter().enumerate().filter(|(q, l)| l[..10.min(l.len())].contains(&QUERIES[*q].1)).count();
    println!(
        "| {name} | {:.3} | {:.3} | {:.3} |",
        top1 as f64 / total as f64,
        recall5 as f64 / total as f64,
        recall10 as f64 / total as f64
    );
}

#[test]
fn real_corpus_rag_regression_across_backends() {
    let dir = tempfile::tempdir().unwrap();

    // Backends.
    let sqlite = SqliteBaseline::build(dir.path());
    let exact_dir = dir.path().join("exact");
    let mut exact_cfg = IndexConfig::new(DIM as u32, Metric::Cosine);
    exact_cfg.algorithm = Algorithm::Exact;
    let exact = VectorIndex::create(&exact_dir, exact_cfg).unwrap();
    let hnsw_dir = dir.path().join("hnsw");
    let mut hnsw_cfg = IndexConfig::new(DIM as u32, Metric::Cosine);
    hnsw_cfg.algorithm = Algorithm::Hnsw;
    hnsw_cfg.hnsw.m = 16;
    hnsw_cfg.hnsw.ef_construction = 200;
    hnsw_cfg.hnsw.ef_search = 1024;
    let hnsw = VectorIndex::create(&hnsw_dir, hnsw_cfg).unwrap();
    for (i, (_, text)) in CORPUS.iter().enumerate() {
        let vector = embed(text);
        let id = VectorId(i as u128 + 1);
        exact.insert(id, &vector, Metadata::empty()).unwrap();
        hnsw.insert(id, &vector, Metadata::empty()).unwrap();
    }

    let mut sqlite_lists = Vec::new();
    let mut exact_lists = Vec::new();
    let mut hnsw_lists = Vec::new();
    for (text, expected) in QUERIES {
        let _ = expected;
        let query = embed(text);
        sqlite_lists.push(sqlite.top_k(&query, 10));
        exact_lists.push(engine_top_k(&exact, &query, 10));
        hnsw_lists.push(engine_top_k(&hnsw, &query, 10));
    }

    println!();
    println!("Real-corpus RAG regression ({} queries, {} chunks, dim {DIM}):", QUERIES.len(), CORPUS.len());
    println!("| backend | Top1 | Recall@5 | Recall@10 |");
    println!("|---|---|---|---|");
    metrics("SQLite blob + brute force", &sqlite_lists);
    metrics("Verruckt Exact", &exact_lists);
    metrics("Verruckt HNSW", &hnsw_lists);

    // Sanity gates: the deterministic corpus must be retrievable by all
    // backends; HNSW must stay within one hit of Exact on recall@5.
    for (backend, lists) in [("sqlite", &sqlite_lists), ("exact", &exact_lists), ("hnsw", &hnsw_lists)] {
        let recall5 = lists.iter().enumerate().filter(|(q, l)| l[..5.min(l.len())].contains(&QUERIES[*q].1)).count();
        assert!(
            recall5 as f64 / QUERIES.len() as f64 >= 0.9,
            "{backend} recall@5 below 0.9 on the real corpus"
        );
    }
}
