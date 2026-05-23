// 向量索引模块
// 使用 fastembed crate 实现脚本的语义搜索功能
// Phase 2 功能：用户可通过自然语言描述查找相关脚本

use anyhow::Result;
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use once_cell::sync::OnceCell;
use std::sync::Mutex;

/// 默认使用的嵌入模型
/// all-MiniLM-L6-v2: 小型高效，384 维，适合本地运行
const DEFAULT_MODEL: EmbeddingModel = EmbeddingModel::AllMiniLML6V2;

/// 模型维度（AllMiniLML6V2 = 384）
const EMBEDDING_DIM: usize = 384;

/// 相似度阈值：低于此值不展示候选
const SIMILARITY_THRESHOLD: f32 = 0.6;

/// 全局嵌入模型实例（懒加载）
static EMBEDDING_MODEL: OnceCell<Mutex<TextEmbedding>> = OnceCell::new();

/// 初始化嵌入模型
/// 首次调用时会下载模型（约 20-50MB）
pub fn init_model() -> Result<()> {
    if EMBEDDING_MODEL.get().is_some() {
        return Ok(());
    }

    let model = TextEmbedding::try_new(
        InitOptions::new(DEFAULT_MODEL)
            .with_show_download_progress(true),
    )?;

    EMBEDDING_MODEL
        .set(Mutex::new(model))
        .map_err(|_| anyhow::anyhow!("嵌入模型已初始化"))?;

    Ok(())
}

/// 计算文本的向量嵌入
pub fn embed_text(text: &str) -> Result<Vec<f32>> {
    let model = EMBEDDING_MODEL
        .get()
        .ok_or_else(|| anyhow::anyhow!("嵌入模型未初始化，请先调用 init_model()"))?;

    let guard = model.lock().unwrap();
    let embeddings = guard.embed(vec![text], None)?;

    embeddings
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("嵌入结果为空"))
}

/// 批量计算文本嵌入
pub fn embed_texts(texts: Vec<String>) -> Result<Vec<Vec<f32>>> {
    let model = EMBEDDING_MODEL
        .get()
        .ok_or_else(|| anyhow::anyhow!("嵌入模型未初始化"))?;

    let guard = model.lock().unwrap();
    let embeddings = guard.embed(texts, None)?;
    Ok(embeddings)
}

/// 计算两个向量之间的余弦相似度
/// 返回值范围: [-1, 1]，通常用于文本相似度时均为正数
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let mut dot_product = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;

    for i in 0..a.len() {
        dot_product += a[i] * b[i];
        norm_a += a[i] * a[i];
        norm_b += b[i] * b[i];
    }

    let denominator = norm_a.sqrt() * norm_b.sqrt();
    if denominator == 0.0 {
        0.0
    } else {
        dot_product / denominator
    }
}

/// 语义搜索结果
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub script_id: String,
    pub similarity: f32,
}

/// 搜索最相关的脚本
/// - query: 用户输入的查询文本
/// - candidates: 候选脚本列表，每个元素为 (script_id, embedding)
/// - top_k: 返回最相关的 N 个结果
/// 返回按相似度降序排列的结果列表
pub fn search(
    query: &str,
    candidates: &[(String, Vec<f32>)],
    top_k: usize,
) -> Result<Vec<SearchResult>> {
    if candidates.is_empty() {
        return Ok(Vec::new());
    }

    let query_embedding = embed_text(query)?;

    let mut results: Vec<SearchResult> = candidates
        .iter()
        .filter_map(|(id, emb)| {
            let sim = cosine_similarity(&query_embedding, emb);
            if sim >= SIMILARITY_THRESHOLD {
                Some(SearchResult {
                    script_id: id.clone(),
                    similarity: sim,
                })
            } else {
                None
            }
        })
        .collect();

    // 按相似度降序排列
    results.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap());
    results.truncate(top_k);

    Ok(results)
}

/// 搜索最相关的脚本（使用批量嵌入）
/// 适用于初始化时批量计算所有脚本的嵌入
pub fn search_batch(
    query: &str,
    script_ids: &[String],
    script_descriptions: &[String],
    top_k: usize,
) -> Result<Vec<SearchResult>> {
    if script_ids.is_empty() || script_descriptions.is_empty() {
        return Ok(Vec::new());
    }

    let query_embedding = embed_text(query)?;
    let embeddings = embed_texts(script_descriptions.to_vec())?;

    let mut results: Vec<SearchResult> = script_ids
        .iter()
        .zip(embeddings.iter())
        .filter_map(|(id, emb)| {
            let sim = cosine_similarity(&query_embedding, emb);
            if sim >= SIMILARITY_THRESHOLD {
                Some(SearchResult {
                    script_id: id.clone(),
                    similarity: sim,
                })
            } else {
                None
            }
        })
        .collect();

    results.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap());
    results.truncate(top_k);

    Ok(results)
}

/// 将嵌入向量序列化为字节（用于数据库存储）
pub fn embedding_to_bytes(embedding: &[f32]) -> Vec<u8> {
    embedding
        .iter()
        .flat_map(|f| f.to_le_bytes())
        .collect()
}

/// 从字节反序列化嵌入向量
pub fn bytes_to_embedding(bytes: &[u8]) -> Result<Vec<f32>> {
    if bytes.len() % 4 != 0 {
        anyhow::bail!("嵌入字节长度不是 4 的倍数");
    }

    let mut result = Vec::with_capacity(bytes.len() / 4);
    for chunk in bytes.chunks_exact(4) {
        let bytes_array: [u8; 4] = [chunk[0], chunk[1], chunk[2], chunk[3]];
        result.push(f32::from_le_bytes(bytes_array));
    }

    Ok(result)
}

/// 获取嵌入维度
pub const fn embedding_dim() -> usize {
    EMBEDDING_DIM
}

/// 获取相似度阈值
pub const fn similarity_threshold() -> f32 {
    SIMILARITY_THRESHOLD
}
