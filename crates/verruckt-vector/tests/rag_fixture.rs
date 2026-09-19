//! RAG fixture test with a real Verruckt-style corpus: Projective Geometry
//! chunks, deterministic "embedding" of the chunk text, and a query whose
//! top-1 must be the chunk that answers it.
//!
//! The deterministic embedding is a toy hashed bag-of-words: each term maps
//! to a seeded pseudo-random direction, and a document is the normalized sum
//! of its terms' directions. This mirrors real embeddings closely enough
//! (semantic overlap → cosine proximity) to be a meaningful retrieval check
//! while staying 100% reproducible.

use verruckt_vector::{
    Algorithm, IndexConfig, Metadata, MetadataFilter, Metric, VectorId, VectorIndex,
};

const DIM: usize = 256;

const CHUNKS: &[(&str, &str)] = &[
    (
        "c1",
        "在射影平面上，任意两条不同的直线恰好相交于一点。平行线在欧氏几何中不相交，但在射影几何中它们相交于一个无穷远点。",
    ),
    (
        "c2",
        "射影平面可以通过在欧氏平面上添加一条无穷远直线来构造。每族平行直线共享同一个无穷远点，所有无穷远点构成无穷远直线。",
    ),
    (
        "c3",
        "齐次坐标用三元组 (x : y : z) 表示射影平面上的点。当 z ≠ 0 时对应欧氏点 (x/z, y/z)；当 z = 0 时表示无穷远点。",
    ),
    (
        "c4",
        "对偶原理：在射影几何中，点与直线的地位可以互换。过两点的直线与两直线的交点互为对偶命题，定理与其对偶命题同时成立。",
    ),
    (
        "c5",
        "交比是射影变换下的基本不变量。给定共线四点 A、B、C、D，其交比 (A,B;C,D) 在任何中心投影下保持不变。",
    ),
    (
        "c6",
        "圆锥曲线在射影几何中被定义为平面与二阶锥面的交线。椭圆、抛物线和双曲线在射影观点下是同一类曲线，区别仅在于与无穷远直线的位置关系。",
    ),
    (
        "c7",
        "笛沙格定理：若两个三角形对应顶点的连线共点，则对应边的交点共线。该定理及其逆定理在三维空间中可以直接证明。",
    ),
    (
        "c8",
        "帕斯卡定理：圆内接六边形三组对边的交点共线。其对偶定理布里昂雄定理指出外切六边形的三条对角线共点。",
    ),
    (
        "c9",
        "变换群观点：射影几何研究在射影变换群作用下保持不变的性质。克莱因的埃尔朗根纲领将不同几何学按其变换群分类。",
    ),
    (
        "c10",
        "仿射几何是射影几何的子几何：保持无穷远直线不动的射影变换构成仿射变换群，度量几何进一步限制为保持绝对形不变的变换。",
    ),
];

/// Deterministic hashed bag-of-words embedding (Chinese-aware: bigrams).
fn embed(text: &str) -> Vec<f32> {
    let mut vector = vec![0.0f32; DIM];
    let chars: Vec<char> = text.chars().collect();
    // Bigrams capture more term structure than single chars.
    for i in 0..chars.len().saturating_sub(1) {
        let bigram: Vec<u16> = chars[i..i + 2].iter().map(|c| *c as u16).collect();
        let mut seed = 0xcbf2_9ce4_8422_2325u64;
        for unit in bigram {
            seed ^= unit as u64;
            seed = seed.wrapping_mul(0x0000_0100_0000_01b3);
        }
        // Fold into a small number of active dims (sparse, like real embeddings).
        let slot = (seed % DIM as u64) as usize;
        let sign = if (seed >> 32) & 1 == 0 { 1.0 } else { -1.0 };
        vector[slot] += sign;
        // Second hash for slightly denser coverage.
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

fn build_fixture(dir: &std::path::Path, algorithm: Algorithm) -> VectorIndex {
    let mut config = IndexConfig::new(DIM as u32, Metric::Cosine);
    config.algorithm = algorithm;
    let index = VectorIndex::create(dir, config).unwrap();

    let route = VectorId(0xA000);
    let source = VectorId(0xB001);
    for (i, (_tag, text)) in CHUNKS.iter().enumerate() {
        let metadata = Metadata {
            route_id: Some(route),
            source_id: Some(source),
            knowledge_point_id: Some(VectorId(0x1000 + i as u128)),
            kind: Some("chunk".into()),
        };
        index
            .insert(VectorId(0x10 + i as u128), &embed(text), metadata)
            .unwrap();
    }
    index
}

fn top1(index: &VectorIndex, query: &str) -> VectorId {
    let hits = index.search(&embed(query), 1).unwrap();
    hits[0].id
}

fn chunk_index_of(id: VectorId) -> usize {
    (id.0 - 0x10) as usize
}

#[test]
fn rag_fixture_query_top1_exact() {
    let dir = tempfile::tempdir().unwrap();
    let index = build_fixture(dir.path(), Algorithm::Exact);
    let hit = top1(&index, "平行线在射影平面在哪里相交？");
    assert_eq!(
        chunk_index_of(hit),
        0,
        "top1 must be c1 (平行线相交于无穷远点), got {hit}"
    );
}

#[test]
fn rag_fixture_query_top1_hnsw() {
    let dir = tempfile::tempdir().unwrap();
    let index = build_fixture(dir.path(), Algorithm::Hnsw);
    let hit = top1(&index, "平行线在射影平面在哪里相交？");
    assert_eq!(
        chunk_index_of(hit),
        0,
        "top1 must be c1 (平行线相交于无穷远点), got {hit}"
    );
}

#[test]
fn rag_fixture_more_queries() {
    let dir = tempfile::tempdir().unwrap();
    let index = build_fixture(dir.path(), Algorithm::Adaptive);

    let cases: &[(&str, usize)] = &[
        ("齐次坐标中 z = 0 表示什么无穷远点？", 2), // c3: z = 0
        ("交比是射影变换下的基本不变量", 4),        // c5
        ("对偶原理：点和直线如何互换？", 3),        // c4
        ("帕斯卡定理说了什么？", 7),                // c8
        ("圆锥曲线在射影几何里如何定义？", 5),      // c6
    ];
    for (query, expected) in cases {
        let hit = top1(&index, query);
        assert_eq!(
            chunk_index_of(hit),
            *expected,
            "query {query:?} must hit chunk c{}, got {}",
            expected + 1,
            chunk_index_of(hit) + 1
        );
    }
}

#[test]
fn rag_fixture_filtered_by_knowledge_point() {
    let dir = tempfile::tempdir().unwrap();
    let index = build_fixture(dir.path(), Algorithm::Adaptive);
    let kp = VectorId(0x1000 + 4); // c5
    let filter = MetadataFilter::new().knowledge_point_id_eq(kp);
    let hits = index
        .search_filtered(&embed("交比是什么？"), 3, Some(&filter))
        .unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].id, VectorId(0x14));
    assert_eq!(hits[0].metadata.knowledge_point_id, Some(kp));
    assert_eq!(hits[0].metadata.kind.as_deref(), Some("chunk"));
}

#[test]
fn rag_fixture_survives_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let index = build_fixture(dir.path(), Algorithm::Adaptive);
    index.flush().unwrap();
    drop(index);

    let reopened = VectorIndex::open(dir.path()).unwrap();
    let hit = top1(&reopened, "平行线在射影平面在哪里相交？");
    assert_eq!(chunk_index_of(hit), 0);
}
