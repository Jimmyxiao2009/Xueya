pub fn vector_to_blob(vector: &[f32]) -> Vec<u8> {
    let mut output = Vec::with_capacity(vector.len() * 4);
    for value in vector {
        output.extend_from_slice(&value.to_le_bytes());
    }
    output
}

pub fn blob_to_vector(blob: &[u8]) -> Option<Vec<f32>> {
    if blob.len() % 4 != 0 {
        return None;
    }
    let mut output = Vec::with_capacity(blob.len() / 4);
    for chunk in blob.chunks_exact(4) {
        let bytes: [u8; 4] = chunk.try_into().ok()?;
        output.push(f32::from_le_bytes(bytes));
    }
    Some(output)
}

pub fn cosine_similarity(left: &[f32], right: &[f32]) -> Option<f32> {
    if left.len() != right.len() || left.is_empty() {
        return None;
    }
    let mut dot = 0.0f32;
    let mut left_norm = 0.0f32;
    let mut right_norm = 0.0f32;
    for (left_value, right_value) in left.iter().zip(right) {
        dot += left_value * right_value;
        left_norm += left_value * left_value;
        right_norm += right_value * right_value;
    }
    if left_norm <= f32::EPSILON || right_norm <= f32::EPSILON {
        return None;
    }
    Some(dot / (left_norm.sqrt() * right_norm.sqrt()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_vectors() {
        let vector = vec![0.25, -1.5, 3.0, 0.0];
        let blob = vector_to_blob(&vector);
        assert_eq!(blob_to_vector(&blob), Some(vector));
        assert_eq!(blob_to_vector(&[1, 2, 3]), None);
    }

    #[test]
    fn cosine_similarity_is_stable() {
        let score = cosine_similarity(&[1.0, 0.0], &[1.0, 0.0]).expect("similarity");
        assert!((score - 1.0).abs() < 0.0001);
        let score = cosine_similarity(&[1.0, 0.0], &[0.0, 1.0]).expect("similarity");
        assert!(score.abs() < 0.0001);
        assert!(cosine_similarity(&[1.0], &[1.0, 2.0]).is_none());
    }
}
