// Varg Runtime: Vector Math (Cosine Similarity, Tensor Creation)

use varg_os_types::Tensor;

/// Compute cosine similarity between two tensors
pub fn __varg_cosine_sim(a: &Tensor, b: &Tensor) -> f32 {
    let dot: f32 = a.data.iter().zip(b.data.iter()).map(|(x, y)| x * y).sum();
    let mag_a: f32 = a.data.iter().map(|v| v * v).sum::<f32>().sqrt();
    let mag_b: f32 = b.data.iter().map(|v| v * v).sum::<f32>().sqrt();
    if mag_a == 0.0 || mag_b == 0.0 { 0.0 } else { dot / (mag_a * mag_b) }
}

/// Create a Tensor from integer data
pub fn __varg_create_tensor(data: Vec<i64>) -> Tensor {
    let data_f32 = data.into_iter().map(|x| x as f32).collect::<Vec<f32>>();
    let len = data_f32.len();
    Tensor { data: data_f32, shape: vec![len] }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_sim_identical() {
        let a = Tensor { data: vec![1.0, 2.0, 3.0], shape: vec![3] };
        let b = Tensor { data: vec![1.0, 2.0, 3.0], shape: vec![3] };
        let sim = __varg_cosine_sim(&a, &b);
        assert!((sim - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_cosine_sim_orthogonal() {
        let a = Tensor { data: vec![1.0, 0.0], shape: vec![2] };
        let b = Tensor { data: vec![0.0, 1.0], shape: vec![2] };
        let sim = __varg_cosine_sim(&a, &b);
        assert!(sim.abs() < 0.001);
    }

    #[test]
    fn test_cosine_sim_zero_vector() {
        let a = Tensor { data: vec![0.0, 0.0], shape: vec![2] };
        let b = Tensor { data: vec![1.0, 1.0], shape: vec![2] };
        assert_eq!(__varg_cosine_sim(&a, &b), 0.0);
    }

    #[test]
    fn test_create_tensor() {
        let t = __varg_create_tensor(vec![1, 2, 3]);
        assert_eq!(t.data, vec![1.0, 2.0, 3.0]);
        assert_eq!(t.shape, vec![3]);
    }
}
