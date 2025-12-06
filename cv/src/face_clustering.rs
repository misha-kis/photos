use anyhow::{Context, Result};
use hdbscan::{DistanceMetric, Hdbscan, HdbscanHyperParams};

/// Configuration for HDBSCAN clustering
#[derive(Clone, Debug)]
pub struct ClusteringConfig {
    /// Minimum cluster size for HDBSCAN
    pub min_cluster_size: usize,
    /// Minimum number of samples in a neighborhood for a point to be considered a core point
    pub min_samples: Option<usize>,
}

impl Default for ClusteringConfig {
    fn default() -> Self {
        Self {
            min_cluster_size: 2,
            min_samples: None,
        }
    }
}

/// Result of clustering operation
pub struct ClusteringResult {
    /// Cluster labels for each embedding
    /// -1 indicates noise/outlier
    /// Non-negative values indicate cluster IDs
    pub labels: Vec<i32>,
    /// Number of clusters found (excluding noise)
    pub n_clusters: usize,
}

fn normalize_embedding(embedding: &mut [f32; 512]) {
    let norm: f32 = embedding.iter().map(|&x| x * x).sum::<f32>().sqrt();
    if norm > 1e-8 {
        for i in 0..512 {
            embedding[i] /= norm;
        }
    }
}

fn cosine_similarity(
    normalized_embedding1: &[f32; 512],
    normalized_embedding2: &[f32; 512],
) -> f32 {
    normalized_embedding1
        .iter()
        .zip(normalized_embedding2.iter())
        .map(|(x, y)| x * y)
        .sum::<f32>()
}

fn distance_matrix(embeddings: &[[f32; 512]]) -> Vec<Vec<f32>> {
    let normalized_embeddings = embeddings
        .iter()
        .map(|embedding| {
            let mut normalized_embedding = *embedding;
            normalize_embedding(&mut normalized_embedding);
            normalized_embedding
        })
        .collect::<Vec<[f32; 512]>>();
    let mut distance_matrix = Vec::new();
    for i in 0..embeddings.len() {
        let mut row = Vec::new();
        for j in 0..embeddings.len() {
            row.push(1.0 - cosine_similarity(&normalized_embeddings[i], &normalized_embeddings[j]));
        }
        distance_matrix.push(row);
    }
    distance_matrix
}

pub fn cluster_embeddings(
    embeddings: &[[f32; 512]],
    config: ClusteringConfig,
) -> Result<ClusteringResult> {
    if embeddings.is_empty() {
        return Ok(ClusteringResult {
            labels: Vec::new(),
            n_clusters: 0,
        });
    }

    if embeddings.len() < config.min_cluster_size {
        // All points are noise if we don't have enough for a cluster
        return Ok(ClusteringResult {
            labels: vec![-1; embeddings.len()],
            n_clusters: 0,
        });
    }

    let distance_matrix = distance_matrix(embeddings);
    let min_samples = config.min_samples.unwrap_or(config.min_cluster_size);
    let hyper_params = HdbscanHyperParams::builder()
        .min_cluster_size(config.min_cluster_size)
        .min_samples(min_samples)
        .dist_metric(DistanceMetric::Precalculated)
        .allow_single_cluster(true)
        .build();

    let clusterer = Hdbscan::new(&distance_matrix, hyper_params);
    let labels = clusterer
        .cluster()
        .context("Failed to run HDBSCAN clustering")?;

    // Count number of clusters (excluding noise/outliers with label -1)
    let n_clusters = labels
        .iter()
        .filter(|&&label| label >= 0)
        .collect::<std::collections::HashSet<_>>()
        .len();

    Ok(ClusteringResult { labels, n_clusters })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cluster_identical_embeddings() {
        // Create two identical embeddings
        let embedding1 = [1.0; 512];
        let embedding2 = [1.0; 512];
        let embeddings = vec![embedding1, embedding2];

        let config = ClusteringConfig {
            min_cluster_size: 2,
            min_samples: None,
        };

        let result = cluster_embeddings(&embeddings, config).unwrap();
        // Identical embeddings should be in the same cluster
        assert!(result.labels[0] == result.labels[1] && result.labels[0] >= 0);
        assert_eq!(result.n_clusters, 1);
    }

    #[test]
    fn test_cluster_empty() {
        let embeddings: Vec<[f32; 512]> = Vec::new();
        let config = ClusteringConfig::default();
        let result = cluster_embeddings(&embeddings, config).unwrap();
        assert_eq!(result.labels.len(), 0);
        assert_eq!(result.n_clusters, 0);
    }

    #[test]
    fn test_cluster_single_embedding() {
        let embeddings = vec![[1.0; 512]];
        let config = ClusteringConfig {
            min_cluster_size: 2,
            min_samples: None,
        };
        let result = cluster_embeddings(&embeddings, config).unwrap();
        // Single point should be noise
        assert_eq!(result.labels[0], -1);
        assert_eq!(result.n_clusters, 0);
    }
}
